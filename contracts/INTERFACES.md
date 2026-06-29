# ForgePass Contracts — Interface Specification

> **This file is the ABI contract between the on-chain trust layer and everything
> built on top of it.** Read this before beginning work on issues #016 (Passport
> contract), #017 (Trust Score anchor contract), #018 (Soulbound NFT contract),
> #019 (Credential store contract), and #022 (contract security review).
>
> **Status:** Complete — Issue #014 closed.
> **Source of truth for:** function signatures, type definitions, storage tier
> assignments, access control model, and admin initialisation sequence.
> **Not in this file:** function bodies, test code, deployment scripts, PostgreSQL
> schema. See Section 12.

---

## 1. Introduction

This document specifies the complete public interface for all four ForgePass Soroban
smart contracts: `forgepass-passport`, `forgepass-credential-store`,
`forgepass-trust-score`, and `forgepass-soulbound-nft`. It covers every function
signature, data structure, error type, storage tier assignment, access control rule,
and initialisation requirement across all four contracts.

### Relationship to FRD

| FRD Reference | Covered by this document |
|---|---|
| FR-02.1, FR-02.2, FR-02.3, FR-02.4, FR-02.5 | Sections 4 and 5 — passport lifecycle and credential anchoring |
| FR-03.8 | Section 5 — `credential_exists` deduplication guarantee (live + archived) |
| FR-05.1, FR-05.2, FR-05.3, FR-05.4, FR-05.7 | Section 7 — soulbound NFT and badge extensibility |
| FRD Section 17 (all five data models) | Section 3 — shared data structures |

### How to read this file

- **Sections 1 and 2:** context and cross-cutting design decisions. Read first.
- **Section 3:** all shared `#[contracttype]` types — every function signature in
  Sections 4–7 references types defined here.
- **Sections 4–7:** one section per contract — Rust trait definition, function table,
  and design notes.
- **Section 8:** consolidated access control matrix across all 23 functions.
- **Section 9:** admin initialisation sequence and canonical auth pattern.
- **Sections 10–12:** storage cost estimates, extensibility model, and scope boundary.

---

## 2. Soroban SDK Version and Design Constraints

### Pinned SDK version

**Soroban SDK: `22.0.11`** — resolved and locked in issue #011, `Cargo.lock`
committed. All type annotations, storage API calls, and macro usages in Sections 3–7
reference this version. Do not use SDK APIs introduced after `22.0.11` without a
workspace-level version bump coordinated across all four contract crates.

### Workspace structure

The `contracts/shared/` crate is a fifth member of the Cargo workspace,
added as part of issue #014. It contains:

- `ContractError` enum (Section 3.8)
- `SignalType` enum (Section 3.6)
- `MilestoneType` enum (Section 3.7)
- All five `#[contracttype]` structs (Sections 3.1–3.5)

Each of the four contract crates imports `forgepass-shared` as a local path dependency.
The `shared/` crate does **not** define `DataKey` — each contract crate defines its
own `DataKey` enum to prevent cross-contract storage key collisions.

### Storage tier assignments

| Data category | Storage tier | Rationale |
|---|---|---|
| `PassportRecord` | Persistent | Passports are permanent per FR-02.3. Must survive ledger entry TTL expiry. |
| `CredentialRecord` (live set) | Persistent | Credential proofs are permanent on-chain anchors. Never deleted except via `remove_credentials` during archival. |
| `ArchiveRecord` | Persistent | One entry per archival cycle per wallet, keyed `(wallet, archive_index)`. Accumulates across cycles; cannot share the contract's instance TTL. |
| `ScoreSnapshot` history (`Vec`) | Persistent | Auditable score history required by FR-02.8. Up to 50 snapshots per wallet. |
| `BadgeRecord` | Persistent | Soulbound NFTs are permanent per FR-05.1. No expiry or archival path. |
| Admin address (all four contracts) | Instance | Set once at initialisation. Config-level value; never expires with the contract. |
| Credential count per wallet | Instance | Checked before every `add_credential` call. Cheaper than persistent for a small numeric value with no independent TTL requirement. |
| Archive index counter per wallet | Instance | Monotonically increasing `u32` per wallet. Incremented by `add_archive_record`. |
| Badge ID counter (global) | Instance | Global monotonic counter under `DataKey::BadgeCounter` in the soulbound NFT contract. |
| Score snapshot count per wallet | Instance | Counter enforcing the 50-snapshot cap in `anchor_score`. |

### Error model

A single `ContractError` enum is defined in `contracts/shared/` and imported
by all four contract crates. Discriminants are grouped by hundreds so the originating
contract is identifiable from error codes in NestJS logs and in Horizon transaction
results. The `OnchainWriterService` in issue #027 requires one exhaustive match across
all four contracts. The security review in issue #022 audits one unified access control
model. See Section 3.8 for the full enum.

### Cross-contract invocation policy

**Standalone contracts — no contract invokes another in v1.** The ForgePass backend
validates all preconditions before calling any contract. Cross-contract calls add gas
cost on every operation and create deployment coupling (the passport contract address
would need to be hard-coded or passed into the NFT contract at initialisation).
The security review in #022 does not need to trace execution paths across contract
boundaries.

### Admin initialisation model

All four contracts expose `initialize(env: Env, admin: Address)`, called once
immediately after deployment. The admin address is stored in instance storage and is
**immutable after initialisation** — a second call returns
`ContractError::AlreadyInitialized`. Key rotation requires contract redeployment.

