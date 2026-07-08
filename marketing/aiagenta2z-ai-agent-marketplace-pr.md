# AI Agent Marketplace PR

## Submission

- Repository: `https://github.com/aiagenta2z/ai-agent-marketplace`
- PR: `https://github.com/aiagenta2z/ai-agent-marketplace/pull/24`
- Fork: `https://github.com/TerminallyLazy/ai-agent-marketplace`
- Branch: `add-tree-ring-memory`
- Commit: `970778525519dc33ade952460201b3fa56cf6137`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4916244166`

## Placement

- `AGENT.md`: `AI AGENT MEMORY AI AGENT` category.
- Entry added at the top of the memory category before existing memory layers.

## Copy

```markdown
## [Tree Ring Memory TerminallyLazy](https://github.com/TerminallyLazy/Tree-Ring-Memory)
![thumbnail_picture](https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/assets/tree-ring-memory-logo.png)

Local-first memory lifecycle framework for AI agents with a Rust CLI/TUI, SQLite/FTS recall, audit trails, forgetting, and deterministic consolidation.
```

## Validation

- Duplicate `AGENT.md` search found no existing Tree Ring Memory entry.
- Duplicate GitHub PR and issue searches found no existing Tree Ring Memory
  submission in the target repo.
- Target project repo, landing page, thumbnail, and media-kit URLs returned HTTP
  `200`.
- `git diff --check` passed in the external PR checkout.
- Entry field check found `Website`, `Description`, `Category`, `Tags`,
  `Reviews`, and `Links`.
- Local smoke passed in isolated `/tmp/trm-aiagenta2z-smoke`:
  `tree-ring init`, `remember --event-type decision --json`,
  `recall --json`, and `forget --json --reason`.
- Smoke memory id emitted and cleaned up:
  `mem_20260708_150740_a28b863d81f4`.
- PR is open, ready, mergeable, and has no required status checks at submission
  time.

## Notes

This places Tree Ring Memory in a broad AI Agent Marketplace as AI-agent memory
infrastructure. The PR body explicitly states that Tree Ring is not a
standalone autonomous agent.
