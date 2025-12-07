use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
#[serde(rename_all = "lowercase")]
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
    #[serde(default)]
    pub faucet_port: Option<u16>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_cluster_default() {
        let cluster: Cluster = Default::default();
        assert!(matches!(cluster, Cluster::Local));
    }

    #[test]
    fn test_cluster_deserialization() {
        // Test "local" (lowercase, default serde behavior)
        let json = r#""local""#;
        let cluster: Cluster = serde_json::from_str(json).unwrap();
        assert!(matches!(cluster, Cluster::Local));

        // Test "localnet" alias
        let json = r#""localnet""#;
        let cluster: Cluster = serde_json::from_str(json).unwrap();
        assert!(matches!(cluster, Cluster::Local));

        // Test "devnet" (lowercase, default serde behavior)
        let json = r#""devnet""#;
        let cluster: Cluster = serde_json::from_str(json).unwrap();
        assert!(matches!(cluster, Cluster::Devnet));
    }

    #[test]
    fn test_load_config_minimal() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test-config.json");
        
        let config_json = r#"{
            "rpcPort": 8899,
            "resetLedger": true,
            "wallets": [],
            "programs": [],
            "tokens": []
        }"#;
        
        fs::write(&config_path, config_json).unwrap();
        
        let config = load_config(config_path.to_str().unwrap()).unwrap();
        assert_eq!(config.rpc_port, 8899);
        assert!(config.reset_ledger);
        assert!(matches!(config.cluster, Cluster::Local)); // default
        assert!(config.wallets.is_empty());
        assert!(config.programs.is_empty());
        assert!(config.tokens.is_empty());
    }

    #[test]
    fn test_load_config_full() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test-config.json");
        
        let config_json = r#"{
            "cluster": "devnet",
            "rpcPort": 8899,
            "faucetPort": 9900,
            "resetLedger": false,
            "wallets": [
                {
                    "name": "alice",
                    "solBalance": 50.5
                }
            ],
            "programs": [
                {
                    "name": "test_program",
                    "binaryPath": "./target/deploy/test.so",
                    "programIdPath": "./keys/test_id.json"
                }
            ],
            "tokens": [
                {
                    "name": "TEST_TOKEN",
                    "decimals": 6,
                    "mintAuthorityWallet": "alice",
                    "recipients": [
                        {
                            "walletName": "alice",
                            "amount": 1000000
                        }
                    ]
                }
            ]
        }"#;
        
        fs::write(&config_path, config_json).unwrap();
        
        let config = load_config(config_path.to_str().unwrap()).unwrap();
        assert!(matches!(config.cluster, Cluster::Devnet));
        assert_eq!(config.rpc_port, 8899);
        assert_eq!(config.faucet_port, Some(9900));
        assert!(!config.reset_ledger);
        assert_eq!(config.wallets.len(), 1);
        assert_eq!(config.wallets[0].name, "alice");
        assert_eq!(config.wallets[0].sol_balance, 50.5);
        assert_eq!(config.programs.len(), 1);
        assert_eq!(config.programs[0].name, "test_program");
        assert_eq!(config.tokens.len(), 1);
        assert_eq!(config.tokens[0].name, "TEST_TOKEN");
        assert_eq!(config.tokens[0].decimals, 6);
        assert_eq!(config.tokens[0].mint_authority_wallet, "alice");
        assert_eq!(config.tokens[0].recipients.len(), 1);
    }

    #[test]
    fn test_load_config_missing_file() {
        let result = load_config("nonexistent-file.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_config_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test-config.json");
        
        fs::write(&config_path, "invalid json {").unwrap();
        
        let result = load_config(config_path.to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_config_default_faucet_port() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test-config.json");
        
        let config_json = r#"{
            "rpcPort": 8899,
            "resetLedger": true,
            "wallets": [],
            "programs": [],
            "tokens": []
        }"#;
        
        fs::write(&config_path, config_json).unwrap();
        
        let config = load_config(config_path.to_str().unwrap()).unwrap();
        assert_eq!(config.faucet_port, None);
    }
}
