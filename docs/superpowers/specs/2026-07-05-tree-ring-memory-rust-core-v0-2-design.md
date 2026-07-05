# Tree Ring Memory Rust Core v0.2 Design

## Summary

Tree Ring Memory v0.2 converts the framework from a Python-only reference implementation into a Rust-first core with Python compatibility. The goal is not a big-bang rewrite. The goal is to make Rust the durable implementation boundary while preserving the existing Python user experience and the published memory schemas.

The current Python package remains the executable reference until Rust parity is proven. Once parity passes, the Python facade delegates to Rust through bindings.

## Goals

- Add a Rust workspace to the repository.
- Implement Rust crates for memory models, validation, sensitivity checks, SQLite/FTS storage, recall ranking, and forget workflows.
- Preserve the existing Python API shape: `TreeRingMemory.open()`, `remember()`, `recall()`, and `forget()`.
- Preserve the existing SQLite schema unless a migration is explicitly added.
- Preserve framework agnosticism: no dependency on any specific agent harness, orchestration framework, transport, or model provider in the core.
- Add shared parity fixtures that both Python and Rust must pass.
- Keep local-first operation with no network services and no external vector database requirement.

## Non-Goals

- No cloud sync.
- No daemon or sidecar in v0.2.
- No Node binding in v0.2, though the Rust API should not block one later.
- No WASM target in v0.2.
- No external vector search requirement.
- No removal of the Python package before compatibility is proven.
- No schema-breaking changes without migration tests.

## Architecture

```text
tree-ring-memory/
├── crates/
│   ├── tree-ring-memory-core/
│   ├── tree-ring-memory-sqlite/
│   └── tree-ring-memory-cli/
├── bindings/
│   └── python/
├── fixtures/
│   └── parity/
├── schemas/
├── src/tree_ring_memory/
└── tests/
```

### `tree-ring-memory-core`

Owns behavior that should not depend on SQLite or Python:

- `MemoryEvent`
- `MemorySource`
- `MemoryLink`
- `MemoryReview`
- ring, scope, sensitivity, and retention enums
- ID generation
- validation
- deterministic sensitivity detection and redaction
- recall score calculation
- source authority scoring
- JSON serialization compatibility

### `tree-ring-memory-sqlite`

Owns persistence:

- SQLite connection setup
- WAL and busy timeout
- migrations
- `memories` table
- `memory_fts` table
- FTS-safe query construction
- insert, get, list, delete, redact, supersede
- raw JSON compatibility with the Python reference

### `tree-ring-memory-cli`

Owns the native CLI:

- `tree-ring init`
- `tree-ring remember`
- `tree-ring recall`
- `tree-ring forget`

The CLI should call the Rust core directly and use the same storage crate as the Python binding.

### `bindings/python`

Owns compatibility with the current Python package:

- exposes an internal native module
- preserves the public Python facade
- maps Rust errors to Python exceptions
- converts Rust recall results into Python-compatible result objects

The public Python import should remain:

```python
from tree_ring_memory import TreeRingMemory
```

## Data Compatibility

The Rust implementation must read databases produced by the current Python implementation and write databases that the Python reference can still inspect during the transition.

Compatibility requirements:

- memory IDs keep the `mem_YYYYMMDD_HHMMSS_<hex>` shape
- timestamps remain ISO-8601 strings
- `raw_json` remains schema-compatible
- FTS rows match memory rows
- redaction clears source refs, tags, links, project, agent profile, details, supersession fields, and review metadata
- default recall excludes sensitive and superseded memory

## Recall Compatibility

Rust recall must match the current behavior:

- empty or filler-only queries return no scored FTS results
- plain user text must not be interpreted as raw FTS syntax
- project, agent profile, scope, ring, event type, sensitivity, and supersession filters apply before final output
- scars receive relevance boost for failure-like queries
- heartwood receives boost for durable preference and project-rule queries
- explainable ranking returns component scores

The exact floating-point score may differ slightly, but ordering should match for parity fixtures unless the fixture explicitly allows tolerance.

## Safety And Privacy

The Rust implementation must fail closed:

- block obvious secrets by default
- redact detected secret-like patterns
- exclude sensitive memory from recall by default
- exclude superseded memory by default
- require a forget reason at the API boundary
- preserve useful redacted shape without retaining sensitive payloads

## Testing Strategy

### Rust Tests

- model validation
- sensitivity detection
- SQLite migration and storage
- FTS query escaping
- recall ranking
- delete, redact, and supersede
- CLI behavior

### Python Tests

Existing Python tests continue to run. After the binding switch, they prove public API compatibility.

### Parity Fixtures

Create language-neutral JSON fixtures for:

- valid memory event
- invalid memory event
- secret-blocking case
- recall ranking case
- scar boost case
- heartwood boost case
- superseded memory case
- redact case
- SQLite round trip case

## Performance Targets

The current Python reference handled roughly:

- 10,000 memory inserts at about 1,400 inserts/sec in a local smoke test
- narrow recall under 3ms
- broad 10k recall around 90ms worst case

v0.2 Rust should target:

- at least 2,500 inserts/sec for local SQLite smoke tests
- under 5ms for narrow recall over 10k memories
- under 50ms for broad recall over 10k memories
- stable behavior under concurrent CLI/API access using WAL and busy timeout

These are smoke targets, not final benchmarks.

## Packaging

The repo should remain easy to use during migration:

- Rust workspace builds with Cargo.
- Python compatibility builds with a standard Python packaging path.
- The README should clearly distinguish Python reference, Rust core, and compatibility layer status.

Preferred Python binding path:

- use PyO3 for bindings
- use maturin for Python extension builds
- keep the public package name `tree-ring-memory`

## Acceptance Criteria

v0.2 is complete when:

1. Rust workspace builds locally.
2. Rust core validates memory events compatible with the published schema.
3. Rust SQLite crate creates and uses the same SQLite/FTS store shape.
4. Rust recall returns relevant ranked memories with filters and explanations.
5. Rust forget supports delete, redact, and supersede.
6. Rust CLI can init, remember, recall, and forget.
7. Python public API remains compatible.
8. Shared parity fixtures pass in Rust and Python.
9. Current Python test suite remains green or is replaced by equivalent compatibility tests.
10. Docs explain Rust-first direction and migration status.

## Open Decisions

- Whether the root Python package switches to maturin immediately or keeps hatchling while bindings live under `bindings/python` first.
- Whether `rusqlite` should use bundled SQLite by default.
- Whether the Rust CLI becomes the only installed `tree-ring` command in v0.2 or ships alongside the Python CLI until v0.3.

## Spec Self-Review

- No placeholders remain.
- The selected approach matches the user decision: Rust core plus Python compatibility.
- The scope is v0.2-sized and avoids sidecar, Node, WASM, and cloud work.
- The plan preserves the current working Python package until Rust parity is proven.
