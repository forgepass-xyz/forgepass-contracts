# ForgePass — Soroban Credential Storage Cost Model

**Issue:** #003 — On-chain Credential Storage Limits & Soroban Cost Model  
**Phase:** 0  
**Date:** 2026-05-21  
**XLM price used:** $0.146 USD (OKX live data, 2026-05-21)  
**Status:** Draft — live `fee_per_rent_1kb` and `persistent_rent_rate_denominator` values
must be verified via `lab.stellar.org/network-limits` before finalising.

---

## 1. Fee Formula (from primary source)

All rent calculations are derived from the canonical fee formula in
[`fees.rs`](https://github.com/stellar/rs-soroban-env/blob/main/soroban-env-host/src/fees.rs):

```
rent_fee_stroops =
  (size_bytes × fee_per_rent_1kb × rent_ledgers)
  ─────────────────────────────────────────────
         (1024 × persistent_rent_rate_denominator)
```

Where:
- `size_bytes` — byte size of the ledger entry
- `fee_per_rent_1kb` — dynamic, scales with total Soroban state size. Floor: **1,000 stroops/KB** (hardcoded in `fees.rs`)
- `rent_ledgers` — number of ledgers in the TTL extension window
- `persistent_rent_rate_denominator` — network config parameter. Mainnet-typical value: **~2,232,000 ledgers** (~129 days). Must be verified live.

**Protocol 23 note (live on mainnet since September 2025):** Read fees for live
Soroban state were eliminated (CAP-0062/0066 — all live state moved to validator RAM).
Only write fees and rent remain as ongoing costs for credential storage.

---

## 2. Assumptions

| Parameter | Value | Source |
|---|---|---|
| Bytes per credential entry | **250 bytes** (conservative) | Roadmap #003 guidance, FRD data model |
| Rent fee per 1KB (floor) | **1,000 stroops** | `fees.rs` `MINIMUM_RENT_WRITE_FEE_PER_1KB` |
| Persistent rent rate denominator | **2,232,000 ledgers** (Scenario A) / **2,048** (Scenario B worst-case) | soroban-settings docs; must verify live |
| Seconds per ledger | **5s** | Stellar network |
| Ledgers per day | **17,280** | 86,400 / 5 |
| Renewal window | **3 months = 1,555,200 ledgers** | Per roadmap #003 |
| Ledgers per year | **6,307,200** | |
| One-time write cost per credential | **10,100 stroops** | `fee_per_write_entry` (10,000) + inclusion fee (100) |
| XLM price | **$0.146 USD** | OKX, 2026-05-21 |
| Scale target | **10,000 contributors** | ForgePass v1 planning |

**Credential byte size breakdown:**

| Field | Bytes |
|---|---|
| `wallet_address` (Stellar public key) | 56 |
| `signal_type` (u32 enum) | 4 |
| `source_id` (avg 32-char string) | 32 |
| `event_date` (u64 timestamp) | 8 |
| `data_hash` (SHA-256 hex string) | 64 |
| XDR encoding overhead + key prefix | 50 |
| **Estimated total** | **214 bytes** |
| **Conservative model value** | **250 bytes** |

---

## 3. Per-Passport Rent Cost

### Scenario A — Mainnet-typical denominator (2,232,000 ledgers)

> 1 KB costs 1,000 stroops every 2,232,000 ledgers ≈ 129 days

| Credentials | Bytes | Rent/3mo (XLM) | Rent/yr (XLM) | Rent/yr ($) | Verdict |
|---:|---:|---:|---:|---:|---|
| 10 | 2,500 | 0.0001701 | 0.0006804 | $0.0000993 | Trivially cheap |
| 50 | 12,500 | 0.0008506 | 0.0034022 | $0.0004967 | Negligible |
| **100** | **25,000** | **0.0017011** | **0.0068044** | **$0.0009934** | **Negligible** |
| 500 | 125,000 | 0.0085055 | 0.0340222 | $0.0049672 | Low |

### Scenario B — Worst-case denominator (2,048 ledgers)

> Conservative floor used for contract design safety margin

| Credentials | Bytes | Rent/3mo (XLM) | Rent/yr (XLM) | Rent/yr ($) | Verdict |
|---:|---:|---:|---:|---:|---|
| 10 | 2,500 | 0.1853943 | 0.7415771 | $0.1082703 | Expensive |
| 50 | 12,500 | 0.9269714 | 3.7078857 | $0.5413513 | Expensive |
| **100** | **25,000** | **1.8539429** | **7.4157715** | **$1.0827026** | **Expensive** |
| 500 | 125,000 | 9.2697144 | 37.0788574 | $5.4135132 | Expensive |

---

## 4. At-Scale: 10,000 Contributors (Annual Cost)

### Scenario A — Mainnet-typical denominator

| Credentials | Rent/yr total ($) | Write cost total ($) | Renewal cost total ($) | Grand Total ($) | Sustainable? |
|---:|---:|---:|---:|---:|---|
| 10 | $0.99 | $14.75 | $5.90 | $21.64 | ✓ Sustainable |
| 50 | $4.97 | $73.73 | $5.90 | $84.60 | ✓ Sustainable |
| **100** | **$9.93** | **$147.46** | **$5.90** | **$163.29** | **✓ Sustainable** |
| 500 | $49.67 | $737.30 | $5.90 | $792.87 | ~ Borderline |

### Scenario B — Worst-case denominator

| Credentials | Rent/yr total ($) | Write cost total ($) | Renewal cost total ($) | Grand Total ($) | Sustainable? |
|---:|---:|---:|---:|---:|---|
| 10 | $1,082.70 | $14.75 | $5.90 | $1,103.35 | ~ Borderline |
| 50 | $5,413.51 | $73.73 | $5.90 | $5,493.14 | ✗ High |
| **100** | **$10,827.03** | **$147.46** | **$5.90** | **$10,980.38** | **✗ High** |
| 500 | $54,135.13 | $737.30 | $5.90 | $54,878.33 | ✗ High |

**Note on Scenario B:** Even in the worst-case scenario at 100 credentials/passport,
the annual cost of ~$10,980 for 10,000 users (~$1.08/user/year) is manageable for a
funded project but would warrant immediate archival enforcement and backend rent
management. This scenario uses a denominator that is likely far too conservative for
the real mainnet value.

---

## 5. One-Time Write Cost Model

| Cost type | Stroops | XLM | USD |
|---|---:|---:|---:|
| Per `add_credential` call | 10,100 | 0.001010 | $0.0001475 |
| 100 credentials (full passport) | 1,010,000 | 0.10100 | $0.01475 |
| 10,000 users × 100 credentials | 10,100,000,000 | 1,010.00 | $147.46 |

Write costs are **one-time only** — paid at ingestion, not recurring.

---

## 6. TTL Renewal Cost Model

- **Renewal frequency:** every 3 months (4×/year)
- **Cost per renewal transaction:** 10,100 stroops = $0.0001475 per passport
- **Annual renewal cost per passport:** $0.000590
- **Annual renewal cost at 10,000 contributors:** $5.90

Renewal costs are negligible at any scale. The backend must call `extend_ttl()` on
each credential's ledger entry before it approaches `liveUntilLedger`. The minimum
persistent TTL on creation is **4,095 ledgers (~23.8 days)**, so the backend must
begin renewal monitoring from day one of passport creation.

---

## 7. Sensitivity Analysis

> Denominator = 2,232,000 (Scenario A) | 100 credentials | 10,000 contributors

| fee_per_rent_1kb | Rent/yr per user ($) | Total rent/yr ($) | Verdict |
|---:|---:|---:|---|
| 1,000 stroops/KB (1× floor) | $0.00099 | $9.93 | ✓ Sustainable |
| 5,000 stroops/KB (5× floor) | $0.00497 | $49.67 | ✓ Sustainable |
| 10,000 stroops/KB (10× floor) | $0.00993 | $99.34 | ✓ Sustainable |
| 50,000 stroops/KB (50× floor) | $0.04967 | $496.72 | ✓ Sustainable |
| 100,000 stroops/KB (100× floor) | $0.09934 | $993.45 | ~ Borderline |

Even at 100× the minimum floor, costs remain manageable. The 100-credential archival
limit is the correct choice for contract design regardless of which fee scenario
materialises.

---

## 8. IPFS Archival Capacity

| Parameter | Value |
|---|---|
| Pinata free tier | 1 GB |
| Estimated archive JSON size | ~500 bytes per credential |
| Archive set size (100 credentials) | ~50,000 bytes (50 KB) |
| Archive sets in free tier | ~21,475 |
| Equivalent contributors (1 archive each) | ~21,475 |

Pinata free tier is more than adequate for v1 (10,000 contributors). A PostgreSQL
backup of all archived credential JSON must also be maintained as a recovery path
(see `ARCHITECTURE.md` Section 6).

---

## 9. Key Findings

1. **Storage is extremely cheap.** Under Scenario A (mainnet-typical), 100 credentials
   per passport costs under $0.001/year/user. Even at 100× the rent floor, costs remain
   under $1/user/year.

2. **The 100-credential limit is correct.** It is the right balance of cost control and
   usability regardless of which fee scenario is real. A realistic active contributor
   (~30 credentials/year) takes over 3 years to reach the limit.

3. **Protocol 23 helps ForgePass.** Eliminating read fees for live state means
   third-party reads of passport data are essentially free. Only writes and rent matter.

4. **The denominator must be verified live.** Query `lab.stellar.org/network-limits`
   (JSON view, mainnet) for `persistentRentRateDenominator` and `rentFeePerByte1KB`
   before committing the final numbers to `ARCHITECTURE.md`.

5. **Archival is a safety valve, not a cost-driven necessity.** At Scenario A costs,
   we could store 500 credentials on-chain sustainably. We apply the 100-credential
   limit for contract simplicity and to protect against the Scenario B worst case.

---

## 10. Action Required Before Finalising

- [ ] Query `lab.stellar.org/network-limits` → JSON → mainnet → extract
      `persistentRentRateDenominator` and `writeFeePerByte` (or `feeWriteLedgerEntry`)
- [ ] Replace placeholder values in `ARCHITECTURE.md` Section 2 with confirmed live values
- [ ] Re-run the rent formula with confirmed values to verify verdicts hold
- [ ] Record the date of the query in both documents (network settings can change by validator vote)

---

## Sources

| Source | URL | Date checked |
|---|---|---|
| fees.rs (canonical formula) | https://github.com/stellar/rs-soroban-env/blob/main/soroban-env-host/src/fees.rs | 2026-05-21 |
| State archival / TTL docs | https://developers.stellar.org/docs/learn/fundamentals/contract-development/storage/state-archival | 2026-05-21 |
| Fees & metering docs | https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering | 2026-05-21 |
| Protocol 23 announcement | https://stellar.org/blog/developers/announcing-protocol-23 | 2026-05-21 |
| XLM price | OKX live data | 2026-05-21 ($0.146) |
| Live network config | https://lab.stellar.org/network-limits | **Must query — not yet verified** |
