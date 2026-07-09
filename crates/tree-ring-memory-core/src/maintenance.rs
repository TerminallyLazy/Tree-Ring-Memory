use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use crate::models::MemoryEvent;
use crate::sensitivity::SensitivityGuard;

/// Planner request for Rust-owned maintenance.
///
/// Core only plans actions. The apply flags are retained for storage executors
/// such as SQLite, which may interpret them when applying a returned plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceRequest {
    #[serde(default = "default_true")]
    pub dry_run: bool,
    #[serde(default)]
    pub apply_expired: bool,
    #[serde(default)]
    pub apply_secret_redactions: bool,
    #[serde(default)]
    pub repair_fts: bool,
    #[serde(default)]
    pub include_superseded: bool,
    #[serde(default)]
    pub project: Option<String>,
}

impl Default for MaintenanceRequest {
    fn default() -> Self {
        Self {
            dry_run: true,
            apply_expired: false,
            apply_secret_redactions: false,
            repair_fts: false,
            include_superseded: false,
            project: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceActionType {
    DeleteExpired,
    RedactSecret,
    ReviewExpiredProtected,
    ReviewInvalidExpiry,
}

impl MaintenanceActionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DeleteExpired => "delete_expired",
            Self::RedactSecret => "redact_secret",
            Self::ReviewExpiredProtected => "review_expired_protected",
            Self::ReviewInvalidExpiry => "review_invalid_expiry",
        }
    }
}

