# GetStream AI Agent Tools Catalog PR

## Submission

- Repository: `https://github.com/GetStream/ai-agent-tools-catalog`
- PR: `https://github.com/GetStream/ai-agent-tools-catalog/pull/15`
- Fork: `https://github.com/TerminallyLazy/ai-agent-tools-catalog`
- Branch: `add-tree-ring-memory`
- Commit: `e9d20aa3c86bc866af507e6ccbaf57bfaaba0d4a`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4916160526`

## Placement

- `README.md`: `Database` tools table.
- Row added between SQL database tooling and vector database tooling.

## Copy

```markdown
| [Tree Ring Memory](https://github.com/TerminallyLazy/Tree-Ring-Memory) | Local-first memory lifecycle layer for AI agents with SQLite/FTS recall, audit, forgetting, and consolidation | Free |
```

## Validation

- Duplicate README search found no existing Tree Ring Memory entry.
- Duplicate GitHub PR and issue searches found no existing Tree Ring Memory
  submission in the target repo.
- Target project repo returned HTTP `200`:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory`
- `git diff --check` passed in the external PR checkout.
- Local smoke passed in isolated `/tmp/trm-getstream-smoke`:
  `tree-ring init`, `remember --event-type decision --json`,
  `recall --json`, and `forget --json --reason`.
- Smoke memory id emitted and cleaned up:
  `mem_20260708_145911_5d360bd512fc`.
- PR is open, ready, mergeable, and has no required status checks at submission
  time.

## Notes

This places Tree Ring Memory in a compact AI-agent tools catalog as free
database-adjacent memory lifecycle infrastructure, not as a standalone
assistant or autonomous agent.
