# Tree Ring Performance And Harness Certification Implementation Plan

## Task 1: Add Bulk Storage Performance Tests

Files:

- `crates/tree-ring-memory-sqlite/src/lib.rs`
- `crates/tree-ring-memory-sqlite/examples/performance_smoke.rs`

Work:

- Add focused tests around duplicate handling, replacement, and supersession for
  bulk import so later refactors are protected.
- Keep the existing performance smoke but make its output easy for automation
  to parse.
- Preserve the current recall latency assertions.
- Keep the 50k path as an extended check so default certification remains
  practical.

Checks:

```bash
cargo test -p tree-ring-memory-sqlite import_jsonl
cargo run --release -p tree-ring-memory-sqlite --example performance_smoke -- 10000
cargo run --release -p tree-ring-memory-sqlite --example performance_smoke -- 30000
```

## Task 2: Batch JSONL Import

Files:

- `crates/tree-ring-memory-sqlite/src/lib.rs`

Work:

- Add an internal helper that fetches existing memory ids in chunks.
- Refactor `SQLiteMemoryStore::import_jsonl` to split normalized events into
  inserts, replacements, and skipped duplicates before writing.
- Write inserts and replacements through transaction-backed batch paths instead
  of calling `put` for every event.
- Preserve dry-run behavior, secret blocking, duplicate skipping,
  `--replace-existing`, and supersession application.
- If `put_many` remains the bottleneck for larger stores, profile and optimize
  the shared write path without changing storage semantics.

Checks:

```bash
cargo test -p tree-ring-memory-sqlite import_jsonl
cargo test -p tree-ring-memory-sqlite put_many
cargo run --release -p tree-ring-memory-sqlite --example performance_smoke -- 30000
```

## Task 3: Report Harness Marker Origin

Files:

- `crates/tree-ring-memory-cli/src/integrations.rs`
- `crates/tree-ring-memory-cli/src/main.rs`

Work:

- Extend `AgentIntegration` marker output so each marker includes its origin:
  `project` or `home`.
- Keep the existing human-readable scan useful, but make JSON output explicit
  enough for certification.
- Preserve existing integration ids and statuses.
- Add tests proving project-only, home-only, and mixed marker cases.

Checks:

```bash
cargo test -p tree-ring-memory-cli integrations
./target/release/tree-ring --json integrations scan --source-root .
```

## Task 4: Add Certification Script

Files:

- `scripts/certify-tree-ring.sh`

Work:

- Add a POSIX shell script that runs from the repo root.
- Build release once, then reuse the binary for CLI smokes.
- Write outputs under `target/tree-ring-certification/`.
- Emit `metrics.json` and `summary.md`.
- Measure release binary size, project install size, CLI import throughput, and
  recall timing.
- Run isolated project and global installer checks.
- Run JSON CLI smoke for `init`, `remember`, `evidence`, `recall`, and `audit`.
- Run DOX and Revolve dry-run/write smokes.
- Run integration-scan smoke with temporary harness markers.
- Support optional environment variables:
  - `TREE_RING_CERT_EXTENDED=1` for the 50k benchmark
  - `TREE_RING_AGENT_ZERO_ROOT=/path/to/a0-ready-test` for Agent Zero plugin
    checks

Checks:

```bash
sh scripts/certify-tree-ring.sh
TREE_RING_CERT_EXTENDED=1 sh scripts/certify-tree-ring.sh
TREE_RING_AGENT_ZERO_ROOT=/Users/lazy/a0-ready-test sh scripts/certify-tree-ring.sh
```

## Task 5: Apply Code-Simplifier Cleanup

Files:

- `crates/tree-ring-memory-cli/src/welcome.rs`
- `crates/tree-ring-memory-cli/src/ring_mark.rs`
- `crates/tree-ring-memory-core/src/maintenance.rs`
- `crates/tree-ring-memory-core/src/models.rs`
- `crates/tree-ring-memory-sqlite/src/lib.rs`

Work:

- Remove the duplicate `welcome_logo_frame` branch.
- Remove or test `RingMarkFrame::layer_at`.
- Derive simple defaults where Clippy already identified manual equivalents.
- Simplify SQLite placeholder construction.
- Avoid broader renderer refactors unless a test requires them.

Checks:

```bash
cargo clippy --locked --all-targets
cargo test --locked
```

## Task 6: Document And Run Final Certification

Files:

- `README.md`
- `docs/architecture/rust-core-status.md`
- `docs/superpowers/specs/2026-07-09-tree-ring-performance-harness-certification-design.md`

Work:

- Document the certification script only after it works.
- Record current measured results with date and machine context.
- Avoid marketing claims or external benchmark claims that are not directly
  proved by the certification output.
- Update architecture status to mention project-vs-home marker reporting and
  the certification evidence artifact.

Final checks:

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets
cargo build --release --locked
sh scripts/certify-tree-ring.sh
git diff --check
```