impl fmt::Display for MaintenanceActionType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<&str> for MaintenanceActionType {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceSeverity {
    Critical,
    High,
    Medium,
    Low,
}

impl MaintenanceSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

impl fmt::Display for MaintenanceSeverity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<&str> for MaintenanceSeverity {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceAction {
    pub action_type: MaintenanceActionType,
    pub memory_id: String,
    pub severity: MaintenanceSeverity,
    pub reason: String,
    pub applied: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceFtsReport {
    pub memory_rows: usize,
    pub fts_rows: usize,
    pub missing_fts_rows: usize,
    pub orphan_fts_rows: usize,
    pub repaired: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceReport {
    pub id: String,
    pub generated_at: String,
    pub memory_count: usize,
    pub planned_action_count: usize,
    pub applied_action_count: usize,
    pub dry_run: bool,
    pub status: String,
    pub actions: Vec<MaintenanceAction>,
    pub fts: MaintenanceFtsReport,
}

/// Build a maintenance plan without mutating memory.
///
/// This pure core planner always returns planned actions with `applied=false`.
/// Storage executors are responsible for interpreting apply flags and marking
/// actions applied after durable mutations succeed.
pub fn plan_maintenance(events: &[MemoryEvent], request: &MaintenanceRequest) -> MaintenanceReport {
    let now = Utc::now();
    let id_suffix = generated_maintenance_id_suffix();
    plan_maintenance_at(events, request, now, &id_suffix)
}

fn plan_maintenance_at(
    events: &[MemoryEvent],
    request: &MaintenanceRequest,
    now: DateTime<Utc>,
    id_suffix: &str,
) -> MaintenanceReport {
    let guard = SensitivityGuard::new(false);
    let candidates: Vec<_> = events
        .iter()
        .filter(|event| {
            request
                .project
                .as_ref()
                .is_none_or(|project| event.project.as_ref() == Some(project))
        })
        .filter(|event| request.include_superseded || event.superseded_by.is_none())
        .collect();

    let mut actions = Vec::new();
    for event in &candidates {
        plan_expiry(event, now, &mut actions);
        plan_secret_redaction(event, &guard, &mut actions);
    }

    MaintenanceReport {
        id: generated_maintenance_id(now, id_suffix),
        generated_at: now.to_rfc3339_opts(SecondsFormat::Micros, true),
        memory_count: candidates.len(),
        planned_action_count: actions.len(),
        applied_action_count: 0,
        dry_run: request.dry_run,
        status: if actions.is_empty() {
            "clean".to_string()
        } else {
            "planned".to_string()
        },
        actions,
        fts: MaintenanceFtsReport::default(),
    }
}

fn plan_expiry(event: &MemoryEvent, now: DateTime<Utc>, actions: &mut Vec<MaintenanceAction>) {
    let Some(expires_at) = event.expires_at.as_deref() else {
        return;
    };
    let Ok(expires_at) = DateTime::parse_from_rfc3339(expires_at) else {
        actions.push(action(
            MaintenanceActionType::ReviewInvalidExpiry,
            event,
            MaintenanceSeverity::Medium,
            "Memory has an invalid expires_at timestamp and requires review.",
        ));
        return;
    };
    if expires_at.with_timezone(&Utc) > now {
        return;
    }
    if protected_memory(event) {
        actions.push(action(
            MaintenanceActionType::ReviewExpiredProtected,
            event,
            MaintenanceSeverity::High,
            "Expired protected memory requires manual review before mutation.",
        ));
        return;
    }
    if matches!(event.retention.as_str(), "ephemeral" | "forget_after_date") {
        actions.push(action(
            MaintenanceActionType::DeleteExpired,
            event,
            MaintenanceSeverity::Medium,
            "Expired memory with temporary retention is eligible for deletion.",
        ));
    }
}

fn plan_secret_redaction(
    event: &MemoryEvent,
    guard: &SensitivityGuard,
    actions: &mut Vec<MaintenanceAction>,
) {
    let detected = guard
        .detect_memory_event_sensitivity(event)
        .unwrap_or_else(|_| "secret".to_string());
    if event.sensitivity == "secret" || detected == "secret" {
        actions.push(action(
            MaintenanceActionType::RedactSecret,
            event,
            MaintenanceSeverity::Critical,
            "Secret-like memory content was detected and should be redacted.",
        ));
    }
}

fn protected_memory(event: &MemoryEvent) -> bool {
    matches!(event.ring.as_str(), "scar" | "heartwood")
        || matches!(event.retention.as_str(), "durable" | "user_pinned")
}

fn action(
    action_type: MaintenanceActionType,
    event: &MemoryEvent,
    severity: MaintenanceSeverity,
    reason: &str,
) -> MaintenanceAction {
    MaintenanceAction {
        action_type,
        memory_id: event.id.clone(),
        severity,
        reason: reason.to_string(),
        applied: false,
    }
}

fn generated_maintenance_id(now: DateTime<Utc>, id_suffix: &str) -> String {
    let timestamp = now.format("%Y%m%d_%H%M%S");
    format!("maint_{timestamp}_{id_suffix}")
}

fn generated_maintenance_id_suffix() -> String {
    let hex = Uuid::new_v4().simple().to_string();
    hex[..12].to_string()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn default_request_is_safe_dry_run() {
        let request = MaintenanceRequest::default();

        assert!(request.dry_run);
        assert!(!request.apply_expired);
        assert!(!request.apply_secret_redactions);
        assert!(!request.repair_fts);
        assert!(!request.include_superseded);
        assert_eq!(request.project, None);
    }

    #[test]
    fn fts_report_uses_core_placeholders_with_spec_fields() {
        let report = plan_maintenance(
            &[],
            &MaintenanceRequest {
                repair_fts: true,
                ..MaintenanceRequest::default()
            },
        );
        let json = serde_json::to_value(&report).unwrap();

        assert_eq!(
            report.fts,
            MaintenanceFtsReport {
                memory_rows: 0,
                fts_rows: 0,
                missing_fts_rows: 0,
                orphan_fts_rows: 0,
                repaired: false,
            }
        );
        assert_eq!(json["fts"]["memory_rows"], 0);
        assert_eq!(json["fts"]["fts_rows"], 0);
        assert_eq!(json["fts"]["missing_fts_rows"], 0);
        assert_eq!(json["fts"]["orphan_fts_rows"], 0);
        assert_eq!(json["fts"]["repaired"], false);
        assert!(json["fts"].get("repair_requested").is_none());
        assert!(json["fts"].get("planned_repair").is_none());
        assert!(json["fts"].get("applied").is_none());
        assert!(json["fts"].get("reason").is_none());
    }

    #[test]
    fn core_planner_does_not_apply_actions_when_apply_flags_are_enabled() {
        let event = expired_event("Temporary cache", "ephemeral", "cambium");
        let request = MaintenanceRequest {
            dry_run: false,
            apply_expired: true,
            apply_secret_redactions: true,
            repair_fts: true,
            ..MaintenanceRequest::default()
        };

        let report = plan_maintenance(&[event], &request);

        assert_eq!(report.status, "planned");
        assert_eq!(report.applied_action_count, 0);
        assert!(report.actions.iter().all(|action| !action.applied));
        assert!(!report.dry_run);
    }

    #[test]
    fn internal_planner_uses_one_now_for_cutoff_generated_at_and_id() {
        let now = fixed_now();
        let mut event = MemoryEvent::new("Temporary cache", "lesson").unwrap();
        event.retention = "ephemeral".to_string();
        event.expires_at = Some(now.to_rfc3339_opts(SecondsFormat::Micros, true));

        let report = plan_maintenance_at(
            &[event],
            &MaintenanceRequest::default(),
            now,
            "abc123def456",
        );

        assert_eq!(report.generated_at, "2026-07-05T12:34:56.789012Z");
        assert_eq!(report.id, "maint_20260705_123456_abc123def456");
        assert_eq!(report.status, "planned");
        assert_eq!(report.planned_action_count, 1);
    }

    #[test]
    fn expired_ephemeral_and_forget_after_date_plan_delete_expired() {
        let ephemeral = expired_event("Temporary cache", "ephemeral", "cambium");
        let forget_after_date = expired_event("TTL note", "forget_after_date", "outer");

        let report = plan_maintenance(
            &[ephemeral.clone(), forget_after_date.clone()],
            &MaintenanceRequest::default(),
        );

        assert_eq!(report.status, "planned");
        assert_eq!(report.planned_action_count, 2);
        assert_action(&report, &ephemeral.id, MaintenanceActionType::DeleteExpired);
        assert_action(
            &report,
            &forget_after_date.id,
            MaintenanceActionType::DeleteExpired,
        );
    }

    #[test]
    fn expired_protected_memories_plan_review_expired_protected() {
        let scar = expired_event("Protected scar", "ephemeral", "scar");
        let heartwood = expired_event("Protected truth", "ephemeral", "heartwood");
        let durable = expired_event("Durable memory", "durable", "cambium");
        let user_pinned = expired_event("Pinned memory", "user_pinned", "outer");

        let report = plan_maintenance(
            &[
                scar.clone(),
                heartwood.clone(),
                durable.clone(),
                user_pinned.clone(),
            ],
            &MaintenanceRequest::default(),
        );

        assert_eq!(report.planned_action_count, 4);
        for event in [scar, heartwood, durable, user_pinned] {
            assert_action(
                &report,
                &event.id,
                MaintenanceActionType::ReviewExpiredProtected,
            );
        }
        assert!(report
            .actions
            .iter()
            .all(|action| { action.action_type != MaintenanceActionType::DeleteExpired }));
    }

    #[test]
    fn invalid_expires_at_plans_review_invalid_expiry() {
        let mut event = MemoryEvent::new("Invalid expiry", "lesson").unwrap();
        event.expires_at = Some("not-a-date".to_string());

        let report = plan_maintenance(&[event.clone()], &MaintenanceRequest::default());

        assert_eq!(report.planned_action_count, 1);
        assert_action(
            &report,
            &event.id,
            MaintenanceActionType::ReviewInvalidExpiry,
        );
    }

    #[test]
    fn secret_like_normal_labeled_event_plans_redaction_without_leaking_reason() {
        let raw_secret = "sk-proj-abcdefghijklmnopqrstuvwxyz1234567890";
        let mut event = MemoryEvent::new("Normal labeled memory", "lesson").unwrap();
        event.sensitivity = "normal".to_string();
        event.details = format!("Configure with {raw_secret}");

        let report = plan_maintenance(&[event.clone()], &MaintenanceRequest::default());

        assert_eq!(report.planned_action_count, 1);
        assert_action(&report, &event.id, MaintenanceActionType::RedactSecret);
        assert!(!report.actions[0].reason.contains(raw_secret));
        assert!(!serde_json::to_string(&report).unwrap().contains(raw_secret));
    }

    #[test]
    fn project_filter_excludes_other_projects() {
        let mut included = expired_event("UI cache", "ephemeral", "cambium");
        included.project = Some("ui".to_string());
        let mut excluded = expired_event("CLI cache", "ephemeral", "cambium");
        excluded.project = Some("cli".to_string());
        let request = MaintenanceRequest {
            project: Some("ui".to_string()),
            ..MaintenanceRequest::default()
        };

        let report = plan_maintenance(&[included.clone(), excluded], &request);

        assert_eq!(report.memory_count, 1);
        assert_eq!(report.planned_action_count, 1);
        assert_eq!(report.actions[0].memory_id, included.id);
    }

    #[test]
    fn include_superseded_false_excludes_superseded_rows_and_true_includes_them() {
        let active = expired_event("Active memory", "ephemeral", "cambium");
        let mut superseded = expired_event("Old memory", "ephemeral", "cambium");
        superseded.superseded_by = Some(active.id.clone());

        let default_report = plan_maintenance(
            &[active.clone(), superseded.clone()],
            &MaintenanceRequest::default(),
        );
        let included_report = plan_maintenance(
            &[active.clone(), superseded.clone()],
            &MaintenanceRequest {
                include_superseded: true,
                ..MaintenanceRequest::default()
            },
        );

        assert_eq!(default_report.memory_count, 1);
        assert_eq!(default_report.planned_action_count, 1);
        assert_eq!(default_report.actions[0].memory_id, active.id);
        assert_eq!(included_report.memory_count, 2);
        assert_eq!(included_report.planned_action_count, 2);
        assert!(included_report
            .actions
            .iter()
            .any(|action| action.memory_id == superseded.id));
    }

    fn expired_event(summary: &str, retention: &str, ring: &str) -> MemoryEvent {
        let mut event = MemoryEvent::new(summary, "lesson").unwrap();
        event.retention = retention.to_string();
        event.ring = ring.to_string();
        event.expires_at = Some((Utc::now() - Duration::days(1)).to_rfc3339());
        event
    }

    fn fixed_now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-07-05T12:34:56.789012Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn assert_action(
        report: &MaintenanceReport,
        memory_id: &str,
        action_type: MaintenanceActionType,
    ) {
        assert!(report.actions.iter().any(|action| {
            action.memory_id == memory_id && action.action_type == action_type && !action.applied
        }));
    }
}
