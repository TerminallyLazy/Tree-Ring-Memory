# Tree Ring Memory Quality Proof Loop Design

## Status

Approved brainstorming direction: build a Memory Quality Proof Loop.

This spec covers the design only. Implementation planning should follow after
user review.

## Intent

Tree Ring Memory already has the right memory lifecycle primitives: scoped
memory events, evidence records, DOX and Revolve adapters, audit,
consolidation, import/export, TUI views, and certification artifacts.

The next improvement should prove and improve whether Tree Ring actually helps
AI agents with long, dynamic, complex workflows. The product should not merely
store more memory. It should show that agents recall the right constraints,
avoid low-value writes, suppress stale or weak memory, and make better
decisions because memory was present.

## Goals

- Reduce missed constraints in complex agent workflows.
- Reduce memory spam from transient planning chatter, duplicate lessons, and
  low-value observations.
- Reduce stale-truth risk when old, superseded, sensitive, or weak memories
  compete with current source files, tests, user instructions, DOX contracts,
  or Revolve evidence.
- Add repeatable proof scenarios that show memory changed the expected agent
  decision path.
- Feed proven recall and write gates back into generated agent guidance.
- Keep the first proof surface CLI/CI-friendly before adding richer TUI
  quality views.
- Specify ambient ring fullness now so the visual surface can later reflect
  real quality and distribution signals.

## Non-Goals

- Do not add a daemon, sidecar, hosted service, telemetry pipeline, or hidden
  recorder.
- Do not scrape transcripts or turn event-stream pulses into durable memory.
- Do not add autonomous durable writes outside explicit user, agent, adapter,
  import, TUI, consolidation, or maintenance actions.
- Do not make Tree Ring memory more authoritative than source files, tests,
  explicit user instructions, root agent contracts, DOX contracts, or Revolve
  evidence.
- Do not replace the existing SQLite store, JSONL import/export shape, recall
  model, or certification workflow.
- Do not implement the full TUI quality cockpit in the first proof-loop pass.

## Selected Approach

Use an eval and certification-first proof loop, with guidance updates in the
same lane and TUI visualization as the downstream surface.

Considered approaches:

1. Guidance-only hardening
   - Tighten generated skills and bridges so agents recall before risky work,
     avoid low-value writes, and treat stale memory carefully.
   - Fast and useful, but weak as proof that behavior improved.

2. Eval and certification-first proof loop
   - Add repeatable workflow scenarios that test missed constraints, memory
     spam, stale recall, and behavior improvement.
   - Strongest credibility because Tree Ring can claim improvement from
     measurable scenarios rather than product copy.

3. Operator-first TUI quality cockpit
   - Surface fullness, stale-risk, low-confidence, and spam signals visually.
   - Valuable for human inspection, but it should follow trustworthy metrics
     instead of inventing meaning visually.

Recommended path: implement approach 2 first, take the practical generated
guidance pieces from approach 1, and make approach 3 consume the resulting
quality metrics.

## Architecture

Add a quality layer around existing Tree Ring primitives. The quality layer
consumes memory events, recall results, and proposed write candidates. It does
not own storage and does not need a background process.

Target components:

- `fixtures/quality/`: reviewable quality scenario fixtures.
- Quality scenario parser: validates scenario files and normalizes expected
  recall and write rules.
- Quality evaluator: runs deterministic checks over recall results and proposed
  memory writes.
- Certification integration: runs the default quality pack and writes summary
  artifacts beside current certification output.
- Generated guidance updates: teaches agents concrete recall, trust, and write
  gates that are backed by scenarios.
- TUI quality contract: defines how future visual views consume quality and
  fullness signals.

The existing SQLite store remains the durable memory backend. The existing
recall path remains the recall engine under test. The quality loop should test
current behavior before changing behavior.

## Scenario Model

Quality scenarios should be readable, deterministic, and code-reviewable.
JSON is the preferred first fixture format because the repo already uses JSON
schemas and fixture files for memory protocol coverage.

Each scenario should include:

- `name`: stable scenario id.
- `category`: `constraint_recall`, `spam_prevention`,
  `stale_truth_suppression`, or `behavior_proof`.
- `seed_memories`: memory events loaded into a temporary store.
- `query` or `workflow_prompt`: simulated agent task context.
- `expected_recall`: memory ids, rings, tags, or source refs that should appear.
- `forbidden_recall`: stale, superseded, sensitive, or weak memories that must
  not appear by default.
- `write_candidates`: optional proposed memories to evaluate before storage.
- `expected_write_decisions`: `accept`, `reject`, `require_evidence`, or
  `require_user_confirmation`.
- `evidence_refs`: source refs required for accepted outcome memories.
- `thresholds`: per-scenario minimums or tolerances.

First scenario pack:

- Real Tree Ring/Codex workflow: recall the no-background-writer constraint
  before proof-loop work.
- Real Tree Ring/Codex workflow: avoid storing transient planning chatter as
  durable heartwood.
- Real Tree Ring/Codex workflow: suppress stale PR or CLI-contract memory when
  current source has changed.
- Synthetic edge case: sensitive memory is blocked or hidden by default.
- Synthetic edge case: superseded heartwood does not outrank its replacement.
- Synthetic edge case: failed approach surfaces as a scar for a failure-like
  query.

