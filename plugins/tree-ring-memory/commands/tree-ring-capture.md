---
description: Capture a concise validated lesson decision warning or preference in Tree Ring Memory
argument-hint: "<memory summary>"
allowed-tools: ["Bash"]
---

# Tree Ring Capture

Capture only durable, useful memory. Do not store transcripts, secrets,
credentials, raw chain-of-thought, or unverified claims as truth.

## Lifecycle Stop Checkpoint

`Stop` and `SubagentStop` hooks enforce one agent-mediated checkpoint. The hook
never inspects or persists `transcript_path`, `last_assistant_message`, prompts,
or transcript content. Evaluate the grounded work already in the active agent's
context. If there is no durable, normal-sensitivity candidate, do not write a
memory.

For each qualifying candidate, use the exact identity-bound template returned
by the lifecycle handler. Its strict shape is:

```bash
tree-ring --root .tree-ring capture "<concise summary>" \
  --event-type <preference|decision|lesson|warning|correction|seed> \
  --ring <cambium|scar|seed> \
  --project <project> \
  --agent-profile <profile> \
  --workflow-id <workflow> \
  --session-id <session> \
  --operation-id auto-<checkpoint>-<1..3> \
  --source-ref agent-checkpoint:<checkpoint> \
  [--tag <tag>]
```

Do not invent or edit the supplied identity, checkpoint, operation, or source
values. Strict capture fixes `scope=agent`, requires identity and provenance,
adds the automatic-capture tag, and accepts normal sensitivity only. Use no more
than three candidates in the single checkpoint. Never replace this with a raw
transcript summary, `remember`, `evidence`, or an import.

Use `cambium` for preferences, decisions, lessons, and corrections; `scar` for
warnings; and `seed` for future work. A candidate still must be durable and
grounded regardless of its ring.

The manual command flow below remains available for an explicit user-directed
capture outside a lifecycle checkpoint.

Read project-local `.tree-ring/SKILL.md` and `.tree-ring/CLI.md` first when
present. Follow the skill's Runtime Bootstrap And Updates procedure and confirm
the selected project-local or global binary reports 0.15.0 or newer. Bootstrap
or upgrade when the user's request already authorizes it; otherwise obtain
permission before downloading or changing software. Never invent a stored
memory.

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
