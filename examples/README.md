This directory is intended for example Solana programs used to dogfood Cadenza.

For the hackathon submission, you can place a simple "Hello World" style
program here (for example, cloned or adapted from Solana's official examples)
and reference its compiled `.so` and program-id keypair in your
`cadenza-config.json` under the `programs` array.

Example entry:

```json
{
  "name": "hello_world",
  "binaryPath": "./target/deploy/hello_world.so",
  "programIdPath": "./keys/hello_world_id.json"
}
```


