# Token Swap Example

A simple token swap program demonstrating how to use Cadenza for DeFi development.

## Overview

This example shows a basic token swap program that can be used to test DeFi applications with Cadenza. The program demonstrates:

- Token transfers between accounts
- Instruction handling
- Account validation
- Real-world Solana program patterns

## Building

```bash
cd examples/token_swap
cargo build-sbf
```

The compiled program will be at `target/deploy/token_swap.so`

## Using with Cadenza

Add this to your `cadenza-config.json`:

```json
{
  "cluster": "local",
  "rpcPort": 8899,
  "resetLedger": true,
  "wallets": [
    { "name": "alice", "solBalance": 10.0 },
    { "name": "bob", "solBalance": 10.0 }
  ],
  "programs": [
    {
      "name": "token_swap",
      "binaryPath": "./examples/token_swap/target/deploy/token_swap.so",
      "programIdPath": "./keys/token_swap_id.json"
    }
  ],
  "tokens": [
    {
      "name": "USDC",
      "decimals": 6,
      "mintAuthorityWallet": "alice",
      "recipients": [
        { "walletName": "alice", "amount": 1000000000 },
        { "walletName": "bob", "amount": 500000000 }
      ]
    },
    {
      "name": "SOL_TOKEN",
      "decimals": 9,
      "mintAuthorityWallet": "bob",
      "recipients": [
        { "walletName": "alice", "amount": 5000000000 },
        { "walletName": "bob", "amount": 10000000000 }
      ]
    }
  ]
}
```

Then start Cadenza:

```bash
cargo run -- start
```

## Testing

Once Cadenza has set up your environment, you can test the swap program using the Solana CLI or by writing integration tests.

This example demonstrates how Cadenza makes it easy to:
- Set up multiple wallets with different balances
- Deploy your program automatically
- Create and distribute multiple tokens
- Test DeFi interactions in a reproducible environment