Immutability is a security property: an admin key that cannot be changed after
deployment cannot be compromised through a future admin update operation. The security
review in #022 must document the single-backend-wallet model as a single-point-of-failure
risk and require that mainnet deployment (#080) uses a hardware wallet, multi-sig, or
MPC key management solution.

### Storage key pattern

All four contracts use per-contract `DataKey` enums decorated with `#[contracttype]`.
Raw string keys are prohibited. Each contract crate defines its own `DataKey`; the
`shared/` crate does not define `DataKey`.

Example (passport contract):

```rust
#[contracttype]
pub enum DataKey {
    Admin,
    Passport(Address),
}
```

---

## 3. Shared Data Structures

All types in this section are defined in `contracts/shared/src/lib.rs` and
re-exported for use by each of the four contract crates.

### 3.1 PassportRecord

```rust
#[contracttype]
#[derive(Clone)]
pub struct PassportRecord {
    pub wallet:         Address, // Primary key — passport owner
    pub ipfs_cid:       String,  // CIDv1 of IPFS/Arweave metadata; max 100 chars
    pub created_at:     u64,     // Unix timestamp (seconds) of passport creation
    pub sybil_flagged:  bool,    // Default: false; set only by admin via set_sybil_flag
}
```

| Field | Soroban type | Constraints | PostgreSQL field |
|---|---|---|---|
| `wallet` | `Address` | Primary key; matches passport owner | `wallet_address` |
| `ipfs_cid` | `String` | Non-empty; max 100 chars (CIDv1 format) | `ipfs_metadata_cid` |
| `created_at` | `u64` | Unix timestamp in seconds | `created_at` |
| `sybil_flagged` | `bool` | Default `false`; toggled by admin only | `sybil_flag` |

Storage key: `DataKey::Passport(wallet: Address)` — one record per contributor.

### 3.2 CredentialRecord

```rust
#[contracttype]
#[derive(Clone)]
pub struct CredentialRecord {
    pub id:          u64,         // Contract-generated monotonic; never reused
    pub wallet:      Address,     // FK to PassportRecord
    pub signal_type: SignalType,  // v1 enum values only
    pub source_id:   String,      // External reference; used for deduplication
    pub event_date:  u64,         // Unix timestamp of the contribution event
    pub data_hash:   String,      // SHA-256 hex string; 64 chars
}
```

| Field | Soroban type | Constraints | Notes |
|---|---|---|---|
| `id` | `u64` | Contract-generated monotonic; never reuses IDs | No PostgreSQL equivalent; on-chain only |
| `wallet` | `Address` | FK to `PassportRecord` | `wallet_address` |
| `signal_type` | `SignalType` | v1 enum values only | `signal_type` |
| `source_id` | `String` | Pairs with `signal_type` as the deduplication key | `source_id` |
| `event_date` | `u64` | Unix timestamp (seconds) of the contribution event | `event_date` |
| `data_hash` | `String` | SHA-256 hex string; 64 chars | `data_hash` |

`id` is generated by the contract using a per-wallet monotonic counter in instance
storage (`DataKey::CredentialCounter(wallet)`). The backend does not supply IDs.
The counter starts at 1, increments by 1 on each successful `add_credential` call,
and is never decremented even after archival.

The internal per-wallet storage key pattern for live credential entries is an
implementation decision for issue #019. The interface only specifies that
`get_credentials` returns a `Vec<CredentialRecord>` of the live set, and that
`credential_exists` can locate any credential by `(wallet, signal_type, source_id)`
regardless of its storage layout.

### 3.3 ScoreSnapshot

```rust
#[contracttype]
#[derive(Clone)]
pub struct ScoreSnapshot {
    pub score:             u32,    // 0–100; validated by anchor_score
    pub algorithm_version: String, // Semver string, e.g. "1.0"
    pub signal_hash:       String, // SHA-256 hex string of the input signal set
    pub computed_at:       u64,    // Unix timestamp when score was computed off-chain
}
```

| Field | Soroban type | Constraints | PostgreSQL field |
|---|---|---|---|
| `score` | `u32` | 0–100; `InvalidScore` returned if outside range | `score` |
| `algorithm_version` | `String` | Semver string, e.g. `"1.0"` | `algorithm_version` |
| `signal_hash` | `String` | SHA-256 hex string of the input signal set used for this computation | `signal_hash` |
| `computed_at` | `u64` | Unix timestamp of when the score was computed off-chain | `computed_at` |

History is stored as a bounded `Vec<ScoreSnapshot>` per wallet in persistent storage
under `DataKey::ScoreHistory(wallet)`. A separate `DataKey::CurrentScore(wallet)`
entry in instance storage holds the latest snapshot for cheap single-read access.
Maximum history size: 50 snapshots. When the cap is reached, the oldest entry is
removed before the new one is appended.

### 3.4 BadgeRecord

```rust
#[contracttype]
#[derive(Clone)]
pub struct BadgeRecord {
    pub badge_id:       u64,           // Contract-generated monotonic; global across all wallets
    pub wallet:         Address,       // Badge holder
    pub milestone_type: MilestoneType, // v1 enum values only
    pub ipfs_cid:       String,        // Badge metadata CID from IPFS/Arweave
    pub minted_at:      u64,           // Unix timestamp of the mint call
}
```

| Field | Soroban type | Constraints | Notes |
|---|---|---|---|
| `badge_id` | `u64` | Contract-generated global monotonic; never reused | No PostgreSQL equivalent; on-chain only |
| `wallet` | `Address` | Badge holder | `wallet_address` |
| `milestone_type` | `MilestoneType` | v1 enum values only; at most one badge per type per wallet | `milestone_type` |
| `ipfs_cid` | `String` | Badge metadata CID from IPFS/Arweave | `ipfs_metadata_cid` |
| `minted_at` | `u64` | Unix timestamp of the `mint` call | `minted_at` |

`badge_id` is generated by the contract using a global monotonic counter in instance
storage (`DataKey::BadgeCounter`). IDs start at 1, increment by 1, and are never
reused even if badges could theoretically be burned (they cannot — there is no burn
function).

### 3.5 ArchiveRecord

```rust
#[contracttype]
#[derive(Clone)]
pub struct ArchiveRecord {
    pub merkle_root:      BytesN<32>, // SHA-256 Merkle root of the archived credential batch
    pub credential_count: u32,        // Number of credentials in this archive batch
    pub archived_at:      u64,        // Unix timestamp when archival was recorded on-chain
    pub ipfs_cid:         String,     // CID of the full archive JSON on IPFS/Arweave
}
```

| Field | Soroban type | Constraints | Notes |
|---|---|---|---|
| `merkle_root` | `BytesN<32>` | 32-byte SHA-256 root; `[u8; 32]` in archive JSON (hex-encoded) | Must match `merkle_root` in the IPFS archive JSON file |
| `credential_count` | `u32` | Count of credentials in this specific archive batch | Must match `credential_count` in the IPFS archive JSON file |
| `archived_at` | `u64` | Unix timestamp (seconds) of when `add_archive_record` was called | IPFS archive JSON uses ISO-8601; this field is Unix seconds |
| `ipfs_cid` | `String` | CID of the archive JSON on IPFS | Stored on-chain so third parties can verify without querying ForgePass API |

**Storage key:** `DataKey::ArchiveRecord(wallet: Address, archive_index: u32)` — one
entry per archival cycle per wallet. `archive_index` is a monotonically increasing
`u32` per wallet, starting at 0. The current index counter is stored in instance
storage under `DataKey::ArchiveIndex(wallet)` and incremented by `add_archive_record`.

**Storage tier:** Persistent. Multiple `ArchiveRecord` entries accumulate across
archival cycles for active contributors. Each entry has its own independent TTL.

**Archival workflow:** archival is triggered by the ForgePass backend before each
`add_credential` call when `get_credential_count` returns 100. The backend calls
`add_archive_record` to anchor the Merkle root, then `remove_credentials` to delete
the archived on-chain entries, then proceeds with `add_credential`. The contract does
not trigger archival internally. Full workflow documented in `contracts/ARCHITECTURE.md`
Section 4.

### 3.6 SignalType

```rust
#[contracttype]
pub enum SignalType {
    GithubPr,        // GitHub pull requests merged into registered Stellar repos
    SorobanContract, // Soroban smart contract deployments by the contributor's wallet
    StellarDex,      // DEX trades and Aquarius LP positions via Stellar Horizon
    Hackathon,       // Hackathon participation records via admin batch upload
    // Reserved — no v1 contract logic associated. Appended via WASM upgrade
    // when partnerships are confirmed via issue #009 (FR-09-B).
    ScfGrant,        // Reserved: SCF grant history (gated on #009 + #R07)
    GrantfoxBounty,  // Reserved: GrantFox bounty completions (gated on #009 + #R05)
    TrustlessWork,   // Reserved: Trustless Work milestone completions (gated on #009 + #R06)
}
```

| Variant | Status | PostgreSQL value | Notes |
|---|---|---|---|
| `GithubPr` | Active | `GITHUB_PR` | PRs merged into registered Stellar repos |
| `SorobanContract` | Active | `SOROBAN_CONTRACT` | Contract deployments by the contributor's wallet |
| `StellarDex` | Active | `STELLAR_DEX` | DEX trades and Aquarius LP positions via Horizon |
| `Hackathon` | Active | `HACKATHON` | Admin-batch-uploaded hackathon participation records |
| `ScfGrant` | Reserved — not active in v1 | `SCF_GRANT` | No v1 contract logic. Added when SCF ingestion (#R07) is active. |
| `GrantfoxBounty` | Reserved — not active in v1 | `GRANTFOX_BOUNTY` | No v1 contract logic. Added when GrantFox ingestion (#R05) is confirmed. |
| `TrustlessWork` | Reserved — not active in v1 | `TRUSTLESS_WORK` | No v1 contract logic. Added when Trustless Work feed (#R06) is confirmed. |

**Extensibility:** the credential store contract does not branch on `SignalType` in
any function body. It stores whatever variant is passed to `add_credential`. New
variants can be appended to this enum via WASM upgrade without any change to credential
store contract logic. All signal-type-specific branching lives in the NestJS scoring
engine (off-chain). See Section 11.

**NestJS mapping:** the NestJS layer maps between Rust PascalCase variants
(`GithubPr`) and PostgreSQL SCREAMING\_SNAKE\_CASE values (`GITHUB_PR`). This mapping
is an implementation concern for issues #027 and #031–#033, not a contract interface
concern.

### 3.7 MilestoneType

All v1 active values are taken from `contracts/badges/milestone-registry.json`
committed in issue #008.

```rust
#[contracttype]
pub enum MilestoneType {
    FirstPr,                // First merged PR into a registered Stellar repo
    FirstContract,          // First Soroban contract deployed by the wallet
    HackathonParticipant,   // Verified hackathon participation via admin batch upload
    RisingContributor,      // 10+ merged PRs (author) across registered Stellar repos
    MultiRepoContributor,   // Merged PRs across 3+ distinct registered Stellar repos
    FirstSorobanInvocation, // First invocation of any contract deployed by the wallet
    FullStackBuilder,       // At least 1 GITHUB_PR credential AND 1 SOROBAN_CONTRACT credential
    // Reserved — no v1 contract logic associated. Appended via WASM upgrade
    // when partnerships are confirmed via issue #009 (FR-09-B).
    FirstBounty,            // Reserved: first GrantFox bounty (gated on #009 + #R05)
    FirstGrant,             // Reserved: first SCF grant (gated on #009 + #R07)
    FirstTrustlessWork,     // Reserved: first Trustless Work milestone (gated on #009 + #R06)
}
```

| Variant | Status | Trigger summary |
|---|---|---|
| `FirstPr` | Active | First merged PR (author) into a registered Stellar repo |
| `FirstContract` | Active | First `create_contract` operation from the contributor's wallet |
| `HackathonParticipant` | Active | Admin-verified participation in a registered Stellar event |
| `RisingContributor` | Active (compound) | 10+ merged PRs (author) across registered Stellar repos |
| `MultiRepoContributor` | Active (compound) | Merged PRs across 3+ distinct registered Stellar repos |
| `FirstSorobanInvocation` | Active (compound) | First invocation of any contract deployed by the wallet (any caller) |
| `FullStackBuilder` | Active (compound) | At least one `GithubPr` credential AND one `SorobanContract` credential |
| `FirstBounty` | Reserved — not active in v1 | Added when GrantFox partnership confirmed via #009. |
| `FirstGrant` | Reserved — not active in v1 | Added when SCF partnership confirmed via #009. |
| `FirstTrustlessWork` | Reserved — not active in v1 | Added when Trustless Work partnership confirmed via #009. |

**Soulbound uniqueness:** at most one `BadgeRecord` per `(wallet, MilestoneType)` pair.
The `mint` function returns `BadgeAlreadyMinted` if the pair already exists. The
`has_badge` function checks this constraint.

**HackathonParticipant duplicate logic:** per issue #008, per-event duplicate
prevention for `HackathonParticipant` is owned by the NestJS `BadgeService` using
PostgreSQL (duplicate key `(wallet_address, milestone_type, event_id)`), not by the
soulbound contract. The contract enforces one `HackathonParticipant` badge per wallet;
the `BadgeService` prevents minting again for an event the contributor already has a
badge record for.

**NestJS mapping:** the NestJS layer maps between Rust PascalCase variants
(`RisingContributor`) and PostgreSQL SCREAMING\_SNAKE\_CASE values
(`RISING_CONTRIBUTOR`).

### 3.8 ContractError

Defined in `contracts/shared/src/lib.rs`. All four contract crates import and
use this enum. Discriminants are positive `u32` values grouped by contract to make
the originating contract identifiable from error codes in NestJS error logs.

```rust
#[contracterror]
pub enum ContractError {
    // 100s — Initialisation (all four contracts)
    AlreadyInitialized      = 100, // initialize called a second time
    NotInitialized          = 101, // admin-only function called before initialize
    Unauthorized            = 102, // non-admin caller on an admin-only function

    // 200s — Passport contract
    PassportNotFound        = 200, // update_metadata_cid or set_sybil_flag: no passport
    PassportAlreadyExists   = 201, // create_passport: wallet already has a passport

    // 300s — Credential store contract
    CredentialAlreadyExists = 300, // add_credential: same (signal_type, source_id) exists
    CredentialNotFound      = 301, // Reserved — no v1 function returns this variant
    ArchiveRecordRequired   = 302, // remove_credentials called before add_archive_record

    // 400s — Trust Score contract
    InvalidScore            = 400, // anchor_score: score value outside 0–100 range
    HistoryCapExceeded      = 401, // anchor_score defensive guard; should not occur

    // 500s — Soulbound NFT contract
    BadgeAlreadyMinted      = 500, // mint: wallet already holds this MilestoneType badge
    BadgeNotFound           = 501, // get_badge: badge_id does not exist
    TransferNotAllowed      = 502, // Test harness only — no transfer function exists
}
```

| Variant | Discriminant | Returned by | Description |
|---|---|---|---|
| `AlreadyInitialized` | 100 | `initialize` (all 4 contracts) | `initialize` called a second time on an already-initialised contract |
| `NotInitialized` | 101 | All admin-only functions | Admin-only function called before `initialize` has stored the admin address |
| `Unauthorized` | 102 | All admin-only functions | Admin-only function called by a non-admin caller after initialisation |
| `PassportNotFound` | 200 | `update_metadata_cid`, `set_sybil_flag` | Function called for a wallet address with no existing passport record |
| `PassportAlreadyExists` | 201 | `create_passport` | `create_passport` called for a wallet that already has a passport; idempotency-safe (no panic) |
| `CredentialAlreadyExists` | 300 | `add_credential` | `(signal_type, source_id)` combination already in live or archived storage |
| `CredentialNotFound` | 301 | Reserved | No v1 function returns this variant. Reserved for future use. |
| `ArchiveRecordRequired` | 302 | `remove_credentials` | `remove_credentials` called for a wallet that has no `ArchiveRecord` — prevents credential deletion without an on-chain proof |
| `InvalidScore` | 400 | `anchor_score` | Score value outside the valid 0–100 range |
| `HistoryCapExceeded` | 401 | `anchor_score` (defensive guard) | Snapshot count exceeded 50 before archival ran. Should not occur in a correct backend implementation. |
| `BadgeAlreadyMinted` | 500 | `mint` | Wallet already holds a badge for the specified `MilestoneType` |
| `BadgeNotFound` | 501 | Reserved | `get_badge` returns `Option<BadgeRecord>` — None for absent badges, not this error. Reserved for future use in admin tooling. |
| `TransferNotAllowed` | 502 | Test harness only | No transfer function exists; present so that issue #018 can assert this error for any path that would constitute a transfer if one existed — a compile-time and runtime-verified proof of non-transferability |

---

---

## 4. Passport Contract (`forgepass-passport`)

**Crate:** `contracts/passport`
**FRD:** FR-02.1, FR-02.2, FR-02.3, FR-02.5
**Implementation issue:** #016

### DataKey enum

```rust
// contracts/passport/src/lib.rs
#[contracttype]
pub enum DataKey {
    Admin,             // Address — stored in instance storage; set once at initialize
    Passport(Address), // PassportRecord — stored in persistent storage; one per wallet
}
```

### Trait definition

```rust
pub trait ForgepassPassportTrait {
    /// Called once immediately after deployment. Stores the admin address in instance
    /// storage. Returns AlreadyInitialized (100) on any subsequent call.
    fn initialize(env: Env, admin: Address) -> Result<(), ContractError>;

    /// Creates a new soulbound passport record anchored to the contributor's wallet.
    /// Admin-only. Idempotency-safe: returns PassportAlreadyExists (201) if the wallet
    /// already has a passport — does not panic. Default: sybil_flagged = false.
    fn create_passport(
        env: Env,
        wallet: Address,
        ipfs_cid: String,
    ) -> Result<(), ContractError>;

    /// Returns the full PassportRecord for the given wallet, or None if no passport
    /// exists. Public — no auth. Does not filter on sybil_flagged; the API layer
    /// applies the sybil exclusion (FR-11.1).
    fn get_passport(env: Env, wallet: Address) -> Option<PassportRecord>;

    /// Returns true if the wallet has a passport AND sybil_flagged is false.
    /// Returns false if the wallet has no passport OR sybil_flagged is true.
    /// Public — no auth. Used by integrating contracts and the NestJS layer.
    fn is_valid(env: Env, wallet: Address) -> bool;

    /// Replaces the on-chain IPFS metadata CID for the contributor's passport.
    /// Owner-only: the passport owner (wallet) must sign this transaction via Freighter.
    /// The backend uploads new metadata to IPFS first, then presents this transaction
    /// to the contributor for signing. Returns PassportNotFound (200) if no passport
    /// exists for the wallet.
    fn update_metadata_cid(
        env: Env,
        wallet: Address,
        new_cid: String,
    ) -> Result<(), ContractError>;

    /// Sets or clears the sybil_flagged field on the passport record.
    /// Admin-only. Returns PassportNotFound (200) if no passport exists for the wallet.
    fn set_sybil_flag(
        env: Env,
        wallet: Address,
        flagged: bool,
    ) -> Result<(), ContractError>;
}
```

### Function table

| Function | Access | Key parameters | Returns | Possible errors |
|---|---|---|---|---|
| `initialize` | Admin-bootstrap | `admin: Address` | `Result<(), ContractError>` | 100, 102 |
| `create_passport` | Admin-only | `wallet`, `ipfs_cid` | `Result<(), ContractError>` | 101, 102, 201 |
| `get_passport` | Public | `wallet` | `Option<PassportRecord>` | — |
| `is_valid` | Public | `wallet` | `bool` | — |
| `update_metadata_cid` | Owner-only | `wallet`, `new_cid` | `Result<(), ContractError>` | 101, 200 |
| `set_sybil_flag` | Admin-only | `wallet`, `flagged` | `Result<(), ContractError>` | 101, 102, 200 |

### Design notes

**Idempotency on create:** `create_passport` must return `PassportAlreadyExists` (201)
on a duplicate call, not panic. The NestJS `OnchainWriterService` treats 201 as a
recoverable condition and logs a warning rather than raising an alert.

**is_valid behaviour:** `is_valid` returns `false` for two distinct cases —
no passport exists, and sybil flagged. Third-party contracts calling `is_valid` cannot
distinguish between these two cases. The API response object (via `get_passport`)
provides the full state when the distinction matters.

**update_metadata_cid auth model:** this is the only write function callable by the
passport owner rather than the ForgePass backend admin. The owner signs the transaction
via Freighter. The backend constructs the unsigned XDR transaction, presents it to
the Freighter SDK for signing, and submits on confirmation. Raw PII is never written
on-chain (FR-02.5); only the IPFS CID is stored.

**Sybil flag API behaviour:** when `sybil_flagged = true`, the contract stores and
returns the full `PassportRecord` including the flag. The contract does not suppress
reads. The API layer (NestJS) reads `sybil_flagged` from the PassportRecord and
returns 404 for unauthenticated third-party requests (FR-11.1). On-chain, the record
is always accessible.

---

## 5. Credential Store Contract (`forgepass-credential-store`)

**Crate:** `contracts/credential-store`
**FRD:** FR-02.4, FR-03.8, FR-04.4
**Implementation issue:** #019
**Architecture reference:** `contracts/ARCHITECTURE.md` Sections 3–6

### DataKey enum

```rust
// contracts/credential-store/src/lib.rs
#[contracttype]
pub enum DataKey {
    Admin,                       // Address — instance; set at initialize
    Credentials(Address),        // Vec<CredentialRecord> — persistent; live set per wallet
    CredentialCounter(Address),  // u64 — instance; monotonic ID generator per wallet
    ArchiveRecord(Address, u32), // ArchiveRecord — persistent; one per archival cycle
    ArchiveIndex(Address),       // u32 — instance; next archive_index for this wallet
}
```

**Note on Credentials storage:** live credentials for a wallet are stored as a single
`Vec<CredentialRecord>` under `DataKey::Credentials(wallet)` in persistent storage.
At 100 credentials × 250 bytes per credential, the maximum entry size is approximately
25 KB — within Soroban's entry value limit. Issue #019 must verify the exact XDR-encoded
size against the SDK version `22.0.11` limits before finalising this layout.

**Note on credential count:** `get_credential_count` returns `Credentials(wallet).len()`
from the live Vec. A separate count entry is not required. The cost of loading the
Vec header for `.len()` is minimal.

### Trait definition

```rust
pub trait ForgepassCredentialStoreTrait {
    /// Called once immediately after deployment. Stores the admin address.
    /// Returns AlreadyInitialized (100) on any subsequent call.
    fn initialize(env: Env, admin: Address) -> Result<(), ContractError>;

    /// Writes a new credential proof to the contributor's live on-chain record.
    /// Admin-only. Performs a live-credential deduplication check before writing:
    /// returns CredentialAlreadyExists (300) if (signal_type, source_id) already
    /// exists in the live set. Does NOT check archived credentials — the backend
    /// checks PostgreSQL for archived duplicates before calling this function.
    /// Returns the contract-generated credential id (u64) on success.
    fn add_credential(
        env: Env,
        wallet: Address,
        signal_type: SignalType,
        source_id: String,
        event_date: u64,
        data_hash: String,
    ) -> Result<u64, ContractError>;

    /// Returns the full live credential set for the wallet as a Vec<CredentialRecord>.
    /// Public — no auth. Returns an empty Vec if the wallet has no live credentials.
    /// Does not include archived credentials; call get_archive_records for those.
    fn get_credentials(env: Env, wallet: Address) -> Vec<CredentialRecord>;

    /// Returns the count of live on-chain credentials for the wallet.
    /// Public — no auth. Returns 0 for wallets with no live credentials.
    /// Does not include archived credentials. The backend checks this before every
    /// add_credential call to determine whether archival is required.
    fn get_credential_count(env: Env, wallet: Address) -> u32;

    /// Returns true if a credential with the given (signal_type, source_id) pair
    /// exists in the live on-chain credential set for the wallet.
    /// Public — no auth. Checks only the live set — not archived credentials.
    /// Used by third parties for on-chain credential verification.
    fn credential_exists(
        env: Env,
        wallet: Address,
        signal_type: SignalType,
        source_id: String,
    ) -> bool;

    /// Anchors a Merkle root on-chain after the backend has completed an archival cycle.
    /// Admin-only. Called by the backend after:
    ///   (1) writing the archive JSON to PostgreSQL
    ///   (2) pinning the archive JSON to IPFS (receiving the CID)
    /// before calling remove_credentials.
    /// Increments DataKey::ArchiveIndex(wallet) on success.
    /// See contracts/ARCHITECTURE.md Section 4 for the full archival workflow.
    fn add_archive_record(
        env: Env,
        wallet: Address,
        merkle_root: BytesN<32>,
        credential_count: u32,
        archived_at: u64,
        ipfs_cid: String,
    ) -> Result<(), ContractError>;

    /// Returns all ArchiveRecord entries for the wallet, ordered by archive_index
    /// ascending (oldest archival cycle first).
    /// Public — no auth. Returns an empty Vec if no archival has occurred.
    fn get_archive_records(env: Env, wallet: Address) -> Vec<ArchiveRecord>;

    /// Deletes the specified live credential entries from on-chain storage.
    /// Admin-only. Safety precondition: at least one ArchiveRecord must exist for
    /// the wallet (verified by the contract). If no ArchiveRecord exists, returns
    /// ArchiveRecordRequired (302) — preventing credential deletion without an
    /// on-chain proof.
    /// source_ids not found in the live set are silently skipped (not an error).
    /// Must only be called after add_archive_record has confirmed on-chain for the
    /// credentials being removed.
    fn remove_credentials(
        env: Env,
        wallet: Address,
        source_ids: Vec<String>,
    ) -> Result<(), ContractError>;
}
```

### Function table

| Function | Access | Key parameters | Returns | Possible errors |
|---|---|---|---|---|
| `initialize` | Admin-bootstrap | `admin` | `Result<(), ContractError>` | 100, 102 |
| `add_credential` | Admin-only | `wallet`, `signal_type`, `source_id`, `event_date`, `data_hash` | `Result<u64, ContractError>` | 101, 102, 300 |
| `get_credentials` | Public | `wallet` | `Vec<CredentialRecord>` | — |
| `get_credential_count` | Public | `wallet` | `u32` | — |
| `credential_exists` | Public | `wallet`, `signal_type`, `source_id` | `bool` | — |
| `add_archive_record` | Admin-only | `wallet`, `merkle_root`, `credential_count`, `archived_at`, `ipfs_cid` | `Result<(), ContractError>` | 101, 102 |
| `get_archive_records` | Public | `wallet` | `Vec<ArchiveRecord>` | — |
| `remove_credentials` | Admin-only | `wallet`, `source_ids` | `Result<(), ContractError>` | 101, 102, 302 |

### Design notes

**Archival is backend-controlled, not contract-triggered:** `add_credential` does not
check the credential count or trigger archival. The backend calls
`get_credential_count` before every `add_credential` call. If the count is 100, the
backend runs the archival workflow (`add_archive_record` then `remove_credentials`)
before proceeding. The contract's role is storage, not orchestration.

**Deduplication scope:** `add_credential` and `credential_exists` cover live on-chain
credentials only. Archived credentials were removed from on-chain storage by
`remove_credentials`. The backend checks PostgreSQL's `archived_credentials` table for
archived duplicates before calling `add_credential`. Third parties verifying archived
credentials must use `get_archive_records` + IPFS (see `contracts/ARCHITECTURE.md`
Section 5 for the full verification procedure).

**ArchiveRecordRequired safety guard:** `remove_credentials` verifies that at least one
`ArchiveRecord` exists for the wallet before deleting any live credential entries.
This prevents a backend bug from silently deleting live credentials without a
corresponding on-chain Merkle root. The backend must call `add_archive_record` first
and confirm the transaction before calling `remove_credentials`.

**source_ids skipping:** `remove_credentials` silently skips `source_ids` not found
in the live set. This is intentional: if the backend retries a `remove_credentials`
call after a partial failure, previously-removed entries are skipped without error.

**Future SignalType variants:** `add_credential` accepts any `SignalType` variant,
including reserved ones. The contract does not branch on `SignalType`. New variants
added to the `shared/` crate via WASM upgrade are automatically stored without any
credential store contract change.

---

## 6. Trust Score Anchor Contract (`forgepass-trust-score`)

**Crate:** `contracts/trust-score`
**FRD:** FR-02.8, FR-04.2, FR-04.4
**Implementation issue:** #017

### DataKey enum

```rust
// contracts/trust-score/src/lib.rs
#[contracttype]
pub enum DataKey {
    Admin,                 // Address — instance; set at initialize
    CurrentScore(Address), // ScoreSnapshot — instance; latest snapshot per wallet
    ScoreHistory(Address), // Vec<ScoreSnapshot> — persistent; up to 50 entries per wallet
    SnapshotCount(Address),// u32 — instance; current history length per wallet
}
```

**CurrentScore vs ScoreHistory:** `CurrentScore` in instance storage provides cheap
O(1) access for the common case (third-party score reads). `ScoreHistory` in persistent
storage provides the full auditable history (FR-02.8). Both are updated on every
`anchor_score` call.

### Trait definition

```rust
pub trait ForgepassTrustScoreTrait {
    /// Called once immediately after deployment. Stores the admin address.
    /// Returns AlreadyInitialized (100) on any subsequent call.
    fn initialize(env: Env, admin: Address) -> Result<(), ContractError>;

    /// Anchors a new Trust Score snapshot on-chain. Admin-only.
    /// Validates score is within 0–100 range; returns InvalidScore (400) otherwise.
    /// Appends the snapshot to ScoreHistory. If history is at 50 entries, the oldest
    /// snapshot is removed before appending the new one.
    /// Updates CurrentScore (instance) with the new snapshot in the same call.
    fn anchor_score(
        env: Env,
        wallet: Address,
        score: u32,
        algorithm_version: String,
        signal_hash: String,
        computed_at: u64,
    ) -> Result<(), ContractError>;

    /// Returns the most recent ScoreSnapshot for the wallet, or None if no score
    /// has been anchored. Public — no auth. Reads from instance storage for cheap access.
    fn get_current_score(env: Env, wallet: Address) -> Option<ScoreSnapshot>;

    /// Returns the full score history for the wallet as a Vec<ScoreSnapshot>, ordered
    /// ascending by computed_at (oldest first). Returns an empty Vec if no scores
    /// have been anchored. Public — no auth.
    fn get_score_history(env: Env, wallet: Address) -> Vec<ScoreSnapshot>;
}
```

### Function table

| Function | Access | Key parameters | Returns | Possible errors |
|---|---|---|---|---|
| `initialize` | Admin-bootstrap | `admin` | `Result<(), ContractError>` | 100, 102 |
| `anchor_score` | Admin-only | `wallet`, `score`, `algorithm_version`, `signal_hash`, `computed_at` | `Result<(), ContractError>` | 101, 102, 400, 401 |
| `get_current_score` | Public | `wallet` | `Option<ScoreSnapshot>` | — |
| `get_score_history` | Public | `wallet` | `Vec<ScoreSnapshot>` | — |

### Design notes

**Score validation:** `anchor_score` must reject any `score` value outside the range
`[0, 100]` inclusive, returning `InvalidScore` (400). The NestJS scoring engine
normalises scores to this range before calling the contract, so 400 indicates a
backend bug and must be treated as a blocking alert (not a retry condition).

**History cap management:** when `SnapshotCount(wallet)` reaches 50 and `anchor_score`
is called, the oldest entry in `ScoreHistory(wallet)` is removed and the new snapshot
is appended. `SnapshotCount` remains at 50. `HistoryCapExceeded` (401) is a defensive
guard for implementation bugs — it should never be returned in a correct implementation
where the cap management logic runs before every append.

**Ordering guarantee:** `get_score_history` must return snapshots ordered ascending
by `computed_at`. Since `anchor_score` appends entries chronologically, this ordering
holds naturally if the Vec preserves insertion order (Soroban `Vec` does). The
implementation in #017 must not sort on read — it must maintain order on write.

**Score computation is off-chain:** this contract anchors what the NestJS scoring
engine provides. It does not validate `algorithm_version` strings, does not verify the
`signal_hash`, and does not recompute scores. Input validation is limited to the score
range check.

---

## 7. Soulbound NFT Contract (`forgepass-soulbound-nft`)

**Crate:** `contracts/soulbound-nft`
**FRD:** FR-05.1, FR-05.2, FR-05.3, FR-05.4, FR-05.7
**Implementation issue:** #018

### DataKey enum

```rust
// contracts/soulbound-nft/src/lib.rs
#[contracttype]
pub enum DataKey {
    Admin,                          // Address — instance; set at initialize
    Badge(u64),                     // BadgeRecord — persistent; keyed by badge_id
    WalletBadges(Address),          // Vec<u64> — persistent; badge_ids held by wallet
    BadgeCounter,                   // u64 — instance; global monotonic badge_id counter
    HasBadge(Address, MilestoneType), // bool — instance; fast duplicate-prevention lookup
}
```

**HasBadge as fast lookup:** storing `DataKey::HasBadge(wallet, milestone_type)` as a
boolean in instance storage allows `has_badge` to return without loading the full
`WalletBadges` Vec. The `mint` function writes this entry at the same time as
`Badge(badge_id)` and `WalletBadges`. Instance storage is appropriate since badge
state (earned or not) never expires independently of the contract.

### Trait definition

```rust
pub trait ForgepassSoulboundNftTrait {
    /// Called once immediately after deployment. Stores the admin address.
    /// Returns AlreadyInitialized (100) on any subsequent call.
    fn initialize(env: Env, admin: Address) -> Result<(), ContractError>;

    /// Mints a soulbound achievement badge NFT to the contributor's wallet.
    /// Admin-only. Returns BadgeAlreadyMinted (500) if the wallet already holds a
    /// badge for the given MilestoneType. Returns the contract-generated badge_id
    /// on success — the backend stores this in PostgreSQL.
    /// The contract does NOT verify that the contributor's wallet has a passport
    /// (standalone contracts — Section 2). The backend validates this precondition.
    fn mint(
        env: Env,
        wallet: Address,
        milestone_type: MilestoneType,
        ipfs_cid: String,
        minted_at: u64,
    ) -> Result<u64, ContractError>;

    /// Returns the BadgeRecord for the given badge_id, or None if it does not exist.
    /// Public — no auth.
    fn get_badge(env: Env, badge_id: u64) -> Option<BadgeRecord>;

    /// Returns all BadgeRecords held by the wallet, ordered ascending by minted_at.
    /// Public — no auth. Returns an empty Vec if the wallet holds no badges.
    fn get_badges_for_wallet(env: Env, wallet: Address) -> Vec<BadgeRecord>;

    /// Returns true if the wallet holds a badge for the given MilestoneType.
    /// Public — no auth. Reads from the HasBadge instance entry for O(1) performance.
    /// Called by the NestJS BadgeService before every mint call.
    fn has_badge(env: Env, wallet: Address, milestone_type: MilestoneType) -> bool;
}
```

### Function table

| Function | Access | Key parameters | Returns | Possible errors |
|---|---|---|---|---|
| `initialize` | Admin-bootstrap | `admin` | `Result<(), ContractError>` | 100, 102 |
| `mint` | Admin-only | `wallet`, `milestone_type`, `ipfs_cid`, `minted_at` | `Result<u64, ContractError>` | 101, 102, 500 |
| `get_badge` | Public | `badge_id` | `Option<BadgeRecord>` | — |
| `get_badges_for_wallet` | Public | `wallet` | `Vec<BadgeRecord>` | — |
| `has_badge` | Public | `wallet`, `milestone_type` | `bool` | — |

### Design notes

**No transfer function — by design:** no transfer function exists in this interface.
This is an interface-level guarantee, not only a runtime check. Any attempt to
implement a transfer function in the crate would require adding it to this trait first.
The test harness for issue #018 must include a test that asserts
`ContractError::TransferNotAllowed` (502) for any invocation path that could
constitute a transfer — providing a runtime-verifiable proof of non-transferability
alongside the interface-level proof.

**BadgeAlreadyMinted duplicate check:** the contract checks `HasBadge(wallet,
milestone_type)` as the authoritative duplicate guard. The NestJS `BadgeService`
also checks `has_badge` via a read call before constructing the mint transaction.
Both layers enforce the constraint independently. The contract check is the
source of truth.

**badge_id is global and monotonic:** `BadgeCounter` starts at 1 and increments by 1
on every successful `mint`. IDs are never reused. The monotonic counter means
badge_id ordering reflects mint chronology across all wallets — useful for admin
tooling and audit logs.

**mint returns badge_id:** the backend stores this in the PostgreSQL `badges` table
(`nft_address` field equivalent is the badge_id). The on-chain badge is keyed by
badge_id; the Stellar transaction ID for the mint is stored in `on_chain_tx` in
PostgreSQL, not on-chain.

**Extensibility — new MilestoneType variants:** the contract does not branch on
`MilestoneType` in any function. New variants appended to the shared enum via WASM
upgrade are automatically supported in `mint`, `has_badge`, and `get_badges_for_wallet`
without any soulbound NFT contract logic change.

**Reverse index not implemented in v1:** a reverse lookup of all wallets holding a
specific `MilestoneType` badge is not supported in v1. The `DataKey::WalletBadges`
pattern is wallet-forward only. If reverse lookup is required in a future version,
add `DataKey::BadgeHolders(MilestoneType) -> Vec<Address>` via WASM upgrade. Adding
this entry to `mint` at that point does not break existing `has_badge` or
`get_badges_for_wallet` callers.

---

---

## 8. Access Control Matrix

This section is the primary reference for issue #022 (contract security review) and
issue #027 (OnchainWriterService). Every function across all four contracts appears
exactly once. The security review must verify that each row's access level is enforced
correctly in the implementation and that no undocumented function exists.

### Access levels

| Level | Definition |
|---|---|
| **Admin-bootstrap** | Callable once by any caller before `initialize` has run. After the first successful call, the admin address is stored and this function returns `AlreadyInitialized` (100) on every subsequent call. |
| **Admin-only** | Requires the stored admin address to have signed the transaction. The contract loads `DataKey::Admin` from instance storage and calls `admin.require_auth()`. If the admin is not set, returns `NotInitialized` (101). If the correct admin signature is absent, the Soroban host traps the transaction — this is not a `ContractError` return value. |
| **Owner-only** | Requires the `wallet` parameter address to have signed the transaction. The contract calls `wallet.require_auth()`. Auth failure is a Soroban host trap, not a `ContractError`. |
| **Public** | No auth check. Any caller, any context. |

### Auth failure model

`require_auth()` in Soroban SDK `22.0.11` causes a host-level transaction trap on auth
failure. It does not return a `ContractError`. The NestJS `OnchainWriterService` must
handle two distinct error shapes in transaction results:

- **ContractError returned:** the transaction completes but the contract returned an
  error value (e.g. `NotInitialized` = 101). Visible in `InvokeHostFunctionOp` result
  as a successful host invocation with an error return.
- **Host trap:** the transaction fails at the Soroban host level. Visible as
  `INVOKE_HOST_FUNCTION_TRAPPED` in the operation result. This is the error shape for
  auth failures on admin-only and owner-only functions.

`ContractError::Unauthorized` (102) is defined in the enum as a semantic marker and
a future extension point. No v1 function returns it — auth violations are host traps.

### Function access control table

| # | Function | Contract | Access | Auth check | ContractErrors possible |
|---|---|---|---|---|---|
| 1 | `initialize` | Passport | Admin-bootstrap | Check `DataKey::Admin` not set; store on success | 100 |
| 2 | `create_passport` | Passport | Admin-only | Load `Admin` → `admin.require_auth()` | 101, 201 |
| 3 | `get_passport` | Passport | Public | None | — |
| 4 | `is_valid` | Passport | Public | None | — |
| 5 | `update_metadata_cid` | Passport | Owner-only | `wallet.require_auth()` | 200 |
| 6 | `set_sybil_flag` | Passport | Admin-only | Load `Admin` → `admin.require_auth()` | 101, 200 |
| 7 | `initialize` | Credential Store | Admin-bootstrap | Check `DataKey::Admin` not set; store on success | 100 |
| 8 | `add_credential` | Credential Store | Admin-only | Load `Admin` → `admin.require_auth()` | 101, 300 |
| 9 | `get_credentials` | Credential Store | Public | None | — |
| 10 | `get_credential_count` | Credential Store | Public | None | — |
| 11 | `credential_exists` | Credential Store | Public | None | — |
| 12 | `add_archive_record` | Credential Store | Admin-only | Load `Admin` → `admin.require_auth()` | 101 |
| 13 | `get_archive_records` | Credential Store | Public | None | — |
| 14 | `remove_credentials` | Credential Store | Admin-only | Load `Admin` → `admin.require_auth()` | 101, 302 |
| 15 | `initialize` | Trust Score | Admin-bootstrap | Check `DataKey::Admin` not set; store on success | 100 |
| 16 | `anchor_score` | Trust Score | Admin-only | Load `Admin` → `admin.require_auth()` | 101, 400, 401 |
| 17 | `get_current_score` | Trust Score | Public | None | — |
| 18 | `get_score_history` | Trust Score | Public | None | — |
| 19 | `initialize` | Soulbound NFT | Admin-bootstrap | Check `DataKey::Admin` not set; store on success | 100 |
| 20 | `mint` | Soulbound NFT | Admin-only | Load `Admin` → `admin.require_auth()` | 101, 500 |
| 21 | `get_badge` | Soulbound NFT | Public | None | — |
| 22 | `get_badges_for_wallet` | Soulbound NFT | Public | None | — |
| 23 | `has_badge` | Soulbound NFT | Public | None | — |

### Access level summary

| Access level | Count | Functions |
|---|---|---|
| Admin-bootstrap | 4 | `initialize` × 4 |
| Admin-only | 7 | `create_passport`, `set_sybil_flag`, `add_credential`, `add_archive_record`, `remove_credentials`, `anchor_score`, `mint` |
| Owner-only | 1 | `update_metadata_cid` |
| Public | 11 | `get_passport`, `is_valid`, `get_credentials`, `get_credential_count`, `credential_exists`, `get_archive_records`, `get_current_score`, `get_score_history`, `get_badge`, `get_badges_for_wallet`, `has_badge` |
| **Total** | **23** | |

### Security review requirements for issue #022

The internal security review must verify each of the following against the actual
contract implementations:

1. **No undocumented write function:** every function that modifies state must appear
   in this matrix. Any state-modifying function not in this table is an unintended
   write path and a critical finding.
2. **Admin-only enforcement:** for all 7 admin-only functions, confirm that
   `admin.require_auth()` is called before any state modification. No state change
   must occur before the auth check completes.
3. **AlreadyInitialized guard:** for all 4 `initialize` functions, confirm that a
   second call always returns `AlreadyInitialized` (100) and does not overwrite the
   stored admin address.
4. **Owner-only isolation:** for `update_metadata_cid`, confirm that `wallet.require_auth()`
   is called and that the implementation cannot be called for a wallet address other
   than the signer.
5. **ArchiveRecordRequired guard:** for `remove_credentials`, confirm that the function
   verifies at least one `ArchiveRecord` exists for the wallet before deleting any
   credential entries.
6. **TransferNotAllowed proof:** confirm that no function in the soulbound NFT contract
   changes the `wallet` field of any `BadgeRecord` or moves a `BadgeRecord` from one
   `DataKey::WalletBadges` to another.
7. **No cross-contract write paths:** confirm that no contract calls another contract's
   write functions. Standalone design means zero cross-contract invocations on any
   write path.

---

## 9. Admin Initialisation Sequence

### Deployment order

The four contracts are standalone and have no dependency on each other's deployed
addresses. They may be deployed in any order. The canonical order used in the
deployment scripts (#021 for testnet, #080 for mainnet) is:

```
1. forgepass-passport
2. forgepass-credential-store
3. forgepass-trust-score
4. forgepass-soulbound-nft
```

This order reflects the logical dependency of the data layer (passport first, then
credentials, then scores, then badges) and matches the flow in FRD Section 18.1.

### Initialisation race window

**There is a brief window between contract deployment and `initialize` being called
during which any actor could call `initialize` with their own address as admin.**

Mitigation: the deployment script must call `initialize` in the **same Stellar
transaction batch** as the WASM upload and contract creation, using Stellar's
multi-operation transaction support. A single transaction that uploads WASM, creates
the contract, and invokes `initialize` is atomic — the race window does not exist.

The security review in #022 must verify that the deployment script in #021 uses this
pattern. Any deployment that calls `initialize` in a separate transaction after
contract creation is a critical security finding.

### Canonical `initialize` pattern

All four contracts implement `initialize` identically. The canonical implementation
is shown once here; #016, #017, #018, and #019 must follow this pattern exactly:

```rust
fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
    // Guard: return AlreadyInitialized if admin is already set.
    // Do not use unwrap_or; use has() to avoid deserialising the stored value.
    if env.storage().instance().has(&DataKey::Admin) {
        return Err(ContractError::AlreadyInitialized);
    }
    // Store the admin address. All subsequent admin-only calls will load this.
    env.storage().instance().set(&DataKey::Admin, &admin);
    Ok(())
}
```

### Canonical admin auth helper pattern

All four contracts implement admin auth checking identically. Since `DataKey` is
per-contract (not shared), this helper must be implemented locally in each crate.
The pattern is:

```rust
/// Load and authenticate the admin address.
/// Returns NotInitialized (101) if initialize has not been called.
/// Panics at the Soroban host level (INVOKE_HOST_FUNCTION_TRAPPED) if
/// the required admin signature is absent from the transaction.
fn require_admin(env: &Env) -> Result<(), ContractError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(ContractError::NotInitialized)?;
    admin.require_auth();
    Ok(())
}
```

Every admin-only function calls `require_admin(env)?;` as its first statement, before
any state read or write. The security review must confirm this ordering for all 7
admin-only functions.

### Post-deployment smoke test sequence

After all four contracts are deployed and initialised, the deployment script must run
this smoke test sequence to confirm each contract is live and the admin is set
correctly. This is required by the deployment acceptance criteria in #021:

```
Test 1 — Passport contract:
  Call: get_passport(test_wallet)
  Expected: Ok(None) — no panic, contract is live

Test 2 — Credential store contract:
  Call: get_credential_count(test_wallet)
  Expected: Ok(0) — no panic, contract is live

Test 3 — Trust Score contract:
  Call: get_current_score(test_wallet)
  Expected: Ok(None) — no panic, contract is live

Test 4 — Soulbound NFT contract:
  Call: has_badge(test_wallet, MilestoneType::FirstPr)
  Expected: Ok(false) — no panic, contract is live

Test 5 — Admin auth guard (all four contracts):
  Call: create_passport(test_wallet, "cid") from a non-admin wallet
  Expected: INVOKE_HOST_FUNCTION_TRAPPED — confirm require_auth() fires correctly

Test 6 — AlreadyInitialized guard (all four contracts):
  Call: initialize(admin) again on each deployed contract
  Expected: Err(ContractError::AlreadyInitialized) — discriminant 100
```

### Mainnet key management requirement

For testnet deployment (#021), the admin wallet may be a standard Stellar keypair
stored in the deployment environment. For mainnet deployment (#080), the admin wallet
**must** use one of the following:

- Hardware wallet (Ledger with Stellar app)
- Multi-signature account (threshold > 1, multiple signers)
- MPC key management (threshold signature scheme)

A standard keypair stored in CI secrets or an environment file is not acceptable for
mainnet. The security review in #022 must document the chosen mainnet key management
approach and confirm it is implemented before #080 begins.

---

---

## 10. Storage Cost Estimates

This section provides per-struct size estimates for all five `#[contracttype]` structs
and cross-references the committed credential cost model from issue #003. The full
cost model for credentials is in `contracts/ARCHITECTURE.md` Section 2 and
`contracts/docs/cost-model.md` — those documents are the authoritative source. This
section covers the four struct types not included in the #003 analysis.

### Credential cost model reference

The #003 cost model covers `CredentialRecord` comprehensively. Key outputs:

| Parameter | Value | Source |
|---|---|---|
| Conservative size per credential | **250 bytes** | `ARCHITECTURE.md` Section 2 |
| Cost at 100 credentials — Scenario A (mainnet-typical) | $0.001/yr/user | `cost-model.md` |
| Cost at 100 credentials — Scenario B (worst-case) | $1.08/yr/user | `cost-model.md` |
| Archival trigger | 100 live credentials per wallet | `ARCHITECTURE.md` Section 3 |
| Archival batch size | 50 oldest credentials removed per cycle | `ARCHITECTURE.md` Section 4 |

**Discrepancy note:** the original issue #014 roadmap document estimated 280 bytes per
`CredentialRecord`. `ARCHITECTURE.md` Section 2 (the committed #003 output) uses
250 bytes as the conservative model value. `ARCHITECTURE.md` is the authoritative
source. The 280-byte figure in the roadmap is superseded.

### Per-struct size estimates (types not in #003 cost model)

All estimates use XDR-encoded sizes. Actual sizes depend on variable-length fields
(Strings). The estimates below assume typical real-world values.

| Struct | Field breakdown | Estimated size | Storage tier | Max entries per wallet |
|---|---|---|---|---|
| `PassportRecord` | wallet (56) + ipfs\_cid (60) + created\_at (8) + sybil\_flagged (1) + overhead (30) | ~155 bytes | Persistent | 1 |
| `ScoreSnapshot` | score (4) + algorithm\_version (5) + signal\_hash (64) + computed\_at (8) + overhead (20) | ~101 bytes | Persistent (in Vec) | 50 |
| `BadgeRecord` | badge\_id (8) + wallet (56) + milestone\_type (4) + ipfs\_cid (60) + minted\_at (8) + overhead (20) | ~156 bytes | Persistent | 7 (v1 active types) |
| `ArchiveRecord` | merkle\_root (32) + credential\_count (4) + archived\_at (8) + ipfs\_cid (60) + overhead (20) | ~124 bytes | Persistent | ~1 per 3.3 years (at 30 credentials/yr) |

### Storage footprint summary at 10,000 contributors

| Data category | Size per wallet | Total at 10k contributors | Notes |
|---|---|---|---|
| Passport records | 155 bytes | ~1.55 MB | One entry per contributor |
| Credential records (live, max) | 25 KB (100 × 250 bytes) | ~250 MB | Upper bound; most contributors well below cap |
| Score snapshots (max) | 5.05 KB (50 × 101 bytes) | ~50 MB | Upper bound |
| Badge records (v1 max) | 1.09 KB (7 × 156 bytes) | ~10.9 MB | Upper bound at all 7 v1 badges earned |
| Archive records | 124 bytes per cycle | Negligible | ~1 cycle per 3.3 years per active contributor |

### Live network parameter requirement

`ARCHITECTURE.md` Section 1 flags that the `persistent_rent_rate_denominator` and
`fee_per_rent_1kb` values must be queried from `lab.stellar.org/network-limits` before
finalising cost projections for mainnet. This check is a prerequisite for issue #080
(mainnet deployment). The cost estimates above are directional; the implementation
issues (#016–#019) must use the live values at the time of mainnet deployment.

---

## 11. Extensibility Model

This section defines how the two reserved enum variant sets (`SignalType` and
`MilestoneType`) are extended in future versions, and what that extension requires
at the contract level vs the backend level. This is the forward-compatibility design
that allows new signal sources and badge types to be activated without full contract
redeployment when partnerships are confirmed via issue #009 (FR-09-B).

### Principle: contracts do not branch on SignalType or MilestoneType

Neither the credential store contract nor the soulbound NFT contract contains any
`match` or `if` statement on `SignalType` or `MilestoneType` variants. Both contracts
treat these as opaque discriminants used for storage keying and deduplication. All
signal-type-specific and milestone-specific logic lives in the NestJS backend.

This means: appending a new variant to either enum does not require any change to the
contract functions that accept or return those types. The change is limited to the
shared crate and the backend.

### Adding a new SignalType variant

**When:** a future signal source partnership is confirmed via issue #009 (e.g.
`GrantfoxBounty`, `TrustlessWork`, or `ScfGrant`).

**What changes:**

| Layer | Change required |
|---|---|
| `contracts/shared/src/lib.rs` | Append the new variant to `SignalType` (remove the `// Reserved` comment) |
| WASM upgrade | Rebuild all four contract crates (shared is a dependency) and upgrade via `invoke_host_function` with the new WASM |
| `contracts/INTERFACES.md` | Move the variant from Reserved to Active in Section 3.6 |
| `forgepass-core/scoring/algorithm-v1.0.json` | Add weight entry for the new signal type |
| `apps/api` | Add the corresponding indexer issue (R05, R06, or R07) and `ScoringService` weight |
| PostgreSQL | Add the new value to the `signal_type` enum in a migration |

**What does NOT change:** `add_credential`, `get_credentials`, `credential_exists`,
`get_credential_count`, `add_archive_record`, `remove_credentials` — none of these
functions require modification. The credential store contract stores the new variant
the same way it stores `GithubPr`.

### Adding a new MilestoneType variant

**When:** a future badge milestone is defined (e.g. `FirstBounty`, `FirstGrant`,
`FirstTrustlessWork`) after the corresponding signal source is confirmed and live.

**What changes:**

| Layer | Change required |
|---|---|
| `contracts/shared/src/lib.rs` | Append the new variant to `MilestoneType` (remove the `// Reserved` comment) |
| `contracts/badges/milestone-registry.json` | Add the new milestone definition with name, description, signal\_type, and trigger criteria |
| WASM upgrade | Rebuild and upgrade all four crates |
| `contracts/INTERFACES.md` | Move the variant from Reserved to Active in Section 3.7 |
| `apps/api` | Add milestone evaluation logic to `BadgeService.checkAndMint` |
| PostgreSQL | Add the new value to the `milestone_type` enum in a migration |

**What does NOT change:** `mint`, `get_badge`, `get_badges_for_wallet`, `has_badge` —
the soulbound NFT contract handles new variants without function-level changes.

### WASM upgrade process

Adding new enum variants requires a WASM upgrade because the XDR encoding of the enum
changes (a new discriminant is appended). Existing stored values remain valid — they
were serialised with the old discriminant values, which are unchanged by appending.
The upgrade path is:

```
1. Build new WASM with the updated shared/ crate
2. Deploy updated WASM via InvokeHostFunctionOp (upload_contract_wasm)
3. Upgrade each contract instance via InvokeHostFunctionOp (update_current_contract_wasm)
4. Verify: call a read function on each upgraded contract and confirm it returns
   existing stored values correctly
```

This upgrade does not change any function signatures, DataKey structures, or stored
data. It is a non-breaking WASM replacement.

### Extensibility boundary: what requires a new contract

The following changes are **not** achievable via WASM upgrade and require deploying
a new contract and migrating data:

- Adding a new parameter to an existing function signature
- Changing the return type of an existing function
- Adding a new storage key prefix that conflicts with existing DataKey entries
- Changing the `ContractError` discriminant values for existing variants

All such changes are out of scope for v1 and must be flagged as breaking changes in
any future design discussion.

---

## 12. Scope Boundary

This section explicitly defines what INTERFACES.md does **not** specify, so that
implementation issues (#016–#019) know exactly which decisions are left to them.

### Specified by this document

- All 23 function signatures (exact parameter names, types, and return types)
- All 5 `#[contracttype]` struct definitions (exact field names and types)
- All 3 enum definitions (`SignalType`, `MilestoneType`, `ContractError`) with all
  discriminants and reserved variants
- Storage tier assignment for each data category
- `DataKey` enum structure for each contract crate
- Access control level for each function (admin-bootstrap / admin-only / owner-only / public)
- The canonical `initialize` pattern and `require_admin` helper
- Auth failure model (host trap vs ContractError)
- Error codes each function can return
- Extensibility model for new enum variants

### Decisions left to implementation issues

| Decision | Implementation issue |
|---|---|
| How live credentials are stored internally (single `Vec<CredentialRecord>` per wallet vs individual entries per credential ID) | #019 |
| Whether to emit Soroban events on state-changing calls (e.g. `create_passport`, `mint`) | #016, #017, #018, #019 |
| Exact XDR-encoded size verification against SDK `22.0.11` limits | #016–#019 |
| Gas optimisation within function bodies | #016–#019 |
| Whether `remove_credentials` removes by `(signal_type, source_id)` pair or `source_id` alone | #019 |
| TTL extension scheduling and implementation | Backend (#027, #036) |
| Soroban test framework setup and test scenario implementation | #020 |
| Deployment script implementation and testnet addresses | #021 |
| Mainnet key management implementation (hardware wallet / multi-sig / MPC) | #080 |
| PostgreSQL schema, migrations, and indexes | #013, #030 |
| NestJS service layer (OnchainWriterService, ScoringService, BadgeService) | #027, #035, #037 |
| IPFS pinning integration | #044 |
| Soroban SDK JS invocation patterns (no JS in this document — see Section 2 decision) | #027 |

### Documents this file depends on

| Document | Relationship |
|---|---|
| `contracts/ARCHITECTURE.md` | Source of truth for archival workflow, Merkle design, IPFS schema, and credential cost model. Read before implementing #019. |
| `contracts/badges/milestone-registry.json` | Source of truth for `MilestoneType` trigger criteria. Read before implementing #018 and #037. |
| `contracts/docs/cost-model.md` | Full credential storage cost model committed in #003. Read before mainnet deployment (#080). |
| `forgepass-core/scoring/algorithm-v1.0.json` | Signal weights committed in #001. Used by NestJS scoring engine; references `SignalType` variant names. |

### Revision history

| Version | Date | Changes |
|---|---|---|
| v1.0 | 2026-05-23 | Initial release. Issue #014 complete. All 23 function signatures, 5 structs, 3 enums, access control matrix, initialisation sequence, extensibility model, and scope boundary defined. Three archival functions added to credential store interface from `ARCHITECTURE.md` Section 4 (not in original roadmap). `ArchiveRecord` added as fifth shared struct. `ArchiveRecordRequired` (302) added as 13th `ContractError` variant. `MilestoneType` enum corrected to use `RisingContributor` and `FullStackBuilder` per committed `milestone-registry.json` from #008. `ArchiveRecord` storage tier corrected to Persistent (roadmap had Instance). |



