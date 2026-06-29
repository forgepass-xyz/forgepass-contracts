//! Soulbound NFT contract — achievement badge minting and lookup.
//!
//! Implements the five-function interface specified in INTERFACES.md Section 7.
//! No transfer function exists anywhere in this crate, by design:
//! non-transferability is an interface-level guarantee, not a runtime check
//! on an existing function. See ARCHITECTURE.md Section 7 for the full
//! soulbound enforcement rationale.
//!
//! FRD coverage: FR-05.1, FR-05.2, FR-05.3, FR-05.4, FR-05.7.
//! Implementation issue: #018.

#![no_std]

#[cfg(test)]
mod test;
mod types;

use soroban_sdk::{contract, contractimpl, Address, Env, String, Vec};
use types::{BadgeRecord, ContractError, DataKey, MilestoneType};

#[contract]
pub struct SoulboundNftContract;

#[contractimpl]
impl SoulboundNftContract {
    /// Admin-bootstrap. Callable once by anyone before an admin is set.
    /// Every subsequent call returns AlreadyInitialized (100).
    /// Canonical pattern: INTERFACES.md Section 9.
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Admin-only. Mints a soulbound badge for `wallet` if it does not
    /// already hold one for `milestone_type`.
    ///
    /// Execution order is a correctness requirement: every validation step
    /// must complete before BadgeCounter is touched, so a rejected mint
    /// never consumes a badge_id and leaves no gap in the sequence.
    pub fn mint(
        env: Env,
        wallet: Address,
        milestone_type: MilestoneType,
        ipfs_cid: String,
        minted_at: u64,
    ) -> Result<u64, ContractError> {
        // 1-2. NotInitialized check + admin auth. Auth failure is a host
        // trap (INVOKE_HOST_FUNCTION_TRAPPED), not a returned ContractError.
        require_admin(&env)?;

        // 3. Duplicate check — must happen before the counter is touched.
        let has_key = DataKey::HasBadge(wallet.clone(), milestone_type.clone());
        let already_minted: bool = env.storage().instance().get(&has_key).unwrap_or(false);
        if already_minted {
            return Err(ContractError::BadgeAlreadyMinted);
        }

        // 4-5. Counter incremented only after all validation passes.
        // unwrap_or(0) means the first minted badge_id is 1.
        let counter: u64 = env
            .storage()
            .instance()
            .get(&DataKey::BadgeCounter)
            .unwrap_or(0);
        let badge_id = counter + 1;
        env.storage()
            .instance()
            .set(&DataKey::BadgeCounter, &badge_id);

        // 6-7. Construct and persist the badge record.
        let record = BadgeRecord {
            badge_id,
            wallet: wallet.clone(),
            milestone_type: milestone_type.clone(),
            ipfs_cid,
            minted_at,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Badge(badge_id), &record);

        // 8. Append badge_id to the wallet's index.
        let wallet_badges_key = DataKey::WalletBadges(wallet.clone());
        let mut wallet_badges: Vec<u64> = env
            .storage()
            .persistent()
            .get(&wallet_badges_key)
            .unwrap_or_else(|| Vec::new(&env));
        wallet_badges.push_back(badge_id);
        env.storage()
            .persistent()
            .set(&wallet_badges_key, &wallet_badges);

        // 9. Mark the duplicate-prevention flag.
        env.storage().instance().set(&has_key, &true);

        Ok(badge_id)
    }

    /// Public. Returns None for an unknown badge_id rather than panicking.
    pub fn get_badge(env: Env, badge_id: u64) -> Option<BadgeRecord> {
        env.storage().persistent().get(&DataKey::Badge(badge_id))
    }

    /// Public. Returns every badge the wallet holds, in mint order.
    /// Empty Vec for a wallet with no badges.
    pub fn get_badges_for_wallet(env: Env, wallet: Address) -> Vec<BadgeRecord> {
        let badge_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::WalletBadges(wallet))
            .unwrap_or_else(|| Vec::new(&env));

        let mut records: Vec<BadgeRecord> = Vec::new(&env);
        for badge_id in badge_ids.iter() {
            if let Some(record) = env.storage().persistent().get(&DataKey::Badge(badge_id)) {
                records.push_back(record);
            }
        }
        records
    }

    /// Public. O(1) duplicate-prevention lookup via HasBadge instance entry.
    /// False for any wallet that has never been minted this milestone type.
    pub fn has_badge(env: Env, wallet: Address, milestone_type: MilestoneType) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::HasBadge(wallet, milestone_type))
            .unwrap_or(false)
    }
}

/// Load and authenticate the admin address.
/// Returns NotInitialized (101) if `initialize` has not been called.
/// Panics at the Soroban host level if admin signature is absent.
/// See INTERFACES.md Section 8 for the full auth failure model.
fn require_admin(env: &Env) -> Result<(), ContractError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(ContractError::NotInitialized)?;
    admin.require_auth();
    Ok(())
}
