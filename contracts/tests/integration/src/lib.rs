//! ForgePass cross-contract integration test harness.
//!
//! Deploys all four Soroban contracts (passport, credential-store,
//! trust-score, soulbound-nft) into a single shared `Env`, initialises
//! them with one admin address, and exposes the resulting clients as
//! reusable fixtures for Scenarios 1-6 (see
//! contracts/tests/integration/SCENARIO-SPEC.md).
//!
//! NOTE: this harness assumes each contract crate exports its `#[contract]`
//! struct under the name `<Purpose>Contract` (e.g. `PassportContract`),
//! generating a `<Purpose>ContractClient` via `#[contractimpl]`. Confirm
//! these names against each crate's `src/lib.rs` before the first run; if
//! the actual struct names differ, update the imports below accordingly.

#![cfg(test)]

use soroban_sdk::{Address, Env};

use credential_store::{CredentialStoreContract, CredentialStoreContractClient};
use passport::{PassportContract, PassportContractClient};
use soulbound_nft::{MilestoneType, SoulboundNftContract, SoulboundNftContractClient};
use trust_score::{TrustScoreContract, TrustScoreContractClient};

/// Shared fixture bundle for all integration scenarios.
///
/// Constructed fresh per test via `setup()`. Soroban test `Env` instances
/// are cheap to build, so no scenario should reuse a `TestContracts`
/// instance from another scenario -- each test gets its own isolated state.
pub struct TestContracts<'a> {
    pub env: Env,
    pub admin: Address,
    pub contributor: Address,
    pub non_admin: Address,
    pub passport: PassportContractClient<'a>,
    pub credentials: CredentialStoreContractClient<'a>,
    pub score: TrustScoreContractClient<'a>,
    pub badges: SoulboundNftContractClient<'a>,
}

/// Deploys and initialises all four contracts in one shared `Env`.
///
/// Mirrors the canonical deployment order from INTERFACES.md Section 9:
/// passport, credential-store, trust-score, soulbound-nft. All four are
/// initialised with the same `admin` address in this call, using
/// `env.mock_all_auths()` to satisfy each `initialize` admin-bootstrap
/// check.
///
/// Negative-path tests that need to observe a host trap on auth failure
/// must NOT call `mock_all_auths()` again after this point in the same
/// `Env` -- construct a fresh `TestContracts` instead and only mock the
/// specific calls that are expected to succeed.
pub fn setup<'a>() -> TestContracts<'a> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contributor = Address::generate(&env);
    let non_admin = Address::generate(&env);

    let passport_id = env.register(PassportContract, ());
    let credentials_id = env.register(CredentialStoreContract, ());
    let score_id = env.register(TrustScoreContract, ());
    let badges_id = env.register(SoulboundNftContract, ());

    let passport = PassportContractClient::new(&env, &passport_id);
    let credentials = CredentialStoreContractClient::new(&env, &credentials_id);
    let score = TrustScoreContractClient::new(&env, &score_id);
    let badges = SoulboundNftContractClient::new(&env, &badges_id);

    passport.initialize(&admin);
    credentials.initialize(&admin);
    score.initialize(&admin);
    badges.initialize(&admin);

    TestContracts {
        env,
        admin,
        contributor,
        non_admin,
        passport,
        credentials,
        score,
        badges,
    }
}

/// Step 1 exit condition: a trivial read call against each of the four
/// contracts succeeds with no panic, confirming all four are live and
/// correctly initialised. Mirrors INTERFACES.md Section 9 "Post-deployment
/// smoke test sequence" Tests 1-4 (Test 5 and Test 6, the auth-trap and
/// AlreadyInitialized guards, are covered separately in #022's security
/// review harness, not here).
#[test]
fn smoke_all_four_contracts_are_live() {
    let fixtures = setup();

    // Test 1 -- passport contract is live.
    let passport_result = fixtures.passport.get_passport(&fixtures.contributor);
    assert_eq!(passport_result, None, "expected no passport for an unused wallet");

    // Test 2 -- credential-store contract is live.
    let count = fixtures.credentials.get_credential_count(&fixtures.contributor);
    assert_eq!(count, 0, "expected zero credentials for an unused wallet");

    // Test 3 -- trust-score contract is live.
    let current_score = fixtures.score.get_current_score(&fixtures.contributor);
    assert_eq!(current_score, None, "expected no score for an unused wallet");

    // Test 4 -- soulbound-nft contract is live.
    let has_badge = fixtures
        .badges
        .has_badge(&fixtures.contributor, &MilestoneType::FirstPr);
    assert!(!has_badge, "expected no badge for an unused wallet");
}
