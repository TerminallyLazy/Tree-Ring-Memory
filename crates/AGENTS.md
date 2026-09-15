# crates

## Purpose

Rust runtime workspace.

## Ownership

The three workspace crates; root Cargo.toml owns their shared version and dependencies.

## Local Contracts

Core is the domain layer, SQLite owns persistence, CLI composes them. Avoid independent implementations of the same action.

## Work Guidance

Follow the relevant crate contract and keep protocol changes compatible or explicitly versioned.

## Verification

`cargo test --workspace --locked`; `cargo fmt --all -- --check`.

## Child DOX Index

- [tree-ring-memory-core/AGENTS.md](tree-ring-memory-core/AGENTS.md) — Models, validation and pure domain behavior.
- [tree-ring-memory-sqlite/AGENTS.md](tree-ring-memory-sqlite/AGENTS.md) — SQLite storage, policy and retrieval.
- [tree-ring-memory-cli/AGENTS.md](tree-ring-memory-cli/AGENTS.md) — Commands, activation, onboarding and terminal UI.
