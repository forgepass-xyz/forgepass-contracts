//! Scenario 1 -- Full Passport-to-Badge Flow
//!
//! See contracts/tests/integration/SCENARIO-SPEC.md Scenario 1 for the full
//! call sequence, parameter values, and assertion rationale. This file
//! implements that spec literally -- any change to expected values must be
//! made in SCENARIO-SPEC.md first, not here.
//!
//! `badge.milestone_type` is not compared directly via `assert_eq!`:
//! `MilestoneType` derives only `Clone` in contracts/shared/src/lib.rs, not
//! `PartialEq`/`Debug`. Badge identity is instead confirmed via `badge_id`
//! and the `has_badge` boolean read, consistent with the workaround used
//! for `PassportRecord`/`ScoreSnapshot` in the harness smoke test.

use soroban_sdk::String as SorobanString;

use forgepass_shared::{MilestoneType, SignalType};
use integration::setup;

#[test]
fn scenario_1_full_passport_to_badge_flow() {
    let fixtures = setup();
    let env = &fixtures.env;

    // --- Step 1 -- create_passport ---
    let ipfs_cid = SorobanString::from_str(
        env,
        "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
    );
    fixtures
        .passport
        .create_passport(&fixtures.contributor, &ipfs_cid);

    // --- Step 2 -- add_credential ---
    let source_id = SorobanString::from_str(env, "stellar-org/rs-soroban-sdk#1234");
    let data_hash = SorobanString::from_str(env, &"a".repeat(64));
    let credential_id = fixtures.credentials.add_credential(
        &fixtures.contributor,
        &SignalType::GithubPr,
        &source_id,
        &1_700_000_000u64,
        &data_hash,
    );
    assert!(credential_id >= 1, "expected a non-zero credential id");

    // --- Step 3 -- anchor_score ---
    let algorithm_version = SorobanString::from_str(env, "1.0");
    let signal_hash = SorobanString::from_str(env, &"b".repeat(64));
    fixtures.score.anchor_score(
        &fixtures.contributor,
        &42u32,
        &algorithm_version,
        &signal_hash,
        &1_700_000_001u64,
    );

    // --- Step 4 -- mint ---
    let badge_cid = SorobanString::from_str(env, "bafybeifirstprbadge");
    let badge_id = fixtures.badges.mint(
        &fixtures.contributor,
        &MilestoneType::FirstPr,
        &badge_cid,
        &1_700_000_002u64,
    );
    assert!(badge_id >= 1, "expected a non-zero badge id");

    // --- A1 -- passport record correct ---
    let passport_record = fixtures
        .passport
        .get_passport(&fixtures.contributor)
        .expect("passport should exist after create_passport");
    assert_eq!(passport_record.wallet, fixtures.contributor);
    assert_eq!(passport_record.sybil_flagged, false);
    assert_eq!(passport_record.ipfs_cid, ipfs_cid);

    // --- A2 -- credential recorded correctly ---
    let credentials = fixtures.credentials.get_credentials(&fixtures.contributor);
    assert_eq!(credentials.len(), 1, "expected exactly one credential");
    let credential = credentials.get(0).expect("credential at index 0");
    assert_eq!(credential.signal_type, SignalType::GithubPr);
    assert_eq!(credential.source_id, source_id);
    assert_eq!(credential.data_hash, data_hash);

    // --- A3 -- credential count ---
    let count = fixtures
        .credentials
        .get_credential_count(&fixtures.contributor);
    assert_eq!(count, 1);

    // --- A4 -- current score ---
    let snapshot = fixtures
        .score
        .get_current_score(&fixtures.contributor)
        .expect("score should exist after anchor_score");
    assert_eq!(snapshot.score, 42);
    assert_eq!(snapshot.algorithm_version, algorithm_version);

    // --- A5 -- has_badge ---
    let has_badge = fixtures
        .badges
        .has_badge(&fixtures.contributor, &MilestoneType::FirstPr);
    assert!(has_badge, "expected FirstPr badge to be present");

    // --- A6 -- get_badges_for_wallet ---
    let badges = fixtures.badges.get_badges_for_wallet(&fixtures.contributor);
    assert_eq!(badges.len(), 1, "expected exactly one badge");
    let badge = badges.get(0).expect("badge at index 0");
    assert_eq!(
        badge.badge_id, badge_id,
        "badge_id should match mint return value"
    );
    assert_eq!(badge.wallet, fixtures.contributor);
}

