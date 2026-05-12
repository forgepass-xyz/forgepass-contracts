# forgepass-contracts

**Soroban smart contracts powering the ForgePass Builder Passport**

This repository contains all on-chain logic for ForgePass — the contracts that anchor contributor identity, store credentials, mint achievement badges, and record Trust Scores on the Stellar blockchain via Soroban.

> Part of the [ForgePass](https://github.com/forgepass-xyz) open-source ecosystem.

---

## What Lives Here

Everything in this repo is designed around one principle: **only what needs to be verifiable lives on-chain**. Soroban stores the truth; the off-chain services in [`forgepass-api`](../forgepass-api) handle speed and computation.

### Contracts

| Contract | Purpose |
|---|---|
| **Passport** | The core record — a soulbound entry tied to a contributor's Stellar wallet address. Stores credential hashes, Trust Score anchors, and metadata references. |
| **Soulbound NFT** | Mints non-transferable achievement badges when contributors reach milestones (e.g. first merged PR, first deployed contract, SCF grant delivered). |
| **Trust Score Anchor** | Records versioned Trust Score snapshots on-chain so any third-party contract or project can read a contributor's credibility without depending on ForgePass infrastructure. |
| **Credential Storage** | Stores verified credential proofs — links between off-chain activity (GitHub PRs, bounty completions, escrow milestones) and their on-chain attestation. |

---

## Key Design Decisions

**Soulbound = contributor-owned.** Passport records cannot be transferred, altered, or revoked by ForgePass or anyone else. The contributor's wallet is the sole authority over their record.

**Non-custodial by default.** ForgePass infrastructure interacts with these contracts to write new credentials, but it cannot modify or delete existing ones. The on-chain record is permanent.

**Modular verification.** The credential storage contract is designed to accept new signal sources (e.g. GitLab, additional Stellar protocols) without requiring changes to the core passport contract.

**Open-read.** Any Stellar project or Soroban contract can read passport data permissionlessly. No API key or ForgePass approval required for read access.

---

## Tech Stack

- **Language:** Rust
- **Platform:** Soroban (Stellar's smart contract layer)
- **Test network:** Stellar Testnet (Futurenet for early development)

---

## Development Setup

> Requires the [Soroban CLI](https://soroban.stellar.org/docs/getting-started/setup) and Rust toolchain with `wasm32-unknown-unknown` target.

```bash
# Clone the repo
git clone https://github.com/forgepass-xyz/forgepass-contracts
cd forgepass-contracts

# Build all contracts
cargo build --target wasm32-unknown-unknown --release

# Run tests
cargo test
```

Refer to the [forgepass-docs](../forgepass-docs) repository for full contract deployment guides and testnet configuration.

---

## Relationship to Other Repos

- **[`forgepass-api`](../forgepass-api)** calls these contracts to write new credentials and anchor updated Trust Scores after indexing off-chain activity.
- **[`forgepass-sdk`](../forgepass-sdk)** wraps read calls to these contracts so third-party projects can query passport data without writing Soroban interaction code themselves.

---

## Contributing

This repository welcomes contributions from Rust and Soroban developers. Issues are labelled with `good-first-issue` for accessible entry points.

All code is **MIT licensed**.