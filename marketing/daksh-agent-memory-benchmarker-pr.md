# Daksh Agent Memory Benchmarker PR

Date: 2026-07-08

Status: submitted as upstream PR #1.

URL: https://github.com/dakshjain-1616/agent-memory-benchmarker/pull/1

Evidence comment: https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4917005318

## Fit

Daksh Jain's Agent Memory Benchmarker compares memory backends across common
factual recall and contradiction detection tasks. Tree Ring Memory fits as an
optional local backend because it exposes write, recall, and clear behavior
through the public `tree-ring` CLI without requiring cloud credentials.

## Submission

The PR adds an optional Tree Ring Memory backend:

- `agent_memory_benchma/backends/tree_ring_backend.py` implements the local
  CLI-backed backend.
- `tree_ring` is registered only when `tree-ring` is on `PATH` or
  `TREE_RING_BIN` points to a binary, so default installs retain the original
  backend set.
- Each backend instance uses an isolated temporary Tree Ring root by default.
- `TREE_RING_ROOT` can point maintainers at an explicit root; in that mode,
  clear removes only memories the backend instance added.
- `TREE_RING_PROJECT` controls the Tree Ring project name.
- Search uses direct recall plus a query-derived fallback so benchmark queries
  can recover fact-oriented entries without using expected answers.
- README usage docs describe the optional backend and environment variables.
- Tests were updated to keep the default registry stable while also accepting
  the optional Tree Ring backend when installed.

## Validation

- `.venv/bin/python -m compileall agent_memory_benchma`
- `env -u TREE_RING_BIN PATH=/usr/bin:/bin .venv/bin/python -m pytest tests/test_backends.py::test_backend_registry_contains_all tests/test_benchmarker.py::TestBackendRegistry -q`
- `TREE_RING_BIN=/Users/lazy/Projects/Tree_Ring_Memory/target/debug/tree-ring .venv/bin/python -m pytest tests/test_backends.py::test_backend_registry_contains_all tests/test_benchmarker.py::TestBackendRegistry -q`
- Direct Tree Ring backend smoke: add, query, and clear passed; query returned
  `Alice likes cerulean blue.` with score `0.72`.
- `TREE_RING_BIN=/Users/lazy/Projects/Tree_Ring_Memory/target/debug/tree-ring .venv/bin/python demo.py --mock --backends tree_ring --tasks factual_recall contradiction_detection --no-pdf --output-dir outputs-tree-ring`

Local demo result with the mock evaluator:

- Factual recall accuracy: 0.334
- Contradiction detection accuracy: 0.205
- Mean accuracy: 0.269
- Average latency was about 30.8ms for factual recall and 23.2ms for
  contradiction detection.

These are recorded as smoke evidence for the submitted adapter, not as public
leaderboard claims.

## GitHub State

- Fork: `TerminallyLazy/agent-memory-benchmarker`
- Branch: `add-tree-ring-backend`
- Commit: `ec0b2fe12ae10fc0bce2e353ac144bdcde625b2d`
- PR state at submission: open, ready, mergeable, no status checks reported.
