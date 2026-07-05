use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use serde::Deserialize;

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
}

impl EventStreamReader {
    pub fn new(path: PathBuf) -> Self {
        Self { path, offset: 0 }
    }

    pub fn read_new_events(&mut self) -> Result<Vec<LiveEvent>, String> {
        let Ok(mut file) = File::open(&self.path) else {
            return Ok(Vec::new());
        };
        let file_len = file.metadata().map_err(|err| err.to_string())?.len();
        if file_len < self.offset {
            self.offset = 0;
        }
        file.seek(SeekFrom::Start(self.offset))
            .map_err(|err| err.to_string())?;
        let mut appended = String::new();
        file.read_to_string(&mut appended)
            .map_err(|err| err.to_string())?;
        self.offset = file.stream_position().map_err(|err| err.to_string())?;

        let mut events = Vec::new();
        for line in appended.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<LiveEvent>(line) {
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
}
