use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use serde::Deserialize;

const MAX_EVENT_LINE_BYTES: usize = 256 * 1024;
const MAX_READ_BATCH_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct LiveEvent {
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub ring: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub agent_profile: Option<String>,
    #[serde(default)]
    pub workflow_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub count_delta: Option<i64>,
    #[serde(default)]
    pub label: Option<String>,
}

impl LiveEvent {
    pub fn safe_label(&self) -> String {
        self.label
            .as_deref()
            .unwrap_or(&self.event)
            .chars()
            .filter(|character| !character.is_control())
            .take(96)
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct EventStreamReader {
    path: PathBuf,
    offset: u64,
    pending: Vec<u8>,
    discarding_oversized_line: bool,
}

impl EventStreamReader {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            offset: 0,
            pending: Vec::new(),
            discarding_oversized_line: false,
        }
    }

    pub fn read_new_events(&mut self) -> Result<Vec<LiveEvent>, String> {
        let Ok(mut file) = File::open(&self.path) else {
            return Ok(Vec::new());
        };
        let file_len = file.metadata().map_err(|err| err.to_string())?.len();
        if file_len < self.offset {
            self.offset = 0;
            self.pending.clear();
            self.discarding_oversized_line = false;
        }
        file.seek(SeekFrom::Start(self.offset))
            .map_err(|err| err.to_string())?;
        let mut appended = Vec::new();
        file.take(MAX_READ_BATCH_BYTES)
            .read_to_end(&mut appended)
            .map_err(|err| err.to_string())?;
        self.offset += appended.len() as u64;

        let appended = if self.discarding_oversized_line {
            let Some(newline) = appended.iter().position(|byte| *byte == b'\n') else {
                return Ok(Vec::new());
            };
            self.discarding_oversized_line = false;
            &appended[newline + 1..]
        } else {
            &appended
        };
        self.pending.extend_from_slice(appended);

        let Some(last_newline) = self.pending.iter().rposition(|byte| *byte == b'\n') else {
            if self.pending.len() > MAX_EVENT_LINE_BYTES {
                self.pending.clear();
                self.discarding_oversized_line = true;
            }
            return Ok(Vec::new());
        };
        let remainder = self.pending.split_off(last_newline + 1);
        let complete = std::mem::replace(&mut self.pending, remainder);
        if self.pending.len() > MAX_EVENT_LINE_BYTES {
            self.pending.clear();
            self.discarding_oversized_line = true;
        }
        let mut events = Vec::new();
        for line in complete.split(|byte| *byte == b'\n') {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.len() > MAX_EVENT_LINE_BYTES || line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            if let Ok(event) = serde_json::from_slice::<LiveEvent>(line) {
                events.push(event);
            }
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn reads_appended_events_incrementally() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"event":"remembered","ring":"cambium","label":"Stored preference"}}"#
        )
        .unwrap();
        let mut reader = EventStreamReader::new(file.path().to_path_buf());

        let first = reader.read_new_events().unwrap();
        let second = reader.read_new_events().unwrap();

        assert_eq!(first.len(), 1);
        assert_eq!(first[0].ring.as_deref(), Some("cambium"));
        assert!(second.is_empty());
    }

    #[test]
    fn ignores_malformed_lines_without_leaking_payload() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "not-json").unwrap();
        writeln!(
            file,
            r#"{{"event":"policy_blocked","label":"secret blocked"}}"#
        )
        .unwrap();
        let mut reader = EventStreamReader::new(file.path().to_path_buf());

        let events = reader.read_new_events().unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].safe_label(), "secret blocked");
    }

    #[test]
    fn retains_partial_final_line_until_a_writer_finishes_it() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            r#"{{"event":"remembered","ring":"cambium","label":"worker "#
        )
        .unwrap();
        file.flush().unwrap();
        let mut reader = EventStreamReader::new(file.path().to_path_buf());

        assert!(reader.read_new_events().unwrap().is_empty());

        writeln!(file, r#"résumé"}}"#).unwrap();
        file.flush().unwrap();
        let events = reader.read_new_events().unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].safe_label(), "worker résumé");
        assert!(reader.read_new_events().unwrap().is_empty());
    }

    #[test]
    fn bounds_partial_line_memory_and_recovers_at_the_next_event() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&vec![b'x'; MAX_EVENT_LINE_BYTES + 1])
            .unwrap();
        file.flush().unwrap();
        let mut reader = EventStreamReader::new(file.path().to_path_buf());

        assert!(reader.read_new_events().unwrap().is_empty());
        assert!(reader.pending.is_empty());
        assert!(reader.discarding_oversized_line);

        writeln!(file).unwrap();
        writeln!(
            file,
            r#"{{"event":"remembered","ring":"cambium","label":"recovered"}}"#
        )
        .unwrap();
        file.flush().unwrap();
        let events = reader.read_new_events().unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].safe_label(), "recovered");
        assert!(!reader.discarding_oversized_line);
    }
}
