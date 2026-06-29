// =============================================================================
// ForgePass -- Trust Score Anchor Contract
// crate: trust-score  |  contracts/trust-score
// =============================================================================
//
// ABI authority:  contracts/INTERFACES.md Section 6  (Issue #014, complete)
// Implementation: Issue #017
// FRD coverage:   FR-02.8, FR-04.2, FR-04.4
//
// STORAGE SCHEMA
// --------------
// DataKey::Admin               -> Address              [instance]   Set once at initialize.
// DataKey::CurrentScore(w)     -> ScoreSnapshot        [instance]   Latest snapshot per wallet.
// DataKey::ScoreHistory(w)     -> Vec<ScoreSnapshot>   [persistent] Up to 50 entries, ascending.
// DataKey::SnapshotCount(w)    -> u32                  [instance]   Current history length.
//
// Instance storage for Admin and CurrentScore is appropriate: config-level
// data and hot-path reads that never expire independently of the contract.
// ScoreHistory uses persistent storage because auditable score history must
// survive ledger entry TTL expiry (FR-02.8). TTL extension is a backend
// concern handled by OnchainWriterService (issue #027).
//
// FUNCTION SUMMARY
// ----------------
//   initialize(env, admin)                                          [admin-bootstrap]
//   anchor_score(env, wallet, score, algo_ver, signal_hash, ts)    [admin-only]
//   get_current_score(env, wallet)  -> Option<ScoreSnapshot>       [public]
//   get_score_history(env, wallet)  -> Vec<ScoreSnapshot>          [public]
//
// ACCESS CONTROL (INTERFACES.md Section 8)
// ----------------------------------------
//   Admin-only  : anchor_score
//     -> require_admin(env)? as first statement before any state read/write.
//   Public      : get_current_score, get_score_history
//     -> no auth check.
//
// Auth failures on admin-only functions are Soroban host traps
// (INVOKE_HOST_FUNCTION_TRAPPED), not ContractError returns. See INTERFACES.md
// Section 8 auth failure model for how OnchainWriterService (#027) must handle
// both error shapes in transaction results.
//
// HISTORY CAP MANAGEMENT
// ----------------------
// Snapshots are stored ascending by computed_at: oldest at index 0, newest
// at the highest index. When the 50-entry cap is reached, pop_front() removes
// the oldest entry before push_back() appends the new snapshot. No secondary
// archive key is written -- the trust-score contract stores only the live Vec.
// Score history auditing beyond 50 snapshots relies on PostgreSQL and the
// API layer (issue #036). See INTERFACES.md Section 6 design notes.
//
// Step 1 discrepancy corrections applied (Issue #017 roadmap Step 1 analysis):
//   1. ScoreSnapshot has four fields only -- no tx_id.
//   2. History is ascending (oldest first) -- append strategy, not prepend.
//   3. No archive DataKey -- dropped snapshots are simply removed.
//   4. Storage keys use DataKey enum, not string-prefix tuples.
//   5. DataKey::SnapshotCount(Address) added per INTERFACES.md Section 6.
// =============================================================================

#![cfg_attr(not(test), no_std)]

use forgepass_shared::{ContractError, ScoreSnapshot};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Vec};

// -----------------------------------------------------------------------------
// Storage key enum -- trust-score contract scope only.
// Each contract crate defines its own DataKey. The shared/ crate does NOT
// define DataKey, preventing cross-contract storage key collisions.
// INTERFACES.md Section 6 DataKey definition.
// -----------------------------------------------------------------------------

/// Storage keys for the trust-score contract.
#[contracttype]
pub enum DataKey {
    /// Admin address. Instance storage. Set once at `initialize`.
    /// Never rotated without a full contract redeployment (INTERFACES.md Sec 2).
    Admin,
    /// Latest `ScoreSnapshot` for the wallet. Instance storage.
    /// Updated on every `anchor_score` call so third-party score reads avoid
    /// loading the full persistent history Vec (O(1) access).
    CurrentScore(Address),
    /// Full score history for the wallet. Persistent storage.
    /// `Vec<ScoreSnapshot>` ordered ascending by `computed_at`: oldest at index
    /// 0, newest at the highest index. Maximum 50 entries. Persistent storage
    /// is required for FR-02.8 auditability -- must survive ledger TTL expiry.
    ScoreHistory(Address),
    /// Current history length for the wallet. Instance storage.
    /// Maintained alongside `ScoreHistory` as a cheap cap check before loading
    /// the full persistent Vec on every `anchor_score` call.
    SnapshotCount(Address),
}

