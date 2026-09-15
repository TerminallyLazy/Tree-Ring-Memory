# tree-ring-memory-cli

## Purpose

Public commands and host-facing orchestration.

## Ownership

main.rs parsing, shared actions, onboarding/update, evidence runners and tui; activation has its own child contract.

## Local Contracts

CLI, TUI and onboarding share action implementations. init and welcome --init preserve existing project material. Updates retain the active installation scope and verify release assets.

## Work Guidance

Use real command acceptance tests for public argument/path behavior; keep JSON machine-readable and diagnostics truthful.

## Verification

`cargo test -p tree-ring-memory-cli --locked`; run the appropriate acceptance target for command changes.

## Child DOX Index

- [src/activation/AGENTS.md](src/activation/AGENTS.md) — Bridge ownership, host events and activation receipts.
