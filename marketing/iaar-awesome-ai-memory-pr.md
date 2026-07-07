# IAAR Awesome AI Memory PR

Submission date: 2026-07-07

## Target

- Directory: https://github.com/IAAR-Shanghai/Awesome-AI-Memory
- Canonical pull request: https://github.com/IAAR-Shanghai/Awesome-AI-Memory/pull/115
- Fork branch: https://github.com/TerminallyLazy/Awesome-AI-Memory/tree/add-tree-ring-memory
- Duplicate cleanup: https://github.com/IAAR-Shanghai/Awesome-AI-Memory/pull/116
- Evidence correction: https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4909388735

## Submitted Entry

Placement: `Systems and Open Sources` in both `README.md` and `README_cn.md`.

```markdown
| Tree Ring Memory | 2026-07-07 | ![GitHub Repo stars](https://img.shields.io/github/stars/TerminallyLazy/Tree-Ring-Memory?style=social) | https://github.com/TerminallyLazy/Tree-Ring-Memory<br>https://terminallylazy.github.io/Tree-Ring-Memory/ |
```

The canonical PR also updates the `Open Source Projects` badge count from `107`
to `108` in both README files.

## Fit Rationale

The target repo explicitly includes open-source frameworks and tools for
memory-enhanced LLMs. Tree Ring Memory fits as a local-first memory lifecycle
layer for AI agents with Rust CLI/TUI, SQLite/FTS recall, forgetting, audit,
consolidation, and project-scoped agent-memory workflows.

## Duplicate Cleanup

During a later outreach pass, duplicate PR #116 was opened against the same
target. It was closed with a maintainer-facing note pointing back to #115. The
useful incremental badge-count update from #116 was pushed onto #115 instead.

## Validation

- `git diff --check`: passed.
- Structural check confirmed exactly one Tree Ring Memory row in `README.md`.
- Structural check confirmed exactly one Tree Ring Memory row in `README_cn.md`.
- Structural check confirmed `Open Source Projects` is `108` and `Papers`
  remains `540` in both README files.
- GitHub PR state after the incremental update: open, ready for review,
  mergeable, no status checks reported.
