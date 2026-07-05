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
- v0.4 adds Rust-owned JSONL import/export with privacy-preserving defaults across the CLI and native Python binding.
- v0.5 adds Rust-owned audit checks for stale, sensitive, low-confidence, supersession, and contradiction candidates.
- v0.6 adds Rust-owned deterministic consolidation with idempotent summary records and cautious sensitive-memory handling.
- v0.7 makes the public facade Rust-native only and adds Rust-owned maintenance for expiry, secret redaction, and FTS repair.
- v0.8 removes Python-owned runtime behavior; Python is now a thin native-binding surface only.
- The Rust CLI also includes a Ratatui operator console behind `tree-ring tui`.

The Rust workspace currently includes:

- `crates/tree-ring-memory-core`: models, validation, sensitivity checks, and recall scoring.
- `crates/tree-ring-memory-sqlite`: schema-compatible SQLite/FTS storage and recall filtering.
- `crates/tree-ring-memory-cli`: native `tree-ring` CLI.

The public runtime is Rust-native. The Rust CLI and optional PyO3
`NativeTreeRingMemory` binding own storage, recall, import/export, audit,
consolidation, and maintenance behavior. Python only provides thin binding
wrappers and model conversion objects; all durable runtime behavior stays in
Rust.

`NativeTreeRingMemory` is the explicit Rust-native backend. `TreeRingMemory`
uses the same backend and fails with a clear install/build error when the PyO3
module is absent:

```python
from tree_ring_memory import TreeRingMemory

memory = TreeRingMemory.open(".tree-ring")
event = memory.remember(summary="Native Rust path works.", event_type="lesson")
results = memory.recall("Rust path")
maintenance = memory.maintain()
```

Build the optional native module with maturin:

```bash
cd bindings/python
pip install -e ../..
maturin develop
```

The native binding package is extension-only. Install the main Python package
and the native extension into the same environment.

The native Python binding exposes portable JSONL import/export:

```python
jsonl = memory.export_jsonl()
preview = memory.import_jsonl(jsonl, dry_run=True)
report = memory.import_jsonl(jsonl)
audit = memory.audit()
consolidation = memory.consolidate(period_type="manual", dry_run=True)
maintenance = memory.maintain()
```

Exports exclude sensitive and superseded memories by default. Pass
`include_sensitive=True` or `include_superseded=True` only when the caller
explicitly needs those rows.

There is no Python backend selector. `TreeRingMemory.open()` always uses the
native Rust binding and fails if the extension is missing.

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

The `tree-ring` command is the Rust CLI. The Python package no longer installs
a Python console script with that name.

```bash
tree-ring init
tree-ring remember "Use protocol-first design." --event-type decision --tag architecture
tree-ring recall "protocol design"
tree-ring forget mem_example --mode delete --reason "example cleanup"
tree-ring export --output memories.jsonl
tree-ring import memories.jsonl --dry-run
tree-ring import memories.jsonl
tree-ring audit --audit-type sensitive
tree-ring consolidate --period-type manual --dry-run
tree-ring maintain
tree-ring maintain --apply-expired --repair-fts
```

The CLI stores memory in `.tree-ring/` by default.

`tree-ring export` writes newline-delimited JSON. The first line is a
`tree_ring_memory_export` header with schema and plugin version metadata; each
remaining line is a `memory_event` envelope. The command excludes sensitive and
superseded memories unless `--include-sensitive` or `--include-superseded` is
set. Import validates all events, skips duplicate ids by default, and replaces
existing ids only with `--replace-existing`.

`tree-ring audit` is non-mutating. It reports deterministic local findings for
stale expiry, sensitive retention, low-confidence durable memory, supersession
integrity, and conservative contradiction candidates.

`tree-ring consolidate` creates deterministic local summary memories without an
LLM. Dry-run mode writes nothing. Persisted consolidation is idempotent for the
same period and source-memory set unless `--force` is provided. Sensitive
non-secret memories are summarized without copying raw payload text and require
review; secret-like memories are excluded from consolidation.

`tree-ring maintain` is safe by default. Without apply flags it is a dry-run
report, including on a missing root. It can apply eligible temporary-memory
expiry, redact secret-like memories, and rebuild SQLite FTS only when explicitly
asked through `--apply-expired`, `--apply-secret-redactions`, or `--repair-fts`.

## Terminal Console Preview

