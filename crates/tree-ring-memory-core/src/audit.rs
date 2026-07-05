use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use crate::models::{now_iso, MemoryEvent, TreeRingError, TreeRingResult};

pub const AUDIT_TYPES: &[&str] = &[
    "all",
    "stale",
    "sensitive",
    "low_confidence",
    "supersession",
    "contradictions",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditType {
    All,
    Stale,
    Sensitive,
    LowConfidence,
    Supersession,
    Contradictions,
}

impl AuditType {
    pub fn parse(value: &str) -> TreeRingResult<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "all" => Ok(Self::All),
            "stale" => Ok(Self::Stale),
            "sensitive" => Ok(Self::Sensitive),
            "low_confidence" => Ok(Self::LowConfidence),
            "supersession" => Ok(Self::Supersession),
            "contradictions" => Ok(Self::Contradictions),
            other => Err(TreeRingError::Validation(format!(
                "unsupported audit_type: {other}"
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Stale => "stale",
            Self::Sensitive => "sensitive",
            Self::LowConfidence => "low_confidence",
            Self::Supersession => "supersession",
            Self::Contradictions => "contradictions",
        }
    }

    pub fn includes(&self, audit_type: AuditType) -> bool {
        matches!(self, Self::All) || self == &audit_type
    }
}

impl fmt::Display for AuditType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<&str> for AuditType {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditSeverity {
    Critical,
    High,
    Medium,
    Low,
}

impl AuditSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

impl fmt::Display for AuditSeverity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<&str> for AuditSeverity {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditFinding {
    pub audit_type: AuditType,
    pub severity: AuditSeverity,
    pub memory_id: Option<String>,
    pub related_memory_id: Option<String>,
    pub finding: String,
    pub recommended_action: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditReport {
    pub generated_at: String,
    pub audit_type: AuditType,
    pub memory_count: usize,
    pub finding_count: usize,
    pub findings: Vec<AuditFinding>,
}

pub fn audit_memories(events: &[MemoryEvent], audit_type: &str) -> TreeRingResult<AuditReport> {
    let audit_type = AuditType::parse(audit_type)?;
    let mut findings = Vec::new();
    if audit_type.includes(AuditType::Stale) {
        audit_stale(events, &mut findings);
    }
    if audit_type.includes(AuditType::Sensitive) {
        audit_sensitive(events, &mut findings);
    }
    if audit_type.includes(AuditType::LowConfidence) {
        audit_low_confidence(events, &mut findings);
    }
    if audit_type.includes(AuditType::Supersession) {
        audit_supersession(events, &mut findings);
    }
    if audit_type.includes(AuditType::Contradictions) {
        audit_contradictions(events, &mut findings);
    }

    Ok(AuditReport {
        generated_at: now_iso(),
        audit_type,
        memory_count: events.len(),
        finding_count: findings.len(),
        findings,
    })
}

fn audit_stale(events: &[MemoryEvent], findings: &mut Vec<AuditFinding>) {
    let now = Utc::now();
    for event in events {
        let Some(expires_at) = event.expires_at.as_deref() else {
            continue;
        };
        let Ok(expires_at) = DateTime::parse_from_rfc3339(expires_at) else {
            findings.push(finding(
                AuditType::Stale,
                AuditSeverity::Medium,
                Some(&event.id),
                None,
                "Memory has an invalid expires_at timestamp.",
                "Review the memory and set a valid ISO-8601 expires_at value or redact it.",
                ["retention", "expiry"],
            ));
            continue;
        };
        if expires_at.with_timezone(&Utc) <= now {
            findings.push(finding(
                AuditType::Stale,
                AuditSeverity::Medium,
                Some(&event.id),
                None,
                "Memory expires_at is in the past.",
                "Review, delete, redact, or refresh this memory.",
                ["retention", "expiry"],
            ));
        }
    }
}

fn audit_sensitive(events: &[MemoryEvent], findings: &mut Vec<AuditFinding>) {
    for event in events {
        if event.sensitivity == "normal" {
            continue;
        }
        if event.sensitivity == "secret" {
            findings.push(finding(
                AuditType::Sensitive,
                AuditSeverity::Critical,
                Some(&event.id),
                None,
                "Secret-like memory is retained.",
                "Redact or delete this memory.",
                ["privacy", "secret"],
            ));
        }
        if matches!(event.retention.as_str(), "durable" | "user_pinned") {
            findings.push(finding(
                AuditType::Sensitive,
                AuditSeverity::High,
                Some(&event.id),
                None,
                "Sensitive memory has durable retention.",
                "Review whether this memory should be redacted or assigned an expiry.",
                ["privacy", "retention"],
            ));
        }
        if event.expires_at.is_none() {
            findings.push(finding(
                AuditType::Sensitive,
                AuditSeverity::Medium,
                Some(&event.id),
                None,
                "Sensitive memory is retained without an expiry.",
                "Set expires_at, redact, or delete this memory.",
                ["privacy", "expiry"],
            ));
        }
    }
}

fn audit_low_confidence(events: &[MemoryEvent], findings: &mut Vec<AuditFinding>) {
    for event in events {
        if event.ring == "heartwood" && event.confidence < 0.75 {
            findings.push(finding(
                AuditType::LowConfidence,
                AuditSeverity::High,
                Some(&event.id),
                None,
                "Heartwood memory has low confidence.",
                "Review evidence before treating this as durable truth.",
                ["confidence", "heartwood"],
            ));
        }
        if matches!(event.retention.as_str(), "durable" | "user_pinned") && event.confidence < 0.5 {
            findings.push(finding(
                AuditType::LowConfidence,
                AuditSeverity::Medium,
                Some(&event.id),
                None,
                "Durable memory has very low confidence.",
                "Review, demote, or supersede this memory.",
                ["confidence", "retention"],
            ));
        }
    }
}

fn audit_supersession(events: &[MemoryEvent], findings: &mut Vec<AuditFinding>) {
    let by_id: HashMap<_, _> = events
        .iter()
        .map(|event| (event.id.as_str(), event))
        .collect();
    let ids: HashSet<_> = by_id.keys().copied().collect();

    for event in events {
        if event.supersedes.iter().any(|old_id| old_id == &event.id) {
            findings.push(finding(
                AuditType::Supersession,
                AuditSeverity::High,
                Some(&event.id),
                None,
                "Memory supersedes itself.",
                "Remove the self-supersession link.",
                ["supersession", "integrity"],
            ));
        }
        if let Some(new_id) = event.superseded_by.as_deref() {
            if !ids.contains(new_id) {
                findings.push(finding(
                    AuditType::Supersession,
                    AuditSeverity::High,
                    Some(&event.id),
                    Some(new_id),
                    "Memory points to a missing superseded_by target.",
                    "Import, restore, or clear the missing supersession target.",
                    ["supersession", "integrity"],
                ));
            }
        }
        for old_id in &event.supersedes {
            let Some(old) = by_id.get(old_id.as_str()) else {
                findings.push(finding(
                    AuditType::Supersession,
                    AuditSeverity::Medium,
                    Some(&event.id),
                    Some(old_id),
                    "Memory supersedes a missing memory.",
                    "Import, restore, or remove the missing supersedes reference.",
                    ["supersession", "integrity"],
                ));
                continue;
            };
            if old.superseded_by.as_deref() != Some(event.id.as_str()) {
                findings.push(finding(
                    AuditType::Supersession,
                    AuditSeverity::Medium,
                    Some(&event.id),
                    Some(old_id),
                    "Supersedes link is missing a reciprocal superseded_by pointer.",
                    "Repair the supersession chain.",
                    ["supersession", "integrity"],
                ));
            }
        }
    }
}

fn audit_contradictions(events: &[MemoryEvent], findings: &mut Vec<AuditFinding>) {
    let mut buckets: BTreeMap<ContradictionKey, ContradictionBucket<'_>> = BTreeMap::new();
    for event in events {
        let Some((action, subject)) = directive(&event.summary) else {
            continue;
        };
        for tag in &event.tags {
            let key = ContradictionKey {
                project: event.project.clone(),
                scope: event.scope.clone(),
                event_type: event.event_type.clone(),
                tag: tag.clone(),
                subject: subject.clone(),
            };
            let bucket = buckets.entry(key).or_default();
            if action == "use" {
                bucket.uses.push(event);
            } else {
                bucket.avoids.push(event);
            }
        }
    }

    let mut emitted_pairs = HashSet::new();
    for bucket in buckets.values() {
        for use_memory in &bucket.uses {
            for avoid_memory in &bucket.avoids {
                let pair_key = ordered_pair_key(&use_memory.id, &avoid_memory.id);
                if !emitted_pairs.insert(pair_key) {
                    continue;
                }
                findings.push(finding(
                    AuditType::Contradictions,
                    AuditSeverity::Medium,
                    Some(&use_memory.id),
                    Some(&avoid_memory.id),
                    "Memories contain contradictory use/avoid guidance.",
                    "Review the pair and supersede the stale or incorrect memory.",
                    ["contradiction", "review"],
                ));
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ContradictionKey {
    project: Option<String>,
    scope: String,
    event_type: String,
    tag: String,
    subject: String,
}

#[derive(Debug, Default)]
struct ContradictionBucket<'a> {
    uses: Vec<&'a MemoryEvent>,
    avoids: Vec<&'a MemoryEvent>,
}

fn ordered_pair_key(left: &str, right: &str) -> (String, String) {
    if left <= right {
        (left.to_string(), right.to_string())
    } else {
        (right.to_string(), left.to_string())
    }
}

fn directive(summary: &str) -> Option<(&'static str, String)> {
    let normalized = normalize(summary);
    if let Some(rest) = normalized.strip_prefix("use ") {
        return Some(("use", rest.to_string()));
    }
    if let Some(rest) = normalized.strip_prefix("avoid ") {
        return Some(("avoid", rest.to_string()));
    }
    None
}

fn normalize(value: &str) -> String {
    let mut normalized = String::new();
    for ch in value.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            normalized.extend(ch.to_lowercase());
        } else {
            normalized.push(' ');
        }
    }
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn finding(
    audit_type: AuditType,
    severity: AuditSeverity,
    memory_id: Option<&str>,
    related_memory_id: Option<&str>,
    finding_text: &str,
    recommended_action: &str,
    tags: impl IntoIterator<Item = &'static str>,
) -> AuditFinding {
    AuditFinding {
        audit_type,
        severity,
        memory_id: memory_id.map(ToOwned::to_owned),
        related_memory_id: related_memory_id.map(ToOwned::to_owned),
        finding: finding_text.to_string(),
        recommended_action: recommended_action.to_string(),
        tags: tags.into_iter().map(ToOwned::to_owned).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn audit_finds_expired_memory() {
        let mut event = MemoryEvent::new("Old temporary memory.", "lesson").unwrap();
        event.expires_at = Some((Utc::now() - Duration::days(1)).to_rfc3339());

        let report = audit_memories(&[event], "stale").unwrap();

        assert_eq!(report.audit_type, "stale");
        assert_eq!(report.finding_count, 1);
        assert_eq!(report.findings[0].audit_type, "stale");
    }

    #[test]
    fn audit_finds_sensitive_retention_risks() {
        let mut event = MemoryEvent::new("Private diagnosis should expire.", "lesson").unwrap();
        event.sensitivity = "health".to_string();
        event.retention = "durable".to_string();

        let report = audit_memories(&[event], "sensitive").unwrap();
        let severities: Vec<_> = report
            .findings
            .iter()
            .map(|finding| finding.severity.as_str())
            .collect();

        assert!(severities.contains(&"high"));
        assert!(severities.contains(&"medium"));
    }

    #[test]
    fn audit_finds_low_confidence_heartwood() {
        let mut event = MemoryEvent::new("Durable but weak truth.", "decision").unwrap();
        event.ring = "heartwood".to_string();
        event.confidence = 0.4;

        let report = audit_memories(&[event], "low_confidence").unwrap();

        assert_eq!(report.findings[0].severity, "high");
    }

    #[test]
    fn audit_finds_supersession_gaps() {
        let mut event = MemoryEvent::new("Replacement decision.", "decision").unwrap();
        event.supersedes = vec!["mem_missing".to_string()];

        let report = audit_memories(&[event], "supersession").unwrap();

        assert_eq!(
            report.findings[0].related_memory_id.as_deref(),
            Some("mem_missing")
        );
    }

    #[test]
    fn audit_finds_missing_reciprocal_supersession() {
        let old = MemoryEvent::new("Use polling.", "decision").unwrap();
        let mut new = MemoryEvent::new("Use snapshot invalidation.", "decision").unwrap();
        new.supersedes = vec![old.id.clone()];

        let report = audit_memories(&[old.clone(), new.clone()], "supersession").unwrap();

        assert_eq!(report.finding_count, 1);
        assert_eq!(
            report.findings[0].memory_id.as_deref(),
            Some(new.id.as_str())
        );
        assert_eq!(
            report.findings[0].related_memory_id.as_deref(),
            Some(old.id.as_str())
        );
    }

    #[test]
    fn audit_finds_conservative_contradiction_candidates() {
        let mut use_memory = MemoryEvent::new("Use cache invalidation", "decision").unwrap();
        let mut avoid_memory = MemoryEvent::new("Avoid cache invalidation.", "decision").unwrap();
        use_memory.project = Some("ui".to_string());
        avoid_memory.project = Some("ui".to_string());
        use_memory.tags = vec!["cache".to_string()];
        avoid_memory.tags = vec!["cache".to_string()];

        let report = audit_memories(&[use_memory, avoid_memory], "contradictions").unwrap();

        assert_eq!(report.finding_count, 1);
        assert_eq!(report.findings[0].audit_type, "contradictions");
    }

    #[test]
    fn contradiction_normalization_preserves_unicode_and_underscores_like_python() {
        let mut use_memory = MemoryEvent::new("Use résumé_cache_key", "decision").unwrap();
        let mut avoid_memory = MemoryEvent::new("Avoid résumé_cache_key.", "decision").unwrap();
        use_memory.project = Some("ui".to_string());
        avoid_memory.project = Some("ui".to_string());
        use_memory.tags = vec!["cache".to_string()];
        avoid_memory.tags = vec!["cache".to_string()];

        let report = audit_memories(&[use_memory, avoid_memory], "contradictions").unwrap();

        assert_eq!(report.finding_count, 1);
    }

    #[test]
    fn audit_all_combines_checks() {
        let mut event = MemoryEvent::new("Private diagnosis should expire.", "lesson").unwrap();
        event.sensitivity = "health".to_string();
        event.retention = "durable".to_string();
        event.expires_at = Some((Utc::now() - Duration::days(1)).to_rfc3339());

        let report = audit_memories(&[event], "all").unwrap();

        assert!(report.finding_count >= 2);
    }

    #[test]
    fn audit_rejects_unknown_type() {
        let err = audit_memories(&[], "unknown").unwrap_err().to_string();

        assert!(err.contains("unsupported audit_type"));
    }

    #[test]
    fn audit_type_parsing_is_case_insensitive_and_trimmed() {
        let report = audit_memories(&[], " Sensitive ").unwrap();

        assert_eq!(report.audit_type, "sensitive");
    }

    #[test]
    fn audit_report_serializes_typed_enums_as_stable_strings() {
        let mut event = MemoryEvent::new("Private diagnosis should expire.", "lesson").unwrap();
        event.sensitivity = "secret".to_string();

        let report = audit_memories(&[event], "sensitive").unwrap();
        let json = serde_json::to_value(report).unwrap();

        assert_eq!(json["audit_type"], "sensitive");
        assert_eq!(json["findings"][0]["audit_type"], "sensitive");
        assert_eq!(json["findings"][0]["severity"], "critical");
    }
}
