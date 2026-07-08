# Antigravity Awesome Skills PR

## Target

- Repository: `https://github.com/sickn33/antigravity-awesome-skills`
- PR: `https://github.com/sickn33/antigravity-awesome-skills/pull/791`
- Fork: `https://github.com/TerminallyLazy/antigravity-awesome-skills`
- Branch: `add-tree-ring-memory-skill`
- Commit: `f7d8c09`
- Evidence comments:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4911550979`
  and
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4911559908`

## Placement

- Path: `skills/tree-ring-memory/SKILL.md`
- Category: `development`
- Risk: `safe`
- Entry:
  `Tree Ring Memory` as a source-only installable skill for local-first agent
  memory lifecycle work: recall, evidence, audit, forgetting, consolidation,
  redaction, and scoped CLI/TUI usage without transcript dumping.

## Validation

- Checked upstream PRs and issues for existing `Tree Ring Memory`,
  `tree-ring`, or `TerminallyLazy` references before opening the PR.
- Ran `npm install`.
- Ran `npm run validate`.
- Ran `npm run security:docs`.
- Ran `npm test`.
- Ran `git diff --check`.
- Confirmed the committed PR changes only `skills/tree-ring-memory/SKILL.md`.
- Left generated `CATALOG.md`, `data/bundles.json`, and `data/catalog.json`
  changes uncommitted because the contribution guide asks community PRs to stay
  source-only.
- PR state: open, ready, mergeable.
- Check status: Socket Security project report, Socket Security pull request
  alerts, Snyk license, and Snyk security passed.

## Notes

This places Tree Ring Memory in a high-visibility cross-agent skills catalog
that serves Antigravity, Codex, Claude Code, Cursor, Gemini, OpenCode, Kiro,
GitHub Copilot, and related skill-loader workflows.
