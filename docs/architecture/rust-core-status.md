# Rust Core Status

Tree Ring Memory v0.2 is being implemented as a Rust-first core with Python compatibility.

## Current Status

- Python remains the stable public reference implementation.
- Rust workspace exists under `crates/`.
- Rust core owns model validation, sensitivity checks, and recall scoring.
- Rust SQLite crate owns schema-compatible SQLite/FTS storage.
- Rust CLI can initialize, remember, recall, and forget local memory.
- Python bindings are planned after Rust parity is proven.

## Build Commands

```bash
cargo test
python3 -m pytest
cargo run -p tree-ring-memory-cli -- --help
```

## Compatibility Rule

Rust must read and write the same SQLite shape and JSON memory event payloads as the Python reference until an explicit migration exists.
