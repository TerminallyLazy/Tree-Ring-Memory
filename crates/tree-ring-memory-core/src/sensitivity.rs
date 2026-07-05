use once_cell::sync::Lazy;
use regex::Regex;

use crate::models::{TreeRingError, TreeRingResult};

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
}

static SECRET_PATTERNS: Lazy<Vec<(&'static str, Regex)>> = Lazy::new(|| {
    vec![
        ("openai_key", Regex::new(r"\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}\b").unwrap()),
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
}
