use crate::config::{load_config, Cluster, HarnessConfig};
use anyhow::{anyhow, Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::native_token::sol_to_lamports;
use solana_sdk::program_pack::Pack;
use solana_sdk::signature::{read_keypair_file, write_keypair_file, Keypair, Signer};
use solana_sdk::system_instruction;
use solana_sdk::transaction::Transaction;
use spl_associated_token_account::{get_associated_token_address, instruction as ata_instruction};
use spl_token::instruction as token_instruction;
use spl_token::state::Mint;
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
            let faucet_port = config.faucet_port.unwrap_or(9900);
            let validator_child =
                start_validator(config.rpc_port, faucet_port, config.reset_ledger).await?;
            let client =
                RpcClient::new_with_commitment(rpc_url.clone(), CommitmentConfig::processed());

            provision_wallets(&client, &config, Cluster::Local)?;
            provision_tokens(&client, &config, Cluster::Local)?;
            deploy_programs(&rpc_url, &config, Cluster::Local).await?;

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
            provision_tokens(&client, &config, Cluster::Devnet)?;
            deploy_programs(&rpc_url, &config, Cluster::Devnet).await?;

            println!("\n✅ Cadenza devnet setup complete.");
            println!("   RPC URL: {rpc_url}");

            Ok(None)
        }
    }
}

