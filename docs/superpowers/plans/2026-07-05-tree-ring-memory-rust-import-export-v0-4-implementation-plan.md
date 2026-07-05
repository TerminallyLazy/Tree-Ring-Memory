# Tree Ring Memory Rust Import/Export v0.4 Implementation Plan

## Goal

Implement Rust-owned JSONL import/export for Tree Ring Memory while preserving
Python compatibility and privacy defaults.

## Constraints

- Keep the framework local-first and framework-agnostic.
- Do not add external services or dependencies unless already present.
- Sensitive memories must be excluded from export by default.
- Superseded memories must be excluded from export by default.
- Import validates schema before storage.
- Import dry-run must not mutate storage.
- Do not remove the Python reference backend.

## Task 1: Rust Import/Export Format

Add a Rust import/export module in `tree-ring-memory-core`.

Required behavior:

- Encode JSONL with a metadata header.
- Encode memory event envelopes as `{"type":"memory_event","memory":...}`.
- Decode JSONL containing header lines, envelope lines, and raw `MemoryEvent`
  JSON lines.
- Validate decoded memory events.
- Ignore blank lines.
- Return line-numbered parse errors.

Verification:

```bash
cargo test -p tree-ring-memory-core import_export
```

## Task 2: Rust Store Import/Export Operations

Add storage-level helpers in `tree-ring-memory-sqlite`.

Required behavior:

- Export events filtered by `include_sensitive` and `include_superseded`.
- Import events with `dry_run` and `replace_existing` options.
- Skip duplicates by default.
- Replace duplicates only when explicitly requested.
- Return structured import/export reports.

Verification:

```bash
cargo test -p tree-ring-memory-sqlite import_export
```

## Task 3: Rust CLI Commands

Add scriptable CLI commands:

```bash
tree-ring export --output memories.jsonl
tree-ring export --include-sensitive --include-superseded
tree-ring import memories.jsonl --dry-run
tree-ring import memories.jsonl --replace-existing
```

Required behavior:

- Export to stdout when `--output` is absent.
- Write reports when output/import actions complete.
- Respect global `--json` for operation reports where useful.
- Never emit sensitive rows unless `--include-sensitive` is set.
- Add CLI tests through the existing Python subprocess harness.

Verification:

```bash
cargo test -p tree-ring-memory-cli
python3 -m pytest tests/test_cli.py
```

## Task 4: Native Python Wrapper

Expose import/export from the optional native backend.

Required behavior:

- `NativeTreeRingMemory.export_jsonl(...) -> str`
- `NativeTreeRingMemory.import_jsonl(..., dry_run=False, replace_existing=False) -> dict`
- The methods call Rust native bindings when installed.
- The Python reference backend may expose equivalent methods for parity, but
  Rust remains the target owner.

Verification:

```bash
cargo test -p tree-ring-memory-python
python3 -m pytest tests/test_cli.py
python3 scripts/native_binding_smoke.py --install-maturin
```

## Task 5: Docs And Final Verification

Update README and Rust status docs.

Verification:

```bash
cargo fmt
cargo test
python3 -m pytest
cargo run -p tree-ring-memory-cli -- export --help
cargo run -p tree-ring-memory-cli -- import --help
cargo build -p tree-ring-memory-python --features extension-module
python3 scripts/native_binding_smoke.py --install-maturin
python3 scripts/rust_performance_smoke.py --count 10000
```

## Definition Of Done

- Rust exports JSONL memory bundles.
- Rust imports JSONL memory bundles.
- Sensitive memory is excluded by default.
- Superseded memory is excluded by default.
- Import dry-run is non-mutating.
- Duplicate import behavior is explicit and tested.
- Native Python wrapper exposes the Rust behavior.
- README documents the feature.
- Tests and smoke checks pass.
