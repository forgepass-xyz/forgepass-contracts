# ForgePass Contracts — Architecture

> This document is the primary design reference for the `forgepass-contracts` repository.
> It must be read before beginning work on issues #011 (passport contract scaffold),
> #014 (contract interface design), #019 (credential store contract), and #030
> (database migrations).
>
> **Status:** Complete. All sections written (Phase 0 Steps 1–5). Ready to commit.

---

## 1. Soroban Storage Mechanics (as of 2026-05-21)

### Storage type used for credentials

All ForgePass credential entries use **`ContractData` persistent storage**
(`env.storage().persistent()`). This is the correct choice because:

- Persistent entries are **archived** (not deleted) when TTL reaches zero. A credential
  that has been archived can be restored — it is never permanently lost.
- Temporary storage is deleted permanently on expiry — unacceptable for credentials.
- Instance storage shares a single TTL and has a 64 KB size cap — unsuitable for
  per-user unbounded credential sets.

### Fee structure (post-Protocol 23)

Protocol 23 (CAP-0062/0066, live on Stellar mainnet since September 2025) moved all
live Soroban state into validator RAM. This eliminated per-byte **read fees** for live
state. The costs that remain for ForgePass are:

- **Write fee** — charged once per `add_credential` call (non-recurring)
- **Ledger space rent** — ongoing cost to keep credentials live, paid via `extend_ttl()`

### Rent formula (canonical source: `fees.rs`)

