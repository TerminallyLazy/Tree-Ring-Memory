# Tree Ring Memory Rust Audit v0.5 Implementation Plan

## Goal

Implement deterministic Rust-owned audit checks and expose them through the
SQLite store, CLI, native Python backend, and Python reference facade.

## Constraints

- Keep Tree Ring Memory framework-agnostic and local-first.
- Do not add external services or new dependencies unless already present.
- Do not mutate storage during audit.
- Do not add schema-breaking migrations.
- Do not leak sensitive payloads in findings.
- Keep Python reference parity while Rust remains the target owner.

## Task 1: Rust Core Audit Module

Add `tree-ring-memory-core/src/audit.rs`.

Required behavior:

- Define `AuditType`, `AuditSeverity`, `AuditFinding`, and `AuditReport`.
- Implement `audit_memories(events, audit_type)` over `MemoryEvent` slices.
- Implement checks for stale, sensitive, low-confidence, supersession, and
  conservative contradiction candidates.
- Keep outputs JSON-serializable with stable string fields.

Verification:

```bash
cargo test -p tree-ring-memory-core audit
```

## Task 2: SQLite Store And CLI Audit

Add storage and CLI surfaces.

Required behavior:

- Add `SQLiteMemoryStore::audit(audit_type)`.
- Add `tree-ring audit --audit-type ...`.
- `--json` emits a report JSON object.
- Text mode prints a concise summary plus one line per finding.
- Audit must include superseded rows while checking integrity.
- Audit must not modify storage.

Verification:

```bash
cargo test -p tree-ring-memory-sqlite audit
cargo test -p tree-ring-memory-cli
python3 -m pytest tests/test_cli.py
```

## Task 3: Python Native And Reference Audit

Expose audit through Python.

Required behavior:

- Add PyO3 `audit_json(audit_type="all") -> str`.
- Add `NativeTreeRingMemory.audit(audit_type="all") -> dict`.
- Add `PythonTreeRingMemory.audit(audit_type="all") -> dict`.
- Mirror deterministic checks in the Python reference backend.

Verification:

```bash
cargo test -p tree-ring-memory-python
python3 -m pytest tests/test_audit.py tests/test_cli.py
python3 scripts/native_binding_smoke.py --install-maturin
```

## Task 4: Docs And Final Verification

Update README and Rust status/roadmap docs.

Verification:

```bash
cargo fmt --check
git diff --check
cargo test
python3 -m pytest
cargo run -q -p tree-ring-memory-cli -- audit --help
cargo build -p tree-ring-memory-python --features extension-module
python3 scripts/rust_performance_smoke.py --count 10000
```

## Definition Of Done

- Rust core owns deterministic audit checks.
- SQLite store and CLI expose audit.
- Native Python and reference Python expose audit.
- Audit is non-mutating and privacy-aware.
- Regression tests cover each audit type.
- Docs explain audit use and limitations.
- Full Rust and Python verification passes.
