# Hashgraph Awesome Codex Plugins PR

## Target

- Repository: `https://github.com/hashgraph-online/awesome-codex-plugins`
- PR: `https://github.com/hashgraph-online/awesome-codex-plugins/pull/279`
- Fork: `https://github.com/TerminallyLazy/awesome-codex-plugins`
- Branch: `add-tree-ring-memory-codex-plugin`
- Commit: `85c794586583dbe378be0dfab21fa66d2964b098`
- Source plugin repo:
  `https://github.com/TerminallyLazy/tree-ring-memory-codex-plugin`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4911409324`

## Placement

- Section: `Development & Workflow`
- Entry:
  `Tree Ring Memory` as local-first memory lifecycle guidance for Codex agents
  with recall, evidence-backed lessons, privacy-safe memory capture, audit,
  consolidation, and explicit forgetting.

## Validation

- Checked upstream PRs and issues for existing `Tree Ring Memory` duplicates.
- Ran target repo alphabetical check:
  `python3 scripts/check-alphabetical.py README.md`.
- Ran target repo remote plugin validator:
  `python3 scripts/validate-plugin-pr.py --base-ref upstream/main`.
- Ran `git diff --check`.
- Source plugin repo has a public passing HOL scanner run on `main`.
- PR state: open, ready, mergeable.
- Upstream fork workflows are `action_required` with no jobs because maintainer
  approval is required before they run on this fork PR.

## Notes

The PR also fixes a pre-existing two-line `Kreuzberg` / `Kreuzberg Cloud`
alphabetical ordering issue in the target README because the repository's
alphabetizer otherwise fails for unrelated existing drift.
