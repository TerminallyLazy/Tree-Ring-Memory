---
name: tree-ring-memory
description: Guides AI agents in using Tree Ring Memory for durable recall, project decisions, user preferences, warnings, future seeds, privacy-safe memory capture, and lifecycle-aware forgetting.
version: 0.12.0
tags: ["memory", "agents", "recall", "privacy", "projects", "dox", "revolve", "skills", "cli"]
triggers:
  - "remember this"
  - "recall what we decided"
  - "what did we learn"
  - "tree ring memory"
  - "consolidate memory"
  - "forget this"
  - "project memory"
  - "sync DOX"
  - "sync Revolve"
  - "evidence loop"
  - "multi-agent memory"
---

# Tree Ring Memory

Use Tree Ring Memory as a lifecycle-aware memory layer, not as a transcript dump.

Tree Ring Memory preserves meaningful agent learning like tree rings:

- fresh work stays detailed
- older learning compresses into stable rings
- important warnings remain visible as scars
- durable truths become heartwood
- speculative future work stays as seeds
- sensitive data is blocked, redacted, or kept out by default

## When To Recall

Recall memory before:

- starting or resuming a project
- changing architecture, storage, security, privacy, or release behavior
- repeating a workflow where prior failures may matter
- responding to a user correction
- making a decision that depends on previous preferences or constraints
- editing files in a repo that has a Tree Ring Memory or `AGENTS.md` contract
- closing out meaningful work and deciding what should be remembered

Use narrow queries with project scope when possible. Prefer source-linked, high-confidence, non-superseded results.

## When To Remember

Store a memory when the information is likely to help future work:

- the user states a durable preference
- the user corrects the agent
- a decision is made and should survive the current session
- an implementation lesson is validated by tests or production behavior
- a failed approach should not be repeated
- a security, privacy, release, or data-loss warning appears
- a useful project convention is discovered
- a future idea should be revisited later

Keep memory concise. Store the lesson, decision, or warning, not the full conversation.

Use `tree-ring evidence` instead of plain `remember` when the lesson comes from
an evaluation, checkpoint, experiment, branch, incident, or reviewed run
artifact.

Use source adapters when project artifacts already contain structured guidance
or evaluated outcomes:

```bash
tree-ring dox sync --source-root . --dry-run
tree-ring revolve sync --source-root revolve --dry-run
tree-ring integrations scan --source-root .
```

Run adapter commands with `--dry-run` first. Sync only concise, source-linked
summaries; never treat imported memory as more authoritative than the source
`AGENTS.md`, Revolve record, evaluation, PR, issue, or test artifact.

Use the exact CLI commands exposed by the local install:

```bash
tree-ring --help
tree-ring dox sync --help
tree-ring revolve sync --help
tree-ring evidence --help
```

If the project was initialized with a project-local binary, prefer the generated
`.tree-ring/CLI.md` reference and include `--root .tree-ring` when needed.

If this skill was loaded through a harness-native bridge file, treat that bridge
as a pointer only. Read the project-local `.tree-ring/SKILL.md` and
`.tree-ring/CLI.md` when present so commands match the installed project root.
Do not assume a global Tree Ring setup applies to the current repo unless the
user explicitly configured it.

Evidence outcome mapping:

- `promoted`: durable heartwood from supported evidence
- `rejected`: scar for reusable failed or rolled-back approaches
- `deferred`: seed for promising unresolved options
- `observed`: outer-ring evaluation result

## Memory Quality Gates

Use these gates before relying on or writing memory.

Recall gates:

- Before substantial project work, recall project constraints, scars, user preferences, and unresolved seeds.
- Before risky changes, recall warnings and evidence-linked prior failures.
- Before repeating a workflow, recall prior errors and accepted procedures.
- Before closeout, recall recent decisions so memory updates do not contradict already-stored lessons.

Trust gates:

- Prefer source-linked, non-superseded, high-confidence memories.
- Treat heartwood as durable only when source evidence or user confirmation supports it.
- Re-read source files, tests, explicit user instructions, DOX contracts, or Revolve evidence when memory conflicts with current sources.
- Do not treat sensitive or hidden-by-default memory as ordinary recall context.

Write gates:

- Remember only durable decisions, validated lessons, reusable warnings, corrections, future seeds, and evidence-backed outcomes.
- Reject transient planning chatter, duplicate wording, tool noise, and unsupported claims.
- Require evidence refs for promoted or rejected evaluated outcomes.
- Require user confirmation before creating or promoting broad cross-project heartwood.

## Ring Selection

Use these rings:

- `cambium`: active or recent task context
- `outer`: recent decisions and task lessons
- `inner`: older compressed project knowledge
- `heartwood`: durable, high-confidence truths and user preferences
- `scar`: important negative memory, failures, regressions, rejected approaches, and warnings
- `seed`: unresolved ideas, hypotheses, follow-ups, and future work

Do not promote to `heartwood` from weak evidence. Prefer `outer` or `seed` unless the user confirms durability or the evidence is strong.

## Event Types

