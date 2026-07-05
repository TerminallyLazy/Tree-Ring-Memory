# Tree Ring Memory Rust Maintenance v0.7 Implementation Plan

## Objective

Make Tree Ring Memory's public runtime Rust-native and add Rust-owned
maintenance lifecycle behavior for expiry, secret redaction, and FTS repair.

The phase must preserve the framework-agnostic promise: local SQLite/FTS, no
cloud dependency, no host-specific assumptions, and no silent destructive
maintenance.

## Task 1: Rust Maintenance Core

Files:

- `crates/tree-ring-memory-core/src/maintenance.rs`
- `crates/tree-ring-memory-core/src/lib.rs`

Implement:

- `MaintenanceRequest`
- `MaintenanceAction`
- `MaintenanceFtsReport`
- `MaintenanceReport`
- `MaintenanceActionType`
- `MaintenanceSeverity`
- `plan_maintenance(events, request) -> MaintenanceReport`

Rules:

- Default request is report-only / dry-run.
- Project and superseded filters are honored.
- Expired `ephemeral` and `forget_after_date` rows plan `delete_expired`.
- Expired protected rows (`scar`, `heartwood`, `durable`, `user_pinned`) plan
  `review_expired_protected`.
- Secret-like memory is detected through deterministic sensitivity inspection
  across the full public event surface and plans `redact_secret`.
- Reports do not include raw secret payloads.

Tests:

```bash
cargo test -p tree-ring-memory-core maintenance
```

## Task 2: SQLite Store And CLI

Files:

- `crates/tree-ring-memory-sqlite/src/lib.rs`
- `crates/tree-ring-memory-cli/src/main.rs`
- focused Rust tests in those files

Implement:

- `SQLiteMemoryStore::maintain(&MaintenanceRequest)`
- FTS drift detection:
  - memory rows
  - FTS rows
  - missing FTS rows
  - orphan FTS rows
- Transactional FTS rebuild when `repair_fts=true` and `dry_run=false`.
- Transactional application of expired deletes and secret redactions.
- `tree-ring maintain` CLI:
  - `--project`
  - `--include-superseded`
  - `--apply-expired`
  - `--apply-secret-redactions`
  - `--repair-fts`
  - JSON and text output

Safety:

- `tree-ring maintain` writes nothing by default.
- Destructive changes require `dry_run=false` semantics expressed through
  explicit apply flags. The CLI should set `dry_run=false` only when at least
  one apply/repair flag is present.
- Protected expired memories are never deleted by maintenance.

Tests:

```bash
cargo test -p tree-ring-memory-sqlite maintenance
cargo test -p tree-ring-memory-cli maintain
```

## Task 3: Native Python Binding And Rust-Only Facade

Files:

- `bindings/python/src/lib.rs`
- `src/tree_ring_memory/native_backend.py`
- `src/tree_ring_memory/api.py`
- `src/tree_ring_memory/__init__.py`
- relevant tests

Implement:

- PyO3 `maintenance_json(...) -> str`.
- `NativeTreeRingMemory.maintain(...) -> dict`.
- `TreeRingMemory.open()` must require the native Rust backend by default.
- Remove silent fallback from `TreeRingMemory.open()` to
  `PythonTreeRingMemory`.
- Keep `PythonTreeRingMemory` only as an explicit legacy/reference class for
  existing compatibility tests; do not add `PythonTreeRingMemory.maintain`.
- Update docs and tests to describe Python as binding/compatibility code, not
  the runtime owner.

Tests:

```bash
cargo test -p tree-ring-memory-python maintenance
python3 -m pytest
python3 scripts/native_binding_smoke.py --install-maturin
```

## Task 4: Docs, Versioning, And Final Verification

Files:

- `Cargo.toml`
- `pyproject.toml`
- `bindings/python/pyproject.toml`
- `README.md`
- `docs/architecture/rust-core-roadmap.md`
- `docs/architecture/rust-core-status.md`
- version assertions in tests

Implement:

- Bump crate/package metadata to `0.7.0`.
- Document v0.7 as Rust-native public runtime plus maintenance lifecycle.
- Update backend selection docs:
  - public facade requires native Rust;
  - `NativeTreeRingMemory` is the Python binding wrapper;
  - `PythonTreeRingMemory` is explicit legacy/reference compatibility only.
- Include current performance smoke numbers after final run.

Final verification:

```bash
cargo fmt --check
git diff --check
cargo test
python3 -m pytest
cargo run -q -p tree-ring-memory-cli -- maintain --help
cargo build -p tree-ring-memory-python --features extension-module
python3 scripts/native_binding_smoke.py --install-maturin
python3 scripts/rust_performance_smoke.py --count 10000
```

## Acceptance Gate

- Rust owns the maintenance planner and store execution.
- Public `TreeRingMemory.open()` no longer silently falls back to Python.
- Maintenance defaults are safe and report-only.
- Expired memory deletion, secret redaction, and FTS repair require explicit
  apply flags.
- Tests pass locally without external services.
- Subagent spec and code-quality reviews have no unresolved blockers.
