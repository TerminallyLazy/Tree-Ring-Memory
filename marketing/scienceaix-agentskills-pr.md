# ScienceAIx AgentSkills PR

## Target

- Repository: `https://github.com/scienceaix/agentskills`
- PR: `https://github.com/scienceaix/agentskills/pull/16`
- Fork: `https://github.com/TerminallyLazy/scienceaix-agentskills`
- Branch: `add-tree-ring-memory`
- Commit: `7515b88dc919586ad957056e65438aec0d67d9e6`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4910489205`

## Placement

- File: `README.md`
- Section: `Open-Source Projects & Frameworks`
- New subsection: `Agent Memory & Skill Infrastructure`
- Entry:
  `Tree Ring Memory | Local-first memory lifecycle framework and portable skill for agent recall, evidence-linked learning, redaction, audit, and forgetting. Rust CLI with SQLite/FTS storage.`

## Validation

- Checked upstream PRs and issues for existing `Tree Ring Memory` references;
  no duplicate submission existed before this PR.
- Confirmed the Tree Ring Memory repository URL returned HTTP `200`.
- Ran `git diff --check`.
- Verified PR state: open, ready, mergeable.
- Check status: no checks reported.

## Notes

The list covers tools and frameworks for the skills-for-LLMs ecosystem. The PR
uses a specific memory-and-skill infrastructure subsection rather than placing
Tree Ring Memory in the generic agent-framework table.