/// Scenario 2 -- Sybil-Flagged Passport
///
/// Sybil flagging is a passport-contract-only state field. It does not
/// block writes on other contracts, and the contract returns full state
/// on read regardless of the flag value -- off-chain filtering (FR-11.1)
/// is the API layer's responsibility, not the contract's. See
/// SCENARIO-SPEC.md Scenario 2 for the full rationale.
#[test]
fn scenario_2_sybil_flagged_passport() {
    let fixtures = setup();
    let env = &fixtures.env;

    // --- Step 1 -- create_passport ---
    let ipfs_cid = SorobanString::from_str(env, "bafybeiscenario2");
    fixtures
        .passport
        .create_passport(&fixtures.contributor, &ipfs_cid);

    // --- Step 2 -- set_sybil_flag ---
    fixtures
        .passport
        .set_sybil_flag(&fixtures.contributor, &true);

    // --- Step 3 -- anchor_score ---
    // No explicit Result capture needed: the panicking client method call
    // itself is the assertion that this succeeds (A3). If sybil flagging
    // blocked anchor_score, this line would panic and fail the test.
    let algorithm_version = SorobanString::from_str(env, "1.0");
    let signal_hash = SorobanString::from_str(env, &"c".repeat(64));
    fixtures.score.anchor_score(
        &fixtures.contributor,
        &55u32,
        &algorithm_version,
        &signal_hash,
        &1_700_001_000u64,
    );

    // --- A1 -- is_valid returns false while sybil flagged ---
    let is_valid = fixtures.passport.is_valid(&fixtures.contributor);
    assert!(
        !is_valid,
        "expected is_valid to be false for a sybil-flagged passport"
    );

    // --- A2 -- get_passport still returns the full record ---
    let passport_record = fixtures
        .passport
        .get_passport(&fixtures.contributor)
        .expect("passport should still be readable after sybil flag is set");
    assert_eq!(passport_record.sybil_flagged, true);
    assert_eq!(passport_record.wallet, fixtures.contributor);

    // --- A4 -- score is readable after the flag was set ---
    let snapshot = fixtures
        .score
        .get_current_score(&fixtures.contributor)
        .expect("score should exist after anchor_score, despite sybil flag");
    assert_eq!(snapshot.score, 55);
}

