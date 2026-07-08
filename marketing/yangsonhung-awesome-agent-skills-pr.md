# YangsonHung Awesome Agent Skills PR

## Target

- Repository: `https://github.com/YangsonHung/awesome-agent-skills`
- PR: `https://github.com/YangsonHung/awesome-agent-skills/pull/6`
- Fork: `https://github.com/TerminallyLazy/awesome-agent-skills-1`
- Branch: `add-tree-ring-memory`
- Commit: `4daea092a50fce94169b3ecb67038223b9b2ca4a`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4910601089`

## Placement

- Files: `skills/en/tree-ring-memory/SKILL.md`,
  `skills/zh-cn/tree-ring-memory-cn/SKILL.md`, `README.md`,
  `README.zh-CN.md`, `AGENTS.md`
- README placement: available skill tables and trigger examples in both
  language variants.
- Skill summary:
  `Use Tree Ring Memory for local-first AI agent memory lifecycle work:
  recall, evidence-linked lessons, privacy-safe capture, audit, redaction, and
  forgetting.`

## Validation

- Checked upstream PRs and issues for existing `Tree Ring Memory` references;
  no duplicate submission existed before this PR.
- Ran `python3 scripts/validate_skills.py --strict`.
- Ran `node scripts/validate-skills.js --strict`.
- Ran `git diff --check`.
- Scanned the English skill for Han characters; no matches.
- Verified PR state: open, ready, mergeable.
- Check status: no checks reported.

## Notes

The target repository requires paired English and Chinese skills. The submission
uses the same lifecycle-memory positioning as the Tree Ring launch materials
while adapting the content to the target repository's skill section contract.
