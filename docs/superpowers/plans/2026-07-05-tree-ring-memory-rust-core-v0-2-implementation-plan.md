# Tree Ring Memory Rust Core v0.2 Implementation Plan

## Goal

Build a Rust-first core for Tree Ring Memory while preserving the current Python API through compatibility bindings.

## Constraints

- Keep the framework agent-harness agnostic.
- Keep SQLite + FTS as the default local store.
- Do not require external services.
- Do not remove the Python API before compatibility tests pass.
- Do not change the public memory schema without migration tests.

## Phase 1: Workspace And Fixtures

### Tasks

1. Add a Cargo workspace.
2. Add crates:
   - `crates/tree-ring-memory-core`
   - `crates/tree-ring-memory-sqlite`
   - `crates/tree-ring-memory-cli`
3. Add `fixtures/parity/`.
4. Add JSON fixtures for valid event, invalid event, recall ranking, sensitivity, redaction, supersession, and SQLite round trip.
5. Add a short `docs/architecture/rust-core-status.md` tracking migration status.

### Verification

```bash
cargo test
python3 -m pytest
```

### Acceptance

- Workspace builds.
- Empty crates have test harnesses.
- Fixtures are documented and loadable.

## Phase 2: Rust Core Models

### Tasks

1. Implement core structs:
   - `MemoryEvent`
   - `MemorySource`
   - `MemoryLink`
   - `MemoryReview`
2. Implement enums:
   - ring
   - scope
   - sensitivity
   - retention
3. Implement validation:
   - required summary
   - required event type
   - valid ring/scope/sensitivity/retention
   - finite 0..1 salience/confidence
4. Implement ID generation with timestamp plus random hex suffix.
5. Implement JSON serialization compatible with `schemas/memory-event.schema.json`.

### Verification

```bash
cargo test -p tree-ring-memory-core
python3 -m pytest tests/test_models.py
```

### Acceptance

- Rust model fixtures match Python output shape.
- Invalid Rust model cases match Python error intent.

## Phase 3: Sensitivity Guard

### Tasks

1. Port deterministic secret detection patterns.
2. Implement redaction helpers.
3. Add tests for API-token-like secrets, bearer tokens, passwords, private keys, and provider-style keys.
4. Keep category names compatible with existing schema.

### Verification

```bash
cargo test -p tree-ring-memory-core sensitivity
python3 -m pytest tests/test_sensitivity.py
```

### Acceptance

- Obvious secrets are blocked.
- Redaction preserves useful shape without sensitive payload.

## Phase 4: SQLite And FTS Store

### Tasks

1. Implement Rust SQLite connection setup.
2. Enable WAL and busy timeout.
3. Create schema-compatible migrations.
4. Implement:
   - put
   - get
   - list
   - search_text
   - delete
   - redact
   - supersede
5. Implement FTS-safe query construction.
6. Add database compatibility tests using Python-created DBs and Rust-created DBs.

### Verification

```bash
cargo test -p tree-ring-memory-sqlite
python3 -m pytest tests/test_store.py
```

### Acceptance

- Rust creates `memory.sqlite`.
- `memories` and `memory_fts` counts match after inserts.
- Python can inspect Rust-written databases during transition.

## Phase 5: Recall Ranking

### Tasks

1. Port recall scoring.
2. Implement filters:
   - project
   - agent profile
   - scope
   - ring
   - event type
   - sensitivity
   - supersession
3. Implement boosts:
   - scar boost for failure-like queries
   - heartwood boost for durable preference/project-rule queries
   - seed boost for future/planning queries
4. Implement explainable ranking output.
5. Add parity ranking tests.

### Verification

```bash
cargo test recall
python3 -m pytest tests/test_recall.py
```

### Acceptance

- Relevant memories appear first in parity fixtures.
- Sensitive and superseded memories are excluded by default.
- Plain user queries do not break FTS.

## Phase 6: Rust CLI

### Tasks

1. Implement native CLI with `clap`.
2. Commands:
   - `init`
   - `remember`
   - `recall`
   - `forget`
3. Match current CLI output closely enough for compatibility.
4. Add CLI tests using temporary stores.

### Verification

```bash
cargo test -p tree-ring-memory-cli
cargo run -p tree-ring-memory-cli -- --help
python3 -m pytest tests/test_cli.py
```

### Acceptance

- Native CLI can init, remember, recall, and forget.
- Deleted memory no longer recalls.

## Phase 7: Python Compatibility Binding

### Tasks

1. Add `bindings/python`.
2. Add PyO3 binding layer.
3. Preserve public Python imports.
4. Route `TreeRingMemory.open`, `remember`, `recall`, and `forget` through Rust.
5. Map Rust errors to Python exceptions.
6. Keep Python dataclass compatibility or provide equivalent Python-facing result objects.

### Verification

```bash
python3 -m pytest
cargo test
```

### Acceptance

- Existing Python tests pass against Rust-backed behavior.
- Public API examples in README still work.

## Phase 8: Performance And Concurrency Smoke

### Tasks

1. Add a small benchmark or smoke script for:
   - 10k inserts
   - narrow recall
   - broad recall
2. Add a concurrency smoke for multiple opens/writes.
3. Record current numbers in docs.

### Targets

- 2,500+ inserts/sec over 10k local memories.
- under 5ms narrow recall over 10k memories.
- under 50ms broad recall over 10k memories.
- no lock failure under basic concurrent CLI/API access.

### Verification

```bash
cargo test
python3 -m pytest
```

### Acceptance

- Smoke numbers are documented.
- Rust does not regress below Python reference behavior without explanation.

## Phase 9: Documentation And Release Prep

### Tasks

1. Update README status.
2. Document Rust crates.
3. Document Python compatibility.
4. Document build/test commands.
5. Document migration status and known limitations.
6. Update version notes for v0.2.

### Verification

```bash
cargo test
python3 -m pytest
```

### Acceptance

- Docs match the actual implementation.
- Release notes state exactly what is Rust-backed.

## Definition Of Done

v0.2 is done when:

- Rust core, SQLite store, recall, forget, and CLI work locally.
- Python API compatibility is preserved.
- Shared parity fixtures pass.
- Tests pass in Rust and Python.
- Docs clearly state Rust-first core status.
- No host-specific agent harness dependency is introduced.
