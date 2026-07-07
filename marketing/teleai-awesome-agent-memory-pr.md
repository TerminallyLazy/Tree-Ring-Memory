# TeleAI Awesome Agent Memory PR

## Outlet

- Directory: `TeleAI-UAGI/Awesome-Agent-Memory`
- Public PR: https://github.com/TeleAI-UAGI/Awesome-Agent-Memory/pull/53
- Fork: https://github.com/TerminallyLazy/Awesome-Agent-Memory
- Branch: `add-tree-ring-memory`
- Commits: `2740cdf`, `2553cf2`

## Submission

Added Tree Ring Memory to the `Products / Open-Source` section:

```markdown
47. **[Tree Ring Memory](https://terminallylazy.github.io/Tree-Ring-Memory/)**
      ![Star](https://img.shields.io/github/stars/TerminallyLazy/Tree-Ring-Memory.svg?style=social&label=Star)
      [[code](https://github.com/TerminallyLazy/Tree-Ring-Memory)]
      _Local-first memory lifecycle for AI agents with a Rust CLI, SQLite/FTS recall, audit, forgetting, consolidation, and Ratatui TUI._
```

## Fit

The target explicitly accepts open-source products whose primary purpose is
memory for LLM/MLLM agents. Tree Ring fits as a local-first memory lifecycle
layer for AI agents rather than a general vector database or RAG framework.

## Disclosure

The PR is submitted from the Tree Ring maintainer account and links to the
official homepage and GitHub repository.

## Validation

- Checked for existing Tree Ring Memory PRs and issues in the target repo.
- Verified the contribution guide accepts agent-memory products.
- Ran `git diff --check` against the PR branch.
- Ran `GITHUB_TOKEN=$(gh auth token) python3 scripts/check_star_order.py README.md`.
  Tree Ring now appears in the correct 3-star slot above PackRat. The script
  still exits non-zero because of pre-existing unrelated upstream order drift
  between Omnigraph/PowerMem and OMEGA/Mnemory plus two unreachable existing
  badge repos.
- Verified the PR is open, ready for review, mergeable, and has no reported
  checks after pushing the follow-up placement commit.
- Evidence comment:
  https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4910014796