/// Scenario 3 -- Credential Deduplication
///
/// `add_credential` must reject a duplicate `(signal_type, source_id)` pair
/// with `CredentialAlreadyExists` (300), and the rejection must not disturb
/// existing state -- even after an intervening `anchor_score` call on a
/// different contract. Dedup is keyed per pair, not per session. See
/// SCENARIO-SPEC.md Scenario 3.
///
/// Note the corrected error variant name: `CredentialAlreadyExists`, not
/// the stale roadmap name `DuplicateCredential` (see SCENARIO-SPEC.md
/// "Corrections from Roadmap v1.0").
#[test]
fn scenario_3_credential_deduplication() {
    use forgepass_shared::ContractError;

    let fixtures = setup();
    let env = &fixtures.env;

    // --- Step 1 -- create_passport ---
    let ipfs_cid = SorobanString::from_str(env, "bafybeiscenario3");
    fixtures
        .passport
        .create_passport(&fixtures.contributor, &ipfs_cid);

    // --- Step 2 -- add_credential (first) ---
    let source_id = SorobanString::from_str(
        env,
        "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM",
    );
    let data_hash = SorobanString::from_str(env, &"d".repeat(64));
    let first_result = fixtures.credentials.add_credential(
        &fixtures.contributor,
        &SignalType::SorobanContract,
        &source_id,
        &1_700_002_000u64,
        &data_hash,
    );
    assert!(
        first_result >= 1,
        "expected a non-zero credential id on first add"
    );

    // --- Step 3 -- anchor_score (intervening call on a different contract) ---
    let algorithm_version = SorobanString::from_str(env, "1.0");
    let signal_hash = SorobanString::from_str(env, &"e".repeat(64));
    fixtures.score.anchor_score(
        &fixtures.contributor,
        &30u32,
        &algorithm_version,
        &signal_hash,
        &1_700_002_001u64,
    );

    // --- Step 4 -- add_credential (duplicate) ---
    // Uses try_add_credential since this call is expected to return Err,
    // not panic.
    let duplicate_result = fixtures.credentials.try_add_credential(
        &fixtures.contributor,
        &SignalType::SorobanContract,
        &source_id,
        &1_700_002_000u64,
        &data_hash,
    );

    // --- A2 -- duplicate returns CredentialAlreadyExists (300) ---
    match duplicate_result {
        Ok(_) => panic!("expected CredentialAlreadyExists, got Ok"),
        Err(Ok(contract_err)) => {
            assert_eq!(
                contract_err,
                ContractError::CredentialAlreadyExists,
                "expected CredentialAlreadyExists (300)"
            );
        }
        Err(Err(invoke_err)) => {
            panic!(
                "expected a ContractError, got a host invocation error: {:?}",
                invoke_err
            );
        }
    }

    // --- A3 -- credential count unchanged ---
    let count = fixtures
        .credentials
        .get_credential_count(&fixtures.contributor);
    assert_eq!(
        count, 1,
        "duplicate add must not change the credential count"
    );

    // --- A4 -- no duplicate entry in the live set ---
    let credentials = fixtures.credentials.get_credentials(&fixtures.contributor);
    assert_eq!(
        credentials.len(),
        1,
        "duplicate add must not create a second entry"
    );
}

/// Scenario 4 -- Score History Accumulation
///
/// `anchor_score` must append to history in chronological order,
/// `get_score_history` must return entries ascending by `computed_at`, and
/// `get_current_score` must always reflect the most recent snapshot. See
/// SCENARIO-SPEC.md Scenario 4. Does not exercise the 50-snapshot cap --
/// that boundary is covered by #017's own unit tests.
#[test]
fn scenario_4_score_history_accumulation() {
    let fixtures = setup();
    let env = &fixtures.env;

    // --- Step 1 -- create_passport ---
    let ipfs_cid = SorobanString::from_str(env, "bafybeiscenario4");
    fixtures
        .passport
        .create_passport(&fixtures.contributor, &ipfs_cid);

    let algorithm_version = SorobanString::from_str(env, "1.0");

    // --- Step 2 -- anchor_score (first) ---
    let signal_hash_1 = SorobanString::from_str(env, &"f".repeat(64));
    fixtures.score.anchor_score(
        &fixtures.contributor,
        &40u32,
        &algorithm_version,
        &signal_hash_1,
        &1_700_003_000u64,
    );

    // --- Step 3 -- anchor_score (second) ---
    let signal_hash_2 = SorobanString::from_str(env, &"g".repeat(64));
    fixtures.score.anchor_score(
        &fixtures.contributor,
        &55u32,
        &algorithm_version,
        &signal_hash_2,
        &1_700_003_001u64,
    );

    // --- Step 4 -- anchor_score (third) ---
    let signal_hash_3 = SorobanString::from_str(env, &"h".repeat(64));
    fixtures.score.anchor_score(
        &fixtures.contributor,
        &68u32,
        &algorithm_version,
        &signal_hash_3,
        &1_700_003_002u64,
    );

    // --- A1 -- current score reflects the most recent snapshot ---
    let current = fixtures
        .score
        .get_current_score(&fixtures.contributor)
        .expect("score should exist after three anchor_score calls");
    assert_eq!(current.score, 68);

    // --- A2 -- history contains all three snapshots ---
    let history = fixtures.score.get_score_history(&fixtures.contributor);
    assert_eq!(history.len(), 3, "expected three accumulated snapshots");

    // --- A3 -- score ordering ascending ---
    let snapshot_0 = history.get(0).expect("history entry 0");
    let snapshot_1 = history.get(1).expect("history entry 1");
    let snapshot_2 = history.get(2).expect("history entry 2");
    assert_eq!(snapshot_0.score, 40);
    assert_eq!(snapshot_1.score, 55);
    assert_eq!(snapshot_2.score, 68);

    // --- A4 -- computed_at ordering ascending ---
    assert_eq!(snapshot_0.computed_at, 1_700_003_000);
    assert_eq!(snapshot_1.computed_at, 1_700_003_001);
    assert_eq!(snapshot_2.computed_at, 1_700_003_002);
}

