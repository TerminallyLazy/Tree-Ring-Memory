---
description: Capture a concise validated lesson decision warning or preference in Tree Ring Memory
argument-hint: "<memory summary>"
allowed-tools: ["Bash"]
---

# Tree Ring Capture

Capture only durable, useful memory. Do not store transcripts, secrets,
credentials, raw chain-of-thought, or unverified claims as truth.

Read project-local `.tree-ring/SKILL.md` and `.tree-ring/CLI.md` first when
present. Confirm `tree-ring --version` reports 0.14.0 or newer. If the runtime
is missing or older, stop and explain the limitation; do not install, upgrade,
or invent a stored memory without explicit user permission.

For a single agent or a store in Open mode, use the user's argument as the
memory summary:

```bash
tree-ring remember "$ARGUMENTS" \
  --event-type lesson \
  --scope project
```

For an ordinary worker in Coordinated mode, use agent scope with the
server/task-derived identity and stable retry metadata:

```bash
tree-ring remember "$ARGUMENTS" \
  --event-type lesson \
  --scope agent \
  --agent-profile "$TREE_RING_AGENT_PROFILE" \
  --workflow-id "$TREE_RING_WORKFLOW_ID" \
  --session-id "$TREE_RING_SESSION_ID" \
  --operation-id "<stable logical write id>" \
  --source-ref "<durable task, run, or result reference>"
```

Do not invent identity values. Reuse the same session and operation IDs only
for an exact retry; conflicting reuse fails closed. Keep
`TREE_RING_COORDINATOR_TOKEN` unset in ordinary worker environments.

Adjust `--event-type`, scope, project, identity, source, and tags to match the
actual memory:

- `decision` for selected architecture, release, storage, or policy choices
- `lesson` for validated work patterns
- `warning` for failures and regressions to avoid
- `user_preference` for durable user preferences
- `hypothesis` for future work that should be revisited as a seed

If the memory comes from a run, test, evaluation, incident, PR, or checkpoint,
prefer `tree-ring evidence` with an `--evidence-ref` and outcome.

In Coordinated mode, project/shared writes, `tree-ring evidence`, heartwood,
and lifecycle mutations require the one-time coordinator capability already
present only in the coordinator process environment. Never put its value in the
prompt, a CLI argument, memory, logs, source references, or committed files.

The shared-root concurrency contract covers cooperative processes on one host
and a local filesystem. Identity and scope are routing fields, not read ACLs.
