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

1. Preserve the current Python implementation as the executable reference.
2. Add a Rust workspace with equivalent schema, sensitivity, storage, recall, and forget behavior.
3. Create parity fixtures shared by Python and Rust.
4. Move the CLI to Rust once parity tests pass.
5. Keep Python as bindings or compatibility package, not the long-term core.
6. Add optional Node bindings after the Rust API stabilizes.

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

## Non-Goals

The Rust rewrite should not:

- introduce cloud services
- require an external vector database
- bind Tree Ring Memory to one agent framework
- remove Python before parity exists
- change the public memory schema without migration support

## Decision

The framework direction is Rust-first core, adapter-friendly edges.
The current Python package remains useful as a protocol reference and compatibility layer during migration.
