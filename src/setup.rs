use crate::config::{load_config, Cluster, HarnessConfig};
use anyhow::{anyhow, Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::native_token::sol_to_lamports;
use solana_sdk::signature::{read_keypair_file, write_keypair_file, Keypair, Signer};
use std::fs;
use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::time::{sleep, Duration};

const LOCAL_RPC_URL: &str = "http://127.0.0.1";
const DEVNET_RPC_URL: &str = "https://api.devnet.solana.com";

// Main function to run the entire setup
pub async fn start_harness(config_path: &str) -> Result<Option<Child>> {
    let config = load_config(config_path).context("Failed to load harness configuration")?;

    match config.cluster {
        Cluster::Local => {
            let rpc_url = format!("{}:{}", LOCAL_RPC_URL, config.rpc_port);
            let validator_child = start_validator(config.rpc_port, config.reset_ledger).await?;
            let client =
                RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::processed());

            provision_wallets(&client, &config, Cluster::Local)?;
            deploy_programs(&rpc_url, &config, Cluster::Local).await?;

            // TODO: Implement token creation for local validator
            // Loop through config.tokens, create mints, create associated token accounts,
            // and mint tokens using the local validator RPC.

            println!(
                "\n✅ Cadenza (local) setup complete. Validator is running on http://127.0.0.1:{}",
                config.rpc_port
            );
            println!("Press Ctrl+C to stop the validator...");

            Ok(Some(validator_child))
        }
        Cluster::Devnet => {
            println!("🌐 Using Solana Devnet (no local validator will be started)...");

            let rpc_url = DEVNET_RPC_URL.to_string();
            let client =
                RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::processed());

            provision_wallets(&client, &config, Cluster::Devnet)?;
            deploy_programs(&rpc_url, &config, Cluster::Devnet).await?;

            // TODO: Implement token creation & minting against devnet RPC if desired.

            println!("\n✅ Cadenza devnet setup complete.");
            println!("   RPC URL: {rpc_url}");

            Ok(None)
        }
    }
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

    let mut child = command.spawn().with_context(|| {
        "Failed to launch solana-test-validator. Is it installed and on your PATH?"
    })?;

    // Give the process a moment to start, then check if it's still alive
    sleep(Duration::from_millis(500)).await;

    if let Some(status) = child.try_wait()? {
        return Err(anyhow!(
            "solana-test-validator exited immediately with status: {status:?}. \
Check the error messages above."
        ));
    }

    wait_for_validator_ready(rpc_port).await?;

    Ok(child)
}

