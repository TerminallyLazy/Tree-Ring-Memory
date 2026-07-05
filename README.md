# Tree Ring Memory

![Tree Ring Memory retro roller-rink banner](assets/tree-ring-memory-banner.png)

Tree Ring Memory is a framework-agnostic memory lifecycle layer for AI agents.

It helps agents remember useful decisions, warnings, preferences, and lessons without turning memory into a transcript dump. Fresh memory stays detailed, older memory compresses into rings, important scars remain visible, and durable truths become heartwood.

Tree Ring Memory is inspired by the spatial project-memory patterns in
[DOX](https://github.com/agent0ai/dox) and the evidence-driven improvement loop
in [Revolve](https://github.com/agent0ai/revolve), with a deliberate nod to
their original creator, [frdel](https://github.com/frdel). This project is
framework-agnostic and does not replace either protocol.

## Status

Tree Ring Memory is in protocol-preview status.

- v0.1 provides a local Python reference library with SQLite storage and no required cloud services.
- v0.2 moved durable behavior into a Rust core while preserving Python compatibility.
- v0.3 makes the public Python facade Rust-first when the optional PyO3 native module is installed.

The Rust workspace currently includes:

- `crates/tree-ring-memory-core`: models, validation, sensitivity checks, and recall scoring.
- `crates/tree-ring-memory-sqlite`: schema-compatible SQLite/FTS storage and recall filtering.
- `crates/tree-ring-memory-cli`: native `tree-ring` CLI.

Python remains the stable public API surface, but Rust is the authoritative
engine when the native module is installed. Source checkouts without the native
extension fall back to the explicit `PythonTreeRingMemory` reference backend
unless `TREE_RING_MEMORY_REQUIRE_NATIVE=1` or `TREE_RING_MEMORY_BACKEND=native`
is set.

Python can also exercise the Rust-backed path explicitly through
`RustCliTreeRingMemory`. This bridge uses the native Rust CLI and returns the
same Python model object shapes, but it is intentionally limited in v0.2:
`remember` supports summary, event type, ring, scope, project, and tags; `recall`
supports query, project, limit, and sensitive-memory inclusion. Unsupported
Python facade fields fail explicitly instead of being silently ignored.

`NativeTreeRingMemory` is the explicit Rust-native backend. `TreeRingMemory`
uses the same backend automatically when the optional PyO3 module is installed:

```python
from tree_ring_memory import TreeRingMemory

memory = TreeRingMemory.open(".tree-ring")
event = memory.remember(summary="Native Rust path works.", event_type="lesson")
results = memory.recall("Rust path")
```

Build the optional native module with maturin:

```bash
cd bindings/python
pip install -e ../..
maturin develop
```

The native binding package is extension-only. It does not package or own the
public `tree_ring_memory` Python package; install the main package separately in
the same environment.

Backend selection:

- `TREE_RING_MEMORY_BACKEND=auto` uses native Rust when available and otherwise
  falls back to the Python reference backend.
- `TREE_RING_MEMORY_BACKEND=native` or `TREE_RING_MEMORY_REQUIRE_NATIVE=1`
  requires Rust native bindings and fails if they are missing.
- `TREE_RING_MEMORY_BACKEND=python` forces the reference backend for parity
  testing or troubleshooting.

For the CLI bridge, set `TREE_RING_MEMORY_CLI=/path/to/tree-ring` to use a
prebuilt binary. If unset, the bridge looks for `tree-ring` on `PATH` and falls
back to `cargo run` for development checkouts.

```python
from tree_ring_memory import RustCliTreeRingMemory

memory = RustCliTreeRingMemory.open(".tree-ring")
event = memory.remember(summary="Rust-backed memory works.", event_type="lesson")
results = memory.recall("Rust-backed memory")
```

## First Example

```python
from tree_ring_memory import TreeRingMemory

memory = TreeRingMemory.open(".tree-ring")
event = memory.remember(
    summary="Use project-scoped recall before changing release behavior.",
    event_type="lesson",
    scope="project",
    project="example-service",
    tags=["release", "workflow"],
)

results = memory.recall("release behavior", project="example-service")
for result in results:
    print(result.memory.summary, result.score)
```

## CLI Preview

```bash
tree-ring init
tree-ring remember "Use protocol-first design." --event-type decision --tag architecture
tree-ring recall "protocol design"
tree-ring forget mem_example --mode delete --reason "example cleanup"
```

The CLI stores memory in `.tree-ring/` by default.

## Development Checks

```bash
cargo test
python3 -m pytest
cargo run -p tree-ring-memory-cli -- --help
python3 scripts/rust_performance_smoke.py --count 1000
cargo build -p tree-ring-memory-python --features extension-module
python3 scripts/native_binding_smoke.py --install-maturin
```

The Rust CLI writes the same SQLite/raw JSON shape as the Python reference. The
performance smoke asserts nonempty recalls, emits a `METRICS_JSON=` line, and
uses conservative local thresholds of at least 500 inserts/sec and max recall
latency of 250 ms for the synthetic workload.

## Design Docs

- `docs/superpowers/specs/2026-07-04-tree-ring-memory-framework-design.md`
- `docs/feature/tree-ring-memory-framework/diverge/options-raw.md`
- `docs/architecture/rust-core-roadmap.md`
- `docs/architecture/rust-core-status.md`
- `docs/superpowers/specs/2026-07-05-tree-ring-memory-rust-core-v0-2-design.md`
- `docs/superpowers/plans/2026-07-05-tree-ring-memory-rust-core-v0-2-implementation-plan.md`
- `docs/superpowers/specs/2026-07-05-tree-ring-memory-rust-python-bindings-v0-3-design.md`
- `docs/superpowers/plans/2026-07-05-tree-ring-memory-rust-python-bindings-v0-3-implementation-plan.md`

## Agent Workflow Integration

- `skills/tree-ring-memory/SKILL.md` gives agents portable guidance for when to recall, remember, redact, forget, or avoid memory capture.
- `templates/dox/AGENTS.md` is a DOX-style project contract template for repos that want Tree Ring Memory rules alongside source code.
- `docs/integrations/agent-skill.md` explains how to use both without making memory more authoritative than local project docs.

## Brand Assets

- `assets/tree-ring-memory-logo.png`
- `assets/tree-ring-memory-banner.png`

## Principles

- Local-first by default.
- Protocol before adapters.
- Explainable recall.
- Sensitive data fails closed.
- Forgetting and supersession are first-class.
- Memory quality should be testable.
