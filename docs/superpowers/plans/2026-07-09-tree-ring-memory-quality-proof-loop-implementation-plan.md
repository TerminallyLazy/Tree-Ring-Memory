# Tree Ring Memory Quality Proof Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a certification-backed memory quality loop that proves Tree Ring helps agents recall constraints, reject memory spam, suppress stale truth, and preserve evidence in complex workflows.

**Architecture:** Add a deterministic quality scenario model and evaluator to `tree-ring-memory-core`, then run it through a certification-only CLI example that loads reviewable fixtures into temporary SQLite stores. Feed the resulting gates into generated agent guidance, and add first-pass TUI fullness styling from the current ring distribution without creating a daemon, hidden writer, or public eval subcommand.

**Tech Stack:** Rust 2021, serde, serde_json, rusqlite-backed `SQLiteMemoryStore`, existing `MemoryRetriever`, Ratatui, shell certification script, JSON fixtures.

## Global Constraints

- Do not add a daemon, sidecar, hosted service, telemetry pipeline, or hidden recorder.
- Do not scrape transcripts or turn event-stream pulses into durable memory.
- Do not add autonomous durable writes outside explicit user, agent, adapter, import, TUI, consolidation, or maintenance actions.
- Do not make Tree Ring memory more authoritative than source files, tests, explicit user instructions, root agent contracts, DOX contracts, or Revolve evidence.
- Do not replace the existing SQLite store, JSONL import/export shape, recall model, or certification workflow.
- Do not add a public `tree-ring eval` subcommand in this pass; keep the quality runner certification-owned until the fixture format settles.
- Keep scenario runs isolated in temporary memory roots.
- Keep TUI fullness separate from pulse and warning state.
- Preserve existing untracked local files; stage only files changed by each task.
- Run `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked --all-targets`, `git diff --check`, and `sh scripts/certify-tree-ring.sh` before final handoff.

## Final Hardening Amendment

This implementation plan now carries the final hardening semantics that
supersede earlier draft snippets:

- Category validation must enforce a primary observation contract:
  `constraint_recall` requires `expected_recall`,
  `spam_prevention` requires at least one expected `reject`,
  `stale_truth_suppression` requires `forbidden_recall`,
  `behavior_proof` requires `behavior_expectation`, and
  `evidence_preservation` requires at least one `evaluation_` write candidate.
- `QualityThresholds` are presence-aware optional fields. Omitted thresholds
  inherit strict defaults only when their metric has observations. Configuring
  a threshold for an inapplicable metric is a validation failure, not a silent
  no-op.
- Runner failures must be sanitized before they are written to
  `quality-report.json`, `quality-summary.md`, or bubbled back through the
  example entrypoint. Reports keep stage plus stable error class, but never raw
  fixture payload values.
- `QualityScenarioReport.behavior_expectation` must deserialize as `None` when
  older artifacts omit the field.

---

## Scope Check

This plan covers one subsystem: a memory quality proof loop and its first consumer surfaces. It produces working, testable software in layers:

1. Core scenario parsing.
2. Core quality evaluation.
3. Default quality fixtures.
4. Certification-only runner.
5. Certification report integration.
6. Generated guidance gates.
7. TUI fullness styling.

No task creates a background process, network service, public CLI command, or alternate storage backend.

## File Structure

- Create `crates/tree-ring-memory-core/src/quality.rs`: scenario structs, parser, deterministic evaluator, quality run aggregation.
- Modify `crates/tree-ring-memory-core/src/lib.rs`: expose the quality module and public quality types.
- Create `fixtures/quality/no-background-writer-constraint.json`: real workflow constraint recall scenario.
- Create `fixtures/quality/transient-planning-spam.json`: real workflow spam-prevention scenario.
- Create `fixtures/quality/stale-cli-contract.json`: real workflow stale-truth suppression scenario.
- Create `fixtures/quality/sensitive-memory-hidden.json`: synthetic sensitive recall scenario.
- Create `fixtures/quality/superseded-heartwood.json`: synthetic supersession scenario.
- Create `fixtures/quality/scar-failure-recall.json`: synthetic scar recall scenario.
- Create `crates/tree-ring-memory-cli/examples/quality_scenarios.rs`: certification-only runner that loads fixtures, runs recall, evaluates reports, and writes JSON/Markdown artifacts.
- Modify `scripts/certify-tree-ring.sh`: run the quality example and include quality metrics in certification artifacts.
- Modify `crates/tree-ring-memory-cli/src/agent_awareness.rs`: generated AGENTS and CLI guidance gates.
- Modify `skills/tree-ring-memory/SKILL.md`: portable recall, trust, and write gates.
- Modify `docs/integrations/agent-skill.md`: document quality-gate usage for agent harnesses.
- Modify `README.md`: document certification quality artifacts.
- Modify `crates/tree-ring-memory-cli/src/tui/model.rs`: add first-pass relative fullness.
- Modify `crates/tree-ring-memory-cli/src/tui/rings.rs`: map fullness to ambient intensity without changing pulse/warning semantics.

---

### Task 1: Add Quality Scenario Model

**Files:**
- Create: `crates/tree-ring-memory-core/src/quality.rs`
- Modify: `crates/tree-ring-memory-core/src/lib.rs`

**Interfaces:**
- Produces: `quality::QualityScenario`
- Produces: `quality::QualityThresholds`
- Produces: `quality::RecallExpectation`
- Produces: `quality::WriteDecisionExpectation`
- Produces: `quality::parse_quality_scenario(input: &str) -> TreeRingResult<QualityScenario>`
- Consumes: `models::MemoryEvent`
- Consumes: `models::TreeRingError`

- [ ] **Step 1: Write the failing scenario parser tests**

Add this complete file at `crates/tree-ring-memory-core/src/quality.rs`:

