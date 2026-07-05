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
- PyO3 bindings remain planned after more parity and packaging work.

## Build Commands

```bash
cargo test
python3 -m pytest
cargo run -p tree-ring-memory-cli -- --help
```

## Python Rust Bridge

```python
from tree_ring_memory import RustCliTreeRingMemory

memory = RustCliTreeRingMemory.open(".tree-ring")
event = memory.remember(summary="Rust-backed memory works.", event_type="lesson")
results = memory.recall("Rust-backed memory")
```

The bridge is opt-in and currently shells out to the native Rust CLI. It proves
schema and object-shape compatibility while keeping the stable Python reference
implementation unchanged.

## Compatibility Rule

Rust must read and write the same SQLite shape and JSON memory event payloads as the Python reference until an explicit migration exists.
