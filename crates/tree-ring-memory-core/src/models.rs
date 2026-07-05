use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const RINGS: &[&str] = &["cambium", "outer", "inner", "heartwood", "scar", "seed"];
pub const SCOPES: &[&str] = &[
    "global", "project", "agent", "session", "workflow", "tool", "eval", "manual",
];
pub const SENSITIVITIES: &[&str] = &[
    "normal",
    "private",
    "secret",
    "health",
    "financial",
    "legal",
    "personal_identifier",
];
pub const RETENTIONS: &[&str] = &[
    "ephemeral",
    "normal",
    "durable",
    "user_pinned",
    "forget_after_date",
];

pub type TreeRingResult<T> = Result<T, TreeRingError>;

#[derive(Debug, Error)]
pub enum TreeRingError {
    #[error("{0}")]
    Validation(String),
    #[error("secret-like memory is blocked by policy")]
    SensitiveMemoryBlocked,
    #[error("storage error: {0}")]
    Storage(String),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn sqlite_error(message: impl Into<String>) -> TreeRingError {
    TreeRingError::Storage(message.into())
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)
}

pub fn generated_memory_id() -> String {
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let hex = Uuid::new_v4().simple().to_string();
    format!("mem_{timestamp}_{}", &hex[..12])
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemorySource {
    #[serde(rename = "type")]
    #[serde(default = "default_source_type")]
    pub source_type: String,
    #[serde(rename = "ref")]
    #[serde(default)]
    pub ref_: String,
    #[serde(default)]
    pub quote: String,
}

impl Default for MemorySource {
    fn default() -> Self {
        Self {
            source_type: "manual".to_string(),
            ref_: String::new(),
            quote: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryLink {
    #[serde(rename = "type")]
    pub link_type: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryReview {
    #[serde(default)]
    pub needs_review: bool,
    #[serde(default)]
    pub review_reason: Option<String>,
    #[serde(default)]
    pub reviewed_at: Option<String>,
    #[serde(default)]
    pub reviewed_by: Option<String>,
}

impl Default for MemoryReview {
    fn default() -> Self {
        Self {
            needs_review: false,
            review_reason: None,
            reviewed_at: None,
            reviewed_by: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryEvent {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub agent_profile: Option<String>,
    #[serde(default = "default_scope")]
    pub scope: String,
    #[serde(default = "default_ring")]
    pub ring: String,
    pub event_type: String,
    pub summary: String,
    #[serde(default)]
    pub details: String,
    #[serde(default)]
    pub source: MemorySource,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_score")]
    pub salience: f64,
    #[serde(default = "default_score")]
    pub confidence: f64,
    #[serde(default = "default_sensitivity")]
    pub sensitivity: String,
    #[serde(default = "default_retention")]
    pub retention: String,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub supersedes: Vec<String>,
    #[serde(default)]
    pub superseded_by: Option<String>,
    #[serde(default)]
    pub links: Vec<MemoryLink>,
    #[serde(default)]
    pub review: MemoryReview,
}

impl MemoryEvent {
    pub fn new(summary: impl Into<String>, event_type: impl Into<String>) -> TreeRingResult<Self> {
        let now = now_iso();
        let event = Self {
            id: generated_memory_id(),
            created_at: now.clone(),
            updated_at: now,
            project: None,
            agent_profile: None,
            scope: "global".to_string(),
            ring: "cambium".to_string(),
            event_type: event_type.into(),
            summary: summary.into(),
            details: String::new(),
            source: MemorySource::default(),
            tags: Vec::new(),
            salience: 0.5,
            confidence: 0.5,
            sensitivity: "normal".to_string(),
            retention: "normal".to_string(),
            expires_at: None,
            supersedes: Vec::new(),
            superseded_by: None,
            links: Vec::new(),
            review: MemoryReview::default(),
        };
        event.validated()
    }

    pub fn validated(mut self) -> TreeRingResult<Self> {
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&mut self) -> TreeRingResult<()> {
        if self.summary.trim().is_empty() {
            return Err(TreeRingError::Validation("summary is required".to_string()));
        }
        if self.event_type.trim().is_empty() {
            return Err(TreeRingError::Validation(
                "event_type is required".to_string(),
            ));
        }
        validate_member("scope", &self.scope, SCOPES)?;
        validate_member("ring", &self.ring, RINGS)?;
        validate_member("sensitivity", &self.sensitivity, SENSITIVITIES)?;
        validate_member("retention", &self.retention, RETENTIONS)?;
        self.salience = validate_score("salience", self.salience)?;
        self.confidence = validate_score("confidence", self.confidence)?;
        Ok(())
    }

    pub fn redact(&mut self) {
        self.summary = "[REDACTED]".to_string();
        self.details.clear();
        self.project = None;
        self.agent_profile = None;
        self.event_type = "redacted".to_string();
        self.tags.clear();
        self.source = MemorySource::default();
        self.supersedes.clear();
        self.superseded_by = None;
        self.links.clear();
        self.review = MemoryReview::default();
        self.sensitivity = "private".to_string();
        self.updated_at = now_iso();
    }
}

fn validate_member(field: &str, value: &str, allowed: &[&str]) -> TreeRingResult<()> {
    if allowed.contains(&value) {
        return Ok(());
    }
    Err(TreeRingError::Validation(format!(
        "invalid {field}: {value}"
    )))
}

fn default_source_type() -> String {
    "manual".to_string()
}

fn default_scope() -> String {
    "global".to_string()
}

fn default_ring() -> String {
    "cambium".to_string()
}

fn default_score() -> f64 {
    0.5
}

fn default_sensitivity() -> String {
    "normal".to_string()
}

fn default_retention() -> String {
    "normal".to_string()
}

fn validate_score(field: &str, value: f64) -> TreeRingResult<f64> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        return Ok(value);
    }
    Err(TreeRingError::Validation(format!(
        "{field} must be a finite number between 0 and 1"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_memory_event_serializes() {
        let mut event = MemoryEvent::new("Prefer local SQLite storage.", "decision").unwrap();
        event.scope = "project".to_string();
        event.ring = "heartwood".to_string();
        event.project = Some("tree-ring-memory".to_string());
        event.salience = 0.8;
        event.confidence = 0.9;
        event.validate().unwrap();

        let payload = serde_json::to_value(&event).unwrap();

        assert!(payload["id"].as_str().unwrap().starts_with("mem_"));
        assert_eq!(payload["summary"], "Prefer local SQLite storage.");
        assert_eq!(payload["ring"], "heartwood");
    }

    #[test]
    fn memory_source_uses_schema_ref_key() {
        let mut event = MemoryEvent::new("Preserve source refs.", "lesson").unwrap();
        event.source.ref_ = "README.md".to_string();

        let payload = serde_json::to_value(&event).unwrap();

        assert_eq!(payload["source"]["ref"], "README.md");
        assert!(payload["source"].get("ref_").is_none());
    }

    #[test]
    fn schema_valid_sparse_memory_defaults_like_python() {
        let payload = include_str!("../../../fixtures/parity/schema-valid-sparse-memory.json");
        let mut event: MemoryEvent = serde_json::from_str(payload).unwrap();
        event.validate().unwrap();

        assert_eq!(event.details, "");
        assert_eq!(event.source.ref_, "");
        assert_eq!(event.source.quote, "");
        assert_eq!(event.supersedes, Vec::<String>::new());
        assert_eq!(event.links, Vec::<MemoryLink>::new());
        assert!(!event.review.needs_review);
    }

    #[test]
    fn generated_id_uses_random_hex_suffix() {
        let event = MemoryEvent::new("Use collision-resistant ids.", "decision").unwrap();
        let suffix = event.id.rsplit('_').next().unwrap();

        assert_eq!(suffix.len(), 12);
        assert!(suffix.chars().all(|ch| ch.is_ascii_hexdigit()));
        assert_ne!(suffix, "000001");
    }

    #[test]
    fn invalid_ring_is_rejected() {
        let mut event = MemoryEvent::new("Bad ring.", "decision").unwrap();
        event.ring = "bark".to_string();

        let err = event.validate().unwrap_err().to_string();

        assert!(err.contains("invalid ring"));
    }

    #[test]
    fn invalid_score_is_rejected() {
        let mut event = MemoryEvent::new("Bad score.", "decision").unwrap();
        event.salience = f64::NAN;

        let err = event.validate().unwrap_err().to_string();

        assert!(err.contains("salience"));
    }

    #[test]
    fn redact_clears_sensitive_metadata() {
        let mut event = MemoryEvent::new("Secret memory.", "lesson").unwrap();
        event.project = Some("secret".to_string());
        event.source.ref_ = "secret".to_string();
        event.tags = vec!["secret".to_string()];
        event.superseded_by = Some("secret".to_string());

        event.redact();
        let serialized = serde_json::to_string(&event).unwrap();

        assert_eq!(event.summary, "[REDACTED]");
        assert_eq!(event.sensitivity, "private");
        assert!(!serialized.contains("secret"));
    }
}
