# forgepass-contracts

> Soroban smart contracts for the ForgePass reputation and identity layer on Stellar.

[![CI](https://github.com/forgepass-xyz/forgepass-contracts/actions/workflows/ci.yml/badge.svg)](https://github.com/forgepass-xyz/forgepass-contracts/actions/workflows/ci.yml)

## What This Repo Does

This repository contains the four on-chain Soroban contracts that form the trust layer of ForgePass. They store what must be permanent and verifiable. All business logic lives off-chain in [forgepass-core](https://github.com/forgepass-xyz/forgepass-core). The contracts enforce invariants.

| Crate | Description |
|---|---|
| `contracts/passport` | Creates and manages soulbound Builder Passport records anchored to Stellar wallet addresses |
| `contracts/trust-score` | Stores versioned Trust Score snapshots for each contributor — readable by any on-chain integrator |
| `contracts/soulbound-nft` | Mints non-transferable achievement badge NFTs when contributors reach defined milestones |
| `contracts/credential-store` | Anchors cryptographic proofs of off-chain contribution events on-chain |

## Design Principle

**On-chain for trust, off-chain for speed.** Soroban stores what must be permanent and verifiable. PostgreSQL and the NestJS API serve it fast.

## Prerequisites

- Rust stable toolchain
- `wasm32-unknown-unknown` target
- Soroban CLI (matching SDK version `22.0.7`)

## Build

```bash
# Add the WASM build target (first time only)
rustup target add wasm32-unknown-unknown

# Build all four contracts
cargo build --target wasm32-unknown-unknown --release

# Confirm WASM artefacts
ls target/wasm32-unknown-unknown/release/*.wasm
```

## Test

```bash
# Tests run against the native host target — not WASM
cargo test --all --target x86_64-unknown-linux-gnu
```

## Lint & Format

```bash
cargo clippy -- -D warnings
cargo fmt --all --check
```

## Architecture

See [ARCHITECTURE.md](./ARCHITECTURE.md) for the full on-chain design, Soroban storage model, credential archival strategy, and cost model.

## Functional Requirements

See the [ForgePass FRD v1.1](https://github.com/forgepass-xyz/forgepass-contracts/blob/main/contracts/docs/) for the complete functional requirements this repository implements.

## Deployment

See [contracts/DEPLOYMENT.md](./contracts/DEPLOYMENT.md) for testnet and mainnet deployment procedures.

Deployed contract addresses:
- Testnet: see [contracts/deployments/testnet.json](./contracts/deployments/testnet.json)
- Mainnet: see [contracts/deployments/mainnet.json](./contracts/deployments/mainnet.json)

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) for how to submit PRs, branch naming conventions, and commit message format.

## Licence

MIT — see [LICENSE](./LICENSE)
