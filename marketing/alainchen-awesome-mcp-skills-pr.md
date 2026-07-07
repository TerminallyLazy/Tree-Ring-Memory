# Awesome MCP Skills Watchlist PR

Submission date: 2026-07-07

## Target

- Directory: https://github.com/alainchen/awesome-mcp-skills
- Pull request: https://github.com/alainchen/awesome-mcp-skills/pull/3
- Fork branch: https://github.com/TerminallyLazy/awesome-mcp-skills/tree/add-tree-ring-memory-watchlist
- Evidence comment: https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4909340027

## Submitted Entry

Placement: `data/watchlist.yml`

```yaml
- name: Tree Ring Memory
  service_scope: global
  status: unverified
  reason: First-party project publishes a portable SKILL.md-style agent memory package, but it is not an official platform-backed MCP server or skill ecosystem.
  last_checked: 2026-07-07
```

## Fit Rationale

The target repo is official-first. Tree Ring Memory has a first-party public
repo and portable `SKILL.md`-style agent memory package, but it is not an
official platform-backed MCP server or official skill ecosystem. The watchlist
is therefore the appropriate conservative placement.

## Validation

- `npm run validate:data`: passed.
- `npm run check`: passed.
- `git diff --check`: passed.
- GitHub PR state at submission: open draft, mergeable, no status checks
  reported.
