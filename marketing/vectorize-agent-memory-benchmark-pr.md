# Vectorize Agent Memory Benchmark PR

## Target

- Repository: `https://github.com/vectorize-io/agent-memory-benchmark`
- PR: `https://github.com/vectorize-io/agent-memory-benchmark/pull/25`
- Fork: `https://github.com/TerminallyLazy/agent-memory-benchmark`
- Branch: `add-tree-ring-provider`
- Commit: `00408ae3acd63127cef87189f353f86932a6e2fd`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4916501736`

## Placement

- Added `src/memory_bench/memory/tree_ring.py`.
- Registered `tree-ring` in `src/memory_bench/memory/__init__.py`.
- Added the static `catalog.json` provider entry.
- Added the README setup note for `TREE_RING_BIN`.

## Copy

The PR positions Tree Ring Memory as a runnable local provider for AMB rather
than a directory listing. It deliberately avoids claiming any benchmark score.

## Validation

- Checked upstream PRs and issues for existing Tree Ring Memory references.
- Built the local Tree Ring CLI with `cargo build -p tree-ring-memory-cli`.
- Ran `python3 -m compileall src/memory_bench/memory/tree_ring.py
  src/memory_bench/memory/__init__.py`.
- Ran `uv run omb providers` and verified `tree-ring` appears.
- Generated the AMB catalog and verified the `tree-ring` provider entry.
- Ran a local ingest and scoped recall smoke with
  `TREE_RING_BIN=/Users/lazy/Projects/Tree_Ring_Memory/target/debug/tree-ring`.
- Ran `git diff --check`.
- Verified PR state: open, ready, mergeable.
- Check status: no checks reported.

## Notes

This is a stronger outreach surface than another awesome-list entry because it
puts Tree Ring Memory into an apples-to-apples agent-memory benchmark harness.
The adapter uses JSONL import for benchmark documents, maps AMB `user_id` to
Tree Ring `project`, and uses `tree-ring recall --include-sensitive` for
personal-context benchmarks while leaving Tree Ring's secret-like import block
intact.
