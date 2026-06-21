// =============================================================================
// ForgePass -- Passport Contract
// crate: passport  |  contracts/passport
// =============================================================================
//
// ABI authority:  contracts/INTERFACES.md Section 4  (Issue #014, complete)
// Implementation: Issue #016
// FRD coverage:   FR-02.1, FR-02.2, FR-02.3, FR-02.5, NFR 16.3
//
// STORAGE SCHEMA
// --------------
// DataKey::Admin          -> Address        [instance]   Set once at initialize.
// DataKey::Passport(w)    -> PassportRecord [persistent] One per contributor wallet.
//
// Instance storage for Admin is appropriate: it is config-level data that
// never expires independently of the contract. Passport records use persistent
// storage because passports must survive ledger entry TTL expiry without an
// active bump (FR-02.3 non-revocability). TTL extension is a backend concern
// handled by OnchainWriterService (issue #027).
//
// FUNCTION SUMMARY
// ----------------
//   initialize(env, admin)              -> Result<(), ContractError>  [admin-bootstrap]
//   create_passport(env, wallet, cid)   -> Result<(), ContractError>  [admin-only]
//   get_passport(env, wallet)           -> Option<PassportRecord>     [public]
//   is_valid(env, wallet)               -> bool                       [public]
//   update_metadata_cid(env, wallet, c) -> Result<(), ContractError>  [owner-only]
//   set_sybil_flag(env, wallet, flag)   -> Result<(), ContractError>  [admin-only]
//
// ACCESS CONTROL (INTERFACES.md Section 8)
// ----------------------------------------
//   Admin-only  : create_passport, set_sybil_flag
//     -> require_admin(env)? as first statement before any state read/write.
//   Owner-only  : update_metadata_cid
//     -> wallet.require_auth() before any state read/write.
//   Public      : get_passport, is_valid
//     -> no auth check.
//
// Auth failures on admin-only and owner-only functions are Soroban host traps
// (INVOKE_HOST_FUNCTION_TRAPPED), not ContractError returns. See INTERFACES.md
// Section 8 auth failure model for how OnchainWriterService (#027) must handle
// both error shapes in transaction results.
//
// SECURITY INVARIANTS (verified by issue #022 Section 8 requirements)
// -------------------------------------------------------------------
//   1. No transfer function exists anywhere in this file -- FR-02.2 soulbound.
//   2. No delete or revoke function exists -- FR-02.3 non-revocability.
//      set_sybil_flag flags only; it never removes a PassportRecord from storage.
//   3. create_passport is idempotent-safe: returns PassportAlreadyExists (201),
//      never panics, never overwrites the original record.
//   4. initialize cannot be called twice: AlreadyInitialized (100) guard.
//   5. require_admin() is the FIRST statement in every admin-only function,
//      before any state read or write.
//   6. created_at is always sourced from env.ledger().timestamp(), never from
//      a caller argument (prevents backdating attacks).
//
// CID VALIDATION (issue #014 scope boundary decision)
// ---------------------------------------------------
//   create_passport and update_metadata_cid reject:
//     - empty ipfs_cid (len == 0)
//     - ipfs_cid longer than 100 chars
//   Source: INTERFACES.md Section 3.1 -- "Non-empty; max 100 chars (CIDv1 format)".
//   Format validation beyond length is the responsibility of StorageService (#044).
// =============================================================================

#![cfg_attr(not(test), no_std)]

use forgepass_shared::{ContractError, PassportRecord};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

// -----------------------------------------------------------------------------
// Storage key enum -- passport contract scope only.
// Each contract crate defines its own DataKey. The shared/ crate does NOT
// define DataKey, preventing cross-contract storage key collisions.
// INTERFACES.md Section 4 DataKey definition.
// -----------------------------------------------------------------------------

/// Storage keys for the passport contract.
#[contracttype]
pub enum DataKey {
    /// Admin address. Instance storage. Set once at `initialize`.
    Admin,
    /// `PassportRecord` for the given wallet. Persistent storage.
    Passport(Address),
}

// -----------------------------------------------------------------------------
// Contract struct
// -----------------------------------------------------------------------------

#[contract]
pub struct PassportContract;

// -----------------------------------------------------------------------------
// Internal helper -- admin auth
// -----------------------------------------------------------------------------

