use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

const LEDGER_DIR: &str = "test-ledger";
const SNAPSHOTS_DIR: &str = "snapshots";

#[derive(Serialize, Deserialize)]
struct SnapshotMetadata {
    name: String,
    created_at: chrono::DateTime<chrono::Utc>,
    config_path: String,
}

/// Creates a snapshot of the current ledger state and configuration
pub fn create_snapshot(name: &str, config_path: &str) -> Result<()> {
    validate_snapshot_name(name)?;

    if is_validator_running()? {
        return Err(anyhow::anyhow!(
            "Validator is currently running. Please stop it first before creating a snapshot."
        ));
    }

    let lock_file = Path::new(LEDGER_DIR).join("rocksdb").join("LOCK");
    if lock_file.exists() {
        println!("⚠️  Removing stale LOCK file (validator is not running)...");
        fs::remove_file(&lock_file).context("Failed to remove stale LOCK file")?;
    }

    let snapshot_dir = PathBuf::from(SNAPSHOTS_DIR).join(name);
    if snapshot_dir.exists() {
        return Err(anyhow::anyhow!(
            "Snapshot '{}' already exists. Use a different name or delete the existing snapshot first.",
            name
        ));
    }
    fs::create_dir_all(&snapshot_dir).context("Failed to create snapshot directory")?;

    let ledger_path = Path::new(LEDGER_DIR);
    if !ledger_path.exists() {
        return Err(anyhow::anyhow!(
            "Ledger directory '{}' does not exist. Start the validator first to create a ledger.",
            LEDGER_DIR
        ));
    }

    let snapshot_ledger = snapshot_dir.join(LEDGER_DIR);
    copy_directory(ledger_path, &snapshot_ledger).context("Failed to copy ledger directory")?;

    if Path::new(config_path).exists() {
        let config_dest = snapshot_dir.join("cadenza-config.json");
        fs::copy(config_path, &config_dest).context("Failed to copy config file")?;
    }

    let metadata = SnapshotMetadata {
        name: name.to_string(),
        created_at: chrono::Utc::now(),
        config_path: config_path.to_string(),
    };
    let metadata_path = snapshot_dir.join("metadata.json");
    let metadata_json =
        serde_json::to_string_pretty(&metadata).context("Failed to serialize metadata")?;
    fs::write(&metadata_path, metadata_json).context("Failed to write metadata")?;

    println!(
        "✅ Snapshot '{}' created successfully at {}",
        name,
        snapshot_dir.display()
    );
    Ok(())
}

/// Loads a saved snapshot and restores the ledger state
pub fn load_snapshot(name: &str) -> Result<()> {
    let snapshot_dir = PathBuf::from(SNAPSHOTS_DIR).join(name);
    if !snapshot_dir.exists() {
        return Err(anyhow::anyhow!("Snapshot '{}' does not exist", name));
    }

    if is_validator_running()? {
        return Err(anyhow::anyhow!(
            "Validator is currently running. Please stop it first before loading a snapshot."
        ));
    }

    let lock_file = Path::new(LEDGER_DIR).join("rocksdb").join("LOCK");
    if lock_file.exists() {
        println!("⚠️  Removing stale LOCK file (validator is not running)...");
        fs::remove_file(&lock_file).context("Failed to remove stale LOCK file")?;
    }

    //Backup current ledger
    let current_ledger = Path::new(LEDGER_DIR);
    if current_ledger.exists() {
        let backup_name = format!("backup-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
        let backup_path = PathBuf::from(SNAPSHOTS_DIR).join(&backup_name);
        println!("📦 Backing up current ledger to '{}'...", backup_name);
        copy_directory(current_ledger, &backup_path).context("Failed to backup current ledger")?;
        println!("✅ Backup complete");
    }

    let snapshot_ledger = snapshot_dir.join(LEDGER_DIR);
    if snapshot_ledger.exists() {
        if current_ledger.exists() {
            fs::remove_dir_all(current_ledger).context("Failed to remove current ledger")?;
        }
        copy_directory(&snapshot_ledger, current_ledger)
            .context("Failed to restore ledger from snapshot")?;
    } else {
        return Err(anyhow::anyhow!(
            "Snapshot '{}' does not contain a ledger directory",
            name
        ));
    }

    let snapshot_config = snapshot_dir.join("cadenza-config.json");
    if snapshot_config.exists() {
        let config_dest = Path::new("cadenza-config.json");
        if config_dest.exists() {
            // Backup current config
            let backup_config = format!(
                "harness-config.json.backup-{}",
                chrono::Utc::now().format("%Y%m%d-%H%M%S")
            );
            fs::copy(config_dest, &backup_config).context("Failed to backup current config")?;
            println!("📝 Backed up current config to '{}'", backup_config);
        }
        fs::copy(&snapshot_config, config_dest).context("Failed to restore config")?;
        println!("📝 Restored config from snapshot");
        println!(
            "💡 You can now run 'start' without specifying a config path to use the snapshot's config"
        );
    } else {
        println!(
            "⚠️  Snapshot does not contain a config file. You'll need to specify a config path when starting."
        );
    }

    println!("✅ Snapshot '{}' loaded successfully", name);
    Ok(())
}

/// Lists all available snapshots
pub fn list_snapshots() -> Result<Vec<String>> {
    let snapshots_path = Path::new(SNAPSHOTS_DIR);
    if !snapshots_path.exists() {
        return Ok(Vec::new());
    }

    let mut snapshots = Vec::new();
    for entry in fs::read_dir(snapshots_path).context("Failed to read snapshots directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                // Skip backup directories
                if !name.starts_with("backup-") {
                    snapshots.push(name.to_string());
                }
            }
        }
    }

    snapshots.sort();
    Ok(snapshots)
}

/// Checks if solana-test-validator process is actually running
fn is_validator_running() -> Result<bool> {
    let pgrep_output = Command::new("pgrep")
        .arg("-f")
        .arg("solana-test-validator")
        .output();

    if let Ok(result) = pgrep_output {
        if result.status.success() {
            // Check if output contains any process IDs (non-empty stdout)
            let stdout = String::from_utf8_lossy(&result.stdout);
            return Ok(!stdout.trim().is_empty());
        }
    }

    // Fallback to ps + grep if pgrep fails or is not available
    let ps_output = Command::new("ps").arg("aux").output();

    match ps_output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let lines: Vec<&str> = stdout
                .lines()
                .filter(|line| line.contains("solana-test-validator"))
                .filter(|line| !line.contains("grep"))
                .collect();
            Ok(!lines.is_empty())
        }
        _ => Ok(false),
    }
}

fn validate_snapshot_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(anyhow::anyhow!("Snapshot name cannot be empty"));
    }
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err(anyhow::anyhow!(
            "Invalid snapshot name: '{}'. Name cannot contain path separators or be '.' or '..'",
            name
        ));
    }
    Ok(())
}

fn copy_directory(src: &Path, dst: &Path) -> Result<()> {
    if !src.is_dir() {
        return Err(anyhow::anyhow!(
            "Source is not a directory: {}",
            src.display()
        ));
    }

    fs::create_dir_all(dst).context("Failed to create destination directory")?;

    for entry in WalkDir::new(src) {
        let entry = entry?;
        let path = entry.path();
        let relative = path
            .strip_prefix(src)
            .context("Failed to get relative path")?;
        let dest_path = dst.join(relative);

        if path.is_dir() {
            fs::create_dir_all(&dest_path)?;
        } else if path.is_file() {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &dest_path).with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    path.display(),
                    dest_path.display()
                )
            })?;
        }
    }

    Ok(())
}
