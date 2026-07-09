# Rust Core Status

Tree Ring Memory has moved from an early prototype to a Rust-owned runtime.
This page tracks the v0.2 Rust core, the Rust-native Ratatui terminal console,
the v0.4 Rust-owned JSONL import/export path, v0.5 deterministic audit checks,
v0.6 deterministic consolidation, v0.7 Rust-owned maintenance, v0.8
Python-runtime removal, v0.9 removal of tracked Python source and optional
CPython bindings from the canonical repo, v0.10 installer/onboarding work, and
v0.11 Rust-native source adapters plus framework discovery.

## Current Status

- The public runtime is Rust-native through the Rust CLI and Rust crates.
- Rust workspace exists under `crates/`.
- Rust core owns model validation, sensitivity checks, and recall scoring.
- Rust SQLite crate owns schema-compatible SQLite/FTS storage.
- Rust CLI owns the full local command surface: init, remember, evidence,
  recall, forget, import/export, audit, consolidate, maintain, DOX sync,
  Revolve sync, framework discovery, welcome onboarding, and TUI operation.
- Rust CLI has JSON output for machine-readable adapter use.
- CLI and TUI durable operations now share action request/report contracts for
  behavior-preserving command execution. This keeps CLI output ownership, TUI
  state/render ownership, and storage ownership separate while preparing the
  TUI cockpit and integration-link workflows.
- The repository no longer tracks a root Python package, Python wrapper layer,
  pytest suite, Python smoke scripts, PyO3 crate, or CPython extension.
- The v0.4 Rust core and SQLite store own portable JSONL import/export.
  Exports exclude sensitive and superseded memories by default; import validates
  events, supports dry-run previews, skips duplicate ids by default, and only
  replaces existing rows when explicitly requested.
- The Rust CLI exposes `tree-ring export` and `tree-ring import`.
- The v0.5 Rust core owns deterministic audit checks for stale expiry,
  sensitive retention, low-confidence durable memory, supersession integrity,
  and conservative contradiction candidates. SQLite and CLI surfaces expose
  matching non-mutating audit reports.
- The v0.6 Rust core owns deterministic consolidation planning. SQLite and CLI
  consolidation create source-linked summary memories, persist idempotent
  consolidation records, and avoid copying sensitive payload text into
  generated summaries.
- The v0.7 Rust core owns maintenance planning for expired memory, secret-like
  memory redaction, protected-memory review, invalid expiry review, and SQLite
  FTS drift reporting. SQLite and CLI can apply eligible expiry deletion,
  secret redaction, and FTS rebuild only through explicit apply/repair flags.
- The Rust CLI now includes `tree-ring tui`, a Ratatui operator console with an
  always-visible animated straight-on tree-ring face, SQLite store-watch
  refresh, optional JSONL event-stream pulses, search/detail panes, and
  confirmation gates for destructive or authority-changing actions. Ring visuals
  now start from backend-independent layer frames rasterized into high-resolution
  terminal cells with alternating clockwise and counter-clockwise highlights,
  activity pulses, and scar-shimmer animation. Store-watch and event-stream
  pulses feed that frame so the matching ambient ring lights and breathes in
  real time. The ambient HUD stays portable while richer terminal image protocols
  can be added for welcome or expanded views without replacing the live HUD
  renderer.
- The repository includes `install.sh` for one-line global or project-local
  installs, plus `tree-ring welcome` for first-run terminal onboarding.
- The Rust CLI includes `tree-ring dox sync` and `tree-ring revolve sync` as
  source adapters that produce concise, source-linked memory events without
  replacing DOX contracts or Revolve evidence records.
- The Rust CLI and TUI include read-only agent-framework discovery for DOX,
  Revolve, Codex, Claude Code, Agent Zero/A0, Goose, OpenCode, Hermes, and Pi.
  Integration scan output distinguishes project markers from user-home markers
  so local harness readiness is explicit.
- JSONL import uses batched SQLite writes while preserving dry-run validation,
  duplicate skipping, explicit replacement, secret blocking, and supersession
  application.
- `scripts/certify-tree-ring.sh` provides a repeatable local certification
  surface for formatting, tests, Clippy, release build, isolated installs, CLI
  smokes, DOX/Revolve smokes, integration marker origins, import throughput, and
  recall timing.
- Project-local agent guidance is generated under `.tree-ring/AGENTS.md`,
  `.tree-ring/SKILL.md`, and `.tree-ring/CLI.md`. The current bridge-linking
  design keeps those files canonical, prefers project-level harness bridges,
  leaves global harness configuration opt-in, and keeps durable memory updates
  agent-mediated instead of background-recorded.

## Build Commands

```bash
cargo test
sh install.sh --help
cargo run -p tree-ring-memory-cli -- --help
cargo run -p tree-ring-memory-cli -- welcome --no-animation
cargo run -p tree-ring-memory-cli -- tui --help
cargo run -p tree-ring-memory-cli -- export --help
cargo run -p tree-ring-memory-cli -- import --help
cargo run -p tree-ring-memory-cli -- audit --help
cargo run -p tree-ring-memory-cli -- consolidate --help
cargo run -p tree-ring-memory-cli -- maintain --help
cargo run -p tree-ring-memory-cli -- dox sync --help
cargo run -p tree-ring-memory-cli -- revolve sync --help
cargo run -p tree-ring-memory-cli -- integrations scan --help
cargo run --release -p tree-ring-memory-sqlite --example performance_smoke -- 1000
sh scripts/certify-tree-ring.sh
```

## Smoke Coverage

- Rust unit tests cover model validation, sensitivity checks, recall scoring,
  SQLite/FTS storage, transactional row/FTS consistency, redaction, JSONL
  import/export filtering and duplicate handling, deterministic audit checks,
  deterministic consolidation planning, maintenance planning/application, FTS
  repair, and basic concurrent writes.
- Rust CLI tests cover the scriptable init/remember/recall/forget commands and
  JSONL import/export/audit/consolidate commands plus the Ratatui TUI model,
  stream reader, slash-command parser, store-watch refresh, confirmation-gated
  actions, DOX/Revolve sync commands, framework discovery, CLI parsing, and
  render-buffer smoke.
- `crates/tree-ring-memory-sqlite/examples/performance_smoke.rs` provides an
  operator-run local insert and recall timing check. It fails if expected
  recalls are empty, emits a stable `METRICS_JSON=` line, and enforces
  conservative synthetic-workload thresholds of at least 500 inserts/sec and max
  recall latency of 250 ms.

Latest local certification run generated at `2026-07-09T04:14:34Z`:

- Release binary: 6,137,088 bytes.
- Project install with init: 6,064 KB.
- Global install: 6,020 KB.
- CLI import: 10,000 memories in 5 seconds, about 2,000/sec.
- 10k performance smoke: 2,182.3 inserts/sec, recall average 3.640 ms,
  recall max 6.556 ms.
- 30k performance smoke: 717.5 inserts/sec, recall average 8.004 ms, recall
  max 14.412 ms.
- Agent Zero plugin smoke: skipped because `TREE_RING_AGENT_ZERO_ROOT` was not
  set.
- Extended 50k smoke was skipped; enable it with `TREE_RING_CERT_EXTENDED=1`.

## Compatibility Rule

Rust owns the SQLite shape and JSON memory event payloads. Host bindings are
adapter artifacts, not behavioral owners.
