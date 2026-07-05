# Tree Ring Memory Rust Maintenance v0.7 Design

## Goal

v0.7 makes the public runtime Rust-native and adds Rust-owned maintenance
lifecycle planning and execution. Tree Ring Memory should be able to inspect
local memory, explain what needs cleanup, and optionally apply narrowly-scoped
maintenance actions without turning retention into silent deletion.

This phase keeps Tree Ring Memory framework-agnostic, local-first, and
privacy-preserving. It does not add cloud services, agent-harness assumptions,
or background daemons.

## Non-Goals

- No automatic scheduled deletion.
- No LLM review or summarization.
- No DOX/Revolve adapter implementation.
- No cloud sync.
- No schema-breaking migration.
- No broad purge command.
- No new Python-owned lifecycle behavior.

## Maintenance API

Rust core exposes:

```rust
plan_maintenance(events, request) -> MaintenanceReport
```

The request supports:

```json
{
  "project": "optional project filter",
  "include_superseded": false,
  "dry_run": true,
  "apply_expired": false,
  "apply_secret_redactions": false,
  "repair_fts": false
}
```

The report shape is:

```json
{
  "id": "mnt_...",
  "generated_at": "...",
  "memory_count": 12,
  "planned_action_count": 2,
  "applied_action_count": 0,
  "dry_run": true,
  "status": "planned|applied|clean",
  "actions": [
    {
      "action_type": "delete_expired|redact_secret|review_expired_protected",
      "memory_id": "mem_...",
      "severity": "critical|high|medium|low",
      "reason": "human-readable reason",
      "applied": false
    }
  ],
  "fts": {
    "memory_rows": 12,
    "fts_rows": 12,
    "missing_fts_rows": 0,
    "orphan_fts_rows": 0,
    "repaired": false
  }
}
```

The pure Rust core does not know about FTS tables. SQLite populates the `fts`
object and applies FTS repair when requested.

## Candidate Rules

Maintenance considers non-superseded memories by default.

Candidates are included when:

- `project` matches when provided;
- `include_superseded=true` or the memory is not superseded;
- the memory has an actionable retention/privacy issue.

Expired candidates:

- A memory is expired when `expires_at` parses as an RFC3339 timestamp and is
  less than or equal to current UTC time.
- Expired memories are eligible for `delete_expired` only when
  `retention in {"ephemeral", "forget_after_date"}`.
- Expired `scar`, `heartwood`, `durable`, or `user_pinned` memories are never
  deleted by maintenance. They produce `review_expired_protected` actions.
- Invalid `expires_at` values are report-only review actions and are never
  auto-mutated by v0.7.

Secret candidates:

- Secret-like memory is detected by deterministic sensitivity inspection across
  the public memory-event surface, not only by trusting `sensitivity`.
- Secret-like memory produces `redact_secret`.
- Redaction is applied only when `apply_secret_redactions=true` and
  `dry_run=false`.

## Apply Semantics

Default behavior is report-only:

- `dry_run=true` writes nothing.
- `apply_expired=false` means expired deletion actions are reported but not
  applied.
- `apply_secret_redactions=false` means secret redaction actions are reported
  but not applied.
- `repair_fts=false` means FTS drift is reported but not repaired.

When applying:

- Expired deletes remove the memory row and FTS row transactionally.
- Secret redactions use the existing redaction behavior.
- FTS repair rebuilds `memory_fts` from `memories` transactionally.
- The report marks only successfully applied actions as `applied=true`.

## CLI

Add:

```bash
tree-ring maintain
tree-ring --json maintain
tree-ring maintain --project agent-runtime
tree-ring maintain --apply-expired
tree-ring maintain --apply-secret-redactions
tree-ring maintain --repair-fts
```

`tree-ring maintain` is safe by default and writes nothing. The CLI should say
when maintenance was report-only and which flags are needed to apply actions.

## Python Binding Surface

Expose:

```python
memory.maintain(
    project=None,
    include_superseded=False,
    dry_run=True,
    apply_expired=False,
    apply_secret_redactions=False,
    repair_fts=False,
)
```

The Python package is a binding surface only for this feature:

- `TreeRingMemory.open()` must use the native Rust extension and fail with a
  clear build/install hint when the extension is missing.
- `NativeTreeRingMemory.maintain(...)` calls Rust.
- The legacy `PythonTreeRingMemory` reference backend must not receive a
  maintenance implementation in v0.7.
- Existing Python reference tests may remain as compatibility fixtures, but the
  public default path must be Rust-native.

## Safety

- No destructive action occurs without `dry_run=False` and an explicit apply
  flag for that action type.
- Heartwood, scars, durable memory, and user-pinned memory are protected from
  automated deletion.
- Secret redaction preserves the memory id but removes sensitive payload and
  metadata.
- Maintenance reports must not print secret payloads.

## Acceptance

1. Rust core can build deterministic maintenance plans from memory events.
2. SQLite store can execute maintenance transactionally.
3. Expired ephemeral/forget-after-date memories can be deleted only with
   explicit apply.
4. Expired protected memories are reported but never deleted by maintenance.
5. Secret-like memories can be redacted only with explicit apply.
6. FTS drift can be detected and repaired.
7. CLI exposes safe text and JSON maintenance reports.
8. Native Python exposes `maintain()` as a thin Rust binding.
9. Tests cover dry-run, apply, protected memory, secret redaction, FTS repair,
   project filtering, CLI behavior, and native/Python parity.
10. `TreeRingMemory.open()` no longer silently falls back to Python reference
    behavior when the Rust native extension is missing.
