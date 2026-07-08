# Orkas Awesome AgentSkills PR

## Target

- Repository: `https://github.com/Orkas-AI/Orkas-Awesome-AgentSkills`
- PR: `https://github.com/Orkas-AI/Orkas-Awesome-AgentSkills/pull/4`
- Fork: `https://github.com/TerminallyLazy/orkas-awesome-agentskills`
- Branch: `add-tree-ring-memory`
- Commit: `38f8af62aa84978c8d130b4d22540e144e90b048`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4910559959`

## Placement

- Files: `README.md`, `README.zh-CN.md`,
  `data/skills/tree-ring-memory/SKILL.md`
- README placement: Data category in both English and Chinese catalogs.
- Skill summary:
  `Use Tree Ring Memory when an agent needs a local-first memory lifecycle:
  recall before decisions, evidence-linked lessons, redaction, audit, and
  intentional forgetting.`

## Validation

- Checked upstream PRs and issues for existing `Tree Ring Memory` references;
  no duplicate submission existed before this PR.
- Confirmed the canonical Tree Ring Memory skill URL returned HTTP `200`.
- Validated the new skill frontmatter and both catalog references.
- Ran `git diff --check`.
- Verified PR state: open, ready, mergeable.
- Check status: no checks reported.

## Notes

This submission packages Tree Ring Memory as a bilingual agent skill instead of
only a link listing. The catalog count moved from 45 to 46 total skills and the
Data category moved from 6 to 7 skills in both README variants.
