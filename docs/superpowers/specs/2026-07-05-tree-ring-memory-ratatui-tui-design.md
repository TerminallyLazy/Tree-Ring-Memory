# Tree Ring Memory Ratatui TUI Design

## Summary

Tree Ring Memory needs a Rust-native terminal interface for humans and AI-agent operators. The interface should preserve the project purpose: a framework-agnostic memory lifecycle layer for agents, not a transcript browser. It should make useful memory visible by age, type, sensitivity, confidence, scars, seeds, and durable heartwood.

The selected design is a **Dual-Mode Ring Console** built with Ratatui in the Rust workspace. The default screen balances a live ambient ASCII tree-ring core with search, details, and safe action panels. A slash-command palette can expand the core into an animated exploded-ring view where each ring separates from the center and shows data bubbles.

## Product Constraints

- The TUI must be Rust-first. It belongs in the Rust CLI/workspace, not in Python.
- Python remains wrapper and compatibility surface only.
- The TUI must use local SQLite/FTS storage and Rust lifecycle APIs.
- The TUI must remain framework-agnostic. Agent harnesses can integrate through an event stream, but no harness is privileged.
- The TUI must not become a transcript browser or raw log viewer.
- DOX and Revolve remain inspirations and integration points, not systems replaced by the TUI.

## Launch Model

Primary launch:

```bash
tree-ring tui
```

The existing scriptable CLI commands remain available:

```bash
tree-ring remember ...
tree-ring recall ...
tree-ring forget ...
```

No-subcommand launch should remain conservative for now. The TUI is substantial enough that it should be explicit.

## Visual Language

The persistent visual anchor is an animated ASCII tree-ring core inspired by the retro ring logo. It should use terminal-safe retro colors:

- Cambium: bright amber/orange for fresh active work.
- Outer ring: warm coral/pink for recent structured memory.
- Inner ring: teal for compressed older memory.
- Heartwood: golden/yellow for durable truth.
- Scars: red/pink warning accents.
- Seeds: cyan/green-blue for future work and hypotheses.
- Sensitive/private state: dimmed violet or guarded outline, never flashy.

The ambient core should pulse slightly even when idle. Real-time changes brighten the affected ring briefly and then decay back to the baseline. The pulse is meaningful:

- New memory: target ring brightens.
- Recall hit: matching rings shimmer.
- Scar relevance: scar mark flashes with warning color.
- Heartwood promotion: center warms and stabilizes.
- Seed backlog growth: seed ring pulses cyan.
- Sensitive memory detected: guarded outline appears and decays slowly.

Animation must degrade cleanly in terminals without truecolor or sufficient size.

## Modes

### Default Console

The default mode includes:

- Left or top-left: ambient live ASCII tree-ring core.
- Status HUD: total memories, health label, last refresh, event-stream state.
- Search/results pane.
- Selected memory detail pane.
- Action rail with available operations.

The layout should use Ratatui layout primitives and stay readable in 100x30 terminals. Smaller terminals fall back to stacked panes.

### Exploded Rings

`/rings` opens the expanded visual mode. Rings separate from the core with animated offsets. Each ring has a data bubble:

- Ring name.
- Count.
- Top event types.
- Average confidence and salience.
- Oldest/newest timestamps.
- Sensitive/private count.
- Scar/seed/heartwood-specific notes.

This mode is visual-first but still keyboard-operable. Enter selects a ring; Escape returns to default.

### Search And Detail

`/search` focuses recall. Search results should support:

- Query input.
- Ring filters.
- Event type filters.
- Project and agent-profile filters.
- Include sensitive toggle.
- Include superseded toggle.
- Ranking explanation toggle.

Selected memory detail shows summary, details, source, links, tags, sensitivity, salience, confidence, supersession chain, and review flags.

### Action Flows

The TUI is a full operator console. It supports:

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

Hybrid interaction model:

- Navigation and focus are keyboard-first.
- Destructive, sensitive, or authority-changing actions use guided confirmation.
- Confirmation panels summarize exactly what will change.
- Secret-like input fails closed and shows a policy-safe error.

## Real-Time Inputs

The TUI combines two live sources.

### Store-Watch Real-Time