```rust
use serde::{Deserialize, Serialize};

use crate::models::{MemoryEvent, TreeRingError, TreeRingResult};

pub const QUALITY_CATEGORIES: &[&str] = &[
    "constraint_recall",
    "spam_prevention",
    "stale_truth_suppression",
    "behavior_proof",
];

pub const WRITE_DECISIONS: &[&str] = &[
    "accept",
    "reject",
    "require_evidence",
    "require_user_confirmation",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityScenario {
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub seed_memories: Vec<MemoryEvent>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub workflow_prompt: Option<String>,
    #[serde(default)]
    pub expected_recall: Vec<RecallExpectation>,
    #[serde(default)]
    pub forbidden_recall: Vec<RecallExpectation>,
    #[serde(default)]
    pub write_candidates: Vec<MemoryEvent>,
    #[serde(default)]
    pub expected_write_decisions: Vec<WriteDecisionExpectation>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub thresholds: QualityThresholds,
}

impl QualityScenario {
    pub fn prompt(&self) -> Option<&str> {
        self.query
            .as_deref()
            .or_else(|| self.workflow_prompt.as_deref())
    }

    pub fn validate(&self) -> TreeRingResult<()> {
        if self.name.trim().is_empty() {
            return Err(TreeRingError::Validation(
                "quality scenario name is required".to_string(),
            ));
        }
        validate_member("quality category", &self.category, QUALITY_CATEGORIES)?;
        if self
            .prompt()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(TreeRingError::Validation(format!(
                "quality scenario {} requires query or workflow_prompt",
                self.name
            )));
        }
        for memory in self.seed_memories.iter().chain(self.write_candidates.iter()) {
            memory.validate()?;
        }
        for expectation in &self.expected_recall {
            expectation.validate("expected_recall", &self.name)?;
        }
        for expectation in &self.forbidden_recall {
            expectation.validate("forbidden_recall", &self.name)?;
        }
        for decision in &self.expected_write_decisions {
            decision.validate(&self.name)?;
        }
        self.thresholds.validate(&self.name)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecallExpectation {
    #[serde(default)]
    pub memory_id: Option<String>,
    #[serde(default)]
    pub ring: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub source_ref: Option<String>,
    #[serde(default)]
    pub reason: String,
}

impl RecallExpectation {
    fn validate(&self, field: &str, scenario_name: &str) -> TreeRingResult<()> {
        if self.memory_id.is_none()
            && self.ring.is_none()
            && self.tag.is_none()
            && self.source_ref.is_none()
        {
            return Err(TreeRingError::Validation(format!(
                "quality scenario {scenario_name} {field} entry needs memory_id, ring, tag, or source_ref"
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteDecisionExpectation {
    pub memory_id: String,
    pub decision: String,
    #[serde(default)]
    pub reason: String,
}

impl WriteDecisionExpectation {
    fn validate(&self, scenario_name: &str) -> TreeRingResult<()> {
        if self.memory_id.trim().is_empty() {
            return Err(TreeRingError::Validation(format!(
                "quality scenario {scenario_name} write decision requires memory_id"
            )));
        }
        validate_member("write decision", &self.decision, WRITE_DECISIONS)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityThresholds {
    #[serde(default = "one")]
    pub min_constraint_recall_rate: f64,
    #[serde(default)]
    pub max_forbidden_recall_rate: f64,
    #[serde(default = "one")]
    pub min_spam_rejection_rate: f64,
    #[serde(default = "one")]
    pub min_evidence_required_rate: f64,
    #[serde(default = "one")]
    pub min_behavior_proof_pass_rate: f64,
}

impl Default for QualityThresholds {
    fn default() -> Self {
        Self {
            min_constraint_recall_rate: 1.0,
            max_forbidden_recall_rate: 0.0,
            min_spam_rejection_rate: 1.0,
            min_evidence_required_rate: 1.0,
            min_behavior_proof_pass_rate: 1.0,
        }
    }
}

impl QualityThresholds {
    fn validate(&self, scenario_name: &str) -> TreeRingResult<()> {
        for (field, value) in [
            (
                "min_constraint_recall_rate",
                self.min_constraint_recall_rate,
            ),
            ("max_forbidden_recall_rate", self.max_forbidden_recall_rate),
            ("min_spam_rejection_rate", self.min_spam_rejection_rate),
            ("min_evidence_required_rate", self.min_evidence_required_rate),
            (
                "min_behavior_proof_pass_rate",
                self.min_behavior_proof_pass_rate,
            ),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(TreeRingError::Validation(format!(
                    "quality scenario {scenario_name} threshold {field} must be between 0 and 1"
                )));
            }
        }
        Ok(())
    }
}

pub fn parse_quality_scenario(input: &str) -> TreeRingResult<QualityScenario> {
    let scenario: QualityScenario = serde_json::from_str(input)?;
    scenario.validate()?;
    Ok(scenario)
}

fn validate_member(field: &str, value: &str, allowed: &[&str]) -> TreeRingResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(TreeRingError::Validation(format!(
            "invalid {field}: {value}"
        )))
    }
}

fn one() -> f64 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_quality_scenario() {
        let input = r#"{
          "name": "constraint recall",
          "category": "constraint_recall",
          "query": "proof loop background writer",
          "seed_memories": [
            {
              "id": "mem_quality_constraint",
              "created_at": "2026-07-09T00:00:00Z",
              "updated_at": "2026-07-09T00:00:00Z",
              "project": "tree-ring",
              "agent_profile": null,
              "scope": "project",
              "ring": "heartwood",
              "event_type": "decision",
              "summary": "Do not add a background writer to proof loop work.",
              "details": "",
              "source": {"type": "evidence", "ref": "docs/spec.md", "quote": ""},
              "tags": ["proof-loop"],
              "salience": 0.9,
              "confidence": 0.9,
              "sensitivity": "normal",
              "retention": "durable",
              "expires_at": null,
              "supersedes": [],
              "superseded_by": null,
              "links": [],
              "review": {"needs_review": false, "review_reason": null, "reviewed_at": null, "reviewed_by": null}
            }
          ],
          "expected_recall": [
            {"memory_id": "mem_quality_constraint", "reason": "must recall proof-loop constraint"}
          ]
        }"#;

        let scenario = parse_quality_scenario(input).unwrap();

        assert_eq!(scenario.name, "constraint recall");
        assert_eq!(scenario.category, "constraint_recall");
        assert_eq!(scenario.seed_memories.len(), 1);
        assert_eq!(scenario.expected_recall[0].memory_id.as_deref(), Some("mem_quality_constraint"));
    }

    #[test]
    fn rejects_scenario_without_prompt() {
        let input = r#"{"name":"bad","category":"constraint_recall"}"#;

        let error = parse_quality_scenario(input).unwrap_err().to_string();

        assert!(error.contains("requires query or workflow_prompt"));
    }

    #[test]
    fn rejects_unknown_write_decision() {
        let input = r#"{
          "name": "bad decision",
          "category": "spam_prevention",
          "query": "planning chatter",
          "expected_write_decisions": [
            {"memory_id": "mem_candidate", "decision": "maybe"}
          ]
        }"#;

        let error = parse_quality_scenario(input).unwrap_err().to_string();

        assert!(error.contains("invalid write decision"));
    }
}
```

- [ ] **Step 2: Export the quality module**

Modify `crates/tree-ring-memory-core/src/lib.rs` by adding the module declaration with the other modules:

```rust
pub mod quality;
```

Then add this public export block after the existing `pub use models::{...};` block:

```rust
pub use quality::{
    parse_quality_scenario, QualityScenario, QualityThresholds, RecallExpectation,
    WriteDecisionExpectation, QUALITY_CATEGORIES, WRITE_DECISIONS,
};
```

- [ ] **Step 3: Run the focused parser tests**

Run:

```bash
cargo test -p tree-ring-memory-core quality:: --locked
```

Expected: PASS with the three quality parser tests passing.

- [ ] **Step 4: Commit Task 1**

Run:

```bash
git add crates/tree-ring-memory-core/src/lib.rs crates/tree-ring-memory-core/src/quality.rs
git commit -m "Add quality scenario model"
```

---

### Task 2: Add Deterministic Quality Evaluator

**Files:**
- Modify: `crates/tree-ring-memory-core/src/quality.rs`
- Modify: `crates/tree-ring-memory-core/src/lib.rs`

**Interfaces:**
- Consumes: `QualityScenario`
- Produces: `quality::QualityRecall`
- Produces: `quality::QualityScenarioReport`
- Produces: `quality::QualityRunReport`
- Produces: `quality::evaluate_quality_scenario(scenario: &QualityScenario, recalls: &[QualityRecall]) -> TreeRingResult<QualityScenarioReport>`
- Produces: `quality::summarize_quality_run(reports: Vec<QualityScenarioReport>) -> QualityRunReport`

- [ ] **Step 1: Add failing evaluator tests**

Append these tests inside the existing `#[cfg(test)] mod tests` in `crates/tree-ring-memory-core/src/quality.rs`:

