//! # ForgePass Credential Store Contract
//!
//! Anchors contribution signal proofs on-chain for each contributor passport.
//! Every signal the off-chain indexers collect travels through this contract
//! before it can influence a Trust Score.
//!
//! **ABI authority:** `contracts/INTERFACES.md` Section 5 (Issue #014, complete)
//! **Architecture:** `contracts/ARCHITECTURE.md` Sections 3–6
//!
//! ## Archival model
//!
//! Archival is **backend-controlled, not contract-triggered**. The contract
//! exposes `get_credential_count`, `add_archive_record`, and `remove_credentials`
//! so the NestJS backend can orchestrate the full archival workflow before each
//! `add_credential` call when the live count reaches 100.
//!
//! ## Functions (7 total)
//!
//! | Function               | Access      |
//! |------------------------|-------------|
//! | `initialize`           | Admin-bootstrap |
//! | `add_credential`       | Admin-only  |
//! | `get_credentials`      | Public      |
//! | `get_credential_count` | Public      |
//! | `credential_exists`    | Public      |
//! | `add_archive_record`   | Admin-only  |
//! | `get_archive_records`  | Public      |
//! | `remove_credentials`   | Admin-only  |

#![cfg_attr(not(test), no_std)]

use soroban_sdk::{contract, contractimpl, contracttype, vec, Address, BytesN, Env, String, Vec};

use forgepass_shared::{
    ArchiveRecord, ContractError, CredentialRecord, SignalType,
};

// =============================================================================
// Storage keys
// =============================================================================

/// Per-contract DataKey enum. Not shared -- prevents cross-contract key
/// collisions. Each variant maps to a distinct ledger entry.
///
/// Storage tiers (INTERFACES.md Section 2):
/// - `Credentials(Address)` → Persistent (credential proofs are permanent anchors)
/// - `ArchiveRecord(Address, u32)` → Persistent (one entry per archival cycle)
/// - `CredentialCounter(Address)` → Instance (monotonic ID generator per wallet)
/// - `ArchiveIndex(Address)` → Instance (next archive_index for this wallet)
/// - `Admin` → Instance (set once at initialize; config-level value)
#[contracttype]
pub enum DataKey {
    /// Admin address. Instance storage. Set once at `initialize`.
    Admin,
    /// Live credential set for a wallet. Persistent. Vec<CredentialRecord>.
    /// At most 100 entries before the backend triggers the archival workflow.
    Credentials(Address),
    /// Per-wallet monotonic credential ID counter. Instance.
    /// Starts at 1, increments on every successful `add_credential`.
    /// Never decremented after archival.
    CredentialCounter(Address),
    /// On-chain Merkle root proof for one archival cycle. Persistent.
    /// Keyed by (wallet, archive_index) where archive_index starts at 0.
    ArchiveRecord(Address, u32),
    /// Next archive_index for a wallet. Instance.
    /// Incremented by `add_archive_record` on each successful anchor.
    ArchiveIndex(Address),
}

// =============================================================================
// Admin auth helper
// =============================================================================

/// Load and authenticate the admin address.
///
/// Returns `NotInitialized` (101) if `initialize` has not been called.
/// Causes a Soroban host trap (`INVOKE_HOST_FUNCTION_TRAPPED`) if the required
/// admin signature is absent from the transaction -- auth failures are host
/// traps, not `ContractError` returns (INTERFACES.md Section 8).
fn require_admin(env: &Env) -> Result<(), ContractError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(ContractError::NotInitialized)?;
    admin.require_auth();
    Ok(())
}

// =============================================================================
// Contract
// =============================================================================

#[contract]
pub struct CredentialStoreContract;

#[contractimpl]
impl CredentialStoreContract {
    // -------------------------------------------------------------------------
    // initialize
    // -------------------------------------------------------------------------

    /// Called once immediately after deployment. Stores the admin address in
    /// instance storage. Returns `AlreadyInitialized` (100) on any subsequent
    /// call. Canonical pattern from INTERFACES.md Section 9.
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // add_credential  (admin-only)
    // -------------------------------------------------------------------------

