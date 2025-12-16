## Cadenza

[![CI](https://github.com/dymchenkko/cadenza/workflows/CI/badge.svg)](https://github.com/dymchenkko/cadenza/actions)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Version](https://img.shields.io/badge/version-0.1.0-orange.svg)](Cargo.toml)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

**Cadenza** is a small Rust CLI that orchestrates a local Solana development environment. It focuses on two things:

- **Spin up a reproducible local validator environment** from a simple JSON config.
- **Checkpoint and restore ledger state** via named snapshots as you iterate on your programs and dapps.

This lets you avoid manually juggling `solana-test-validator` flags, ledger directories, and ad‑hoc scripts every time you want a clean state or to go back to a “known good” setup.

---

## ⚡ Quick Start

Get up and running in under 5 minutes! See the [Quick Start Guide](./QUICKSTART.md) for detailed instructions with screenshots.

```bash
# Clone and build
git clone https://github.com/yourusername/cadenza.git
cd cadenza
cargo build --release

# Copy config template
cp cadenza-config.template.json cadenza-config.json

# Start your local Solana environment
cargo run -- start
```

**That's it!** Your local validator is now running with wallets, programs, and tokens configured. Press `Ctrl+C` to stop.

> 📖 **New to Cadenza?** Check out the [Quick Start Guide](./QUICKSTART.md) for a step-by-step walkthrough with screenshots.

---

## What problem does it solve?

Developing on Solana locally usually involves a lot of repetitive manual work:

- **Starting a validator** with the right ports and options.
- **Creating and funding wallets** for different actors in your tests.
- **Deploying one or more programs** from compiled `.so` files.
- **Minting and distributing SPL tokens** to specific wallets.
- **Copying and backing up the ledger directory** if you want to keep a particular state.

Over time this becomes fragile and hard to reproduce. Cadenza’s goal is to:

- Make **local environments declarative** (via a config file).
- Make **state checkpoints trivial** (via snapshots you can create and load).

---

## Features

- **Start a local or devnet harness**
  - Launches `solana-test-validator` with a configured RPC port and optional ledger reset (for local cluster).
  - Supports both local validator and Solana Devnet.
  - Waits for the validator to become healthy before returning.
  - Non-blocking mode available for background operation.

- **Wallet provisioning**
  - Automatically creates and funds wallets with specified SOL balances.
  - Keypairs are persisted to `keys/` directory for reuse.
  - Handles rate limiting gracefully on Devnet.

- **SPL token provisioning**
  - Creates SPL token mints with configurable decimals.
  - Creates associated token accounts (ATAs) for recipients.
  - Distributes tokens to multiple wallets automatically.

- **Program deployment**
  - Deploys one or more Solana programs from compiled `.so` files.
  - Automatically generates program ID keypairs if they don't exist.
  - Supports multiple programs in a single configuration.

- **Snapshot & restore ledger state**
  - `snapshot <name>`: copies the current `test-ledger` and config into `snapshots/<name>`.
  - `load <name>`: restores the ledger and config from `snapshots/<name>`, with backups of the current state.
  - `list-snapshots`: lists available snapshots.

- **Declarative configuration**
  - JSON file (`cadenza-config.json` by default) that describes:
    - Cluster type (local or devnet).
    - Wallets to create and fund.
    - Programs to deploy (binary paths + program ID keypairs).
    - Tokens to mint and distribute to specific wallets.

---

## Configuration file

By default Cadenza looks for `cadenza-config.json` in the project root. You can copy `cadenza-config.template.json` as a starter and fill in your values. The shape of the config is:

```json
{
  "cluster": "local",
  "rpcPort": 8899,
  "faucetPort": 9900,
  "resetLedger": true,
  "wallets": [
    { "name": "alice_payer", "solBalance": 50.0 },
    { "name": "bob_user", "solBalance": 1.0 }
  ],
  "programs": [
    {
      "name": "staking_vault",
      "binaryPath": "./target/deploy/staking_vault.so",
      "programIdPath": "./keys/staking_vault_id.json"
    }
  ],
  "tokens": [
    {
      "name": "USDC_DEV",
      "decimals": 6,
      "mintAuthorityWallet": "alice_payer",
      "recipients": [
        { "walletName": "alice_payer", "amount": 10000000000 },
        { "walletName": "bob_user", "amount": 5000000 }
      ]
    }
  ]
}
```

Field meanings:

- **`cluster`**: Cluster type - `"local"` (default) or `"devnet"`. When set to `"devnet"`, no local validator is started.
- **`rpcPort`**: Port for the local `solana-test-validator` RPC endpoint (only used for local cluster).
- **`faucetPort`**: Optional port for the local validator faucet (defaults to 9900, only used for local cluster).
- **`resetLedger`**: When `true`, starts the validator with `--reset` for a fresh ledger (only used for local cluster).
- **`wallets`**: List of named wallets with initial SOL balances (in SOL, not lamports). Wallets are created in the `keys/` directory.
- **`programs`**:
  - `name`: Descriptive name for the program.
  - `binaryPath`: Path to the compiled program `.so` file.
  - `programIdPath`: Path to the JSON keypair file for the program ID. If it doesn't exist, it will be generated automatically.
- **`tokens`**:
  - `name`: Symbol/identifier for this dev token.
  - `decimals`: SPL token decimals (typically 6 for stablecoins, 9 for native-like tokens).
  - `mintAuthorityWallet`: Name of a wallet (must be listed in `wallets`) that will be the mint authority.
  - `recipients`: Array of wallet recipients and their token amounts (amounts are in raw token units, not accounting for decimals).

You can create multiple config files and point Cadenza at a specific one using the `--config-path` option on commands that support it.

---

## CLI usage

From the repository root:

- **Start the harness**

  ```bash
  # With default config (cadenza-config.json)
  cargo run -- start

  # With explicit config path
  cargo run -- start --config-path my-config.json

  # Non-blocking mode (validator runs in background)
  cargo run -- start --no-block
  ```

  For local cluster: This launches `solana-test-validator` on `http://127.0.0.1:<rpcPort>` and waits for it to be healthy. The process keeps running until you press Ctrl+C (unless `--no-block` is used).

  For devnet: This provisions wallets, tokens, and programs on Solana Devnet without starting a local validator.

- **Create a snapshot**

  ```bash
  # Validator must be stopped
  cargo run -- snapshot my-snapshot-name
  ```

  This writes to `snapshots/my-snapshot-name/`, including:

  - A copy of the `test-ledger` directory.
  - A copy of `cadenza-config.json`.
  - Metadata file with timestamp and config path.

- **Load a snapshot**

  ```bash
  # Validator must be stopped
  cargo run -- load my-snapshot-name
  ```

  This:

  - Backs up the current ledger to `snapshots/backup-<timestamp>/` (if it exists).
  - Replaces `test-ledger/` with the snapshot’s ledger.
  - Restores `cadenza-config.json` from the snapshot (backing up the existing one first).

- **List snapshots**

  ```bash
  cargo run -- list-snapshots
  ```

---

## Requirements

- Rust toolchain (to run via `cargo`).
- `solana-test-validator` and the Solana CLI tooling installed and available on your `PATH`.

---

## 📖 Documentation

- **[Quick Start Guide](./QUICKSTART.md)** - Get started in 5 minutes with screenshots
- **[Configuration Reference](#configuration-file)** - Detailed config file documentation
- **[CLI Usage](#cli-usage)** - Complete command reference

---

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

---

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](./LICENSE) file for details.

