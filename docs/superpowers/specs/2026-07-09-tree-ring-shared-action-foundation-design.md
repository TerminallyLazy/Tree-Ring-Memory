# Tree Ring Shared Action Foundation Design

## Status

Approved brainstorming direction: implement the improvement roadmap in this
order:

1. Shared action foundation and core simplification.
2. TUI operator cockpit and integration-link workflow.
3. Harness certification matrix.
4. Recall quality dashboard and observability.

This spec covers the first lane only. Later lanes are named here as downstream
consumers, not implementation scope.

## Intent

Tree Ring Memory is now Rust-native across the CLI, SQLite store, TUI, import
and export, maintenance, consolidation, DOX/Revolve adapters, and integration
discovery. The next improvement should reduce internal coupling before adding
more operator and harness features.

The current pressure points are large files that mix parsing, operation
semantics, storage calls, and presentation:

- `crates/tree-ring-memory-cli/src/main.rs`
- `crates/tree-ring-memory-cli/src/tui/app.rs`
- `crates/tree-ring-memory-cli/src/tui/render.rs`
- `crates/tree-ring-memory-sqlite/src/lib.rs`

The shared action foundation should make common durable operations reusable by
both CLI and TUI callers. It should preserve existing behavior while creating a
clean place for later `/audit`, `/maintain`, real `/sync`, and
`integrations link` workflows.

## Goals

- Keep user-facing behavior stable while simplifying internal boundaries.
- Move operation semantics out of CLI parsing and TUI state handling.
- Give CLI and TUI shared request/report contracts for durable operations.
- Keep `SQLiteMemoryStore` as the public storage facade while allowing focused
  internal modules.
- Prepare the TUI cockpit lane to call shared actions instead of duplicating
  CLI behavior.
- Keep tests and certification as the safety net for the refactor.

## Non-Goals

- Do not add new user-facing product behavior in this lane.
- Do not change the SQLite schema or public JSONL schema.
- Do not change recall ranking, sensitivity classification, adapter summaries,
  generated guidance wording, installer behavior, or TUI layout.
- Do not add a daemon, MCP server, background recorder, or hidden durable
  memory writer.
- Do not bind Tree Ring Memory to one agent harness.

## Architecture

The target shape is a behavior-preserving split with a shared action layer:

```text
crates/tree-ring-memory-cli/src/
├── main.rs                  # clap parsing and thin dispatch
├── commands/                # CLI command handlers and output formatting
├── actions/                 # shared operation semantics
│   ├── remember.rs
│   ├── recall.rs
│   ├── export_import.rs
│   ├── audit.rs
│   ├── maintain.rs
│   ├── consolidate.rs
│   ├── adapters.rs
│   └── integrations.rs
└── tui/                     # TUI input, state, rendering, confirmations

crates/tree-ring-memory-sqlite/src/
├── lib.rs                   # public SQLiteMemoryStore facade
├── schema.rs                # open/init/migrations/helpers
├── write.rs                 # put/delete/redact/supersede helpers
├── search.rs                # list/search/recall filtering helpers
├── import_export.rs         # JSONL store integration helpers
└── lifecycle.rs             # audit/maintain/consolidate storage glue
```

The action layer belongs in the CLI crate for this pass because it coordinates
CLI/TUI behavior and depends on the SQLite store. The core crate should remain
focused on portable models, validation, recall scoring, sensitivity, audit
planning, consolidation planning, maintenance planning, and adapters that do
not require terminal or command presentation concerns.

## Shared Action Contracts

Actions should use request/report structs instead of CLI arg structs or TUI app
state. Each action should answer:

- What operation is being requested?
- What durable operation was attempted?
- What changed?
- What concise status should a caller display?

Initial action families:

- `RememberRequest` / `RememberReport`
- `RecallRequest` / `RecallReport`
- `ExportRequest` / `ExportReport`
- `ImportRequest` / `ImportReport`
- `AuditActionRequest` / `AuditActionReport`
- `MaintainActionRequest` / `MaintainActionReport`
- `ConsolidateActionRequest` / `ConsolidateActionReport`
- `AdapterSyncRequest` / `AdapterSyncReport`
- `IntegrationScanRequest` / `IntegrationScanReport`

The exact Rust names may vary to avoid collisions with existing core types, but
the boundary should stay consistent.

The CLI remains responsible for:

- Clap argument parsing.
- Text and JSON presentation.
- Exit behavior.
- Help text.

The TUI remains responsible for:

- Mode transitions.
- Selection state.
- Confirmation panels.
- Rendering.
- Keyboard and slash-command input.

The action layer owns:

- Validation and sensitivity handling for a requested operation.
- Store calls.
- Operation-level warnings.
- Structured success reports.
- Structured errors where they materially improve caller behavior.

## Data Flow

CLI flow:

