# Helloianneo Awesome Claude Code Skills PR

## Target

- Repository: `https://github.com/helloianneo/awesome-claude-code-skills`
- PR: `https://github.com/helloianneo/awesome-claude-code-skills/pull/56`
- Fork: `https://github.com/TerminallyLazy/awesome-claude-code-skills`
- Branch: `add-tree-ring-memory`
- Commit: `e64a610`
- Source skill repo:
  `https://github.com/TerminallyLazy/tree-ring-memory-claude-plugin`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4912029243`

## Placement

- Section: `工作流 / 方法论`
- Entry:
  `Tree Ring Memory` as a local-first agent memory lifecycle skill for recall,
  capture, audit, and explicit forgetting.

## Validation

- Checked upstream PRs and issues for existing `Tree Ring Memory`,
  `tree-ring`, or `TerminallyLazy` references; no duplicates were found.
- Ran `npx skills add TerminallyLazy/tree-ring-memory-claude-plugin -l`;
  the CLI found the `tree-ring-memory` skill.
- Ran project-scope install in an isolated temporary project:
  `npx skills add TerminallyLazy/tree-ring-memory-claude-plugin@tree-ring-memory -y --copy`.
- Ran `npx skills use TerminallyLazy/tree-ring-memory-claude-plugin@tree-ring-memory`;
  the skill content resolved.
- Ran `git diff --check`.
- PR state: open, ready, mergeable, with no checks reported.

## Notes

The row uses the target repository's preferred `owner/repo@skill` install
syntax and labels the recommendation `可选` to avoid overstating adoption.