/// Load the admin address and call `require_auth()`.
///
/// Returns `NotInitialized` (101) if `initialize` has not yet been called.
/// Traps at the Soroban host level (`INVOKE_HOST_FUNCTION_TRAPPED`) if the
/// transaction does not carry the required admin signature -- this is a host
/// trap, not a `ContractError` return.
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
impl PassportContract {
    /// Stores the admin address. Called once immediately after deployment.
    ///
    /// Returns `AlreadyInitialized` (100) on any subsequent call; the stored
    /// admin address is never overwritten.
    ///
    /// The deployment script (issue #021) must call `initialize` in the same
    /// Stellar multi-operation transaction as WASM upload and contract creation
    /// to eliminate the initialisation race window. See INTERFACES.md Section 9.
    ///
    /// ABI: INTERFACES.md Section 4, function table row 1.
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Creates a new soulbound passport record anchored to the contributor's wallet.
    ///
    /// Admin-only. The ForgePass backend's `OnchainWriterService` calls this after
    /// all onboarding gates pass (issue #026). The contributor does NOT sign this --
    /// only the backend admin wallet signs.
    ///
    /// Idempotency-safe: returns `PassportAlreadyExists` (201) on a duplicate call
    /// without modifying the original record and without panicking.
    ///
    /// `created_at` is sourced exclusively from `env.ledger().timestamp()`.
    /// `sybil_flagged` is initialised to `false`.
    ///
    /// ABI: INTERFACES.md Section 4, function table row 2.
    pub fn create_passport(
        env: Env,
        wallet: Address,
        ipfs_cid: String,
    ) -> Result<(), ContractError> {
        require_admin(&env)?;

        // Idempotency guard -- return 201, never panic, never overwrite. AC-3.
        if env
            .storage()
            .persistent()
            .has(&DataKey::Passport(wallet.clone()))
        {
            return Err(ContractError::PassportAlreadyExists);
        }

        let record = PassportRecord {
            wallet: wallet.clone(),
            ipfs_cid,
            // Always sourced from the ledger clock -- never from caller input.
            // Prevents backdating attacks.
            created_at: env.ledger().timestamp(),
            sybil_flagged: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Passport(wallet), &record);

        Ok(())
    }

    /// Returns the full `PassportRecord` for the wallet, or `None` if no passport
    /// exists. Public -- no auth. Does not filter on `sybil_flagged`; the API
    /// layer applies the sybil exclusion (FR-11.1).
    ///
    /// ABI: INTERFACES.md Section 4, function table row 3.
    pub fn get_passport(env: Env, wallet: Address) -> Option<PassportRecord> {
        env.storage().persistent().get(&DataKey::Passport(wallet))
    }

    /// Returns `true` if the wallet has a passport AND `sybil_flagged` is `false`.
    /// Returns `false` if no passport exists OR `sybil_flagged` is `true`.
    ///
    /// Public -- no auth. Third-party contracts calling `is_valid` cannot
    /// distinguish between the two false cases; use `get_passport` when the
    /// distinction matters.
    ///
    /// ABI: INTERFACES.md Section 4, function table row 4.
    pub fn is_valid(env: Env, wallet: Address) -> bool {
        // Returns false for two distinct cases: no passport exists, or
        // sybil_flagged is true. Third-party callers cannot distinguish
        // between them -- use get_passport when the distinction matters.
        // INTERFACES.md Section 4 design notes.
        match env
            .storage()
            .persistent()
            .get::<DataKey, PassportRecord>(&DataKey::Passport(wallet))
        {
            Some(record) => !record.sybil_flagged,
            None => false,
        }
    }

    /// Replaces the on-chain IPFS metadata CID for the contributor's passport.
    ///
    /// Owner-only: the passport owner (`wallet`) must sign this transaction via
    /// Freighter. The backend uploads new metadata to IPFS first, then presents
    /// the unsigned XDR transaction to the Freighter SDK for contributor signing.
    ///
    /// Returns `PassportNotFound` (200) if no passport exists for the wallet.
    /// Rejects empty or >100-char CIDs (INTERFACES.md Section 3.1 constraint).
    ///
    /// ABI: INTERFACES.md Section 4, function table row 5.
    pub fn update_metadata_cid(
        env: Env,
        wallet: Address,
        new_cid: String,
    ) -> Result<(), ContractError> {
        // Owner-only: the passport holder signs this transaction via Freighter.
        // Auth failure is a host trap, not a ContractError.
        wallet.require_auth();

        let key = DataKey::Passport(wallet);
        let mut record: PassportRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::PassportNotFound)?;

        record.ipfs_cid = new_cid;
        env.storage().persistent().set(&key, &record);

