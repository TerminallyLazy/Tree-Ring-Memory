use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::models::{MemoryEvent, TreeRingError, TreeRingResult};

pub const QUALITY_CATEGORIES: &[&str] = &[
    "constraint_recall",
    "spam_prevention",
    "stale_truth_suppression",
    "behavior_proof",
    "evidence_preservation",
];

pub const WRITE_DECISIONS: &[&str] = &[
    "accept",
    "reject",
    "require_evidence",
    "require_user_confirmation",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    pub behavior_expectation: Option<BehaviorExpectation>,
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
        for memory in self
            .seed_memories
            .iter()
            .chain(self.write_candidates.iter())
        {
            memory.validate()?;
        }
        for expectation in &self.expected_recall {
            expectation.validate("expected_recall", &self.name)?;
        }
        for (index, expectation) in self.forbidden_recall.iter().enumerate() {
            expectation.validate("forbidden_recall", &self.name)?;
            if matching_seed_memory_ids(expectation, &self.seed_memories).is_empty() {
                return Err(TreeRingError::Validation(format!(
                    "quality scenario {} forbidden_recall[{}] does not match a seed_memory",
                    self.name, index
                )));
            }
        }
        for decision in &self.expected_write_decisions {
            decision.validate(&self.name)?;
        }
        validate_write_decision_coverage(self)?;
        for (index, evidence_ref) in self.evidence_refs.iter().enumerate() {
            if evidence_ref.trim().is_empty() {
                return Err(TreeRingError::Validation(format!(
                    "quality scenario {} evidence_refs[{}] must not be blank",
                    self.name, index
                )));
            }
        }
        if self.category == "behavior_proof" && self.behavior_expectation.is_none() {
            return Err(TreeRingError::Validation(format!(
                "quality scenario {} category behavior_proof requires behavior_expectation",
                self.name
            )));
        }
        if let Some(expectation) = &self.behavior_expectation {
            expectation.validate(self)?;
        }
        self.validate_primary_observation_contract()?;
        self.thresholds
            .validate(&self.name, self.metric_applicability())?;
        Ok(())
    }

    fn validate_primary_observation_contract(&self) -> TreeRingResult<()> {
        match self.category.as_str() {
            "constraint_recall" if self.expected_recall.is_empty() => {
                Err(TreeRingError::Validation(format!(
                    "quality scenario {} category constraint_recall requires at least one expected_recall",
                    self.name
                )))
            }
            "spam_prevention"
                if !self
                    .expected_write_decisions
                    .iter()
                    .any(|decision| decision.decision == "reject") =>
            {
                Err(TreeRingError::Validation(format!(
                    "quality scenario {} category spam_prevention requires at least one expected_write_decision of reject",
                    self.name
                )))
            }
            "stale_truth_suppression" if self.forbidden_recall.is_empty() => {
                Err(TreeRingError::Validation(format!(
                    "quality scenario {} category stale_truth_suppression requires at least one forbidden_recall",
                    self.name
                )))
            }
            "evidence_preservation" if !self.has_evaluation_write_candidate() => {
                Err(TreeRingError::Validation(format!(
                    "quality scenario {} category evidence_preservation requires at least one evaluation write_candidate",
                    self.name
                )))
            }
            _ => Ok(()),
        }
    }

    fn metric_applicability(&self) -> MetricApplicability {
        MetricApplicability {
            constraint_recall: !self.expected_recall.is_empty(),
            forbidden_recall: !self.forbidden_recall.is_empty(),
            spam_rejection: self
                .expected_write_decisions
                .iter()
                .any(|decision| decision.decision == "reject"),
            evidence_required: self.has_evaluation_write_candidate(),
            behavior_proof: self.behavior_expectation.is_some(),
        }
    }

    fn has_evaluation_write_candidate(&self) -> bool {
        self.write_candidates
            .iter()
            .any(|candidate| candidate.event_type.starts_with("evaluation_"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MetricApplicability {
    constraint_recall: bool,
    forbidden_recall: bool,
    spam_rejection: bool,
    evidence_required: bool,
    behavior_proof: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BehaviorExpectation {
    pub required_memory_id: String,
    pub baseline_decision: String,
    pub memory_informed_decision: String,
    pub expected_decision: String,
    #[serde(default)]
    pub reason: Option<String>,
}

impl BehaviorExpectation {
    fn validate(&self, scenario: &QualityScenario) -> TreeRingResult<()> {
        for (field, value) in [
            ("required_memory_id", self.required_memory_id.as_str()),
            ("baseline_decision", self.baseline_decision.as_str()),
            (
                "memory_informed_decision",
                self.memory_informed_decision.as_str(),
            ),
            ("expected_decision", self.expected_decision.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(TreeRingError::Validation(format!(
                    "quality scenario {} behavior_expectation {field} must not be blank",
                    scenario.name
                )));
            }
        }
        if self
            .reason
            .as_deref()
            .is_some_and(|reason| reason.trim().is_empty())
        {
            return Err(TreeRingError::Validation(format!(
                "quality scenario {} behavior_expectation reason must not be blank when provided",
                scenario.name
            )));
        }
        if !scenario
            .seed_memories
            .iter()
            .any(|memory| memory.id == self.required_memory_id)
        {
            return Err(TreeRingError::Validation(format!(
                "quality scenario {} behavior_expectation required_memory_id {} does not match a seed_memory",
                scenario.name, self.required_memory_id
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualityThresholds {
    #[serde(default)]
    pub min_constraint_recall_rate: Option<f64>,
    #[serde(default)]
    pub max_forbidden_recall_rate: Option<f64>,
    #[serde(default)]
    pub min_spam_rejection_rate: Option<f64>,
    #[serde(default)]
    pub min_evidence_required_rate: Option<f64>,
    #[serde(default)]
    pub min_behavior_proof_pass_rate: Option<f64>,
}

impl QualityThresholds {
    fn validate(
        &self,
        scenario_name: &str,
        applicability: MetricApplicability,
    ) -> TreeRingResult<()> {
        for (field, value) in [
            (
                "min_constraint_recall_rate",
                self.min_constraint_recall_rate,
            ),
            ("max_forbidden_recall_rate", self.max_forbidden_recall_rate),
            ("min_spam_rejection_rate", self.min_spam_rejection_rate),
            (
                "min_evidence_required_rate",
                self.min_evidence_required_rate,
            ),
            (
                "min_behavior_proof_pass_rate",
                self.min_behavior_proof_pass_rate,
            ),
        ] {
            if value.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value)) {
                return Err(TreeRingError::Validation(format!(
                    "quality scenario {scenario_name} threshold {field} must be between 0 and 1"
                )));
            }
        }
        for (field, configured, applicable, required_observation) in [
            (
                "min_constraint_recall_rate",
                self.min_constraint_recall_rate.is_some(),
                applicability.constraint_recall,
                "expected_recall observations",
            ),
            (
                "max_forbidden_recall_rate",
                self.max_forbidden_recall_rate.is_some(),
                applicability.forbidden_recall,
                "forbidden_recall observations",
            ),
            (
                "min_spam_rejection_rate",
                self.min_spam_rejection_rate.is_some(),
                applicability.spam_rejection,
                "expected_write_decision reject observations",
            ),
            (
                "min_evidence_required_rate",
                self.min_evidence_required_rate.is_some(),
                applicability.evidence_required,
                "evaluation write_candidate observations",
            ),
            (
                "min_behavior_proof_pass_rate",
                self.min_behavior_proof_pass_rate.is_some(),
                applicability.behavior_proof,
                "behavior_expectation observations",
            ),
        ] {
            if configured && !applicable {
                return Err(TreeRingError::Validation(format!(
                    "quality scenario {scenario_name} threshold {field} is inapplicable without {required_observation}"
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
    #[serde(default)]
    pub evidence_applicable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BehaviorExpectationReport {
    pub expectation: BehaviorExpectation,
    pub required_memory_recalled: bool,
    pub decision_changed: bool,
    pub expected_decision_reached: bool,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityScenarioReport {
    pub name: String,
    pub category: String,
    pub constraint_recall_rate: Option<f64>,
    pub forbidden_recall_rate: Option<f64>,
    pub spam_rejection_rate: Option<f64>,
    pub evidence_required_rate: Option<f64>,
    pub behavior_proof_pass: Option<bool>,
    #[serde(default)]
    pub behavior_expectation: Option<BehaviorExpectationReport>,
    pub quality_pass: bool,
    pub expected_recall: Vec<RecallExpectationReport>,
    pub forbidden_recall: Vec<RecallExpectationReport>,
    pub write_decisions: Vec<WriteDecisionReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRunError {
    #[serde(default)]
    pub scenario: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    pub stage: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityRunReport {
    pub ok: bool,
    pub scenario_count: usize,
    pub constraint_recall_rate: Option<f64>,
    pub forbidden_recall_rate: Option<f64>,
    pub spam_rejection_rate: Option<f64>,
    pub evidence_required_rate: Option<f64>,
    pub behavior_proof_pass_rate: Option<f64>,
    pub quality_pass: bool,
    pub scenarios: Vec<QualityScenarioReport>,
    #[serde(default)]
    pub errors: Vec<QualityRunError>,
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
        .collect::<TreeRingResult<Vec<_>>>()?;

    let constraint_recall_rate = pass_rate(&expected_recall);
    let forbidden_recall_rate = failure_rate(&forbidden_recall);
    let spam_rejection_rate =
        decision_pass_rate(&write_decisions, |report| report.expected == "reject");
    let evidence_required_rate =
        decision_pass_rate(&write_decisions, |report| report.evidence_applicable);
    let behavior_expectation = scenario
        .behavior_expectation
        .as_ref()
        .map(|expectation| behavior_expectation_report(expectation, recalls));
    let behavior_proof_pass = behavior_expectation.as_ref().map(|report| report.passed);
    let behavior_rate = behavior_proof_pass.map(|passed| if passed { 1.0 } else { 0.0 });
    let thresholds = scenario
        .metric_applicability()
        .thresholds(&scenario.thresholds);
    let quality_pass =
        minimum_met(
            constraint_recall_rate,
            thresholds.min_constraint_recall_rate,
        ) && maximum_met(forbidden_recall_rate, thresholds.max_forbidden_recall_rate)
            && write_decisions.iter().all(|report| report.passed)
            && minimum_met(spam_rejection_rate, thresholds.min_spam_rejection_rate)
            && minimum_met(
                evidence_required_rate,
                thresholds.min_evidence_required_rate,
            )
            && minimum_met(behavior_rate, thresholds.min_behavior_proof_pass_rate);

    Ok(QualityScenarioReport {
        name: scenario.name.clone(),
        category: scenario.category.clone(),
        constraint_recall_rate,
        forbidden_recall_rate,
        spam_rejection_rate,
        evidence_required_rate,
        behavior_proof_pass,
        behavior_expectation,
        quality_pass,
        expected_recall,
        forbidden_recall,
        write_decisions,
    })
}

pub fn summarize_quality_run(reports: Vec<QualityScenarioReport>) -> QualityRunReport {
    let scenario_count = reports.len();
    let constraint_recall_rate = aggregate_rate(
        reports
            .iter()
            .flat_map(|report| report.expected_recall.iter().map(|item| item.passed)),
    );
    let forbidden_recall_rate = aggregate_rate(
        reports
            .iter()
            .flat_map(|report| report.forbidden_recall.iter().map(|item| !item.passed)),
    );
    let spam_rejection_rate = aggregate_rate(reports.iter().flat_map(|report| {
        report
            .write_decisions
            .iter()
            .filter(|item| item.expected == "reject")
            .map(|item| item.passed)
    }));
    let evidence_required_rate = aggregate_rate(reports.iter().flat_map(|report| {
        report
            .write_decisions
            .iter()
            .filter(|item| item.evidence_applicable)
            .map(|item| item.passed)
    }));
    let behavior_proof_pass_rate = aggregate_rate(
        reports
            .iter()
            .filter_map(|report| report.behavior_proof_pass),
    );
    let quality_pass = scenario_count > 0 && reports.iter().all(|report| report.quality_pass);

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
        errors: Vec::new(),
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

fn validate_write_decision_coverage(scenario: &QualityScenario) -> TreeRingResult<()> {
    let mut candidate_ids = HashSet::new();
    for (index, candidate) in scenario.write_candidates.iter().enumerate() {
        if !candidate_ids.insert(candidate.id.as_str()) {
            return Err(TreeRingError::Validation(format!(
                "quality scenario {} write_candidates[{index}] duplicates memory_id {}",
                scenario.name, candidate.id
            )));
        }
    }

    let candidate_index_by_id = scenario
        .write_candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| (candidate.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut decision_index_by_id = HashMap::new();

    for (index, decision) in scenario.expected_write_decisions.iter().enumerate() {
        if let Some(previous_index) =
            decision_index_by_id.insert(decision.memory_id.as_str(), index)
        {
            return Err(TreeRingError::Validation(format!(
                "quality scenario {} expected_write_decisions[{index}] duplicates memory_id {} from expected_write_decisions[{previous_index}]",
                scenario.name, decision.memory_id
            )));
        }
        if !candidate_index_by_id.contains_key(decision.memory_id.as_str()) {
            return Err(TreeRingError::Validation(format!(
                "quality scenario {} expected_write_decisions[{index}] memory_id {} does not match any write_candidate",
                scenario.name, decision.memory_id
            )));
        }
    }

    for (index, candidate) in scenario.write_candidates.iter().enumerate() {
        if !decision_index_by_id.contains_key(candidate.id.as_str()) {
            return Err(TreeRingError::Validation(format!(
                "quality scenario {} write_candidates[{index}] id {} is missing expected_write_decision coverage",
                scenario.name, candidate.id
            )));
        }
    }

    Ok(())
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

fn behavior_expectation_report(
    expectation: &BehaviorExpectation,
    recalls: &[QualityRecall],
) -> BehaviorExpectationReport {
    let required_memory_recalled = recalls
        .iter()
        .any(|recall| recall.memory.id == expectation.required_memory_id);
    let decision_changed = expectation.baseline_decision != expectation.memory_informed_decision;
    let expected_decision_reached =
        expectation.memory_informed_decision == expectation.expected_decision;
    BehaviorExpectationReport {
        expectation: expectation.clone(),
        required_memory_recalled,
        decision_changed,
        expected_decision_reached,
        passed: required_memory_recalled && decision_changed && expected_decision_reached,
    }
}

fn matching_recall_ids(expectation: &RecallExpectation, recalls: &[QualityRecall]) -> Vec<String> {
    recalls
        .iter()
        .filter(|recall| expectation_matches_memory(expectation, &recall.memory))
        .map(|recall| recall.memory.id.clone())
        .collect()
}

fn matching_seed_memory_ids(
    expectation: &RecallExpectation,
    seed_memories: &[MemoryEvent],
) -> Vec<String> {
    seed_memories
        .iter()
        .filter(|memory| expectation_matches_memory(expectation, memory))
        .map(|memory| memory.id.clone())
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
) -> TreeRingResult<WriteDecisionReport> {
    let candidate = scenario
        .write_candidates
        .iter()
        .find(|candidate| candidate.id == expectation.memory_id)
        .ok_or_else(|| {
            TreeRingError::Validation(format!(
                "quality scenario {} expected write decision for memory_id {} could not resolve a write_candidate",
                scenario.name, expectation.memory_id
            ))
        })?;
    let actual = classify_write_candidate(candidate, &scenario.evidence_refs);
    Ok(WriteDecisionReport {
        memory_id: expectation.memory_id.clone(),
        expected: expectation.decision.clone(),
        passed: actual == expectation.decision,
        actual,
        reason: expectation.reason.clone(),
        evidence_applicable: candidate.event_type.starts_with("evaluation_"),
    })
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

fn pass_rate(reports: &[RecallExpectationReport]) -> Option<f64> {
    aggregate_rate(reports.iter().map(|report| report.passed))
}

fn failure_rate(reports: &[RecallExpectationReport]) -> Option<f64> {
    aggregate_rate(reports.iter().map(|report| !report.passed))
}

fn decision_pass_rate(
    reports: &[WriteDecisionReport],
    applicable: impl Fn(&WriteDecisionReport) -> bool,
) -> Option<f64> {
    aggregate_rate(
        reports
            .iter()
            .filter(|report| applicable(report))
            .map(|report| report.passed),
    )
}

fn aggregate_rate(observations: impl Iterator<Item = bool>) -> Option<f64> {
    let (passed, total) = observations.fold((0usize, 0usize), |(passed, total), observation| {
        (passed + usize::from(observation), total + 1)
    });
    (total > 0).then_some(passed as f64 / total as f64)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct EffectiveThresholds {
    min_constraint_recall_rate: Option<f64>,
    max_forbidden_recall_rate: Option<f64>,
    min_spam_rejection_rate: Option<f64>,
    min_evidence_required_rate: Option<f64>,
    min_behavior_proof_pass_rate: Option<f64>,
}

impl MetricApplicability {
    fn thresholds(&self, configured: &QualityThresholds) -> EffectiveThresholds {
        EffectiveThresholds {
            min_constraint_recall_rate: configured
                .min_constraint_recall_rate
                .or(self.constraint_recall.then_some(1.0)),
            max_forbidden_recall_rate: configured
                .max_forbidden_recall_rate
                .or(self.forbidden_recall.then_some(0.0)),
            min_spam_rejection_rate: configured
                .min_spam_rejection_rate
                .or(self.spam_rejection.then_some(1.0)),
            min_evidence_required_rate: configured
                .min_evidence_required_rate
                .or(self.evidence_required.then_some(1.0)),
            min_behavior_proof_pass_rate: configured
                .min_behavior_proof_pass_rate
                .or(self.behavior_proof.then_some(1.0)),
        }
    }
}

fn minimum_met(rate: Option<f64>, threshold: Option<f64>) -> bool {
    threshold.is_none_or(|threshold| rate.is_some_and(|rate| rate >= threshold))
}

fn maximum_met(rate: Option<f64>, threshold: Option<f64>) -> bool {
    threshold.is_none_or(|threshold| rate.is_some_and(|rate| rate <= threshold))
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

    fn scenario(name: &str, category: &str) -> QualityScenario {
        QualityScenario {
            name: name.to_string(),
            category: category.to_string(),
            seed_memories: Vec::new(),
            query: Some("quality proof".to_string()),
            workflow_prompt: None,
            expected_recall: Vec::new(),
            forbidden_recall: Vec::new(),
            write_candidates: Vec::new(),
            expected_write_decisions: Vec::new(),
            evidence_refs: Vec::new(),
            behavior_expectation: None,
            thresholds: QualityThresholds::default(),
        }
    }

    fn report_json(scenario: &QualityScenario, recalls: &[QualityRecall]) -> serde_json::Value {
        serde_json::to_value(evaluate_quality_scenario(scenario, recalls).unwrap()).unwrap()
    }

    #[test]
    fn rejects_unknown_top_level_fixture_field() {
        let error = parse_quality_scenario(
            r#"{
              "name": "misspelled recall",
              "category": "constraint_recall",
              "query": "quality proof",
              "expected_recal": []
            }"#,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("unknown field `expected_recal`"), "{error}");
    }

    #[test]
    fn rejects_unknown_threshold_field() {
        let error = parse_quality_scenario(
            r#"{
              "name": "misspelled threshold",
              "category": "constraint_recall",
              "query": "quality proof",
              "thresholds": {"min_constraint_recal_rate": 1.0}
            }"#,
        )
        .unwrap_err()
        .to_string();

        assert!(
            error.contains("unknown field `min_constraint_recal_rate`"),
            "{error}"
        );
    }

    #[test]
    fn rejects_unknown_nested_expectation_field() {
        let error = parse_quality_scenario(
            r#"{
              "name": "misspelled nested field",
              "category": "constraint_recall",
              "query": "quality proof",
              "expected_recall": [{"memoryid": "mem_required"}]
            }"#,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("unknown field `memoryid`"), "{error}");
    }

    #[test]
    fn behavior_proof_requires_explicit_expectation() {
        let error = parse_quality_scenario(
            r#"{
              "name": "implicit behavior proof",
              "category": "behavior_proof",
              "query": "quality proof"
            }"#,
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("requires behavior_expectation"), "{error}");
    }

    #[test]
    fn constraint_recall_requires_expected_recall_observations() {
        let error = parse_quality_scenario(
            r#"{
              "name": "empty constraint recall",
              "category": "constraint_recall",
              "query": "proof loop"
            }"#,
        )
        .unwrap_err()
        .to_string();

        assert!(
            error.contains("requires at least one expected_recall"),
            "{error}"
        );
    }

    #[test]
    fn spam_prevention_requires_reject_observations() {
        let input = serde_json::json!({
            "name": "empty spam prevention",
            "category": "spam_prevention",
            "query": "memory spam",
            "write_candidates": [{
                "id": "mem_non_reject_candidate",
                "created_at": "2026-07-09T00:00:00Z",
                "updated_at": "2026-07-09T00:00:00Z",
                "project": "tree-ring",
                "agent_profile": null,
                "scope": "project",
                "ring": "cambium",
                "event_type": "lesson",
                "summary": "Durable note",
                "details": "",
                "source": {"type": "evidence", "ref": "docs/spec.md", "quote": ""},
                "tags": [],
                "salience": 0.8,
                "confidence": 0.8,
                "sensitivity": "normal",
                "retention": "durable",
                "expires_at": null,
                "supersedes": [],
                "superseded_by": null,
                "links": [],
                "review": {"needs_review": false, "review_reason": null, "reviewed_at": null, "reviewed_by": null}
            }],
            "expected_write_decisions": [{
                "memory_id": "mem_non_reject_candidate",
                "decision": "accept"
            }]
        });

        let error = parse_quality_scenario(&input.to_string())
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("requires at least one expected_write_decision of reject"),
            "{error}"
        );
    }

    #[test]
    fn stale_truth_suppression_requires_forbidden_recall_observations() {
        let error = parse_quality_scenario(
            r#"{
              "name": "empty stale truth suppression",
              "category": "stale_truth_suppression",
              "query": "stale rule"
            }"#,
        )
        .unwrap_err()
        .to_string();

        assert!(
            error.contains("requires at least one forbidden_recall"),
            "{error}"
        );
    }

    #[test]
    fn evidence_preservation_requires_evaluation_write_candidates() {
        let input = serde_json::json!({
            "name": "empty evidence preservation",
            "category": "evidence_preservation",
            "query": "preserve evidence",
            "write_candidates": [{
                "id": "mem_non_evaluation_candidate",
                "created_at": "2026-07-09T00:00:00Z",
                "updated_at": "2026-07-09T00:00:00Z",
                "project": "tree-ring",
                "agent_profile": null,
                "scope": "project",
                "ring": "cambium",
                "event_type": "lesson",
                "summary": "Regular lesson",
                "details": "",
                "source": {"type": "evidence", "ref": "docs/spec.md", "quote": ""},
                "tags": [],
                "salience": 0.8,
                "confidence": 0.8,
                "sensitivity": "normal",
                "retention": "durable",
                "expires_at": null,
                "supersedes": [],
                "superseded_by": null,
                "links": [],
                "review": {"needs_review": false, "review_reason": null, "reviewed_at": null, "reviewed_by": null}
            }],
            "expected_write_decisions": [{
                "memory_id": "mem_non_evaluation_candidate",
                "decision": "accept"
            }]
        });

        let error = parse_quality_scenario(&input.to_string())
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("requires at least one evaluation write_candidate"),
            "{error}"
        );
    }

    #[test]
    fn rejects_explicit_thresholds_for_inapplicable_metrics() {
        let cases = [
            (
                serde_json::json!({
                    "name": "irrelevant constraint threshold",
                    "category": "stale_truth_suppression",
                    "query": "stale rule",
                    "seed_memories": [{
                        "id": "mem_stale",
                        "created_at": "2026-07-09T00:00:00Z",
                        "updated_at": "2026-07-09T00:00:00Z",
                        "project": "tree-ring",
                        "agent_profile": null,
                        "scope": "project",
                        "ring": "heartwood",
                        "event_type": "lesson",
                        "summary": "Stale instruction",
                        "details": "",
                        "source": {"type": "evidence", "ref": "docs/spec.md", "quote": ""},
                        "tags": [],
                        "salience": 0.8,
                        "confidence": 0.8,
                        "sensitivity": "normal",
                        "retention": "durable",
                        "expires_at": null,
                        "supersedes": [],
                        "superseded_by": null,
                        "links": [],
                        "review": {"needs_review": false, "review_reason": null, "reviewed_at": null, "reviewed_by": null}
                    }],
                    "forbidden_recall": [{"memory_id": "mem_stale"}],
                    "thresholds": {"min_constraint_recall_rate": 1.0}
                }),
                "min_constraint_recall_rate",
            ),
            (
                serde_json::json!({
                    "name": "irrelevant forbidden threshold",
                    "category": "constraint_recall",
                    "query": "proof loop",
                    "expected_recall": [{"memory_id": "mem_required"}],
                    "thresholds": {"max_forbidden_recall_rate": 0.0}
                }),
                "max_forbidden_recall_rate",
            ),
            (
                serde_json::json!({
                    "name": "irrelevant spam threshold",
                    "category": "evidence_preservation",
                    "query": "preserve evidence",
                    "write_candidates": [{
                        "id": "mem_eval",
                        "created_at": "2026-07-09T00:00:00Z",
                        "updated_at": "2026-07-09T00:00:00Z",
                        "project": "tree-ring",
                        "agent_profile": null,
                        "scope": "project",
                        "ring": "cambium",
                        "event_type": "evaluation_outcome",
                        "summary": "Evidence-backed evaluation",
                        "details": "",
                        "source": {"type": "evidence", "ref": "evals/run-001", "quote": ""},
                        "tags": ["evaluation"],
                        "salience": 0.8,
                        "confidence": 0.8,
                        "sensitivity": "normal",
                        "retention": "durable",
                        "expires_at": null,
                        "supersedes": [],
                        "superseded_by": null,
                        "links": [],
                        "review": {"needs_review": false, "review_reason": null, "reviewed_at": null, "reviewed_by": null}
                    }],
                    "expected_write_decisions": [{
                        "memory_id": "mem_eval",
                        "decision": "accept"
                    }],
                    "evidence_refs": ["evals/run-001"],
                    "thresholds": {"min_spam_rejection_rate": 1.0}
                }),
                "min_spam_rejection_rate",
            ),
            (
                serde_json::json!({
                    "name": "irrelevant evidence threshold",
                    "category": "spam_prevention",
                    "query": "memory spam",
                    "write_candidates": [{
                        "id": "mem_spam",
                        "created_at": "2026-07-09T00:00:00Z",
                        "updated_at": "2026-07-09T00:00:00Z",
                        "project": "tree-ring",
                        "agent_profile": null,
                        "scope": "project",
                        "ring": "heartwood",
                        "event_type": "lesson",
                        "summary": "Transient planning chatter",
                        "details": "",
                        "source": {"type": "manual", "ref": "", "quote": ""},
                        "tags": ["transient"],
                        "salience": 0.2,
                        "confidence": 0.2,
                        "sensitivity": "normal",
                        "retention": "ephemeral",
                        "expires_at": null,
                        "supersedes": [],
                        "superseded_by": null,
                        "links": [],
                        "review": {"needs_review": false, "review_reason": null, "reviewed_at": null, "reviewed_by": null}
                    }],
                    "expected_write_decisions": [{
                        "memory_id": "mem_spam",
                        "decision": "reject"
                    }],
                    "thresholds": {"min_evidence_required_rate": 1.0}
                }),
                "min_evidence_required_rate",
            ),
            (
                serde_json::json!({
                    "name": "irrelevant behavior threshold",
                    "category": "constraint_recall",
                    "query": "proof loop",
                    "expected_recall": [{"memory_id": "mem_required"}],
                    "thresholds": {"min_behavior_proof_pass_rate": 1.0}
                }),
                "min_behavior_proof_pass_rate",
            ),
        ];

        for (input, field) in cases {
            let error = parse_quality_scenario(&input.to_string())
                .unwrap_err()
                .to_string();
            assert!(error.contains(field), "{field}: {error}");
            assert!(error.contains("inapplicable"), "{field}: {error}");
        }
    }

    #[test]
    fn older_quality_reports_deserialize_without_behavior_expectation() {
        let report = serde_json::from_str::<QualityScenarioReport>(
            r#"{
              "name": "legacy report",
              "category": "constraint_recall",
              "constraint_recall_rate": 1.0,
              "forbidden_recall_rate": null,
              "spam_rejection_rate": null,
              "evidence_required_rate": null,
              "behavior_proof_pass": null,
              "quality_pass": true,
              "expected_recall": [],
              "forbidden_recall": [],
              "write_decisions": []
            }"#,
        )
        .unwrap();

        assert!(report.behavior_expectation.is_none());
    }

    #[test]
    fn behavior_proof_reports_an_observed_decision_change() {
        let required = memory(
            "mem_required_behavior",
            "Rollback instead of retrying the stale cache migration.",
            "scar",
        );
        let input = serde_json::json!({
            "name": "explicit behavior proof",
            "category": "behavior_proof",
            "query": "stale cache migration failed",
            "seed_memories": [required],
            "behavior_expectation": {
                "required_memory_id": "mem_required_behavior",
                "baseline_decision": "retry the migration unchanged",
                "memory_informed_decision": "rollback and inspect cache state",
                "expected_decision": "rollback and inspect cache state",
                "reason": "the scar changes the recovery decision"
            }
        });
        let scenario = parse_quality_scenario(&input.to_string()).unwrap();
        let recalls = [QualityRecall {
            memory: scenario.seed_memories[0].clone(),
            score: 0.9,
        }];

        let report = report_json(&scenario, &recalls);

        assert_eq!(report["behavior_proof_pass"], true);
        assert_eq!(
            report["behavior_expectation"]["required_memory_recalled"],
            true
        );
        assert_eq!(report["behavior_expectation"]["decision_changed"], true);
        assert_eq!(
            report["behavior_expectation"]["expected_decision_reached"],
            true
        );
        assert_eq!(report["behavior_expectation"]["passed"], true);
    }

    #[test]
    fn behavior_threshold_applies_to_an_applicable_failure() {
        let required = memory("mem_behavior_threshold", "Use rollback recovery.", "scar");
        let input = serde_json::json!({
            "name": "behavior threshold",
            "category": "behavior_proof",
            "query": "failed recovery",
            "seed_memories": [required],
            "behavior_expectation": {
                "required_memory_id": "mem_behavior_threshold",
                "baseline_decision": "retry unchanged",
                "memory_informed_decision": "retry unchanged",
                "expected_decision": "rollback"
            },
            "thresholds": {"min_behavior_proof_pass_rate": 0.0}
        });
        let mut scenario = parse_quality_scenario(&input.to_string()).unwrap();
        let recalls = [QualityRecall {
            memory: scenario.seed_memories[0].clone(),
            score: 0.9,
        }];

        assert_eq!(report_json(&scenario, &recalls)["quality_pass"], true);
        scenario.thresholds.min_behavior_proof_pass_rate = Some(0.5);
        assert_eq!(report_json(&scenario, &recalls)["quality_pass"], false);
    }

    #[test]
    fn inapplicable_metrics_are_null_when_a_primary_observation_exists() {
        let mut scenario = scenario("constraint only", "constraint_recall");
        scenario.expected_recall = vec![RecallExpectation {
            memory_id: Some("mem_missing".to_string()),
            ..Default::default()
        }];
        let report = report_json(&scenario, &[]);

        assert_eq!(report["constraint_recall_rate"], 0.0);
        assert_eq!(report["quality_pass"], false);
        assert!(report["forbidden_recall_rate"].is_null());
        assert!(report["spam_rejection_rate"].is_null());
        assert!(report["evidence_required_rate"].is_null());
        assert!(report["behavior_proof_pass"].is_null());
    }

    #[test]
    fn run_metrics_weight_only_applicable_expectations() {
        let mut failing = scenario("missed constraint", "constraint_recall");
        failing.expected_recall = vec![RecallExpectation {
            memory_id: Some("mem_missing".to_string()),
            ..Default::default()
        }];
        let mut unrelated = scenario("unrelated", "evidence_preservation");
        let mut candidate = memory("mem_eval", "Evidence-backed evaluation.", "cambium");
        candidate.event_type = "evaluation_outcome".to_string();
        candidate.source.ref_ = "evals/run-001".to_string();
        unrelated.write_candidates = vec![candidate];
        unrelated.expected_write_decisions = vec![WriteDecisionExpectation {
            memory_id: "mem_eval".to_string(),
            decision: "accept".to_string(),
            reason: "valid unrelated evidence scenario".to_string(),
        }];
        unrelated.evidence_refs = vec!["evals/run-001".to_string()];
        let reports = vec![
            evaluate_quality_scenario(&failing, &[]).unwrap(),
            evaluate_quality_scenario(&unrelated, &[]).unwrap(),
        ];

        let run = serde_json::to_value(summarize_quality_run(reports)).unwrap();

        assert_eq!(run["constraint_recall_rate"], 0.0);
        assert!(run["spam_rejection_rate"].is_null());
    }

    #[test]
    fn forbidden_run_rate_uses_failures_over_applicable_expectations() {
        let mut mixed = scenario("mixed forbidden recall", "stale_truth_suppression");
        mixed.seed_memories = vec![
            memory("mem_forbidden_recalled", "Stale instruction.", "heartwood"),
            memory(
                "mem_forbidden_hidden",
                "Hidden stale instruction.",
                "heartwood",
            ),
        ];
        mixed.forbidden_recall = vec![
            RecallExpectation {
                memory_id: Some("mem_forbidden_recalled".to_string()),
                ..Default::default()
            },
            RecallExpectation {
                memory_id: Some("mem_forbidden_hidden".to_string()),
                ..Default::default()
            },
        ];
        let recalls = [QualityRecall {
            memory: memory("mem_forbidden_recalled", "Stale instruction.", "heartwood"),
            score: 0.8,
        }];
        let mut unrelated = scenario("unrelated", "evidence_preservation");
        let mut candidate = memory("mem_eval", "Evidence-backed evaluation.", "cambium");
        candidate.event_type = "evaluation_outcome".to_string();
        candidate.source.ref_ = "evals/run-001".to_string();
        unrelated.write_candidates = vec![candidate];
        unrelated.expected_write_decisions = vec![WriteDecisionExpectation {
            memory_id: "mem_eval".to_string(),
            decision: "accept".to_string(),
            reason: "valid unrelated evidence scenario".to_string(),
        }];
        unrelated.evidence_refs = vec!["evals/run-001".to_string()];
        let reports = vec![
            evaluate_quality_scenario(&mixed, &recalls).unwrap(),
            evaluate_quality_scenario(&unrelated, &[]).unwrap(),
        ];

        let run = serde_json::to_value(summarize_quality_run(reports)).unwrap();

        assert_eq!(run["forbidden_recall_rate"], 0.5);
    }

    #[test]
    fn applicable_thresholds_gate_scenario_quality() {
        let required = memory("mem_recalled", "Required constraint.", "heartwood");
        let mut recall = scenario("threshold gates", "constraint_recall");
        recall.expected_recall = vec![
            RecallExpectation {
                memory_id: Some("mem_recalled".to_string()),
                ..Default::default()
            },
            RecallExpectation {
                memory_id: Some("mem_missing".to_string()),
                ..Default::default()
            },
        ];
        recall.thresholds.min_constraint_recall_rate = Some(0.5);
        let recalls = [QualityRecall {
            memory: required,
            score: 0.9,
        }];
        assert_eq!(report_json(&recall, &recalls)["quality_pass"], true);
        recall.thresholds.min_constraint_recall_rate = Some(0.6);
        assert_eq!(report_json(&recall, &recalls)["quality_pass"], false);

        let mut forbidden = scenario("forbidden threshold", "stale_truth_suppression");
        forbidden.seed_memories = vec![memory("mem_stale", "Stale instruction.", "heartwood")];
        forbidden.forbidden_recall = vec![RecallExpectation {
            memory_id: Some("mem_stale".to_string()),
            ..Default::default()
        }];
        forbidden.thresholds.max_forbidden_recall_rate = Some(0.0);
        let stale = [QualityRecall {
            memory: memory("mem_stale", "Stale instruction.", "heartwood"),
            score: 0.9,
        }];
        assert_eq!(report_json(&forbidden, &stale)["quality_pass"], false);
    }

    #[test]
    fn evaluation_write_decisions_are_evidence_applicable() {
        let mut candidate = memory(
            "mem_evaluation",
            "Preserve the evaluated outcome with its evidence.",
            "cambium",
        );
        candidate.event_type = "evaluation_result".to_string();
        candidate.source.ref_.clear();
        let mut scenario = scenario("evidence applicability", "evidence_preservation");
        scenario.write_candidates = vec![candidate];
        scenario.expected_write_decisions = vec![WriteDecisionExpectation {
            memory_id: "mem_evaluation".to_string(),
            decision: "accept".to_string(),
            reason: "accepted outcomes preserve evidence".to_string(),
        }];

        let report = report_json(&scenario, &[]);

        assert_eq!(report["evidence_required_rate"], 0.0);
        assert_eq!(report["write_decisions"][0]["evidence_applicable"], true);
    }

    #[test]
    fn zero_scenario_run_is_failed_and_has_null_metrics() {
        let report = serde_json::to_value(summarize_quality_run(Vec::new())).unwrap();

        assert_eq!(report["ok"], false);
        assert_eq!(report["quality_pass"], false);
        assert!(report["constraint_recall_rate"].is_null());
        assert!(report["behavior_proof_pass_rate"].is_null());
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
        assert_eq!(
            scenario.expected_recall[0].memory_id.as_deref(),
            Some("mem_quality_constraint")
        );
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
          "category": "constraint_recall",
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
          "category": "constraint_recall",
          "query": "   ",
          "workflow_prompt": "  validate behavior proof  ",
          "expected_recall": [{"memory_id": "mem_required"}]
        }"#;

        let scenario = parse_quality_scenario(input).unwrap();

        assert_eq!(scenario.prompt(), Some("  validate behavior proof  "));
    }

    #[test]
    fn rejects_uncovered_write_candidate() {
        let input = r#"{
          "name": "missing decision",
          "category": "spam_prevention",
          "query": "write gates",
          "write_candidates": [
            {
              "id": "mem_candidate",
              "created_at": "2026-07-09T00:00:00Z",
              "updated_at": "2026-07-09T00:00:00Z",
              "project": "tree-ring",
              "agent_profile": null,
              "scope": "project",
              "ring": "heartwood",
              "event_type": "decision",
              "summary": "Candidate without expected decision.",
              "details": "",
              "source": {"type": "evidence", "ref": "docs/spec.md", "quote": ""},
              "tags": [],
              "salience": 0.8,
              "confidence": 0.8,
              "sensitivity": "normal",
              "retention": "durable",
              "expires_at": null,
              "supersedes": [],
              "superseded_by": null,
              "links": [],
              "review": {"needs_review": false, "review_reason": null, "reviewed_at": null, "reviewed_by": null}
            }
          ]
        }"#;

        let error = parse_quality_scenario(input).unwrap_err().to_string();

        assert!(error.contains("write_candidates[0]"));
        assert!(error.contains("missing expected_write_decision"));
    }

    #[test]
    fn rejects_orphan_write_decision_mapping() {
        let scenario = QualityScenario {
            name: "orphan decision".to_string(),
            category: "spam_prevention".to_string(),
            seed_memories: Vec::new(),
            query: Some("write gates".to_string()),
            workflow_prompt: None,
            expected_recall: Vec::new(),
            forbidden_recall: Vec::new(),
            write_candidates: vec![memory("mem_real", "Real candidate.", "heartwood")],
            expected_write_decisions: vec![WriteDecisionExpectation {
                memory_id: "mem_missing".to_string(),
                decision: "reject".to_string(),
                reason: "orphan mapping".to_string(),
            }],
            evidence_refs: Vec::new(),
            behavior_expectation: None,
            thresholds: QualityThresholds::default(),
        };

        let error = scenario.validate().unwrap_err().to_string();

        assert!(error.contains("expected_write_decisions[0]"));
        assert!(error.contains("does not match any write_candidate"));
    }

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

        assert_eq!(parsed, 7);
    }

    #[test]
    fn rejects_duplicate_write_decision_mapping() {
        let scenario = QualityScenario {
            name: "duplicate decision".to_string(),
            category: "spam_prevention".to_string(),
            seed_memories: Vec::new(),
            query: Some("write gates".to_string()),
            workflow_prompt: None,
            expected_recall: Vec::new(),
            forbidden_recall: Vec::new(),
            write_candidates: vec![memory("mem_real", "Real candidate.", "heartwood")],
            expected_write_decisions: vec![
                WriteDecisionExpectation {
                    memory_id: "mem_real".to_string(),
                    decision: "reject".to_string(),
                    reason: "first mapping".to_string(),
                },
                WriteDecisionExpectation {
                    memory_id: "mem_real".to_string(),
                    decision: "accept".to_string(),
                    reason: "duplicate mapping".to_string(),
                },
            ],
            evidence_refs: Vec::new(),
            behavior_expectation: None,
            thresholds: QualityThresholds::default(),
        };

        let error = scenario.validate().unwrap_err().to_string();

        assert!(error.contains("expected_write_decisions[1]"));
        assert!(error.contains("duplicate"));
    }

    #[test]
    fn evaluates_required_and_forbidden_recall() {
        let scenario = QualityScenario {
            name: "recall gate".to_string(),
            category: "constraint_recall".to_string(),
            seed_memories: vec![
                memory(
                    "mem_required",
                    "Do not add a background writer.",
                    "heartwood",
                ),
                memory("mem_forbidden", "Stale instruction.", "heartwood"),
            ],
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
            behavior_expectation: None,
            thresholds: QualityThresholds::default(),
        };
        scenario.validate().unwrap();

        let report = evaluate_quality_scenario(
            &scenario,
            &[QualityRecall {
                memory: memory(
                    "mem_required",
                    "Do not add a background writer.",
                    "heartwood",
                ),
                score: 0.91,
            }],
        )
        .unwrap();

        assert!(report.quality_pass);
        assert_eq!(report.constraint_recall_rate, Some(1.0));
        assert_eq!(report.forbidden_recall_rate, Some(0.0));
        assert!(report.expected_recall[0].passed);
        assert!(report.forbidden_recall[0].passed);
    }

    #[test]
    fn validates_forbidden_recall_against_seed_memories() {
        let scenario = QualityScenario {
            name: "stale recall seed coverage".to_string(),
            category: "stale_truth_suppression".to_string(),
            seed_memories: vec![memory("mem_stale", "Old CLI contract.", "heartwood")],
            query: Some("cli contract".to_string()),
            workflow_prompt: None,
            expected_recall: Vec::new(),
            forbidden_recall: vec![RecallExpectation {
                ring: Some("heartwood".to_string()),
                reason: "seeded stale memory".to_string(),
                ..Default::default()
            }],
            write_candidates: Vec::new(),
            expected_write_decisions: Vec::new(),
            evidence_refs: Vec::new(),
            behavior_expectation: None,
            thresholds: QualityThresholds::default(),
        };

        scenario.validate().unwrap();
    }

    #[test]
    fn rejects_forbidden_recall_with_missing_seed_selector() {
        let scenario = QualityScenario {
            name: "stale recall typo".to_string(),
            category: "stale_truth_suppression".to_string(),
            seed_memories: vec![memory("mem_stale", "Old CLI contract.", "heartwood")],
            query: Some("cli contract".to_string()),
            workflow_prompt: None,
            expected_recall: Vec::new(),
            forbidden_recall: vec![RecallExpectation {
                memory_id: Some("mem_typo".to_string()),
                reason: "typo selector".to_string(),
                ..Default::default()
            }],
            write_candidates: Vec::new(),
            expected_write_decisions: Vec::new(),
            evidence_refs: Vec::new(),
            behavior_expectation: None,
            thresholds: QualityThresholds::default(),
        };

        let error = scenario.validate().unwrap_err().to_string();

        assert!(error.contains("forbidden_recall[0]"));
        assert!(error.contains("does not match a seed_memory"));
    }

    #[test]
    fn fails_when_forbidden_memory_is_recalled() {
        let scenario = QualityScenario {
            name: "stale recall".to_string(),
            category: "stale_truth_suppression".to_string(),
            seed_memories: vec![memory("mem_stale", "Old CLI contract.", "heartwood")],
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
            behavior_expectation: None,
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
        assert_eq!(report.forbidden_recall_rate, Some(1.0));
        assert!(!report.forbidden_recall[0].passed);
    }

    #[test]
    fn classifies_write_candidates_against_expected_decisions() {
        let mut spam = memory(
            "mem_spam",
            "Thinking about options for a moment.",
            "heartwood",
        );
        spam.tags = vec!["transient".to_string()];
        let mut promoted_without_evidence = memory(
            "mem_missing_evidence",
            "Promote evaluated proof.",
            "heartwood",
        );
        promoted_without_evidence.event_type = "evaluation_promotion".to_string();
        promoted_without_evidence.source.ref_.clear();
        let mut broad_heartwood = memory(
            "mem_needs_confirmation",
            "All projects should prefer this.",
            "heartwood",
        );
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
            behavior_expectation: None,
            thresholds: QualityThresholds::default(),
        };

        let report = evaluate_quality_scenario(&scenario, &[]).unwrap();

        assert!(report.quality_pass);
        assert_eq!(report.spam_rejection_rate, Some(1.0));
        assert_eq!(report.evidence_required_rate, Some(1.0));
        assert_eq!(report.write_decisions.len(), 3);
    }

    #[test]
    fn quality_pass_fails_when_expected_reject_is_accepted() {
        let scenario = QualityScenario {
            name: "reject mismatch".to_string(),
            category: "spam_prevention".to_string(),
            seed_memories: Vec::new(),
            query: Some("write gates".to_string()),
            workflow_prompt: None,
            expected_recall: Vec::new(),
            forbidden_recall: Vec::new(),
            write_candidates: vec![memory(
                "mem_accept",
                "Durable evidence-backed note.",
                "cambium",
            )],
            expected_write_decisions: vec![WriteDecisionExpectation {
                memory_id: "mem_accept".to_string(),
                decision: "reject".to_string(),
                reason: "should have been rejected".to_string(),
            }],
            evidence_refs: Vec::new(),
            behavior_expectation: None,
            thresholds: QualityThresholds::default(),
        };

        let report = evaluate_quality_scenario(&scenario, &[]).unwrap();

        assert!(!report.quality_pass);
        assert_eq!(report.write_decisions[0].actual, "accept");
        assert!(!report.write_decisions[0].passed);
    }

    #[test]
    fn quality_pass_fails_when_expected_require_evidence_is_accepted() {
        let mut candidate = memory("mem_accept", "Durable evidence-backed note.", "cambium");
        candidate.event_type = "evaluation_outcome".to_string();
        candidate.source.ref_ = "evals/run-002".to_string();
        let scenario = QualityScenario {
            name: "require evidence mismatch".to_string(),
            category: "evidence_preservation".to_string(),
            seed_memories: Vec::new(),
            query: Some("write gates".to_string()),
            workflow_prompt: None,
            expected_recall: Vec::new(),
            forbidden_recall: Vec::new(),
            write_candidates: vec![candidate],
            expected_write_decisions: vec![WriteDecisionExpectation {
                memory_id: "mem_accept".to_string(),
                decision: "require_evidence".to_string(),
                reason: "should need evidence".to_string(),
            }],
            evidence_refs: vec!["evals/run-002".to_string()],
            behavior_expectation: None,
            thresholds: QualityThresholds::default(),
        };

        let report = evaluate_quality_scenario(&scenario, &[]).unwrap();

        assert!(!report.quality_pass);
        assert_eq!(report.write_decisions[0].actual, "accept");
        assert!(!report.write_decisions[0].passed);
    }

    #[test]
    fn quality_pass_fails_when_confirmation_mismatch_occurs() {
        let mut candidate = memory("mem_accept", "Durable evidence-backed note.", "cambium");
        candidate.event_type = "evaluation_outcome".to_string();
        candidate.source.ref_ = "evals/run-003".to_string();
        let scenario = QualityScenario {
            name: "confirmation mismatch".to_string(),
            category: "evidence_preservation".to_string(),
            seed_memories: Vec::new(),
            query: Some("write gates".to_string()),
            workflow_prompt: None,
            expected_recall: Vec::new(),
            forbidden_recall: Vec::new(),
            write_candidates: vec![candidate],
            expected_write_decisions: vec![WriteDecisionExpectation {
                memory_id: "mem_accept".to_string(),
                decision: "require_user_confirmation".to_string(),
                reason: "should require confirmation".to_string(),
            }],
            evidence_refs: vec!["evals/run-003".to_string()],
            behavior_expectation: None,
            thresholds: QualityThresholds::default(),
        };

        let report = evaluate_quality_scenario(&scenario, &[]).unwrap();

        assert!(!report.quality_pass);
        assert_eq!(report.write_decisions[0].actual, "accept");
        assert!(!report.write_decisions[0].passed);
    }

    #[test]
    fn summarizes_quality_run() {
        let passing = QualityScenarioReport {
            name: "pass".to_string(),
            category: "constraint_recall".to_string(),
            constraint_recall_rate: Some(1.0),
            forbidden_recall_rate: Some(0.0),
            spam_rejection_rate: Some(1.0),
            evidence_required_rate: Some(1.0),
            behavior_proof_pass: Some(true),
            behavior_expectation: None,
            quality_pass: true,
            expected_recall: Vec::new(),
            forbidden_recall: Vec::new(),
            write_decisions: Vec::new(),
        };

        let run = summarize_quality_run(vec![passing.clone(), passing]);

        assert!(run.quality_pass);
        assert_eq!(run.scenario_count, 2);
        assert_eq!(run.behavior_proof_pass_rate, Some(1.0));
    }
}