// -----------------------------------------------------------------------------
// Contract struct
// -----------------------------------------------------------------------------

#[contract]
pub struct TrustScoreContract;

// -----------------------------------------------------------------------------
// Internal helper -- admin auth
// -----------------------------------------------------------------------------

/// Load the admin address and call `require_auth()`.
///
/// Returns `NotInitialized` (101) if `initialize` has not yet been called.
/// Traps at the Soroban host level (`INVOKE_HOST_FUNCTION_TRAPPED`) if the
/// transaction does not carry the required admin signature -- not a
/// `ContractError` return. See INTERFACES.md Section 8 auth failure model.
///
/// Must be called as the **first statement** in every admin-only function,
/// before any state read or write. Verified by issue #022 security review.
/// Canonical pattern: INTERFACES.md Section 9.
fn require_admin(env: &Env) -> Result<(), ContractError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(ContractError::NotInitialized)?;
    admin.require_auth();
    Ok(())
}

// -----------------------------------------------------------------------------
// Contract implementation
// -----------------------------------------------------------------------------

#[contractimpl]
impl TrustScoreContract {
    /// Stores the admin address. Called once immediately after deployment.
    ///
    /// Returns `AlreadyInitialized` (100) on any subsequent call; the stored
    /// admin address is never overwritten.
    ///
    /// The deployment script (issue #021) must call `initialize` in the same
    /// Stellar multi-operation transaction as WASM upload and contract creation
    /// to eliminate the initialisation race window (INTERFACES.md Section 9).
    ///
    /// ABI: INTERFACES.md Section 6, function table row 1.
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Anchors a new Trust Score snapshot on-chain. Admin-only.
    ///
    /// The ForgePass backend's `OnchainWriterService` calls this after each
    /// score recalculation (issue #036). The NestJS scoring engine normalises
    /// scores to [0, 100] before calling this contract; a 400 response
    /// indicates a backend bug and must be treated as a blocking alert.
    ///
    /// **Validation:** rejects `score > 100` with `InvalidScore` (400). u32 is
    /// always >= 0 so only the upper bound is checked.
    ///
    /// **History management (Step 1 corrected -- ascending, append strategy):**
    /// Snapshots are appended to the end of the Vec so the Vec remains ordered
    /// ascending by `computed_at` (oldest at index 0, newest at highest index).
    /// When the 50-entry cap is reached, `pop_front()` removes the oldest entry
    /// before `push_back()` appends the new snapshot. `SnapshotCount` remains
    /// at 50 across the removal and append. No archive key is written.
    ///
    /// **Score computation is off-chain.** This function does not validate
    /// `algorithm_version` strings or verify `signal_hash`. Input validation
    /// is limited to the score range check.
    ///
    /// ABI: INTERFACES.md Section 6, function table row 2.
    pub fn anchor_score(
        env: Env,
        wallet: Address,
        score: u32,
        algorithm_version: String,
        signal_hash: String,
        computed_at: u64,
    ) -> Result<(), ContractError> {
        require_admin(&env)?;

        // Validate score range [0, 100]. u32 >= 0 is guaranteed, upper bound only.
        if score > 100 {
            return Err(ContractError::InvalidScore);
        }

        let new_snapshot = ScoreSnapshot {
            score,
            algorithm_version,
            signal_hash,
            computed_at,
        };

        // Load existing history from persistent storage (or empty Vec on first call).
        let history_key = DataKey::ScoreHistory(wallet.clone());
        let mut history: Vec<ScoreSnapshot> = env
            .storage()
            .persistent()
            .get(&history_key)
            .unwrap_or_else(|| Vec::new(&env));

        // Defensive guard: history should never exceed 50 in a correct implementation.
        // HistoryCapExceeded (401) indicates a backend bug, not a user error.
        if history.len() > 50 {
            return Err(ContractError::HistoryCapExceeded);
        }

        // Cap management: remove oldest entry (index 0) when at the 50-entry ceiling.
        // History is ascending -- index 0 is oldest. pop_front() removes it.
        // SnapshotCount goes 50 -> 49 here, then back to 50 after push_back.
        if history.len() == 50 {
            history.pop_front();
        }

        // Append new snapshot at the end, maintaining ascending computed_at order.
        history.push_back(new_snapshot.clone());

        // Persist updated history to persistent storage.
        env.storage().persistent().set(&history_key, &history);

        // Mirror latest snapshot to instance storage for O(1) third-party reads.
        env.storage()
            .instance()
            .set(&DataKey::CurrentScore(wallet.clone()), &new_snapshot);

        // Update SnapshotCount in instance storage to match the Vec length.
        env.storage()
            .instance()
            .set(&DataKey::SnapshotCount(wallet), &history.len());

        Ok(())
    }

