use tree_ring_memory_core::{MemoryEvent, SensitivityGuard};
use tree_ring_memory_sqlite::SQLiteMemoryStore;

use super::ActionResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RememberRequest {
    pub summary: String,
    pub event_type: String,
    pub ring: String,
    pub scope: String,
    pub project: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RememberReport {
    pub memory: MemoryEvent,
}

pub fn remember(
    store: &mut SQLiteMemoryStore,
    request: RememberRequest,
) -> ActionResult<RememberReport> {
    let guard = SensitivityGuard::default();
    let values = [
        request.summary.as_str(),
        request.event_type.as_str(),
        request.ring.as_str(),
        request.scope.as_str(),
    ]
    .into_iter()
    .chain(request.project.iter().map(String::as_str))
    .chain(request.tags.iter().map(String::as_str));
    let detected_sensitivity = guard
        .detect_text_sensitivity(values)
        .map_err(|err| err.to_string())?;
    let mut event =
        MemoryEvent::new(request.summary, request.event_type).map_err(|err| err.to_string())?;
    event.ring = request.ring;
    event.scope = request.scope;
    event.project = request.project;
    event.tags = request.tags;
    if detected_sensitivity != "normal" {
        event.sensitivity = detected_sensitivity;
    }
    event.validate().map_err(|err| err.to_string())?;
    store.put(&event).map_err(|err| err.to_string())?;
    Ok(RememberReport { memory: event })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn remember_action_stores_memory_with_cli_defaults() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();

        let report = remember(
            &mut store,
            RememberRequest {
                summary: "Use shared actions for durable operations.".to_string(),
                event_type: "lesson".to_string(),
                ring: "cambium".to_string(),
                scope: "project".to_string(),
                project: Some("tree-ring".to_string()),
                tags: vec!["refactor".to_string()],
            },
        )
        .unwrap();

        let stored = store.get(&report.memory.id).unwrap().unwrap();
        assert_eq!(stored.summary, "Use shared actions for durable operations.");
        assert_eq!(stored.ring, "cambium");
        assert_eq!(stored.scope, "project");
        assert_eq!(stored.project.as_deref(), Some("tree-ring"));
        assert_eq!(stored.tags, vec!["refactor"]);
    }

    #[test]
    fn remember_action_classifies_sensitive_input_before_storage() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();

        let report = remember(
            &mut store,
            RememberRequest {
                summary: "Private diagnosis should be guarded.".to_string(),
                event_type: "lesson".to_string(),
                ring: "cambium".to_string(),
                scope: "global".to_string(),
                project: None,
                tags: Vec::new(),
            },
        )
        .unwrap();

        let stored = store.get(&report.memory.id).unwrap().unwrap();
        assert_eq!(stored.sensitivity, "health");
    }
}
