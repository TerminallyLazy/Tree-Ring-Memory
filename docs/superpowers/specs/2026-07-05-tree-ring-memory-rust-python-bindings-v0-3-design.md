# Tree Ring Memory Rust Python Bindings v0.3 Design

## Summary

v0.3 starts replacing the CLI-shell bridge with a real PyO3 native module while keeping the existing Python reference facade stable.

The goal is an incremental native boundary, not a disruptive package switch. The default `TreeRingMemory` Python class remains Python-backed until the native module supports the full facade contract. The explicit `NativeTreeRingMemory` wrapper lets users and tests exercise Rust-backed behavior without pretending parity is complete.

## Goals

- Add a `bindings/python` PyO3 crate.
- Build a native module named `tree_ring_memory._tree_ring_memory_native`.
- Expose a low-level JSON API for remember, put full event JSON, recall, forget, and native version.
- Add a Python `NativeTreeRingMemory` wrapper that converts native JSON into existing Python dataclasses.
- Keep `TreeRingMemory` unchanged until native parity is broader.
- Keep local-first SQLite/FTS behavior and no external services.
- Preserve schema compatibility with the Rust core and Python reference.

## Non-Goals

- Do not switch the default Python facade in this phase.
- Do not remove the CLI bridge yet.
- Do not require maturin for normal repository tests.
- Do not add Node, WASM, sidecar, or daemon packaging.
- Do not change the public memory event schema.

## Binding Shape

```text
bindings/python/
├── Cargo.toml
├── build.rs
├── pyproject.toml
└── src/lib.rs
```

The native module exposes `TreeRingMemoryNative`, which stores through `tree-ring-memory-sqlite` and serializes results as JSON. Python wrappers own conversion back into the existing dataclasses.

## Python API Shape

```python
from tree_ring_memory import NativeTreeRingMemory

memory = NativeTreeRingMemory.open(".tree-ring")
event = memory.remember(summary="Native Rust path works.", event_type="lesson")
results = memory.recall("Rust path")
memory.forget(event.id, mode="delete", reason="test cleanup")
```

If the extension is not installed, `NativeTreeRingMemory.open()` must fail with a clean install/build hint.

## Build Strategy

- Normal `cargo test` compiles and tests the binding crate without the `extension-module` feature.
- Extension builds use `--features extension-module`.
- `bindings/python/build.rs` supplies PyO3 macOS extension-module linker args.
- `bindings/python/pyproject.toml` is maturin-ready, but maturin is not required for the default test suite.

## Acceptance Criteria

1. `cargo test` passes across the workspace including binding unit tests.
2. `cargo build -p tree-ring-memory-python --features extension-module` builds.
3. `python3 -m pytest` passes without the native extension installed.
4. Missing native extension errors are clear and actionable.
5. README and status docs state that `NativeTreeRingMemory` is explicit and preview-only.
