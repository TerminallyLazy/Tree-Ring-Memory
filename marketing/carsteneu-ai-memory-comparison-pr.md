# AI Memory Comparison PR

Status: submitted
Submitted: 2026-07-07
Outlet: carsteneu/ai-memory-comparison
Public PR: https://github.com/carsteneu/ai-memory-comparison/pull/14
Fork branch: https://github.com/TerminallyLazy/ai-memory-comparison/tree/add-tree-ring-memory-evidence
External commit: 8847433df78ac88631de4169365bac2dbd97358b
Evidence comment: https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4909903832

## Submission Path

The AI Memory Comparison contribution guide asks new systems to submit a PR with
one evidence file at `evidence/<system-id>.md`. Maintainers create the data row
from the evidence file, so this PR intentionally commits only
`evidence/tree-ring-memory.md`.

## Fit

AI Memory Comparison is a source-backed feature comparison of memory systems for
AI coding agents. Tree Ring Memory fits as a local-first memory lifecycle layer
for AI agents with Rust CLI, SQLite/FTS recall, forgetting, audit,
consolidation, and portable agent guidance.

The submitted evidence is conservative:

- Only public README, docs, and source links are used.
- Checked claims are pinned to Tree Ring commit
  `6e734451f150704197fa872a3bb213a7e4cc3c33`.
- Unsupported items such as semantic/vector search, hidden auto-extraction,
  benchmarks, and undocumented platform bridges are marked absent.
- The PR discloses maintainer authorship.

## Validation

- `git diff --check` passed.
- `git diff --cached --check` passed.
- `node build.js` passed locally and reported `Built 79 systems x 79 features.`

`node build.js` regenerated `comparison.md` and `index.html` because the
upstream project refreshes live star counts. Those generated changes were
restored and not committed because the contribution guide asks for only the
evidence file on new-system PRs.

## PR State

Verified after creation:

- Open: yes
- Draft: yes
- Mergeable: yes
- Reported checks: none

## Follow-Up

Monitor maintainer response. If requested, convert the evidence into a
`data.js` row, rerun `node build.js`, and commit the generated `comparison.md`
and `index.html` changes in the same branch.
