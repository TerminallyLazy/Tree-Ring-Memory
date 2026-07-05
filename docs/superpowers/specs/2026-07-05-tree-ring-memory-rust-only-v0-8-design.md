# Tree Ring Memory Rust-Only v0.8 Design

v0.8 removes the remaining Python-owned runtime behavior from Tree Ring
Memory. Python remains a binding surface for agent workflows that want Python
ergonomics, but durable behavior is owned by Rust.

## Intent

Tree Ring Memory is a framework-agnostic memory lifecycle layer for AI agents.
The implementation should now match that purpose by making Rust the only
runtime owner for storage, recall, privacy checks, import/export, audit,
consolidation, maintenance, CLI, and TUI behavior.

## Goals

- Remove the Python reference backend from the public package.
- Remove Python-owned SQLite, recall ranking, sensitivity, audit,
  consolidation, import/export, and CLI behavior.
- Keep `TreeRingMemory` as a thin Python facade over the PyO3 native module.
- Keep lightweight Python dataclasses only for request/response conversion.
- Keep Python tests focused on wrapper marshalling and packaging boundaries.
- Keep Rust tests as the behavioral authority.
- Preserve the Rust CLI as the only `tree-ring` command.

## Non-Goals

- Do not remove Python bindings.
- Do not remove historical design documents that describe earlier migration
  phases.
- Do not change the memory schema.
- Do not introduce cloud services or framework-specific assumptions.

## Public API Shape

Python package exports:

- `TreeRingMemory`
- `NativeTreeRingMemory`
- `MemoryEvent`
- `MemorySource`
- `MemoryLink`
- `MemoryReview`
- `RecallResult`
- `ValidationError`

Removed exports:

- `PythonTreeRingMemory`
- `RustCliTreeRingMemory`

Removed modules:

- `tree_ring_memory.cli`
- `tree_ring_memory.store`
- `tree_ring_memory.recall`
- `tree_ring_memory.sensitivity`
- `tree_ring_memory.rust_backend`

## Compatibility Position

Previous Python reference tests become historical evidence, not active runtime
contract tests. Equivalent behavior is covered by Rust core, SQLite, CLI, and
PyO3 binding tests.

## Acceptance Criteria

1. Importing `tree_ring_memory` does not expose `PythonTreeRingMemory`.
2. The Python package has no Python-owned SQLite store, recall engine,
   sensitivity guard, or CLI module.
3. `TreeRingMemory.open()` still requires the Rust native binding.
4. `NativeTreeRingMemory` returns Python model objects for ergonomic use.
5. The Rust CLI remains the only supported `tree-ring` command.
6. `cargo test` passes.
7. `python3 -m pytest` passes with wrapper/packaging tests only.
8. Native binding smoke passes and reports version `0.8.0`.
9. README and architecture docs describe Python as a binding surface only.

