# Rust Core v0.2 Brainstorm

## HMW Question

How might we move Tree Ring Memory from a Python reference implementation to a Rust-first core while preserving framework-agnostic adoption, the existing Python API, and confidence that storage, recall, privacy, and forgetting still work correctly?

## SCAMPER Options

### Option 1: Rust Core With Python Compatibility

**Core idea**: Users keep the current Python API while the underlying memory model, validation, SQLite storage, sensitivity guard, recall ranking, and forget workflows move into Rust.
**Key mechanism**: Build Rust crates for core and SQLite behavior, expose a PyO3-backed Python module, and run parity tests against the existing Python reference before switching the facade.
**Key assumption**: Existing Python users matter, and preserving API continuity is worth the binding complexity.
**SCAMPER origin**: Substitute
**Closest competitor**: `ruff` and `pydantic-core`, where Python-facing packages rely on Rust internals.

### Option 2: Rust CLI Plus Python Reference

**Core idea**: Users get a fast native `tree-ring` CLI first, while Python remains the library reference until the Rust core stabilizes.
**Key mechanism**: Create a Rust CLI and SQLite backend that reads/writes the same database and JSON schemas, then gradually migrate Python calls later.
**Key assumption**: CLI performance and portability are the fastest way to prove the Rust direction.
**SCAMPER origin**: Combine
**Closest competitor**: `ripgrep`, `uv`, and other Rust CLIs that later grow language bindings.

### Option 3: SQLite Extension Style Core

**Core idea**: Users integrate Tree Ring Memory as a tiny local SQLite-backed engine that exposes memory functions through SQL-compatible primitives and small host adapters.
**Key mechanism**: Adapt the mental model of SQLite extensions: Rust owns schema, migrations, FTS query construction, and ranking functions; hosts call stable operations.
**Key assumption**: The durable center of the product is the local database contract, not any host language API.
**SCAMPER origin**: Adapt
**Closest competitor**: SQLite extensions, Tantivy-backed search libraries, and DuckDB-style embeddable engines.

### Option 4: Performance-First Rust Recall Engine

**Core idea**: Users keep Python storage for v0.2, but recall scoring, query planning, FTS query construction, and ranking move to Rust first.
**Key mechanism**: Magnify the performance-critical surface and replace only recall with a Rust extension module.
**Key assumption**: Recall quality and speed are the highest-value path, and storage migration can wait.
**SCAMPER origin**: Modify/Magnify
**Closest competitor**: Native ranking extensions inside search-heavy Python packages.

### Option 5: Multi-Language Adapter Kit

**Core idea**: Users in Python, Node, CLIs, and agent sidecars all consume one Rust core through generated adapters and shared schema fixtures.
**Key mechanism**: Treat the Rust core as an SDK kernel and produce bindings for Python first, then Node, with adapter compliance tests.
**Key assumption**: The main adoption barrier is cross-framework integration, so v0.2 should establish binding architecture early.
**SCAMPER origin**: Put to other use
**Closest competitor**: `tree-sitter`, `llama.cpp` wrappers, and `libsql` bindings.

### Option 6: Minimal Rust Kernel

**Core idea**: Users get only the smallest Rust core needed to validate memory events and serialize schema-compatible records; Python keeps storage and recall temporarily.
**Key mechanism**: Eliminate migration complexity by moving schema validation, enums, IDs, and sensitivity checks first.
**Key assumption**: Small incremental replacement lowers risk more than it accelerates the final architecture.
**SCAMPER origin**: Eliminate
**Closest competitor**: Libraries that introduce a native validator before migrating the full runtime.

### Option 7: Rust Truth, Python Compatibility Shell

**Core idea**: Users treat Rust as the canonical implementation immediately, and the Python package becomes only a compatibility shell around Rust.
**Key mechanism**: Reverse the current reference relationship: write Rust behavior first, then force Python tests to call Rust and delete duplicated Python behavior as soon as parity passes.
**Key assumption**: Maintaining two behavior implementations is more dangerous than a faster migration to one source of truth.
**SCAMPER origin**: Reverse
**Closest competitor**: Python packages where all durable logic lives in native modules.

