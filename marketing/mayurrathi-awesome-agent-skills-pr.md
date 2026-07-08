# Mayurrathi Awesome Agent Skills PR

## Target

- Repository: `https://github.com/mayurrathi/awesome-agent-skills`
- PR: `https://github.com/mayurrathi/awesome-agent-skills/pull/5`
- Fork: `https://github.com/TerminallyLazy/mayurrathi-awesome-agent-skills`
- Branch: `add-tree-ring-memory`
- Commit: `5140b1ee67d6c4d1b37d125a2b2a1ee8d224172b`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4910530735`

## Placement

- Files: `README.md`, `skills/tree-ring-memory/SKILL.md`
- README placement: install block near existing `agent-memory-mcp` and
  `agent-memory-systems` entries.
- Skill summary:
  `Use Tree Ring Memory for local-first AI agent memory lifecycle work: recall before risky tasks, evidence-linked learning, privacy-safe capture, redaction, audit, and forgetting.`

## Validation

- Checked upstream PRs and issues for existing `Tree Ring Memory` references;
  no duplicate submission existed before this PR.
- Confirmed the canonical Tree Ring Memory skill URL returned HTTP `200`.
- Validated local `SKILL.md` frontmatter keys.
- Ran `git diff --check`.
- Verified PR state: open, ready, mergeable.
- Check status: no checks reported.

## Notes

The repository's generator was not run because the current repository has a
flat `skills/` layout while `scripts/build_github_directory.py` removes and
recreates the directory. Running it would create broad unrelated churn, so the
PR adds only the new skill folder and matching README install block.
