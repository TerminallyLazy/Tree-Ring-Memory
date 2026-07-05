# Rust Core Status

Tree Ring Memory has moved from an early Python prototype to a Rust-owned
runtime. This page tracks the v0.2 Rust
core, v0.3 native Python binding work, the Rust-native Ratatui terminal
console, the v0.4 Rust-owned JSONL import/export path, and v0.5 deterministic
audit checks, the v0.6 deterministic consolidation path, the v0.7 Rust-owned
maintenance lifecycle, v0.8 Python-runtime removal, v0.9 removal of tracked
Python source from the canonical repo, and v0.10 installer/onboarding work.

## Current Status

- The public runtime is Rust-native through the Rust CLI and Rust crates.
- Rust workspace exists under `crates/`.
- Rust core owns model validation, sensitivity checks, and recall scoring.
- Rust SQLite crate owns schema-compatible SQLite/FTS storage.
- Rust CLI can initialize, remember, recall, and forget local memory.
- Rust CLI has JSON output for machine-readable adapter use.
- `bindings/python` remains an optional Rust-built CPython extension crate.
- The repository no longer tracks a root Python package, Python wrapper layer,
  pytest suite, or Python smoke scripts.
- The v0.3 native backend supports the full public `remember()` and `recall()`
  contracts, including details, source metadata, agent profile, scores,
  retention, expiry, links, review metadata, supersession, recall filters,
  superseded-memory inclusion, and ranking explanations.
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
  always-visible animated ASCII tree-ring view, SQLite store-watch refresh,
  optional JSONL event-stream pulses, search/detail panes, and confirmation
  gates for destructive or authority-changing actions.
- The repository includes `install.sh` for one-line global or project-local
  installs, plus `tree-ring welcome` for first-run terminal onboarding.

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
cargo build -p tree-ring-memory-python --features extension-module
cargo run --release -p tree-ring-memory-sqlite --example performance_smoke -- 1000
```

## Optional CPython Extension

`bindings/python` builds a CPython extension from Rust through PyO3. It is an
adapter artifact for host runtimes that need it, not a Python implementation and
not the canonical public runtime.

## Smoke Coverage

- Rust unit tests cover model validation, sensitivity checks, recall scoring,
  SQLite/FTS storage, transactional row/FTS consistency, redaction, JSONL
  import/export filtering and duplicate handling, deterministic audit checks,
  deterministic consolidation planning, maintenance planning/application, FTS
  repair, and basic concurrent writes. Rust PyO3 tests cover native JSON
  remember/recall round-trip, forget validation, JSONL import/export, audit,
  consolidation, and maintenance.
- Rust CLI tests cover the scriptable init/remember/recall/forget commands and
  JSONL import/export/audit/consolidate commands plus the Ratatui TUI model,
  stream reader, slash-command parser, store-watch refresh, confirmation-gated
  actions, CLI parsing, and render-buffer smoke.
- `crates/tree-ring-memory-sqlite/examples/performance_smoke.rs` provides an
  operator-run local insert and recall timing check. It fails if expected
  recalls are empty, emits a stable `METRICS_JSON=` line, and enforces
  conservative synthetic-workload thresholds of at least 500 inserts/sec and max
  recall latency of 250 ms.

Latest local smoke on July 5, 2026 with `--count 10000`:

- Inserted 10,000 memories in 5,316.4 ms.
- Insert throughput: 1,881.0 inserts/sec.
- Recall average latency: 4.298 ms.
- Recall max latency: 6.813 ms.

## Compatibility Rule

Rust owns the SQLite shape and JSON memory event payloads. Host bindings are
adapter artifacts, not behavioral owners.
