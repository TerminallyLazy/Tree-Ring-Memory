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
        if let Some(query) = self.query.as_deref() {
            if !query.trim().is_empty() {
                return Some(query);
            }
        }
        self.workflow_prompt
            .as_deref()
            .filter(|value| !value.trim().is_empty())
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
        for (index, evidence_ref) in self.evidence_refs.iter().enumerate() {
            if evidence_ref.trim().is_empty() {
                return Err(TreeRingError::Validation(format!(
                    "quality scenario {} evidence_refs[{}] must not be blank",
                    self.name, index
                )));
            }
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
        let has_nonblank_selector = [
            self.memory_id.as_deref(),
            self.ring.as_deref(),
            self.tag.as_deref(),
            self.source_ref.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|value| !value.trim().is_empty());

        if !has_nonblank_selector {
            return Err(TreeRingError::Validation(format!(
                "quality scenario {scenario_name} {field} entry must include a nonblank memory_id, ring, tag, or source_ref"
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn rejects_blank_evidence_refs() {
        let input = r#"{
          "name": "bad evidence refs",
          "category": "behavior_proof",
          "query": "prove behavior",
          "evidence_refs": ["docs/spec.md", "   "]
        }"#;

        let error = parse_quality_scenario(input).unwrap_err().to_string();

        assert!(error.contains("evidence_refs"));
        assert!(error.contains("must not be blank"));
    }

    #[test]
    fn rejects_blank_recall_selectors() {
        let input = r#"{
          "name": "blank recall selectors",
          "category": "constraint_recall",
          "query": "proof loop",
          "expected_recall": [
            {"memory_id": "   ", "ring": "", "tag": " ", "source_ref": "\t"}
          ]
        }"#;

        let error = parse_quality_scenario(input).unwrap_err().to_string();

        assert!(error.contains("expected_recall"));
        assert!(error.contains("must include a nonblank"));
    }

    #[test]
    fn uses_workflow_prompt_when_query_is_blank() {
        let input = r#"{
          "name": "workflow prompt fallback",
          "category": "behavior_proof",
          "query": "   ",
          "workflow_prompt": "  validate behavior proof  "
        }"#;

        let scenario = parse_quality_scenario(input).unwrap();

        assert_eq!(scenario.prompt(), Some("  validate behavior proof  "));
    }

    #[test]
    fn evaluates_required_and_forbidden_recall() {
        let scenario = QualityScenario {
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
}
