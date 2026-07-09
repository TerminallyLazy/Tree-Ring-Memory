# Task 4 Report: TUI Recall-Quality Dashboard Rendering

## What changed

- Updated `/evidence` list rendering in `crates/tree-ring-memory-cli/src/tui/render.rs` so the `Recall quality` row shows the actual evidence status from `evidence-index.json` instead of the generic `loaded` marker.
- Added recall-quality detail rendering in the evidence detail pane for:
  - status
  - query set id
  - query/pass/fail/review counts
  - average and max latency
  - record path
  - up to four query rows with query id, status, expected rank, latency, and returned ids
- Kept the renderer limited to sanitized `EvidenceSnapshot.recall_quality` fields. No memory summaries, details, or raw diagnostic payload fields are rendered.
- Tightened the certification header formatting slightly so the new recall-quality lines remain visible in the existing 120x36 TUI test layout.

## TDD evidence

1. Added failing test: `render_evidence_mode_shows_recall_quality_records`
2. Ran:

```bash
cargo test -p tree-ring-memory-cli render_evidence_mode_shows_recall_quality_records --locked
```

Observed failure:

- missing `default-fixture-v1` from the rendered `/evidence` output because recall-quality record details were not yet rendered

3. Implemented the render changes in `crates/tree-ring-memory-cli/src/tui/render.rs`
4. Ran focused suite:

```bash
cargo test -p tree-ring-memory-cli render_evidence_mode --locked
```

Result: pass

## Tests

- `cargo test -p tree-ring-memory-cli render_evidence_mode_shows_recall_quality_records --locked` -> failed first, then passed after implementation as part of the focused suite
- `cargo test -p tree-ring-memory-cli render_evidence_mode --locked` -> passed

## Files changed

- `crates/tree-ring-memory-cli/src/tui/render.rs`
- `.superpowers/sdd/task-4-report.md`

## Self-review

- Scope stayed inside the requested TUI renderer file for code changes.
- The new detail block only consumes `snapshot.recall_quality`, which already contains scrubbed `returned_ids`.
- Existing evidence render tests still pass.
- No runner, evidence loader, certification script, or README changes were made.

## Concerns

- The evidence detail pane is height-constrained in the current layout. The renderer now keeps the first recall-quality rows visible in the existing test size, but larger certification blocks plus future evidence sections may need a scrollable detail pane rather than more line compaction.

## Follow-up fix: review findings

### What changed

- Updated the `/evidence` list `Recall quality` row to prefer `snapshot.recall_quality.status` when the payload is loaded, and only fall back to the index record when the payload is unavailable.
- Updated recall-quality detail query rows to render operator-facing rank strings as `rank 1` or `rank -` instead of `Some(1)` / `None`.
- Expanded the recall-quality render regression test so the index reports `pass` while the payload reports `needs_review`, and asserted the rendered surface shows `Recall quality needs_review` without stale `Recall quality pass`.

### Commands and results

```bash
cargo test -p tree-ring-memory-cli render_evidence_mode --locked
```

- Result: pass (`6` tests)

### Files changed for follow-up fix

- `crates/tree-ring-memory-cli/src/tui/render.rs`
- `.superpowers/sdd/task-4-report.md`
