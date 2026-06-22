extern crate std;

use soroban_sdk::{testutils::Address as _, Address, Env, String};

use crate::types::{ContractError, MilestoneType};
use crate::{SoulboundNftContract, SoulboundNftContractClient};

fn setup<'a>() -> (Env, SoulboundNftContractClient<'a>, Address) {
    let env = Env::default();
    let contract_id = env.register(SoulboundNftContract, ());
    let client = SoulboundNftContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

// --- initialize ---

#[test]
fn initialize_rejects_a_second_call() {
    let (env, client, _) = setup();
    let result = client.try_initialize(&Address::generate(&env));
    assert_eq!(result, Err(Ok(ContractError::AlreadyInitialized)));
}

// --- mint ---

#[test]
fn mint_before_initialize_returns_not_initialized() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(SoulboundNftContract, ());
    let client = SoulboundNftContractClient::new(&env, &contract_id);
    let result = client.try_mint(
        &Address::generate(&env),
        &MilestoneType::FirstPr,
        &String::from_str(&env, "cid"),
        &1_700_000_000u64,
    );
    assert_eq!(result, Err(Ok(ContractError::NotInitialized)));
}

#[test]
fn mint_panics_without_admin_authorization() {
    let env = Env::default();
    // No mock_all_auths — admin auth will be absent.
    let contract_id = env.register(SoulboundNftContract, ());
    let client = SoulboundNftContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.mint(
            &Address::generate(&env),
            &MilestoneType::FirstPr,
            &String::from_str(&env, "cid"),
            &1_700_000_000u64,
        )
    }));
    assert!(result.is_err(), "mint must host-trap without admin auth");
}

#[test]
fn mint_assigns_sequential_badge_ids_starting_at_one() {
    let (env, client, _) = setup();
    env.mock_all_auths();
    let wallet = Address::generate(&env);
    let cid = String::from_str(&env, "cid");
    assert_eq!(
        client.mint(&wallet, &MilestoneType::FirstPr, &cid, &1_000u64),
        1
    );
    assert_eq!(
        client.mint(&wallet, &MilestoneType::FirstContract, &cid, &2_000u64),
        2
    );
}

#[test]
fn mint_rejects_duplicate_milestone_for_same_wallet() {
    let (env, client, _) = setup();
    env.mock_all_auths();
    let wallet = Address::generate(&env);
    let cid = String::from_str(&env, "cid");
    client.mint(&wallet, &MilestoneType::FirstPr, &cid, &1_000u64);
    let result = client.try_mint(&wallet, &MilestoneType::FirstPr, &cid, &2_000u64);
    assert_eq!(result, Err(Ok(ContractError::BadgeAlreadyMinted)));
}

#[test]
fn rejected_mint_does_not_consume_a_badge_id() {
    let (env, client, _) = setup();
    env.mock_all_auths();
    let wallet = Address::generate(&env);
    let cid = String::from_str(&env, "cid");
    assert_eq!(
        client.mint(&wallet, &MilestoneType::FirstPr, &cid, &1_000u64),
        1
    );
    let _ = client.try_mint(&wallet, &MilestoneType::FirstPr, &cid, &2_000u64);
    assert_eq!(
        client.mint(&wallet, &MilestoneType::FirstContract, &cid, &3_000u64),
        2
    );
}

#[test]
fn hackathon_participant_is_one_badge_per_wallet_ever() {
    // #018 decision: option 1. One HackathonParticipant badge per wallet
    // ever, not one per event. See ARCHITECTURE.md Section 7.2.
    let (env, client, _) = setup();
    env.mock_all_auths();
    let wallet = Address::generate(&env);
    let cid = String::from_str(&env, "cid");
    client.mint(
        &wallet,
        &MilestoneType::HackathonParticipant,
        &cid,
        &1_000u64,
    );
    let result = client.try_mint(
        &wallet,
        &MilestoneType::HackathonParticipant,
        &cid,
        &2_000u64,
    );
    assert_eq!(result, Err(Ok(ContractError::BadgeAlreadyMinted)));
}