    /// Writes a new credential proof to the contributor's live on-chain record.
    ///
    /// Execution order (strict):
    /// 1. Admin auth check
    /// 2. Live-set deduplication check -- returns `CredentialAlreadyExists` (300)
    ///    if `(signal_type, source_id)` already exists. Does NOT check archived
    ///    credentials; the backend checks PostgreSQL for archived duplicates before
    ///    calling this function.
    /// 3. Write CredentialRecord to live Vec
    /// 4. Increment per-wallet counter; return new credential id
    ///
    /// The backend is responsible for checking `get_credential_count` before
    /// calling this function and running the archival workflow if count == 100.
    /// This contract does not trigger archival internally.
    pub fn add_credential(
        env: Env,
        wallet: Address,
        signal_type: SignalType,
        source_id: String,
        event_date: u64,
        data_hash: String,
    ) -> Result<u64, ContractError> {
        // 1. Admin auth -- must be first statement before any state read or write.
        require_admin(&env)?;

        // 2. Deduplication check on live set.
        let mut credentials: Vec<CredentialRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Credentials(wallet.clone()))
            .unwrap_or(vec![&env]);

        for cred in credentials.iter() {
            if cred.signal_type == signal_type && cred.source_id == source_id {
                return Err(ContractError::CredentialAlreadyExists);
            }
        }

        // 3. Generate credential id from per-wallet monotonic counter.
        let counter_key = DataKey::CredentialCounter(wallet.clone());
        let id: u64 = env
            .storage()
            .instance()
            .get(&counter_key)
            .unwrap_or(0u64)
            + 1;

        // 4. Build and append the new record.
        let record = CredentialRecord {
            id,
            wallet: wallet.clone(),
            signal_type,
            source_id,
            event_date,
            data_hash,
        };
        credentials.push_back(record);

        // 5. Persist updated Vec and incremented counter.
        env.storage()
            .persistent()
            .set(&DataKey::Credentials(wallet.clone()), &credentials);
        env.storage().instance().set(&counter_key, &id);

