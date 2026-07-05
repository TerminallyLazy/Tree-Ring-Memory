# Tree Ring Memory Rust-Only v0.8 Implementation Plan

## Task 1: Remove Python Runtime Owners

Files:

- `src/tree_ring_memory/api.py`
- `src/tree_ring_memory/__init__.py`
- `src/tree_ring_memory/models.py`
- delete `src/tree_ring_memory/cli.py`
- delete `src/tree_ring_memory/store.py`
- delete `src/tree_ring_memory/recall.py`
- delete `src/tree_ring_memory/sensitivity.py`
- delete `src/tree_ring_memory/rust_backend.py`

Work:

- Remove `PythonTreeRingMemory`.
- Make `TreeRingMemory.open()` a direct alias to `NativeTreeRingMemory.open()`.
- Move `RecallResult` into `models.py`.
- Keep only conversion dataclasses and validation helpers in Python.

Checks:

```bash
python3 -m pytest tests/test_cli.py tests/test_models.py tests/test_native_packaging.py
```

## Task 2: Retire Python Reference Tests

Files:

- delete Python reference behavior tests that only validate removed runtime:
  - `tests/test_store.py`
  - `tests/test_recall.py`
  - `tests/test_sensitivity.py`
  - `tests/test_audit.py`
  - `tests/test_consolidation.py`
  - `tests/test_import_export.py`
- keep and update wrapper/packaging tests.

Work:

- Keep tests that validate native wrapper marshalling.
- Remove tests that create `PythonTreeRingMemory`.
- Add tests proving removed exports/modules are absent.

Checks:

```bash
python3 -m pytest
```

## Task 3: Version, Docs, Smoke

Files:

- `Cargo.toml`
- `Cargo.lock`
- `pyproject.toml`
- `bindings/python/pyproject.toml`
- `src/tree_ring_memory/store.py` is deleted, so no Python plugin-version
  constant remains.
- `README.md`
- `docs/architecture/rust-core-status.md`
- `docs/architecture/rust-core-roadmap.md`
- `docs/integrations/agent-skill.md`
- `scripts/native_binding_smoke.py`

Work:

- Bump workspace and package metadata to `0.8.0`.
- Update docs to say Python is bindings only.
- Ensure native smoke exercises Rust-owned behavior through Python.

Final checks:

```bash
cargo test
python3 -m pytest
cargo build -p tree-ring-memory-python --features extension-module
python3 scripts/native_binding_smoke.py --install-maturin
python3 scripts/rust_performance_smoke.py --count 10000
```

