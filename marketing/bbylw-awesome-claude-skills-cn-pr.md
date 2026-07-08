# Bbylw Awesome Claude Skills CN PR

## Target

- Repository: `https://github.com/bbylw/awesome-claude-skills-cn`
- PR: `https://github.com/bbylw/awesome-claude-skills-cn/pull/8`
- Fork: `https://github.com/TerminallyLazy/bbylw-awesome-claude-skills-cn`
- Branch: `add-tree-ring-memory`
- Commit: `94269707465f4d6b69e6630ffa564c6889e42a36`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4915863622`
- CodeRabbit evidence update:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4915886160`

## Change

Adds the standalone `TerminallyLazy/tree-ring-memory-skill` package to a
simplified-Chinese Claude Skills catalogue under `开发与代码工具`, and keeps the
static `index.html` catalogue page in sync.

README entry:

```markdown
- [Tree Ring Memory](https://github.com/TerminallyLazy/tree-ring-memory-skill) - 本地优先的 agent 记忆生命周期技能，指导 Claude 进行项目记忆召回、证据记录、审计、遗忘与合并。*作者：[@TerminallyLazy](https://github.com/TerminallyLazy)*
```

## Validation

- Read target `README.md` and inspected `index.html`.
- No existing Tree Ring Memory / `tree-ring` / `TerminallyLazy` PRs found.
- No existing Tree Ring Memory / `tree-ring` / `TerminallyLazy` issues found.
- No existing Tree Ring Memory / `tree-ring` / `TerminallyLazy` README or
  static-page entry found.
- Verified `https://github.com/TerminallyLazy/tree-ring-memory-skill` returns
  HTTP 200.
- Ran a local isolated smoke at `/tmp/trm-bbylw-smoke` covering
  `tree-ring init`, `tree-ring remember`, `tree-ring recall`, and
  `tree-ring forget` with the actual emitted memory id.
- `git diff --check` passed.
- PR is open, ready, and mergeable.
- CodeRabbit passed.

## Notes

The target repository has both a Markdown list and a static HTML catalogue.
Updating both avoids a visible mismatch between the GitHub README and hosted
page.
