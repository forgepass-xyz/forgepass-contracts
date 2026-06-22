//! Local storage key definitions for the Soulbound NFT contract.
//!
//! `BadgeRecord`, `MilestoneType`, and `ContractError` are NOT defined here.
//! They live in `contracts/shared/src/lib.rs` and are imported below.
//! Per INTERFACES.md Section 2 ("Workspace structure"), the `shared/` crate
//! is the single source of truth for cross-contract types; each contract
//! crate defines only its own `DataKey` enum, to prevent storage key
//! collisions across contracts.

use soroban_sdk::{contracttype, Address};

// Re-exported for convenience so other modules in this crate can `use
// crate::types::{BadgeRecord, ContractError, MilestoneType}` instead of a
// separate `use forgepass_shared::...` line. The types themselves are
// defined in shared/, not here.
pub use forgepass_shared::{BadgeRecord, ContractError, MilestoneType};

/// Storage keys for the Soulbound NFT contract.
///
/// Source of truth: INTERFACES.md Section 7. Five keys, covering four
/// distinct query patterns (admin lookup, badge-by-id, badges-for-wallet,
/// has-badge) plus the global mint counter. No function bodies in this
/// file; functions are implemented in `lib.rs`.
#[contracttype]
pub enum DataKey {
    /// Address — instance storage. Set once at `initialize`, immutable
    /// after. Loaded by `require_admin()` before every admin-only call.
    Admin,

    /// BadgeRecord — persistent storage, keyed by `badge_id`. Written once
    /// at mint, never mutated, never deleted. Backing store for `get_badge`.
    Badge(u64),

    /// Vec<u64> — persistent storage, one entry per wallet. Holds every
    /// badge_id the wallet has ever been minted, in mint order. Backing
    /// store for `get_badges_for_wallet`. Appended to in `mint`; never
    /// pruned, soulbound badges are never removed.
    WalletBadges(Address),

    /// u64 — instance storage, global (not per-wallet). Monotonically
    /// increasing badge_id counter. Starts at 0; first minted badge_id is
    /// 1 (incremented before use, never after a failed validation,
    /// guaranteeing no gaps from rejected mints).
    BadgeCounter,

    /// bool — instance storage. Keyed by `(wallet, milestone_type)`. O(1)
    /// duplicate-prevention check used by both `has_badge` and the `mint`
    /// guard.
    ///
    /// Applies uniformly to all seven v1 MilestoneType variants, including
    /// HackathonParticipant: per the #018 decision, a wallet can hold at
    /// most one HackathonParticipant badge ever, regardless of how many
    /// hackathons it has participated in. This deliberately diverges from
    /// the "one_per_event" cardinality in milestone-registry.json.
    /// Rationale documented in ARCHITECTURE.md Section 7.
    HasBadge(Address, MilestoneType),
}
