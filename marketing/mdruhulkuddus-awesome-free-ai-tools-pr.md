# Awesome Free AI Tools PR

## Submission

- Repository: `https://github.com/mdruhulkuddus/awesome-free-ai-tools`
- Pull request: https://github.com/mdruhulkuddus/awesome-free-ai-tools/pull/19
- Fork: `https://github.com/TerminallyLazy/awesome-free-ai-tools`
- Branch: `add-tree-ring-memory`
- Commit: `11fa5081b30c28039a079cd014e536ee2d8a436c`
- Status: open, ready, mergeable
- Submitted: 2026-07-08
- Evidence: https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4914734029

## Positioning

Added Tree Ring Memory to `AI-powered dev tools` as a free MIT open-source
Rust CLI/TUI for local-first AI-agent memory lifecycle work.

The copy intentionally frames Tree Ring Memory as developer infrastructure,
not a standalone chatbot:

- Free: MIT open-source, local SQLite storage, no hosted service required.
- Best for: developers adding persistent memory to local agent workflows.
- Catch: early-stage and CLI-first.

## Files Changed Upstream

- `README.md`
- `data/tools.json`
- `index.html`

## Validation

- Duplicate issue and PR searches found no existing Tree Ring Memory
  submission before opening the PR.
- `python3` JSON parse and rendered-count check passed: one Tree Ring Memory
  entry in `dev-tools`, rendered total `232`, metadata total `232`.
- `rg -i "tree ring|tree-ring" README.md data/tools.json index.html script.js`
  shows only the intended README and JSON entries.
- `git diff --check` passed.
- `curl -I -L https://github.com/TerminallyLazy/Tree-Ring-Memory` returned
  HTTP `200`.
- GitHub reports the PR as `MERGEABLE` with no status checks.

## Follow-Up

- Monitor PR #19 for maintainer review.
- If merged, check the GitHub Pages directory search/filter for the Tree Ring
  Memory card after deployment.
