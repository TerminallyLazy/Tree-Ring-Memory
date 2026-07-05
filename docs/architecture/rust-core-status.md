# Rust Core Status

Tree Ring Memory is moving from a Python reference package toward a Rust-first core with Python compatibility. This page tracks the v0.2 Rust core and v0.3 native Python binding work.

## Current Status

- Python remains the stable public reference implementation.
- Rust workspace exists under `crates/`.
- Rust core owns model validation, sensitivity checks, and recall scoring.
- Rust SQLite crate owns schema-compatible SQLite/FTS storage.
- Rust CLI can initialize, remember, recall, and forget local memory.
- Rust CLI has JSON output for machine-readable adapter use.
- Python has an explicit `RustCliTreeRingMemory` compatibility adapter.
- Python also has an explicit `NativeTreeRingMemory` preview wrapper backed by
  the optional PyO3 module in `bindings/python`.
- The v0.2 adapter is intentionally limited: `remember` supports summary,
  event type, ring, scope, project, and tags; `recall` supports query, project,
  limit, and sensitive-memory inclusion. Unsupported Python facade fields raise
  `NotImplementedError`.
- The default `TreeRingMemory` facade remains Python-backed until native parity
  is broader.

## Build Commands

```bash
cargo test
python3 -m pytest
cargo run -p tree-ring-memory-cli -- --help
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

## Python Native Binding Preview

```python
from tree_ring_memory import NativeTreeRingMemory

memory = NativeTreeRingMemory.open(".tree-ring")
event = memory.remember(summary="Native Rust path works.", event_type="lesson")
results = memory.recall("Rust path")
```

`NativeTreeRingMemory` requires the optional PyO3 extension module. Build it
with `cd bindings/python && pip install -e ../.. && maturin develop`. The
binding package is extension-only and does not package or own the public
`tree_ring_memory` Python package. If the module is absent, the wrapper raises a
clear `ImportError` with that build hint.

## Smoke Coverage

- Rust unit tests cover model validation, sensitivity checks, recall scoring,
  SQLite/FTS storage, transactional row/FTS consistency, redaction, and basic
  concurrent writes. Rust binding tests cover native JSON remember/recall
  round-trip and forget validation.
- Python tests cover the existing reference package, Rust CLI database
  compatibility, the opt-in `RustCliTreeRingMemory` adapter, and the clean
  missing-extension path for `NativeTreeRingMemory`.
- `scripts/rust_performance_smoke.py` provides an operator-run local insert and
  recall timing check. It fails if expected recalls are empty, emits a stable
  `METRICS_JSON=` line, and enforces conservative synthetic-workload thresholds
  of at least 500 inserts/sec and max recall latency of 250 ms.

Latest local smoke on July 5, 2026 with `--count 10000`:

- Inserted 10,000 memories in 4,428.0 ms.
- Insert throughput: 2,258.3 inserts/sec.
- Recall average latency: 6.296 ms.
- Recall max latency: 9.904 ms.

## Compatibility Rule

Rust must read and write the same SQLite shape and JSON memory event payloads as the Python reference until an explicit migration exists.
