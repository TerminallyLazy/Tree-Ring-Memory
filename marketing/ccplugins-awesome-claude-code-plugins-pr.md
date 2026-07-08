# CCPlugins Awesome Claude Code Plugins PR

## Target

- Repository: `https://github.com/ccplugins/awesome-claude-code-plugins`
- PR: `https://github.com/ccplugins/awesome-claude-code-plugins/pull/303`
- Fork: `https://github.com/TerminallyLazy/awesome-claude-code-plugins`
- Branch: `add-tree-ring-memory`
- Commit: `5dc3878`
- Source plugin repo:
  `https://github.com/TerminallyLazy/tree-ring-memory-claude-plugin`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4911976708`

## Placement

- Section: `Workflow Orchestration`
- Entry:
  `tree-ring-memory` as local-first memory lifecycle guidance for Claude Code
  using Tree Ring Memory.

## Validation

- Checked upstream PRs and issues for existing `Tree Ring Memory`,
  `tree-ring`, or `TerminallyLazy` references; no duplicates were found.
- Created the public source plugin repo and verified its validation workflow
  passed.
- Ran `claude plugin validate .claude-plugin/plugin.json --strict` in the
  source plugin repo.
- Ran `claude plugin validate .claude-plugin/marketplace.json --strict` in the
  source plugin repo.
- Ran target repo marketplace validation:
  `claude plugin validate .claude-plugin/marketplace.json`.
- Target validation passed with the existing upstream `metadata.homepage`
  warning.
- Checked duplicate plugin names in the target marketplace.
- Ran `git diff --check`.
- PR state: open, ready, mergeable, with no checks reported.

## Notes

The target repo already has a strict-mode marketplace warning for
`metadata.homepage`. The new Tree Ring Memory entry passes the non-strict
validation path used for the repository's current manifest baseline.
