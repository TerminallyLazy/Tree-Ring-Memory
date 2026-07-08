# Tree Ring Memory Codex Plugin Repo

## Target

- Repository: `https://github.com/TerminallyLazy/tree-ring-memory-codex-plugin`
- Purpose: installable Codex plugin wrapper for Tree Ring Memory local-first
  memory lifecycle guidance.
- Commit: `6b2842d4a733d0b007df528f7a8eb33f0180ff38`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4911409324`

## Contents

- `.codex-plugin/plugin.json`
- `skills/tree-ring-memory/SKILL.md`
- `assets/icon.svg`
- `README.md`
- `SECURITY.md`
- `LICENSE`
- `.codexignore`
- `.github/workflows/hol-plugin-scanner.yml`
- `.github/dependabot.yml`

## Validation

- Local HOL `plugin-scanner` v2.0.1004 score: `97/100`.
- Finding counts: no critical, high, medium, or low findings.
- Public scanner run passed:
  `https://github.com/TerminallyLazy/tree-ring-memory-codex-plugin/actions/runs/28917677294`
- Scanner job passed:
  `https://github.com/TerminallyLazy/tree-ring-memory-codex-plugin/actions/runs/28917677294/job/85787954750`
- Repository topics set:
  `codex-plugin`, `codex-skills`, `agent-memory`, `ai-memory`,
  `local-first`, `tree-ring-memory`, `coding-agents`, `llm-memory`,
  `developer-tools`, `privacy`.

## Notes

This wrapper keeps the main Tree Ring Memory framework repo canonical while
creating a clean Codex-plugin packaging surface for plugin marketplaces.
