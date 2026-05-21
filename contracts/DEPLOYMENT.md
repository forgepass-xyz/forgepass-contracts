# Deployment

> Stub — to be completed in Issue #021 (testnet deployment) and Issue #080 (mainnet deployment).

## Prerequisites

- Rust stable toolchain with `wasm32-unknown-unknown` target
- Soroban CLI at version matching SDK `22.0.7`
- Stellar account funded with XLM for deployment fees
- Access to the ForgePass admin wallet

## Testnet Deployment

> To be documented in Issue #021.

Deployment order is architecturally significant. Contracts must be deployed in this exact order because later contracts reference the passport contract address during initialisation:

1. `passport`
2. `credential-store`
3. `trust-score`
4. `soulbound-nft`

See `contracts/scripts/deploy.sh` for the deployment script.

Deployed testnet addresses: see `contracts/deployments/testnet.json`

## Mainnet Deployment

> To be documented in Issue #080.

No mainnet deployment may begin until:

- External security audit complete (Issue #071)
- All Critical and High audit findings resolved
- Mainnet readiness checklist signed off (Issue #079)

Deployed mainnet addresses: see `contracts/deployments/mainnet.json`

## Rollback Procedure

> To be documented in Issue #021.

Soroban contracts are immutable once deployed. Rollback means deploying a new contract version and updating the ForgePass backend to point at the new address. The old contract remains on-chain permanently.