// --- get_badge ---

#[test]
fn get_badge_returns_none_for_unknown_id() {
    let (_env, client, _) = setup();
    assert!(client.get_badge(&999).is_none());
}

#[test]
fn get_badge_returns_correct_record_after_mint() {
    let (env, client, _) = setup();
    env.mock_all_auths();
    let wallet = Address::generate(&env);
    let cid = String::from_str(&env, "cid");
    let minted_at = 1_700_000_000u64;
    let badge_id = client.mint(&wallet, &MilestoneType::FirstPr, &cid, &minted_at);
    let record = client.get_badge(&badge_id).expect("badge should exist");
    assert_eq!(record.badge_id, badge_id);
    assert_eq!(record.wallet, wallet);
    assert!(matches!(record.milestone_type, MilestoneType::FirstPr));
    assert_eq!(record.ipfs_cid, cid);
    assert_eq!(record.minted_at, minted_at);
}

// --- get_badges_for_wallet ---

#[test]
fn get_badges_for_wallet_is_empty_with_no_badges() {
    let (env, client, _) = setup();
    assert_eq!(
        client.get_badges_for_wallet(&Address::generate(&env)).len(),
        0
    );
}

#[test]
fn get_badges_for_wallet_returns_badges_in_mint_order() {
    let (env, client, _) = setup();
    env.mock_all_auths();
    let wallet = Address::generate(&env);
    let cid = String::from_str(&env, "cid");
    let id1 = client.mint(&wallet, &MilestoneType::FirstPr, &cid, &1_000u64);
    let id2 = client.mint(&wallet, &MilestoneType::FirstContract, &cid, &2_000u64);
    let badges = client.get_badges_for_wallet(&wallet);
    assert_eq!(badges.len(), 2);
    assert_eq!(badges.get(0).unwrap().badge_id, id1);
    assert_eq!(badges.get(1).unwrap().badge_id, id2);
}

#[test]
fn get_badges_for_wallet_does_not_mix_wallets() {
    let (env, client, _) = setup();
    env.mock_all_auths();
    let wallet_a = Address::generate(&env);
    let wallet_b = Address::generate(&env);
    let cid = String::from_str(&env, "cid");
    client.mint(&wallet_a, &MilestoneType::FirstPr, &cid, &1_000u64);
    assert_eq!(client.get_badges_for_wallet(&wallet_a).len(), 1);
    assert_eq!(client.get_badges_for_wallet(&wallet_b).len(), 0);
}

// --- has_badge ---

#[test]
fn has_badge_is_false_before_any_mint() {
    let (env, client, _) = setup();
    assert!(!client.has_badge(&Address::generate(&env), &MilestoneType::FirstPr));
}

#[test]
fn has_badge_is_true_only_for_the_minted_type() {
    let (env, client, _) = setup();
    env.mock_all_auths();
    let wallet = Address::generate(&env);
    let cid = String::from_str(&env, "cid");
    client.mint(&wallet, &MilestoneType::FirstPr, &cid, &1_000u64);
    assert!(client.has_badge(&wallet, &MilestoneType::FirstPr));
    assert!(!client.has_badge(&wallet, &MilestoneType::FirstContract));
}

// --- MilestoneType exhaustiveness guard (AC-2) ---

#[test]
fn milestone_type_variant_set_is_exhaustively_pinned() {
    // No wildcard arm: adding, removing, or renaming any MilestoneType
    // variant causes a compile error here, not a silent test pass.
    fn assert_exhaustive(m: MilestoneType) {
        match m {
            MilestoneType::FirstPr => {}
            MilestoneType::FirstContract => {}
            MilestoneType::HackathonParticipant => {}
            MilestoneType::RisingContributor => {}
            MilestoneType::MultiRepoContributor => {}
            MilestoneType::FirstSorobanInvocation => {}
            MilestoneType::FullStackBuilder => {}
            MilestoneType::FirstBounty => {}
            MilestoneType::FirstGrant => {}
            MilestoneType::FirstTrustlessWork => {}
        }
    }
    let _ = assert_exhaustive;
}
