use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub enum Cluster {
    #[serde(alias = "localnet")]
    #[default]
    Local,
    Devnet,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WalletConfig {
    pub name: String,
    pub sol_balance: f64, // Use f64 and convert to lamports later for precision
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProgramConfig {
    pub name: String,
    pub binary_path: String,
    pub program_id_path: String,
}

// --- Token Configuration ---
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TokenRecipient {
    pub wallet_name: String,
    pub amount: u64,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TokenConfig {
    pub name: String,
    pub decimals: u8,
    pub mint_authority_wallet: String, // References a wallet name from the WalletConfig list
    pub recipients: Vec<TokenRecipient>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HarnessConfig {
    #[serde(default)]
    pub cluster: Cluster,
    pub rpc_port: u16,
    pub reset_ledger: bool,
    pub wallets: Vec<WalletConfig>,
    pub programs: Vec<ProgramConfig>,
    pub tokens: Vec<TokenConfig>,
}

pub fn load_config(path: &str) -> anyhow::Result<HarnessConfig> {
    let contents = fs::read_to_string(path)?;
    let config: HarnessConfig = serde_json::from_str(&contents)?;
    Ok(config)
}
