# Tree Ring Performance And Harness Certification Design

## Status

Approved direction: performance and harness certification before adding broader
features. Tree Ring Memory should keep the Rust-native CLI small and fast while
making agent-harness compatibility provable through repeatable local checks.

## Intent

Recent verification showed that core recall is healthy, installer behavior is
working in isolated project and global installs, and the generated
agent-awareness files give real harnesses usable instructions. It also exposed
two gaps that should be fixed before claiming the app is smooth at scale:

- bulk import and seed writes slow down at larger memory counts
- harness proof is spread across manual commands instead of one repeatable
  certification surface

This change should make Tree Ring easier to trust. A maintainer should be able
to run one certification command or script and get install size, storage size,
recall latency, import throughput, adapter behavior, and harness bridge status
from the same checkout.

## Current Baseline

Measured on the current Rust-native v0.11 checkout:

- release binary: about 5.8 MB
- project install with init: about 5.9 MB
- 10k in-process recall: max about 6.3 ms
- 30k in-process recall: max about 14.3 ms
- 50k in-process recall: max about 22.1 ms
- 10k CLI subprocess recall: p95 about 7.5 ms to 12.4 ms depending on query
- 50k performance smoke: recall stayed fast, but insert throughput fell below
  the existing 500 inserts/s threshold
- Agent Zero plugin tests pass when run with the expected Python path
- Agent Zero maintenance commands work on a host when
  `TREE_RING_MEMORY_DATA_DIR` points at a writable data root

## Goals

- Add a repeatable certification workflow for installer, size, recall, import,
  adapter, and harness bridge checks.
- Improve bulk JSONL import so larger seed or benchmark loads do not write one
  event at a time.
- Preserve current recall speed and privacy defaults.
- Distinguish project-local harness detection from user-home harness detection.
- Keep certification output machine-readable and human-readable.
- Keep Tree Ring framework-agnostic: no Agent Zero core changes, no Codex-only
  assumptions, no hidden daemon, and no background transcript capture.
- Clean up recently touched code where it removes obvious noise without changing
  behavior.

## Non-Goals

- Do not add vector search in this change.
- Do not add remote sync, cloud storage, or hosted telemetry.
- Do not auto-write global harness configuration.
- Do not turn TUI event-stream pulses into durable memory.
- Do not replace DOX, Revolve, Agent Zero, Codex, Claude Code, OpenCode, Goose,
  Hermes, or Pi; Tree Ring remains a local CLI memory lifecycle layer they can
  call deliberately.

## Considered Approaches

### 1. Certification Script Plus Targeted Import Fix

Add a repo-owned certification script under `scripts/` that drives the existing
CLI and examples, emits metrics JSON, and fails on defined thresholds. Pair it
with a targeted SQLite import batching fix.

Trade-off: this is the smallest useful change and keeps the product model
unchanged. It does not create a new long-lived CLI subcommand yet.

### 2. New `tree-ring certify` CLI Subcommand

Add a first-class Rust CLI command that runs the certification checks itself.

Trade-off: this is polished for users, but it grows the public command surface
before the checks have stabilized. It also pulls installer and host-harness
concerns into the binary.

### 3. External Benchmark-Only Validation

Focus only on external benchmark repositories and published harness PRs.

Trade-off: useful for credibility, but it does not protect this repository from
regressions and cannot verify local install, generated files, or host/container
harness behavior.

Recommended approach: start with approach 1. After the script and thresholds
settle, consider promoting the stable subset into `tree-ring certify`.

## Design

### Certification Workflow

Add `scripts/certify-tree-ring.sh`.

Default checks:

- `cargo fmt --check`
- `cargo test --locked`
- `cargo clippy --locked --all-targets`
- `cargo build --release --locked`
- release binary size check
- isolated project install with `--source`, `--project`, `--init`, and
  `--no-animation`
- isolated global install with `--source`, `--global`, and `--no-onboarding`
- JSON CLI smoke for `init`, `remember`, `evidence`, `recall`, and `audit`
- DOX dry-run/write recall smoke
- Revolve dry-run/write recall smoke
- integration scan smoke with temporary Codex, Claude Code, Agent Zero, DOX,
  Revolve, Goose, and OpenCode markers
- recall benchmark at 10k and 30k memories
- optional extended recall benchmark at 50k memories
- optional Agent Zero plugin smoke when a plugin checkout path is supplied

The script should emit:

- concise console output for humans
- `target/tree-ring-certification/metrics.json`
- `target/tree-ring-certification/summary.md`

