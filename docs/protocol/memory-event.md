# Memory Event Protocol

`MemoryEvent` is the portable unit of Tree Ring Memory.

The event is not a transcript line. It is a meaningful memory statement with
scope, ring, source evidence, confidence, salience, sensitivity, retention,
review state, and optional multi-agent correlation metadata.

## Multi-Agent Context

The portable JSON event supports these optional fields:

- `project`: project or repository label
- `agent_profile`: producer role or worker identity
- `workflow_id`: shared fan-out/fan-in correlation ID
- `session_id`: one execution-attempt correlation ID
- `operation_id`: idempotency key for one logical write

Missing fields deserialize as `null`. At import, read, and SQLite migration
boundaries, a pre-0.12 `agent`, `workflow`, or `session` record missing its
required identity receives a deterministic, non-sensitive, per-record
`legacy-*` identity and is marked for review. New events remain strict and
cannot be written identity-less. When present, each value must be nonblank,
contain no control characters, and contain at most 256 Unicode characters.
Sensitivity inspection includes all five fields.

Scope establishes a partition invariant:

| Scope | Required field | Meaning |
| --- | --- | --- |
| `agent` | `agent_profile` | Partition by producer identity |
| `workflow` | `workflow_id` | Partition by coordinated workflow |
| `session` | `session_id` | Partition by execution attempt |
| `project` | None | Shared project memory; project-local roots may omit `project` |
| `global` | None | Deliberate cross-project memory |

Other supported scopes retain their existing meanings. Scope and identity are
routing metadata, not a read ACL or authentication boundary. Any process with
filesystem access to the local store can issue unfiltered recall.

## Store Write Policy

Write authorization is store-local rather than part of the portable
`MemoryEvent` JSON. Stores default to `Open`, preserving the existing single
agent and trusted-process behavior. A coordinator can explicitly enable
`Coordinated` mode:

```bash
tree-ring policy enable --coordinator <label>
export TREE_RING_COORDINATOR_TOKEN='<one-time capability printed by enable>'
tree-ring policy status
tree-ring policy audit --limit 100
```

The capability is accepted only through `TREE_RING_COORDINATOR_TOKEN`; it is
never a CLI flag or event field. The store persists only a hash. Enable and
rotate return the plaintext capability once, while status and audit never
return it. Coordinators must keep the variable out of ordinary worker
environments; a fan-out child that inherits the valid token inherits
coordinator write authority.

Official Rust writers open the store with a `WriteContext` containing an
optional actor profile, an optional coordinator capability, and a bounded audit
origin. In Coordinated mode, an unauthenticated context may only create a
non-heartwood event when:

- the event has `scope=agent`
- the event has a nonblank `agent_profile`
- that profile exactly matches the `WriteContext` actor

A valid coordinator capability is required for non-agent/shared creates,
heartwood, import, persisted DOX/Revolve adapter writes, persisted
consolidation, ring changes, supersession, deletion, redaction, and applied
maintenance/FTS repair. Authorization is checked inside the same immediate
transaction as the protected mutation. Denied mutations leave memory, FTS, and
operation/tombstone state unchanged; allowed and denied protected decisions are
recorded in the store-local authorization audit without the plaintext token.

Rotate and disable require the current capability:

```bash
tree-ring policy rotate --coordinator <label>
export TREE_RING_COORDINATOR_TOKEN='<new one-time capability>'
tree-ring policy disable
unset TREE_RING_COORDINATOR_TOKEN
```

Rotation invalidates the old capability. Disabling returns the store to Open
mode. Recall, export, policy status/audit, adapter and consolidation dry-runs,
and report-only maintenance remain read-only.

This policy is operational authorization in official Rust/CLI write paths. It
does not create a read ACL, distributed authority, an OS security boundary, or
protection from an adversary who controls the database files or process
environment.

## Idempotent Writes

When `operation_id` is present, SQLite resolves it inside the
`(project, workflow_id, agent_profile)` namespace:

- An exact replay of the same write returns the existing memory ID.
- A different payload using the same namespaced key fails closed.
- `session_id` is retained in the payload but is not part of that namespace, so
  changing only the session is a conflicting reuse.
- Without `operation_id`, each accepted command is an ordinary new write.
- Replacing an active row under the same memory ID preserves its prior
  operation namespace as a one-way claim.

Derived consolidation memories always use `operation_id: null`; they must not
reuse a source event's write key. Redaction clears all identity/correlation
metadata. If the original scope was `agent`, `workflow`, or `session`,
redaction changes it to `manual` so the redacted event remains valid without
retaining its partition identifier. Storage retains only a length-delimited
SHA-256 namespace claim plus the memory ID, and a separate memory-ID tombstone,
so neither an automated retry nor replacement import can recreate redacted
content while the raw project, profile, workflow, session, and operation values
remain scrubbed. Only explicit hard deletion removes those claims.

## Partition-Aware Lifecycle Behavior

Consolidation keeps agent-scoped groups separate by `agent_profile`,
workflow-scoped groups separate by `workflow_id`, and session-scoped groups
separate by `session_id`. Output summaries retain producer/correlation fields
only when every contributing event agrees; a shared project/global summary does
not claim one producer when sources differ. Source-memory links remain the
provenance trail.

Contradiction audit uses the same scoped partitions, preventing
cross-worker, cross-workflow, or cross-session false positives. Project and
global directives remain shared and can surface contradictions across
producers.

## Rings

- `cambium`: fresh active memory
- `outer`: recent summarized memory
- `inner`: older compressed memory
- `heartwood`: durable truths
- `scar`: important negative lessons
- `seed`: unresolved future possibilities

## Recall Defaults

Recall excludes sensitive and superseded memory unless explicitly requested.
Callers may filter by project, agent profile, workflow ID, session ID, and
scope. A coordinator intentionally omits the agent-profile filter when
collecting all worker results in one workflow/session. Results should include
source evidence and ranking explanation when `explain_ranking` is true.

## Privacy Defaults

Secrets are blocked by default. Sensitive memory is excluded from recall and
export by default.

## Storage Boundary

The shared-root contract covers concurrent processes on one host using a local
filesystem. SQLite WAL, a busy timeout, and bounded lock retries handle this
local contention. The protocol does not claim distributed locking, multi-host
database coordination, or safe SQLite sharing over NFS or other network
filesystems.

Schema v3 adds coordinated-policy state, a protected-write audit, and a
connection-level old-memory-mutation fence. Before a v0.13 process first opens
an existing store, stop every Tree Ring process, checkpoint and back up the
database, and upgrade every CLI, plugin, and bundled worker. A v0.12 connection
does not register the schema-v3 writer protocol, so its memory inserts, updates,
and deletes are rejected after migration. All mixed-version operation remains
unsupported, including older reads and maintenance. Roll back only by stopping
all processes and restoring the complete pre-upgrade backup.

The bounded real-process acceptance test covers concurrent unauthorized worker
denials, permitted agent-partitioned creates, coordinator-authorized shared
publication and promotion, capability rotation, audit evidence, and exact
memory-row/FTS parity. That evidence remains limited to cooperative official
processes on one host; it is not adversarial filesystem or distributed-system
certification.