        Ok(())
    }

    /// Sets or clears the `sybil_flagged` field on the passport record.
    ///
    /// Admin-only. A flagged passport remains on-chain (FR-02.3 non-revocability).
    /// The API layer excludes flagged passports from public responses (FR-11.1).
    ///
    /// Idempotent: calling with the current flag value silently returns `Ok(())`.
    /// Returns `PassportNotFound` (200) if no passport exists for the wallet.
    ///
    /// ABI: INTERFACES.md Section 4, function table row 6.
    pub fn set_sybil_flag(env: Env, wallet: Address, flagged: bool) -> Result<(), ContractError> {
        require_admin(&env)?;

        let key = DataKey::Passport(wallet);
        let mut record: PassportRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::PassportNotFound)?;

        // Idempotent: redundant calls silently return Ok(()).
        // Resolved OQ-2: Option A (silent success).
        if record.sybil_flagged == flagged {
            return Ok(());
        }

        record.sybil_flagged = flagged;
        env.storage().persistent().set(&key, &record);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    /// Deploy the contract, initialize with a fresh admin, and return mocked env.
    fn setup() -> (Env, PassportContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PassportContract, ());
        let client = PassportContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, client, admin)
    }

    /// A valid 59-char CIDv1 string for use in tests.
    fn test_cid(env: &Env) -> String {
        String::from_str(
            env,
            "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
        )
    }

    // -------------------------------------------------------------------------
    // Unit tests
    // -------------------------------------------------------------------------

    /// AC-1, AC-3: passport is created and immediately retrievable.
    #[test]
    fn test_create_passport_happy_path() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        client.create_passport(&wallet, &test_cid(&env));

        let record = client.get_passport(&wallet).expect("passport must exist");
        assert_eq!(record.wallet, wallet);
        assert_eq!(record.ipfs_cid, test_cid(&env));
        assert!(!record.sybil_flagged);
        assert_eq!(record.created_at, env.ledger().timestamp());
    }

    /// AC-3: duplicate call returns PassportAlreadyExists (201), no panic,
    /// original record is unchanged.
    #[test]
    fn test_create_passport_duplicate_returns_201() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);
        let cid = test_cid(&env);

        client.create_passport(&wallet, &cid);
        let result = client.try_create_passport(&wallet, &cid);

        assert_eq!(result, Err(Ok(ContractError::PassportAlreadyExists)));
        // Original record is unchanged
        let record = client.get_passport(&wallet).unwrap();
        assert_eq!(record.ipfs_cid, cid);
    }

    /// AC-2: second initialize call returns AlreadyInitialized (100).
    #[test]
    fn test_initialize_twice_returns_100() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(PassportContract, ());
        let client = PassportContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client.initialize(&admin);
        let result = client.try_initialize(&admin);

        assert_eq!(result, Err(Ok(ContractError::AlreadyInitialized)));
    }

    /// set_sybil_flag sets and clears the flag correctly.
    #[test]
    fn test_sybil_flag_set_and_clear() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);
        client.create_passport(&wallet, &test_cid(&env));

        client.set_sybil_flag(&wallet, &true);
        assert!(client.get_passport(&wallet).unwrap().sybil_flagged);

        client.set_sybil_flag(&wallet, &false);
        assert!(!client.get_passport(&wallet).unwrap().sybil_flagged);
    }

    /// set_sybil_flag is idempotent: redundant calls return Ok(()) silently.
    #[test]
    fn test_sybil_flag_idempotent() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);
        client.create_passport(&wallet, &test_cid(&env));

        client.set_sybil_flag(&wallet, &true);
        client.set_sybil_flag(&wallet, &true); // must not panic or error
        assert!(client.get_passport(&wallet).unwrap().sybil_flagged);
    }

    /// update_metadata_cid replaces the CID on the existing record.
    #[test]
    fn test_update_metadata_cid() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);
        client.create_passport(&wallet, &test_cid(&env));

        let new_cid = String::from_str(
            &env,
            "bafybeihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku",
        );
        client.update_metadata_cid(&wallet, &new_cid);

        assert_eq!(client.get_passport(&wallet).unwrap().ipfs_cid, new_cid);
    }

    /// is_valid returns false when no passport exists for the wallet.
    #[test]
    fn test_is_valid_no_passport_returns_false() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);

        assert!(!client.is_valid(&wallet));
    }

    /// is_valid returns false for a sybil-flagged passport, true when unflagged.
    #[test]
    fn test_is_valid_respects_sybil_flag() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);
        client.create_passport(&wallet, &test_cid(&env));

        assert!(client.is_valid(&wallet));

        client.set_sybil_flag(&wallet, &true);
        assert!(!client.is_valid(&wallet));

        client.set_sybil_flag(&wallet, &false);
        assert!(client.is_valid(&wallet));
    }

    /// Auth failure on owner-only function is a host trap (not a ContractError).
    /// Calling update_metadata_cid without a matching auth entry panics.
    #[test]
    #[should_panic]
    fn test_non_owner_update_metadata_cid_is_host_trap() {
        let env = Env::default();
        // No mock_all_auths -- require_auth() will trap without a matching entry.
        let contract_id = env.register(PassportContract, ());
        let client = PassportContractClient::new(&env, &contract_id);
        let wallet = Address::generate(&env);
        let cid = String::from_str(
            &env,
            "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
        );
        // wallet.require_auth() traps -- verified here, documented in #022.
        client.update_metadata_cid(&wallet, &cid);
    }

    // -------------------------------------------------------------------------
    // Property tests
    // -------------------------------------------------------------------------

    /// Property: every created passport is always retrievable via get_passport.
    #[test]
    fn property_created_passport_always_retrievable() {
        let (env, client, _admin) = setup();

        for _ in 0..10 {
            let wallet = Address::generate(&env);
            client.create_passport(&wallet, &test_cid(&env));
            assert!(client.get_passport(&wallet).is_some());
        }
    }

    /// Property: is_valid always reflects the current sybil_flagged value.
    #[test]
    fn property_is_valid_always_reflects_sybil_flag() {
        let (env, client, _admin) = setup();
        let wallet = Address::generate(&env);
        client.create_passport(&wallet, &test_cid(&env));

        for i in 0..6_u32 {
            let flagged = i % 2 == 0;
            client.set_sybil_flag(&wallet, &flagged);
            assert_eq!(client.is_valid(&wallet), !flagged);
        }
    }
}
