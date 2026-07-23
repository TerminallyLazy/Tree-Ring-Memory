use once_cell::sync::Lazy;
use regex::Regex;

use crate::models::{MemoryEvent, TreeRingError, TreeRingResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensitivityResult {
    pub sensitivity: String,
    pub redacted_text: String,
    pub findings: Vec<String>,
    pub requires_explicit_approval: bool,
}

#[derive(Debug, Clone)]
pub struct SensitivityGuard {
    block_secret_storage: bool,
}

impl Default for SensitivityGuard {
    fn default() -> Self {
        Self {
            block_secret_storage: true,
        }
    }
}

impl SensitivityGuard {
    pub fn new(block_secret_storage: bool) -> Self {
        Self {
            block_secret_storage,
        }
    }

    pub fn inspect(&self, text: &str) -> SensitivityResult {
        let mut findings = Vec::new();
        let mut redacted = text.to_string();
        for (name, pattern) in SECRET_PATTERNS.iter() {
            if pattern.is_match(&redacted) {
                findings.push((*name).to_string());
                redacted = pattern
                    .replace_all(&redacted, "[REDACTED_SECRET]")
                    .to_string();
            }
        }
        if !findings.is_empty() {
            return SensitivityResult {
                sensitivity: "secret".to_string(),
                redacted_text: redacted,
                findings,
                requires_explicit_approval: false,
            };
        }

        for (category, pattern) in CATEGORY_PATTERNS.iter() {
            if pattern.is_match(text) {
                return SensitivityResult {
                    sensitivity: (*category).to_string(),
                    redacted_text: text.to_string(),
                    findings: vec![(*category).to_string()],
                    requires_explicit_approval: true,
                };
            }
        }

        SensitivityResult {
            sensitivity: "normal".to_string(),
            redacted_text: text.to_string(),
            findings: Vec::new(),
            requires_explicit_approval: false,
        }
    }

    pub fn check_or_raise(&self, text: &str) -> TreeRingResult<SensitivityResult> {
        let result = self.inspect(text);
        if result.sensitivity == "secret" && self.block_secret_storage {
            return Err(TreeRingError::SensitiveMemoryBlocked);
        }
        Ok(result)
    }

    pub fn detect_text_sensitivity<'a, I>(&self, values: I) -> TreeRingResult<String>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut detected = "normal".to_string();
        for value in values {
            let result = self.check_or_raise(value)?;
            if detected == "normal" && result.sensitivity != "normal" {
                detected = result.sensitivity;
            }
        }
        Ok(detected)
    }

    pub fn detect_memory_event_sensitivity(&self, event: &MemoryEvent) -> TreeRingResult<String> {
        let mut detected = "normal".to_string();
        self.accumulate(&mut detected, &event.id)?;
        self.accumulate(&mut detected, &event.created_at)?;
        self.accumulate(&mut detected, &event.updated_at)?;
        self.accumulate_optional(&mut detected, event.project.as_deref())?;
        self.accumulate_optional(&mut detected, event.agent_profile.as_deref())?;
        self.accumulate_optional(&mut detected, event.workflow_id.as_deref())?;
        self.accumulate_optional(&mut detected, event.session_id.as_deref())?;
        self.accumulate_optional(&mut detected, event.operation_id.as_deref())?;
        self.accumulate(&mut detected, &event.scope)?;
        self.accumulate(&mut detected, &event.ring)?;
        self.accumulate(&mut detected, &event.event_type)?;
        self.accumulate(&mut detected, &event.summary)?;
        self.accumulate(&mut detected, &event.details)?;
        self.accumulate(&mut detected, &event.source.source_type)?;
        self.accumulate(&mut detected, &event.source.ref_)?;
        self.accumulate(&mut detected, &event.source.quote)?;
        for tag in &event.tags {
            self.accumulate(&mut detected, tag)?;
        }
        self.accumulate(&mut detected, &event.sensitivity)?;
        self.accumulate(&mut detected, &event.retention)?;
        self.accumulate_optional(&mut detected, event.expires_at.as_deref())?;
        for superseded_id in &event.supersedes {
            self.accumulate(&mut detected, superseded_id)?;
        }
        self.accumulate_optional(&mut detected, event.superseded_by.as_deref())?;
        for link in &event.links {
            self.accumulate(&mut detected, &link.link_type)?;
            self.accumulate(&mut detected, &link.target)?;
        }
        self.accumulate_optional(&mut detected, event.review.review_reason.as_deref())?;
        self.accumulate_optional(&mut detected, event.review.reviewed_at.as_deref())?;
        self.accumulate_optional(&mut detected, event.review.reviewed_by.as_deref())?;
        Ok(detected)
    }

    fn accumulate(&self, detected: &mut String, value: &str) -> TreeRingResult<()> {
        let result = self.check_or_raise(value)?;
        if *detected == "normal" && result.sensitivity != "normal" {
            *detected = result.sensitivity;
        }
        Ok(())
    }

    fn accumulate_optional(
        &self,
        detected: &mut String,
        value: Option<&str>,
    ) -> TreeRingResult<()> {
        if let Some(value) = value {
            self.accumulate(detected, value)?;
        }
        Ok(())
    }
}

