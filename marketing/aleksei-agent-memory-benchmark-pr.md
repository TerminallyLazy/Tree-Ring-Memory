# Aleksei Agent Memory Benchmark PR

Date: 2026-07-08

Status: submitted as upstream PR #1.

URL: https://github.com/AlekseiMarchenko/agent-memory-benchmark/pull/1

Evidence comment: https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4916894140

## Fit

Aleksei's Agent Memory Benchmark explicitly welcomes new provider adapters and
tests memory systems across factual recall, semantic search, temporal reasoning,
conflict resolution, selective forgetting, cross-session continuity,
multi-agent collaboration, and cost efficiency. Tree Ring Memory fits because
its local-first CLI exposes JSON `remember`, `recall`, and `forget` surfaces
without requiring an API key or background service.

## Submission

The PR adds an optional Tree Ring Memory provider adapter:

- `src/adapters/tree-ring.ts` implements the benchmark `MemoryAdapter`
  contract through the local `tree-ring` CLI.
- `--provider tree-ring` and alias `--provider trm` are wired into the CLI.
- The package export surface exposes `TreeRingAdapter`.
- The adapter creates an isolated temporary Tree Ring root by default and
  removes it during cleanup.
- `TREE_RING_BIN`, `TREE_RING_ROOT`, and `TREE_RING_PROJECT` let maintainers
  point the benchmark at a specific binary, root, or shared project.
- Tree Ring scoped benchmark semantics are mapped through Tree Ring projects:
  agent-scoped memories use agent-specific projects and org-scoped memories use
  a shared benchmark project.
- Search uses direct Tree Ring recall first, then query-derived salient-term
  fallback recall so the CLI behaves like normal OR-style lexical retrieval
  without using expected benchmark answers.
- `amb-results-tree-ring/` records a local benchmark artifact with report,
  badge, and machine-readable JSON.

## Validation

- `npm ci`
- `npm run build`
- `npm test`
- `git diff --check`
- `TREE_RING_BIN=/Users/lazy/Projects/Tree_Ring_Memory/target/debug/tree-ring npm run dev -- --provider tree-ring --no-delay --layer all --layer3-scales 25 --output ./amb-results-tree-ring || true`

Local submitted result artifact:

- Layer 1: 61/100
- Layer 2: 40/100
- Layer 3: 48.6/100 at 25 distractors
- Factual recall: 100%
- Conflict resolution: 86%
- Selective forgetting: 83%
- Cross-session continuity: 71%
- Cost efficiency: 100%

These results are recorded as adapter smoke evidence for the submitted PR, not
as a broad public leaderboard claim.

## GitHub State

- Fork: `TerminallyLazy/agent-memory-benchmark-aleksei`
- Branch: `add-tree-ring-adapter`
- Commit: `9357ab33f418ac3d07eb3e8c11b341e22cbdb80e`
- PR state at submission: open, ready, mergeable, no status checks reported.
