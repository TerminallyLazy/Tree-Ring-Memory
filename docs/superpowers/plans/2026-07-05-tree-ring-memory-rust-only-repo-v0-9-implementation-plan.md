# Tree Ring Memory Rust-Only Repository v0.9 Implementation Plan

## Task 1: Remove Tracked Python Repository Surface

Files:

- delete `pyproject.toml`
- delete `src/tree_ring_memory/__init__.py`
- delete `src/tree_ring_memory/api.py`
- delete `src/tree_ring_memory/models.py`
- delete `src/tree_ring_memory/native_backend.py`
- delete `tests/test_cli.py`
- delete `tests/test_models.py`
- delete `tests/test_native_packaging.py`
- delete `scripts/native_binding_smoke.py`
- delete `scripts/rust_performance_smoke.py`

Work:

- Remove the root Python package and pytest surface.
- Keep `bindings/python` because it is a Rust PyO3 crate.
- Do not remove ignored Python cache files; they are local artifacts and not
  tracked.

Checks:

```bash
git ls-files '*.py'
```

## Task 2: Update Version And Docs

Files:

- `Cargo.toml`
- `Cargo.lock`
- `bindings/python/pyproject.toml`
- `README.md`
- `bindings/python/README.md`
- `docs/architecture/rust-core-status.md`
- `docs/architecture/rust-core-roadmap.md`
- `docs/integrations/agent-skill.md`

Work:

- Bump Rust workspace and optional binding metadata to `0.9.0`.
- Remove root Python package instructions.
- Replace Python smoke commands with Rust cargo commands.
- Present the optional CPython extension as Rust-built integration output, not
  the canonical repo API.

Checks:

```bash
cargo test
cargo build -p tree-ring-memory-python --features extension-module
```

## Task 3: Verify Rust-Only Repository Proof

Work:

- Run Rust-only development checks.
- Run release performance smoke through the Rust example.
- Confirm no tracked `.py` files remain.
- Confirm `.vscode/` and ignored build/cache files remain untracked.

Final checks:

```bash
cargo test
cargo run -q -p tree-ring-memory-cli -- --help
cargo run -q -p tree-ring-memory-cli -- tui --help
cargo build -p tree-ring-memory-python --features extension-module
cargo run --release -p tree-ring-memory-sqlite --example performance_smoke -- 10000
git ls-files '*.py'
git diff --check
```
