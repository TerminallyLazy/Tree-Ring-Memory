# Task 3 Report: Add TUI Evidence State And Confirmed Refresh Next Step

## Scope

Implemented Task 3 in:

- `crates/tree-ring-memory-cli/src/tui/input.rs`
- `crates/tree-ring-memory-cli/src/tui/actions.rs`
- `crates/tree-ring-memory-cli/src/tui/app.rs`

User-approved scope adjustment:

- Added a minimal compile-only branch in `crates/tree-ring-memory-cli/src/tui/render.rs` for `AppMode::Evidence`.
- Reason: adding `AppMode::Evidence` made `render.rs` fail exhaustive matching at compile time. This task still does **not** implement evidence list/detail rendering; it only labels the mode as `"evidence"` so the crate compiles and Task 4 can own actual rendering.

## What Changed

### `crates/tree-ring-memory-cli/src/tui/input.rs`

- Added `SlashCommand::Evidence(String)`.
- Extended slash parsing so both `/evidence` and `/proof <arg>` map to the evidence command.
- Updated `command_help()` to include `/evidence`.
- Added parser coverage for:
  - `/evidence`
  - `/proof refresh`

### `crates/tree-ring-memory-cli/src/tui/actions.rs`

- Added `ActionKind::RefreshCertification { command: String }`.
- Added `PendingAction::refresh_certification(command: &str)`.
- Kept the pending-action semantics explicit and confirmation-gated.
- Added a focused test asserting:
  - the confirmation prompt still says `press y`
  - the summary is `Refresh certification evidence`
  - the action kind stores the exact external command

### `crates/tree-ring-memory-cli/src/tui/app.rs`

- Imported evidence helpers:
  - `certification_dir_for_project`
  - `load_snapshot`
  - `EvidenceSnapshot`
- Added `AppMode::Evidence`.
- Added `App::evidence_snapshot: Option<EvidenceSnapshot>`.
- Wired `/evidence` to load current certification evidence into app state and switch mode.
- Wired `/evidence refresh` to create a pending confirmation action instead of running certification.
- Added confirmed refresh behavior to set status to:
  - `run externally: sh scripts/certify-tree-ring.sh`
- Added focused TUI state tests covering:
  - missing evidence snapshot state
  - confirmation-required refresh flow

### `crates/tree-ring-memory-cli/src/tui/render.rs`

- Added the minimal compile-only `AppMode::Evidence => "evidence"` header label branch.
- No evidence body rendering was added.

## TDD Evidence

### Red

Added the requested tests first, then ran the focused commands from the brief.

Observed failures before implementation:

- `SlashCommand::Evidence` missing
- `ActionKind::RefreshCertification` missing
- `PendingAction::refresh_certification(...)` missing
- `AppMode::Evidence` missing
- `App::evidence_snapshot` missing

After implementing `AppMode::Evidence`, compilation still failed because `render.rs` had an exhaustive `match app.mode` without an `Evidence` branch. I stopped at that point, requested scope clarification, and proceeded only after user approval for the minimal compile-only patch.

### Green

Focused tests passed after implementation:

```text
cargo test -p tree-ring-memory-cli parses_evidence_command_and_refresh_argument --locked
test result: ok. 1 passed; 0 failed

cargo test -p tree-ring-memory-cli evidence_refresh_is_explicit_pending_value --locked
test result: ok. 1 passed; 0 failed

cargo test -p tree-ring-memory-cli slash_evidence --locked
test result: ok. 2 passed; 0 failed
```

Additional sanity check:

```text
git diff --check
clean
```

## Files Changed

- `crates/tree-ring-memory-cli/src/tui/input.rs`
- `crates/tree-ring-memory-cli/src/tui/actions.rs`
- `crates/tree-ring-memory-cli/src/tui/app.rs`
- `crates/tree-ring-memory-cli/src/tui/render.rs`

## Self-Review

- Kept the implementation inside the requested Task 3 behavior: slash parsing, app state, pending-action semantics, and explicit confirmation behavior.
- Preserved the brief’s external-command contract for refresh instead of launching certification from the TUI.
- Limited the `render.rs` change to compile coverage only.
- Did not modify docs, certification scripts, or the evidence model.

## Concerns

- `AppMode::Evidence` currently only sets header mode/state and stores `evidence_snapshot`; actual evidence rendering still remains for Task 4.
- Focused tests passed, but there is no dedicated rendering assertion yet for evidence mode because rendering is intentionally deferred.
