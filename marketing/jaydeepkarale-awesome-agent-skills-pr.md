# Jaydeepkarale Awesome Agent Skills PR

## Target

- Repository: `https://github.com/jaydeepkarale/awesome-agent-skills`
- PR: `https://github.com/jaydeepkarale/awesome-agent-skills/pull/6`
- Fork: `https://github.com/TerminallyLazy/jaydeepkarale-awesome-agent-skills`
- Branch: `add-tree-ring-memory`
- Commit: `b7f04a345c9f0f3b240e4e738000f6fb2ecae9ad`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4910467123`

## Placement

- Files: `README.md`, `tree-ring-memory/.github/copilot-instructions.md`,
  `tree-ring-memory/CLAUDE.md`, `tree-ring-memory/AGENTS.md`,
  `tree-ring-memory/.cursorrules`, `tree-ring-memory/.windsurfrules`,
  `tree-ring-memory/sync.sh`
- README section: `Skills`
- Entry:
  `Tree Ring Memory | Local-first memory lifecycle guidance for AI agents with recall, evidence, forgetting, redaction, and audit habits`

## Validation

- Checked upstream PRs and issues for existing `Tree Ring Memory` references;
  no duplicate submission existed before this PR.
- Confirmed the canonical Tree Ring Memory skill URL returned HTTP `200`.
- Ran `tree-ring-memory/sync.sh` and verified Copilot source content matched
  the Claude, Codex, Cursor, and Windsurf copies.
- Ran `git diff --check`.
- Verified PR state: open, ready, mergeable.
- Check status: no checks reported.

## Notes

The target repository accepts reusable instruction packs rather than just link
entries. This PR therefore vendors a concise Tree Ring Memory usage skill in
the repo's cross-tool format and keeps the source link to the canonical Tree
Ring Memory repository.
