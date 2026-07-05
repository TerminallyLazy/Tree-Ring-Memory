# Rust Core Roadmap

Tree Ring Memory should move toward a Rust core while keeping the protocol and host adapters framework-agnostic.

## Why Rust

Rust fits the long-term shape of Tree Ring Memory because the framework should be:

- embeddable in many agent harnesses
- fast for local recall and consolidation
- safe around privacy-sensitive local data
- predictable under concurrent CLI, server, and tool access
- portable across Python, Node, CLI, desktop, and sidecar deployments
- strict about schema boundaries and invalid state

SQLite and FTS remain the right default storage layer. Rust should own the lifecycle logic around it.

## Target Shape

```text
tree-ring-memory/
├── crates/
│   ├── tree-ring-memory-core/      # models, validation, privacy, recall ranking
│   ├── tree-ring-memory-sqlite/    # SQLite/FTS storage backend
│   └── tree-ring-memory-cli/       # native CLI
├── bindings/
│   ├── python/                     # optional Python package wrapper
│   └── node/                       # optional Node package wrapper
├── skills/
├── templates/
├── schemas/
└── docs/
```

## Migration Strategy

1. Preserve the original Python implementation only as historical migration
   evidence.
2. Keep Rust as the runtime owner for schema, sensitivity, storage, recall,
   forget, import/export, audit, consolidation, maintenance, CLI, and TUI.
3. Keep Python as a thin PyO3 binding package plus model conversion objects.
4. Add optional Node bindings after the Rust API stabilizes.

## Rust Core Requirements

The Rust core must support:

- memory event validation
- deterministic sensitivity checks
- SQLite creation and migrations
- FTS indexing and query escaping
- project, scope, ring, event type, supersession, and sensitivity filters
- recall ranking with explainable scores
- delete, redact, and supersede workflows
- JSON import/export compatibility with the published schemas

v0.4 implements the JSONL import/export baseline in Rust and exposes it through
the CLI and Python backends. Markdown exports, SQLite backups, and signed
bundles remain future extension points.

v0.5 implements deterministic local audit checks in Rust and exposes them
through SQLite, CLI, and native Python surfaces. Consolidation and automatic
repair remain future extension points.

v0.6 implements deterministic consolidation in Rust. It creates source-linked
summary memories without LLMs, persists idempotent consolidation records, and
keeps sensitive payloads out of generated summaries.

v0.7 implements Rust-owned maintenance. It plans expired-memory deletion,
secret-like redaction, protected-memory review, invalid-expiry review, and
SQLite FTS drift repair. Apply/repair behavior is explicit and Rust-owned; the
public `TreeRingMemory.open()` facade requires the native Rust binding.
Adapter-specific sync remains a future extension point.

v0.8 removes Python-owned runtime behavior. Python remains useful for agent
workflows through the native binding, but no durable behavior is implemented in
Python.

## Non-Goals

The Rust rewrite should not:

- introduce cloud services
- require an external vector database
- bind Tree Ring Memory to one agent framework
- change the public memory schema without migration support

## Decision

The framework direction is Rust-native runtime, adapter-friendly edges.
Python is a binding surface, not a runtime owner.
