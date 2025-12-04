//! NOTE:
//! - Tests are marked `#[ignore]` because they require `solana-test-validator`
//!   on PATH and are relatively slow.
//! - Run explicitly with:
//!       cargo test --test token_provisioning -- --ignored

use anyhow::Result;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[test]
#[ignore]
fn provisions_spl_tokens_for_recipients() -> Result<()> {
    // Use a dedicated, checked-in config file for this test harness.
    let config_path = PathBuf::from("tests/cadenza-provision_spl_tokens_for_recipients.json");

    let cadenza_bin = env!("CARGO_BIN_EXE_cadenza");

    let child = Command::new(cadenza_bin)
        .arg("start")
        .arg("--config-path")
        .arg(&config_path)
        .arg("--no-block")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    struct ChildGuard(std::process::Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
        }
    }
    let _guard = ChildGuard(child);

    let alice_keypair_path = PathBuf::from("keys").join("alice.json");

    const MAX_ATTEMPTS: usize = 60;
    const RETRY_DELAY_MS: u64 = 1000;

    let mut found_correct_balance = false;

    for _ in 0..MAX_ATTEMPTS {
        if alice_keypair_path.exists() {
            let output = Command::new("spl-token")
                .arg("accounts")
                .arg("--owner")
                .arg(&alice_keypair_path)
                .arg("--url")
                .arg("http://127.0.0.1:8899")
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines() {
                        if line.trim().is_empty() || line.starts_with("Token") {
                            continue;
                        }
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 && parts[1] == "1" {
                            found_correct_balance = true;
                            break;
                        }
                    }

                    if found_correct_balance {
                        break;
                    }
                }
            }
        }

        thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
    }

    assert!(
        found_correct_balance,
        "Alice did not receive the expected SPL token balance in time"
    );

    // Best-effort: stop any solana-test-validator started by this test.
    let _ = Command::new("pkill")
        .arg("solana-test-validator")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    Ok(())
}
