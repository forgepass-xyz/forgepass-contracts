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

## Project Board
 
All active work is tracked on the
[ForgePass Development Board](https://github.com/orgs/forgepass-xyz/projects/2).
New issues are auto-added to the **Backlog** column. Opening a pull request
moves the linked issue to **In Progress**; marking the PR ready for review
moves it to **Review**; closing the issue moves it to **Done**.
 
When creating an issue, set the **Phase**, **Effort**, **Repo**, and **Epic**
custom fields on the board so the issue is correctly grouped in views and
reports.
 
## Cross-Repo Issue Linking Convention
 
ForgePass spans three repositories: `forgepass-contracts`, `forgepass-core`,
and (once activated) `forgepass-sdk`. Many issues depend on or are blocked by
issues in a different repository. Follow this convention so dependencies are
traceable across repos:
 
- Reference issues in another repo using the full form:
  `forgepass-xyz/forgepass-contracts#42` (not just `#42`, which only
  resolves within the current repo).
- In an issue description, list cross-repo dependencies under a
  **Depends on** heading using the full cross-repo reference form above.
- When a PR closes an issue that lives in a different repo, use a PR
  description comment referencing the full cross-repo issue rather than a
  GitHub closing keyword, since closing keywords (`Closes #N`) only work
  within the same repository.
- Issue numbers are never reused across repos. `forgepass-contracts` and
  `forgepass-core` issues share the same numbering sequence as defined in
  *ForgePass Issues v1.1* (`#001`-`#087`, plus roadmap issues `R01`-`R10`
  held in `forgepass-sdk` once activated).
## Branch Naming Convention
 
All branches follow the pattern:
 
```
phase-N/[issue-number]-[short-description]
```
 
Where `N` is the phase number (`0` through `5`) the issue belongs to, or
`future` for Epic 9 roadmap issues. Examples:
 
```
phase-0/015-project-board-setup
phase-1/016-passport-contract
phase-2/035-trust-score-engine
future/r01-sdk-scaffold
```
 
## SDK Repository Status
 
The `forgepass-sdk` repository is a roadmap item and is **not** connected to
the project board, the label set, or the milestone list described above. Its
activation is gated on issue **#010 (FR-10-A)**, which decides the target
development phase and resource allocation for the SDK.
 
Once `#010` is resolved:
 
- `forgepass-sdk/ROADMAP.md` will be updated to record the confirmed phase.
- `scripts/create-labels.sh` and `scripts/create-milestones.sh` (this repo)
  will be run against `forgepass-sdk` to bring it into line with the other
  active repos.
- The `forgepass-sdk` repo will be added to the project board.
- Epic 9 SDK issues (`R01`-`R04`) will be promoted to the confirmed phase.
Until then, no issues should be opened against `forgepass-sdk` and no PRs
should target it.

## Security

If you discover a security vulnerability, do not open a public issue. Email the team directly. See [contracts/SECURITY-REVIEW.md](./contracts/SECURITY-REVIEW.md) once it is available.

## Licence

By contributing, you agree that your contributions will be licensed under the MIT Licence.