```rust
    fn memory(id: &str, summary: &str, ring: &str) -> MemoryEvent {
        let mut event = MemoryEvent::new(summary, "lesson").unwrap();
        event.id = id.to_string();
        event.created_at = "2026-07-09T00:00:00Z".to_string();
        event.updated_at = "2026-07-09T00:00:00Z".to_string();
        event.scope = "project".to_string();
        event.project = Some("tree-ring".to_string());
        event.ring = ring.to_string();
        event.source.source_type = "evidence".to_string();
        event.source.ref_ = "docs/spec.md".to_string();
        event.salience = 0.8;
        event.confidence = 0.8;
        event
    }

    #[test]
    fn evaluates_required_and_forbidden_recall() {
        let mut scenario = QualityScenario {
            name: "recall gate".to_string(),
            category: "constraint_recall".to_string(),
            seed_memories: Vec::new(),
            query: Some("background writer".to_string()),
            workflow_prompt: None,
            expected_recall: vec![RecallExpectation {
                memory_id: Some("mem_required".to_string()),
                reason: "required constraint".to_string(),
                ..Default::default()
            }],
            forbidden_recall: vec![RecallExpectation {
                memory_id: Some("mem_forbidden".to_string()),
                reason: "stale memory".to_string(),
                ..Default::default()
            }],
            write_candidates: Vec::new(),
            expected_write_decisions: Vec::new(),
            evidence_refs: Vec::new(),
            thresholds: QualityThresholds::default(),
        };
        scenario.validate().unwrap();

        let report = evaluate_quality_scenario(
            &scenario,
            &[QualityRecall {
                memory: memory("mem_required", "Do not add a background writer.", "heartwood"),
                score: 0.91,
            }],
        )
        .unwrap();

        assert!(report.quality_pass);
        assert_eq!(report.constraint_recall_rate, 1.0);
        assert_eq!(report.forbidden_recall_rate, 0.0);
        assert!(report.expected_recall[0].passed);
        assert!(report.forbidden_recall[0].passed);
    }

    #[test]
    fn fails_when_forbidden_memory_is_recalled() {
        let scenario = QualityScenario {
            name: "stale recall".to_string(),
            category: "stale_truth_suppression".to_string(),
            seed_memories: Vec::new(),
            query: Some("cli contract".to_string()),
            workflow_prompt: None,
            expected_recall: Vec::new(),
            forbidden_recall: vec![RecallExpectation {
                memory_id: Some("mem_stale".to_string()),
                reason: "stale CLI contract".to_string(),
                ..Default::default()
            }],
            write_candidates: Vec::new(),
            expected_write_decisions: Vec::new(),
            evidence_refs: Vec::new(),
            thresholds: QualityThresholds::default(),
        };

        let report = evaluate_quality_scenario(
            &scenario,
            &[QualityRecall {
                memory: memory("mem_stale", "Old CLI contract.", "heartwood"),
                score: 0.88,
            }],
        )
        .unwrap();

        assert!(!report.quality_pass);
        assert_eq!(report.forbidden_recall_rate, 1.0);
        assert!(!report.forbidden_recall[0].passed);
    }

    #[test]
    fn classifies_write_candidates_against_expected_decisions() {
        let mut spam = memory("mem_spam", "Thinking about options for a moment.", "heartwood");
        spam.tags = vec!["transient".to_string()];
        let mut promoted_without_evidence =
            memory("mem_missing_evidence", "Promote evaluated proof.", "heartwood");
        promoted_without_evidence.event_type = "evaluation_promotion".to_string();
        promoted_without_evidence.source.ref_.clear();
        let mut broad_heartwood =
            memory("mem_needs_confirmation", "All projects should prefer this.", "heartwood");
        broad_heartwood.source.ref_.clear();

        let scenario = QualityScenario {
            name: "write gates".to_string(),
            category: "spam_prevention".to_string(),
            seed_memories: Vec::new(),
            query: Some("write gates".to_string()),
            workflow_prompt: None,
            expected_recall: Vec::new(),
            forbidden_recall: Vec::new(),
            write_candidates: vec![spam, promoted_without_evidence, broad_heartwood],
            expected_write_decisions: vec![
                WriteDecisionExpectation {
                    memory_id: "mem_spam".to_string(),
                    decision: "reject".to_string(),
                    reason: "transient planning chatter".to_string(),
                },
                WriteDecisionExpectation {
                    memory_id: "mem_missing_evidence".to_string(),
                    decision: "require_evidence".to_string(),
                    reason: "promotion needs evidence".to_string(),
                },
                WriteDecisionExpectation {
                    memory_id: "mem_needs_confirmation".to_string(),
                    decision: "require_user_confirmation".to_string(),
                    reason: "broad heartwood needs confirmation".to_string(),
                },
            ],
            evidence_refs: vec!["evals/run-001".to_string()],
            thresholds: QualityThresholds::default(),
        };

        let report = evaluate_quality_scenario(&scenario, &[]).unwrap();

        assert!(report.quality_pass);
        assert_eq!(report.spam_rejection_rate, 1.0);
        assert_eq!(report.evidence_required_rate, 1.0);
        assert_eq!(report.write_decisions.len(), 3);
    }

    #[test]
    fn summarizes_quality_run() {
        let passing = QualityScenarioReport {
            name: "pass".to_string(),
            category: "constraint_recall".to_string(),
            constraint_recall_rate: 1.0,
            forbidden_recall_rate: 0.0,
            spam_rejection_rate: 1.0,
            evidence_required_rate: 1.0,
            behavior_proof_pass: true,
            quality_pass: true,
            expected_recall: Vec::new(),
            forbidden_recall: Vec::new(),
            write_decisions: Vec::new(),
        };

        let run = summarize_quality_run(vec![passing.clone(), passing]);

        assert!(run.quality_pass);
        assert_eq!(run.scenario_count, 2);
        assert_eq!(run.behavior_proof_pass_rate, 1.0);
    }
```

- [ ] **Step 2: Run the focused evaluator tests and verify missing types**

Run:

```bash
cargo test -p tree-ring-memory-core quality:: --locked
```

Expected: FAIL because `QualityRecall`, `evaluate_quality_scenario`, `QualityScenarioReport`, and `summarize_quality_run` are not defined.

- [ ] **Step 3: Add evaluator types and functions**

