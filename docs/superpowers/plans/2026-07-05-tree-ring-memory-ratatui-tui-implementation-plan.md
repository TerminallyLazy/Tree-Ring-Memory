# Tree Ring Memory Ratatui TUI Implementation Plan

## Goal

Implement the approved Rust-native Ratatui terminal interface behind `tree-ring tui`. The first implementation must be useful, testable, and faithful to the tree-ring memory lifecycle: ambient animated ASCII rings, real-time store refresh, event-stream pulses, search/detail views, and guided operator actions.

## Constraints

- Implement in Rust, inside the CLI crate/workspace.
- Do not implement the TUI in Python.
- Keep existing scriptable CLI commands working.
- Keep memory framework-agnostic.
- Do not make the TUI a raw transcript/log browser.
- Keep dangerous actions confirmation-gated.

## Tasks

### 1. Dependencies And CLI Entry

- Add Ratatui with the default crossterm backend to the CLI crate.
- Add a `tree-ring tui` subcommand.
- Add optional arguments:
  - `--event-stream <path>` for local JSONL event stream input.
  - `--tick-ms <number>` for animation/update cadence.
- Keep `--json` ignored or rejected for `tui`; the TUI is interactive.

### 2. TUI Module Skeleton

Create:

```text
crates/tree-ring-memory-cli/src/tui/
├── mod.rs
├── app.rs
├── actions.rs
├── event.rs
├── input.rs
├── model.rs
├── render.rs
├── rings.rs
├── store_watch.rs
└── stream.rs
```

### 3. State Model

- Derive ring stats from the Rust SQLite store.
- Track ring counts, event type counts, sensitive counts, superseded counts, average salience/confidence, newest/oldest timestamps, pulse level, and warning level.
- Track search query, recall results, selected result, selected ring, mode, pending action, and status message.

### 4. Store Watch

- Implement polling-based store refresh first.
- Refresh stats and current recall results on each configured tick.
- Detect ring count deltas and feed pulse events.
- Keep file watching replaceable later through a `StoreWatcher` boundary.

### 5. Event Stream

- Read optional JSONL events from `--event-stream`.
- Process appended lines incrementally.
- Treat events as untrusted display signals.
- Supported event payload fields: timestamp, event, ring, project, agent_profile, count_delta, label.
- Event-stream signals update pulse state and the status/event pane without bypassing persisted store truth.

### 6. Rendering

- Default mode:
  - Ambient ASCII ring core.
  - Ring stats/status HUD.
  - Search/results pane.
  - Selected memory detail.
  - Action rail/help footer.
- Exploded mode:
  - Separated rings with data bubbles.
  - Ring-specific counts and notes.
- Use retro color mapping with 16-color fallback.
- Keep small-terminal fallback readable.

### 7. Input And Commands

- Keyboard navigation: arrows/j/k, Tab, Enter, Escape, q.
- Slash command palette:
  - `/rings`
  - `/search`
  - `/remember`
  - `/forget`
  - `/redact`
  - `/promote`
  - `/scar`
  - `/seed`
  - `/supersede`
  - `/consolidate`
  - `/export`
  - `/sync`
  - `/stream`
  - `/watch`
- Start with guided action panels for remember, forget, redact, promote, scar, seed, supersede.
- Show not-yet-implemented but non-crashing messages for consolidate/export/sync if the underlying Rust APIs are not ready.

### 8. Safety Gates

- Require explicit confirmation for forget, redact, promote, scar, seed, supersede, export sensitive, and consolidate.
- Run sensitivity checks before remember/edit actions.
- Never render sensitive details unless the include-sensitive toggle is enabled.
- Do not render raw event-stream payloads; render policy-safe labels.

### 9. Tests

- Ring stats derive correct counts.
- Store refresh updates stats.
- Event stream updates pulse state.
- Slash commands switch modes.
- Dangerous action commands create pending confirmation instead of executing directly.
- Render snapshots or buffer assertions include the ambient ring, ring labels, and action rail.
- CLI parse test covers `tree-ring tui`.

### 10. Verification

Run:

```bash
cargo fmt
cargo test
python3 -m pytest
cargo build -p tree-ring-memory-python --features extension-module
python3 scripts/native_binding_smoke.py --install-maturin
python3 scripts/rust_performance_smoke.py --count 10000
```

## Definition Of Done

- `tree-ring tui` launches an interactive Ratatui application.
- Ambient ASCII tree rings are always visible.
- Ring lighting responds to store-watch and event-stream changes.
- `/rings` opens an exploded-ring view.
- Search/detail/filters are usable.
- Full operator actions have safe guided flows or clear non-destructive placeholders where backend APIs are intentionally deferred.
- Rust owns the TUI behavior.
- Tests and smoke checks pass.
