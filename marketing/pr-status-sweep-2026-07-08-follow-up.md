# PR Status Sweep Follow-Up - 2026-07-08

Status: tracker update and conflict repair

Checked with `gh pr view` on 2026-07-08. Full local sweep output was written
to `/tmp/tree-ring-github-sweep-2026-07-08.json`.

Public evidence comment:
https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4913887305

## Newly Merged

| Outlet | PR | Merged At | Notes |
| --- | --- | --- | --- |
| Jenqyang Awesome AI Agents | https://github.com/Jenqyang/Awesome-AI-Agents/pull/358 | 2026-07-08T10:18:38Z | Adds Tree Ring Memory to Applications / Tools. |
| Hashgraph Awesome Codex Plugins | https://github.com/hashgraph-online/awesome-codex-plugins/pull/279 | 2026-07-08T09:56:42Z | Adds the Tree Ring Memory Codex plugin; upstream action posted HOL registry claim instructions. |

## Live Via Upstream Sync

| Outlet | URL | Notes |
| --- | --- | --- |
| Hashgraph Awesome AI Plugins | https://github.com/hashgraph-online/awesome-ai-plugins | PR https://github.com/hashgraph-online/awesome-ai-plugins/pull/59 was closed as superseded after verifying Tree Ring Memory is already present on `main` in `README.md`, `plugins.json`, and `.agents/plugins/marketplace.json`. |

## Conflict Repairs

| Outlet | PR | New Head | Result |
| --- | --- | --- | --- |
| Correia Awesome AI Tools | https://github.com/Correia-jpv/fucking-awesome-ai-tools/pull/30 | `8bf26e5` | Rebased the fork branch onto current upstream and resolved the conflict by keeping the current upstream README plus the Tree Ring Memory developer-tools row. GitHub reports the PR as `MERGEABLE` with no checks. |
| Awesome CLI Agents | https://github.com/phamquiluan/awesome-cli-agents/pull/21 | `f788d28` | Rebased the fork branch onto current upstream and resolved the conflict by keeping the current upstream generated README plus the Tree Ring Memory terminal entry. GitHub reports the PR as `MERGEABLE` with no checks. |

## Still Owner-Gated

- https://github.com/e2b-dev/awesome-ai-sdks/pull/268 still requires the
  owner to sign E2B's CLA and comment `@cla-bot check`.
- https://github.com/github/awesome-copilot/pull/2235 still has an upstream
  duplicate-check job that cannot check out the fork without maintainer write
  permission; Vally lint and local skill validation passed earlier.

## Summary

- Submitted GitHub PRs/issues checked: 175.
- New merged PRs found: 2.
- Conflicting PRs found: 3.
- Conflicts repaired: 2.
- Redundant conflicted PR closed because the listing was already live: 1.
