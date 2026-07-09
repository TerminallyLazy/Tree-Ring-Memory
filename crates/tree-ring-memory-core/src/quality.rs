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