        Ok(id)
    }

    // -------------------------------------------------------------------------
    // get_credentials  (public)
    // -------------------------------------------------------------------------

    /// Returns the full live credential set for the wallet.
    /// Returns an empty Vec if the wallet has no live credentials.
    /// Does not include archived credentials -- call `get_archive_records` for those.
    pub fn get_credentials(env: Env, wallet: Address) -> Vec<CredentialRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::Credentials(wallet))
            .unwrap_or(vec![&env])
    }

    // -------------------------------------------------------------------------
    // get_credential_count  (public)
    // -------------------------------------------------------------------------

    /// Returns the count of live on-chain credentials for the wallet.
    /// Returns 0 for wallets with no live credentials.
    /// Does not include archived credentials.
    /// The backend checks this before every `add_credential` call to determine
    /// whether the archival workflow must run first.
    pub fn get_credential_count(env: Env, wallet: Address) -> u32 {
        let credentials: Vec<CredentialRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Credentials(wallet))
            .unwrap_or(vec![&env]);
        credentials.len()
    }

    // -------------------------------------------------------------------------
    // credential_exists  (public)
    // -------------------------------------------------------------------------

    /// Returns true if `(signal_type, source_id)` exists in the live credential
    /// set for the wallet. Checks live set only -- not archived credentials.
    /// Linear scan over at most 100 entries (acceptable at the ceiling defined
    /// in ARCHITECTURE.md Section 3).
    pub fn credential_exists(
        env: Env,
        wallet: Address,
        signal_type: SignalType,
        source_id: String,
    ) -> bool {
        let credentials: Vec<CredentialRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Credentials(wallet))
            .unwrap_or(vec![&env]);

        for cred in credentials.iter() {
            if cred.signal_type == signal_type && cred.source_id == source_id {
                return true;
            }
        }
        false
    }

    // -------------------------------------------------------------------------
    // add_archive_record  (admin-only)
    // -------------------------------------------------------------------------

    /// Anchors a Merkle root on-chain after the backend completes an archival
    /// cycle. Admin-only.
    ///
    /// Call order (enforced by backend, not contract):
    ///   1. Write archive JSON to PostgreSQL
    ///   2. Pin archive JSON to IPFS (receive CID)
    ///   3. Call `add_archive_record` (this function) -- anchors Merkle root
    ///   4. Call `remove_credentials` -- removes archived entries from live set
    ///   5. Call `add_credential` -- writes the new credential that triggered archival
    ///
    /// Increments `DataKey::ArchiveIndex(wallet)` on success so each call gets
    /// a unique archive_index slot.
    pub fn add_archive_record(
        env: Env,
        wallet: Address,
        merkle_root: BytesN<32>,
        credential_count: u32,
        archived_at: u64,
        ipfs_cid: String,
    ) -> Result<(), ContractError> {
        // Admin auth -- must be first.
        require_admin(&env)?;

        // Read current archive_index for this wallet (starts at 0).
        let index_key = DataKey::ArchiveIndex(wallet.clone());
        let archive_index: u32 = env
            .storage()
            .instance()
            .get(&index_key)
            .unwrap_or(0u32);

        // Build and store the ArchiveRecord.
        let record = ArchiveRecord {
            merkle_root,
            credential_count,
            archived_at,
            ipfs_cid,
        };
        env.storage()
            .persistent()
            .set(&DataKey::ArchiveRecord(wallet.clone(), archive_index), &record);

        // Advance the index counter for the next archival cycle.
        env.storage()
            .instance()
            .set(&index_key, &(archive_index + 1));

        Ok(())
    }

    // -------------------------------------------------------------------------
    // get_archive_records  (public)
    // -------------------------------------------------------------------------

    /// Returns all ArchiveRecord entries for the wallet, ordered by
    /// archive_index ascending (oldest archival cycle first).
    /// Returns an empty Vec if no archival has occurred.
    pub fn get_archive_records(env: Env, wallet: Address) -> Vec<ArchiveRecord> {
        let index_key = DataKey::ArchiveIndex(wallet.clone());
        let total: u32 = env
            .storage()
            .instance()
            .get(&index_key)
            .unwrap_or(0u32);

        let mut records: Vec<ArchiveRecord> = vec![&env];
        for i in 0..total {
            if let Some(record) = env
                .storage()
                .persistent()
                .get(&DataKey::ArchiveRecord(wallet.clone(), i))
            {
                records.push_back(record);
            }
        }
        records
    }

    // -------------------------------------------------------------------------
    // remove_credentials  (admin-only)
    // -------------------------------------------------------------------------

    /// Deletes the specified live credential entries from on-chain storage.
    /// Admin-only.
    ///
    /// Safety precondition: at least one ArchiveRecord must exist for the wallet.
    /// If none exists, returns `ArchiveRecordRequired` (302) -- prevents credential
    /// deletion without a corresponding on-chain Merkle root proof.
    ///
    /// `source_ids` not found in the live set are silently skipped (not an error),
    /// so retries after partial failure are safe.
    ///
    /// Must only be called AFTER `add_archive_record` has confirmed on-chain for
    /// the credentials being removed (ARCHITECTURE.md Section 4).
    pub fn remove_credentials(
        env: Env,
        wallet: Address,
        source_ids: Vec<String>,
    ) -> Result<(), ContractError> {
        // Admin auth -- must be first.
        require_admin(&env)?;

        // Safety guard: at least one ArchiveRecord must exist for this wallet.
        let index_key = DataKey::ArchiveIndex(wallet.clone());
        let archive_count: u32 = env
            .storage()
            .instance()
            .get(&index_key)
            .unwrap_or(0u32);

        if archive_count == 0 {
            return Err(ContractError::ArchiveRecordRequired);
        }

        // Load live credential set.
        let credentials: Vec<CredentialRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::Credentials(wallet.clone()))
            .unwrap_or(vec![&env]);

        // Rebuild Vec excluding credentials whose source_id is in the removal set.
        let mut updated: Vec<CredentialRecord> = vec![&env];
        for cred in credentials.iter() {
            let mut should_remove = false;
            for sid in source_ids.iter() {
                if cred.source_id == sid {
                    should_remove = true;
                    break;
                }
            }
            if !should_remove {
                updated.push_back(cred);
            }
        }

        env.storage()
            .persistent()
            .set(&DataKey::Credentials(wallet), &updated);

        Ok(())
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, vec, Env};

    /// Shared test setup: deploy the contract and initialize with a fresh admin.
    fn setup() -> (Env, CredentialStoreContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CredentialStoreContract, ());
        let client = CredentialStoreContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    /// Helper: add one credential with default test values.
    fn add_test_credential(
        client: &CredentialStoreContractClient,
        env: &Env,
        wallet: &Address,
        signal_type: SignalType,
        source_id: &str,
    ) -> u64 {
        client.add_credential(
            wallet,
            &signal_type,
            &String::from_str(env, source_id),
            &1_716_249_600u64,
            &String::from_str(env, "a3f1c2d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2"),
        )
    }

    // -------------------------------------------------------------------------
    // Test 1: happy path -- add and retrieve one credential
    // -------------------------------------------------------------------------
    #[test]
    fn test_add_and_retrieve_credential() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        let id = add_test_credential(&client, &env, &wallet, SignalType::GithubPr, "stellar/core#1");
        assert_eq!(id, 1);

        let creds = client.get_credentials(&wallet);
        assert_eq!(creds.len(), 1);

        let cred = creds.get(0).unwrap();
        assert_eq!(cred.id, 1);
        assert_eq!(cred.signal_type, SignalType::GithubPr);
        assert_eq!(cred.source_id, String::from_str(&env, "stellar/core#1"));
        assert_eq!(cred.event_date, 1_716_249_600u64);
    }

    // -------------------------------------------------------------------------
    // Test 2: credential counter increments on each distinct add
    // -------------------------------------------------------------------------
    #[test]
    fn test_credential_count_increments() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        add_test_credential(&client, &env, &wallet, SignalType::GithubPr, "stellar/core#1");
        add_test_credential(&client, &env, &wallet, SignalType::GithubPr, "stellar/core#2");
        add_test_credential(&client, &env, &wallet, SignalType::SorobanContract, "contract-addr-1");

        assert_eq!(client.get_credential_count(&wallet), 3);

        let creds = client.get_credentials(&wallet);
        assert_eq!(creds.get(0).unwrap().id, 1);
        assert_eq!(creds.get(1).unwrap().id, 2);
        assert_eq!(creds.get(2).unwrap().id, 3);
    }

    // -------------------------------------------------------------------------
    // Test 3: exact duplicate (same signal_type + source_id) is rejected
    // -------------------------------------------------------------------------
    #[test]
    fn test_deduplication_same_signal_same_source() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        add_test_credential(&client, &env, &wallet, SignalType::GithubPr, "stellar/core#1");

        let result = client.try_add_credential(
            &wallet,
            &SignalType::GithubPr,
            &String::from_str(&env, "stellar/core#1"),
            &1_716_249_600u64,
            &String::from_str(&env, "a3f1c2d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2"),
        );

        assert_eq!(
            result,
            Err(Ok(ContractError::CredentialAlreadyExists)),
            "duplicate (signal_type, source_id) must return CredentialAlreadyExists"
        );
        // Count unchanged
        assert_eq!(client.get_credential_count(&wallet), 1);
    }

    // -------------------------------------------------------------------------
    // Test 4: same source_id with different signal_type is NOT a duplicate
    // -------------------------------------------------------------------------
    #[test]
    fn test_deduplication_different_signal_same_source() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        let id1 = add_test_credential(&client, &env, &wallet, SignalType::GithubPr, "shared-source-id");
        let id2 = add_test_credential(&client, &env, &wallet, SignalType::SorobanContract, "shared-source-id");

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(client.get_credential_count(&wallet), 2);
    }

    // -------------------------------------------------------------------------
    // Test 5: non-admin call is rejected (host trap via #[should_panic])
    // -------------------------------------------------------------------------
    #[test]
    #[should_panic]
    fn test_admin_only_add_credential_host_trap() {
        let env = Env::default();
        // No mock_all_auths -- auth will trap.
        let contract_id = env.register(CredentialStoreContract, ());
        let client = CredentialStoreContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        // Initialize with mocked auth just for setup.
        env.mock_all_auths();
        client.initialize(&admin);

        // Now attempt add_credential without any auth mock -- should trap.
        env.set_auths(&[]);
        let wallet = Address::generate(&env);
        client.add_credential(
            &wallet,
            &SignalType::GithubPr,
            &String::from_str(&env, "stellar/core#1"),
            &1_716_249_600u64,
            &String::from_str(&env, "a3f1c2d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2"),
        );
    }

    // -------------------------------------------------------------------------
    // Test 6: credential_exists -- positive case
    // -------------------------------------------------------------------------
    #[test]
    fn test_credential_exists_positive() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        add_test_credential(&client, &env, &wallet, SignalType::StellarDex, "pair-xlm-usdc");

        assert!(client.credential_exists(
            &wallet,
            &SignalType::StellarDex,
            &String::from_str(&env, "pair-xlm-usdc"),
        ));
    }

    // -------------------------------------------------------------------------
    // Test 7: credential_exists -- negative case
    // -------------------------------------------------------------------------
    #[test]
    fn test_credential_exists_negative() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        assert!(!client.credential_exists(
            &wallet,
            &SignalType::GithubPr,
            &String::from_str(&env, "never-added"),
        ));
    }

    // -------------------------------------------------------------------------
    // Test 8: add_archive_record anchors a Merkle root on-chain
    // -------------------------------------------------------------------------
    #[test]
    fn test_add_archive_record_stores_record() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        let merkle_root = BytesN::from_array(&env, &[0xabu8; 32]);

        client.add_archive_record(
            &wallet,
            &merkle_root,
            &50u32,
            &1_716_249_600u64,
            &String::from_str(&env, "QmTestCID"),
        );

        let records = client.get_archive_records(&wallet);
        assert_eq!(records.len(), 1);

        let rec = records.get(0).unwrap();
        assert_eq!(rec.merkle_root, merkle_root);
        assert_eq!(rec.credential_count, 50u32);
        assert_eq!(rec.ipfs_cid, String::from_str(&env, "QmTestCID"));
    }

    // -------------------------------------------------------------------------
    // Test 9: multiple archive cycles accumulate in order
    // -------------------------------------------------------------------------
    #[test]
    fn test_archive_index_increments_across_cycles() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        let root1 = BytesN::from_array(&env, &[0x11u8; 32]);
        let root2 = BytesN::from_array(&env, &[0x22u8; 32]);

        client.add_archive_record(&wallet, &root1, &50u32, &1_000u64, &String::from_str(&env, "CID1"));
        client.add_archive_record(&wallet, &root2, &50u32, &2_000u64, &String::from_str(&env, "CID2"));

        let records = client.get_archive_records(&wallet);
        assert_eq!(records.len(), 2);
        assert_eq!(records.get(0).unwrap().merkle_root, root1);
        assert_eq!(records.get(1).unwrap().merkle_root, root2);
    }

    // -------------------------------------------------------------------------
    // Test 10: remove_credentials without an ArchiveRecord returns 302
    // -------------------------------------------------------------------------
    #[test]
    fn test_remove_credentials_without_archive_record_returns_302() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        add_test_credential(&client, &env, &wallet, SignalType::GithubPr, "stellar/core#1");

        let result = client.try_remove_credentials(
            &wallet,
            &vec![&env, String::from_str(&env, "stellar/core#1")],
        );

        assert_eq!(
            result,
            Err(Ok(ContractError::ArchiveRecordRequired)),
            "remove_credentials without an ArchiveRecord must return ArchiveRecordRequired (302)"
        );
        // Live set unchanged
        assert_eq!(client.get_credential_count(&wallet), 1);
    }

    // -------------------------------------------------------------------------
    // Test 11: remove_credentials after add_archive_record removes entries
    // -------------------------------------------------------------------------
    #[test]
    fn test_remove_credentials_after_archive_record_succeeds() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        add_test_credential(&client, &env, &wallet, SignalType::GithubPr, "stellar/core#1");
        add_test_credential(&client, &env, &wallet, SignalType::GithubPr, "stellar/core#2");

        // Anchor an ArchiveRecord first.
        let merkle_root = BytesN::from_array(&env, &[0xabu8; 32]);
        client.add_archive_record(
            &wallet,
            &merkle_root,
            &1u32,
            &1_716_249_600u64,
            &String::from_str(&env, "QmTestCID"),
        );

        // Remove one credential.
        client.remove_credentials(
            &wallet,
            &vec![&env, String::from_str(&env, "stellar/core#1")],
        );

        // Live set now has one entry.
        assert_eq!(client.get_credential_count(&wallet), 1);
        let remaining = client.get_credentials(&wallet);
        assert_eq!(
            remaining.get(0).unwrap().source_id,
            String::from_str(&env, "stellar/core#2")
        );
    }

    // -------------------------------------------------------------------------
    // Test 12: remove_credentials silently skips unknown source_ids
    // -------------------------------------------------------------------------
    #[test]
    fn test_remove_credentials_skips_unknown_source_ids() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        add_test_credential(&client, &env, &wallet, SignalType::GithubPr, "stellar/core#1");

        let merkle_root = BytesN::from_array(&env, &[0xabu8; 32]);
        client.add_archive_record(
            &wallet,
            &merkle_root,
            &1u32,
            &1_716_249_600u64,
            &String::from_str(&env, "QmCID"),
        );

        // Remove a source_id that does not exist -- must not error.
        client.remove_credentials(
            &wallet,
            &vec![&env, String::from_str(&env, "never-added")],
        );

        // Live set unchanged.
        assert_eq!(client.get_credential_count(&wallet), 1);
    }

    // -------------------------------------------------------------------------
    // Test 13: empty wallet returns empty Vec and zero count
    // -------------------------------------------------------------------------
    #[test]
    fn test_empty_wallet_returns_empty_vec_and_zero() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        assert_eq!(client.get_credential_count(&wallet), 0);
        assert_eq!(client.get_credentials(&wallet).len(), 0);
        assert_eq!(client.get_archive_records(&wallet).len(), 0);
    }

    // -------------------------------------------------------------------------
    // Test 14: initialize twice returns AlreadyInitialized (100)
    // -------------------------------------------------------------------------
    #[test]
    fn test_initialize_twice_returns_already_initialized() {
        let (_env, client, admin) = setup();
        let result = client.try_initialize(&admin);
        assert_eq!(result, Err(Ok(ContractError::AlreadyInitialized)));
    }

    // -------------------------------------------------------------------------
    // Test 15: credential ID never reuses after remove
    // -------------------------------------------------------------------------
    #[test]
    fn test_credential_id_monotonic_after_remove() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        let id1 = add_test_credential(&client, &env, &wallet, SignalType::GithubPr, "stellar/core#1");
        let id2 = add_test_credential(&client, &env, &wallet, SignalType::GithubPr, "stellar/core#2");
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        // Archive and remove.
        let merkle_root = BytesN::from_array(&env, &[0xabu8; 32]);
        client.add_archive_record(&wallet, &merkle_root, &1u32, &1_000u64, &String::from_str(&env, "QmCID"));
        client.remove_credentials(&wallet, &vec![&env, String::from_str(&env, "stellar/core#1")]);

        // New credential ID must be higher than all previously issued IDs.
        let id3 = add_test_credential(&client, &env, &wallet, SignalType::StellarDex, "pair-xlm-usdc");
        assert!(id3 > id2, "ID after archival/removal must exceed all prior IDs -- was {id3}, expected > {id2}");
    }

    // -------------------------------------------------------------------------
    // Test 16: wallets are fully isolated
    // -------------------------------------------------------------------------
    #[test]
    fn test_wallet_isolation() {
        let (env, client, _admin) = setup();
        let wallet_a = Address::generate(&env);
        let wallet_b = Address::generate(&env);

        add_test_credential(&client, &env, &wallet_a, SignalType::GithubPr, "stellar/core#1");

        assert_eq!(client.get_credential_count(&wallet_a), 1);
        assert_eq!(client.get_credential_count(&wallet_b), 0);
        assert!(!client.credential_exists(
            &wallet_b,
            &SignalType::GithubPr,
            &String::from_str(&env, "stellar/core#1"),
        ));
    }
}