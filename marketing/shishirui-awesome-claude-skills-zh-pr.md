# Shishirui Awesome Claude Skills ZH PR

## Target

- Repository: `https://github.com/shishirui/awesome-claude-skills-zh`
- PR: `https://github.com/shishirui/awesome-claude-skills-zh/pull/8`
- Fork: `https://github.com/TerminallyLazy/shishirui-awesome-claude-skills-zh`
- Branch: `add-tree-ring-memory`
- Commit: `cea2f556b4aef93ab109330f536d7c212cf599f2`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4915777239`

## Change

Adds the standalone `TerminallyLazy/tree-ring-memory-skill` package to the
Chinese-first Claude Skills catalogue under `开发辅助`.

Entry:

```markdown
- **[tree-ring-memory](https://github.com/TerminallyLazy/tree-ring-memory-skill)** — 本地优先的 agent 记忆生命周期 skill，指导 Claude 进行项目记忆召回、证据记录、审计、遗忘与合并。
  **什么时候用：** 做长期项目、代码代理交接、复盘已验证决策或踩坑，并且需要可删除、可审计的本地记忆时。
  **作者：** [@TerminallyLazy](https://github.com/TerminallyLazy)　**标签：** `记忆` `本地优先` `审计` `遗忘`
```

## Validation

- Read target `README.md`, `CONTRIBUTING.md`, PR template, and issue template.
- No existing Tree Ring Memory / `tree-ring` / `TerminallyLazy` PRs found.
- No existing Tree Ring Memory / `tree-ring` / `TerminallyLazy` issues found.
- No existing Tree Ring Memory / `tree-ring` / `TerminallyLazy` README entry
  found.
- Verified `https://github.com/TerminallyLazy/tree-ring-memory-skill` returns
  HTTP 200.
- Verified the standalone skill repo contains `SKILL.md`.
- Ran a local isolated smoke at `/tmp/trm-shishirui-smoke` covering
  `tree-ring init`, `tree-ring remember`, `tree-ring recall`, and
  `tree-ring forget` with the actual emitted memory id.
- `git diff --check` passed.
- PR is open, ready, and mergeable.
- No required status checks were reported at submission time.

## Notes

The target catalogue prefers Chinese descriptions and requires a direct
use-case explanation. The entry avoids "best" claims and frames Tree Ring as a
local, auditable lifecycle-memory skill for long-running project work.
