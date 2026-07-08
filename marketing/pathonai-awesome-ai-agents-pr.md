# PathOnAI Awesome AI Agents PR

## Submission

- Repository: `https://github.com/PathOnAIOrg/awesome-ai-agents`
- Pull request: https://github.com/PathOnAIOrg/awesome-ai-agents/pull/17
- Custom fork: `https://github.com/TerminallyLazy/pathonai-awesome-ai-agents`
- Branch: `add-tree-ring-memory`
- Commit: `a1a50e2ba8705810ec60a2bf1413100782d51f7f`
- Status: open, ready, mergeable
- Submitted: 2026-07-08
- Evidence: https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4914976770

## Positioning

Added Tree Ring Memory to a new `Agent Memory and Context` section in a
compact AI-agent materials list.

The copy intentionally avoids classifying Tree Ring Memory as a standalone
agent or generic agent framework:

- Local-first memory lifecycle infrastructure.
- Rust CLI/TUI.
- SQLite/FTS recall.
- Audit, redaction, forgetting, and consolidation.
- Framework-agnostic agent-memory support.

## Files Changed Upstream

- `README.md`

## Validation

- Duplicate issue and PR searches found no existing Tree Ring Memory
  submission before opening the PR.
- `git diff --check` passed.
- README contains exactly one Tree Ring Memory entry.
- `curl -I -L https://github.com/TerminallyLazy/Tree-Ring-Memory` returned
  HTTP `200`.
- GitHub reports the PR as `MERGEABLE` with no status checks.

## Follow-Up

- Monitor PR #17 for maintainer review.
- If merged, verify Tree Ring Memory remains in the `Agent Memory and Context`
  section on the upstream default branch.
