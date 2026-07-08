# Alexbevi Awesome Agent Frameworks PR

## Submission

- Repository: `https://github.com/alexbevi/awesome-agent-frameworks`
- Pull request: https://github.com/alexbevi/awesome-agent-frameworks/pull/2
- Fork: `https://github.com/TerminallyLazy/awesome-agent-frameworks`
- Branch: `add-tree-ring-memory`
- Commit: `e0290c7b47ae519098027150254b32d4971f8088`
- Status: open, ready, mergeable
- Submitted: 2026-07-08
- Evidence: https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4914798989

## Positioning

Added Tree Ring Memory to `Memory / context system` in a curated list of
open-source agent frameworks and agent infrastructure.

The copy intentionally frames Tree Ring Memory as memory/context
infrastructure, not as a standalone autonomous agent framework:

- Local-first Rust CLI/TUI.
- SQLite/FTS recall.
- Evidence, audit, forgetting, and consolidation.
- AI-agent memory lifecycle work.

## Files Changed Upstream

- `README.md`

## Validation

- Duplicate issue and PR searches found no existing Tree Ring Memory
  submission before opening the PR.
- `rg -n "Tree Ring|Tree-Ring-Memory|tree-ring" README.md` shows exactly
  one intended README entry.
- `git diff --check` passed.
- `curl -I -L https://github.com/TerminallyLazy/Tree-Ring-Memory` returned
  HTTP `200`.
- GitHub reports the PR as `MERGEABLE` with no status checks.

## Follow-Up

- Monitor PR #2 for maintainer review.
- If merged, verify Tree Ring Memory remains in the `Memory / context system`
  section on the upstream default branch.