    /// Returns the most recent `ScoreSnapshot` for the wallet, or `None` if no
    /// score has been anchored yet. Public -- no auth.
    ///
    /// Reads from instance storage (`DataKey::CurrentScore`) for O(1) access,
    /// avoiding the persistent storage read required for the full history Vec.
    /// This is the common path for third-party score reads.
    ///
    /// ABI: INTERFACES.md Section 6, function table row 3.
    pub fn get_current_score(env: Env, wallet: Address) -> Option<ScoreSnapshot> {
        env.storage().instance().get(&DataKey::CurrentScore(wallet))
    }

    /// Returns the full score history for the wallet as a `Vec<ScoreSnapshot>`,
    /// ordered ascending by `computed_at` (oldest at index 0). Returns an empty
    /// `Vec` if no scores have been anchored. Public -- no auth.
    ///
    /// Ordering holds naturally because `anchor_score` appends entries
    /// chronologically and Soroban `Vec` preserves insertion order. The
    /// implementation does not sort on read -- order is maintained on write.
    ///
    /// ABI: INTERFACES.md Section 6, function table row 4.
    pub fn get_score_history(env: Env, wallet: Address) -> Vec<ScoreSnapshot> {
        env.storage()
            .persistent()
            .get(&DataKey::ScoreHistory(wallet))
            .unwrap_or_else(|| Vec::new(&env))
    }
}

