# Rust Core Status

Tree Ring Memory has moved from a Python-owned reference implementation toward
a Rust-first core with Python compatibility. This page tracks the v0.2 Rust
core, v0.3 native Python binding work, and the Rust-native Ratatui terminal
console now being added to the CLI.

## Current Status

- Python remains the stable public API surface.
- Rust workspace exists under `crates/`.
- Rust core owns model validation, sensitivity checks, and recall scoring.
- Rust SQLite crate owns schema-compatible SQLite/FTS storage.
- Rust CLI can initialize, remember, recall, and forget local memory.
- Rust CLI has JSON output for machine-readable adapter use.
- Python has an explicit `RustCliTreeRingMemory` compatibility adapter.
- Python has an explicit `NativeTreeRingMemory` wrapper backed by the optional
  PyO3 module in `bindings/python`.
- The public `TreeRingMemory.open()` facade is Rust-first when the native module
  is installed. It falls back to the explicit `PythonTreeRingMemory` reference
  backend in source checkouts unless native is required by configuration.
- The v0.2 adapter is intentionally limited: `remember` supports summary,
  event type, ring, scope, project, and tags; `recall` supports query, project,
  limit, and sensitive-memory inclusion. Unsupported Python facade fields raise
  `NotImplementedError`.
- The v0.3 native backend supports the full public `remember()` and `recall()`
  contracts, including details, source metadata, agent profile, scores,
  retention, expiry, links, review metadata, supersession, recall filters,
  superseded-memory inclusion, and ranking explanations.
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
```

`NativeTreeRingMemory` requires the optional PyO3 extension module. Build it
with `cd bindings/python && pip install -e ../.. && maturin develop`. The
binding package is extension-only and does not package or own the public
`tree_ring_memory` Python package. If the module is absent, the wrapper raises a
clear `ImportError` with that build hint.

Backend controls:

- `TREE_RING_MEMORY_BACKEND=auto`: use Rust native when available, otherwise
  Python reference fallback.
- `TREE_RING_MEMORY_BACKEND=native` or `TREE_RING_MEMORY_REQUIRE_NATIVE=1`:
  require Rust native bindings.
- `TREE_RING_MEMORY_BACKEND=python`: force the reference backend.

## Smoke Coverage

- Rust unit tests cover model validation, sensitivity checks, recall scoring,
  SQLite/FTS storage, transactional row/FTS consistency, redaction, and basic
  concurrent writes. Rust binding tests cover native JSON remember/recall
  round-trip and forget validation.
- Rust CLI tests cover the scriptable init/remember/recall/forget commands and
  the Ratatui TUI model, stream reader, slash-command parser, store-watch
  refresh, confirmation-gated actions, CLI parsing, and render-buffer smoke.
- Python tests cover the existing reference backend, Rust CLI database
  compatibility, the opt-in `RustCliTreeRingMemory` adapter, default facade
  native selection, full native wrapper argument marshalling, and clean
  missing-extension behavior.
- `scripts/rust_performance_smoke.py` provides an operator-run local insert and
  recall timing check. It fails if expected recalls are empty, emits a stable
  `METRICS_JSON=` line, and enforces conservative synthetic-workload thresholds
  of at least 500 inserts/sec and max recall latency of 250 ms.

Latest local smoke on July 5, 2026 with `--count 10000`:

- Inserted 10,000 memories in 4,598.8 ms.
- Insert throughput: 2,174.5 inserts/sec.
- Recall average latency: 8.142 ms.
- Recall max latency: 15.017 ms.

## Compatibility Rule

Rust must read and write the same SQLite shape and JSON memory event payloads as
the Python reference. Python reference code is compatibility scaffolding, not
the target behavioral owner.
