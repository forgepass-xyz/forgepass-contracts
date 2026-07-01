# ForgePass Contract Integration Tests — Scenario Specification

**Issue:** #020 — Write Contract Integration Tests (Cross-Contract)
**Repo:** forgepass-contracts
**Path:** contracts/tests/integration/SCENARIO-SPEC.md
**Status:** Step 2 complete — approved for implementation in Step 3 and Step 4.

---

## Purpose

This document specifies each of the six integration test scenarios before
any test code is written. Every call sequence, expected state, assertion
field, and error discriminant is resolved here against `contracts/INTERFACES.md`
(the committed ABI from issue #014) and `contracts/shared/src/lib.rs`
(the committed `ContractError` enum from issue #016).

Step 3 (Scenarios 1--5) and Step 4 (Scenario 6) are straight translation
from this document. Any discrepancy between this spec and the committed
source is a spec error to correct here before implementation begins, not
a decision to make during implementation.

---

## Corrections from Roadmap v1.0

The original #020 roadmap scenario table used two error variant names that
do not match the committed `ContractError` enum in `contracts/shared/src/lib.rs`.
Both are corrected here. Use the names below in all test assertions.

| Roadmap name (stale) | Correct name | Discriminant | Contract |
|---|---|---|---|
| `ContractError::DuplicateCredential` | `ContractError::CredentialAlreadyExists` | 300 | credential-store |
| `ContractError::AlreadyMinted` | `ContractError::BadgeAlreadyMinted` | 500 | soulbound-nft |

---

## Resolved Open Questions

| OQ | Question | Resolution |
|---|---|---|
| OQ-1 | CI target: testnet on every push vs sandbox + one-time testnet gate | **Hybrid.** Local Soroban sandbox runs on every CI push. One full testnet pass is required as a manual gate before issue close, captured in `RESULTS.md`. |
| OQ-2 | Partial failure semantics for Scenario 6 | **Option A — on-chain rejection.** Call `anchor_score` with `score = 101`, which exceeds the valid 0--100 range and triggers `ContractError::InvalidScore` (400). Tests contract-level isolation directly. No mock of off-chain orchestration. |

---

## Test Fixtures

All six scenarios share a single set of fixtures, set up once in the
test harness (Step 1). These values are referenced by name throughout
the scenario tables below.

| Fixture | Description | Notes |
|---|---|---|
| `env` | `soroban_sdk::Env::default()` | Single shared test environment for all contracts |
| `admin` | `Address::generate(&env)` | Used to `initialize` all four contracts; signs all admin-only calls |
| `contributor` | `Address::generate(&env)` | The test contributor wallet; used as the `wallet` parameter across all scenarios |
| `non_admin` | `Address::generate(&env)` | A wallet with no admin role; used for negative-path assertions only |
| `passport_client` | `PassportContractClient` registered in `env` | Client for `forgepass-passport` |
| `credential_client` | `CredentialStoreContractClient` registered in `env` | Client for `forgepass-credential-store` |
| `score_client` | `TrustScoreContractClient` registered in `env` | Client for `forgepass-trust-score` |
| `badge_client` | `SoulboundNftContractClient` registered in `env` | Client for `forgepass-soulbound-nft` |

All four contracts are initialised with the same `admin` address before
any scenario runs. Use `env.mock_all_auths()` to satisfy `admin.require_auth()`
on admin-only calls within the test environment. Do not use `mock_all_auths()`
in negative-path tests where auth rejection must be observed.

---

## Scenario 1 — Full Passport-to-Badge Flow

**Tests:** the complete on-chain sequence that mirrors the real-world path
#027 will execute for a new merged PR event. Proves each contract's
independent write is readable by any other contract's read call, with no
contract calling another directly (standalone design per INTERFACES.md
Section 2).

### Call sequence

| Step | Contract | Function | Parameters |
|---|---|---|---|
| 1 | passport | `create_passport` | `wallet: contributor`, `ipfs_cid: "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"` |
| 2 | credential-store | `add_credential` | `wallet: contributor`, `signal_type: SignalType::GithubPr`, `source_id: "stellar-org/rs-soroban-sdk#1234"`, `event_date: 1_700_000_000`, `data_hash: "a".repeat(64)` |
| 3 | trust-score | `anchor_score` | `wallet: contributor`, `score: 42`, `algorithm_version: "1.0"`, `signal_hash: "b".repeat(64)`, `computed_at: 1_700_000_001` |
| 4 | soulbound-nft | `mint` | `wallet: contributor`, `milestone_type: MilestoneType::FirstPr`, `ipfs_cid: "bafybeifirstprbadge"`, `minted_at: 1_700_000_002` |

### Assertions

| # | Read call | Assert |
|---|---|---|
| A1 | `passport_client.get_passport(&contributor)` | Returns `Some(PassportRecord)` where `record.wallet == contributor`, `record.sybil_flagged == false`, `record.ipfs_cid == "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"` |
| A2 | `credential_client.get_credentials(&contributor)` | Returns `Vec` of length 1; `records[0].signal_type == SignalType::GithubPr`, `records[0].source_id == "stellar-org/rs-soroban-sdk#1234"`, `records[0].data_hash == "a".repeat(64)` |
| A3 | `credential_client.get_credential_count(&contributor)` | Returns `1u32` |
| A4 | `score_client.get_current_score(&contributor)` | Returns `Some(ScoreSnapshot)` where `snapshot.score == 42`, `snapshot.algorithm_version == "1.0"` |
| A5 | `badge_client.has_badge(&contributor, &MilestoneType::FirstPr)` | Returns `true` |
| A6 | `badge_client.get_badges_for_wallet(&contributor)` | Returns `Vec` of length 1; `badges[0].milestone_type == MilestoneType::FirstPr`, `badges[0].wallet == contributor` |

### Key assertions for seam validation

- A2 and A3 together confirm the credential written in Step 2 is unaffected
  by the score anchor (Step 3) and badge mint (Step 4) that follow it.
- A5 uses `has_badge` (O(1) instance lookup) -- confirms the `HasBadge`
  instance entry was written correctly by `mint`, independent of `get_badges_for_wallet`.
- No contract called another contract's function during Steps 1--4. All
  state changes are independent ledger entries.

---

## Scenario 2 — Sybil-Flagged Passport

**Tests:** sybil flagging does not block on-chain writes or reads. The
contract stores and returns full state regardless of flag value. Off-chain
filtering (FR-11.1) is an API layer concern, not a contract concern.

### Call sequence

| Step | Contract | Function | Parameters |
|---|---|---|---|
| 1 | passport | `create_passport` | `wallet: contributor`, `ipfs_cid: "bafybeiscenario2"` |
| 2 | passport | `set_sybil_flag` | `wallet: contributor`, `flagged: true` |
| 3 | trust-score | `anchor_score` | `wallet: contributor`, `score: 55`, `algorithm_version: "1.0"`, `signal_hash: "c".repeat(64)`, `computed_at: 1_700_001_000` |

### Assertions

| # | Read call | Assert |
|---|---|---|
| A1 | `passport_client.is_valid(&contributor)` | Returns `false` -- sybil flag active |
| A2 | `passport_client.get_passport(&contributor)` | Returns `Some(PassportRecord)` where `record.sybil_flagged == true` -- record is fully readable on-chain |
| A3 | `score_client.anchor_score(...)` result from Step 3 | Returns `Ok(())` -- sybil flag does not block `anchor_score` |
| A4 | `score_client.get_current_score(&contributor)` | Returns `Some(ScoreSnapshot)` where `snapshot.score == 55` -- score readable after flag |

### Key assertions for seam validation

- A1 and A2 together confirm `is_valid` returns `false` while `get_passport`
  returns the full record -- both behaviours documented in INTERFACES.md
  Section 4 design notes.
- A3 confirms the sybil flag is a passport-contract-only state field; the
  trust-score contract has no knowledge of it and cannot read it.

---

## Scenario 3 — Credential Deduplication

**Tests:** `add_credential` returns `ContractError::CredentialAlreadyExists`
(300) on a duplicate `(signal_type, source_id)` pair, even after intervening
calls to other contracts have processed the original credential.

### Call sequence

| Step | Contract | Function | Parameters |
|---|---|---|---|
| 1 | passport | `create_passport` | `wallet: contributor`, `ipfs_cid: "bafybeiscenario3"` |
| 2 | credential-store | `add_credential` (first) | `wallet: contributor`, `signal_type: SignalType::SorobanContract`, `source_id: "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM"`, `event_date: 1_700_002_000`, `data_hash: "d".repeat(64)` |
| 3 | trust-score | `anchor_score` | `wallet: contributor`, `score: 30`, `algorithm_version: "1.0"`, `signal_hash: "e".repeat(64)`, `computed_at: 1_700_002_001` |
| 4 | credential-store | `add_credential` (duplicate) | Same parameters as Step 2 |

### Assertions

| # | Call | Assert |
|---|---|---|
| A1 | Step 2 result | Returns `Ok(credential_id)` where `credential_id >= 1` |
| A2 | Step 4 result | Returns `Err(ContractError::CredentialAlreadyExists)` (discriminant 300) |
| A3 | `credential_client.get_credential_count(&contributor)` after Step 4 | Returns `1u32` -- count did not change |
| A4 | `credential_client.get_credentials(&contributor)` after Step 4 | Returns `Vec` of length 1 -- no duplicate entry |

### Key assertions for seam validation

- A2 confirms the correct error variant name: `CredentialAlreadyExists`, not
  `DuplicateCredential` (the stale roadmap name).
- A3 and A4 confirm deduplication state is not reset by the intervening
  `anchor_score` call in Step 3 -- dedup is per `(signal_type, source_id)` pair,
  not per session.

---

## Scenario 4 — Score History Accumulation

**Tests:** `anchor_score` correctly appends to history in chronological order,
`get_score_history` returns entries ascending by `computed_at`, and
`get_current_score` always reflects the most recent snapshot.

### Call sequence

| Step | Contract | Function | Parameters |
|---|---|---|---|
| 1 | passport | `create_passport` | `wallet: contributor`, `ipfs_cid: "bafybeiscenario4"` |
| 2 | trust-score | `anchor_score` | `wallet: contributor`, `score: 40`, `algorithm_version: "1.0"`, `signal_hash: "f".repeat(64)`, `computed_at: 1_700_003_000` |
| 3 | trust-score | `anchor_score` | `wallet: contributor`, `score: 55`, `algorithm_version: "1.0"`, `signal_hash: "g".repeat(64)`, `computed_at: 1_700_003_001` |
| 4 | trust-score | `anchor_score` | `wallet: contributor`, `score: 68`, `algorithm_version: "1.0"`, `signal_hash: "h".repeat(64)`, `computed_at: 1_700_003_002` |

### Assertions

| # | Read call | Assert |
|---|---|---|
| A1 | `score_client.get_current_score(&contributor)` after Step 4 | Returns `Some(ScoreSnapshot)` where `snapshot.score == 68` |
| A2 | `score_client.get_score_history(&contributor)` | Returns `Vec` of length 3 |
| A3 | History ordering | `history[0].score == 40`, `history[1].score == 55`, `history[2].score == 68` -- ascending by `computed_at` |
| A4 | History `computed_at` ordering | `history[0].computed_at == 1_700_003_000`, `history[1].computed_at == 1_700_003_001`, `history[2].computed_at == 1_700_003_002` |

### Key assertions for seam validation

- A3 and A4 assert on both `score` and `computed_at` ordering -- a future
  regression where insertion order is not preserved would fail both.
- This scenario does not test the 50-snapshot cap (would require 51 calls);
  that boundary is covered by #017's own unit tests.

---

## Scenario 5 — Badge Duplicate Prevention

**Tests:** `mint` returns `ContractError::BadgeAlreadyMinted` (500) on a
second mint attempt for the same `(wallet, MilestoneType)` pair. The
`badge_id` counter does not increment for the rejected call.

### Call sequence

| Step | Contract | Function | Parameters |
|---|---|---|---|
| 1 | passport | `create_passport` | `wallet: contributor`, `ipfs_cid: "bafybeiscenario5"` |
| 2 | soulbound-nft | `mint` (first) | `wallet: contributor`, `milestone_type: MilestoneType::FirstPr`, `ipfs_cid: "bafybeifirstprbadge"`, `minted_at: 1_700_004_000` |
| 3 | soulbound-nft | `mint` (duplicate) | Same parameters as Step 2 |

### Assertions

| # | Call | Assert |
|---|---|---|
| A1 | Step 2 result | Returns `Ok(badge_id)` where `badge_id >= 1` -- call `first_badge_id = badge_id` |
| A2 | Step 3 result | Returns `Err(ContractError::BadgeAlreadyMinted)` (discriminant 500) |
| A3 | `badge_client.get_badges_for_wallet(&contributor)` after Step 3 | Returns `Vec` of length 1 |
| A4 | `badge_client.has_badge(&contributor, &MilestoneType::FirstPr)` after Step 3 | Returns `true` |
| A5 | Badge counter did not advance | Mint a new badge for a different milestone type (e.g. `FirstContract`) after Step 3; its returned `badge_id` must equal `first_badge_id + 1`, not `first_badge_id + 2` |

### Key assertions for seam validation

- A2 confirms the correct error variant name: `BadgeAlreadyMinted`, not
  `AlreadyMinted` (the stale roadmap name).
- A5 is the critical counter assertion: proves the rejected Step 3 call did
  not consume a `badge_id`. A regression where the counter increments on
  rejected mints would produce `badge_id = first_badge_id + 2` here.

---

## Scenario 6 — Partial Failure (On-Chain Rejection)

**Tests:** a credential write that commits successfully is not rolled back
when a subsequent `anchor_score` call is rejected at the contract level.
Proves contracts commit independently -- there is no cross-contract
atomicity in Soroban.

**Failure mode (OQ-2, Option A):** call `anchor_score` with `score = 101`,
which exceeds the valid 0--100 range. The trust-score contract returns
`ContractError::InvalidScore` (400). The credential-store contract is not
involved in this rejection; its state is unaffected.

### Call sequence

| Step | Contract | Function | Parameters |
|---|---|---|---|
| 1 | passport | `create_passport` | `wallet: contributor`, `ipfs_cid: "bafybeiscenario6"` |
| 2 | credential-store | `add_credential` | `wallet: contributor`, `signal_type: SignalType::GithubPr`, `source_id: "stellar-org/stellar-core#5678"`, `event_date: 1_700_005_000`, `data_hash: "i".repeat(64)` |
| 3 | Capture state snapshot | `credential_client.get_credentials(&contributor)` | Record `before_credentials` and `before_count` |
| 4 | trust-score | `anchor_score` (intentional failure) | `wallet: contributor`, `score: 101`, `algorithm_version: "1.0"`, `signal_hash: "j".repeat(64)`, `computed_at: 1_700_005_001` |
| 5 | Capture state snapshot | `credential_client.get_credentials(&contributor)` | Record `after_credentials` and `after_count` |

### Assertions

| # | Call | Assert |
|---|---|---|
| A1 | Step 2 result | Returns `Ok(credential_id)` -- credential write succeeded |
| A2 | Step 4 result | Returns `Err(ContractError::InvalidScore)` (discriminant 400) |
| A3 | `before_count` vs `after_count` | Both equal `1u32` -- count unchanged by the failed anchor call |
| A4 | `before_credentials` vs `after_credentials` | Both Vecs have length 1 and identical contents -- credential not modified |
| A5 | `score_client.get_current_score(&contributor)` after Step 4 | Returns `None` -- no score was anchored; the failed call left no partial state |

### Key assertions for seam validation

- A3 and A4 use explicit before/after snapshots rather than a single post-hoc
  read. A regression where the credential is silently removed by the failed
  `anchor_score` call would fail A3 and A4.
- A5 confirms the rejection was clean -- no partial score record was written
  to the trust-score contract before the validation check fired.
- The test comment must document: "This test proves the assumption that
  OnchainWriterService (#027) depends on: a failed anchor_score call does
  not affect credential_store state. The failure mode tested is on-chain
  rejection (score out of range), not off-chain submission failure."

---

## Step 1 Harness Notes (for implementation reference)

- All four contracts are deployed in one shared `soroban_sdk::Env`.
- All four `initialize` calls use the same `admin` address.
- Use `env.mock_all_auths()` in all positive-path tests.
- For `test_non_admin_cannot_create_passport` style negative tests, do not
  mock auths -- let the host trap fire.
- Each scenario function resets fixture state by creating a fresh `env` and
  re-registering all contracts (Soroban test envs are cheap to construct).
  Do not share mutable contract state across scenarios.
- Local sandbox invocation: `cargo test -p integration --target x86_64-pc-windows-gnu`
  (Windows) or `cargo test -p integration` (Linux CI).
- Testnet invocation: documented separately in `RESULTS.md` template (Step 5).

---

## Acceptance Criteria Cross-Reference

| AC | Criterion | Covered by |
|---|---|---|
| AC-1 | All six tests pass on Stellar testnet | Scenarios 1--6; Step 5 RESULTS.md |
| AC-2 | Partial failure scenario explicitly tested with before/after assertions | Scenario 6 (A3, A4, A5) |
| AC-3 | Each of the five named scenarios has a corresponding test function | Scenarios 1--5 |
| AC-4 | Test failures attributable to a single contract or seam | Each assertion names the specific contract client and field |

---

## Revision History

| Version | Date | Changes |
|---|---|---|
| v1.0 | 2026-06-29 | Initial release. Six scenarios specified. OQ-1 resolved: hybrid CI strategy (sandbox on every push, testnet gate before close). OQ-2 resolved: Option A on-chain rejection via score = 101. Two error variant name corrections applied: CredentialAlreadyExists (300) and BadgeAlreadyMinted (500). All assertions cross-referenced against INTERFACES.md v1.0 and ContractError discriminants from contracts/shared/src/lib.rs. |