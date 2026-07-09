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

Final hardening semantics for this lane:

- Every category must carry a primary observation contract during validation.
  `constraint_recall` needs at least one `expected_recall`,
  `spam_prevention` needs at least one expected `reject` decision,
  `stale_truth_suppression` needs at least one `forbidden_recall`,
  `behavior_proof` needs `behavior_expectation`, and
  `evidence_preservation` needs at least one `evaluation_` write candidate.
- Threshold configuration is presence-aware. Threshold fields are optional in
  fixtures and default to strict behavior only when the corresponding metric
  has observations. An explicitly configured threshold for a metric with no
  observations is invalid and must fail scenario validation.
- Runner failures must emit stable stage plus error-class output in JSON,
  markdown, and terminal failure paths. Fixture values, invalid memory field
  values, and other raw payload fragments must not be echoed back in failure
  messages.

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
  `stale_truth_suppression`, `behavior_proof`, or `evidence_preservation`.
- `seed_memories`: memory events loaded into a temporary store.
- `query` or `workflow_prompt`: simulated agent task context.
- `expected_recall`: memory ids, rings, tags, or source refs that should appear.
- `forbidden_recall`: stale, superseded, sensitive, or weak memories that must
  not appear by default.
- `write_candidates`: optional proposed memories to evaluate before storage.
- `expected_write_decisions`: `accept`, `reject`, `require_evidence`, or
  `require_user_confirmation`.
- `evidence_refs`: source refs required for accepted outcome memories.
- `behavior_expectation`: required recalled memory id, baseline decision,
  memory-informed decision, expected decision, and an optional reason. It is
  required for `behavior_proof` scenarios.
- `thresholds`: per-scenario minimums or tolerances.
  Threshold entries are optional and only legal for metrics the scenario
  actually observes.

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
  query and changes a baseline retry into the expected rollback-safe decision.
- Synthetic edge case: evaluated outcomes without evidence are held while an
  outcome preserving the required evidence ref is accepted.

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
- `evidence_required_rate`: evaluated candidates correctly held when evidence
  is missing or accepted when the required evidence ref is preserved.
- `behavior_proof_pass_rate`: scenarios where memory changed the expected
  decision path.
- `quality_pass`: boolean certification gate.

Rates are observation-weighted across applicable expectations, not averaged
from per-scenario defaults. A scenario or run emits JSON `null` for a dimension
with no observations. Constraint and forbidden recall use their respective
expectation counts; spam uses expected `reject` decisions; evidence uses every
`evaluation_` write candidate; behavior uses scenarios with an explicit
behavior expectation. The default seven-scenario certification pack observes
all five dimensions, so its aggregate metrics remain numeric.

`quality_pass` should fail when required constraints are missed, forbidden
recall exceeds tolerance, spam candidates are accepted, or required evidence
refs are lost. Scenario thresholds apply only to dimensions with observations,
every write-decision report must match, and an empty run cannot pass.

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

- Fixture-facing objects reject unknown fields. Parse and validation failures
  should report the fixture path plus a stable error class, without extracting
  a scenario name or echoing invalid fields and other raw fixture values.
- Missing expected recall should produce a precise failed expectation.
- Forbidden recall should include the returned memory id and reason.
- Write-decision mismatches should show the candidate id or summary and the
  expected decision.
- Certification should preserve completed scenarios in partial JSON and
  markdown artifacts on failure. Reports include one terminal structured error
  with scenario and path when known, a stable stage, and a privacy-safe message.
- Markdown renders null rates as `n/a` and adds an Errors section when errors
  are present.
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
9. The seven-scenario default pack includes evidence preservation and produces
   numeric passing aggregate values for every quality metric.
10. Runner failures preserve structured partial artifacts without embedding
    fixture contents or memory payloads in error messages.

## Final-Review Hardening

The final review replaced inferred behavior proof with an explicit decision
change contract, made non-applicable metrics nullable and observation-weighted,
added strict fixture fields and the `evidence_preservation` category, and made
runner failures artifact-producing. This section and the seven-scenario pack
supersede the original six-fixture count while preserving the certification
gate's exact top-level `"quality_pass": true` JSON line.

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
