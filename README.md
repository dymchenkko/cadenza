## Cadenza

**Cadenza** is a small Rust CLI that orchestrates a local Solana development environment. It focuses on two things:

- **Spin up a reproducible local validator environment** from a simple JSON config.
- **Checkpoint and restore ledger state** via named snapshots as you iterate on your programs and dapps.

This lets you avoid manually juggling `solana-test-validator` flags, ledger directories, and ad‑hoc scripts every time you want a clean state or to go back to a “known good” setup.

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

## Features (current and planned)

- **Start a local harness**
  - Launches `solana-test-validator` with a configured RPC port and optional ledger reset.
  - Waits for the validator to become healthy before returning.

- **Snapshot & restore ledger state**
  - `snapshot <name>`: copies the current `test-ledger` and config into `snapshots/<name>`.
  - `load <name>`: restores the ledger and config from `snapshots/<name>`, with backups of the current state.
  - `list-snapshots`: lists available snapshots.

- **Declarative config** (implemented in code, provisioning logic WIP)
  - JSON file (`cadenza-config.json` by default) that describes:
    - Wallets to create and fund.
    - Programs to deploy (binary paths + program ID keypairs).
    - Tokens to mint and distribute to specific wallets.

---

## Configuration file

By default Cadenza looks for `cadenza-config.json` in the project root. You can copy `cadenza-config.template.json` as a starter and fill in your values. The shape of the config is:

```json
{
  "rpcPort": 8899,
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

- **`rpcPort`**: Port for the local `solana-test-validator` RPC endpoint.
- **`resetLedger`**: When `true`, starts the validator with `--reset` for a fresh ledger.
- **`wallets`**: List of named wallets with initial SOL balances (in SOL, not lamports).
- **`programs`**:
  - `binaryPath`: Path to the compiled program `.so`.
  - `programIdPath`: Path to the JSON keypair file for the program ID.
- **`tokens`**:
  - `name`: Symbol/identifier for this dev token.
  - `decimals`: SPL token decimals.
  - `mintAuthorityWallet`: Name of a wallet that will be the mint authority.
  - `recipients`: Who receives how many tokens (amounts are in raw token units).

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
  ```

  This launches `solana-test-validator` on `http://127.0.0.1:<rpcPort>` and waits for it to be healthy. The process keeps running until you press Ctrl+C.

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


