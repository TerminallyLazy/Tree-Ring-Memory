use tree_ring_memory_core::{MemoryEvent, MemorySource, SensitivityGuard};
use tree_ring_memory_sqlite::{PutOutcome, SQLiteMemoryStore};

use super::ActionResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RememberRequest {
    pub summary: String,
    pub event_type: String,
    pub ring: String,
    pub scope: String,
    pub project: Option<String>,
    pub agent_profile: Option<String>,
    pub workflow_id: Option<String>,
    pub session_id: Option<String>,
    pub operation_id: Option<String>,
    pub source_ref: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RememberReport {
    pub memory: MemoryEvent,
    pub created: bool,
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
    .chain(request.agent_profile.iter().map(String::as_str))
    .chain(request.workflow_id.iter().map(String::as_str))
    .chain(request.session_id.iter().map(String::as_str))
    .chain(request.operation_id.iter().map(String::as_str))
    .chain(request.source_ref.iter().map(String::as_str))
    .chain(request.tags.iter().map(String::as_str));
    let detected_sensitivity = guard
        .detect_text_sensitivity(values)
        .map_err(|err| err.to_string())?;
    let mut event =
        MemoryEvent::new(request.summary, request.event_type).map_err(|err| err.to_string())?;
    event.ring = request.ring;
    event.scope = request.scope;
    event.project = request.project;
    event.agent_profile = request.agent_profile;
    event.workflow_id = request.workflow_id;
    event.session_id = request.session_id;
    event.operation_id = request.operation_id;
    if let Some(source_ref) = request.source_ref {
        event.source = MemorySource {
            source_type: "agent".to_string(),
            ref_: source_ref,
            quote: String::new(),
        };
    }
    event.tags = request.tags;
    if detected_sensitivity != "normal" {
        event.sensitivity = detected_sensitivity;
    }
    event.validate().map_err(|err| err.to_string())?;
    let (memory, created) = store_event_idempotently(store, &event)?;
    Ok(RememberReport { memory, created })
}

pub fn store_event_idempotently(
    store: &mut SQLiteMemoryStore,
    event: &MemoryEvent,
) -> ActionResult<(MemoryEvent, bool)> {
    match store.put_idempotent(event).map_err(|err| err.to_string())? {
        PutOutcome::Created => Ok((event.clone(), true)),
        PutOutcome::Existing(existing) if same_write_intent(&existing, event) => {
            Ok((existing, false))
        }
        PutOutcome::Existing(_) => Err(format!(
            "operation_id {} is already bound to a different memory write",
            event.operation_id.as_deref().unwrap_or("<missing>")
        )),
    }
}

fn same_write_intent(existing: &MemoryEvent, requested: &MemoryEvent) -> bool {
    existing.project == requested.project
        && existing.agent_profile == requested.agent_profile
        && existing.workflow_id == requested.workflow_id
        && existing.session_id == requested.session_id
        && existing.operation_id == requested.operation_id
        && existing.scope == requested.scope
        && existing.ring == requested.ring
        && existing.event_type == requested.event_type
        && existing.summary == requested.summary
        && existing.details == requested.details
        && existing.source == requested.source
        && existing.tags == requested.tags
        && existing.salience == requested.salience
        && existing.confidence == requested.confidence
        && existing.sensitivity == requested.sensitivity
        && existing.retention == requested.retention
        && existing.expires_at == requested.expires_at
        && existing.supersedes == requested.supersedes
        && existing.links == requested.links
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
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                operation_id: None,
                source_ref: None,
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
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                operation_id: None,
                source_ref: None,
                tags: Vec::new(),
            },
        )
        .unwrap();

        let stored = store.get(&report.memory.id).unwrap().unwrap();
        assert_eq!(stored.sensitivity, "health");
    }

    #[test]
    fn remember_action_round_trips_multi_agent_context_and_source() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();

        let report = remember(
            &mut store,
            RememberRequest {
                summary: "Worker found the failing boundary.".to_string(),
                event_type: "lesson".to_string(),
                ring: "cambium".to_string(),
                scope: "agent".to_string(),
                project: Some("tree-ring".to_string()),
                agent_profile: Some("reviewer-2".to_string()),
                workflow_id: Some("fanout-42".to_string()),
                session_id: Some("attempt-1".to_string()),
                operation_id: Some("finding-storage-lock".to_string()),
                source_ref: Some("runs/fanout-42/reviewer-2.json".to_string()),
                tags: vec!["storage".to_string()],
            },
        )
        .unwrap();

        assert!(report.created);
        assert_eq!(report.memory.agent_profile.as_deref(), Some("reviewer-2"));
        assert_eq!(report.memory.workflow_id.as_deref(), Some("fanout-42"));
        assert_eq!(report.memory.session_id.as_deref(), Some("attempt-1"));
        assert_eq!(
            report.memory.operation_id.as_deref(),
            Some("finding-storage-lock")
        );
        assert_eq!(report.memory.source.ref_, "runs/fanout-42/reviewer-2.json");
    }

    #[test]
    fn operation_id_replay_is_idempotent_and_conflicts_fail_closed() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let request = RememberRequest {
            summary: "One logical worker result.".to_string(),
            event_type: "lesson".to_string(),
            ring: "cambium".to_string(),
            scope: "workflow".to_string(),
            project: Some("tree-ring".to_string()),
            agent_profile: Some("worker-1".to_string()),
            workflow_id: Some("fanout-42".to_string()),
            session_id: None,
            operation_id: Some("task-7".to_string()),
            source_ref: None,
            tags: Vec::new(),
        };

        let first = remember(&mut store, request.clone()).unwrap();
        let replay = remember(&mut store, request).unwrap();
        let mut conflict_request = RememberRequest {
            summary: "Conflicting worker result.".to_string(),
            event_type: "lesson".to_string(),
            ring: "cambium".to_string(),
            scope: "workflow".to_string(),
            project: Some("tree-ring".to_string()),
            agent_profile: Some("worker-1".to_string()),
            workflow_id: Some("fanout-42".to_string()),
            session_id: None,
            operation_id: Some("task-7".to_string()),
            source_ref: None,
            tags: Vec::new(),
        };
        let conflict = remember(&mut store, conflict_request.clone()).unwrap_err();
        conflict_request.operation_id = Some("task-8".to_string());

        assert!(first.created);
        assert!(!replay.created);
        assert_eq!(first.memory.id, replay.memory.id);
        assert!(conflict.contains("already bound"));
        assert_eq!(store.list_all(true).unwrap().len(), 1);
        assert!(remember(&mut store, conflict_request).unwrap().created);
    }
}
