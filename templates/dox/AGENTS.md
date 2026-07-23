# Tree Ring Memory Project Contract

This file is a DOX-style template for projects that use Tree Ring Memory.
Copy it into a project as `AGENTS.md` or merge the relevant sections into an existing project instruction file.

## Authority

Project source files, tests, local `AGENTS.md` files, and explicit user instructions remain authoritative.
Tree Ring Memory is a recall aid. It must not replace reading the local project contract.

## Memory Store

Default local memory path:

```text
.tree-ring/
```

Do not commit local memory databases or exports unless the project explicitly requires sanitized fixtures.

Harness-native bridge files should point agents back to this memory root's
generated guidance instead of duplicating memory data. Prefer project-level
bridges for the current repo. Treat global Tree Ring bridges as explicit user
configuration that affects every project.

## Recall Rules

Before substantial work, recall project-scoped memory for:

- current project conventions
- prior decisions
- durable user preferences
- warnings and scars related to the task
- unresolved seeds that may affect the plan

Prefer narrow recall queries that include the project name, subsystem, file path, or workflow.

## Remember Rules

Remember only meaningful information:

- durable decisions
- verified lessons
- user corrections
- repeated workflow warnings
- project conventions
- future seeds

Do not store full transcripts, scratchpad notes, raw chain-of-thought, secrets, credentials, or sensitive personal details.

Use `tree-ring evidence` for evaluated outcomes from runs, checkpoints,
experiments, incidents, PRs, issues, or reviewed artifacts. Promotions should
become heartwood only when the evidence supports durable reuse. Rejections with
reusable warning value should become scars. Deferred possibilities should become
seeds.

Use source adapters when local authoritative files already contain the guidance
or evaluated outcome:

```bash
tree-ring dox sync --source-root . --dry-run
tree-ring revolve sync --source-root revolve --dry-run
tree-ring integrations scan --source-root .
```

Run adapter syncs as previews first. Persist only concise, source-linked
summaries that help future recall.

## Agent-Mediated Updates

Tree Ring Memory writes durable memories only when a user, agent, adapter,
import, TUI action, consolidation command, or explicit maintenance command calls
the CLI. It must not be used as a hidden transcript recorder. Bridge files tell
the active agent when to call Tree Ring; they do not authorize autonomous
background capture.

## Multi-Agent Coordination

When multiple local workers share this memory root:

- Give every worker a unique `agent_profile`.
- Share one `workflow_id` across the fan-out/fan-in.
- Use one `session_id` for each genuine execution attempt; exact retries reuse
  the original session ID.
- Give each logical write a stable unique `operation_id` and a source reference.
- Use `scope=agent` only with `agent_profile`, `scope=workflow` only with
  `workflow_id`, and `scope=session` only with `session_id`.
- Recall at fan-in with explicit workflow, session, and scope filters. Omit the
  agent-profile filter when the coordinator needs every worker.
- Treat project and global memories as shared. Do not attribute a shared summary
  to one worker unless every source has that producer identity.

`TREE_RING_AGENT_PROFILE`, `TREE_RING_WORKFLOW_ID`, and
`TREE_RING_SESSION_ID` can provide the matching CLI defaults. An exact retry of
the same operation and payload returns the original memory; conflicting reuse
of the operation key must fail. Replaced operation namespaces and redacted
memory IDs stay claimed until explicit hard deletion.

Scope and identity fields are routing partitions, not read authorization
boundaries. A same-user coordinator with filesystem access can recall across
worker profiles.

This shared-root contract covers concurrent processes on one host and a local
filesystem. It is not a distributed lock service and does not claim safe
cross-host or NFS database sharing.

## Coordinated Store Policy

Stores default to backward-compatible Open mode. When only a designated
coordinator should publish or mutate shared memory, enable the optional
Coordinated policy:

```bash
tree-ring --root .tree-ring policy enable --coordinator release-coordinator
export TREE_RING_COORDINATOR_TOKEN='<one-time capability printed by enable>'
tree-ring --root .tree-ring policy status
tree-ring --root .tree-ring policy audit --limit 100
```

Never pass the capability as a CLI flag or store it in memory, logs, source
refs, or committed files. Tree Ring prints it once and stores only its hash.
Inject it only into coordinator processes; launch ordinary workers with
`TREE_RING_COORDINATOR_TOKEN` unset.

In Coordinated mode, an ordinary worker may create only non-heartwood
`scope=agent` memory whose `agent_profile` matches `--agent-profile` or
`TREE_RING_AGENT_PROFILE`. Project/global/workflow/session writes, heartwood,
imports, persisted DOX/Revolve sync, persisted consolidation, ring changes,
supersede/delete/redact, and applied maintenance require
`TREE_RING_COORDINATOR_TOKEN`. Read-only recall/export, policy status/audit,
adapter and consolidation dry-runs, and report-only maintenance remain
available without it.

For the TUI, set `--agent-profile <worker>` or
`TREE_RING_AGENT_PROFILE=<worker>` so `/remember` defaults to agent scope.
Lifecycle actions require the coordinator capability.

Rotate and disable only while the current capability is exported:

```bash
tree-ring --root .tree-ring policy rotate --coordinator release-coordinator-next
export TREE_RING_COORDINATOR_TOKEN='<new one-time capability>'
tree-ring --root .tree-ring policy disable
unset TREE_RING_COORDINATOR_TOKEN
```

This is operational authorization in official Rust/CLI write paths, not a read
ACL or protection against an adversary who controls local files or the process
environment.

Before a v0.13/schema-v3 upgrade, stop all Tree Ring processes, checkpoint and
back up the store, and upgrade every CLI, plugin, and bundled worker before
reopening it. Schema v3 fences memory inserts, updates, and deletes from old
v0.12 writers; all mixed-version operation is unsupported. Roll back only by
restoring the pre-upgrade backup.

## Ring Mapping

- Use `cambium` for active task context.
- Use `outer` for recent project lessons.
- Use `inner` for compressed older project knowledge.
- Use `heartwood` for confirmed durable rules and preferences.
- Use `scar` for failures, regressions, rejected approaches, and security or privacy warnings.
- Use `seed` for future work and unresolved hypotheses.

## Sensitive Data

Secrets and credentials must not be stored.
If a useful lesson involves sensitive data, store a redacted summary and source pointer only.

## Forgetting

If a memory is incorrect, stale, sensitive, or superseded, delete, redact, or supersede it with an explicit reason.

## Source Discipline

Memory summaries should point back to source evidence such as:

- file paths
- PRs or issues
- tests
- evaluation runs
- local project docs

When source documents and memory disagree, re-read the source documents and update or forget the stale memory.

DOX-style `AGENTS.md` files and Revolve/evaluation records remain
authoritative. Tree Ring Memory can summarize and point to them, but it must not
replace DOX traversal, copy whole contract trees, treat stale scores as current
truth, or promote an outcome without source evidence.