## Data Flow

Quality runs should be explicit and reproducible:

1. Load a scenario fixture into a temporary `.tree-ring` root.
2. Run existing recall behavior against the scenario query or workflow prompt.
3. Evaluate returned memories against expected and forbidden recall rules.
4. Evaluate optional write candidates before storage.
5. Classify write candidates as useful, spam, stale, evidence-required, or
   user-confirmation-required.
6. Emit `quality-report.json` plus a short markdown summary.
7. Include the quality summary in certification output.
8. Feed durable lessons into generated agent guidance and later TUI metrics.

The quality loop should not mutate the user's real memory root. Scenario runs
use temporary stores and fixture-owned memory events.

## Agent Guidance Gates

Generated Tree Ring guidance should make agents do the right thing at the
moments where memory matters.

Recall gates:

- Before substantial project work, recall project constraints, scars, user
  preferences, and unresolved seeds.
- Before risky changes, recall warnings and evidence-linked prior failures.
- Before repeating a workflow, recall prior errors and accepted procedures.
- Before closeout, recall recent decisions so memory updates do not contradict
  already-stored lessons.

Trust gates:

- Prefer source-linked, non-superseded, high-confidence memories.
- Treat heartwood as durable only when source evidence or user confirmation
  supports it.
- Re-read source files, tests, user instructions, DOX contracts, or Revolve
  evidence when memory conflicts with current sources.
- Do not treat sensitive or hidden-by-default memory as ordinary recall context.

Write gates:

- Remember only durable decisions, validated lessons, reusable warnings,
  corrections, future seeds, and evidence-backed outcomes.
- Reject transient planning chatter, duplicate wording, tool noise, and
  unsupported claims as durable memory.
- Require evidence refs for promoted or rejected evaluated outcomes.
- Require user confirmation before creating or promoting broad cross-project
  heartwood.

## Metrics

The quality report should include:

- `constraint_recall_rate`: required constraints recalled before the workflow.
- `forbidden_recall_rate`: stale, sensitive, superseded, or weak memories
  returned when they should not be.
- `spam_rejection_rate`: low-value write candidates rejected.
- `evidence_required_rate`: accepted outcome memories that preserve required
  evidence refs.
- `behavior_proof_pass_rate`: scenarios where memory changed the expected
  decision path.
- `quality_pass`: boolean certification gate.

`quality_pass` should fail when required constraints are missed, forbidden
recall exceeds tolerance, spam candidates are accepted, or required evidence
refs are lost.

## TUI Contract

The TUI quality cockpit is downstream of CLI/CI quality proof. The first spec
still defines the visual contract so implementation work does not drift.

Ambient ring fullness uses a hybrid model:

- First pass: fullness is relative share of memories in the current store.
- Later pass: per-ring thresholds are calibrated from real usage and
  certification data.
- Empty rings stay dull or dim.
- Low/full rings are light but not visually dominant.
- High/full rings become brighter and more legible.
- Warning-heavy rings can override the palette, but fullness and warning are
  separate signals.
- Pulse remains activity, not fullness.

Current TUI internals already expose `RingStats.total`, `pulse_level`, warning
state, and per-cell brightness. Fullness should become part of the style
contract that maps ring distribution to intensity. It should not replace pulse
or warning behavior.

## Error Handling

- Invalid scenario fixtures should report the scenario name, file path, and
  invalid field.
- Missing expected recall should produce a precise failed expectation.
- Forbidden recall should include the returned memory id and reason.
- Write-decision mismatches should show the candidate id or summary and the
  expected decision.
- Certification should preserve partial quality artifacts on failure.
- Scenario runs should clean up temporary stores on success and avoid touching
  real user memory roots.

## Testing

Add tests in layers:

- Unit tests for scenario parsing and validation.
- Unit tests for deterministic recall expectation evaluation.
- Unit tests for write-candidate decision evaluation.
- Integration tests that load fixture stores and run recall checks.
- Certification tests that verify the default quality pack emits JSON and
  markdown artifacts.
- Generated guidance tests that check recall, trust, and write gates appear in
  `.tree-ring` awareness files and skill docs.
- Later TUI tests that verify fullness changes style intensity without changing
  layout or confusing pulse state.

## Acceptance Criteria

1. A default quality scenario pack exists under `fixtures/quality/`.
2. Quality runs use temporary memory stores and do not mutate real user roots.
3. Required recall expectations and forbidden recall expectations are
   deterministic.
4. Write candidates are classified as accept, reject, require evidence, or
   require user confirmation.
5. The default quality run emits `quality-report.json` and a markdown summary.
6. Certification includes quality metrics and fails on quality regressions.
7. Generated guidance includes explicit recall, trust, and write gates.
8. TUI fullness semantics are documented as hybrid relative-share first,
   calibrated thresholds later.

## Rollout

Phase 1: Add fixture format, evaluator, default quality pack, reports, and
certification integration.

Phase 2: Update generated guidance and skill docs so agents use the proven
recall, trust, and write gates.

Phase 3: Add TUI quality and fullness visualization after CLI/CI metrics are
stable.

## Open Follow-Up

After the first quality pack is running, choose whether the stable surface
should be a dedicated `tree-ring eval` subcommand or remain certification-only
until the fixture format settles.