/// Scenario 5 -- Badge Duplicate Prevention
///
/// `mint` must reject a second attempt for the same `(wallet,
/// MilestoneType)` pair with `BadgeAlreadyMinted` (500), and the rejected
/// call must not advance the global `badge_id` counter. See
/// SCENARIO-SPEC.md Scenario 5.
///
/// Note the corrected error variant name: `BadgeAlreadyMinted`, not the
/// stale roadmap name `AlreadyMinted`.
#[test]
fn scenario_5_badge_duplicate_prevention() {
    use forgepass_shared::ContractError;

    let fixtures = setup();
    let env = &fixtures.env;

    // --- Step 1 -- create_passport ---
    let ipfs_cid = SorobanString::from_str(env, "bafybeiscenario5");
    fixtures
        .passport
        .create_passport(&fixtures.contributor, &ipfs_cid);

    // --- Step 2 -- mint (first) ---
    let badge_cid = SorobanString::from_str(env, "bafybeifirstprbadge");
    let first_badge_id = fixtures.badges.mint(
        &fixtures.contributor,
        &MilestoneType::FirstPr,
        &badge_cid,
        &1_700_004_000u64,
    );
    assert!(
        first_badge_id >= 1,
        "expected a non-zero badge id on first mint"
    );

    // --- Step 3 -- mint (duplicate) ---
    // Uses try_mint since this call is expected to return Err, not panic.
    let duplicate_result = fixtures.badges.try_mint(
        &fixtures.contributor,
        &MilestoneType::FirstPr,
        &badge_cid,
        &1_700_004_000u64,
    );

    // --- A2 -- duplicate returns BadgeAlreadyMinted (500) ---
    match duplicate_result {
        Ok(_) => panic!("expected BadgeAlreadyMinted, got Ok"),
        Err(Ok(contract_err)) => {
            assert_eq!(
                contract_err,
                ContractError::BadgeAlreadyMinted,
                "expected BadgeAlreadyMinted (500)"
            );
        }
        Err(Err(invoke_err)) => {
            panic!(
                "expected a ContractError, got a host invocation error: {:?}",
                invoke_err
            );
        }
    }

    // --- A3 -- still exactly one badge ---
    let badges = fixtures.badges.get_badges_for_wallet(&fixtures.contributor);
    assert_eq!(
        badges.len(),
        1,
        "duplicate mint must not create a second badge"
    );

    // --- A4 -- has_badge still true ---
    let has_badge = fixtures
        .badges
        .has_badge(&fixtures.contributor, &MilestoneType::FirstPr);
    assert!(has_badge, "expected FirstPr badge to remain present");

    // --- A5 -- badge_id counter did not advance on the rejected mint ---
    // Mint a different milestone type after the rejected duplicate. If the
    // counter had incorrectly advanced on the rejected call, this would
    // return first_badge_id + 2 instead of first_badge_id + 1.
    let second_badge_id = fixtures.badges.mint(
        &fixtures.contributor,
        &MilestoneType::FirstContract,
        &badge_cid,
        &1_700_004_001u64,
    );
    assert_eq!(
        second_badge_id,
        first_badge_id + 1,
        "rejected duplicate mint must not consume a badge_id"
    );
}