// =============================================================================
// Unit tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    /// Deploy and initialize the trust-score contract.
    /// Returns the env and a ready client with mock_all_auths active.
    fn setup() -> (Env, TrustScoreContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(TrustScoreContract, ());
        let client = TrustScoreContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client)
    }

    /// A valid semver algorithm version string for use in tests.
    fn test_algo_version(env: &Env) -> String {
        String::from_str(env, "1.0")
    }

    /// A valid 64-char SHA-256 hex string for use in tests.
    fn test_signal_hash(env: &Env) -> String {
        String::from_str(
            env,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
    }

    // -------------------------------------------------------------------------
    // Test 1 -- anchor_score happy path
    // -------------------------------------------------------------------------

    /// AC-1: anchoring a score stores a retrievable snapshot with all four
    /// `ScoreSnapshot` fields intact. No tx_id field (Step 1 correction).
    #[test]
    fn test_anchor_and_retrieve_happy_path() {
        let (env, client) = setup();
        let wallet = Address::generate(&env);

        client.anchor_score(
            &wallet,
            &75u32,
            &test_algo_version(&env),
            &test_signal_hash(&env),
            &1000u64,
        );

        let snapshot = client
            .get_current_score(&wallet)
            .expect("snapshot must exist after anchor_score");

        assert_eq!(snapshot.score, 75);
        assert_eq!(snapshot.algorithm_version, test_algo_version(&env));
        assert_eq!(snapshot.signal_hash, test_signal_hash(&env));
        assert_eq!(snapshot.computed_at, 1000u64);
    }

    // -------------------------------------------------------------------------
    // Test 2 -- get_current_score returns None before any anchor
    // -------------------------------------------------------------------------

    /// `get_current_score` returns `None` for a wallet with no anchored score.
    #[test]
    fn test_get_current_score_none_before_anchor() {
        let (env, client) = setup();
        let wallet = Address::generate(&env);

        assert!(client.get_current_score(&wallet).is_none());
    }

    // -------------------------------------------------------------------------
    // Test 3 -- history is ascending by computed_at
    // -------------------------------------------------------------------------

    /// AC-2 (revised): five sequential anchor calls produce a history Vec ordered
    /// ascending by `computed_at`. Index 0 is the oldest; index 4 is the newest.
    /// Ordering is maintained on write (append) -- not sorted on read.
    #[test]
    fn test_history_ascending_order() {
        let (env, client) = setup();
        let wallet = Address::generate(&env);

        let timestamps: [u64; 5] = [100, 200, 300, 400, 500];
        for &ts in &timestamps {
            client.anchor_score(
                &wallet,
                &50u32,
                &test_algo_version(&env),
                &test_signal_hash(&env),
                &ts,
            );
        }

        let history = client.get_score_history(&wallet);
        assert_eq!(history.len(), 5);

        for (i, &expected_ts) in timestamps.iter().enumerate() {
            let actual_ts = history.get(i as u32).unwrap().computed_at;
            assert_eq!(
                actual_ts, expected_ts,
                "index {i}: expected computed_at={expected_ts}, got {actual_ts}"
            );
        }
    }

    // -------------------------------------------------------------------------
    // Test 4 -- admin-only enforcement is a host trap
    // -------------------------------------------------------------------------

    /// AC-3: calling `anchor_score` without the admin signature is a Soroban host
    /// trap (`INVOKE_HOST_FUNCTION_TRAPPED`), not a `ContractError` return.
    /// `require_admin` calls `admin.require_auth()` which traps on auth failure.
    #[test]
    #[should_panic]
    fn test_non_admin_anchor_score_is_host_trap() {
        let env = Env::default();
        // No mock_all_auths -- require_auth() traps without a matching entry.
        let contract_id = env.register(TrustScoreContract, ());
        let client = TrustScoreContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        // initialize is admin-bootstrap and does not call require_auth, so it
        // succeeds without mocked auth.
        client.initialize(&admin);

        let wallet = Address::generate(&env);
        // require_admin loads the admin address then calls admin.require_auth().
        // No auth entry in env -> host trap -> #[should_panic] catches this.
        client.anchor_score(
            &wallet,
            &50u32,
            &test_algo_version(&env),
            &test_signal_hash(&env),
            &1000u64,
        );
    }

    // -------------------------------------------------------------------------
    // Test 5 -- score range validation: invalid score
    // -------------------------------------------------------------------------

    /// AC-4: `anchor_score` with `score = 101` returns `InvalidScore` (400).
    /// No snapshot is written -- both `get_current_score` and
    /// `get_score_history` remain empty after the rejected call.
    #[test]
    fn test_score_range_invalid() {
        let (env, client) = setup();
        let wallet = Address::generate(&env);

        let result = client.try_anchor_score(
            &wallet,
            &101u32,
            &test_algo_version(&env),
            &test_signal_hash(&env),
            &1000u64,
        );

        assert_eq!(result, Err(Ok(ContractError::InvalidScore)));
        assert!(client.get_current_score(&wallet).is_none());
        assert_eq!(client.get_score_history(&wallet).len(), 0);
    }

    // -------------------------------------------------------------------------
    // Test 6 -- score range validation: boundary values are valid
    // -------------------------------------------------------------------------

    /// `score = 0` (lower boundary) and `score = 100` (upper boundary) both
    /// succeed without returning an error.
    #[test]
    fn test_score_range_boundary_valid() {
        let (env, client) = setup();
        let wallet = Address::generate(&env);

        // Lower boundary: score = 0.
        client.anchor_score(
            &wallet,
            &0u32,
            &test_algo_version(&env),
            &test_signal_hash(&env),
            &1000u64,
        );
        assert_eq!(client.get_current_score(&wallet).unwrap().score, 0);

        // Upper boundary: score = 100.
        client.anchor_score(
            &wallet,
            &100u32,
            &test_algo_version(&env),
            &test_signal_hash(&env),
            &2000u64,
        );
        assert_eq!(client.get_current_score(&wallet).unwrap().score, 100);
    }

    // -------------------------------------------------------------------------
    // Test 7 -- initialize idempotency
    // -------------------------------------------------------------------------

    /// Second `initialize` call returns `AlreadyInitialized` (100).
    /// The stored admin address is never overwritten.
    #[test]
    fn test_initialize_twice_returns_already_initialized() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(TrustScoreContract, ());
        let client = TrustScoreContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client.initialize(&admin);
        let result = client.try_initialize(&admin);

        assert_eq!(result, Err(Ok(ContractError::AlreadyInitialized)));
    }

    // -------------------------------------------------------------------------
    // Test 8 -- get_score_history returns empty Vec before any anchor
    // -------------------------------------------------------------------------

    /// `get_score_history` returns an empty `Vec` for a wallet with no anchored
    /// scores. Never panics on a missing key.
    #[test]
    fn test_get_score_history_empty_before_anchor() {
        let (env, client) = setup();
        let wallet = Address::generate(&env);

        assert_eq!(client.get_score_history(&wallet).len(), 0);
    }

    // -------------------------------------------------------------------------
    // Test 9 -- get_current_score returns the most recent snapshot
    // -------------------------------------------------------------------------

    /// After three anchor calls, `get_current_score` returns the snapshot from
    /// the most recent call, not the first or second.
    #[test]
    fn test_get_current_score_returns_most_recent() {
        let (env, client) = setup();
        let wallet = Address::generate(&env);

        client.anchor_score(
            &wallet,
            &30u32,
            &test_algo_version(&env),
            &test_signal_hash(&env),
            &1000u64,
        );
        client.anchor_score(
            &wallet,
            &55u32,
            &test_algo_version(&env),
            &test_signal_hash(&env),
            &2000u64,
        );
        client.anchor_score(
            &wallet,
            &80u32,
            &test_algo_version(&env),
            &test_signal_hash(&env),
            &3000u64,
        );

        let current = client
            .get_current_score(&wallet)
            .expect("must have a score after three anchors");

        assert_eq!(current.score, 80, "must reflect the most recent score");
        assert_eq!(
            current.computed_at, 3000u64,
            "must reflect the most recent timestamp"
        );
    }

    // -------------------------------------------------------------------------
    // Test 10 -- history cap drops oldest entry after 51 anchor calls
    // -------------------------------------------------------------------------

    /// AC-5 and AC-6 (revised): after 51 `anchor_score` calls:
    ///   - The history Vec has exactly 50 entries (cap enforced).
    ///   - The first call's snapshot (computed_at=1000) is absent (oldest dropped).
    ///   - Index 0 holds the second call's snapshot (computed_at=1001).
    ///   - Index 49 holds the final call's snapshot (computed_at=1050).
    ///
    /// No archive DataKey is checked -- the trust-score contract does not write
    /// an archive entry when dropping the oldest snapshot (Step 1 correction).
    #[test]
    fn test_history_cap_drops_oldest() {
        let (env, client) = setup();
        let wallet = Address::generate(&env);

        // Anchor 51 scores with strictly increasing computed_at values.
        // Call 1 -> computed_at=1000, call 2 -> 1001, ..., call 51 -> 1050.
        for i in 0u64..51 {
            client.anchor_score(
                &wallet,
                &50u32,
                &test_algo_version(&env),
                &test_signal_hash(&env),
                &(1000 + i),
            );
        }

        let history = client.get_score_history(&wallet);

        // Vec must have exactly 50 entries after the oldest was removed.
        assert_eq!(
            history.len(),
            50,
            "history must be capped at exactly 50 entries"
        );

        // The first call (computed_at=1000) must be absent from every index.
        for i in 0..50u32 {
            assert_ne!(
                history.get(i).unwrap().computed_at,
                1000u64,
                "computed_at=1000 (first call) must not appear at index {i}"
            );
        }

        // Second call (computed_at=1001) must now occupy index 0 (oldest).
        assert_eq!(
            history.get(0).unwrap().computed_at,
            1001u64,
            "index 0 must hold the second call after the first is dropped"
        );

        // Final call (computed_at=1050) must occupy index 49 (newest).
        assert_eq!(
            history.get(49).unwrap().computed_at,
            1050u64,
            "index 49 must hold the most recent anchor call"
        );
    }
}
