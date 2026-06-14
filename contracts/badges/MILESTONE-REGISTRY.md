# ForgePass · Milestone Badge Registry
# MILESTONE-REGISTRY.md · v1.0
# Companion to milestone-registry.json · Issue #008 · Phase 0

---

## 1. Overview

This document defines the complete ForgePass v1 badge milestone set. It is the authoritative human-readable reference for the `milestone-registry.json` file in this directory. All badge types defined here cover GitHub and Soroban signals only. Bounty, grant, and Trustless Work milestone types are reserved for future versions when those signal sources are introduced (see Section 5).

**FRD references:** FR-05-A, FR-05.1, FR-05.3, FR-05.4, FR-05.7

**Unblocks:** #018 (Soulbound NFT contract), #013 (PostgreSQL schema milestone_type enum), #037 (BadgeService pipeline), informs #003 (on-chain storage cost model), informs #014 (Soroban contract ABI)

---

## 2. Research Findings Summary

Six badge and achievement systems were surveyed before any milestone thresholds were proposed. Key findings that shaped every threshold and design decision in this registry:

**GitHub Achievements (Pull Shark):** Base tier at 2 merged PRs is earned by virtually every active GitHub contributor, providing zero differentiation. First meaningful tier is Bronze (16 PRs). Trivial PRs (whitespace, single character) into own repos count toward tier thresholds. ForgePass mitigation: registered-repo restriction blocks self-merge gaming by requiring PRs to be merged into admin-curated Stellar ecosystem projects.

**Gitcoin Passport / Human Passport:** Flat binary stamps optimised toward cheapest-path score accumulation. Low-effort stamps (Discord join, Twitter follow) dilute the composite score. Acquired by Holonym Foundation December 2024. Lesson: binary gates with equal weight produce cheapest-path gaming. ForgePass uses quantitative thresholds, not binary presence.

**Galxe OATs:** 66M+ tokens distributed. Permissionless campaign creation is the primary inflation vector. OAT count carries no credibility signal because any organiser can create campaigns for trivial actions. Lesson: admin-controlled badge issuance is the correct architecture for preserving signal value.

**POAP:** Virtual event farming (join, collect, disconnect) demonstrates that presence-based credentials cannot distinguish genuine participation from opportunistic collection. ForgePass HACKATHON_PARTICIPANT is admin-verified, not self-claimed.

**Stackup Developer Quests:** Requires technical execution output (deployed address, test results, working repo). Substantially more resistant to gaming than social-action systems. Validates the approach of requiring verifiable on-chain outputs for Soroban badge triggers.

**Stellar Quest (SDF):** NFT badges minted on Stellar for technically-executed challenges, verified via on-chain operation submission. XLM reward farming occurred when monetary incentives were attached; badge-specific farming was not documented at scale because badges carry no monetary value and cannot be traded. SCF uses Stellar Quest Level 1 badges as Pathfinder governance eligibility, directly validating that on-chain technically-executed credentials are trusted reputation signals in the Stellar ecosystem. ForgePass badges are soulbound and non-transferable, eliminating monetary arbitrage as an attack vector.

**ForgePass-specific constraints derived from this survey:**
- Registered-repo restriction is the primary defence against PR self-merge gaming.
- Soulbound plus no monetary value eliminates the primary farming attack documented in Stellar Quest.
- Base mandatory badges are entry markers, not differentiation signals. Compound badges carry the differentiation weight.
- On-chain technical execution (merged PR state, create_contract operation) is substantially more gaming-resistant than social-action or presence-based credentials.
- Cross-signal compound badge thresholds must require independently meaningful activity in each domain, not just one qualifying event each.

---

## 3. Complete v1 Badge Set