```text
CLI args
  -> commands::<command>
  -> actions::<operation>
  -> SQLiteMemoryStore facade
  -> tree-ring-memory-core logic where applicable
  -> commands::<command> formats text or JSON
```

TUI flow:

```text
TUI key/slash input
  -> tui::app selection and confirmation state
  -> actions::<operation>
  -> SQLiteMemoryStore facade
  -> tree-ring-memory-core logic where applicable
  -> TUI status/report rendering
```

Storage flow:

```text
actions::<operation>
  -> SQLiteMemoryStore public method
  -> focused internal SQLite helper module
  -> SQLite transaction/query
```

`SQLiteMemoryStore` should remain the public storage API used by tests and
callers. Internal modules should reduce file size and clarify responsibilities
without forcing downstream consumers to know about the split.

## Migration Strategy

The refactor should move one action family at a time.

### Phase 1: Simple Action Extraction

Start with flows that already have clear behavior and strong tests:

- `remember`
- `recall`
- `export`
- `audit`

Keep CLI output and JSON stable. Update the TUI to call the shared action only
where its current behavior is equivalent, especially `/remember` and `/export`.

### Phase 2: Lifecycle And Adapter Actions

Move operation semantics for:

- `import`
- `consolidate`
- `maintain`
- `dox sync`
- `revolve sync`
- `integrations scan`

Keep dry-run behavior unchanged. Keep source refs, adapter summaries,
sensitivity behavior, and supersession behavior unchanged.

### Phase 3: Storage Internal Split

Split SQLite internals only around seams already touched by actions:

- schema and open/init helpers
- write helpers
- search and recall helpers
- import/export helpers
- lifecycle helpers

Avoid reshaping the storage API beyond private/internal module boundaries.

## Error Handling

Action functions should return operation-specific `Result<Report, Error>`
values. A perfect global error hierarchy is not required for this pass, but
errors should preserve context.

Expected behavior:

- CLI turns action reports and errors into existing text or JSON output.
- TUI turns action reports and errors into status strings and confirmation
  panels.
- Storage errors stay specific enough to diagnose the failing operation.
- Secret-like input and sensitive-memory behavior remain fail-closed where they
  already fail closed.
- Existing non-mutating dry-run semantics remain non-mutating.

## TUI Boundary

This lane does not redesign the TUI. It prepares the TUI cockpit lane by making
the TUI call shared actions.

Allowed TUI changes:

- Replace inline durable operation logic with action calls.
- Keep confirmation ownership in `tui::app`.
- Keep rendering ownership in `tui::render`.
- Add small report-to-status adapters when needed.

Deferred to the next lane:

- `/audit`
- `/maintain`
- real `/sync`
- guided integration linking
- richer filters
- score explanation panels
- visual layout changes

## Testing

Verification should focus on behavior preservation:

- Full `cargo test --locked`.
- `cargo clippy --locked --all-targets`.
- `cargo fmt --check`.
- `git diff --check`.
- `sh scripts/certify-tree-ring.sh`.
- Focused CLI JSON checks for remember, recall, export, import, audit,
  consolidate, maintain, DOX sync, Revolve sync, and integrations scan.
- TUI action tests for `/remember`, `/export`, `/consolidate`, and existing
  confirmation flows.

Where practical, tests should exercise the action layer directly and also
confirm CLI/TUI callers still behave as before.

## Acceptance Criteria

- `main.rs`, `tui/app.rs`, and `sqlite/lib.rs` become meaningfully thinner or
  their responsibilities become clearly narrower.
- Shared actions exist for the main durable operations.
- CLI and TUI both use at least the first shared actions.
- Existing user-facing CLI text and JSON behavior remain stable.
- Existing TUI behavior remains stable.
- Existing tests pass.
- Certification still passes.
- The next TUI cockpit design can add lifecycle and integration workflows by
  calling action contracts instead of duplicating command logic.

## Downstream Roadmap

After this lane:

1. **TUI operator cockpit** can add `/audit`, `/maintain`, real `/sync`, and
   guided integration linking through shared actions.
2. **Harness certification matrix** can certify real CLI and bridge flows
   against Codex, Claude Code, OpenCode, Goose, Pi, Agent Zero, DOX, and
   Revolve.
3. **Recall quality dashboard** can add ranking explanations, evaluation
   fixtures, stale-memory visibility, and contradiction visibility without
   mixing those concerns into CLI parsing or TUI state management.

## Spec Self-Review

- Incomplete-marker scan: no incomplete markers remain.
- Consistency check: architecture, action contracts, data flow, and testing all
  preserve existing public behavior.
- Scope check: this is a single refactor foundation lane; later TUI, harness,
  and recall-quality work is deferred.
- Ambiguity check: behavior changes are explicitly out of scope; public storage
  and CLI/TUI behavior should remain stable.
