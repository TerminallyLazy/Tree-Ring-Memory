# Rust Core Status

Tree Ring Memory has moved from a Python-owned reference implementation toward
a Rust-first core with Python compatibility. This page tracks the v0.2 Rust
core, v0.3 native Python binding work, the Rust-native Ratatui terminal
console, the v0.4 Rust-owned JSONL import/export path, and v0.5 deterministic
audit checks, the v0.6 deterministic consolidation path, and the v0.7
Rust-owned maintenance lifecycle.

## Current Status

- The public runtime is Rust-native through the Rust CLI and optional PyO3
  native binding.
- Rust workspace exists under `crates/`.
- Rust core owns model validation, sensitivity checks, and recall scoring.
- Rust SQLite crate owns schema-compatible SQLite/FTS storage.
- Rust CLI can initialize, remember, recall, and forget local memory.
- Rust CLI has JSON output for machine-readable adapter use.
- Python has an explicit `RustCliTreeRingMemory` compatibility adapter.
- Python has an explicit `NativeTreeRingMemory` wrapper backed by the optional
  PyO3 module in `bindings/python`.
- The public `TreeRingMemory.open()` facade requires the native Rust module and
  no longer silently falls back to `PythonTreeRingMemory`.
- The v0.2 adapter is intentionally limited: `remember` supports summary,
  event type, ring, scope, project, and tags; `recall` supports query, project,
  limit, and sensitive-memory inclusion. Unsupported Python facade fields raise
  `NotImplementedError`.
- The v0.3 native backend supports the full public `remember()` and `recall()`
  contracts, including details, source metadata, agent profile, scores,
  retention, expiry, links, review metadata, supersession, recall filters,
  superseded-memory inclusion, and ranking explanations.
- The v0.4 Rust core and SQLite store own portable JSONL import/export.
  Exports exclude sensitive and superseded memories by default; import validates
  events, supports dry-run previews, skips duplicate ids by default, and only
  replaces existing rows when explicitly requested.
- The Rust CLI exposes `tree-ring export` and `tree-ring import`; the optional
  native Python backend and Python reference backend expose matching
  `export_jsonl()` and `import_jsonl()` methods.
- The v0.5 Rust core owns deterministic audit checks for stale expiry,
  sensitive retention, low-confidence durable memory, supersession integrity,
  and conservative contradiction candidates. SQLite, CLI, native Python, and
  Python reference surfaces expose matching non-mutating audit reports.
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

## Build Commands

```bash
cargo test
python3 -m pytest
cargo run -p tree-ring-memory-cli -- --help
cargo run -p tree-ring-memory-cli -- tui --help
cargo run -p tree-ring-memory-cli -- export --help
cargo run -p tree-ring-memory-cli -- import --help
cargo run -p tree-ring-memory-cli -- audit --help
cargo run -p tree-ring-memory-cli -- consolidate --help
cargo run -p tree-ring-memory-cli -- maintain --help
python3 scripts/rust_performance_smoke.py --count 1000
cargo build -p tree-ring-memory-python --features extension-module
python3 scripts/native_binding_smoke.py --install-maturin
```

## Python Rust Bridge

```python
from tree_ring_memory import RustCliTreeRingMemory

memory = RustCliTreeRingMemory.open(".tree-ring")
event = memory.remember(summary="Rust-backed memory works.", event_type="lesson")
results = memory.recall("Rust-backed memory")
```

The bridge is opt-in and currently shells out to the native Rust CLI. It proves
database/schema compatibility and Python return-object compatibility for the
supported v0.2 subset while keeping the stable Python reference implementation
unchanged.

## Python Native Backend

```python
from tree_ring_memory import TreeRingMemory

memory = TreeRingMemory.open(".tree-ring")
event = memory.remember(summary="Native Rust path works.", event_type="lesson")
results = memory.recall("Rust path")
jsonl = memory.export_jsonl()
preview = memory.import_jsonl(jsonl, dry_run=True)
audit_report = memory.audit()
consolidation_report = memory.consolidate(period_type="manual", dry_run=True)
maintenance_report = memory.maintain()
```

`NativeTreeRingMemory` requires the optional PyO3 extension module. Build it
with `cd bindings/python && pip install -e ../.. && maturin develop`. The
binding package is extension-only and does not package or own the public
`tree_ring_memory` Python package. If the module is absent, the wrapper raises a
clear `ImportError` with that build hint.

Backend controls:

- `TREE_RING_MEMORY_BACKEND=auto`, `native`, `rust`, and `rust-native`: use the
  native Rust binding and fail if it is missing.
- `TREE_RING_MEMORY_BACKEND=python`: rejected by `TreeRingMemory.open()`; use
  `PythonTreeRingMemory.open()` explicitly for reference-backend parity work.

## Smoke Coverage

- Rust unit tests cover model validation, sensitivity checks, recall scoring,
  SQLite/FTS storage, transactional row/FTS consistency, redaction, JSONL
  import/export filtering and duplicate handling, deterministic audit checks,
  deterministic consolidation planning, maintenance planning/application, FTS
  repair, and basic concurrent writes. Rust binding tests cover native JSON
  remember/recall round-trip, forget validation, JSONL import/export, audit,
  consolidation, and maintenance.
- Rust CLI tests cover the scriptable init/remember/recall/forget commands and
  JSONL import/export/audit/consolidate commands plus the Ratatui TUI model,
  stream reader, slash-command parser, store-watch refresh, confirmation-gated
  actions, CLI parsing, and render-buffer smoke.
- Python tests cover the existing reference backend, Rust CLI database
  compatibility, the opt-in `RustCliTreeRingMemory` adapter, default facade
  native selection without Python fallback, full native wrapper argument
  marshalling, and clean missing-extension behavior. They also cover Python
  reference and native wrapper JSONL import/export parity and audit parity.
- `scripts/rust_performance_smoke.py` provides an operator-run local insert and
  recall timing check. It fails if expected recalls are empty, emits a stable
  `METRICS_JSON=` line, and enforces conservative synthetic-workload thresholds
  of at least 500 inserts/sec and max recall latency of 250 ms.

Latest local smoke on July 5, 2026 with `--count 10000`:

- Inserted 10,000 memories in 4,423.3 ms.
- Insert throughput: 2,260.7 inserts/sec.
- Recall average latency: 4.267 ms.
- Recall max latency: 6.095 ms.

## Compatibility Rule

Rust must read and write the same SQLite shape and JSON memory event payloads as
the Python reference. Python reference code is compatibility scaffolding, not
the target behavioral owner.
