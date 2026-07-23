use chrono::{DateTime, Datelike, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::models::{
    now_iso, MemoryEvent, MemoryLink, MemoryReview, MemorySource, TreeRingError, TreeRingResult,
};
use crate::sensitivity::SensitivityGuard;

pub const CONSOLIDATION_PERIODS: &[&str] = &["daily", "weekly", "monthly", "yearly", "manual"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsolidationPeriod {
    Daily,
    Weekly,
    Monthly,
    Yearly,
    Manual,
}

impl ConsolidationPeriod {
    pub fn parse(value: &str) -> TreeRingResult<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "daily" => Ok(Self::Daily),
            "weekly" => Ok(Self::Weekly),
            "monthly" => Ok(Self::Monthly),
            "yearly" => Ok(Self::Yearly),
            "manual" => Ok(Self::Manual),
            other => Err(TreeRingError::Validation(format!(
                "unsupported period_type: {other}"
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Yearly => "yearly",
            Self::Manual => "manual",
        }
    }

    pub fn default_output_ring(&self) -> &'static str {
        match self {
            Self::Daily | Self::Manual => "outer",
            Self::Weekly | Self::Monthly | Self::Yearly => "inner",
        }
    }
}

impl fmt::Display for ConsolidationPeriod {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<&str> for ConsolidationPeriod {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsolidationRequest {
    pub period_type: ConsolidationPeriod,
    pub period_key: Option<String>,
    pub project: Option<String>,
    #[serde(default)]
    pub agent_profile: Option<String>,
    #[serde(default)]
    pub workflow_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub force: bool,
}

impl ConsolidationRequest {
    pub fn new(period_type: &str) -> TreeRingResult<Self> {
        Ok(Self {
            period_type: ConsolidationPeriod::parse(period_type)?,
            period_key: None,
            project: None,
            agent_profile: None,
            workflow_id: None,
            session_id: None,
            dry_run: false,
            force: false,
        })
    }

    pub fn resolved_period_key(&self) -> String {
        self.period_key
            .clone()
            .unwrap_or_else(|| period_key_for_datetime(self.period_type, Utc::now()))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsolidationOutput {
    pub memory: MemoryEvent,
    pub source_memory_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsolidationReport {
    pub id: String,
    pub created_at: String,
    pub period_type: ConsolidationPeriod,
    pub period_key: String,
    pub candidate_count: usize,
    pub source_memory_ids: Vec<String>,
    pub output_memory_ids: Vec<String>,
    pub dry_run: bool,
    pub force: bool,
    pub status: String,
    pub notes: String,
    pub outputs: Vec<ConsolidationOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct GroupKey {
    project: Option<String>,
    scope: String,
    agent_partition: Option<String>,
    workflow_partition: Option<String>,
    session_partition: Option<String>,
    ring: String,
    event_type: String,
    sensitivity_bucket: String,
}

pub fn consolidate_memories(
    events: &[MemoryEvent],
    request: &ConsolidationRequest,
) -> TreeRingResult<ConsolidationReport> {
    let period_key = request.resolved_period_key();
    let mut candidates: Vec<&MemoryEvent> = events
        .iter()
        .filter(|event| is_candidate(event, request, &period_key))
        .collect();
    candidates.sort_by(|left, right| left.id.cmp(&right.id));

    let source_memory_ids: Vec<String> = candidates.iter().map(|event| event.id.clone()).collect();
    if candidates.is_empty() {
        return Ok(ConsolidationReport {
            id: generated_consolidation_id(),
            created_at: now_iso(),
            period_type: request.period_type,
            period_key,
            candidate_count: 0,
            source_memory_ids,
            output_memory_ids: Vec::new(),
            dry_run: request.dry_run,
            force: request.force,
            status: if request.dry_run { "dry_run" } else { "empty" }.to_string(),
            notes: "No memories matched consolidation criteria.".to_string(),
            outputs: Vec::new(),
        });
    }

    let mut groups: BTreeMap<GroupKey, Vec<&MemoryEvent>> = BTreeMap::new();
    for event in candidates {
        groups.entry(group_key(event)).or_default().push(event);
    }

    let mut outputs = Vec::new();
    for (key, group_events) in groups {
        outputs.push(build_output(
            &key,
            &group_events,
            request.period_type,
            &period_key,
        )?);
    }
    let output_memory_ids = outputs
        .iter()
        .map(|output| output.memory.id.clone())
        .collect::<Vec<_>>();

    Ok(ConsolidationReport {
        id: generated_consolidation_id(),
        created_at: now_iso(),
        period_type: request.period_type,
        period_key,
        candidate_count: source_memory_ids.len(),
        source_memory_ids,
        output_memory_ids,
        dry_run: request.dry_run,
        force: request.force,
        status: if request.dry_run {
            "dry_run"
        } else {
            "planned"
        }
        .to_string(),
        notes: "Consolidation plan generated.".to_string(),
        outputs,
    })
}

pub fn period_key_for_datetime(
    period_type: ConsolidationPeriod,
    datetime: DateTime<Utc>,
) -> String {
    match period_type {
        ConsolidationPeriod::Daily => datetime.format("%Y-%m-%d").to_string(),
        ConsolidationPeriod::Weekly => {
            let week = datetime.iso_week();
            format!("{}-W{:02}", week.year(), week.week())
        }
        ConsolidationPeriod::Monthly => datetime.format("%Y-%m").to_string(),
        ConsolidationPeriod::Yearly => datetime.format("%Y").to_string(),
        ConsolidationPeriod::Manual => {
            format!(
                "manual-{}",
                datetime
                    .to_rfc3339_opts(SecondsFormat::Secs, true)
                    .replace(['-', ':'], "")
            )
        }
    }
}

fn is_candidate(event: &MemoryEvent, request: &ConsolidationRequest, period_key: &str) -> bool {
    if event.superseded_by.is_some() {
        return false;
    }
    if event.event_type == "summary" && event.source.source_type == "consolidation" {
        return false;
    }
    if request
        .project
        .as_ref()
        .is_some_and(|project| event.project.as_ref() != Some(project))
    {
        return false;
    }
    if request
        .agent_profile
        .as_ref()
        .is_some_and(|agent_profile| event.agent_profile.as_ref() != Some(agent_profile))
    {
        return false;
    }
    if request
        .workflow_id
        .as_ref()
        .is_some_and(|workflow_id| event.workflow_id.as_ref() != Some(workflow_id))
    {
        return false;
    }
    if request
        .session_id
        .as_ref()
        .is_some_and(|session_id| event.session_id.as_ref() != Some(session_id))
    {
        return false;
    }
    if effective_sensitivity(event) == "secret" {
        return false;
    }
    if request.period_type != ConsolidationPeriod::Manual
        && event_period_key(event, request.period_type).as_deref() != Some(period_key)
    {
        return false;
    }
    event.salience >= 0.45
        || matches!(event.ring.as_str(), "heartwood" | "scar" | "seed")
        || matches!(event.retention.as_str(), "durable" | "user_pinned")
}

fn group_key(event: &MemoryEvent) -> GroupKey {
    let sensitivity = effective_sensitivity(event);
    GroupKey {
        project: event.project.clone(),
        scope: event.scope.clone(),
        agent_partition: (event.scope == "agent")
            .then(|| event.agent_profile.clone())
            .flatten(),
        workflow_partition: (event.scope == "workflow")
            .then(|| event.workflow_id.clone())
            .flatten(),
        session_partition: (event.scope == "session")
            .then(|| event.session_id.clone())
            .flatten(),
        ring: event.ring.clone(),
        event_type: event.event_type.clone(),
        sensitivity_bucket: if sensitivity == "normal" {
            "normal".to_string()
        } else {
            "sensitive".to_string()
        },
    }
}

fn effective_sensitivity(event: &MemoryEvent) -> String {
    let guard = SensitivityGuard::new(false);
    let detected = guard
        .detect_memory_event_sensitivity(event)
        .unwrap_or_else(|_| "secret".to_string());
    if detected == "secret" || event.sensitivity == "secret" {
        "secret".to_string()
    } else if event.sensitivity != "normal" {
        event.sensitivity.clone()
    } else {
        detected
    }
}

fn build_output(
    key: &GroupKey,
    events: &[&MemoryEvent],
    period_type: ConsolidationPeriod,
    period_key: &str,
) -> TreeRingResult<ConsolidationOutput> {
    let source_memory_ids = events
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    let tags = top_tags(events);
    let sensitive = key.sensitivity_bucket == "sensitive";
    let summary = if sensitive {
        format!(
            "Consolidated {} sensitive memory group requiring review.",
            events.len()
        )
    } else {
        let project_label = key.project.as_deref().unwrap_or("global");
        format!(
            "Consolidated {} {} memory group for project {} in {} ring.",
            events.len(),
            key.event_type,
            project_label,
            key.ring
        )
    };
    let mut memory = MemoryEvent::new(summary, "summary")?;
    memory.project = key.project.clone();
    memory.agent_profile =
        uniform_optional(events.iter().map(|event| event.agent_profile.as_deref()));
    memory.workflow_id = uniform_optional(events.iter().map(|event| event.workflow_id.as_deref()));
    memory.session_id = uniform_optional(events.iter().map(|event| event.session_id.as_deref()));
    memory.operation_id = None;
    memory.scope = key.scope.clone();
    memory.ring = output_ring(period_type, key, events).to_string();
    memory.details = format!(
        "Period: {}:{}; source_count={}; source_ids={}",
        period_type,
        period_key,
        events.len(),
        source_memory_ids.join(",")
    );
    memory.source = MemorySource {
        source_type: "consolidation".to_string(),
        ref_: format!("{period_type}:{period_key}"),
        quote: String::new(),
    };
    memory.tags = tags;
    memory.salience = average(events.iter().map(|event| event.salience));
    memory.confidence = average(events.iter().map(|event| event.confidence));
    memory.sensitivity = if sensitive { "private" } else { "normal" }.to_string();
    memory.retention = "normal".to_string();
    memory.links = source_memory_ids
        .iter()
        .map(|id| MemoryLink {
            link_type: "memory".to_string(),
            target: id.clone(),
        })
        .collect();
    memory.review = MemoryReview {
        needs_review: sensitive,
        review_reason: sensitive
            .then(|| "Sensitive memories contributed to this consolidation.".to_string()),
        reviewed_at: None,
        reviewed_by: None,
    };
    memory.validate()?;
    Ok(ConsolidationOutput {
        memory,
        source_memory_ids,
    })
}

fn output_ring(
    period_type: ConsolidationPeriod,
    key: &GroupKey,
    events: &[&MemoryEvent],
) -> &'static str {
    match key.ring.as_str() {
        "scar" => "scar",
        "seed" => "seed",
        "heartwood" if average(events.iter().map(|event| event.confidence)) >= 0.75 => "heartwood",
        _ => period_type.default_output_ring(),
    }
}

fn top_tags(events: &[&MemoryEvent]) -> Vec<String> {
    let guard = SensitivityGuard::new(false);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for event in events {
        for tag in &event.tags {
            if guard.inspect(tag).sensitivity != "normal" {
                continue;
            }
            *counts.entry(tag.clone()).or_default() += 1;
        }
    }
    let mut ranked = counts.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let mut tags = ranked
        .into_iter()
        .take(5)
        .map(|(tag, _)| tag)
        .collect::<BTreeSet<_>>();
    tags.insert("consolidation".to_string());
    tags.into_iter().collect()
}

fn average(values: impl Iterator<Item = f64>) -> f64 {
    let mut count = 0.0;
    let mut total = 0.0;
    for value in values {
        count += 1.0;
        total += value;
    }
    if count == 0.0 {
        0.5
    } else {
        (total / count).clamp(0.0, 1.0)
    }
}

fn uniform_optional<'a>(mut values: impl Iterator<Item = Option<&'a str>>) -> Option<String> {
    let first = values.next()?;
    if values.all(|value| value == first) {
        first.map(ToOwned::to_owned)
    } else {
        None
    }
}

fn event_period_key(event: &MemoryEvent, period_type: ConsolidationPeriod) -> Option<String> {
    let created_at = DateTime::parse_from_rfc3339(&event.created_at).ok()?;
    Some(period_key_for_datetime(
        period_type,
        created_at.with_timezone(&Utc),
    ))
}

fn generated_consolidation_id() -> String {
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    format!(
        "con_{timestamp}_{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn event(summary: &str, project: &str, created_at: &str) -> MemoryEvent {
        let mut event = MemoryEvent::new(summary, "decision").unwrap();
        event.project = Some(project.to_string());
        event.scope = "project".to_string();
        event.created_at = created_at.to_string();
        event.updated_at = created_at.to_string();
        event.tags = vec!["cache".to_string()];
        event.salience = 0.8;
        event.confidence = 0.7;
        event
    }

    #[test]
    fn derives_stable_period_keys() {
        let instant = Utc.with_ymd_and_hms(2026, 7, 5, 8, 0, 0).unwrap();

        assert_eq!(
            period_key_for_datetime(ConsolidationPeriod::Daily, instant),
            "2026-07-05"
        );
        assert_eq!(
            period_key_for_datetime(ConsolidationPeriod::Monthly, instant),
            "2026-07"
        );
        assert_eq!(
            period_key_for_datetime(ConsolidationPeriod::Yearly, instant),
            "2026"
        );
    }

    #[test]
    fn creates_deterministic_safe_summary_outputs() {
        let first = event("Use snapshot invalidation.", "ui", "2026-07-05T08:00:00Z");
        let second = event("Avoid stale cache.", "ui", "2026-07-05T09:00:00Z");
        let request = ConsolidationRequest {
            period_type: ConsolidationPeriod::Daily,
            period_key: Some("2026-07-05".to_string()),
            project: Some("ui".to_string()),
            agent_profile: None,
            workflow_id: None,
            session_id: None,
            dry_run: false,
            force: false,
        };

        let report = consolidate_memories(&[second.clone(), first.clone()], &request).unwrap();
        let mut expected_source_ids = vec![first.id.clone(), second.id.clone()];
        expected_source_ids.sort();

        assert_eq!(report.candidate_count, 2);
        assert_eq!(report.source_memory_ids, expected_source_ids);
        assert_eq!(report.outputs.len(), 1);
        assert_eq!(report.outputs[0].memory.ring, "outer");
        assert_eq!(report.outputs[0].memory.event_type, "summary");
        assert!(!report.outputs[0].memory.summary.contains("snapshot"));
        assert_eq!(report.outputs[0].memory.links.len(), 2);
    }

    #[test]
    fn excludes_secret_and_superseded_memories() {
        let mut secret = event(
            "sk-proj-secret should not consolidate.",
            "ui",
            "2026-07-05T08:00:00Z",
        );
        secret.sensitivity = "secret".to_string();
        let mut superseded = event("Old stale decision.", "ui", "2026-07-05T09:00:00Z");
        superseded.superseded_by = Some("mem_new".to_string());
        let request = ConsolidationRequest {
            period_type: ConsolidationPeriod::Daily,
            period_key: Some("2026-07-05".to_string()),
            project: Some("ui".to_string()),
            agent_profile: None,
            workflow_id: None,
            session_id: None,
            dry_run: true,
            force: false,
        };

        let report = consolidate_memories(&[secret, superseded], &request).unwrap();

        assert_eq!(report.status, "dry_run");
        assert_eq!(report.candidate_count, 0);
        assert!(report.outputs.is_empty());
    }

    #[test]
    fn excludes_secret_like_memory_even_when_stored_label_is_normal() {
        let mut source = event(
            "Use key sk-proj-abcdefghijklmnopqrstuvwxyz1234567890.",
            "ui",
            "2026-07-05T08:00:00Z",
        );
        source.sensitivity = "normal".to_string();
        let request = ConsolidationRequest {
            period_type: ConsolidationPeriod::Daily,
            period_key: Some("2026-07-05".to_string()),
            project: Some("ui".to_string()),
            agent_profile: None,
            workflow_id: None,
            session_id: None,
            dry_run: true,
            force: false,
        };

        let report = consolidate_memories(&[source], &request).unwrap();

        assert_eq!(report.candidate_count, 0);
        assert!(report.outputs.is_empty());
    }

    #[test]
    fn sensitive_non_secret_summary_requires_review_without_payload() {
        let mut sensitive = event("Private diagnosis text.", "ui", "2026-07-05T08:00:00Z");
        sensitive.sensitivity = "health".to_string();
        let request = ConsolidationRequest {
            period_type: ConsolidationPeriod::Manual,
            period_key: Some("manual-test".to_string()),
            project: Some("ui".to_string()),
            agent_profile: None,
            workflow_id: None,
            session_id: None,
            dry_run: false,
            force: false,
        };

        let report = consolidate_memories(&[sensitive], &request).unwrap();

        let output = &report.outputs[0].memory;
        assert_eq!(output.sensitivity, "private");
        assert!(output.review.needs_review);
        assert!(!output.summary.contains("diagnosis"));
        assert!(!output.details.contains("diagnosis"));
    }

    #[test]
    fn sensitive_metadata_labels_do_not_leak_into_generated_text() {
        let mut sensitive = event("Safe summary.", "diagnosis_lesson", "2026-07-05T08:00:00Z");
        sensitive.project = Some("private diagnosis program".to_string());
        sensitive.sensitivity = "normal".to_string();
        let request = ConsolidationRequest {
            period_type: ConsolidationPeriod::Manual,
            period_key: Some("manual-test".to_string()),
            project: Some("private diagnosis program".to_string()),
            agent_profile: None,
            workflow_id: None,
            session_id: None,
            dry_run: false,
            force: false,
        };

        let report = consolidate_memories(&[sensitive], &request).unwrap();
        let output = &report.outputs[0].memory;

        assert_eq!(output.sensitivity, "private");
        assert!(output.review.needs_review);
        assert!(!output.summary.contains("diagnosis"));
        assert!(!output.summary.contains("private diagnosis program"));
        assert!(!output.summary.contains("diagnosis_lesson"));
        assert!(!output.details.contains("diagnosis"));
        assert!(!output.details.contains("private diagnosis program"));
        assert!(!output.details.contains("diagnosis_lesson"));
    }

    #[test]
    fn unsafe_source_tags_do_not_leak_into_summary() {
        let mut source = event("Safe project decision.", "ui", "2026-07-05T08:00:00Z");
        source.tags = vec!["memory".to_string(), "diagnosis".to_string()];
        let request = ConsolidationRequest {
            period_type: ConsolidationPeriod::Manual,
            period_key: Some("manual-test".to_string()),
            project: Some("ui".to_string()),
            agent_profile: None,
            workflow_id: None,
            session_id: None,
            dry_run: false,
            force: false,
        };

        let report = consolidate_memories(&[source], &request).unwrap();
        let output = &report.outputs[0].memory;

        assert_eq!(output.tags, vec!["consolidation", "memory"]);
        assert!(!output.tags.iter().any(|tag| tag.contains("diagnosis")));
    }

    #[test]
    fn legacy_consolidation_request_defaults_private_scope_filters() {
        let request: ConsolidationRequest = serde_json::from_value(serde_json::json!({
            "period_type": "manual",
            "period_key": "manual-test",
            "project": "ui",
            "dry_run": false,
            "force": false
        }))
        .unwrap();

        assert_eq!(request.agent_profile, None);
        assert_eq!(request.workflow_id, None);
        assert_eq!(request.session_id, None);
    }

    #[test]
    fn agent_scoped_consolidation_isolates_producers_and_retains_correlation() {
        let mut coder = event("Use the implementation plan.", "ui", "2026-07-05T08:00:00Z");
        coder.scope = "agent".to_string();
        coder.agent_profile = Some("coder".to_string());
        coder.workflow_id = Some("workflow-1".to_string());
        coder.session_id = Some("session-1".to_string());
        coder.operation_id = Some("operation-code".to_string());
        coder.tags = vec!["coordination".to_string(), "implementation".to_string()];

        let mut reviewer = event("Review the implementation.", "ui", "2026-07-05T09:00:00Z");
        reviewer.scope = "agent".to_string();
        reviewer.agent_profile = Some("reviewer".to_string());
        reviewer.workflow_id = Some("workflow-1".to_string());
        reviewer.session_id = Some("session-1".to_string());
        reviewer.operation_id = Some("operation-review".to_string());
        reviewer.tags = vec!["coordination".to_string(), "review".to_string()];

        let report = consolidate_memories(
            &[reviewer.clone(), coder.clone()],
            &ConsolidationRequest {
                period_type: ConsolidationPeriod::Manual,
                period_key: Some("manual-agent-isolation".to_string()),
                project: Some("ui".to_string()),
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                dry_run: false,
                force: false,
            },
        )
        .unwrap();

        assert_eq!(report.outputs.len(), 2);
        for source in [&coder, &reviewer] {
            let output = report
                .outputs
                .iter()
                .find(|output| output.memory.agent_profile == source.agent_profile)
                .unwrap();
            assert_eq!(output.source_memory_ids, vec![source.id.clone()]);
            assert_eq!(output.memory.workflow_id, source.workflow_id);
            assert_eq!(output.memory.session_id, source.session_id);
            assert_eq!(output.memory.operation_id, None);
            assert!(output.memory.tags.contains(&"coordination".to_string()));
            assert_eq!(
                output.memory.links,
                vec![MemoryLink {
                    link_type: "memory".to_string(),
                    target: source.id.clone(),
                }]
            );
            output.memory.validate().unwrap();
        }
    }

    #[test]
    fn workflow_scoped_consolidation_isolates_workflows_without_claiming_mixed_producers() {
        let mut coder = event("Use the implementation plan.", "ui", "2026-07-05T08:00:00Z");
        coder.scope = "workflow".to_string();
        coder.agent_profile = Some("coder".to_string());
        coder.workflow_id = Some("workflow-1".to_string());
        coder.session_id = Some("session-shared".to_string());
        coder.operation_id = Some("operation-code".to_string());

        let mut reviewer = event("Review the implementation.", "ui", "2026-07-05T09:00:00Z");
        reviewer.scope = "workflow".to_string();
        reviewer.agent_profile = Some("reviewer".to_string());
        reviewer.workflow_id = Some("workflow-1".to_string());
        reviewer.session_id = Some("session-shared".to_string());
        reviewer.operation_id = Some("operation-review".to_string());

        let mut other_workflow = event("Coordinate the release.", "ui", "2026-07-05T10:00:00Z");
        other_workflow.scope = "workflow".to_string();
        other_workflow.agent_profile = Some("coordinator".to_string());
        other_workflow.workflow_id = Some("workflow-2".to_string());
        other_workflow.session_id = Some("session-other".to_string());
        other_workflow.operation_id = Some("operation-release".to_string());

        let report = consolidate_memories(
            &[coder.clone(), reviewer.clone(), other_workflow],
            &ConsolidationRequest {
                period_type: ConsolidationPeriod::Manual,
                period_key: Some("manual-workflow-isolation".to_string()),
                project: Some("ui".to_string()),
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                dry_run: false,
                force: false,
            },
        )
        .unwrap();

        assert_eq!(report.outputs.len(), 2);
        let shared_workflow = report
            .outputs
            .iter()
            .find(|output| output.memory.workflow_id.as_deref() == Some("workflow-1"))
            .unwrap();
        assert_eq!(shared_workflow.source_memory_ids.len(), 2);
        assert_eq!(shared_workflow.memory.agent_profile, None);
        assert_eq!(
            shared_workflow.memory.session_id.as_deref(),
            Some("session-shared")
        );
        assert_eq!(shared_workflow.memory.operation_id, None);
        shared_workflow.memory.validate().unwrap();
    }

    #[test]
    fn session_scoped_consolidation_isolates_sessions() {
        let mut first = event("Use the implementation plan.", "ui", "2026-07-05T08:00:00Z");
        first.scope = "session".to_string();
        first.agent_profile = Some("coder".to_string());
        first.workflow_id = Some("workflow-1".to_string());
        first.session_id = Some("session-1".to_string());

        let mut second = event("Review the implementation.", "ui", "2026-07-05T09:00:00Z");
        second.scope = "session".to_string();
        second.agent_profile = Some("reviewer".to_string());
        second.workflow_id = Some("workflow-2".to_string());
        second.session_id = Some("session-1".to_string());

        let mut other_session = event("Coordinate the release.", "ui", "2026-07-05T10:00:00Z");
        other_session.scope = "session".to_string();
        other_session.agent_profile = Some("coordinator".to_string());
        other_session.workflow_id = Some("workflow-1".to_string());
        other_session.session_id = Some("session-2".to_string());

        let report = consolidate_memories(
            &[first, second, other_session],
            &ConsolidationRequest {
                period_type: ConsolidationPeriod::Manual,
                period_key: Some("manual-session-isolation".to_string()),
                project: Some("ui".to_string()),
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                dry_run: false,
                force: false,
            },
        )
        .unwrap();

        assert_eq!(report.outputs.len(), 2);
        let shared_session = report
            .outputs
            .iter()
            .find(|output| output.memory.session_id.as_deref() == Some("session-1"))
            .unwrap();
        assert_eq!(shared_session.source_memory_ids.len(), 2);
        assert_eq!(shared_session.memory.agent_profile, None);
        assert_eq!(shared_session.memory.workflow_id, None);
        shared_session.memory.validate().unwrap();
    }

    #[test]
    fn shared_scope_consolidation_clears_mixed_producer_and_correlation_claims() {
        let mut coder = event("Use the implementation plan.", "ui", "2026-07-05T08:00:00Z");
        coder.agent_profile = Some("coder".to_string());
        coder.workflow_id = Some("workflow-1".to_string());
        coder.session_id = Some("session-1".to_string());
        coder.operation_id = Some("operation-code".to_string());

        let mut reviewer = event("Review the implementation.", "ui", "2026-07-05T09:00:00Z");
        reviewer.agent_profile = Some("reviewer".to_string());
        reviewer.workflow_id = Some("workflow-2".to_string());
        reviewer.session_id = Some("session-2".to_string());
        reviewer.operation_id = Some("operation-review".to_string());

        let report = consolidate_memories(
            &[coder, reviewer],
            &ConsolidationRequest {
                period_type: ConsolidationPeriod::Manual,
                period_key: Some("manual-shared-producers".to_string()),
                project: Some("ui".to_string()),
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                dry_run: false,
                force: false,
            },
        )
        .unwrap();

        assert_eq!(report.outputs.len(), 1);
        let output = &report.outputs[0];
        assert_eq!(output.source_memory_ids.len(), 2);
        assert_eq!(output.memory.agent_profile, None);
        assert_eq!(output.memory.workflow_id, None);
        assert_eq!(output.memory.session_id, None);
        assert_eq!(output.memory.operation_id, None);
        assert_eq!(output.memory.links.len(), 2);
    }

    #[test]
    fn derived_consolidation_output_does_not_reuse_a_source_operation_id() {
        let mut source = event("Use the accepted plan.", "ui", "2026-07-05T08:00:00Z");
        source.operation_id = Some("operation-source".to_string());

        let report = consolidate_memories(
            &[source],
            &ConsolidationRequest {
                period_type: ConsolidationPeriod::Manual,
                period_key: Some("manual-derived-operation".to_string()),
                project: Some("ui".to_string()),
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                dry_run: false,
                force: false,
            },
        )
        .unwrap();

        assert_eq!(report.outputs.len(), 1);
        assert_eq!(report.outputs[0].memory.operation_id, None);
    }

    #[test]
    fn consolidation_request_filters_each_private_scope_dimension() {
        let mut matching = event("Matching memory.", "ui", "2026-07-05T08:00:00Z");
        matching.scope = "agent".to_string();
        matching.agent_profile = Some("coder".to_string());
        matching.workflow_id = Some("workflow-1".to_string());
        matching.session_id = Some("session-1".to_string());

        let mut other_agent = matching.clone();
        other_agent.id = "mem_other_agent".to_string();
        other_agent.agent_profile = Some("reviewer".to_string());
        let mut other_workflow = matching.clone();
        other_workflow.id = "mem_other_workflow".to_string();
        other_workflow.workflow_id = Some("workflow-2".to_string());
        let mut other_session = matching.clone();
        other_session.id = "mem_other_session".to_string();
        other_session.session_id = Some("session-2".to_string());

        let report = consolidate_memories(
            &[matching.clone(), other_agent, other_workflow, other_session],
            &ConsolidationRequest {
                period_type: ConsolidationPeriod::Manual,
                period_key: Some("manual-filtered".to_string()),
                project: Some("ui".to_string()),
                agent_profile: Some("coder".to_string()),
                workflow_id: Some("workflow-1".to_string()),
                session_id: Some("session-1".to_string()),
                dry_run: false,
                force: false,
            },
        )
        .unwrap();

        assert_eq!(report.candidate_count, 1);
        assert_eq!(report.source_memory_ids, vec![matching.id]);
    }
}
