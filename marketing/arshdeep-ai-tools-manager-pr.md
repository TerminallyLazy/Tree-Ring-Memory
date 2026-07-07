# Arshdeep AI Tools Manager PR

## Outlet

- Directory: `ArshdeepGrover/ai-tools-manager`
- Public PR: https://github.com/ArshdeepGrover/ai-tools-manager/pull/193
- Fork: https://github.com/TerminallyLazy/ai-tools-manager
- Branch: `agent/add-tree-ring-memory`
- Commit: `b822df4`

## Submission

Added Tree Ring Memory to `AI Coding & Development`:

```json
{
  "title": "Tree Ring Memory",
  "url": "https://terminallylazy.github.io/Tree-Ring-Memory/",
  "description": "Local-first memory lifecycle for AI coding agents"
}
```

Also added the required contributor record for `TerminallyLazy`.

## Fit

The target accepts free or freemium AI tools. Tree Ring fits as a free
open-source developer tool for AI coding agents: local-first memory lifecycle
infrastructure with recall, redaction, audit, and framework adapters.

## Disclosure

The PR body discloses affiliation with Tree Ring Memory.

## Validation

- `npm run validate-json` passed.
- Targeted duplicate check passed for the new Tree Ring title and URL.
- The Tree Ring launch page returned HTTP `200`.
- `git diff --check` passed.
- Verified the PR is open, draft, and mergeable at submission time.

## Caveats

- `npm run check-duplicates` fails on pre-existing upstream data: duplicate
  `ImagineClip`.
- `node scripts/validate-contributors.js --quick` fails on pre-existing
  upstream contributor data: LinkedIn format, missing roles, and duplicate
  `chugzb`.
- Vercel status contexts report authorization failures for the target project.
