This directory contains example Solana programs that demonstrate how to use Cadenza for real-world development scenarios.

## Available Examples

### 1. Hello World (`hello_world/`)
A minimal Solana program that prints "Hello, world!" - perfect for getting started.

**Use case:** Learning the basics of Solana program development

**Config example:**
```json
{
  "name": "hello_world",
  "binaryPath": "./target/deploy/hello_world.so",
  "programIdPath": "./keys/hello_world_id.json"
}
```

### 2. Token Swap (`token_swap/`)
A practical DeFi example demonstrating token swaps between users. Shows how Cadenza simplifies testing complex multi-token, multi-wallet scenarios.

**Use case:** Testing DeFi applications, token interactions, multi-wallet scenarios

**Features:**
- Multiple wallets (Alice and Bob)
- Multiple tokens (USDC and SOL_TOKEN)
- Token swap program
- Real-world DeFi patterns

**Quick start:**
```bash
# Build the program
cd examples/token_swap
cargo build-sbf

# Use the example config
cp examples/token_swap-config.example.json cadenza-config.json

# Start Cadenza
cargo run -- start
```

**Config example:** See `token_swap-config.example.json` for a complete setup with:
- Two wallets (alice, bob) with 10 SOL each
- Two tokens (USDC with 6 decimals, SOL_TOKEN with 9 decimals)
- Token swap program deployment
- Token distribution to both wallets

## Adding Your Own Example

To add a new example:

1. Create a new directory under `examples/`
2. Add your Solana program code
3. Create a `README.md` explaining the example
4. Optionally create a `*-config.example.json` showing how to use it with Cadenza
5. Update this README with your example

## Why Examples Matter

These examples demonstrate how Cadenza makes Solana development easier:

- **Reproducible environments:** Same setup every time
- **Multi-token testing:** Easy to create and distribute multiple tokens
- **Multi-wallet scenarios:** Simple to set up complex test scenarios
- **State snapshots:** Save and restore your testing state
- **Team collaboration:** Share configs, not scripts