Prefer specific event types:

- `user_preference`
- `decision`
- `lesson`
- `warning`
- `correction`
- `file_change`
- `tool_result`
- `summary`
- `hypothesis`

If a host integration has stricter event type names, use the closest local equivalent.

## What Not To Store

Do not store:

- secrets
- credentials
- tokens
- private keys
- raw chain-of-thought
- temporary scratchpad notes
- unverified claims as durable truth
- private health, financial, legal, or personal identifier details without explicit user instruction
- copyrighted source text beyond short allowed snippets

If a useful memory contains sensitive material, store a redacted summary with enough context to be useful.

## Source And Scope

Set project and scope deliberately:

- use project scope for repo-specific rules, decisions, warnings, and lessons
- use agent scope for agent-partitioned behavior and always set `agent_profile`
- use workflow scope for one coordinated fan-out/fan-in and always set `workflow_id`
- use session scope for one execution attempt and always set `session_id`
- use global scope only for durable user preferences or cross-project guidance
- include source references such as file paths, issue ids, PR ids, run ids, or docs paths
- use `tree-ring evidence ... --evidence-ref <ref>` for evaluated outcomes
- use `tree-ring dox sync` for concise `AGENTS.md` summaries
- use `tree-ring revolve sync` for promoted, rejected, deferred, or observed evaluation records
- use `tree-ring integrations scan` before configuring a new agent harness

Memory does not replace source documents. If a repo has `AGENTS.md`, project docs, tests, architectural records, or host-specific instruction files, read those sources directly and treat them as authoritative.

When DOX or Revolve source records change, re-run the matching sync adapter with
`--dry-run`, inspect the generated memories, then run the write command only
when the summaries are useful and source-linked.

## Multi-Agent Coordination

For workers sharing one local Tree Ring root, give every write explicit
coordination metadata:

```bash
tree-ring --root .tree-ring remember "Worker validated the storage boundary." \
  --event-type lesson \
  --scope agent \
  --project example-service \
  --agent-profile worker-storage \
  --workflow-id release-readiness \
  --session-id attempt-1 \
  --operation-id validate-storage-v1 \
  --source-ref runs/release-readiness/worker-storage.json
```

Use a unique `agent_profile` per worker, one shared `workflow_id` for the
fan-out/fan-in, one `session_id` for each genuine execution attempt, and a stable
unique `operation_id` for each logical write. An exact retry reuses both the
original session ID and operation ID; changing only the session is a conflicting
reuse. Start a new session and use new operation IDs only for a genuinely new
attempt. Exact retries with the same operation metadata and payload return the
original memory. Reusing that operation key for a different payload fails
closed. Replacing a stored memory keeps its old operation namespace claimed.
Redaction also tombstones the memory ID; only an explicit hard delete releases
those claims.

At fan-in, recall the shared workflow and session without an agent-profile
filter, inspect the source refs, then write a source-linked workflow or project
summary:

```bash
tree-ring --root .tree-ring recall "release readiness" \
  --project example-service \
  --workflow-id release-readiness \
  --session-id attempt-1 \
  --scope agent
```

`TREE_RING_AGENT_PROFILE`, `TREE_RING_WORKFLOW_ID`, and
`TREE_RING_SESSION_ID` provide the same defaults as their CLI flags. Do not
leave an agent-profile environment filter set when the coordinator intends to
recall every worker.

This shared-root pattern is for concurrent processes on one host using a local
filesystem. It is not a distributed lock service and does not claim safe
cross-host or NFS operation. Scope and identity fields are routing metadata, not
an ACL; a same-user coordinator can recall across profiles. Use per-host stores
plus an explicit, evidence-preserving fan-in process when work spans hosts.

## Agent-Mediated Updates

Tree Ring Memory does not autonomously scrape chats or write durable memory in
the background. The active agent is responsible for deciding when a Tree Ring
command is warranted, then calling the CLI deliberately.

Use bridge files only to discover Tree Ring and its command reference:

- project-level bridges should point to `.tree-ring/SKILL.md` and
  `.tree-ring/CLI.md`
- global bridges should be treated as opt-in user configuration
- TUI event-stream pulses are display signals, not durable memories

Before writing memory, verify the lesson is durable, useful, privacy-safe, and
grounded in user instruction or source evidence.

## Forgetting And Correction

If memory is wrong, private, stale, or superseded:

- redact it when the durable shape is useful but details are unsafe
- delete it when it should not be retained
- supersede it when a newer decision replaces it
- prefer explicit reasons for every forget operation

Treat redaction as monotonic. Do not try to restore a redacted ID through
replacement import; create a new reviewed memory only if the user deliberately
reintroduces safe content.

Never keep known-wrong memory merely because it was previously recalled.

## Closeout Habit

At the end of meaningful work, ask:

- What did we decide?
- What did we learn?
- What should future agents avoid repeating?
- Did the user state a durable preference?
- Is there a future seed worth revisiting?
- Is any memory sensitive and better left unstored?

Only remember the answers that will materially improve future work.