| # | Milestone type | Signal(s) | Threshold | Cardinality | Status |
|---|---|---|---|---|---|
| 1 | `FIRST_PR` | GITHUB_PR | 1 merged PR (author) in registered Stellar repo | One-time | Active |
| 2 | `FIRST_CONTRACT` | SOROBAN_CONTRACT | 1 create_contract from linked wallet | One-time | Active |
| 3 | `HACKATHON_PARTICIPANT` | HACKATHON | Admin-verified event participation | One per event | Active |
| 4 | `RISING_CONTRIBUTOR` | GITHUB_PR | 10 merged PRs in registered Stellar repos | One-time | Active |
| 5 | `MULTI_REPO_CONTRIBUTOR` | GITHUB_PR | 1 merged PR each in 3 distinct registered repos | One-time | Active |
| 6 | `FIRST_SOROBAN_INVOCATION` | SOROBAN_CONTRACT | 1 invocation on any deployed contract (any caller) | One-time | Active |
| 7 | `FULL_STACK_BUILDER` | GITHUB_PR + SOROBAN_CONTRACT | >= 1 GITHUB_PR credential AND >= 1 SOROBAN_CONTRACT credential | One-time | Active |

---

## 4. Badge Type Specifications

### 4.1 FIRST_PR

**Name:** First Contribution
**Description:** Merged your first pull request into a registered Stellar ecosystem repository.

**Trigger criteria:**
- Signal type: GITHUB_PR
- Condition: merged PR count >= 1
- Repo scope: registered Stellar repos only (ForgePass admin-controlled registry)
- Authorship: PR author only; reviewers do not qualify
- PR state: fully merged only; approved-but-unmerged and closed-without-merge are excluded
- Content type: any (code, documentation, CI config, dependency updates)
- Retroactive: yes, mints on first index if qualifying history exists

**Duplicate prevention key:** `(wallet_address, milestone_type)`

**Design decisions recorded:**
- Registered repos only: prevents self-merge gaming; consistent with cross-project indexer scope (FR-03.11)
- Any content type: content classification at indexer level is operationally fragile; quality is a Trust Score concern
- Author only: review credit belongs in a future FIRST_REVIEW badge, not as a qualifying path to FIRST_PR
- Retroactive: penalising early Stellar contributors for creating their passport after their first PR contradicts ForgePass's purpose

---

### 4.2 FIRST_CONTRACT

**Name:** First Deployment
**Description:** Deployed your first Soroban smart contract on Stellar.

**Trigger criteria:**
- Signal type: SOROBAN_CONTRACT
- Condition: create_contract operation count >= 1
- Operation type: create_contract (instantiation) only; upload_contract_wasm alone does not qualify
- Wallet scope: ForgePass-linked Stellar wallet only; deployments from unlinked wallets excluded
- WASM size floor: none; any instantiated contract qualifies
- Invocation required: no; instantiation alone is sufficient
- Retroactive: yes

**Duplicate prevention key:** `(wallet_address, milestone_type)`

**Design decisions recorded:**
- create_contract only: produces a verifiable contract address; upload_contract_wasm without instantiation is an incomplete deployment
- No WASM size floor: Soroban SDK compilation overhead makes truly empty contracts impractical; size complexity is a Trust Score concern
- No invocation requirement: invocation depth belongs in Trust Score algorithm and progressive compound badges, not a first-milestone gate
- Linked wallet only: v1 identity model is one wallet per passport; multi-wallet support requires a future FR with its own verification flow

---

### 4.3 HACKATHON_PARTICIPANT

**Name:** Hackathon Participant
**Description:** Participated in a verified Stellar ecosystem hackathon.

**Trigger criteria:**
- Signal type: HACKATHON
- Condition: HACKATHON credential exists for a specific admin-registered event
- Event scope: admin-registered Stellar ecosystem events only; self-reported participation never qualifies
- Trigger path: admin CSV ingest writes HACKATHON credential, then BadgeService.checkAndMint evaluates
- Minting: immediate on validated CSV ingestion; dry-run mode is optional pre-flight, not a mandatory gate
- Retroactive: yes

**Duplicate prevention key:** `(wallet_address, milestone_type, event_id)`