The Rust CLI includes a framework-agnostic Ratatui console for humans and agent
operators working from a terminal:

```bash
tree-ring tui
tree-ring --root .tree-ring tui --event-stream ./tree-ring-events.jsonl --tick-ms 150
```

The console keeps an animated ASCII tree-ring cross-section visible at all
times. Store-watch polling updates persisted counts from SQLite, while the
optional event stream lights rings in real time without treating stream events
as durable truth.

Useful keys and commands:

- `s` focuses search, `/` opens the slash command palette, `r` opens exploded
  ring view, `q` quits.
- `i` toggles sensitive-memory visibility, `u` toggles superseded-memory
  visibility.
- Slash commands include `/rings`, `/search <query>`, `/remember <summary>`,
  `/forget`, `/redact`, `/promote`, `/scar`, `/seed`, `/supersede <old_id>`,
  `/consolidate`, `/export`, `/sync`, `/stream`, and `/watch`.

Destructive or authority-changing operations are confirmation-gated. Sensitive
details stay hidden by default, and secret-like memory is blocked before
storage.

Event stream lines are local JSONL objects. They are display signals only:

```json
{"event":"remembered","ring":"cambium","label":"Stored project lesson"}
{"event":"policy_blocked","ring":"scar","label":"Secret-like memory blocked"}
```

## Development Checks

```bash
cargo test
python3 -m pytest
cargo run -p tree-ring-memory-cli -- --help
cargo run -p tree-ring-memory-cli -- export --help
cargo run -p tree-ring-memory-cli -- import --help
cargo run -p tree-ring-memory-cli -- audit --help
cargo run -p tree-ring-memory-cli -- consolidate --help
cargo run -p tree-ring-memory-cli -- maintain --help
python3 scripts/rust_performance_smoke.py --count 1000
cargo build -p tree-ring-memory-python --features extension-module
python3 scripts/native_binding_smoke.py --install-maturin
```

The Rust CLI writes the canonical SQLite/raw JSON shape. The performance smoke
asserts nonempty recalls, emits a `METRICS_JSON=` line, and uses conservative
local thresholds of at least 500 inserts/sec and max recall latency of 250 ms
for the synthetic workload.

## Design Docs

- `docs/superpowers/specs/2026-07-04-tree-ring-memory-framework-design.md`
- `docs/feature/tree-ring-memory-framework/diverge/options-raw.md`
- `docs/architecture/rust-core-roadmap.md`
- `docs/architecture/rust-core-status.md`
- `docs/superpowers/specs/2026-07-05-tree-ring-memory-rust-core-v0-2-design.md`
- `docs/superpowers/plans/2026-07-05-tree-ring-memory-rust-core-v0-2-implementation-plan.md`
- `docs/superpowers/specs/2026-07-05-tree-ring-memory-rust-python-bindings-v0-3-design.md`
- `docs/superpowers/plans/2026-07-05-tree-ring-memory-rust-python-bindings-v0-3-implementation-plan.md`
- `docs/superpowers/specs/2026-07-05-tree-ring-memory-ratatui-tui-design.md`
- `docs/superpowers/plans/2026-07-05-tree-ring-memory-ratatui-tui-implementation-plan.md`
- `docs/superpowers/specs/2026-07-05-tree-ring-memory-rust-import-export-v0-4-design.md`
- `docs/superpowers/plans/2026-07-05-tree-ring-memory-rust-import-export-v0-4-implementation-plan.md`
- `docs/superpowers/specs/2026-07-05-tree-ring-memory-rust-audit-v0-5-design.md`
- `docs/superpowers/plans/2026-07-05-tree-ring-memory-rust-audit-v0-5-implementation-plan.md`
- `docs/superpowers/specs/2026-07-05-tree-ring-memory-rust-consolidation-v0-6-design.md`
- `docs/superpowers/plans/2026-07-05-tree-ring-memory-rust-consolidation-v0-6-implementation-plan.md`
- `docs/superpowers/specs/2026-07-05-tree-ring-memory-rust-maintenance-v0-7-design.md`
- `docs/superpowers/plans/2026-07-05-tree-ring-memory-rust-maintenance-v0-7-implementation-plan.md`
- `docs/superpowers/specs/2026-07-05-tree-ring-memory-rust-only-v0-8-design.md`
- `docs/superpowers/plans/2026-07-05-tree-ring-memory-rust-only-v0-8-implementation-plan.md`

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