Source: [`stellar/rs-soroban-env — fees.rs`](https://github.com/stellar/rs-soroban-env/blob/main/soroban-env-host/src/fees.rs)

```
rent_fee_stroops =
  (size_bytes × fee_per_rent_1kb × rent_ledgers)
  ─────────────────────────────────────────────
         (1024 × persistent_rent_rate_denominator)
```

| Parameter | Value | Notes |
|---|---|---|
| `fee_per_rent_1kb` | Dynamic — minimum floor **1,000 stroops/KB** | Hardcoded in `fees.rs` as `MINIMUM_RENT_WRITE_FEE_PER_1KB`. Actual mainnet value scales with Soroban state size. **Must query live before finalising.** |
| `persistent_rent_rate_denominator` | ~2,232,000 ledgers (mainnet-typical) | Network config parameter. **Must verify at `lab.stellar.org/network-limits`**. |
| Minimum persistent TTL (on creation) | **4,095 ledgers** (~23.8 days at 5s/ledger) | Enforced by protocol. Backend must begin TTL monitoring from passport creation. |
| Maximum TTL extension | Network config — see `lab.stellar.org/network-limits` | Entries can be extended up to the network maximum on any ledger. |

### TTL and rent extension

Each `ContractData` entry has its own TTL (`liveUntilLedger`). For persistent entries:

- When `liveUntilLedger` is reached, the entry is **archived** (not deleted) and
  inaccessible until restored.
- As of Protocol 23, archived entries are automatically restored when accessed via
  `InvokeHostFunctionOp` (no separate `RestoreFootprintOp` required in most cases).
- The ForgePass backend is responsible for calling `extend_ttl()` on credential entries
  **before** they approach archival. The recommended strategy is to renew all credentials
  for a passport in a single transaction every 3 months.
- Each `extend_ttl` call incurs a transaction fee (~10,100 stroops per transaction,
  i.e. the fee covers all entries in that passport in one call).

### Source reference

| Resource | URL | Date checked |
|---|---|---|
| `fees.rs` (canonical formula) | https://github.com/stellar/rs-soroban-env/blob/main/soroban-env-host/src/fees.rs | 2026-05-21 |
| State archival / TTL | https://developers.stellar.org/docs/learn/fundamentals/contract-development/storage/state-archival | 2026-05-21 |
| Fees & metering | https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering | 2026-05-21 |
| Protocol 23 announcement | https://stellar.org/blog/developers/announcing-protocol-23 | 2026-05-21 |
| **Live network config** | **https://lab.stellar.org/network-limits** | **Not yet queried — required before finalising numbers** |

---

## 2. Credential Storage Cost Model

Full cost model committed at `contracts/docs/cost-model.md`. Summary below.

### Credential entry size

| Field | Bytes |
|---|---|
| `wallet_address` (Stellar public key) | 56 |
| `signal_type` (u32 enum) | 4 |
| `source_id` (avg 32-char string) | 32 |
| `event_date` (u64 timestamp) | 8 |
| `data_hash` (SHA-256 hex string) | 64 |
| XDR encoding overhead + key prefix | 50 |
| **Conservative model value** | **250 bytes** |

### Cost summary at 100 credentials per passport

> XLM price: $0.146 USD (OKX, 2026-05-21)

| Scenario | `persistent_rent_rate_denominator` | Rent/yr per user | Grand total/yr at 10,000 users |
|---|---|---|---|
| A — mainnet-typical | 2,232,000 ledgers | $0.00099 | $163 |
| B — worst-case | 2,048 ledgers | $1.08 | $10,980 |

Both scenarios include rent, one-time write costs, and quarterly renewal transactions.

**Scenario A is the expected real-world value.** Scenario B is a safety margin used
to justify the 100-credential archival limit as a hard ceiling.

### One-time write cost per credential

- Fee per `add_credential` call: **10,100 stroops** = $0.000148 at current XLM price
- 100 credentials per passport: **$0.0148 total one-time write cost**
- 10,000 contributors × 100 credentials: **$147 total** (paid once, across all ingestion)

### Quarterly TTL renewal cost

- Cost per renewal transaction: **10,100 stroops** = $0.000148
- Annual renewal cost per passport (4 renewals): **$0.000590**
- Annual renewal cost at 10,000 contributors: **$5.90**

### Full cost model

See `contracts/docs/cost-model.md` for the complete breakdown including all credential
counts (10 / 50 / 100 / 500), both denominator scenarios, sensitivity analysis at up
to 100× the rent floor, and IPFS archival capacity estimates.

---

## 3. Storage Limit Decision

### Decision

**Limit: 100 credentials stored on-chain per passport.**

### Rationale

All four decision criteria from issue #003 were evaluated:

**Criterion 1 — Cost at the 100-credential limit:**
Under the mainnet-typical fee scenario (denominator ~2,232,000), 100 credentials per
passport costs approximately $0.001/year/user, with a grand total of ~$163/year at
10,000 contributors. Even in the worst-case scenario (denominator 2,048), the annual
cost is ~$1.08/user ($10,980 total at 10,000 users) — manageable for a funded project.
Allowing 500 credentials under the worst-case scenario would cost ~$54,878/year, which
is not sustainable. The 100-credential limit is the correct safety ceiling.

**Criterion 2 — Time for a realistic active contributor to hit the limit:**
A realistic active contributor adds approximately 30 credentials per year (24 merged
PRs + 4 Soroban deployments + 2 hackathons). At that rate, the 100-credential limit is
reached in approximately **3.3 years** — well beyond the v1 horizon. Archival is an
edge case, not a routine operation, for most contributors. Power users (10+ PRs/month)
may reach the limit in roughly 1 year; these are exactly the contributors whose full
history is most valuable, and the Merkle root archival strategy (Section 5) preserves
their full verifiable record.

**Criterion 3 — Is a rolling window too complex for v1?**
Yes. A rolling window contract (Option C) requires the credential store contract to
manage insertion order, trigger archival logic, and update a Merkle root on every
`add_credential` call that exceeds the limit. This places business logic inside the
smart contract, makes every add call have variable and unpredictable cost, doubles the
testing surface, and makes the contract harder to audit. The correct design is for the
backend to handle archival before calling `add_credential`. The contract stays simple
and correct; the backend takes responsibility for the archival workflow.

**Criterion 4 — On-chain vs backend enforcement:**
Backend enforcement is the correct choice. The ForgePass backend is already a trusted
actor — it is the only authorised wallet that can call write functions on the contracts
(per FRD FR-02.2, FR-02.3, and the NFR security section). Adding a count check to the
contract would add complexity without meaningful security uplift, given this existing
trust assumption. The contract exposes `get_credential_count()` so any third party can
independently verify that the limit is being respected. The limit value is published
in this document and enforced transparently in the open-source backend.

### Decision summary

| Parameter | Value |
|---|---|
| **Limit** | **100 credentials per passport** |
| **Cost at limit (Scenario A)** | $0.001/yr/user — $163/yr at 10,000 contributors |
| **Cost at limit (Scenario B worst-case)** | $1.08/yr/user — $10,980/yr at 10,000 contributors |
| **Enforcement** | Off-chain — backend checks `get_credential_count()` before each `add_credential` call |
| **Archival trigger** | When `get_credential_count()` reaches 100, backend archives oldest credentials before writing the new one |
| **Archival method** | Merkle root on-chain + full credential JSON on IPFS (see Section 5) |
| **Contract complexity** | Unchanged — no rolling window logic in contract |

### Options considered and rejected

| Option | Limit | Decision |
|---|---|---|
| Option A | 50 credentials | Rejected — active contributors hit this in ~1.7 years; archival too frequent |
| **Option B** | **100 credentials** | **Selected** |
| Option C | Rolling window (last 100) | Rejected — too complex for v1 contract; archival logic belongs in backend |

---

## 4. Archival Strategy — Overview

### When archival triggers

Archival is triggered by the backend, not the contract. Before every `add_credential`
call, the backend calls `get_credential_count(wallet)` on the credential store contract.
If the count has reached **100**, the backend must complete the archival workflow before
the new credential can be written.

The sequence is:

```
1. Backend checks get_credential_count(wallet) → count == 100
2. Backend fetches the oldest N credentials from PostgreSQL
   (those with the earliest event_date values)
3. Backend builds a Merkle tree from the credential set to be archived
4. Backend uploads the full credential JSON archive to IPFS → receives CID
5. Backend calls add_archive_record(wallet, merkle_root, credential_count, archived_at, ipfs_cid)
   on the credential store contract → writes ArchiveRecord on-chain
6. Backend deletes the archived credential entries from on-chain storage
   (calls remove_credential for each archived entry — admin-only function)
7. Backend writes the same archived credentials to the PostgreSQL
   archived_credentials table (recovery path)
8. Backend now has room to call add_credential for the new credential
```

### How many credentials are archived per trigger

The archival removes the **oldest 50 credentials** (by `event_date`) when the limit
is reached. This leaves 50 on-chain and creates headroom for 50 more before the next
archival cycle. Archiving exactly at the limit (removing all 100 and starting fresh)
is rejected because it loses the continuity of recent on-chain history — integrators
querying `get_credentials()` would see an empty list immediately after archival.

**Archival batch size: 50 oldest credentials per cycle.**

### What remains on-chain after archival

After an archival cycle completes, the passport has:
- **50 most recent credentials** — still in `ContractData` entries, directly readable
  via `get_credentials()`
- **One or more `ArchiveRecord` entries** — each committing a Merkle root + IPFS CID
  for a past batch of archived credentials

Both types are queryable. The API response for a contributor's credential history
stitches together on-chain live credentials and archived credential data (fetched from
IPFS or PostgreSQL) into a single unified timeline.

### Backend contract function requirements (for issue #014 and #019)

The archival workflow requires the following functions on the credential store contract,
beyond the core `add_credential` / `get_credentials` pair:

| Function | Caller | Purpose |
|---|---|---|
| `get_credential_count(wallet: Address) -> u32` | Backend + third parties | Check count before write; verify compliance |
| `add_archive_record(wallet, merkle_root, count, archived_at, ipfs_cid)` | Backend (admin-only) | Anchor the Merkle root on-chain after archival |
| `get_archive_records(wallet: Address) -> Vec<ArchiveRecord>` | Backend + third parties | Retrieve all archive entries for a passport |
| `remove_credentials(wallet: Address, source_ids: Vec<String>)` | Backend (admin-only) | Delete on-chain entries after they are safely archived |

> **Note for #019:** `remove_credentials` must be admin-only and must only be callable
> after a valid `ArchiveRecord` exists for that wallet. The contract should verify this
> precondition to prevent accidental credential deletion without a corresponding on-chain
> proof.

---

## 5. Merkle Root Design

### Purpose

The Merkle root is the cryptographic commitment that makes archived credentials
verifiable without trusting ForgePass. Once the Merkle root of an archived batch is
anchored on-chain, any third party can verify that a specific credential genuinely
existed in that batch — without ForgePass's involvement — by fetching the IPFS archive
and reconstructing the Merkle path.

### Leaf format (canonical and deterministic)

Each Merkle leaf is the SHA-256 hash of a credential's canonical JSON representation.
The JSON must be **deterministic**: sorted keys, no extra whitespace, no trailing
newlines. Any deviation breaks verification.

**Canonical leaf JSON (exact field order, exact types):**

```json
{"data_hash":"<sha256-hex>","event_date":<unix-timestamp-integer>,"signal_type":"<ENUM_VALUE>","source_id":"<string>","wallet":"<G...stellar-address>"}
```

Field order: alphabetical by key name. This is unambiguous and reproducible in any
language using standard JSON serialisation with sorted keys.

**Example leaf:**

```json
{"data_hash":"a3f1...","event_date":1716249600,"signal_type":"GITHUB_PR","source_id":"stellar/stellar-core#1234","wallet":"GABC...XYZ"}
```

**TypeScript serialisation (canonical):**

```typescript
function canonicalLeafJson(credential: ArchivedCredential): string {
  return JSON.stringify({
    data_hash: credential.data_hash,
    event_date: credential.event_date,       // integer Unix timestamp
    signal_type: credential.signal_type,
    source_id: credential.source_id,
    wallet: credential.wallet_address,
  });
  // JSON.stringify with a plain object literal always produces sorted keys
  // in the order they are written — define them alphabetically.
}

function leafHash(credential: ArchivedCredential): Buffer {
  return crypto.createHash('sha256')
    .update(canonicalLeafJson(credential), 'utf8')
    .digest();
}
```

> **Important:** `event_date` is serialised as an **integer** (Unix timestamp seconds),
> never as an ISO-8601 string. The on-chain `Credential` struct stores it as `u64`.
> Using a string here would produce a different hash.

### Tree construction algorithm

| Parameter | Value | Rationale |
|---|---|---|
| Hash function | SHA-256 | Standard; matches `data_hash` field and on-chain storage |
| Leaf ordering | Sort by `event_date` ascending before tree construction | Deterministic ordering means any party with the IPFS archive can reconstruct the same tree |
| Odd leaf handling | Duplicate the last leaf | Standard Merkle tree behaviour; avoids padding with zeros which changes the root |
| Tree type | Standard binary Merkle tree (not a sparse Merkle tree) | Simpler implementation; archive sets are bounded (≤50 leaves) |
| TypeScript library | `merkletreejs` (npm) with SHA-256 as the hash function | Well-maintained, zero additional native dependencies |

**TypeScript tree construction:**

```typescript
import { MerkleTree } from 'merkletreejs';
import * as crypto from 'crypto';

function buildArchiveMerkleTree(credentials: ArchivedCredential[]): MerkleTree {
  // Sort by event_date ascending — deterministic ordering
  const sorted = [...credentials].sort((a, b) => a.event_date - b.event_date);

  // Hash each leaf using the canonical leaf format
  const leaves = sorted.map(leafHash);

  // Build tree — merkletreejs duplicates odd leaves by default
  return new MerkleTree(leaves, (data: Buffer) =>
    crypto.createHash('sha256').update(data).digest()
  );
}

function getMerkleRoot(tree: MerkleTree): Buffer {
  return tree.getRoot();
}
```

### On-chain storage format (`ArchiveRecord`)

Each archived batch is stored as a single `ContractData` entry on the credential store
contract. The Rust struct to be defined in issue #019:

```rust
#[contracttype]
pub struct ArchiveRecord {
    pub merkle_root:       [u8; 32],  // SHA-256 Merkle root of the archived batch
    pub credential_count:  u32,       // number of credentials in this archive
    pub archived_at:       u64,       // ledger timestamp when archival was recorded
    pub ipfs_cid:          String,    // CID of the full archive JSON on IPFS
}
```

**Storage key format:** `(wallet_address, archive_index)` where `archive_index` is a
monotonically increasing `u32` starting at 0. Each passport can accumulate multiple
`ArchiveRecord` entries as it cycles through archival rounds.

**CID storage decision: stored on-chain (in `ArchiveRecord.ipfs_cid`).** Rationale:
storing the CID on-chain means any third party can locate the IPFS archive without
querying ForgePass's database. If the CID were stored only in PostgreSQL, ForgePass
becomes a trusted intermediary for the verification path — defeating the purpose of
decentralised proof. The additional storage cost of a CID string (~60 bytes per
`ArchiveRecord`) is negligible given archival is triggered at most once per year for
most contributors.

### Proof verification method (third-party, trustless)

A third party who wants to verify that a specific credential existed in an archived
batch follows this procedure without trusting ForgePass:

**Step 1 — Fetch the on-chain `ArchiveRecord`.**
Call `get_archive_records(wallet)` on the credential store contract. Identify the
`ArchiveRecord` whose `archived_at` timestamp covers the period when the credential
was active.

**Step 2 — Fetch the IPFS archive.**
Fetch the JSON archive using the `ipfs_cid` from the `ArchiveRecord`. Any IPFS gateway
works (e.g. `https://ipfs.io/ipfs/<cid>`, Pinata's gateway, or a local node). The
archive contains the full list of credentials and the `merkle_root` field for
cross-checking.

**Step 3 — Locate the credential and compute its leaf hash.**
Find the credential in the archive's `credentials` array. Reconstruct its canonical
leaf JSON using the same field order and types defined in the Leaf Format section above.
Compute `SHA-256(canonical_leaf_json)`.

**Step 4 — Reconstruct the Merkle path.**
Using the `credentials` array from the IPFS archive (sorted by `event_date` ascending),
rebuild the Merkle tree locally and generate the Merkle proof path for the target
credential's leaf.

**Step 5 — Verify the proof against the on-chain root.**
Verify that the Merkle path from Step 4, applied to the leaf hash from Step 3,
produces the `merkle_root` stored in the on-chain `ArchiveRecord`. If they match,
the credential is genuine and was present at the time of archival.

**Reference TypeScript verification:**

```typescript
import { MerkleTree } from 'merkletreejs';
import * as crypto from 'crypto';

function verifyArchivedCredential(
  targetCredential: ArchivedCredential,
  archiveJson: ArchiveJson,           // parsed IPFS archive file
  onChainMerkleRoot: Buffer,          // merkle_root from ArchiveRecord
): boolean {
  const sorted = [...archiveJson.credentials]
    .sort((a, b) => a.event_date - b.event_date);

  const leaves = sorted.map(leafHash);
  const tree = new MerkleTree(leaves, (data: Buffer) =>
    crypto.createHash('sha256').update(data).digest()
  );

  const targetLeaf = leafHash(targetCredential);
  const proof = tree.getProof(targetLeaf);

  return tree.verify(proof, targetLeaf, onChainMerkleRoot);
}
```

### Security properties

- **Tamper-evidence:** any modification to an archived credential changes its leaf hash,
  which changes the Merkle root, which no longer matches the on-chain anchor. Forgery
  is computationally infeasible.
- **Non-repudiation:** ForgePass cannot retroactively deny a credential existed once it
  is included in an on-chain `ArchiveRecord`. The Merkle root is immutable.
- **Independence:** third-party verification requires only: the credential store contract
  address, an IPFS gateway, and the `merkletreejs` library. No ForgePass API call is
  needed.
- **Privacy:** archived credential data is public on IPFS. Contributors who set signals
  to private (FRD FR-02.6) should be aware that archived credentials are not
  retroactively removed from IPFS. The privacy setting controls what the ForgePass API
  returns — it does not delete on-chain or IPFS data. This is consistent with the
  soulbound, non-revocable design (FRD FR-02.3).

---

## 6. IPFS Archival Flow

### Purpose

The Merkle root anchored on-chain (Section 5) is a cryptographic commitment, but it
is not the data itself. For a third party to actually verify an archived credential,
they need the full credential JSON that was committed to. That full dataset lives on
IPFS. This section defines exactly what is uploaded, in what format, by which provider,
and what happens if any part of the upload or on-chain write fails.

### Archive JSON schema

Every archived batch is serialised as a single JSON file before being uploaded to IPFS.
The schema is versioned (`"version": "1.0"`) so future changes can be detected. The
`merkle_root` field is included in the archive itself so a verifier can cross-check it
against the on-chain `ArchiveRecord` without an extra lookup.

**Complete schema:**

```json
{
  "version": "1.0",
  "wallet": "G...",
  "archived_at": "2026-05-21T14:30:00Z",
  "merkle_root": "a3f1c2...64-char-hex-string",
  "credential_count": 50,
  "credentials": [
    {
      "signal_type": "GITHUB_PR",
      "source_id": "stellar/stellar-core#1234",
      "event_date": 1716249600,
      "data_hash": "sha256-hex-string",
      "on_chain_tx": "stellar-transaction-hash",
      "ingested_at": "2025-11-03T08:12:00Z"
    }
  ]
}
```

**Field definitions:**

| Field | Type | Notes |
|---|---|---|
| `version` | string | Schema version. `"1.0"` for all v1 archives. Increment if schema changes. |
| `wallet` | string | Stellar public key (G...) of the passport owner. |
| `archived_at` | string | ISO-8601 UTC timestamp of when the backend triggered this archival cycle. |
| `merkle_root` | string | SHA-256 Merkle root as a lowercase hex string (64 chars). Must match `ArchiveRecord.merkle_root` on-chain. |
| `credential_count` | integer | Number of credentials in this archive. Must match `ArchiveRecord.credential_count` on-chain. |
| `credentials` | array | The archived credentials, sorted by `event_date` ascending — same ordering used to build the Merkle tree. |
| `credentials[].signal_type` | string | Enum value: `GITHUB_PR`, `SOROBAN_CONTRACT`, `STELLAR_DEX`, `HACKATHON`. |
| `credentials[].source_id` | string | External reference ID (e.g. GitHub PR URL, contract address). |
| `credentials[].event_date` | integer | Unix timestamp (seconds) of when the contribution occurred. Integer, not ISO string. |
| `credentials[].data_hash` | string | SHA-256 hex hash of the raw signal data — the same value stored on-chain. |
| `credentials[].on_chain_tx` | string | Stellar transaction ID of the original on-chain credential write. Allows independent verification via Horizon. |
| `credentials[].ingested_at` | string | ISO-8601 UTC timestamp of when ForgePass first recorded this credential. |

**Ordering requirement:** the `credentials` array must be sorted by `event_date`
ascending. This is the same sort order used to build the Merkle tree (Section 5), so
a verifier reconstructing the tree from this array will produce the same root without
any reordering step.

**Merkle root encoding:** the `merkle_root` field is a lowercase hex string, not Base64
or a byte array. The on-chain `ArchiveRecord.merkle_root` is `[u8; 32]` — convert to
hex for the JSON field and back to bytes for on-chain comparison.

### Pinning provider

**v1 provider: Pinata (free tier).**

| Provider | Decision | Rationale |
|---|---|---|
| **Pinata** | **Selected for v1** | Free tier: 1 GB storage, unlimited pins, HTTPS gateway at `gateway.pinata.cloud`. No credit card required for free tier. Well-documented API (`/pinning/pinJSONToIPFS`). Widely used in Stellar ecosystem projects. |
| web3.storage | Rejected | Discontinued. Do not use. |
| Arweave | Deferred to mainnet consideration | Permanent storage, pay-once model. Worth evaluating before mainnet if IPFS pin availability becomes a concern. No ongoing renewal cost, but higher per-byte cost than Pinata free tier. |
| Self-hosted IPFS node | Rejected for v1 | Full control but requires operational overhead ForgePass cannot commit to in v1. |

**Pinata capacity check (from cost model):**
Pinata's free tier supports 1 GB. Each archive file is approximately 500 bytes of JSON
per credential × 50 credentials = ~25 KB per archive. 1 GB supports approximately
**40,960 archive files** — sufficient for 10,000 contributors cycling through multiple
archival rounds before a paid tier or Arweave migration is needed.

**Upgrade path:** if Pinata free tier is exhausted or discontinued, the PostgreSQL
`archived_credentials` table (see below) serves as the recovery path. All archive JSON
is stored there in parallel. Migration to a new IPFS provider or Arweave can be done
by re-pinning from the PostgreSQL backup without data loss.

**Pinata API call (TypeScript):**

```typescript
async function pinArchiveToIPFS(archiveJson: ArchiveJson): Promise<string> {
  const response = await fetch('https://api.pinata.cloud/pinning/pinJSONToIPFS', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${process.env.PINATA_JWT}`,
    },
    body: JSON.stringify({
      pinataContent: archiveJson,
      pinataMetadata: {
        name: `forgepass-archive-${archiveJson.wallet}-${Date.now()}`,
      },
    }),
  });

  if (!response.ok) {
    throw new StorageError('IPFS_PIN_FAILED', response.status, await response.text());
  }

  const result = await response.json();
  return result.IpfsHash; // CID string
}
```

### Failure handling

Three failure scenarios must be handled explicitly. The guiding principle is: **never
lose credential data, and never anchor an on-chain proof without confirmed IPFS
availability.**

---

**Scenario 1 — IPFS upload fails before on-chain write**

```
Backend → Pinata upload → FAILS
```

Action: abort the archival cycle entirely. No on-chain write occurs. The credentials
being archived are still present on-chain as `ContractData` entries — nothing is lost.
The backend logs the failure, marks the archival job as `FAILED` in the
`archival_jobs` table, and schedules a retry on the next indexer cycle.

The new credential that triggered the archival check is held in the queue and retried
after the archival succeeds. It is not written while the credential count is at the
limit.

Retry policy: exponential backoff, up to 3 attempts per indexer cycle. Alert raised
if 3 consecutive cycles fail to complete archival for the same passport.

---

**Scenario 2 — IPFS upload succeeds but on-chain write fails**

```
Backend → Pinata upload → SUCCESS (CID received)
Backend → add_archive_record() → FAILS
```

Action: do not re-upload to IPFS. The CID is already valid and the archive already
exists on IPFS. Persist the CID in the `archival_jobs` table with status `PENDING_ONCHAIN`.
Retry only the on-chain `add_archive_record()` call using the same CID on the next
cycle.

Alert raised after 3 consecutive on-chain write failures. If the on-chain write
ultimately fails after all retries, the state is: archive exists on IPFS (findable by
CID), credentials still on-chain (count has not been reduced). No data is lost. The
`archival_jobs` table records the CID so manual recovery is possible.

Do not proceed to `remove_credentials()` until `add_archive_record()` has confirmed
a successful on-chain transaction.

---

**Scenario 3 — IPFS pin becomes unavailable after successful archival**

```
Archival completed successfully → later → IPFS CID unreachable
```

Action: the on-chain `ArchiveRecord` still exists with the Merkle root. The root
proves the credentials existed. The full data is recoverable from the PostgreSQL
`archived_credentials` table (see below). ForgePass re-pins the archive from PostgreSQL
to IPFS and updates nothing on-chain (the CID stored on-chain remains valid once the
archive is re-pinned — IPFS CIDs are content-addressed, so re-pinning the same JSON
produces the same CID).

Alert policy: the backend should periodically check IPFS CID reachability for all
`ArchiveRecord` entries (weekly is sufficient). Alert raised if any CID becomes
unreachable. The alert runbook entry must include the re-pinning procedure.

---

### PostgreSQL backup requirement

**All archived credential JSON must be written to PostgreSQL at the same time as the
IPFS upload.** IPFS is the primary store and the source of truth for public
verifiability. PostgreSQL is the recovery path.

This requires an `archived_credentials` table in the database schema (issue #030,
migration #011). The table stores:

| Column | Type | Notes |
|---|---|---|
| `id` | UUID | Primary key |
| `wallet_address` | String | FK → `passports.wallet_address` |
| `archive_index` | Integer | Matches the on-chain `(wallet, archive_index)` storage key |
| `merkle_root` | String | Hex-encoded Merkle root — must match on-chain `ArchiveRecord` |
| `credential_count` | Integer | Number of credentials in this archive |
| `ipfs_cid` | String | Pinata CID — used for re-pinning if unavailable |
| `archive_json` | JSONB | Full archive JSON matching the schema above |
| `archived_at` | Timestamp | UTC timestamp of archival |
| `on_chain_tx` | String | Stellar transaction ID of the `add_archive_record` call |

**Write order:** write to PostgreSQL first, then upload to IPFS, then write on-chain.
If IPFS or on-chain fails, the PostgreSQL record exists as a recovery anchor. If the
PostgreSQL write fails, abort the archival cycle — do not proceed without the backup
in place.

> **Note for issue #030:** the `archived_credentials` table must be included in
> migration #011. The `archive_json` column should be `JSONB` (not `TEXT`) to allow
> efficient querying of specific credentials within an archive without full JSON parsing.
> Index on `wallet_address` for fast lookup by contributor.

---

## 7. Referenced By

- **Issue #003** — this decision (all sections complete)
- **Issue #011** — passport contract scaffold: reads Section 3 (storage limit and enforcement model)
- **Issue #014** — contract interface design: reads Sections 3 and 4 (archival trigger, `get_credential_count`, `add_archive_record`, `remove_credentials` requirements)
- **Issue #019** — credential store contract: reads Sections 3, 4, and 5 (limit, archival workflow, `ArchiveRecord` struct, Merkle design)
- **Issue #030** — database migrations: reads Section 6 (`archived_credentials` table schema, column definitions, JSONB index requirement)


## Section 8 -- Credential Store Contract Implementation Notes (#019)

### 8.1 Archival is backend-controlled, not contract-triggered

The original #019 execution roadmap specified an internal archive_oldest_credentials
private function that would fire automatically inside add_credential when the live
count reached the ceiling. This approach was superseded by INTERFACES.md (issue #014,
closed before #019 began).

The implemented design is: add_credential does not check the credential count or
trigger archival. The backend calls get_credential_count before every add_credential
call. If the count is 100, the backend runs the full archival workflow
(add_archive_record then remove_credentials) before proceeding with the new write.
The contract responsibility is storage and deduplication only -- not orchestration.

This keeps the contract simple, auditable, and free of variable-cost operations.
Every add_credential call has predictable gas cost regardless of history depth.

### 8.2 ArchiveRecordRequired (302) safety guard

remove_credentials verifies that at least one ArchiveRecord exists for the wallet
before deleting any live credential entries. If DataKey::ArchiveIndex(wallet) is 0
(no archival has occurred), the function returns ContractError::ArchiveRecordRequired
(302) and leaves the live Vec unchanged.

This prevents a backend bug or misconfigured call sequence from silently deleting
on-chain credentials without a corresponding Merkle root proof. The backend must call
add_archive_record first and confirm the on-chain transaction before calling
remove_credentials.

remove_credentials silently skips source_ids not found in the live set, making
retries after partial failure safe.

### 8.3 DataKey design

| Key | Storage tier | Purpose |
|---|---|---|
| Admin | Instance | Admin address set once at initialize. Never expires with the contract. |
| Credentials(Address) | Persistent | Vec<CredentialRecord> -- live set per wallet. At most 100 entries. |
| CredentialCounter(Address) | Instance | Per-wallet monotonic u64 ID generator. Starts at 1, never decremented. |
| ArchiveRecord(Address, u32) | Persistent | One entry per archival cycle per wallet. Accumulates across cycles. |
| ArchiveIndex(Address) | Instance | Next archive_index slot for a wallet. Incremented by add_archive_record. |

The credential ID counter is per-wallet (not global), consistent with INTERFACES.md
Section 3.2. IDs are never reused after archival -- the counter is stored in instance
storage and only ever incremented.

### 8.4 SignalType extension pathway

The credential store contract never branches on SignalType in any function body. It
stores whatever variant is passed to add_credential and returns it from
get_credentials. Appending ScfGrant, GrantfoxBounty, or TrustlessWork to the shared
enum via WASM upgrade requires zero changes to credential store contract logic.

The deduplication check in add_credential and credential_exists compares
(signal_type, source_id) pairs using PartialEq on the SignalType enum. New variants
are automatically supported once the WASM is upgraded.

See INTERFACES.md Section 11 for the full WASM upgrade procedure.

### 8.5 Windows development note

On Windows with x86_64-pc-windows-msvc, cdylib builds work correctly via MSVC.
Tests are run with: cargo test -p credential-store --target x86_64-pc-windows-msvc

The .cargo/config.toml sets wasm32-unknown-unknown as the global build target so
bare cargo test without an explicit target will fail. Always specify the native
target explicitly.
