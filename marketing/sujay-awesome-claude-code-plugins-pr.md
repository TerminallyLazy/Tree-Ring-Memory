# Sujay Awesome Claude Code Plugins PR

## Target

- Repository: `https://github.com/sujayjayjay/awesome-claude-code-plugins`
- PR: `https://github.com/sujayjayjay/awesome-claude-code-plugins/pull/2`
- Fork: `https://github.com/TerminallyLazy/sujay-awesome-claude-code-plugins`
- Branch: `add-tree-ring-memory`
- Commit: `28e7535a59d082c78d31382d5f0a6ad2ab52e037`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4912846478`

## Placement

- Plugin directory: `.claude-plugin/plugins/tree-ring-memory`
- Plugin manifest: `.claude-plugin/plugins/tree-ring-memory/plugin.json`
- Scripts:
  - `.claude-plugin/plugins/tree-ring-memory/tree-ring-recall.sh`
  - `.claude-plugin/plugins/tree-ring-memory/tree-ring-capture.sh`
  - `.claude-plugin/plugins/tree-ring-memory/tree-ring-audit.sh`
- Plugin README: `.claude-plugin/plugins/tree-ring-memory/README.md`
- Marketplace entry: `.claude-plugin/marketplace.json`
- Root README install table structure and usage examples

## Validation

- Checked upstream PRs and issues for existing `Tree Ring Memory`,
  `tree-ring-memory`, and `TerminallyLazy` references before opening the PR.
- Found no duplicate Sujay submission.
- Parsed `.claude-plugin/marketplace.json` as valid JSON.
- Parsed `.claude-plugin/plugins/tree-ring-memory/plugin.json` as valid JSON.
- Ran `bash -n` on all three scripts.
- Verified all three scripts are executable.
- Tested missing-CLI behavior with a restricted `PATH`; scripts exit `127` and
  print the Tree Ring Memory install URL.
- Checked for stray `.DS_Store`, `__pycache__`, and `.pyc` files.
- Ran `git diff --check` in the fork checkout.
- Verified PR state: open, ready, mergeable.
- Check status: no checks reported.

## Notes

The target repository prioritizes focused, high-value plugins over broad prompt
collections. This submission packages Tree Ring Memory as CLI-backed slash
commands rather than a prompt-only wrapper, matching the target's contribution
guidelines.
