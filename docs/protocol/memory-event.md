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
routing metadata, not an ACL or authentication boundary. Any process with
filesystem access to the local store can issue unfiltered recall.

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