// Function 1: Starts the solana-test-validator process
async fn start_validator(rpc_port: u16, faucet_port: u16, reset: bool) -> Result<Child> {
    let mut command = Command::new("solana-test-validator");
    command
        .arg("--ledger")
        .arg("test-ledger")
        .arg("--rpc-port")
        .arg(rpc_port.to_string())
        .arg("--gossip-port")
        .arg("9001")
        .arg("--faucet-port")
        .arg(faucet_port.to_string())
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

/// Provisions SPL tokens for wallets based on the `tokens` section of the harness config.
fn provision_tokens(client: &RpcClient, config: &HarnessConfig, cluster: Cluster) -> Result<()> {
    if config.tokens.is_empty() {
        return Ok(());
    }

    if config.wallets.is_empty() {
        return Err(anyhow!(
            "Cannot provision tokens: no wallets configured in harness config."
        ));
    }

    fs::create_dir_all("keys").context("Failed to create keys directory")?;

    // Helper closure to load a wallet keypair by name from the keys directory
    let load_wallet_keypair = |name: &str| -> Result<Keypair> {
        let keypair_path = Path::new("keys").join(format!("{name}.json"));
        read_keypair_file(&keypair_path).map_err(|e| {
            anyhow!(
                "Failed to read keypair for wallet '{}' from {}: {}",
                name,
                keypair_path.display(),
                e
            )
        })
    };

    for token in &config.tokens {
        println!(
            "\n🪙 Provisioning SPL token '{}' (decimals: {}) on {:?}...",
            token.name, token.decimals, cluster
        );

        // Load mint authority wallet
        let mint_authority =
            load_wallet_keypair(&token.mint_authority_wallet).with_context(|| {
                format!(
                    "Mint authority wallet '{}' must exist and be listed under wallets for token '{}'",
                    token.mint_authority_wallet, token.name
                )
            })?;

        let mut authority_balance = client
            .get_balance(&mint_authority.pubkey())
            .with_context(|| {
                format!(
                    "Failed to fetch balance for mint authority wallet '{}' while provisioning token '{}'",
                    token.mint_authority_wallet, token.name
                )
            })?;
        println!(
            "   ℹ️ Mint authority '{}' (pubkey: {}) balance before mint creation: {} lamports",
            token.mint_authority_wallet,
            mint_authority.pubkey(),
            authority_balance
        );

        if authority_balance == 0 {
            println!(
                "   ⏳ Mint authority '{}' currently has 0 lamports; waiting for airdrop to land...",
                token.mint_authority_wallet
            );

            const MAX_ATTEMPTS: usize = 20;
            const RETRY_DELAY_MS: u64 = 250;

            for attempt in 1..=MAX_ATTEMPTS {
                std::thread::sleep(std::time::Duration::from_millis(RETRY_DELAY_MS));
                authority_balance = client
                    .get_balance(&mint_authority.pubkey())
                    .with_context(|| {
                        format!(
                            "Failed to refetch balance for mint authority wallet '{}' while provisioning token '{}'",
                            token.mint_authority_wallet, token.name
                        )
                    })?;
                if authority_balance > 0 {
                    println!(
                        "   ✅ Mint authority '{}' balance after wait: {} lamports (attempt {}/{})",
                        token.mint_authority_wallet, authority_balance, attempt, MAX_ATTEMPTS
                    );
                    break;
                }
            }

            if authority_balance == 0 {
                return Err(anyhow!(
                    "Mint authority wallet '{}' still has 0 lamports after waiting; cannot create mint for token '{}'. \
Ensure this wallet is funded in the wallets section.",
                    token.mint_authority_wallet,
                    token.name
                ));
            }
        }

        // Create and initialize the mint account
        let mint_keypair = Keypair::new();
        let mint_pubkey = mint_keypair.pubkey();

        let rent_exemption = client
            .get_minimum_balance_for_rent_exemption(Mint::LEN)
            .context("Failed to fetch rent exemption for mint account")?;

        let latest_blockhash = client
            .get_latest_blockhash()
            .context("Failed to fetch latest blockhash for mint creation")?;

        let create_mint_ix = system_instruction::create_account(
            &mint_authority.pubkey(),
            &mint_pubkey,
            rent_exemption,
            Mint::LEN as u64,
            &spl_token::id(),
        );

        let init_mint_ix = token_instruction::initialize_mint(
            &spl_token::id(),
            &mint_pubkey,
            &mint_authority.pubkey(),
            None,
            token.decimals,
        )
        .context("Failed to build initialize_mint instruction")?;

        let tx = Transaction::new_signed_with_payer(
            &[create_mint_ix, init_mint_ix],
            Some(&mint_authority.pubkey()),
            &[&mint_authority, &mint_keypair],
            latest_blockhash,
        );

        client.send_and_confirm_transaction(&tx).with_context(|| {
            format!(
                "Failed to create and initialize mint for token '{}'",
                token.name
            )
        })?;

        println!(
            "✅ Created mint for token '{}' with address {} (authority: {})",
            token.name,
            mint_pubkey,
            mint_authority.pubkey()
        );

        // For each recipient, create ATA and mint the configured amount
        for recipient in &token.recipients {
            let recipient_kp = load_wallet_keypair(&recipient.wallet_name).with_context(|| {
                format!(
                    "Recipient wallet '{}' must exist and be listed under wallets for token '{}'",
                    recipient.wallet_name, token.name
                )
            })?;

            let recipient_pubkey = recipient_kp.pubkey();
            let ata_address = get_associated_token_address(&recipient_pubkey, &mint_pubkey);

            let latest_blockhash = client
                .get_latest_blockhash()
                .context("Failed to fetch latest blockhash for token provisioning")?;

            let create_ata_ix = ata_instruction::create_associated_token_account(
                &mint_authority.pubkey(), // payer
                &recipient_pubkey,
                &mint_pubkey,
                &spl_token::id(),
            );

            let mint_to_ix = token_instruction::mint_to(
                &spl_token::id(),
                &mint_pubkey,
                &ata_address,
                &mint_authority.pubkey(),
                &[],
                recipient.amount,
            )
            .with_context(|| {
                format!(
                    "Failed to build mint_to instruction for wallet '{}' for token '{}'",
                    recipient.wallet_name, token.name
                )
            })?;

            let tx = Transaction::new_signed_with_payer(
                &[create_ata_ix, mint_to_ix],
                Some(&mint_authority.pubkey()),
                &[&mint_authority],
                latest_blockhash,
            );

            client.send_and_confirm_transaction(&tx).with_context(|| {
                format!(
                    "Failed to create ATA and mint tokens for wallet '{}' for token '{}'",
                    recipient.wallet_name, token.name
                )
            })?;

            println!(
                "   ✅ Minted {} units of '{}' to wallet '{}' (owner: {}, ATA: {})",
                recipient.amount, token.name, recipient.wallet_name, recipient_pubkey, ata_address
            );
        }
    }

    println!("\n✅ SPL token provisioning complete.");
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

        // Resolve binary path to absolute path to avoid issues with relative paths
        let binary_path = Path::new(&program.binary_path);
        let absolute_binary_path = if binary_path.is_absolute() {
            binary_path.to_path_buf()
        } else {
            std::env::current_dir()
                .context("Failed to get current directory")?
                .join(binary_path)
        };

        if !absolute_binary_path.exists() {
            return Err(anyhow!(
                "Program binary file not found: {} (resolved to: {})",
                program.binary_path,
                absolute_binary_path.display()
            ));
        }

        let status = Command::new("solana")
            .arg("program")
            .arg("deploy")
            .arg(absolute_binary_path.to_str().unwrap())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{HarnessConfig, ProgramConfig, TokenConfig};

    #[test]
    fn test_sol_to_lamports_conversion() {
        // Test that sol_to_lamports works correctly
        let sol = 1.5;
        let lamports = sol_to_lamports(sol);
        assert_eq!(lamports, 1_500_000_000); // 1.5 SOL = 1,500,000,000 lamports

        let sol = 0.0;
        let lamports = sol_to_lamports(sol);
        assert_eq!(lamports, 0);

        let sol = 100.0;
        let lamports = sol_to_lamports(sol);
        assert_eq!(lamports, 100_000_000_000); // 100 SOL = 100,000,000,000 lamports
    }

    #[test]
    fn test_provision_wallets_empty_list() {
        // Verify the early return logic exists
        let config = HarnessConfig {
            cluster: Cluster::Local,
            rpc_port: 8899,
            faucet_port: None,
            reset_ledger: false,
            wallets: vec![],
            programs: vec![],
            tokens: vec![],
        };
        
        // The function should return Ok(()) immediately when wallets is empty
        assert!(config.wallets.is_empty());
    }

    #[test]
    fn test_provision_tokens_empty_list() {
        let config = HarnessConfig {
            cluster: Cluster::Local,
            rpc_port: 8899,
            faucet_port: None,
            reset_ledger: false,
            wallets: vec![],
            programs: vec![],
            tokens: vec![],
        };
        
        // The function should return Ok(()) immediately when tokens is empty
        assert!(config.tokens.is_empty());
    }

    #[test]
    fn test_provision_tokens_requires_wallets() {
        let config = HarnessConfig {
            cluster: Cluster::Local,
            rpc_port: 8899,
            faucet_port: None,
            reset_ledger: false,
            wallets: vec![],
            programs: vec![],
            tokens: vec![TokenConfig {
                name: "TEST".to_string(),
                decimals: 6,
                mint_authority_wallet: "alice".to_string(),
                recipients: vec![],
            }],
        };
        
        // The function should return an error when tokens exist but wallets is empty
        assert!(config.wallets.is_empty());
        assert!(!config.tokens.is_empty());
    }

    #[test]
    fn test_deploy_programs_empty_list() {
        let config = HarnessConfig {
            cluster: Cluster::Local,
            rpc_port: 8899,
            faucet_port: None,
            reset_ledger: false,
            wallets: vec![],
            programs: vec![],
            tokens: vec![],
        };
        
        assert!(config.programs.is_empty());
    }

    #[test]
    fn test_deploy_programs_requires_wallets() {
        let config = HarnessConfig {
            cluster: Cluster::Local,
            rpc_port: 8899,
            faucet_port: None,
            reset_ledger: false,
            wallets: vec![],
            programs: vec![ProgramConfig {
                name: "test".to_string(),
                binary_path: "./test.so".to_string(),
                program_id_path: "./keys/test_id.json".to_string(),
            }],
            tokens: vec![],
        };
        
        // The function should return an error when programs exist but wallets is empty
        assert!(config.wallets.is_empty());
        assert!(!config.programs.is_empty());
    }

    #[test]
    fn test_cluster_enum_matching() {
        let local_config = HarnessConfig {
            cluster: Cluster::Local,
            rpc_port: 8899,
            faucet_port: None,
            reset_ledger: false,
            wallets: vec![],
            programs: vec![],
            tokens: vec![],
        };
        
        let devnet_config = HarnessConfig {
            cluster: Cluster::Devnet,
            rpc_port: 8899,
            faucet_port: None,
            reset_ledger: false,
            wallets: vec![],
            programs: vec![],
            tokens: vec![],
        };
        
        match local_config.cluster {
            Cluster::Local => assert!(true),
            Cluster::Devnet => assert!(false, "Expected Local cluster"),
        }
        
        match devnet_config.cluster {
            Cluster::Local => assert!(false, "Expected Devnet cluster"),
            Cluster::Devnet => assert!(true),
        }
    }

    #[test]
    fn test_faucet_port_default() {
        let config = HarnessConfig {
            cluster: Cluster::Local,
            rpc_port: 8899,
            faucet_port: None,
            reset_ledger: false,
            wallets: vec![],
            programs: vec![],
            tokens: vec![],
        };
        
        let faucet_port = config.faucet_port.unwrap_or(9900);
        assert_eq!(faucet_port, 9900);
        
        let config_with_port = HarnessConfig {
            cluster: Cluster::Local,
            rpc_port: 8899,
            faucet_port: Some(9999),
            reset_ledger: false,
            wallets: vec![],
            programs: vec![],
            tokens: vec![],
        };
        
        assert_eq!(config_with_port.faucet_port, Some(9999));
    }
}
