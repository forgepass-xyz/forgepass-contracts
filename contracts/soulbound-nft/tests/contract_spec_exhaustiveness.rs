//! Integration test: proves the compiled contract exposes exactly the five
//! documented public functions and no transfer function exists.
//!
//! Reads the wasm's `contractspecv0` custom section via soroban-spec.
//! Must be run after a wasm build:
//!   cargo build --target wasm32-unknown-unknown --release -p soulbound-nft
//!   cargo test --test contract_spec_exhaustiveness -p soulbound-nft

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use stellar_xdr::curr::ScSpecEntry;

const EXPECTED_FUNCTIONS: &[&str] = &[
    "initialize",
    "mint",
    "get_badge",
    "get_badges_for_wallet",
    "has_badge",
];

#[test]
fn contract_exposes_exactly_the_five_documented_functions() {
    let wasm_path = built_wasm_path();
    let wasm_bytes = fs::read(&wasm_path).unwrap_or_else(|e| {
        panic!(
            "could not read {wasm_path:?}: {e}\n\n\
             build the contract first:\n\
             cargo build --target wasm32-unknown-unknown --release -p soulbound-nft"
        )
    });

    let entries = soroban_spec::read::from_wasm(&wasm_bytes)
        .expect("failed to parse contractspecv0 section from wasm");

    let actual: BTreeSet<std::string::String> = entries
        .into_iter()
        .filter_map(|e| match e {
            ScSpecEntry::FunctionV0(f) => Some(f.name.to_string()),
            _ => None,
        })
        .collect();

    let expected: BTreeSet<std::string::String> =
        EXPECTED_FUNCTIONS.iter().map(|s| s.to_string()).collect();

    assert_eq!(
        actual, expected,
        "\n\nthe compiled contract's public function set has changed.\n\
         expected: {EXPECTED_FUNCTIONS:?}\n\n\
         A transfer function or any undocumented function is a soulbound-\
         guarantee regression. Update INTERFACES.md, ARCHITECTURE.md Section 7,\
         and this test together with explicit sign-off before merging."
    );
}

/// CARGO_MANIFEST_DIR is contracts/soulbound-nft/.
/// One level up is contracts/.
/// Two levels up is the workspace root (forgepass-contracts/), where target/ lives.
fn built_wasm_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("soulbound_nft.wasm")
}
