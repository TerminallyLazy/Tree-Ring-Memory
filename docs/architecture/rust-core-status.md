# Rust Core Status

Tree Ring Memory v0.2 is being implemented as a Rust-first core with Python compatibility.

## Current Status

- Python remains the stable public reference implementation.
- Rust workspace exists under `crates/`.
- Rust core owns model validation, sensitivity checks, and recall scoring.
- Rust SQLite crate owns schema-compatible SQLite/FTS storage.
- Rust CLI can initialize, remember, recall, and forget local memory.
- Rust CLI has JSON output for machine-readable adapter use.
- Python has an explicit `RustCliTreeRingMemory` compatibility adapter.
- The v0.2 adapter is intentionally limited: `remember` supports summary,
  event type, ring, scope, project, and tags; `recall` supports query, project,
  limit, and sensitive-memory inclusion. Unsupported Python facade fields raise
  `NotImplementedError`.
- PyO3 bindings remain planned after more parity and packaging work.

## Build Commands

```bash
cargo test
python3 -m pytest
cargo run -p tree-ring-memory-cli -- --help
python3 scripts/rust_performance_smoke.py --count 1000
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

## Smoke Coverage

- Rust unit tests cover model validation, sensitivity checks, recall scoring,
  SQLite/FTS storage, transactional row/FTS consistency, redaction, and basic
  concurrent writes.
- Python tests cover the existing reference package, Rust CLI database
  compatibility, and the opt-in `RustCliTreeRingMemory` adapter.
- `scripts/rust_performance_smoke.py` provides an operator-run local insert and
  recall timing check. It fails if expected recalls are empty, emits a stable
  `METRICS_JSON=` line, and enforces conservative synthetic-workload thresholds
  of at least 500 inserts/sec and max recall latency of 250 ms.

Latest local smoke on July 5, 2026 with `--count 10000`:

- Inserted 10,000 memories in 4,509.8 ms.
- Insert throughput: 2,217.4 inserts/sec.
- Recall average latency: 6.340 ms.
- Recall max latency: 9.858 ms.

## Compatibility Rule

Rust must read and write the same SQLite shape and JSON memory event payloads as the Python reference until an explicit migration exists.