**Admin CSV schema:**

| Column | Required | Notes |
|---|---|---|
| wallet_address | Yes | Contributor's ForgePass-linked Stellar public key |
| event_id | Yes | Admin-assigned slug (e.g. stellar-meridian-2026); must match a pre-registered event record |
| event_name | Yes | Human-readable event name; stored in IPFS metadata |
| event_date | Yes | ISO 8601 date (e.g. 2026-04-12) |
| placement | No | 1st, 2nd, 3rd, finalist, participant, or null |

**IPFS metadata includes:** event_id, event_name, event_date, placement (nullable)

**Design decisions recorded:**
- One per event (not one-time): accumulated hackathon history is the signal; three badges shows more ecosystem engagement than one
- Admin-registered events only: validates ForgePass as a verified credential, not a self-claimed one
- Placement captured now: data cannot be reconstructed retroactively after ingestion; preserves ability to evaluate future placement-tier compound badges
- Credential-first trigger path: keeps all badge minting logic in BadgeService; avoids duplicating logic in the admin ingest endpoint
- event_id from admin-assigned slug: prevents double-minting from CSV naming inconsistencies

---

### 4.4 RISING_CONTRIBUTOR

**Name:** Rising Contributor
**Description:** Merged 10 or more pull requests into registered Stellar ecosystem repositories.

**Trigger criteria:**
- Signal type: GITHUB_PR
- Condition: merged PR count >= 10
- Same scope as FIRST_PR (registered repos, merged state, author only, any content type)
- Retroactive: yes

**Duplicate prevention key:** `(wallet_address, milestone_type)`

**Threshold rationale:** GitHub Pull Shark base tier (2 PRs, any repo) is earned by virtually every active contributor and provides no differentiation. Bronze (16 PRs) is the first meaningful tier in a global GitHub context. ForgePass's registered-repo restriction raises the bar: 10 merged PRs into curated Stellar ecosystem projects represents 3 to 6 months of consistent contribution at a realistic cadence. It is achievable without requiring years of activity, but filters contributors who merged one batch and went quiet.

---

### 4.5 MULTI_REPO_CONTRIBUTOR

**Name:** Multi-Repo Contributor
**Description:** Merged pull requests into 3 or more distinct registered Stellar ecosystem repositories.

**Trigger criteria:**
- Signal type: GITHUB_PR
- Condition: distinct repo count >= 3 (each repo must have at least 1 merged PR authored by the contributor)
- Same scope as FIRST_PR for individual PRs
- Retroactive: yes

**Duplicate prevention key:** `(wallet_address, milestone_type)`

