# Astek Awesome Context Engineering PR

## Target

- Repository: `https://github.com/AstekGroup/awesome-context-engineering`
- PR: `https://github.com/AstekGroup/awesome-context-engineering/pull/10`
- Fork: `https://github.com/TerminallyLazy/astek-awesome-context-engineering`
- Branch: `add-tree-ring-memory`
- Commit: `621e274a583d80d46b6b29ea748e7ce0b13e845a`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4911259150`

## Placement

- Root README: added a `Mémoire & Persistance de Contexte` tool subsection.
- Toolkit README: added a memory/context-persistence section.
- New guide: `outils/memoire/tree-ring-memory.md`.

## Copy

The PR positions Tree Ring Memory as a local lifecycle-first memory layer for
AI agents, with recall, forgetting, audit, consolidation, evidence handling,
SQLite/FTS storage, DOX/Revolve adapters, and a Rust CLI/TUI. The French guide
also calls out `protocol-preview` status and the difference between agent
experience memory and document RAG.

## Validation

- Checked upstream PRs and issues for existing `Tree Ring Memory`,
  `Tree-Ring-Memory`, `tree-ring`, and `TerminallyLazy` references before
  opening the PR.
- Found no Tree Ring Memory duplicate.
- Ran `git diff --check` in the fork checkout.
- Verified the new local links from `README.md` and `outils/README.md` resolve
  to `outils/memoire/tree-ring-memory.md`.
- Verified PR state: open, ready, mergeable.
- Check status: no checks reported.

## Notes

This target is a French practical Context Engineering guide. A simple one-line
listing would have been weak, so the PR adds a practical guide page in the
existing Astek tool-page style.
