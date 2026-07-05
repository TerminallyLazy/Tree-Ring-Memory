# Tree Ring Memory Rust-Only Repository v0.9 Design

v0.9 removes tracked Python source, tests, and smoke scripts from the canonical
repository. Tree Ring Memory remains framework-agnostic, but the implementation
surface becomes Rust-first end to end.

## Intent

Tree Ring Memory is a framework-agnostic memory lifecycle layer for AI agents.
The repository should now reflect that by making Rust the only tracked
implementation language. Host integrations can still bind to Rust later, but
the core project should not ship a root Python package, Python wrappers, Python
tests, or Python smoke scripts.

## Goals

- Remove the root Python package metadata.
- Remove tracked Python wrapper modules.
- Remove tracked Python tests.
- Remove Python smoke scripts.
- Keep the optional CPython extension crate as Rust code under
  `bindings/python`.
- Keep performance smoke coverage through the Rust example.
- Keep native binding smoke coverage through Rust unit tests and cargo build.
- Update README and architecture docs to present Rust as the canonical runtime
  and CLI as the primary integration surface.

## Non-Goals

- Do not remove the Rust PyO3 crate.
- Do not remove historical docs that describe the Python prototype or migration
  phases.
- Do not change memory schemas.
- Do not add a new non-Rust host adapter.

## Public Surface

Canonical supported surfaces:

- `tree-ring` Rust CLI
- `tree-ring tui` Ratatui operator console
- Rust crates:
  - `tree-ring-memory-core`
  - `tree-ring-memory-sqlite`
  - `tree-ring-memory-cli`
- Optional Rust-built CPython extension crate:
  - `tree-ring-memory-python`

Removed canonical surfaces:

- root `tree-ring-memory` Python package
- Python dataclass wrapper API
- Python pytest suite
- Python smoke scripts

## Acceptance Criteria

1. `git ls-files '*.py'` returns no tracked Python source files.
2. Root `pyproject.toml` is removed.
3. README development checks are Rust-only.
4. Architecture status says Python is optional binding output, not repository
   source.
5. `cargo test` passes.
6. `cargo build -p tree-ring-memory-python --features extension-module` passes.
7. `cargo run --release -p tree-ring-memory-sqlite --example performance_smoke
   -- 10000` passes.
8. `git diff --check` passes.
