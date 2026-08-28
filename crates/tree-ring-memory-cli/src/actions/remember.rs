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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureRequest {
    pub summary: String,
    pub event_type: String,
    pub ring: String,
    pub project: String,
    pub agent_profile: String,
    pub workflow_id: String,
    pub session_id: String,
    pub operation_id: String,
    pub source_ref: String,
    pub tags: Vec<String>,
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

/// Stores one strict agent-mediated automatic capture candidate.
///
/// Automatic capture deliberately cannot create shared, heartwood, evaluation,
/// or sensitive memory. The lifecycle hook supplies the trusted routing and
/// checkpoint provenance; the active agent supplies only the concise durable
/// candidate and its bounded classification.
pub fn capture(
    store: &mut SQLiteMemoryStore,
    mut request: CaptureRequest,
) -> ActionResult<RememberReport> {
    let allowed = matches!(
        (request.event_type.as_str(), request.ring.as_str()),
        ("preference" | "decision", "cambium")
            | ("lesson" | "correction", "cambium" | "scar")
            | ("warning", "scar")
            | ("seed", "seed")
    );
    if !allowed {
        if !matches!(request.ring.as_str(), "cambium" | "scar" | "seed") {
            return Err("automatic capture uses an unsupported ring".to_string());
        }
        return Err("automatic capture uses an unsupported event type or ring pairing".to_string());
    }
    let checkpoint_id = request
        .source_ref
        .strip_prefix("agent-checkpoint:")
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || "-_.".contains(character))
        })
        .ok_or_else(|| {
            "automatic capture source-ref must contain a safe agent-checkpoint id".to_string()
        })?;
    let operation_prefix = format!("auto-{checkpoint_id}-");
    let slot = request
        .operation_id
        .strip_prefix(&operation_prefix)
        .filter(|slot| matches!(*slot, "1" | "2" | "3"));
    if slot.is_none() {
        return Err(
            "automatic capture operation-id must match its checkpoint and slot 1, 2, or 3"
                .to_string(),
        );
    }

    let guard = SensitivityGuard::default();
    let values = [
        request.summary.as_str(),
        request.event_type.as_str(),
        request.ring.as_str(),
        request.project.as_str(),
        request.agent_profile.as_str(),
        request.workflow_id.as_str(),
        request.session_id.as_str(),
        request.operation_id.as_str(),
        request.source_ref.as_str(),
    ]
    .into_iter()
    .chain(request.tags.iter().map(String::as_str));
    let sensitivity = guard
        .detect_text_sensitivity(values)
        .map_err(|_| "automatic capture accepts only normal-sensitivity candidates".to_string())?;
    if sensitivity != "normal" {
        return Err("automatic capture accepts only normal-sensitivity candidates".to_string());
    }
    if !request.tags.iter().any(|tag| tag == "automatic-capture") {
        request.tags.push("automatic-capture".to_string());
    }

    remember(
        store,
        RememberRequest {
            summary: request.summary,
            event_type: request.event_type,
            ring: request.ring,
            scope: "agent".to_string(),
            project: Some(request.project),
            agent_profile: Some(request.agent_profile),
            workflow_id: Some(request.workflow_id),
            session_id: Some(request.session_id),
            operation_id: Some(request.operation_id),
            source_ref: Some(request.source_ref),
            tags: request.tags,
        },
    )
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
    fn automatic_capture_stores_only_agent_scoped_checkpoint_memory() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();

        let report = capture(
            &mut store,
            CaptureRequest {
                summary: "Use receipt-backed lifecycle hooks for automatic recall.".to_string(),
                event_type: "decision".to_string(),
                ring: "cambium".to_string(),
                project: "tree-ring-memory".to_string(),
                agent_profile: "codex".to_string(),
                workflow_id: "workflow-1".to_string(),
                session_id: "session-1".to_string(),
                operation_id: "auto-checkpoint-1-1".to_string(),
                source_ref: "agent-checkpoint:checkpoint-1".to_string(),
                tags: vec!["hooks".to_string()],
            },
        )
        .unwrap();

        assert!(report.created);
        assert_eq!(report.memory.scope, "agent");
        assert_eq!(report.memory.agent_profile.as_deref(), Some("codex"));
        assert!(report
            .memory
            .tags
            .iter()
            .any(|tag| tag == "automatic-capture"));
    }

    #[test]
    fn automatic_capture_rejects_sensitive_or_unbounded_candidates() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let request = |summary: &str, event_type: &str, ring: &str| CaptureRequest {
            summary: summary.to_string(),
            event_type: event_type.to_string(),
            ring: ring.to_string(),
            project: "tree-ring-memory".to_string(),
            agent_profile: "codex".to_string(),
            workflow_id: "workflow-1".to_string(),
            session_id: "session-1".to_string(),
            operation_id: "auto-checkpoint-1-1".to_string(),
            source_ref: "agent-checkpoint:checkpoint-1".to_string(),
            tags: Vec::new(),
        };

        assert!(capture(
            &mut store,
            request(
                "token = sk-proj-abcdefghijklmnopqrstuvwxyz1234567890",
                "lesson",
                "cambium"
            )
        )
        .unwrap_err()
        .contains("normal-sensitivity"));
        assert!(capture(
            &mut store,
            request("Promote this automatically.", "decision", "heartwood")
        )
        .unwrap_err()
        .contains("unsupported ring"));
        assert!(capture(
            &mut store,
            request("Store a transcript summary.", "transcript", "cambium")
        )
        .unwrap_err()
        .contains("unsupported event type"));
        let mut mismatched = request("Store a fourth candidate.", "lesson", "cambium");
        mismatched.operation_id = "auto-checkpoint-1-4".to_string();
        assert!(capture(&mut store, mismatched)
            .unwrap_err()
            .contains("slot 1, 2, or 3"));
        assert!(store.list_all(false).unwrap().is_empty());
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
