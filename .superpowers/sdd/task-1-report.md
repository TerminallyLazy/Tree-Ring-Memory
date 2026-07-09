# Task 1 Report

## What changed
- Added `mod evidence;` to `crates/tree-ring-memory-cli/src/main.rs` so the CLI crate exposes the new evidence reader module.
- Created `crates/tree-ring-memory-cli/src/evidence.rs` with the typed evidence model and reader:
  - `EvidenceStatus`
  - `EvidenceRecordRef`
  - `EvidenceIndex`
  - `CertificationEvidence`
  - `EvidenceSnapshot`
  - `certification_dir_for_project`
  - `load_snapshot`
- Implemented file-backed loading from `evidence-index.json` and `metrics.json`, including path resolution and extraction of certification metrics.
- Added the two focused tests from the brief:
  - missing evidence index returns a `Missing` snapshot
  - certification metrics load correctly from the index

## Tests and outputs
- Ran:
  - `cargo test -p tree-ring-memory-cli evidence --locked`
- Result:
  - PASS
  - `5` tests passed, `0` failed
  - The two new `evidence::*` tests passed, along with three existing evidence-related tests already present in the crate.

## TDD evidence
- The brief supplied the exact tests to implement and verify.
- I created the module to match the brief, then ran the focused test command from the task and confirmed both new tests passed.

## Files changed
- `crates/tree-ring-memory-cli/src/main.rs`
- `crates/tree-ring-memory-cli/src/evidence.rs`

## Self-review
- The implementation matches the brief’s public surface and keeps the scope limited to the requested CLI files.
- Snapshot loading is resilient: missing indexes become `Missing`, parse/read failures become `Error`, and certification metrics are loaded only when the index points at a certification record.
- The tests cover the required happy path and missing-index path, and the focused crate test run passed without additional fixes.

## Concerns
- The reader currently treats certification metric parsing as best-effort and returns `None` for missing fields, which is consistent with the brief but means downstream consumers will need to decide how to surface partial evidence.
- No additional integration points were changed in this task; later phases still need to wire the loaded snapshot into the TUI and certification writer.

## Fix report

### What changed
- Added a compatibility path for metrics-only certification directories. When `evidence-index.json` is absent but `metrics.json` exists, `load_snapshot` now loads a certification snapshot instead of returning `Missing`.
- Changed indexed certification loading to fail hard when the index points at a certification payload that cannot be read or parsed. That now returns an `Error` snapshot with the payload path in the message instead of silently dropping certification data.
- Added focused tests for:
  - metrics-only fallback without an index
  - indexed certification payload failure when `metrics.json` is missing

### Tests and output
- Ran:
  - `cargo test -p tree-ring-memory-cli evidence --locked`
- Result:
  - PASS
  - `7` tests passed, `0` failed

### Files changed
- `crates/tree-ring-memory-cli/src/evidence.rs`
- `crates/tree-ring-memory-cli/src/main.rs`
- `.superpowers/sdd/task-1-report.md`

### Self-review
- The compatibility branch preserves the original `Missing` behavior when neither index nor metrics are present.
- The indexed path no longer hides certification-payload errors, which prevents a false pass when the index is present but the payload is broken.
- The change stayed confined to the Task 1 file set plus the required report file.
