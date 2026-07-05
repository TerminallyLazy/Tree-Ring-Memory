# Tree Ring Memory Rust Consolidation v0.6 Implementation Plan

## Goal

Implement deterministic Rust-owned consolidation and expose it through SQLite,
CLI, native Python, and the Python reference facade.

## Constraints

- Keep Tree Ring Memory framework-agnostic and local-first.
- Do not use LLMs or external services.
- Do not delete, redact, expire, or repair raw rows.
- Do not leak sensitive payloads in summary text.
- Preserve Rust/Python parity while Rust remains the behavioral owner.

## Task 1: Rust Core Consolidation Module

Add `tree-ring-memory-core/src/consolidation.rs`.

Required behavior:

- Define `ConsolidationPeriod`, `ConsolidationRequest`,
  `ConsolidationGroup`, `ConsolidationOutput`, and `ConsolidationReport`.
- Implement deterministic candidate filtering and grouping.
- Generate summary `MemoryEvent` values with source links.
- Provide stable period-key helpers.
- Avoid sensitive payload leakage.

Verification:

```bash
cargo test -p tree-ring-memory-core consolidation
```

## Task 2: SQLite Store And CLI

Add persistence and command support.

Required behavior:

- Add additive `consolidations` table in migration.
- Add `SQLiteMemoryStore::consolidate(request)`.
- Store consolidation records and summary memories transactionally.
- Implement dry-run, idempotency, and force supersession.
- Add `tree-ring consolidate` with text and JSON output.

Verification:

```bash
cargo test -p tree-ring-memory-sqlite consolidation
cargo test -p tree-ring-memory-cli consolidate
```

## Task 3: Python Native And Reference Surfaces

Expose consolidation through Python.

Required behavior:

- Add PyO3 `consolidate_json(...) -> str`.
- Add `NativeTreeRingMemory.consolidate(...) -> dict`.
- Add `PythonTreeRingMemory.consolidate(...) -> dict`.
- Mirror deterministic reference behavior in Python or delegate to compatible
  store helpers.
- Update native smoke to exercise consolidation.

Verification:

```bash
cargo test -p tree-ring-memory-python consolidation
python3 -m pytest tests/test_consolidation.py
python3 scripts/native_binding_smoke.py --install-maturin
```

## Task 4: Docs And Final Verification

Update README, Rust status, and roadmap docs.

Verification:

```bash
cargo fmt --check
git diff --check
cargo test
python3 -m pytest
cargo run -q -p tree-ring-memory-cli -- consolidate --help
cargo build -p tree-ring-memory-python --features extension-module
python3 scripts/rust_performance_smoke.py --count 10000
```

## Definition Of Done

- Rust core owns deterministic consolidation planning.
- SQLite and CLI can perform dry-run and persisted consolidation.
- Consolidation is idempotent unless forced.
- Forced consolidation supersedes previous summary memories.
- Python native and reference surfaces expose `consolidate()`.
- Tests cover safety, idempotency, CLI behavior, and parity.
- Docs explain consolidation scope and limitations.