## Crazy 8s Supplements

### Option 8: Sidecar Daemon First

**Core idea**: Users run Tree Ring Memory as a local Rust sidecar service and every agent harness talks to it over a local protocol.
**Key mechanism**: Build a daemon around Rust storage and recall, then keep language clients thin.
**Key assumption**: Process isolation and universal local APIs matter more than in-process embedding.
**SCAMPER origin**: Crazy 8s supplement
**Closest competitor**: Local model servers and embedded service daemons.

### Option 9: Parity Harness First

**Core idea**: Users do not see a Rust runtime yet; v0.2 ships a rigorous language-neutral parity test suite that makes the Rust rewrite safe.
**Key mechanism**: Create fixtures for remember, recall, sensitivity, supersession, redaction, FTS escaping, and import/export before adding Rust code.
**Key assumption**: The hardest part of conversion is preventing behavior drift.
**SCAMPER origin**: Crazy 8s supplement
**Closest competitor**: Compatibility test suites for protocol implementations.

### Option 10: WASM Core

**Core idea**: Users embed Tree Ring Memory in browser, desktop, server, and agent runtimes through one Rust-to-WASM core.
**Key mechanism**: Compile memory validation, sensitivity, and ranking to WASM; leave SQLite-backed persistence to host adapters.
**Key assumption**: Maximum portability is more important than owning SQLite directly in the first Rust release.
**SCAMPER origin**: Crazy 8s supplement
**Closest competitor**: Portable WASM validation and policy engines.

## Curated 6

### Option 1: Rust Core With Python Compatibility

**Diversity test**:

- Different mechanism: Rust core plus PyO3 compatibility layer.
- Different user behavior assumption: Existing Python users keep current API.
- Different cost/effort profile: Medium-high effort, strong continuity.

### Option 2: Rust CLI Plus Python Reference

**Diversity test**:

- Different mechanism: Native CLI proves Rust independently.
- Different user behavior assumption: Users adopt CLI before library bindings.
- Different cost/effort profile: Medium effort, lower binding complexity.

### Option 3: SQLite Extension Style Core

**Diversity test**:

- Different mechanism: Database contract is the central integration layer.
- Different user behavior assumption: Hosts prefer stable local storage semantics.
- Different cost/effort profile: High design rigor, strong long-term portability.

### Option 5: Multi-Language Adapter Kit

**Diversity test**:

- Different mechanism: Rust SDK kernel with generated or thin bindings.
- Different user behavior assumption: Adoption depends on many agent harnesses.
- Different cost/effort profile: High effort, broader ecosystem payoff.

### Option 7: Rust Truth, Python Compatibility Shell

**Diversity test**:

- Different mechanism: Rust becomes canonical immediately after parity.
- Different user behavior assumption: Users tolerate faster internals migration if API remains stable.
- Different cost/effort profile: Higher migration pressure, lower long-term duplication.

### Option 9: Parity Harness First

**Diversity test**:

- Different mechanism: Test contract precedes runtime migration.
- Different user behavior assumption: Maintainers value correctness gates before new implementation.
- Different cost/effort profile: Low runtime change, high safety value.

## Eliminated Or Merged Options

- Option 4 was merged into Option 1 because recall ranking should move with the storage and model boundaries rather than as a partial isolated extension.
- Option 6 was merged into Option 9 because minimal Rust validation alone does not prove product value without shared parity fixtures.
- Option 8 was deferred because a sidecar daemon is useful later, but v0.2 should preserve simple embedded local usage.
- Option 10 was deferred because WASM is attractive for portability, but SQLite-backed local storage and Python compatibility are higher-priority next-version goals.

## Selected Direction For v0.2

Use Option 1, strengthened by Option 9:

Rust core with Python compatibility, backed by a shared parity fixture suite that prevents behavior drift while storage, recall, privacy, and forget workflows move out of Python.
