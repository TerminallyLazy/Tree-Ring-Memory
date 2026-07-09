use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::{now_iso, MemoryEvent, TreeRingError, TreeRingResult};
use crate::sensitivity::SensitivityGuard;

pub const EXPORT_RECORD_TYPE: &str = "tree_ring_memory_export";
pub const MEMORY_EVENT_RECORD_TYPE: &str = "memory_event";
pub const EXPORT_SCHEMA_VERSION: u32 = 1;
pub const EXPORT_PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

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
                let parsed: MemoryEventEnvelope = serde_json::from_value(value).map_err(|err| {
                    TreeRingError::Validation(format!(
                        "line {line_number}: invalid memory envelope: {err}"
                    ))
                })?;
                parsed.memory.validate().map_err(|err| {
                    TreeRingError::Validation(format!("line {line_number}: {err}"))
                })?;
                events.push(parsed.memory);
            }
            _ => {
                let parsed: MemoryEvent = serde_json::from_value(value).map_err(|err| {
                    TreeRingError::Validation(format!(
                        "line {line_number}: invalid memory event: {err}"
                    ))
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
    let detected = SensitivityGuard::default().detect_memory_event_sensitivity(&event)?;
    if event.sensitivity == "normal" && detected != "normal" {
        event.sensitivity = detected;
    }
    event.validate()?;
    Ok(event)
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
