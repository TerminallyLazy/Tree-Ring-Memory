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
- v0.9 removes tracked Python source, tests, and smoke scripts from the canonical repo; the optional CPython extension is Rust-built.
- The Rust CLI also includes a Ratatui operator console behind `tree-ring tui`.

The Rust workspace currently includes:

- `crates/tree-ring-memory-core`: models, validation, sensitivity checks, and recall scoring.
- `crates/tree-ring-memory-sqlite`: schema-compatible SQLite/FTS storage and recall filtering.
- `crates/tree-ring-memory-cli`: native `tree-ring` CLI.
- `bindings/python`: optional Rust PyO3 crate for CPython extension builds.

The public runtime is Rust-native. The Rust CLI and Rust crates own storage,
recall, import/export, audit, consolidation, maintenance, and terminal UI
behavior. There is no tracked root Python package, Python wrapper layer, pytest
suite, or Python smoke script.

## First Example

```bash
tree-ring init
tree-ring remember "Use project-scoped recall before changing release behavior." \
  --event-type lesson \
  --scope project \
  --project example-service \
  --tag release \
  --tag workflow
tree-ring recall "release behavior" --project example-service
```

## CLI Preview

The `tree-ring` command is the Rust CLI.

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

## Optional CPython Extension

`bindings/python` is a Rust PyO3 crate for building a CPython extension from
the same Rust runtime. It is an optional host-adapter artifact, not the
canonical API and not a Python implementation.

```bash
cargo build -p tree-ring-memory-python --features extension-module
```

For package builds, run maturin from `bindings/python`. The repository does not
ship a root Python package or Python wrapper modules.

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
cargo run -p tree-ring-memory-cli -- --help
cargo run -p tree-ring-memory-cli -- tui --help
cargo run -p tree-ring-memory-cli -- export --help
cargo run -p tree-ring-memory-cli -- import --help
cargo run -p tree-ring-memory-cli -- audit --help
cargo run -p tree-ring-memory-cli -- consolidate --help
cargo run -p tree-ring-memory-cli -- maintain --help
cargo build -p tree-ring-memory-python --features extension-module
cargo run --release -p tree-ring-memory-sqlite --example performance_smoke -- 1000
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
- `docs/superpowers/specs/2026-07-05-tree-ring-memory-rust-only-repo-v0-9-design.md`
- `docs/superpowers/plans/2026-07-05-tree-ring-memory-rust-only-repo-v0-9-implementation-plan.md`

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
