# ATOM00blue Awesome Agent Skills PR

## Submission

- Repository: `https://github.com/ATOM00blue/awesome-agent-skills`
- Pull request: https://github.com/ATOM00blue/awesome-agent-skills/pull/3
- Custom fork: `https://github.com/TerminallyLazy/atom-awesome-agent-skills`
- Branch: `add-tree-ring-memory`
- Commit: `39bec3a20ae196644933a6e6a86c1b499f11a130`
- Status: open, ready, mergeable
- Submitted: 2026-07-08
- Evidence: https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4915057467

## Positioning

Added `tree-ring-memory` to the `Workflow & Process` section of an
opinionated agent-skills list.

The entry links the dedicated root skill repo rather than the broader framework
repo because the target list is focused on skills, plugins, and instruction
packs.

The copy intentionally follows the target template:

- What it is: a project-memory skill for recall, capture, redaction, forgetting,
  and consolidation through the Tree Ring Memory CLI.
- When to use: long-running repo work where decisions, corrections, failed
  approaches, and privacy boundaries must survive sessions.
- When not: tiny one-off edits, repos without an initialized Tree Ring store,
  or situations where the agent has not read current source docs and tests
  first.
- Why it works: memory stays scoped, evidence-linked, aged by rings, and
  forgettable while source files, tests, issues, and project contracts remain
  authoritative.

## Files Changed Upstream

- `README.md`

## Validation

- Searched existing README, issues, and PRs for `Tree Ring Memory`,
  `tree-ring-memory`, and `TerminallyLazy`; no duplicate found.
- The target contribution guide requires real use, clear when-not-to-use copy,
  permissive licensing, no duplicates, and a named problem.
- Tree Ring Memory root skill repo is MIT-licensed.
- Root skill `SKILL.md` is 205 lines.
- `git diff --check` passed.
- `curl -I -L https://github.com/TerminallyLazy/tree-ring-memory-skill`
  returned HTTP `200`.
- GitHub reports the PR as `MERGEABLE` with no status checks.

## Follow-Up

- Monitor PR #3 for maintainer review.
- If merged, verify the entry remains under `Workflow & Process`.
