# Tree Ring Memory Skill Repo

## Target

- Repository: `https://github.com/TerminallyLazy/tree-ring-memory-skill`
- Commit: `c8bbaea`
- Validation workflow:
  `https://github.com/TerminallyLazy/tree-ring-memory-skill/actions/runs/28923770033`
- Source project:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4912191916`

## Purpose

Create a root-level skill package for directories that expect direct installs
with:

```bash
git clone https://github.com/owner/skill-name ~/.claude/skills/skill-name
```

The existing Claude plugin wrapper remains useful for plugin marketplaces. This
repo is the cleaner source for catalogues that require a root `SKILL.md`.

## Contents

- `SKILL.md` copied from the canonical Tree Ring Memory skill package.
- `README.md` with personal and project-local install commands.
- `SECURITY.md` routing reports to the main project.
- `LICENSE` copied from the main project.
- `tree-ring-memory-logo.png` brand asset.
- GitHub Actions workflow validating frontmatter and secret-marker absence.

## Validation

- Local `SKILL.md` validation passed.
- `git diff --check` passed.
- GitHub Actions `Validate skill` passed.
- Repository topics set for Claude skills, agent skills, AI memory, local-first
  memory, and context engineering discovery.