Insert this code in `crates/tree-ring-memory-core/src/quality.rs` above `pub fn parse_quality_scenario`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityRecall {
    pub memory: MemoryEvent,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecallExpectationReport {
    pub expectation: RecallExpectation,
    pub matched_memory_ids: Vec<String>,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WriteDecisionReport {
    pub memory_id: String,
    pub expected: String,
    pub actual: String,
    pub passed: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityScenarioReport {
    pub name: String,
    pub category: String,
    pub constraint_recall_rate: f64,
    pub forbidden_recall_rate: f64,
    pub spam_rejection_rate: f64,
    pub evidence_required_rate: f64,
    pub behavior_proof_pass: bool,
    pub quality_pass: bool,
    pub expected_recall: Vec<RecallExpectationReport>,
    pub forbidden_recall: Vec<RecallExpectationReport>,
    pub write_decisions: Vec<WriteDecisionReport>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityRunReport {
    pub ok: bool,
    pub scenario_count: usize,
    pub constraint_recall_rate: f64,
    pub forbidden_recall_rate: f64,
    pub spam_rejection_rate: f64,
    pub evidence_required_rate: f64,
    pub behavior_proof_pass_rate: f64,
    pub quality_pass: bool,
    pub scenarios: Vec<QualityScenarioReport>,
}

pub fn evaluate_quality_scenario(
    scenario: &QualityScenario,
    recalls: &[QualityRecall],
) -> TreeRingResult<QualityScenarioReport> {
    scenario.validate()?;

    let expected_recall = scenario
        .expected_recall
        .iter()
        .map(|expectation| expected_recall_report(expectation, recalls))
        .collect::<Vec<_>>();
    let forbidden_recall = scenario
        .forbidden_recall
        .iter()
        .map(|expectation| forbidden_recall_report(expectation, recalls))
        .collect::<Vec<_>>();
    let write_decisions = scenario
        .expected_write_decisions
        .iter()
        .map(|expectation| write_decision_report(expectation, scenario))
        .collect::<Vec<_>>();

    let constraint_recall_rate = pass_rate(&expected_recall);
    let forbidden_recall_rate = failure_rate(&forbidden_recall);
    let spam_rejection_rate = decision_pass_rate(&write_decisions, "reject");
    let evidence_required_rate = decision_pass_rate(&write_decisions, "require_evidence");
    let behavior_proof_pass =
        constraint_recall_rate >= scenario.thresholds.min_constraint_recall_rate
            && forbidden_recall_rate <= scenario.thresholds.max_forbidden_recall_rate;
    let quality_pass = behavior_proof_pass
        && spam_rejection_rate >= scenario.thresholds.min_spam_rejection_rate
        && evidence_required_rate >= scenario.thresholds.min_evidence_required_rate;

    Ok(QualityScenarioReport {
        name: scenario.name.clone(),
        category: scenario.category.clone(),
        constraint_recall_rate,
        forbidden_recall_rate,
        spam_rejection_rate,
        evidence_required_rate,
        behavior_proof_pass,
        quality_pass,
        expected_recall,
        forbidden_recall,
        write_decisions,
    })
}

pub fn summarize_quality_run(reports: Vec<QualityScenarioReport>) -> QualityRunReport {
    let scenario_count = reports.len();
    let denominator = scenario_count.max(1) as f64;
    let constraint_recall_rate =
        reports.iter().map(|report| report.constraint_recall_rate).sum::<f64>() / denominator;
    let forbidden_recall_rate =
        reports.iter().map(|report| report.forbidden_recall_rate).sum::<f64>() / denominator;
    let spam_rejection_rate =
        reports.iter().map(|report| report.spam_rejection_rate).sum::<f64>() / denominator;
    let evidence_required_rate =
        reports.iter().map(|report| report.evidence_required_rate).sum::<f64>() / denominator;
    let behavior_proof_pass_rate = reports
        .iter()
        .filter(|report| report.behavior_proof_pass)
        .count() as f64
        / denominator;
    let quality_pass = reports.iter().all(|report| report.quality_pass);

    QualityRunReport {
        ok: quality_pass,
        scenario_count,
        constraint_recall_rate,
        forbidden_recall_rate,
        spam_rejection_rate,
        evidence_required_rate,
        behavior_proof_pass_rate,
        quality_pass,
        scenarios: reports,
    }
}

fn expected_recall_report(
    expectation: &RecallExpectation,
    recalls: &[QualityRecall],
) -> RecallExpectationReport {
    let matched_memory_ids = matching_recall_ids(expectation, recalls);
    RecallExpectationReport {
        expectation: expectation.clone(),
        passed: !matched_memory_ids.is_empty(),
        matched_memory_ids,
    }
}

fn forbidden_recall_report(
    expectation: &RecallExpectation,
    recalls: &[QualityRecall],
) -> RecallExpectationReport {
    let matched_memory_ids = matching_recall_ids(expectation, recalls);
    RecallExpectationReport {
        expectation: expectation.clone(),
        passed: matched_memory_ids.is_empty(),
        matched_memory_ids,
    }
}

fn matching_recall_ids(expectation: &RecallExpectation, recalls: &[QualityRecall]) -> Vec<String> {
    recalls
        .iter()
        .filter(|recall| expectation_matches_memory(expectation, &recall.memory))
        .map(|recall| recall.memory.id.clone())
        .collect()
}

fn expectation_matches_memory(expectation: &RecallExpectation, memory: &MemoryEvent) -> bool {
    if expectation
        .memory_id
        .as_deref()
        .is_some_and(|id| memory.id != id)
    {
        return false;
    }
    if expectation
        .ring
        .as_deref()
        .is_some_and(|ring| memory.ring != ring)
    {
        return false;
    }
    if expectation
        .tag
        .as_deref()
        .is_some_and(|tag| !memory.tags.iter().any(|candidate| candidate == tag))
    {
        return false;
    }
    if expectation
        .source_ref
        .as_deref()
        .is_some_and(|source_ref| memory.source.ref_ != source_ref)
    {
        return false;
    }
    true
}

fn write_decision_report(
    expectation: &WriteDecisionExpectation,
    scenario: &QualityScenario,
) -> WriteDecisionReport {
    let actual = scenario
        .write_candidates
        .iter()
        .find(|candidate| candidate.id == expectation.memory_id)
        .map(|candidate| classify_write_candidate(candidate, &scenario.evidence_refs))
        .unwrap_or_else(|| "reject".to_string());
    WriteDecisionReport {
        memory_id: expectation.memory_id.clone(),
        expected: expectation.decision.clone(),
        passed: actual == expectation.decision,
        actual,
        reason: expectation.reason.clone(),
    }
}

fn classify_write_candidate(candidate: &MemoryEvent, required_evidence_refs: &[String]) -> String {
    if candidate.sensitivity == "secret" {
        return "reject".to_string();
    }
    if candidate.retention == "ephemeral"
        || candidate.tags.iter().any(|tag| {
            matches!(
                tag.as_str(),
                "transient" | "planning-chatter" | "scratchpad" | "tool-noise"
            )
        })
    {
        return "reject".to_string();
    }
    if candidate.event_type.starts_with("evaluation_") && candidate.source.ref_.trim().is_empty() {
        return "require_evidence".to_string();
    }
    if !required_evidence_refs.is_empty()
        && candidate.event_type.starts_with("evaluation_")
        && !required_evidence_refs
            .iter()
            .any(|required| required == &candidate.source.ref_)
    {
        return "require_evidence".to_string();
    }
    if candidate.ring == "heartwood" && candidate.source.ref_.trim().is_empty() {
        return "require_user_confirmation".to_string();
    }
    "accept".to_string()
}

fn pass_rate(reports: &[RecallExpectationReport]) -> f64 {
    if reports.is_empty() {
        return 1.0;
    }
    reports.iter().filter(|report| report.passed).count() as f64 / reports.len() as f64
}

fn failure_rate(reports: &[RecallExpectationReport]) -> f64 {
    if reports.is_empty() {
        return 0.0;
    }
    reports.iter().filter(|report| !report.passed).count() as f64 / reports.len() as f64
}

fn decision_pass_rate(reports: &[WriteDecisionReport], decision: &str) -> f64 {
    let relevant = reports
        .iter()
        .filter(|report| report.expected == decision)
        .collect::<Vec<_>>();
    if relevant.is_empty() {
        return 1.0;
    }
    relevant.iter().filter(|report| report.passed).count() as f64 / relevant.len() as f64
}
```

- [ ] **Step 4: Export evaluator types**

Modify the `pub use quality::{...};` block in `crates/tree-ring-memory-core/src/lib.rs` so it includes:

```rust
    evaluate_quality_scenario, parse_quality_scenario, summarize_quality_run, QualityRecall,
    QualityRunReport, QualityScenario, QualityScenarioReport, QualityThresholds,
    RecallExpectation, RecallExpectationReport, WriteDecisionExpectation, WriteDecisionReport,
    QUALITY_CATEGORIES, WRITE_DECISIONS,
```

- [ ] **Step 5: Run the focused evaluator tests**

Run:

```bash
cargo test -p tree-ring-memory-core quality:: --locked
```

Expected: PASS.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add crates/tree-ring-memory-core/src/lib.rs crates/tree-ring-memory-core/src/quality.rs
git commit -m "Add memory quality evaluator"
```

---

### Task 3: Add Default Quality Fixture Pack

**Files:**
- Create: `fixtures/quality/no-background-writer-constraint.json`
- Create: `fixtures/quality/transient-planning-spam.json`
- Create: `fixtures/quality/stale-cli-contract.json`
- Create: `fixtures/quality/sensitive-memory-hidden.json`
- Create: `fixtures/quality/superseded-heartwood.json`
- Create: `fixtures/quality/scar-failure-recall.json`

**Interfaces:**
- Consumes: `quality::parse_quality_scenario`
- Produces: six valid quality scenarios for certification.

- [ ] **Step 1: Add the real workflow constraint scenario**

Create `fixtures/quality/no-background-writer-constraint.json`:

```json
{
  "name": "no-background-writer-constraint",
  "category": "constraint_recall",
  "query": "proof loop background writer hidden durable writes",
  "seed_memories": [
    {
      "id": "mem_quality_no_background_writer",
      "created_at": "2026-07-09T00:00:00Z",
      "updated_at": "2026-07-09T00:00:00Z",
      "project": "Tree_Ring_Memory",
      "agent_profile": null,
      "scope": "project",
      "ring": "heartwood",
      "event_type": "decision",
      "summary": "Do not add a background proof runner, hidden recorder, or autonomous durable writer to Tree Ring proof-loop work.",
      "details": "Quality proof must remain explicit, visible, and certification-owned.",
      "source": {
        "type": "evidence",
        "ref": "docs/superpowers/specs/2026-07-09-tree-ring-memory-quality-proof-loop-design.md#non-goals",
        "quote": ""
      },
      "tags": ["proof-loop", "agent-mediated", "no-daemon"],
      "salience": 0.95,
      "confidence": 0.95,
      "sensitivity": "normal",
      "retention": "durable",
      "expires_at": null,
      "supersedes": [],
      "superseded_by": null,
      "links": [],
      "review": {
        "needs_review": false,
        "review_reason": null,
        "reviewed_at": null,
        "reviewed_by": null
      }
    }
  ],
  "expected_recall": [
    {
      "memory_id": "mem_quality_no_background_writer",
      "reason": "proof-loop work must recall the explicit no-background-writer constraint"
    }
  ],
  "thresholds": {
    "min_constraint_recall_rate": 1.0,
    "max_forbidden_recall_rate": 0.0,
    "min_spam_rejection_rate": 1.0,
    "min_evidence_required_rate": 1.0,
    "min_behavior_proof_pass_rate": 1.0
  }
}
```

- [ ] **Step 2: Add the real workflow spam-prevention scenario**

Create `fixtures/quality/transient-planning-spam.json`:

```json
{
  "name": "transient-planning-spam",
  "category": "spam_prevention",
  "query": "memory spam transient planning durable heartwood",
  "seed_memories": [
    {
      "id": "mem_quality_store_only_durable",
      "created_at": "2026-07-09T00:00:00Z",
      "updated_at": "2026-07-09T00:00:00Z",
      "project": "Tree_Ring_Memory",
      "agent_profile": null,
      "scope": "project",
      "ring": "heartwood",
      "event_type": "decision",
      "summary": "Store only durable decisions, validated lessons, reusable warnings, corrections, future seeds, and evidence-backed outcomes.",
      "details": "",
      "source": {
        "type": "evidence",
        "ref": "docs/superpowers/specs/2026-07-09-tree-ring-memory-quality-proof-loop-design.md#agent-guidance-gates",
        "quote": ""
      },
      "tags": ["write-gate", "memory-quality"],
      "salience": 0.9,
      "confidence": 0.9,
      "sensitivity": "normal",
      "retention": "durable",
      "expires_at": null,
      "supersedes": [],
      "superseded_by": null,
      "links": [],
      "review": {
        "needs_review": false,
        "review_reason": null,
        "reviewed_at": null,
        "reviewed_by": null
      }
    }
  ],
  "expected_recall": [
    {
      "memory_id": "mem_quality_store_only_durable",
      "reason": "the write gate should be recalled before evaluating candidate memories"
    }
  ],
  "write_candidates": [
    {
      "id": "mem_candidate_planning_chatter",
      "created_at": "2026-07-09T00:00:00Z",
      "updated_at": "2026-07-09T00:00:00Z",
      "project": "Tree_Ring_Memory",
      "agent_profile": null,
      "scope": "project",
      "ring": "heartwood",
      "event_type": "lesson",
      "summary": "We discussed maybe doing the quality proof loop next.",
      "details": "Transient planning chatter should not become durable heartwood.",
      "source": {
        "type": "manual",
        "ref": "",
        "quote": ""
      },
      "tags": ["planning-chatter", "transient"],
      "salience": 0.2,
      "confidence": 0.4,
      "sensitivity": "normal",
      "retention": "normal",
      "expires_at": null,
      "supersedes": [],
      "superseded_by": null,
      "links": [],
      "review": {
        "needs_review": false,
        "review_reason": null,
        "reviewed_at": null,
        "reviewed_by": null
      }
    }
  ],
  "expected_write_decisions": [
    {
      "memory_id": "mem_candidate_planning_chatter",
      "decision": "reject",
      "reason": "transient planning chatter is memory spam"
    }
  ]
}
```

- [ ] **Step 3: Add the real workflow stale CLI-contract scenario**

Create `fixtures/quality/stale-cli-contract.json`:

```json
{
  "name": "stale-cli-contract",
  "category": "stale_truth_suppression",
  "query": "tree ring helper script cli contract remember recall forget",
  "seed_memories": [
    {
      "id": "mem_stale_cli_contract",
      "created_at": "2026-07-08T00:00:00Z",
      "updated_at": "2026-07-08T00:00:00Z",
      "project": "Tree_Ring_Memory",
      "agent_profile": null,
      "scope": "project",
      "ring": "heartwood",
      "event_type": "decision",
      "summary": "Old helper scripts can call remember recall and forget without required arguments.",
      "details": "This stale CLI contract must not guide current helper-script reviews.",
      "source": {
        "type": "manual",
        "ref": "old-review-note",
        "quote": ""
      },
      "tags": ["cli-contract", "stale"],
      "salience": 0.8,
      "confidence": 0.6,
      "sensitivity": "normal",
      "retention": "durable",
      "expires_at": null,
      "supersedes": [],
      "superseded_by": "mem_current_cli_contract",
      "links": [],
      "review": {
        "needs_review": false,
        "review_reason": null,
        "reviewed_at": null,
        "reviewed_by": null
      }
    },
    {
      "id": "mem_current_cli_contract",
      "created_at": "2026-07-09T00:00:00Z",
      "updated_at": "2026-07-09T00:00:00Z",
      "project": "Tree_Ring_Memory",
      "agent_profile": null,
      "scope": "project",
      "ring": "heartwood",
      "event_type": "warning",
      "summary": "Review Tree Ring helper scripts against the current CLI contract: remember needs --event-type, recall needs a query, and forget needs a memory id.",
      "details": "Current source in crates/tree-ring-memory-cli/src/main.rs remains authoritative.",
      "source": {
        "type": "file",
        "ref": "crates/tree-ring-memory-cli/src/main.rs",
        "quote": ""
      },
      "tags": ["cli-contract", "current"],
      "salience": 0.95,
      "confidence": 0.95,
      "sensitivity": "normal",
      "retention": "durable",
      "expires_at": null,
      "supersedes": ["mem_stale_cli_contract"],
      "superseded_by": null,
      "links": [],
      "review": {
        "needs_review": false,
        "review_reason": null,
        "reviewed_at": null,
        "reviewed_by": null
      }
    }
  ],
  "expected_recall": [
    {
      "memory_id": "mem_current_cli_contract",
      "reason": "current CLI contract should be recalled"
    }
  ],
  "forbidden_recall": [
    {
      "memory_id": "mem_stale_cli_contract",
      "reason": "superseded stale CLI contract must be suppressed"
    }
  ]
}
```

- [ ] **Step 4: Add the sensitive-memory hidden scenario**

Create `fixtures/quality/sensitive-memory-hidden.json`:

```json
{
  "name": "sensitive-memory-hidden",
  "category": "stale_truth_suppression",
  "query": "private diagnosis source metadata",
  "seed_memories": [
    {
      "id": "mem_sensitive_private_diagnosis",
      "created_at": "2026-07-09T00:00:00Z",
      "updated_at": "2026-07-09T00:00:00Z",
      "project": "Tree_Ring_Memory",
      "agent_profile": null,
      "scope": "project",
      "ring": "outer",
      "event_type": "lesson",
      "summary": "Private diagnosis source metadata should stay hidden by default.",
      "details": "",
      "source": {
        "type": "manual",
        "ref": "private-diagnosis-note",
        "quote": ""
      },
      "tags": ["privacy"],
      "salience": 0.7,
      "confidence": 0.8,
      "sensitivity": "private",
      "retention": "normal",
      "expires_at": null,
      "supersedes": [],
      "superseded_by": null,
      "links": [],
      "review": {
        "needs_review": false,
        "review_reason": null,
        "reviewed_at": null,
        "reviewed_by": null
      }
    }
  ],
  "forbidden_recall": [
    {
      "memory_id": "mem_sensitive_private_diagnosis",
      "reason": "private memory is hidden by default"
    }
  ]
}
```

- [ ] **Step 5: Add the superseded-heartwood scenario**

Create `fixtures/quality/superseded-heartwood.json`:

```json
{
  "name": "superseded-heartwood",
  "category": "stale_truth_suppression",
  "query": "durable rule project bridge command",
  "seed_memories": [
    {
      "id": "mem_old_bridge_rule",
      "created_at": "2026-07-08T00:00:00Z",
      "updated_at": "2026-07-08T00:00:00Z",
      "project": "Tree_Ring_Memory",
      "agent_profile": null,
      "scope": "project",
      "ring": "heartwood",
      "event_type": "decision",
      "summary": "Durable rule: project bridge setup should happen automatically during init.",
      "details": "",
      "source": {
        "type": "manual",
        "ref": "old-bridge-note",
        "quote": ""
      },
      "tags": ["bridge", "durable-rule"],
      "salience": 0.9,
      "confidence": 0.8,
      "sensitivity": "normal",
      "retention": "durable",
      "expires_at": null,
      "supersedes": [],
      "superseded_by": "mem_new_bridge_rule",
      "links": [],
      "review": {
        "needs_review": false,
        "review_reason": null,
        "reviewed_at": null,
        "reviewed_by": null
      }
    },
    {
      "id": "mem_new_bridge_rule",
      "created_at": "2026-07-09T00:00:00Z",
      "updated_at": "2026-07-09T00:00:00Z",
      "project": "Tree_Ring_Memory",
      "agent_profile": null,
      "scope": "project",
      "ring": "heartwood",
      "event_type": "decision",
      "summary": "Durable rule: project bridge setup should be an explicit integrations link command, not automatic init behavior.",
      "details": "",
      "source": {
        "type": "evidence",
        "ref": "docs/superpowers/specs/2026-07-06-tree-ring-agent-mediated-bridges-design.md#command-surface",
        "quote": ""
      },
      "tags": ["bridge", "durable-rule"],
      "salience": 0.95,
      "confidence": 0.95,
      "sensitivity": "normal",
      "retention": "durable",
      "expires_at": null,
      "supersedes": ["mem_old_bridge_rule"],
      "superseded_by": null,
      "links": [],
      "review": {
        "needs_review": false,
        "review_reason": null,
        "reviewed_at": null,
        "reviewed_by": null
      }
    }
  ],
  "expected_recall": [
    {
      "memory_id": "mem_new_bridge_rule",
      "reason": "replacement heartwood should be recalled"
    }
  ],
  "forbidden_recall": [
    {
      "memory_id": "mem_old_bridge_rule",
      "reason": "superseded heartwood should not outrank the replacement"
    }
  ]
}
```

- [ ] **Step 6: Add the scar failure recall scenario**

Create `fixtures/quality/scar-failure-recall.json`:

```json
{
  "name": "scar-failure-recall",
  "category": "behavior_proof",
  "query": "failure stale cache rollback",
  "seed_memories": [
    {
      "id": "mem_scar_stale_cache",
      "created_at": "2026-07-09T00:00:00Z",
      "updated_at": "2026-07-09T00:00:00Z",
      "project": "Tree_Ring_Memory",
      "agent_profile": null,
      "scope": "project",
      "ring": "scar",
      "event_type": "warning",
      "summary": "Aggressive caching caused a stale rollback failure; recall this scar before cache-related workflow changes.",
      "details": "",
      "source": {
        "type": "evidence",
        "ref": "evals/cache-branch/run-013",
        "quote": ""
      },
      "tags": ["failure", "cache", "rollback"],
      "salience": 0.95,
      "confidence": 0.9,
      "sensitivity": "normal",
      "retention": "durable",
      "expires_at": null,
      "supersedes": [],
      "superseded_by": null,
      "links": [],
      "review": {
        "needs_review": false,
        "review_reason": null,
        "reviewed_at": null,
        "reviewed_by": null
      }
    }
  ],
  "expected_recall": [
    {
      "memory_id": "mem_scar_stale_cache",
      "ring": "scar",
      "reason": "failure-like queries should surface scar memory"
    }
  ]
}
```

- [ ] **Step 7: Add a fixture parser test**

Append this test inside `crates/tree-ring-memory-core/src/quality.rs` tests:

```rust
    #[test]
    fn parses_default_quality_fixtures() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures")
            .join("quality");
        let mut parsed = 0usize;
        for entry in std::fs::read_dir(&root).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let input = std::fs::read_to_string(&path).unwrap();
            parse_quality_scenario(&input)
                .unwrap_or_else(|err| panic!("{}: {err}", path.display()));
            parsed += 1;
        }

        assert_eq!(parsed, 6);
    }
```

- [ ] **Step 8: Run fixture validation tests**

Run:

```bash
cargo test -p tree-ring-memory-core parses_default_quality_fixtures --locked
```

Expected: PASS.

- [ ] **Step 9: Commit Task 3**

Run:

```bash
git add crates/tree-ring-memory-core/src/quality.rs fixtures/quality
git commit -m "Add default memory quality fixtures"
```

---

### Task 4: Add Certification-Only Quality Runner

**Files:**
- Create: `crates/tree-ring-memory-cli/examples/quality_scenarios.rs`

**Interfaces:**
- Consumes: `tree_ring_memory_core::parse_quality_scenario`
- Consumes: `tree_ring_memory_core::evaluate_quality_scenario`
- Consumes: `tree_ring_memory_core::summarize_quality_run`
- Consumes: `tree_ring_memory_sqlite::{SQLiteMemoryStore, MemoryRetriever}`
- Produces: `quality-report.json`
- Produces: `quality-summary.md`

- [ ] **Step 1: Add the failing example smoke test**

Run:

```bash
cargo run -p tree-ring-memory-cli --example quality_scenarios -- fixtures/quality target/tree-ring-quality-test
```

Expected: FAIL because the `quality_scenarios` example does not exist.

- [ ] **Step 2: Add the quality runner example**

Create `crates/tree-ring-memory-cli/examples/quality_scenarios.rs`:

```rust
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tree_ring_memory_core::{
    evaluate_quality_scenario, parse_quality_scenario, summarize_quality_run, QualityRecall,
    QualityRunReport, QualityScenario, QualityScenarioReport,
};
use tree_ring_memory_sqlite::{MemoryRetriever, SQLiteMemoryStore};

fn main() {
    if let Err(err) = run() {
        eprintln!("quality scenarios failed: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let fixture_dir = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "usage: quality_scenarios <fixture-dir> <output-dir>".to_string())?;
    let output_dir = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| "usage: quality_scenarios <fixture-dir> <output-dir>".to_string())?;
    if args.next().is_some() {
        return Err("usage: quality_scenarios <fixture-dir> <output-dir>".to_string());
    }

    fs::create_dir_all(&output_dir).map_err(|err| err.to_string())?;
    let scenarios = load_scenarios(&fixture_dir)?;
    let mut reports = Vec::new();
    for scenario in scenarios {
        reports.push(run_scenario(&scenario)?);
    }

    let run_report = summarize_quality_run(reports);
    write_reports(&output_dir, &run_report)?;
    if !run_report.quality_pass {
        return Err(format!(
            "quality run failed: {} scenarios evaluated",
            run_report.scenario_count
        ));
    }
    println!(
        "quality scenarios passed: {} scenario(s)",
        run_report.scenario_count
    );
    Ok(())
}

fn load_scenarios(fixture_dir: &Path) -> Result<Vec<QualityScenario>, String> {
    let mut paths = fs::read_dir(fixture_dir)
        .map_err(|err| err.to_string())?
        .map(|entry| entry.map(|entry| entry.path()).map_err(|err| err.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();

    let mut scenarios = Vec::new();
    for path in paths {
        if path.extension() != Some(OsStr::new("json")) {
            continue;
        }
        let input = fs::read_to_string(&path).map_err(|err| err.to_string())?;
        let scenario = parse_quality_scenario(&input)
            .map_err(|err| format!("{}: {err}", path.display()))?;
        scenarios.push(scenario);
    }
    if scenarios.is_empty() {
        return Err(format!("no quality scenario json files in {}", fixture_dir.display()));
    }
    Ok(scenarios)
}

fn run_scenario(scenario: &QualityScenario) -> Result<QualityScenarioReport, String> {
    let root = temporary_root(&scenario.name)?;
    let db_path = root.join("memory.sqlite");
    let mut store = SQLiteMemoryStore::open(&db_path).map_err(|err| err.to_string())?;
    store
        .put_many(&scenario.seed_memories)
        .map_err(|err| err.to_string())?;

    let prompt = scenario
        .prompt()
        .ok_or_else(|| format!("quality scenario {} has no prompt", scenario.name))?;
    let recalls = MemoryRetriever::new(&store)
        .recall(
            prompt,
            None,
            None,
            None,
            None,
            None,
            false,
            false,
            8,
            true,
        )
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(|result| QualityRecall {
            memory: result.memory,
            score: result.score,
        })
        .collect::<Vec<_>>();

    let report = evaluate_quality_scenario(scenario, &recalls).map_err(|err| err.to_string())?;
    fs::remove_dir_all(&root).map_err(|err| err.to_string())?;
    Ok(report)
}

fn temporary_root(name: &str) -> Result<PathBuf, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_millis();
    let safe_name = name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    let root = std::env::temp_dir().join(format!(
        "tree-ring-quality-{}-{}-{safe_name}",
        std::process::id(),
        millis
    ));
    if root.exists() {
        fs::remove_dir_all(&root).map_err(|err| err.to_string())?;
    }
    fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    Ok(root)
}

fn write_reports(output_dir: &Path, report: &QualityRunReport) -> Result<(), String> {
    let json = serde_json::to_string_pretty(report).map_err(|err| err.to_string())?;
    fs::write(output_dir.join("quality-report.json"), json).map_err(|err| err.to_string())?;
    fs::write(output_dir.join("quality-summary.md"), markdown_summary(report))
        .map_err(|err| err.to_string())?;
    Ok(())
}

fn markdown_summary(report: &QualityRunReport) -> String {
    let mut lines = vec![
        "# Tree Ring Memory Quality Summary".to_string(),
        String::new(),
        format!("- quality pass: {}", report.quality_pass),
        format!("- scenarios: {}", report.scenario_count),
        format!(
            "- constraint recall rate: {:.3}",
            report.constraint_recall_rate
        ),
        format!(
            "- forbidden recall rate: {:.3}",
            report.forbidden_recall_rate
        ),
        format!("- spam rejection rate: {:.3}", report.spam_rejection_rate),
        format!(
            "- evidence required rate: {:.3}",
            report.evidence_required_rate
        ),
        format!(
            "- behavior proof pass rate: {:.3}",
            report.behavior_proof_pass_rate
        ),
        String::new(),
        "## Scenarios".to_string(),
        String::new(),
    ];
    for scenario in &report.scenarios {
        lines.push(format!(
            "- `{}` [{}]: pass={}",
            scenario.name, scenario.category, scenario.quality_pass
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}
```

- [ ] **Step 3: Run the quality runner**

Run:

```bash
rm -rf target/tree-ring-quality-test
cargo run -p tree-ring-memory-cli --example quality_scenarios -- fixtures/quality target/tree-ring-quality-test
```

Expected: PASS and output contains:

```text
quality scenarios passed: 6 scenario(s)
```

- [ ] **Step 4: Inspect generated artifacts**

Run:

```bash
test -f target/tree-ring-quality-test/quality-report.json
test -f target/tree-ring-quality-test/quality-summary.md
grep -F '"quality_pass": true' target/tree-ring-quality-test/quality-report.json
grep -F 'Tree Ring Memory Quality Summary' target/tree-ring-quality-test/quality-summary.md
```

Expected: all commands pass.

- [ ] **Step 5: Commit Task 4**

Run:

```bash
git add crates/tree-ring-memory-cli/examples/quality_scenarios.rs
git commit -m "Add memory quality scenario runner"
```

---

### Task 5: Include Quality Metrics In Certification

**Files:**
- Modify: `scripts/certify-tree-ring.sh`

**Interfaces:**
- Consumes: `fixtures/quality/*.json`
- Consumes: `cargo run --release -p tree-ring-memory-cli --example quality_scenarios`
- Produces: `target/tree-ring-certification/quality/quality-report.json`
- Produces: `target/tree-ring-certification/quality/quality-summary.md`
- Produces: top-level `metrics.json` field `quality`

- [ ] **Step 1: Add the failing certification expectation**

Run:

```bash
rm -rf target/tree-ring-certification/quality
sh scripts/certify-tree-ring.sh
test -f target/tree-ring-certification/quality/quality-report.json
```

Expected: FAIL at the `test -f` command because certification does not run quality scenarios yet.

- [ ] **Step 2: Run the quality example from certification**

In `scripts/certify-tree-ring.sh`, add this block after the performance metric extraction block and before Agent Zero handling:

```sh
quality_out="$OUT_DIR/quality"
mkdir -p "$quality_out"
run cargo run --release -p tree-ring-memory-cli --example quality_scenarios -- "$ROOT/fixtures/quality" "$quality_out" \
  > "$OUT_DIR/quality-run.out" 2>&1
require_file "$quality_out/quality-report.json"
require_file "$quality_out/quality-summary.md"
grep -F '"quality_pass": true' "$quality_out/quality-report.json" > /dev/null \
  || fail "memory quality scenarios did not pass"
quality_json=$(cat "$quality_out/quality-report.json")
```

- [ ] **Step 3: Add quality JSON to metrics**

In the `cat > "$METRICS" <<EOF` JSON body, add this field after the `performance` object:

```json
  "quality": $quality_json,
```

The surrounding section should read:

```json
  "performance": {
    "records_10000": $perf_10k_json,
    "records_30000": $perf_30k_json,
    "records_50000": $perf_50k_json
  },
  "quality": $quality_json,
  "agent_zero": {
```

- [ ] **Step 4: Add quality lines to the markdown summary**

In the summary heredoc, add these lines after the 50k extended metrics line:

```md
- memory quality scenarios: passed
- memory quality summary: `quality/quality-summary.md`
```

- [ ] **Step 5: Run certification and inspect quality artifacts**

Run:

```bash
sh scripts/certify-tree-ring.sh
test -f target/tree-ring-certification/quality/quality-report.json
test -f target/tree-ring-certification/quality/quality-summary.md
grep -F '"quality_pass": true' target/tree-ring-certification/metrics.json
grep -F 'memory quality scenarios: passed' target/tree-ring-certification/summary.md
```

Expected: PASS.

- [ ] **Step 6: Commit Task 5**

Run:

```bash
git add scripts/certify-tree-ring.sh
git commit -m "Include memory quality in certification"
```

---

### Task 6: Update Generated Agent Guidance With Quality Gates

**Files:**
- Modify: `crates/tree-ring-memory-cli/src/agent_awareness.rs`
- Modify: `skills/tree-ring-memory/SKILL.md`
- Modify: `docs/integrations/agent-skill.md`
- Modify: `README.md`

**Interfaces:**
- Consumes: quality gates from the approved design.
- Produces: generated `.tree-ring/AGENTS.md` and `.tree-ring/CLI.md` guidance that mentions recall, trust, and write gates.

- [ ] **Step 1: Add failing generated guidance tests**

Append these tests inside `crates/tree-ring-memory-cli/src/agent_awareness.rs` tests:

```rust
    #[test]
    fn generated_agents_file_mentions_quality_gates() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        ensure_agent_awareness(&root).unwrap();
        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();

        assert!(agents.contains("Memory Quality Gates"));
        assert!(agents.contains("Recall gates"));
        assert!(agents.contains("Trust gates"));
        assert!(agents.contains("Write gates"));
        assert!(agents.contains("Reject transient planning chatter"));
    }

    #[test]
    fn generated_cli_reference_mentions_quality_gates() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        ensure_agent_awareness(&root).unwrap();
        let cli = fs::read_to_string(root.join("CLI.md")).unwrap();

        assert!(cli.contains("Memory quality gates"));
        assert!(cli.contains("Before risky work, recall constraints"));
        assert!(cli.contains("Before trusting memory, prefer source-linked"));
        assert!(cli.contains("Before writing memory, reject transient planning chatter"));
    }
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run:

```bash
cargo test -p tree-ring-memory-cli generated_ --locked
```

Expected: FAIL because generated guidance does not mention quality gates yet.

- [ ] **Step 3: Add quality gates to generated CLI reference**

In `crates/tree-ring-memory-cli/src/agent_awareness.rs`, add this block to `CLI_REFERENCE` after the `Adapter rules:` section and before `Safety rules:`:

```md
Memory quality gates:

- Before risky work, recall constraints, scars, user preferences, and unresolved seeds.
- Before trusting memory, prefer source-linked, non-superseded, high-confidence results.
- Re-read source files, tests, explicit user instructions, DOX contracts, or Revolve evidence when memory conflicts with current sources.
- Before writing memory, reject transient planning chatter, duplicate wording, tool noise, and unsupported claims.
- Require evidence refs for promoted or rejected evaluated outcomes.
- Require user confirmation before creating or promoting broad cross-project heartwood.
```

- [ ] **Step 4: Add quality gates to generated AGENTS content**

In the `agent_contract` template in `crates/tree-ring-memory-cli/src/agent_awareness.rs`, add this section after `## Harness Bridges` and before `## DOX Integration`:

```md
## Memory Quality Gates

Recall gates:

- Before substantial project work, recall project constraints, scars, user preferences, and unresolved seeds.
- Before risky changes, recall warnings and evidence-linked prior failures.
- Before repeating a workflow, recall prior errors and accepted procedures.
- Before closeout, recall recent decisions so memory updates do not contradict already-stored lessons.

Trust gates:

- Prefer source-linked, non-superseded, high-confidence memories.
- Re-read source files, tests, explicit user instructions, DOX contracts, or Revolve evidence when memory conflicts with current sources.
- Do not treat sensitive or hidden-by-default memory as ordinary recall context.

Write gates:

- Remember only durable decisions, validated lessons, reusable warnings, corrections, future seeds, and evidence-backed outcomes.
- Reject transient planning chatter, duplicate wording, tool noise, and unsupported claims.
- Require evidence refs for promoted or rejected evaluated outcomes.
- Require user confirmation before creating or promoting broad cross-project heartwood.
```

- [ ] **Step 5: Add quality gates to portable skill docs**

In `skills/tree-ring-memory/SKILL.md`, add this section after the `When To Remember` section and before `Ring Selection`:

```md
## Memory Quality Gates

Use these gates before relying on or writing memory.

Recall gates:

- Before substantial project work, recall project constraints, scars, user preferences, and unresolved seeds.
- Before risky changes, recall warnings and evidence-linked prior failures.
- Before repeating a workflow, recall prior errors and accepted procedures.
- Before closeout, recall recent decisions so memory updates do not contradict already-stored lessons.

Trust gates:

- Prefer source-linked, non-superseded, high-confidence memories.
- Treat heartwood as durable only when source evidence or user confirmation supports it.
- Re-read source files, tests, explicit user instructions, DOX contracts, or Revolve evidence when memory conflicts with current sources.
- Do not treat sensitive or hidden-by-default memory as ordinary recall context.

Write gates:

- Remember only durable decisions, validated lessons, reusable warnings, corrections, future seeds, and evidence-backed outcomes.
- Reject transient planning chatter, duplicate wording, tool noise, and unsupported claims.
- Require evidence refs for promoted or rejected evaluated outcomes.
- Require user confirmation before creating or promoting broad cross-project heartwood.
```

- [ ] **Step 6: Update integration docs and README**

In `docs/integrations/agent-skill.md`, add this section after `Evidence-Driven Improvement`:

```md
## Memory Quality Gates

Tree Ring guidance is meant to improve agent behavior, not increase memory volume.
Use these gates when wiring Tree Ring into an agent harness:

- Recall gates: before substantial or risky work, recall constraints, scars, preferences, and unresolved seeds.
- Trust gates: prefer source-linked, non-superseded, high-confidence memories and re-read authoritative sources when memory conflicts with source files or user instructions.
- Write gates: reject transient planning chatter, duplicate wording, tool noise, and unsupported claims; require evidence refs for promoted or rejected evaluated outcomes.

The certification suite includes quality scenarios that exercise missed constraints, memory spam, stale truth suppression, and behavior proof.
```

In `README.md`, add this paragraph after the certification summary bullet list:

```md
Certification also runs the default memory quality scenario pack under
`fixtures/quality/`. Those scenarios prove recall gates, spam rejection,
stale-truth suppression, evidence requirements, and behavior-proof outcomes.
The quality report is written to
`target/tree-ring-certification/quality/quality-report.json` with a readable
summary at `target/tree-ring-certification/quality/quality-summary.md`.
```

- [ ] **Step 7: Run focused guidance tests**

Run:

```bash
cargo test -p tree-ring-memory-cli generated_ --locked
```

Expected: PASS.

- [ ] **Step 8: Run docs text checks**

Run:

```bash
grep -F 'Memory Quality Gates' skills/tree-ring-memory/SKILL.md
grep -F 'memory quality scenario pack' README.md
grep -F 'Memory Quality Gates' docs/integrations/agent-skill.md
```

Expected: all commands pass.

- [ ] **Step 9: Commit Task 6**

Run:

```bash
git add crates/tree-ring-memory-cli/src/agent_awareness.rs skills/tree-ring-memory/SKILL.md docs/integrations/agent-skill.md README.md
git commit -m "Add memory quality guidance gates"
```

---

### Task 7: Add First-Pass Ambient Ring Fullness

**Files:**
- Modify: `crates/tree-ring-memory-cli/src/tui/model.rs`
- Modify: `crates/tree-ring-memory-cli/src/tui/rings.rs`

**Interfaces:**
- Produces: `RingStats.fullness_level: f64`
- Consumes: `DashboardStats.total`
- Consumes: existing `RingStats.pulse_level`
- Consumes: existing `RingStats.warning_level`

- [ ] **Step 1: Add failing model fullness test**

Append this test inside `crates/tree-ring-memory-cli/src/tui/model.rs` tests:

```rust
    #[test]
    fn derives_relative_ring_fullness() {
        let dashboard = DashboardStats::from_memories(
            &[
                event("Fresh one", "cambium", "lesson"),
                event("Fresh two", "cambium", "lesson"),
                event("Durable", "heartwood", "decision"),
            ],
            None,
        );

        assert_eq!(dashboard.ring("cambium").unwrap().fullness_level, 2.0 / 3.0);
        assert_eq!(dashboard.ring("heartwood").unwrap().fullness_level, 1.0 / 3.0);
        assert_eq!(dashboard.ring("outer").unwrap().fullness_level, 0.0);
    }
```

- [ ] **Step 2: Run focused model test and verify missing field**

Run:

```bash
cargo test -p tree-ring-memory-cli derives_relative_ring_fullness --locked
```

Expected: FAIL because `RingStats` has no `fullness_level` field.

- [ ] **Step 3: Add fullness to `RingStats`**

Modify `crates/tree-ring-memory-cli/src/tui/model.rs`.

Add this field to `RingStats`:

```rust
    pub fullness_level: f64,
```

Set it in `RingStats::empty`:

```rust
            fullness_level: 0.0,
```

In `DashboardStats::from_memories`, after average salience/confidence are calculated and before `stats.warning_level = ring_warning_level(stats);`, add:

```rust
            stats.fullness_level = if dashboard.total == 0 {
                0.0
            } else {
                stats.total as f64 / dashboard.total as f64
            };
```

- [ ] **Step 4: Run focused model test**

Run:

```bash
cargo test -p tree-ring-memory-cli derives_relative_ring_fullness --locked
```

Expected: PASS.

- [ ] **Step 5: Add failing ambient style test**

Append this test inside `crates/tree-ring-memory-cli/src/tui/rings.rs` tests:

```rust
    fn brightness_sum(color: Color) -> u16 {
        match color {
            Color::Rgb(red, green, blue) => red as u16 + green as u16 + blue as u16,
            _ => 0,
        }
    }

    fn stats_with_fullness(ring: &str, total: usize, fullness_level: f64) -> RingStats {
        let mut stats = RingStats::empty(ring);
        stats.total = total;
        stats.fullness_level = fullness_level;
        stats
    }

    #[test]
    fn fullness_brightens_ambient_ring_without_pulse() {
        let low = stats_with_fullness("cambium", 1, 0.10);
        let high = stats_with_fullness("cambium", 8, 0.80);

        let low_color = animated_color("cambium", Some(&low), 10, 0);
        let high_color = animated_color("cambium", Some(&high), 10, 0);

        assert!(brightness_sum(high_color) > brightness_sum(low_color));
    }
```

- [ ] **Step 6: Run focused ambient style test and verify it fails**

Run:

```bash
cargo test -p tree-ring-memory-cli fullness_brightens_ambient_ring_without_pulse --locked
```

Expected: FAIL because fullness does not affect `animated_color`.

- [ ] **Step 7: Map fullness to ambient intensity**

Modify `animated_color` in `crates/tree-ring-memory-cli/src/tui/rings.rs` to this full function:

```rust
fn animated_color(ring: &str, stats: Option<&RingStats>, tick: u64, offset: u64) -> Color {
    let warning_level = stats.map(|stats| stats.warning_level).unwrap_or_default();
    let base = theme::ring_color(ring, warning_level);
    let Some(stats) = stats else {
        return dim_color(base, 0.36);
    };
    if stats.total == 0 {
        return dim_color(base, 0.34);
    }
    let fullness = stats.fullness_level.clamp(0.0, 1.0);
    if stats.pulse_level > 0.05 && (tick + offset) % 6 < 3 {
        return brighten_color(base, 0.28 + fullness * 0.16 + stats.pulse_level * 0.18);
    }
    if (tick + offset) % 18 < 3 {
        return brighten_color(base, 0.08 + fullness * 0.16);
    }
    if fullness < 0.18 {
        return dim_color(base, 0.42 + fullness * 1.35);
    }
    brighten_color(base, fullness * 0.20)
}
```

- [ ] **Step 8: Run TUI focused tests**

Run:

```bash
cargo test -p tree-ring-memory-cli fullness --locked
```

Expected: PASS.

- [ ] **Step 9: Commit Task 7**

Run:

```bash
git add crates/tree-ring-memory-cli/src/tui/model.rs crates/tree-ring-memory-cli/src/tui/rings.rs
git commit -m "Add ambient ring fullness styling"
```

---

### Task 8: Final Verification And Documentation Consistency

**Files:**
- Verify all changed files from Tasks 1-7.

**Interfaces:**
- Consumes: complete quality proof loop implementation.
- Produces: final verified branch state.

- [ ] **Step 1: Run formatting**

Run:

```bash
cargo fmt --check
```

Expected: PASS.

- [ ] **Step 2: Run locked tests**

Run:

```bash
cargo test --locked
```

Expected: PASS.

- [ ] **Step 3: Run Clippy**

Run:

```bash
cargo clippy --locked --all-targets
```

Expected: PASS with no warnings.

- [ ] **Step 4: Run certification**

Run:

```bash
sh scripts/certify-tree-ring.sh
```

Expected: PASS and output includes:

```text
certification passed
```

- [ ] **Step 5: Inspect certification quality artifacts**

Run:

```bash
test -f target/tree-ring-certification/quality/quality-report.json
test -f target/tree-ring-certification/quality/quality-summary.md
grep -F '"quality_pass": true' target/tree-ring-certification/quality/quality-report.json
grep -F '"quality"' target/tree-ring-certification/metrics.json
grep -F 'memory quality scenarios: passed' target/tree-ring-certification/summary.md
```

Expected: PASS.

- [ ] **Step 6: Run whitespace check**

Run:

```bash
git diff --check
```

Expected: PASS with no output.

- [ ] **Step 7: Review final changed files**

Run:

```bash
git status --short
git log --oneline -8
```

Expected: only expected tracked changes are present; pre-existing untracked local files may still appear and must not be staged.

- [ ] **Step 8: Commit final verification note only if files changed during verification**

If formatting or documentation consistency changed files, run:

```bash
git add <changed-files-from-this-verification-step>
git commit -m "Verify memory quality proof loop"
```

Expected: create a commit only when verification produced tracked file changes.

---

## Final-Review Hardening Addendum

This addendum supersedes the historical six-fixture exact count and related
non-null default assumptions in the task evidence above. The final default pack
contains seven fixtures, including `evidence-preservation.json`. Fixture-facing
types reject unknown fields; behavior proof requires an explicit recalled-memory
and decision-change expectation; scenario and run metrics are nullable and
aggregate applicable raw observations; and runner errors preserve completed
scenarios in JSON and markdown with one privacy-safe structured terminal error.

The final default run must contain seven scenarios, zero errors, numeric values
for all five aggregate metrics, and top-level `"quality_pass": true`. The
certification shell's dependency-free exact line gate remains unchanged.