/// Scenario 6 -- Partial Failure (On-Chain Rejection)
///
/// Proves a credential write that commits successfully is not rolled back
/// when a subsequent anchor_score call is rejected at the contract level.
/// Contracts commit independently -- there is no cross-contract atomicity
/// in Soroban. See SCENARIO-SPEC.md Scenario 6.
///
/// Failure mode (OQ-2, Option A -- on-chain rejection): anchor_score is
/// called with score = 101, exceeding the valid 0-100 range, triggering
/// ContractError::InvalidScore (400). This proves the contract boundary
/// holds regardless of what happens to adjacent calls -- it does not test
/// off-chain submission failure or retry-queue behaviour, which depends on
/// OnchainWriterService (#027) and is out of scope here.
#[test]
fn scenario_6_partial_failure_on_chain_rejection() {
    use forgepass_shared::ContractError;

    let fixtures = setup();
    let env = &fixtures.env;

    // --- Step 1 -- create_passport ---
    let ipfs_cid = SorobanString::from_str(env, "bafybeiscenario6");
    fixtures
        .passport
        .create_passport(&fixtures.contributor, &ipfs_cid);

    // --- Step 2 -- add_credential ---
    let source_id = SorobanString::from_str(env, "stellar-org/stellar-core#5678");
    let data_hash = SorobanString::from_str(env, &"i".repeat(64));
    let credential_id = fixtures.credentials.add_credential(
        &fixtures.contributor,
        &SignalType::GithubPr,
        &source_id,
        &1_700_005_000u64,
        &data_hash,
    );
    assert!(credential_id >= 1, "expected a non-zero credential id");

    // --- Step 3 -- capture before-state ---
    let before_credentials = fixtures.credentials.get_credentials(&fixtures.contributor);
    let before_count = fixtures
        .credentials
        .get_credential_count(&fixtures.contributor);
    assert_eq!(before_count, 1);
    assert_eq!(before_credentials.len(), 1);

    // --- Step 4 -- anchor_score (intentional failure, score = 101) ---
    // Uses try_anchor_score since this call is expected to return Err.
    let algorithm_version = SorobanString::from_str(env, "1.0");
    let signal_hash = SorobanString::from_str(env, &"j".repeat(64));
    let failure_result = fixtures.score.try_anchor_score(
        &fixtures.contributor,
        &101u32,
        &algorithm_version,
        &signal_hash,
        &1_700_005_001u64,
    );

    // --- A2 -- anchor_score rejected with InvalidScore (400) ---
    match failure_result {
        Ok(_) => panic!("expected InvalidScore, got Ok"),
        Err(Ok(contract_err)) => {
            assert_eq!(
                contract_err,
                ContractError::InvalidScore,
                "expected InvalidScore (400) for score = 101"
            );
        }
        Err(Err(invoke_err)) => {
            panic!(
                "expected a ContractError, got a host invocation error: {:?}",
                invoke_err
            );
        }
    }

    // --- Step 5 -- capture after-state ---
    let after_credentials = fixtures.credentials.get_credentials(&fixtures.contributor);
    let after_count = fixtures
        .credentials
        .get_credential_count(&fixtures.contributor);

    // --- A3 -- count unchanged by the failed anchor call ---
    assert_eq!(
        after_count, before_count,
        "credential count must survive the failed anchor_score call"
    );
    assert_eq!(after_count, 1);

    // --- A4 -- credential contents unchanged ---
    assert_eq!(after_credentials.len(), before_credentials.len());
    let before_cred = before_credentials
        .get(0)
        .expect("before credential at index 0");
    let after_cred = after_credentials
        .get(0)
        .expect("after credential at index 0");
    assert_eq!(before_cred.id, after_cred.id);
    assert_eq!(before_cred.source_id, after_cred.source_id);
    assert_eq!(before_cred.data_hash, after_cred.data_hash);
    assert_eq!(before_cred.signal_type, after_cred.signal_type);

    // --- A5 -- no partial score state was written ---
    let current_score = fixtures.score.get_current_score(&fixtures.contributor);
    assert!(
        current_score.is_none(),
        "rejected anchor_score must leave no partial score record"
    );
}
