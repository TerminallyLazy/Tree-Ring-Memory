# Kenryu Awesome Claude Skills PR

## Target

- Repository: `https://github.com/kenryu42/awesome-claude-skills`
- PR: `https://github.com/kenryu42/awesome-claude-skills/pull/22`
- Fork: `https://github.com/TerminallyLazy/kenryu-awesome-claude-skills`
- Branch: `add-tree-ring-memory`
- Commit: `28babcdb4ffb6cd3ac31104747495febd2005953`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4915527023`

## Change

Adds the standalone `TerminallyLazy/tree-ring-memory-skill` package to the
Community Skills table.

Entry:

```markdown
| [TerminallyLazy/tree-ring-memory-skill](https://github.com/TerminallyLazy/tree-ring-memory-skill) | Guides Claude through local-first agent memory recall, evidence capture, audit, forgetting, and consolidation. |
```

## Validation

- Read target `README.md` and `CONTRIBUTING.md`.
- No existing Tree Ring Memory / `tree-ring` / `TerminallyLazy` PRs found.
- No existing Tree Ring Memory / `tree-ring` / `TerminallyLazy` issues found.
- Verified `https://github.com/TerminallyLazy/tree-ring-memory-skill` returns
  HTTP 200.
- `git diff --check` passed.
- PR is open, ready, and mergeable.
- CodeRabbit passed.

## Notes

The target catalogue is focused on Claude Skills. Linking the dedicated root
skill repo is a better fit than linking the main framework repo or a plugin
wrapper.