**BadgeService implementation note:** Requires `COUNT(DISTINCT repo_name)` query on the credentials table filtered by `wallet_address`, `signal_type = GITHUB_PR`, `state = merged`. The `repo_name` column must be indexed on GITHUB_PR credential rows (flag on #013).

**Threshold rationale:** Two distinct repos provides minimal signal above FIRST_PR and is almost always incidentally satisfied by contributors who reach RISING_CONTRIBUTOR. Five repos may be exclusionary in a small ecosystem and could reward superficial spreading over deep contribution. Three repos represents genuine cross-project engagement and is distinct from the volume signal that RISING_CONTRIBUTOR captures.

---

### 4.6 FIRST_SOROBAN_INVOCATION

**Name:** First Invocation
**Description:** One of your deployed Soroban contracts has been called for the first time.

**Trigger criteria:**
- Signal type: SOROBAN_CONTRACT
- Condition: invocation_count >= 1 on any contract deployed by the contributor's linked wallet
- Caller scope: any caller including the contributor's own linked wallet
- Retroactive: yes, mints on first index if any deployed contract already has invocation_count >= 1

**Duplicate prevention key:** `(wallet_address, milestone_type)`

**BadgeService implementation note:** Evaluates `EXISTS (SELECT 1 FROM credentials WHERE wallet_address = $1 AND signal_type = 'SOROBAN_CONTRACT' AND invocation_count >= 1)`. The `invocation_count` field must be refreshed on every incremental Soroban index cycle, not only on initial contract detection (flag on #033).

**Design decisions recorded:**
- Any caller: distinguishing self-invocations requires per-operation source account inspection on every indexed contract on every cycle; the badge marks contract liveness, not external adoption
- Threshold of 1: progressive invocation milestones belong in a future compound badge when Stellar ecosystem contract usage patterns are better understood
- Invocation count in Trust Score: adoption depth is already handled by SOROBAN_CONTRACT invocation volume weighting in algorithm-v1.0.json

---

### 4.7 FULL_STACK_BUILDER

**Name:** Full-Stack Builder
**Description:** Contributed code to the Stellar ecosystem on GitHub and deployed a Soroban smart contract.

**Trigger criteria:**
- Signal types: GITHUB_PR AND SOROBAN_CONTRACT
- Condition: ALL_OF
  - GITHUB_PR_COUNT_GTE 1 (same filters as FIRST_PR)
  - SOROBAN_CONTRACT_COUNT_GTE 1 (same filters as FIRST_CONTRACT)
- Trigger basis: underlying credentials, not badge mint state
- Retroactive: yes; may mint in the same checkAndMint pass as FIRST_PR and FIRST_CONTRACT

**Duplicate prevention key:** `(wallet_address, milestone_type)`

**Design decisions recorded:**
- Credential-based trigger: badge mint state of FIRST_PR and FIRST_CONTRACT does not gate this evaluation; a transient on-chain write failure on FIRST_PR does not block FULL_STACK_BUILDER
- Single-pass evaluation: BadgeService evaluates all milestone criteria in one checkAndMint call; all qualifying badges mint in the same pass
- Cross-signal threshold: credentials at count >= 1 in each domain; the differentiation from FIRST_PR and FIRST_CONTRACT alone is the co-presence of both signal types, not a higher count in either

---

## 5. Extensibility Model

### 5.1 How to add a new milestone type

The following steps are required when a new signal source is introduced (e.g. GrantFox bounties, SCF grants, Trustless Work escrows):

1. Confirm the signal source is active (the corresponding indexer or ingest endpoint is live and producing credentials)
2. Add the new milestone_type entry to the `milestones` array in `milestone-registry.json`, moving it from `reserved_future_milestones`
3. Add the new MilestoneType enum variant to the `soulbound_nft` Soroban contract source
4. Upload the new contract WASM and call `update_current_contract_wasm` to upgrade in place (Soroban contract upgrade mechanism preserves the contract address and all existing badge records)
5. Run a database migration to add the new milestone_type enum value to the PostgreSQL `badges` table
6. Deploy updated BadgeService with trigger logic for the new condition
7. Increment `registry_version` and add a changelog entry

**Key distinction:** this is a contract upgrade, not a redeployment. The contract address and all existing badge history are preserved. FR-05.7 requires that new milestone types can be added without contract redeployment; a Soroban in-place upgrade satisfies this requirement.

### 5.2 Reserved future milestone types

The following milestone types are reserved. No badge of these types will mint until the corresponding signal source is active and the type is moved from `reserved_future_milestones` to `milestones` in the registry.

| Milestone type | Signal source | Gated on |
|---|---|---|
| `FIRST_BOUNTY` | GRANTFOX_BOUNTY | #009 confirming GrantFox partnership AND R05 (GrantFox indexer) active |
| `FIRST_GRANT` | SCF_GRANT | #009 confirming SCF partnership AND R07 (SCF grant history ingestion) active |
| `FIRST_TRUSTLESS_WORK` | TRUSTLESS_WORK | #009 confirming Trustless Work partnership AND R06 (Trustless Work credential feed) active |

### 5.3 Naming convention

All milestone_type values use SCREAMING_SNAKE_CASE. New types must follow this convention. Identifiers are permanent once a badge has been minted against them; do not rename or remove active milestone types from the registry.

---

## 6. Schema Impacts

The following impacts were identified during the #008 decision process and must be communicated to the dependent issues before they are finalised.

### Flag on #013 (PostgreSQL database schema)

- `credentials` table: add `repo_name VARCHAR(255) NULL` column for GITHUB_PR signal type rows, indexed alongside `signal_type` and `wallet_address` to support `COUNT(DISTINCT repo_name)` queries for MULTI_REPO_CONTRIBUTOR evaluation.
- `credentials` table: add `invocation_count INTEGER NOT NULL DEFAULT 0` column for SOROBAN_CONTRACT signal type rows, updated on every incremental Soroban index cycle.
- `credentials` table: add `event_id VARCHAR(255) NULL` column for HACKATHON signal type rows, indexed for duplicate-prevention lookups.
- `credentials` table: add `placement VARCHAR(50) NULL` column for HACKATHON signal type rows.
- `badges` table: add `event_id VARCHAR(255) NULL` column (null for non-event badges; populated for HACKATHON_PARTICIPANT).
- `milestone_type` enum: initial v1 values: FIRST_PR, FIRST_CONTRACT, HACKATHON_PARTICIPANT, RISING_CONTRIBUTOR, MULTI_REPO_CONTRIBUTOR, FIRST_SOROBAN_INVOCATION, FULL_STACK_BUILDER. Document FIRST_BOUNTY, FIRST_GRANT, and FIRST_TRUSTLESS_WORK as reserved future values in the migration comment.

### Flag on #018 (Soulbound NFT contract)

- `MilestoneType` enum in the contract must include all 7 active v1 values. FIRST_BOUNTY, FIRST_GRANT, and FIRST_TRUSTLESS_WORK must NOT be included in the v1 contract; they are added via WASM upgrade when those signal sources go live.
- HACKATHON_PARTICIPANT duplicate prevention is BadgeService-owned (PostgreSQL `(wallet_address, milestone_type, event_id)` lookup) before calling `mint`. The contract does not need an event-scoped `has_badge` function in v1.
- Document the WASM upgrade path for adding new MilestoneType variants in `contracts/ARCHITECTURE.md`.

### Flag on #014 (Soroban contract ABI)

- No contract-level changes required for FIRST_PR, FIRST_CONTRACT, RISING_CONTRIBUTOR, MULTI_REPO_CONTRIBUTOR, or FULL_STACK_BUILDER duplicate prevention.
- FULL_STACK_BUILDER is evaluated entirely in BadgeService against the credentials table; no contract function is needed for its trigger logic.

### Flag on #033 (Soroban contract deployment indexer)

- `invocation_count` must be fetched and updated on every incremental index cycle for all contracts previously indexed for the contributor, not only on initial contract detection. FIRST_SOROBAN_INVOCATION evaluation in BadgeService depends on this field being current.

---

## 7. Open Questions at Close

None. All sub-questions for all 7 badge types were resolved during the #008 decision process. The following items are noted as future decisions, not open questions for this issue:

- Specific invocation count thresholds for a future progressive Soroban badge (e.g. ACTIVE_CONTRACT at 100 invocations) are deferred to the registry update that adds that badge type.
- Placement-tier compound badges (e.g. HACKATHON_WINNER) are deferred to a future registry update; placement data is being captured now to enable retroactive evaluation.
- RISING_CONTRIBUTOR threshold recalibration: the 10-PR threshold should be reviewed after 6 months of live data to confirm it represents genuine minority achievement in the registered Stellar ecosystem contributor population.

---

## 8. Revision History

| Version | Date | Changes |
|---|---|---|
| 1.0 | 2026-06-14 | Initial release. Seven active milestone types defined. Three future types reserved. Extensibility model documented. Schema impacts flagged for #013, #014, #018, #033. Issue #008 phase-0 decision. |
