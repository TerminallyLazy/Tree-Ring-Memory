# Ammosu Awesome Claude Skills ZH-TW PR

## Submission

- Repository: `https://github.com/ammosu/awesome-claude-skills-zh-TW`
- PR: `https://github.com/ammosu/awesome-claude-skills-zh-TW/pull/7`
- Fork: `https://github.com/TerminallyLazy/awesome-claude-skills-zh-TW`
- Branch: `add-tree-ring-memory`
- Commit: `deaa057b7c81a5ad62b85f805781056ca8a25969`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4915977893`

## Placement

- `README.md`: `開發與程式工具`
- `README.en.md`: `Development & Code Tools`

## Copy

Traditional Chinese:

```markdown
- [Tree Ring Memory](https://github.com/TerminallyLazy/tree-ring-memory-skill) - 本地優先的 agent 記憶生命週期技能，協助 Claude 進行專案記憶召回、證據記錄、審計、遺忘與合併。*By [@TerminallyLazy](https://github.com/TerminallyLazy)*
```

English:

```markdown
- [Tree Ring Memory](https://github.com/TerminallyLazy/tree-ring-memory-skill) - Local-first agent memory lifecycle skill for project memory recall, evidence capture, audit, forgetting, and consolidation. *By [@TerminallyLazy](https://github.com/TerminallyLazy)*
```

## Validation

- Duplicate README search found no existing Tree Ring Memory entry.
- Duplicate GitHub PR and issue searches found no existing Tree Ring Memory
  submission in the target repo.
- Target skill repo returned HTTP `200`:
  `https://github.com/TerminallyLazy/tree-ring-memory-skill`
- `git diff --check` passed in the external PR checkout.
- Local smoke passed in isolated `/tmp/trm-ammosu-smoke`:
  `tree-ring init`, `remember --event-type decision --json`,
  `recall --json`, and `forget --json --reason`.
- Smoke memory id emitted and cleaned up:
  `mem_20260708_144004_94931d0631c5`.
- PR is open, ready, mergeable, and has no required status checks at submission
  time.

## Notes

This expands the Claude Skills wave into a Traditional Chinese catalogue while
pointing readers at the maintained standalone Tree Ring Memory skill repo
instead of vendoring a duplicate copy.
