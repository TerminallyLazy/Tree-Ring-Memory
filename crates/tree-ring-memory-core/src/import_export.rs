use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::models::{now_iso, MemoryEvent, TreeRingError, TreeRingResult};
use crate::sensitivity::SensitivityGuard;

pub const EXPORT_RECORD_TYPE: &str = "tree_ring_memory_export";
pub const MEMORY_EVENT_RECORD_TYPE: &str = "memory_event";
pub const EXPORT_SCHEMA_VERSION: u32 = 1;
pub const EXPORT_PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");
const LEGACY_PRIVATE_SCOPE_IDENTITY_DOMAIN: &[u8] =
    b"tree-ring-memory/legacy-private-scope-identity/v1";
const LEGACY_PRIVATE_SCOPE_REVIEW_REASON: &str =
    "legacy private-scope identity synthesized during portability normalization";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportHeader {
    #[serde(rename = "type")]
    pub record_type: String,
    pub schema_version: u32,
    pub plugin_version: String,
    pub created_at: String,
    pub memory_count: usize,
    pub sensitive_included: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryEventEnvelope {
    #[serde(rename = "type")]
    pub record_type: String,
    pub memory: MemoryEvent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecodedJsonl {
    pub header: Option<ExportHeader>,
    pub events: Vec<MemoryEvent>,
}

pub fn encode_jsonl(events: &[MemoryEvent], sensitive_included: bool) -> TreeRingResult<String> {
    for event in events {
        event.validate()?;
    }
    let header = ExportHeader {
        record_type: EXPORT_RECORD_TYPE.to_string(),
        schema_version: EXPORT_SCHEMA_VERSION,
        plugin_version: EXPORT_PLUGIN_VERSION.to_string(),
        created_at: now_iso(),
        memory_count: events.len(),
        sensitive_included,
    };

    let mut output = String::new();
    output.push_str(&serde_json::to_string(&header)?);
    output.push('\n');
    for event in events {
        let envelope = MemoryEventEnvelope {
            record_type: MEMORY_EVENT_RECORD_TYPE.to_string(),
            memory: event.clone(),
        };
        output.push_str(&serde_json::to_string(&envelope)?);
        output.push('\n');
    }
    Ok(output)
}

pub fn decode_jsonl(input: &str) -> TreeRingResult<DecodedJsonl> {
    let mut header = None;
    let mut events = Vec::new();

    for (index, line) in input.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(trimmed).map_err(|err| {
            TreeRingError::Validation(format!("line {line_number}: invalid json: {err}"))
        })?;
        match value.get("type").and_then(Value::as_str) {
            Some(EXPORT_RECORD_TYPE) => {
                let parsed: ExportHeader = serde_json::from_value(value).map_err(|err| {
                    TreeRingError::Validation(format!(
                        "line {line_number}: invalid export header: {err}"
                    ))
                })?;
                if parsed.schema_version != EXPORT_SCHEMA_VERSION {
                    return Err(TreeRingError::Validation(format!(
                        "line {line_number}: unsupported export schema version {}",
                        parsed.schema_version
                    )));
                }
                header = Some(parsed);
            }
            Some(MEMORY_EVENT_RECORD_TYPE) => {
                let mut parsed: MemoryEventEnvelope =
                    serde_json::from_value(value).map_err(|err| {
                        TreeRingError::Validation(format!(
                            "line {line_number}: invalid memory envelope: {err}"
                        ))
                    })?;
                normalize_legacy_private_scope_identity(&mut parsed.memory).map_err(|err| {
                    TreeRingError::Validation(format!("line {line_number}: {err}"))
                })?;
                parsed.memory.validate().map_err(|err| {
                    TreeRingError::Validation(format!("line {line_number}: {err}"))
                })?;
                events.push(parsed.memory);
            }
            _ => {
                let mut parsed: MemoryEvent = serde_json::from_value(value).map_err(|err| {
                    TreeRingError::Validation(format!(
                        "line {line_number}: invalid memory event: {err}"
                    ))
                })?;
                normalize_legacy_private_scope_identity(&mut parsed).map_err(|err| {
                    TreeRingError::Validation(format!("line {line_number}: {err}"))
                })?;
                parsed.validate().map_err(|err| {
                    TreeRingError::Validation(format!("line {line_number}: {err}"))
                })?;
                events.push(parsed);
            }
        }
    }

    Ok(DecodedJsonl { header, events })
}

pub fn normalize_import_events(events: Vec<MemoryEvent>) -> TreeRingResult<Vec<MemoryEvent>> {
    events.into_iter().map(normalize_import_event).collect()
}

pub fn normalize_import_event(mut event: MemoryEvent) -> TreeRingResult<MemoryEvent> {
    normalize_legacy_private_scope_identity(&mut event)?;
    let detected = SensitivityGuard::default().detect_memory_event_sensitivity(&event)?;
    if event.sensitivity == "normal" && detected != "normal" {
        event.sensitivity = detected;
    }
    event.validate()?;
    Ok(event)
}

/// Restores the partition identity required by current private-scope events.
///
/// Tree Ring Memory versions before 0.12 allowed `agent`, `workflow`, and
/// `session` records without their corresponding identity. Portability and
/// storage-migration boundaries may call this helper before strict validation.
/// Ordinary event construction remains strict: [`MemoryEvent::validate`] still
/// rejects an identity-less private scope.
///
/// The generated label is deterministic per scope and record ID, contains no
/// original identifier text, and is deliberately marked for human review.
/// Returns `true` only when a synthetic identity was added.
pub fn normalize_legacy_private_scope_identity(event: &mut MemoryEvent) -> TreeRingResult<bool> {
    let is_missing = |value: &Option<String>| {
        value
            .as_deref()
            .is_none_or(|identity| identity.trim().is_empty())
    };
    let missing_identity = match event.scope.as_str() {
        "agent" => is_missing(&event.agent_profile),
        "workflow" => is_missing(&event.workflow_id),
        "session" => is_missing(&event.session_id),
        _ => false,
    };
    if !missing_identity {
        return Ok(false);
    }
    if event.id.trim().is_empty() {
        return Err(TreeRingError::Validation(
            "legacy private-scope normalization requires a non-blank memory id".to_string(),
        ));
    }

    let identity = synthetic_legacy_private_scope_identity(&event.scope, &event.id);
    match event.scope.as_str() {
        "agent" => event.agent_profile = Some(identity),
        "workflow" => event.workflow_id = Some(identity),
        "session" => event.session_id = Some(identity),
        _ => unreachable!("missing_identity is only true for a private scope"),
    }
    event.review.needs_review = true;
    event.review.reviewed_at = None;
    event.review.reviewed_by = None;
    match event.review.review_reason.as_mut() {
        Some(reason) if !reason.contains(LEGACY_PRIVATE_SCOPE_REVIEW_REASON) => {
            reason.push_str("; ");
            reason.push_str(LEGACY_PRIVATE_SCOPE_REVIEW_REASON);
        }
        Some(_) => {}
        None => {
            event.review.review_reason = Some(LEGACY_PRIVATE_SCOPE_REVIEW_REASON.to_string());
        }
    }
    Ok(true)
}

fn synthetic_legacy_private_scope_identity(scope: &str, memory_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(LEGACY_PRIVATE_SCOPE_IDENTITY_DOMAIN);
    hasher.update([0]);
    hasher.update(scope.as_bytes());
    hasher.update([0]);
    hasher.update(memory_id.as_bytes());
    let digest = hasher.finalize();
    let compact_digest: String = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("legacy-{scope}-{compact_digest}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_header_and_event_envelopes() {
        let event = MemoryEvent::new("Export this memory.", "lesson").unwrap();

        let jsonl = encode_jsonl(std::slice::from_ref(&event), false).unwrap();
        let lines: Vec<_> = jsonl.lines().collect();

        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains(EXPORT_RECORD_TYPE));
        assert!(lines[0].contains("\"plugin_version\""));
        assert!(lines[1].contains(MEMORY_EVENT_RECORD_TYPE));
        assert!(lines[1].contains(&event.id));
    }

    #[test]
    fn decodes_export_envelope_jsonl() {
        let event = MemoryEvent::new("Import this memory.", "lesson").unwrap();
        let jsonl = encode_jsonl(std::slice::from_ref(&event), false).unwrap();

        let decoded = decode_jsonl(&jsonl).unwrap();

        assert_eq!(decoded.header.unwrap().memory_count, 1);
        assert_eq!(decoded.events, vec![event]);
    }

    #[test]
    fn decodes_raw_memory_event_lines_for_compatibility() {
        let event = MemoryEvent::new("Raw event import.", "lesson").unwrap();
        let raw = serde_json::to_string(&event).unwrap();

        let decoded = decode_jsonl(&raw).unwrap();

        assert_eq!(decoded.events, vec![event]);
    }

    #[test]
    fn decodes_and_round_trips_legacy_private_scopes_from_raw_and_enveloped_jsonl() {
        let raw = include_str!("../../../fixtures/parity/legacy-private-scopes-raw.jsonl");
        let enveloped = include_str!("../../../fixtures/parity/legacy-private-scopes-export.jsonl");

        let raw_events = decode_jsonl(raw).unwrap().events;
        let enveloped_events = decode_jsonl(enveloped).unwrap().events;

        assert_eq!(raw_events, enveloped_events);
        assert_eq!(raw_events.len(), 3);
        for event in &raw_events {
            let identity = match event.scope.as_str() {
                "agent" => event.agent_profile.as_deref(),
                "workflow" => event.workflow_id.as_deref(),
                "session" => event.session_id.as_deref(),
                scope => panic!("unexpected scope {scope}"),
            }
            .unwrap();
            assert!(identity.starts_with(&format!("legacy-{}-", event.scope)));
            assert_eq!(identity.len(), "legacy--".len() + event.scope.len() + 32);
            assert!(event.review.needs_review);
            assert_eq!(
                event.review.review_reason.as_deref(),
                Some(LEGACY_PRIVATE_SCOPE_REVIEW_REASON)
            );
            event.validate().unwrap();
        }

        let exported = encode_jsonl(&raw_events, false).unwrap();
        let round_tripped = decode_jsonl(&exported).unwrap().events;
        assert_eq!(round_tripped, raw_events);
    }

    #[test]
    fn legacy_private_scope_normalization_is_stable_per_record_and_scope() {
        let mut first = MemoryEvent::new("Legacy record one.", "lesson").unwrap();
        first.id = "mem_legacy_record_1".to_string();
        first.scope = "agent".to_string();
        let mut repeated = first.clone();
        let mut second = first.clone();
        second.id = "mem_legacy_record_2".to_string();
        let mut workflow = first.clone();
        workflow.scope = "workflow".to_string();

        assert!(normalize_legacy_private_scope_identity(&mut first).unwrap());
        assert!(normalize_legacy_private_scope_identity(&mut repeated).unwrap());
        assert!(normalize_legacy_private_scope_identity(&mut second).unwrap());
        assert!(normalize_legacy_private_scope_identity(&mut workflow).unwrap());

        assert_eq!(first.agent_profile, repeated.agent_profile);
        assert_ne!(first.agent_profile, second.agent_profile);
        assert_ne!(first.agent_profile, workflow.workflow_id);
        assert!(!normalize_legacy_private_scope_identity(&mut first).unwrap());
    }

    #[test]
    fn blank_legacy_private_scope_identity_is_normalized_from_fixture() {
        let input =
            include_str!("../../../fixtures/parity/legacy-private-scope-blank-identity.jsonl");

        let events = decode_jsonl(input).unwrap().events;

        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert!(event
            .agent_profile
            .as_deref()
            .is_some_and(|identity| identity.starts_with("legacy-agent-")));
        assert!(event.review.needs_review);
        event.validate().unwrap();
    }

    #[test]
    fn explicit_import_normalization_accepts_a_deserialized_legacy_record() {
        let raw = include_str!("../../../fixtures/parity/legacy-private-scopes-raw.jsonl")
            .lines()
            .next()
            .unwrap();
        let legacy: MemoryEvent = serde_json::from_str(raw).unwrap();

        assert!(legacy
            .validate()
            .unwrap_err()
            .to_string()
            .contains("agent_profile"));

        let normalized = normalize_import_event(legacy).unwrap();

        assert!(normalized.agent_profile.is_some());
        assert!(normalized.review.needs_review);
        normalized.validate().unwrap();
    }

    #[test]
    fn portability_normalization_fails_closed_for_blank_record_id() {
        let mut event = MemoryEvent::new("Malformed legacy record.", "lesson").unwrap();
        event.id = "  ".to_string();
        event.scope = "session".to_string();

        let error = normalize_legacy_private_scope_identity(&mut event)
            .unwrap_err()
            .to_string();

        assert!(error.contains("non-blank memory id"));
        assert_eq!(event.session_id, None);
        assert!(!event.review.needs_review);
    }

    #[test]
    fn direct_export_keeps_strict_validation_for_new_identity_less_events() {
        let mut event = MemoryEvent::new("Invalid new private record.", "lesson").unwrap();
        event.scope = "workflow".to_string();

        let error = encode_jsonl(&[event], false).unwrap_err().to_string();

        assert!(error.contains("workflow_id"));
    }

    #[test]
    fn ignores_blank_lines() {
        let decoded = decode_jsonl("\n\n  \n").unwrap();

        assert!(decoded.events.is_empty());
        assert!(decoded.header.is_none());
    }

    #[test]
    fn returns_line_number_for_invalid_json() {
        let err = decode_jsonl("\nnot-json").unwrap_err().to_string();

        assert!(err.contains("line 2"));
        assert!(err.contains("invalid json"));
    }

    #[test]
    fn returns_line_number_for_invalid_memory() {
        let err = decode_jsonl(r#"{"type":"memory_event","memory":{"summary":""}}"#)
            .unwrap_err()
            .to_string();

        assert!(err.contains("line 1"));
    }

    #[test]
    fn normalize_import_event_upgrades_misclassified_sensitive_memory() {
        let mut event =
            MemoryEvent::new("Private diagnosis should not be normal.", "lesson").unwrap();
        event.sensitivity = "normal".to_string();

        let normalized = normalize_import_event(event).unwrap();

        assert_eq!(normalized.sensitivity, "health");
    }

    #[test]
    fn normalize_import_event_blocks_secrets() {
        let event = MemoryEvent::new(
            "Imported key sk-proj-abcdefghijklmnopqrstuvwxyz1234567890 must fail.",
            "lesson",
        )
        .unwrap();

        let err = normalize_import_event(event).unwrap_err().to_string();

        assert!(err.contains("blocked"));
    }
}