The app watches or polls the SQLite store for persisted memory changes. v1 may use low-latency polling if cross-platform file watching is noisy. The design should isolate this behind a `StoreWatcher` abstraction so native file notification can replace polling later.

Store-watch updates drive authoritative counts and search refreshes.

### Event-Stream Real-Time

The app exposes a framework-agnostic local event stream for AI-agent harnesses and companion tools. The stream is optional. Events are treated as live signals until persisted storage confirms them.

Initial event types:

- `remember_started`
- `remembered`
- `recall_started`
- `recalled`
- `forgotten`
- `redacted`
- `promoted`
- `marked_scar`
- `marked_seed`
- `consolidated`
- `sync_started`
- `sync_finished`
- `policy_blocked`

Each event should include timestamp, optional project, optional agent profile, ring, event type, count delta, and a policy-safe label. Event payloads must not require secrets or raw transcripts.

## Architecture

Recommended Rust structure:

```text
crates/tree-ring-memory-cli/src/
├── main.rs
├── tui/
│   ├── app.rs
│   ├── event.rs
│   ├── input.rs
│   ├── model.rs
│   ├── render.rs
│   ├── rings.rs
│   ├── actions.rs
│   ├── store_watch.rs
│   └── stream.rs
```

Responsibilities:

- `app.rs`: app state and mode transitions.
- `event.rs`: terminal ticks, key events, store updates, stream events.
- `input.rs`: slash-command palette and key handling.
- `model.rs`: derived dashboard state from store and events.
- `render.rs`: top-level Ratatui rendering.
- `rings.rs`: ASCII ring geometry, color mapping, pulse decay.
- `actions.rs`: remember/forget/redact/promote/scar/seed/consolidate flows.
- `store_watch.rs`: SQLite refresh and store-change detection.
- `stream.rs`: optional local event stream.

Ratatui should use its default crossterm backend first. Ratatui documentation currently shows `ratatui = "0.30.2"` and default crossterm support, with documented support for widgets, layouts, styling, popups, tables, charts, gauges, and event handling.

## State Model

The TUI should derive a compact state snapshot:

```text
RingStats {
  ring,
  total,
  event_type_counts,
  sensitive_count,
  superseded_count,
  average_salience,
  average_confidence,
  newest_at,
  oldest_at,
  pulse_level,
  warning_level
}
```

The render loop reads derived state, not raw database rows. Raw memory details are loaded only for visible selections.

## Safety

- Forget, redact, promote to heartwood, mark scar, mark seed, supersede, export sensitive, and consolidation require confirmation.
- Secret-like strings are blocked before storage.
- Sensitive memories are hidden from recall by default.
- Sensitive detail rendering is guarded by explicit toggle.
- Event-stream data is treated as untrusted and policy-safe.
- The TUI never stores raw chain-of-thought.

## Testing

Required tests:

- Ring stats derive correct counts from seeded memories.
- Store-watch refresh updates counts after insert/delete/redact.
- Event-stream updates pulse state before persisted refresh.
- Full operator actions call Rust storage APIs correctly.
- Forget/redact confirmations are required.
- Sensitive memories are hidden by default.
- Slash commands switch modes correctly.
- Rendering snapshot tests cover default and exploded-ring modes.
- Small terminal fallback remains readable.

## Acceptance Criteria

1. `tree-ring tui` launches a Ratatui app from the Rust CLI.
2. Ambient ASCII tree rings are always visible in the TUI.
3. Rings light in real time from both store-watch and event-stream updates.
4. `/rings` opens the exploded-ring view with ring data bubbles.
5. Search, detail, filters, and ranking explanation are available.
6. Remember, forget, redact, promote, scar, seed, supersede, consolidate, export, and sync flows exist with guided confirmations where required.
7. The TUI uses Rust storage, recall, sensitivity, and lifecycle APIs directly.
8. Python is not required for the TUI.
9. The design remains framework-agnostic and does not replace DOX or Revolve.
10. Automated tests cover model derivation, safety gates, event handling, and render snapshots.

## Open Implementation Notes

- Prefer `tree-ring tui` over a separate binary for v1.
- Keep the event-stream transport local and optional.
- Start with polling-based store-watch if it reduces cross-platform risk.
- Use truecolor when available and a 16-color fallback otherwise.
- Avoid over-animating: pulse should communicate state, not distract from operation.
