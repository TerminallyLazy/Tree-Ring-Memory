# Tree Ring Memory Rust Consolidation v0.6 Design

## Goal

v0.6 adds Rust-owned deterministic consolidation. Tree Ring Memory should be
able to compress selected memory rows into source-linked summary memories
without using an LLM, mutating raw memories, leaking sensitive payloads, or
creating duplicate summaries for the same source set.

This phase keeps Tree Ring Memory framework-agnostic and local-first. It does
not add cloud services, vector dependencies, or host-specific behavior.

## Non-Goals

- No LLM summarization.
- No automatic deletion, expiry, redaction, or repair.
- No DOX/Revolve adapter implementation.
- No replacement for audit.
- No schema-breaking migration.

## Consolidation API

Rust core exposes:

```rust
consolidate_memories(events, request) -> ConsolidationPlan
```

The request supports:

```json
{
  "period_type": "daily|weekly|monthly|yearly|manual",
  "period_key": "optional stable key",
  "project": "optional project filter",
  "force": false,
  "dry_run": false
}
```

The plan/report shape is:

```json
{
  "id": "con_...",
  "created_at": "...",
  "period_type": "daily",
  "period_key": "2026-07-05",
  "candidate_count": 12,
  "source_memory_ids": ["mem_..."],
  "output_memory_ids": ["mem_..."],
  "dry_run": false,
  "force": false,
  "status": "planned|created|unchanged|dry_run|empty",
  "notes": "..."
}
```

`planned` is used only by pure in-memory planner output before a storage layer
persists summary memories. Persisted SQLite, CLI, and Python/native reports
return `created`, `unchanged`, `dry_run`, or `empty`.

## Candidate Selection

Consolidation considers non-superseded memories by default.

Candidates are included when:

- the memory is not itself a consolidation summary;
- `project` matches when a project filter is provided;
- `created_at` falls in the requested period when `period_type` is not
  `manual`;
- the memory is not secret-like;
- the memory has meaningful salience or durable ring value.

Sensitive non-secret memory may be counted but must not copy summary/details
into the generated summary text.

## Period Keys

If omitted, `period_key` is derived from current UTC time:

- daily: `YYYY-MM-DD`
- weekly: `YYYY-Www`
- monthly: `YYYY-MM`
- yearly: `YYYY`
- manual: `manual-YYYYMMDDTHHMMSSZ`

When a period key is provided, it is treated as caller-owned and used for
idempotency.

## Grouping

Candidates are grouped by:

- project
- scope
- ring
- event type
- sensitivity bucket: `normal` or `sensitive`

Generated summary memories should include compact group counts and source IDs.
They should not concatenate raw summaries. Tags are reduced to the most common
safe tags with deterministic ordering. Sensitive groups use opaque summary
labels rather than copying project, event-type, or other sensitive metadata
into generated human-readable text.

## Output Memories

Consolidation creates summary `MemoryEvent` rows:

- `event_type`: `summary`
- `source.type`: `consolidation`
- `source.ref`: `period_type:period_key`
- `retention`: `normal`
- `sensitivity`: `normal` unless all grouped rows are sensitive, then
  `private`
- `links`: source memory IDs as `memory` links
- `review.needs_review`: true when sensitive rows contributed

Output ring:

- daily/manual -> `outer`
- weekly/monthly/yearly -> `inner`
- groups for scars remain `scar`
- groups for seeds remain `seed`
- groups for heartwood remain `heartwood` only when all source rows are
  heartwood and average confidence is at least `0.75`; otherwise `inner`

## Idempotency

SQLite stores consolidation records in an additive `consolidations` table.

Idempotency key:

```text
period_type + period_key + sorted source_memory_ids
```

If a matching successful consolidation exists and `force=false`, the store
returns `status=unchanged` and does not create summary rows.

If `force=true`, a new consolidation record may be created and previous output
summary memories are superseded by the new summaries.

## CLI

Add:

```bash
tree-ring consolidate --period-type daily
tree-ring consolidate --period-type weekly --period-key 2026-W27
tree-ring --json consolidate --period-type manual --dry-run
tree-ring consolidate --period-type daily --project agent-runtime --force
```

Text mode prints a concise summary. JSON mode emits the report.

## Python Surface

Expose:

```python
memory.consolidate(period_type="daily", period_key=None, project=None, dry_run=False, force=False)
```

Native Python calls Rust. Python reference mirrors the deterministic behavior
for compatibility.

## Safety

- No raw sensitive summaries/details in generated summaries.
- Secrets are never summarized; they remain audit findings.
- Dry-run writes nothing.
- Default consolidation never deletes or expires raw rows.
- Consolidation summary rows are source-linked so users can audit provenance.

## Acceptance

1. Rust core can build deterministic consolidation plans from memory events.
2. SQLite store persists consolidation records and summary memories.
3. Consolidation is idempotent for the same period and source set.
4. Forced consolidation creates a new record and supersedes previous summary
   rows.
5. Dry-run writes nothing.
6. CLI exposes text and JSON consolidation.
7. Native Python and Python reference expose `consolidate()`.
8. Sensitive memory is summarized cautiously without payload leakage.
9. Tests cover empty, dry-run, idempotent, forced, sensitive, scoped, and CLI
   consolidation behavior.
