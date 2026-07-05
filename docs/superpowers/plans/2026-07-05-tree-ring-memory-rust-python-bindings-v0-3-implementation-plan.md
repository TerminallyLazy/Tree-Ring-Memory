# Tree Ring Memory Rust Python Bindings v0.3 Implementation Plan

## Goal

Create the first testable PyO3 binding layer for Tree Ring Memory while preserving the current Python reference package.

## Constraints

- Keep the framework agent-harness agnostic.
- Keep `TreeRingMemory` Python-backed until parity is broader.
- Keep normal repo tests independent of maturin installation.
- Keep SQLite/FTS local and no external services.
- Keep schema compatibility with current fixtures.

## Phase 1: Binding Crate

### Tasks

1. Add `bindings/python` to the Cargo workspace.
2. Add a PyO3 crate named `tree-ring-memory-python`.
3. Add `build.rs` for extension-module link args.
4. Add a maturin `pyproject.toml`.

### Verification

```bash
cargo test
cargo build -p tree-ring-memory-python --features extension-module
```

## Phase 2: Native JSON API

### Tasks

1. Expose `TreeRingMemoryNative`.
2. Implement `open`, `remember_json`, `put_event_json`, `recall_json`, and `forget`.
3. Map validation errors to `ValueError`.
4. Map storage errors to `RuntimeError`.
5. Add Rust unit tests for round-trip memory and forget validation.

### Verification

```bash
cargo test -p tree-ring-memory-python
```

## Phase 3: Python Preview Wrapper

### Tasks

1. Add `src/tree_ring_memory/native_backend.py`.
2. Expose `NativeTreeRingMemory`.
3. Convert native JSON payloads into existing Python dataclasses.
4. Keep missing-extension errors clean.
5. Export wrapper from `tree_ring_memory.__init__`.

### Verification

```bash
python3 -m pytest
```

## Phase 4: Documentation

### Tasks

1. Update README status.
2. Update Rust core status doc.
3. Document build commands and limitations.

### Verification

```bash
cargo test
python3 -m pytest
cargo build -p tree-ring-memory-python --features extension-module
```

## Acceptance

- Native binding crate compiles and tests.
- Extension-module build compiles.
- Python package still works without native extension installed.
- Native wrapper is explicit and not presented as default parity.
