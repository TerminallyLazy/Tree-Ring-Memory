# Tree Ring Memory Rust Audit v0.5 Design

## Summary

v0.5 adds Rust-owned memory audit and doctor-style maintenance checks. Tree Ring
Memory is meant to be explainable, forgettable, and privacy-preserving; audit is
the lifecycle layer that tells an agent or operator when memory has become
risky, stale, contradictory, or structurally inconsistent.

This phase stays local-first and deterministic. It does not add LLM review,
cloud services, schema-breaking migrations, or consolidation summaries.

## Goals

- Add Rust audit models and deterministic audit checks.
- Add SQLite store audit helpers over existing memory rows.
- Add scriptable CLI support for `tree-ring audit`.
- Expose audit through the optional native Python backend and Python facade.
- Keep Python reference behavior in parity with Rust.
- Document audit semantics and verification.

## Non-Goals

- No automatic deletion or redaction.
- No LLM contradiction analysis.
- No new SQLite tables in this phase.
- No consolidation implementation.
- No external services or vector databases.
- No cloud reporting.

## Audit Types

The v0.5 audit API supports:

- `all`
- `stale`
- `sensitive`
- `low_confidence`
- `supersession`
- `contradictions`

## Finding Shape

Rust and Python should expose JSON-compatible findings:

```json
{
  "audit_type": "sensitive",
  "severity": "high",
  "memory_id": "mem_...",
  "related_memory_id": null,
  "finding": "Sensitive memory is retained without an expiry.",
  "recommended_action": "Review, redact, or set expires_at.",
  "tags": ["privacy", "retention"]
}
```

Reports include:

```json
{
  "generated_at": "2026-07-05T00:00:00Z",
  "audit_type": "all",
  "memory_count": 42,
  "finding_count": 3,
  "findings": []
}
```

## Check Semantics

### Stale

Find memories with `expires_at` in the past. Expiration is advisory in v0.5:
the audit reports the row but does not delete it.

### Sensitive

Find privacy-risk rows:

- `sensitivity == "secret"` is critical.
- non-normal sensitive rows with durable/user-pinned retention are high.
- non-normal sensitive rows with no `expires_at` are medium.

### Low Confidence

Find durable memory that should be reviewed:

- `ring == "heartwood"` and `confidence < 0.75`.
- `retention in {"durable", "user_pinned"}` and `confidence < 0.5`.

### Supersession

Find structural gaps:

- `superseded_by` references a missing memory.
- `supersedes` references a missing memory.
- a memory listed in another row's `supersedes` is missing the reciprocal
  `superseded_by` pointer.

### Contradictions

Use a conservative deterministic heuristic only:

- candidates must share project, scope, and event type.
- candidates must share at least one tag.
- one summary starts with `use ...` and the other starts with `avoid ...` for
  the same normalized phrase.

The heuristic reports a review candidate, not truth.

## CLI Shape

```bash
tree-ring audit
tree-ring audit --audit-type sensitive
tree-ring --json audit --audit-type all
```

Text output should be concise and human-readable. JSON output should use the
report shape above.

## Python Shape

```python
report = memory.audit()
report = memory.audit(audit_type="sensitive")
```

The native backend should call Rust. The Python reference backend should mirror
the deterministic checks for parity while Rust remains the behavioral target.

## Safety

- Audit must not print sensitive payloads beyond existing summaries.
- Audit must not mutate storage.
- Secret-like memory findings should identify the memory id and policy issue,
  not reproduce the secret.
- Findings should recommend review, redaction, expiry, or supersession repair.

## Acceptance

1. Rust core can audit a list of `MemoryEvent` values.
2. Rust SQLite store can audit persisted memories.
3. CLI `tree-ring audit` returns text and JSON reports.
4. Python native wrapper exposes `audit()`.
5. Python reference backend exposes parity `audit()`.
6. Stale, sensitive, low-confidence, supersession, and contradiction checks are tested.
7. Audit is non-mutating.
8. Docs describe audit semantics and commands.
9. Rust and Python tests pass.
