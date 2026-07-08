# Tree Ring Memory Claude Plugin Repo

## Target

- Repository: `https://github.com/TerminallyLazy/tree-ring-memory-claude-plugin`
- Purpose: installable Claude Code plugin wrapper for Tree Ring Memory
  local-first memory lifecycle guidance.
- Commit: `c6705795dd94b25861a7c87ae27678eb7620fe4d`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4911976708`

## Contents

- `.claude-plugin/plugin.json`
- `.claude-plugin/marketplace.json`
- `skills/tree-ring-memory/SKILL.md`
- `commands/tree-ring-recall.md`
- `commands/tree-ring-capture.md`
- `commands/tree-ring-audit.md`
- `assets/tree-ring-memory-logo.png`
- `assets/tree-ring-memory-banner.png`
- `README.md`
- `SECURITY.md`
- `LICENSE`
- `.github/workflows/validate.yml`

## Validation

- `claude plugin validate .claude-plugin/plugin.json --strict` passed.
- `claude plugin validate .claude-plugin/marketplace.json --strict` passed.
- `git diff --cached --check` passed before commit.
- Public GitHub Actions validation passed:
  `https://github.com/TerminallyLazy/tree-ring-memory-claude-plugin/actions/runs/28922230261`
- Repository topics set:
  `claude-code`, `claude-code-plugin`, `agent-memory`, `ai-memory`,
  `local-first`, `privacy`, `tree-ring-memory`, `llm-memory`,
  `developer-tools`.

## Notes

This wrapper keeps the main Tree Ring Memory framework repo canonical while
creating a clean Claude Code plugin and marketplace packaging surface for
Claude-oriented directories.
