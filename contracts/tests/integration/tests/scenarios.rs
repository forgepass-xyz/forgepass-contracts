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
    assert_eq!(badge.badge_id, badge_id, "badge_id should match mint return value");
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
    assert!(!is_valid, "expected is_valid to be false for a sybil-flagged passport");

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
