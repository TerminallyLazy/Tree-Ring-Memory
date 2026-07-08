# H-XX-D AMBIENT Benchmark PR

Date: 2026-07-08

Status: submitted as upstream PR #5.

URL: https://github.com/H-XX-D/ambient-benchmark/pull/5

Evidence comment: https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4916634939

## Fit

AMBIENT is a substrate-neutral agent-memory benchmark with an adapter contract
that requires observable store calls. Tree Ring Memory is a strong fit because
it can expose local-first memory writes and recall through the `tree-ring` CLI's
JSON output without changing benchmark internals or vendoring Tree Ring code.

## Submission

The PR adds an optional `tree-ring-cli` bridge:

- `adapters/tree-ring-cli-adapter.mjs` creates isolated Tree Ring roots per
  AMBIENT store.
- AMBIENT `write` maps to `tree-ring remember --json`.
- AMBIENT `query` maps to `tree-ring recall --json --include-sensitive`.
- Returned support/provenance uses Tree Ring memory summaries, IDs, timestamps,
  event type, and recall scores.
- Tree Ring policy refusals for synthetic secret-like turns return
  `accepted: false` instead of crashing the benchmark runner.

The adapter is optional and gated by `AMBIENT_TREE_RING_BIN` or `tree-ring` on
`PATH`, so upstream users without Tree Ring installed are not blocked.

## Validation

- `npm run verify:adapter:tree-ring`
- `node --check adapters/tree-ring-cli-adapter.mjs`
- `node --check scripts/verify-tree-ring-adapter-bridge.mjs`
- `git diff --check`
- `AMBIENT_TREE_RING_BIN=/Users/lazy/Projects/Tree_Ring_Memory/target/debug/tree-ring npm run verify:adapters:prereqs -- --out results/optional-tree-ring-prereqs.json`
- `AMBIENT_TREE_RING_BIN=/Users/lazy/Projects/Tree_Ring_Memory/target/debug/tree-ring npm run verify:adapters:matrix -- --adapters tree-ring-cli --out results/cross-adapter-tree-ring-smoke.json`

The focused matrix passed with 8 rows for `tree-ring-cli` on BEAM small using
AMBIENT's mock fixed reader/checker.

## GitHub State

- Fork: `TerminallyLazy/ambient-benchmark`
- Branch: `add-tree-ring-adapter`
- Commit: `c51423c38fbc52a191801558f4ee29e5f24b58f0`
- PR state at submission: open, ready, mergeable, no status checks reported.
