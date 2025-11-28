use crate::config::load_config;
use anyhow::{Context, Result, anyhow};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::time::{Duration, sleep};

const LOCAL_RPC_URL: &str = "http://127.0.0.1";

// Main function to run the entire setup
pub async fn start_harness(config_path: &str) -> Result<Child> {
    let config = load_config(config_path).context("Failed to load harness configuration")?;

    let validator_child = start_validator(config.rpc_port, config.reset_ledger).await?;

    // TODO: Implement wallet provisioning
    // Loop through config.wallets, generate keypairs, save to disk, and request airdrops.

    // TODO: Implement program deployment
    // Loop through config.programs, run `solana program deploy` using tokio::process::Command.

    // TODO: Implement token creation
    // Loop through config.tokens, create mints, create associated token accounts, and mint tokens.

    println!(
        "\n✅ Cadenza setup complete. Validator is running on http://127.0.0.1:{}",
        config.rpc_port
    );
    println!("Press Ctrl+C to stop the validator...");

    Ok(validator_child)
}

// Function 1: Starts the solana-test-validator process
async fn start_validator(rpc_port: u16, reset: bool) -> Result<Child> {
    let mut command = Command::new("solana-test-validator");
    command
        .arg("--ledger")
        .arg("test-ledger")
        .arg("--rpc-port")
        .arg(rpc_port.to_string())
        .arg("--gossip-port")
        .arg("9001")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if reset {
        command.arg("--reset");
    }

    let mut child = command.spawn().with_context(
        || "Failed to launch solana-test-validator. Is it installed and on your PATH?",
    )?;

    // Give the process a moment to start, then check if it's still alive
    sleep(Duration::from_millis(500)).await;

    if let Some(status) = child.try_wait()? {
        return Err(anyhow!(
            "solana-test-validator exited immediately with status: {:?}. Check the error messages above.",
            status
        ));
    }

    wait_for_validator_ready(rpc_port).await?;

    Ok(child)
}

// Function 2: Polls the RPC endpoint until it responds
async fn wait_for_validator_ready(rpc_port: u16) -> Result<()> {
    let rpc_url = format!("{}:{}", LOCAL_RPC_URL, rpc_port);
    let client = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::processed());

    const MAX_ATTEMPTS: usize = 40;
    const RETRY_DELAY_MS: u64 = 500;

    for attempt in 1..=MAX_ATTEMPTS {
        match client.get_health() {
            Ok(_) => {
                println!("Validator healthy on {}", rpc_url);
                return Ok(());
            }
            Err(err) => {
                if attempt == MAX_ATTEMPTS {
                    return Err(anyhow!(
                        "Validator on {rpc_url} failed to become healthy after {} attempts: {err}",
                        MAX_ATTEMPTS
                    ));
                }
                sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
            }
        }
    }

    Err(anyhow!(
        "Validator readiness loop exited unexpectedly for {rpc_url}"
    ))
}

// Function 3: Creates a keypair and funds it (Crucial step)
/* pub async fn fund_wallet(client: &RpcClient, config: &WalletConfig) -> Result<()> {
    // 1. Generate Keypair
    let keypair = solana_sdk::signature::Keypair::new();
    // 2. Airdrop SOL (Request Airdrop logic using client.request_airdrop)
    // ...
    // 3. Save keypair to disk
    // ...
    Ok(())
} */