The summary should include install size, storage size, insert throughput,
recall avg/max/p95 where available, command versions, and pass/fail status.

### Bulk Storage Throughput

Bulk memory loading has two paths that matter:

- `SQLiteMemoryStore::put_many`, used by benchmarks and internal batch writes
- `SQLiteMemoryStore::import_jsonl`, used by CLI imports and harness adapters

Both paths should be measured by certification. `import_jsonl` currently checks
and writes events one by one. Replace that path with a transaction-oriented
import:

1. Decode and normalize JSONL exactly as today.
2. Query existing ids in batches.
3. Split events into insert, replace, and skipped duplicate sets.
4. Write inserts and replacements through `put_many` inside one transaction.
5. Apply supersession updates after rows are present, preserving current
   behavior.
6. Keep dry-run behavior unchanged.

The implementation must preserve sensitivity checks, secret blocking,
duplicate skipping, replace semantics, and supersession behavior.

If `put_many` still falls below the 500 inserts/s threshold at 50k records after
the import change, optimize the shared SQLite write path rather than hiding the
failure in the benchmark.

### Harness Detection Reporting

Extend integration scan reporting so each marker records its origin:

- `project`
- `home`

The existing `detected` and `available` statuses can remain, but JSON output
should make it clear whether a harness is proven for the current repository or
only known from the user's home environment.

This avoids overstating readiness when a project has no local Claude, Codex,
OpenCode, Goose, or Agent Zero bridge files but the user's machine has global
configuration.

### Harness Smoke Matrix

Certification should exercise harnesses at the level Tree Ring actually owns:

- Codex/Gemini-style: generated `.tree-ring/SKILL.md` and
  `.tree-ring/CLI.md` are present and contain recall/remember guidance.
- Claude Code: scanner detects `.claude`/`CLAUDE.md` markers and gives a
  bridge next step.
- Agent Zero: optional plugin smoke runs syntax checks, tests, and maintenance
  commands with `TREE_RING_MEMORY_DATA_DIR` when a checkout is available.
- DOX: `dox sync --dry-run` previews source-linked memories and write mode
  stores retrievable memories.
- Revolve: `revolve sync --dry-run` previews evidence memories and write mode
  stores retrievable memories.
- OpenCode/Goose/Hermes/Pi: scanner detects markers and reports explicit next
  steps without modifying those frameworks.

### Code Simplification

Keep cleanup narrowly scoped to files touched by this work or surfaced by the
current verification:

- remove duplicate `welcome_logo_frame` branches in `welcome.rs`
- remove or test the unused `RingMarkFrame::layer_at`
- derive simple defaults for `MaintenanceFtsReport` and `MemoryReview`
- simplify filter placeholder construction in SQLite code
- avoid broader ring renderer refactors unless needed for tests

These changes should not alter user-visible behavior.

## Error Handling

- Certification must fail with a clear section name and preserve the metrics
  gathered before failure.
- Temporary install roots must be cleaned up on success and failure.
- Host Agent Zero checks must not write to `/a0`; they must use
  `TREE_RING_MEMORY_DATA_DIR`.
- Missing optional harness checkouts should be reported as skipped, not failed.
- `cargo clippy` warnings caused by known cleanup items should be fixed before
  certification is considered green.

## Acceptance Criteria

1. `scripts/certify-tree-ring.sh` runs from the repo root and produces metrics
   JSON plus a markdown summary.
2. Certification passes on the default local suite.
3. The release binary remains under 8 MB for the current macOS ARM64 build.
4. Project install with init remains under 8 MB before user memories are added.
5. 10k and 30k recall max latency remain under 50 ms in release mode.
6. 50k recall max latency remains under 100 ms in the optional extended mode.
7. 10k JSONL CLI import completes at or above 1,500 events/s on the
   certification host.
8. Optional 50k bulk storage smoke completes at or above 500 inserts/s, or the
   certification report fails with the measured throughput.
9. Integration scan JSON distinguishes project markers from home markers.
10. DOX and Revolve dry-run/write smokes store retrievable source-linked memory.
11. Optional Agent Zero plugin smoke passes when the checkout path is supplied.
12. `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked
    --all-targets`, and `git diff --check` pass.

## Documentation

Update README or architecture docs only after implementation proves the final
commands and thresholds. The docs should state measured local results with the
machine/date context, not broad benchmark claims.

The certification summary is the primary evidence artifact. Marketing or launch
materials should link to evidence and avoid unsupported leaderboard claims.

## Open Follow-Up

After the script and import fix land, decide whether the stable certification
subset should become a user-facing `tree-ring certify` command.
