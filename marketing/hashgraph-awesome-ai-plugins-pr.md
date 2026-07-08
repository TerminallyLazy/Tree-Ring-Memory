# Hashgraph Awesome AI Plugins PR

## Target

- Repository: `https://github.com/hashgraph-online/awesome-ai-plugins`
- PR: `https://github.com/hashgraph-online/awesome-ai-plugins/pull/59`
- Fork: `https://github.com/TerminallyLazy/awesome-ai-plugins`
- Branch: `add-tree-ring-memory-plugin`
- Commit: `5701b6c`
- Evidence comment:
  `https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4911496648`

## Placement

- Section: `Development & Workflow`
- Entry:
  `Tree Ring Memory` as a Codex plugin wrapper for local-first agent memory
  lifecycle guidance: scoped recall, explicit writes, evidence records, audit,
  forgetting, and consolidation without transcript dumping.

## Validation

- Checked upstream PRs for existing `Tree Ring Memory`, `tree-ring`, and
  `TerminallyLazy` duplicates.
- Verified the plugin repo link returned HTTP `200`.
- Verified the raw `.codex-plugin/plugin.json` manifest link returned HTTP
  `200`.
- Ran `python3 scripts/generate_plugins_json.py`.
- Ran `python3 scripts/check-alphabetical.py README.md`.
- Ran `git diff --check`.
- Ran `python3 -m json.tool plugins.json`.
- Ran `python3 -m json.tool .agents/plugins/marketplace.json`.
- Confirmed the generated `plugins.json` and `.agents/plugins/marketplace.json`
  each contain exactly one Tree Ring Memory entry.
- Confirmed the public HOL Plugin Scanner run for the plugin repo passed:
  `https://github.com/TerminallyLazy/tree-ring-memory-codex-plugin/actions/runs/28917677294`
- PR state: closed as superseded on 2026-07-08T10:29:39Z.
- Tree Ring Memory is already live on upstream `main` in `README.md`,
  `plugins.json`, and `.agents/plugins/marketplace.json` via the merged Codex
  plugin sync.
- Check status: no further action required unless the upstream listing regresses.

## Notes

This extended the Codex plugin launch into Hashgraph's broader AI plugins
marketplace surface without needing a duplicate PR merge.