// Function 2: Polls the RPC endpoint until it responds
async fn wait_for_validator_ready(rpc_port: u16) -> Result<()> {
    let rpc_url = format!("{LOCAL_RPC_URL}:{rpc_port}");
    let client = RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::processed());

    const MAX_ATTEMPTS: usize = 40;
    const RETRY_DELAY_MS: u64 = 500;

    for attempt in 1..=MAX_ATTEMPTS {
        match client.get_health() {
            Ok(_) => {
                println!("Validator healthy on {rpc_url}");
                return Ok(());
            }
            Err(err) => {
                if attempt == MAX_ATTEMPTS {
                    return Err(anyhow!(
                        "Validator on {rpc_url} failed to become healthy after {MAX_ATTEMPTS} attempts: {err}"
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

fn provision_wallets(client: &RpcClient, config: &HarnessConfig, cluster: Cluster) -> Result<()> {
    if config.wallets.is_empty() {
        return Ok(());
    }

    fs::create_dir_all("keys").context("Failed to create keys directory")?;

    for wallet in &config.wallets {
        let keypair_path = Path::new("keys").join(format!("{}.json", wallet.name));

        let keypair = if keypair_path.exists() {
            read_keypair_file(&keypair_path).map_err(|e| {
                anyhow!(
                    "Failed to read existing keypair for wallet '{}' from {}: {}",
                    wallet.name,
                    keypair_path.display(),
                    e
                )
            })?
        } else {
            let kp = Keypair::new();
            write_keypair_file(&kp, &keypair_path).map_err(|e| {
                anyhow!(
                    "Failed to write new keypair for wallet '{}' to {}: {}",
                    wallet.name,
                    keypair_path.display(),
                    e
                )
            })?;
            println!(
                "🔑 Created new keypair for wallet '{}' at {}",
                wallet.name,
                keypair_path.display()
            );
            kp
        };

        let lamports = sol_to_lamports(wallet.sol_balance);
        println!(
            "💸 Requesting airdrop of {} SOL to '{}' ({}) on {:?}...",
            wallet.sol_balance,
            wallet.name,
            keypair.pubkey(),
            cluster
        );

        match client.request_airdrop(&keypair.pubkey(), lamports) {
            Ok(sig) => {
                if let Err(e) = client.confirm_transaction(&sig) {
                    return Err(anyhow!("Failed to confirm airdrop transaction: {e}"));
                }

                println!(
                    "✅ Funded wallet '{}' with {} SOL (pubkey: {})",
                    wallet.name,
                    wallet.sol_balance,
                    keypair.pubkey()
                );
            }
            Err(e) => {
                let msg = e.to_string();
                if matches!(cluster, Cluster::Devnet) && msg.to_lowercase().contains("rate limit") {
                    println!(
                        "⚠️  Airdrop for wallet '{}' was rate-limited on Devnet: {msg}. \
Continuing without treating this as a hard error (you may already have enough SOL).",
                        wallet.name
                    );
                } else {
                    return Err(anyhow!("Failed to request airdrop: {e}"));
                }
            }
        }
    }

    Ok(())
}

async fn deploy_programs(rpc_url: &str, config: &HarnessConfig, cluster: Cluster) -> Result<()> {
    if config.programs.is_empty() {
        return Ok(());
    }

    if config.wallets.is_empty() {
        return Err(anyhow!(
            "No wallets configured; at least one wallet is required to act as the deploy payer."
        ));
    }

    let payer_wallet = &config.wallets[0];
    let payer_keypair_path = Path::new("keys").join(format!("{}.json", payer_wallet.name));
    if !payer_keypair_path.exists() {
        return Err(anyhow!(
            "Payer keypair file '{}' for wallet '{}' does not exist. It should have been created during wallet provisioning.",
            payer_keypair_path.display(),
            payer_wallet.name
        ));
    }

    for program in &config.programs {
        let program_id_path = Path::new(&program.program_id_path);

        if !program_id_path.exists() {
            fs::create_dir_all(program_id_path.parent().unwrap_or_else(|| Path::new(".")))
                .context("Failed to create program ID directory")?;

            println!(
                "🔐 Generating new program ID keypair for '{}' at {}...",
                program.name,
                program_id_path.display()
            );

            let status = Command::new("solana-keygen")
                .arg("new")
                .arg("-o")
                .arg(&program.program_id_path)
                .arg("--no-bip39-passphrase")
                .stdin(Stdio::null())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .await
                .context("Failed to run solana-keygen")?;

            if !status.success() {
                return Err(anyhow!(
                    "solana-keygen failed for program '{}' with status: {:?}",
                    program.name,
                    status
                ));
            }
        }

        println!(
            "🚀 Deploying program '{}' from {} with program ID file {} to {} using payer wallet '{}' ({})...",
            program.name,
            program.binary_path,
            program.program_id_path,
            rpc_url,
            payer_wallet.name,
            payer_keypair_path.display()
        );

        let status = Command::new("solana")
            .arg("program")
            .arg("deploy")
            .arg(&program.binary_path)
            .arg("--program-id")
            .arg(&program.program_id_path)
            .arg("--url")
            .arg(rpc_url)
            .arg("--keypair")
            .arg(&payer_keypair_path)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
            .context("Failed to run solana program deploy")?;

        if !status.success() {
            return Err(anyhow!(
                "solana program deploy failed for '{}' with status: {:?}",
                program.name,
                status
            ));
        }

        println!(
            "✅ Program '{}' deployed successfully to {:?}.",
            program.name, cluster
        );
    }

    Ok(())
}
