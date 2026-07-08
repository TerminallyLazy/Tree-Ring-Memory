# Lobu Agent Memory Benchmark PR

Date: 2026-07-08

Status: submitted as upstream PR #1.

URL: https://github.com/lobu-ai/agent-memory-benchmark/pull/1

Evidence comment: https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4916736562

## Fit

The lobu-ai benchmark explicitly accepts new memory systems by PR and requires
adapters to use public client surfaces only. Tree Ring Memory fits because its
CLI exposes JSON write and recall paths without requiring direct SQLite access.

## Submission

The PR adds an optional Tree Ring Memory benchmark adapter:

- `adapters/tree_ring_adapter.py` implements the benchmark JSONL protocol.
- Ingest uses `tree-ring remember --json`.
- Retrieval uses `tree-ring recall --json --include-sensitive`.
- Benchmark step IDs are stored as tags and mapped back from recalled events
  for retrieval and citation scoring.
- Each benchmark run uses an isolated Tree Ring root.
- Tree Ring policy refusals for secret-like synthetic inputs are counted as
  policy refusals instead of bypassed.
- `configs/longmemeval-oracle-10.tree-ring.json` provides an opt-in smoke
  config and does not modify the target repo's default all-systems CI config.

## Validation

- `python3 -m py_compile adapters/tree_ring_adapter.py`
- `bun install`
- `bun run typecheck`
- `git diff --check`
- `TREE_RING_BIN=/Users/lazy/Projects/Tree_Ring_Memory/target/debug/tree-ring bun run scripts/run.ts --config configs/longmemeval-oracle-10.tree-ring.json`

Local smoke result on `longmemeval-oracle-10` with the deterministic extractive
answerer:

- Answer: 30.0%
- Retrieval: 95.0%
- Citation: 48.3%
- Average retrieval latency: 167ms

These are recorded as smoke evidence only, not as public leaderboard claims.

## GitHub State

- Fork: `TerminallyLazy/agent-memory-benchmark-1`
- Branch: `add-tree-ring-adapter`
- Commit: `46388cebefa015049741a441be73e852dbf6cdcc`
- PR state at submission: open, ready, mergeable, no status checks reported.
