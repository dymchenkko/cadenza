use crate::setup::start_harness;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
//use crate::snapshot::{create_snapshot, load_snapshot}; // Placeholder for snapshot functions
use tokio::signal;

mod config;
mod setup;
mod snapshot;

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
        #[arg(short, long)]
        config_path: String,
    },
    /*/// Saves the current ledger state and keypairs to a named snapshot
    Snapshot {
        /// Name of the snapshot to create
        name: String,
    },
    /// Loads a saved snapshot and starts the validator from that state
    Load {
        /// Name of the snapshot to load
        name: String,
    }*/
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start { config_path } => {
            println!("Starting Cadenza harness setup...");
            let mut validator_child = start_harness(&config_path).await?;

            signal::ctrl_c()
                .await
                .context("Failed to listen for Ctrl+C signal")?;

            println!("\n🛑 Stopping validator...");
            validator_child
                .kill()
                .await
                .context("Failed to stop validator process")?;

            let _ = validator_child.wait().await;
            println!("✅ Validator stopped.");
        } /*Commands::Snapshot { name } => {
              println!("Creating snapshot: {}", name);
              create_snapshot(&name)?;
          }*/
          /*Commands::Load { name } => {
              println!("Loading state from snapshot: {}", name);
              load_snapshot(&name)?;
          }*/
    }

    Ok(())
}