static SECRET_PATTERNS: Lazy<Vec<(&'static str, Regex)>> = Lazy::new(|| {
    vec![
        ("openai_key", Regex::new(r"\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}\b").unwrap()),
        (
            "tree_ring_coordinator_capability",
            Regex::new(r"trcap_v1_[A-Fa-f0-9]{64}").unwrap(),
        ),
        ("github_token", Regex::new(r"\bgh[pousr]_[A-Za-z0-9_]{20,}\b").unwrap()),
        ("github_fine_grained_token", Regex::new(r"\bgithub_pat_[A-Za-z0-9_]{20,}\b").unwrap()),
        ("aws_access_key", Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap()),
        ("bearer_token", Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]{20,}\b").unwrap()),
        (
            "private_key",
            Regex::new(
                r"(?s)-----BEGIN (?:(?:RSA|EC|OPENSSH|DSA|ENCRYPTED) )?PRIVATE KEY-----.*?-----END (?:(?:RSA|EC|OPENSSH|DSA|ENCRYPTED) )?PRIVATE KEY-----",
            )
            .unwrap(),
        ),
        (
            "secret_assignment",
            Regex::new(
                r#"(?i)\b(?:[A-Z0-9_]*(?:API[_-]?KEY|PRIVATE[_-]?KEY|SECRET(?:_ACCESS_KEY)?|TOKEN|PASSWORD|PASSWD|PWD)|DATABASE_URL)\s*=\s*(?:"[^"\n]*"|'[^'\n]*'|\S+)"#,
            )
            .unwrap(),
        ),
    ]
});

static CATEGORY_PATTERNS: Lazy<Vec<(&'static str, Regex)>> = Lazy::new(|| {
    vec![
        (
            "health",
            Regex::new(
                r"(?i)\b(diagnosis|diagnosed|medical|medication|prescription|therapy|hospital)\b",
            )
            .unwrap(),
        ),
        (
            "financial",
            Regex::new(
                r"(?i)\b(bank account|routing number|credit card|tax return|paystub|salary)\b",
            )
            .unwrap(),
        ),
        (
            "legal",
            Regex::new(
                r"(?i)\b(lawsuit|attorney|legal advice|court order|subpoena|contract dispute)\b",
            )
            .unwrap(),
        ),
        (
            "personal_identifier",
            Regex::new(r"(?i)\b(ssn|social security|passport|driver'?s license)\b").unwrap(),
        ),
    ]
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_secret_by_default() {
        let guard = SensitivityGuard::default();

        let err = guard
            .check_or_raise("Use key sk-proj-abcdefghijklmnopqrstuvwxyz1234567890")
            .unwrap_err()
            .to_string();

        assert!(err.contains("blocked"));
    }

    #[test]
    fn blocks_tree_ring_coordinator_capabilities_by_default() {
        let guard = SensitivityGuard::default();
        let capability = format!("trcap_v1_{}", "a".repeat(64));

        for candidate in [
            capability.clone(),
            format!("worker_{capability}_suffix"),
            format!("prefix{capability}suffix"),
        ] {
            let err = guard.check_or_raise(&candidate).unwrap_err().to_string();

            assert!(err.contains("blocked"));
            assert!(!err.contains(&capability));
        }
    }

    #[test]
    fn redacts_password_assignment() {
        let guard = SensitivityGuard::new(false);
        let result = guard.inspect("password = hunter2");

        assert_eq!(result.sensitivity, "secret");
        assert!(!result.redacted_text.contains("hunter2"));
        assert!(result.redacted_text.contains("[REDACTED_SECRET]"));
    }

    #[test]
    fn detects_health_text() {
        let result = SensitivityGuard::default().inspect("User mentioned a private diagnosis.");

        assert_eq!(result.sensitivity, "health");
        assert!(result.requires_explicit_approval);
    }

    #[test]
    fn detects_category_after_checking_all_fields() {
        let guard = SensitivityGuard::default();
        let detected = guard
            .detect_text_sensitivity([
                "normal summary",
                "private diagnosis in source metadata",
                "ordinary tag",
            ])
            .unwrap();

        assert_eq!(detected, "health");
    }

    #[test]
    fn memory_event_detection_checks_full_public_schema_surface() {
        let guard = SensitivityGuard::default();
        let mut event = MemoryEvent::new("Full event policy.", "lesson").unwrap();
        event.details = "private diagnosis belongs in sensitive recall".to_string();
        event.superseded_by = Some("mem_replacement".to_string());
        event.links.push(crate::models::MemoryLink {
            link_type: "evidence".to_string(),
            target: "local".to_string(),
        });
        event.review.review_reason = Some("reviewed by maintainer".to_string());

        let detected = guard.detect_memory_event_sensitivity(&event).unwrap();

        assert_eq!(detected, "health");
    }

    #[test]
    fn memory_event_detection_blocks_secret_in_review_metadata() {
        let guard = SensitivityGuard::default();
        let mut event = MemoryEvent::new("Full event policy.", "lesson").unwrap();
        event.review.reviewed_by = Some("TOKEN=abcdefghijklmnopqrstuvwxyz123456".to_string());

        let error = guard.detect_memory_event_sensitivity(&event).unwrap_err();

        assert!(error.to_string().contains("blocked"));
    }

    #[test]
    fn memory_event_detection_checks_all_correlation_metadata() {
        let guard = SensitivityGuard::default();

        for set_sensitive_field in [
            (|event: &mut MemoryEvent| {
                event.workflow_id = Some("private diagnosis workflow".to_string())
            }) as fn(&mut MemoryEvent),
            |event: &mut MemoryEvent| {
                event.session_id = Some("private diagnosis session".to_string())
            },
            |event: &mut MemoryEvent| {
                event.operation_id = Some("private diagnosis operation".to_string())
            },
        ] {
            let mut event = MemoryEvent::new("Correlation policy.", "lesson").unwrap();
            set_sensitive_field(&mut event);

            let detected = guard.detect_memory_event_sensitivity(&event).unwrap();

            assert_eq!(detected, "health");
        }
    }
}
