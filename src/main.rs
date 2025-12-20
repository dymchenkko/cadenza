use crate::setup::start_harness;
use crate::snapshot::{create_snapshot, list_snapshots, load_snapshot};
use crate::web::start_web_server;
use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use tokio::signal;

mod config;
mod setup;
mod snapshot;
mod web;

#[derive(Parser, Debug)]
#[command(author, version, about = "Solana Dev Environment Orchestrator (Cadenza)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Starts the local validator and provisions the full environment
    Start {
        /// Path to the config file (defaults to cadenza-config.json, which is restored from snapshots)
        #[arg(short, long, default_value = "cadenza-config.json")]
        config_path: String,
        #[arg(long)]
        no_block: bool,
    },
    /// Saves the current ledger state and keypairs to a named snapshot
    Snapshot {
        /// Name of the snapshot to create
        name: String,
        /// Path to the config file (defaults to cadenza-config.json)
        #[arg(short, long, default_value = "cadenza-config.json")]
        config_path: String,
    },
    /// Loads a saved snapshot and restores the ledger state
    Load {
        /// Name of the snapshot to load
        name: String,
    },
    /// Lists all available snapshots
    ListSnapshots,
    /// Starts the web UI server
    Web {
        /// Port to run the web server on (defaults to 8080)
        #[arg(short, long, default_value = "8080")]
        port: u16,
        /// Path to the config file (defaults to cadenza-config.json)
        #[arg(short, long, default_value = "cadenza-config.json")]
        config_path: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--help-all") {
        print_full_help()?;
        return Ok(());
    }

    let cli = Cli::parse();

    match cli.command {
        Commands::Start {
            config_path,
            no_block,
        } => {
            println!("Starting Cadenza harness setup...");
            let validator_child = start_harness(&config_path).await?;

            if let Some(mut child) = validator_child {
                if no_block {
                    println!("Local validator started in non-blocking mode (no_block=true).");
                    return Ok(());
                }

                // Local cluster: keep the validator running until Ctrl+C
                signal::ctrl_c()
                    .await
                    .context("Failed to listen for Ctrl+C signal")?;

                println!("\n🛑 Stopping validator...");
                child
                    .kill()
                    .await
                    .context("Failed to stop validator process")?;

                let _ = child.wait().await;
                println!("✅ Validator stopped.");
            } else {
                println!(
                    "Cadenza finished provisioning on remote cluster; no local validator to manage."
                );
            }
        }
        Commands::Snapshot { name, config_path } => {
            println!("Creating snapshot: {name}");
            create_snapshot(&name, &config_path)?;
        }
        Commands::Load { name } => {
            println!("Loading state from snapshot: {name}");
            load_snapshot(&name)?;
        }
        Commands::ListSnapshots => {
            let snapshots = list_snapshots()?;
            if snapshots.is_empty() {
                println!("No snapshots found.");
            } else {
                println!("Available snapshots:");
                for snapshot in snapshots {
                    println!("  - {snapshot}");
                }
            }
        }
        Commands::Web { port, config_path } => {
            start_web_server(port, config_path).await?;
        }
    }

    Ok(())
}

fn print_full_help() -> Result<()> {
    let mut cmd = Cli::command();
    cmd.print_long_help()?;
    println!("\n\nSubcommand details:");

    for sub in cmd.get_subcommands_mut() {
        println!("\n--- {} ---", sub.get_name());
        sub.print_long_help()?;
    }

    println!();
    Ok(())
}
