# Mdsakalu Agent Plugins PR

## Target

- Repository: `https://github.com/mdsakalu/agent-plugins`
- Submission type: plugin marketplace PR
- PR: `https://github.com/mdsakalu/agent-plugins/pull/1`
- Fork branch:
  `https://github.com/TerminallyLazy/agent-plugins/tree/add-tree-ring-memory`
- Head commit: `36bcf70d1e149cd52cc8cbf41c50b8ca87774578`
- Central evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4913697560`
- Current state: open, ready, mergeable, no status checks reported

## What Changed

- Added `tree-ring-memory/.claude-plugin/plugin.json`.
- Added `tree-ring-memory/skills/tree-ring-memory/SKILL.md`.
- Added `tree-ring-memory/skills/tree-ring-memory/README.md`.
- Added `tree-ring-memory` to `.claude-plugin/marketplace.json`.
- Added README install command, table row, and plugin detail section.

## PR Body

```markdown
## Summary

Adds `tree-ring-memory` as an installable Claude Code marketplace plugin.

The plugin packages Tree Ring Memory's memory-lifecycle guidance as a skill for:

- local-first recall
- deliberate capture
- evidence-backed lessons
- audit and consolidation
- redaction, deletion, and lifecycle-aware forgetting

It points back to the canonical framework and portable root skill repositories:

- https://github.com/TerminallyLazy/Tree-Ring-Memory
- https://github.com/TerminallyLazy/tree-ring-memory-skill

## Validation

- `python3 -m json.tool .claude-plugin/marketplace.json`
- `python3 -m json.tool tree-ring-memory/.claude-plugin/plugin.json`
- checked marketplace plugin names for duplicates
- checked `tree-ring-memory/skills/tree-ring-memory/SKILL.md` frontmatter basics
- `git diff --check`

## Disclosure

I maintain Tree Ring Memory.
```

## Validation

- Checked open issues, PRs, and code search for `Tree Ring Memory` and
  `tree-ring-memory`; no existing submission found.
- Confirmed target marketplace format uses `.claude-plugin/plugin.json` plus
  skill directories.
- JSON validation passed for marketplace and plugin manifests.
- Marketplace plugin-name duplicate check passed.
- Skill frontmatter basics passed.
- `git diff --check` passed.
