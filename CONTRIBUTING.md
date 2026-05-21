# Contributing to forgepass-contracts

Thank you for contributing to ForgePass. This document covers everything you need to know before submitting a PR.

## Before You Start

- Check the [issue tracker](https://github.com/forgepass-xyz/forgepass-contracts/issues) for open issues
- If you are fixing a bug or adding a feature, make sure an issue exists first
- Comment on the issue to let others know you are working on it

## Branch Naming

Branches must follow this format:

    feat/issue-NNN-short-description
    fix/issue-NNN-short-description
    chore/issue-NNN-short-description
    docs/issue-NNN-short-description

Example: `feat/issue-016-passport-contract`

## Commit Message Format

We use [Conventional Commits](https://www.conventionalcommits.org/):

    type(scope): short description

    longer explanation if needed

    closes #NNN

Types: `feat`, `fix`, `chore`, `docs`, `test`, `ci`, `refactor`

Example:

    feat(passport): implement create_passport contract function

    - stores PassportRecord in persistent Soroban storage
    - admin-only access control via stored admin address
    - returns ContractError::AlreadyExists on duplicate

    closes #016

## CI Requirements

All four CI checks must pass before a PR can be merged. These are enforced automatically — there are no exceptions.

| Check | Command | What it enforces |
|---|---|---|
| Build | `cargo build --target wasm32-unknown-unknown --release` | All four crates compile to WASM |
| Test | `cargo test --all --target x86_64-unknown-linux-gnu` | All tests pass on native host |
| Clippy | `cargo clippy -- -D warnings` | Zero clippy warnings — warnings are errors |
| Format | `cargo fmt --all --check` | Code is formatted — run `cargo fmt --all` to fix |

Run all four locally before pushing:

    cargo build --target wasm32-unknown-unknown --release
    cargo test --all --target x86_64-unknown-linux-gnu
    cargo clippy -- -D warnings
    cargo fmt --all --check

## Pull Request Requirements

- PR must reference the issue it closes: `Closes #NNN`
- PR must have at least one approving review before merge
- All four CI checks must be green
- If you changed contract interfaces, update `contracts/ARCHITECTURE.md`
- If you added a new contract function, add tests for it

## Code Review

- Reviews are expected within 2 business days
- Address all review comments before requesting re-review
- Squash fixup commits before merge

## Security

If you discover a security vulnerability, do not open a public issue. Email the team directly. See [contracts/SECURITY-REVIEW.md](./contracts/SECURITY-REVIEW.md) once it is available.

## Licence

By contributing, you agree that your contributions will be licensed under the MIT Licence.