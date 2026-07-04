# Tree Ring Memory

Tree Ring Memory is a framework-agnostic memory lifecycle layer for AI agents.

It helps agents remember useful decisions, warnings, preferences, and lessons without turning memory into a transcript dump. Fresh memory stays detailed, older memory compresses into rings, important scars remain visible, and durable truths become heartwood.

## v0.1 Status

This repository is in protocol-preview status. The first implementation target is a local Python reference library with SQLite storage and no required cloud services.

## First Example

```python
from tree_ring_memory import TreeRingMemory

memory = TreeRingMemory.open(".tree-ring")
event = memory.remember(
    summary="Use Store Gate before reading Agent Zero frontend stores.",
    event_type="lesson",
    scope="project",
    project="agent-zero",
    tags=["frontend", "agent-zero"],
)

results = memory.recall("frontend store initialization", project="agent-zero")
for result in results:
    print(result.memory.summary, result.score)
```

## Design Docs

- `docs/superpowers/specs/2026-07-04-tree-ring-memory-framework-design.md`
- `docs/feature/tree-ring-memory-framework/diverge/options-raw.md`

## Principles

- Local-first by default.
- Protocol before adapters.
- Explainable recall.
- Sensitive data fails closed.
- Forgetting and supersession are first-class.
- Memory quality should be testable.
