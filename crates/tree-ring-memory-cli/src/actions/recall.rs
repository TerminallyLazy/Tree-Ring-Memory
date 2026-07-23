use tree_ring_memory_sqlite::{MemoryRetriever, RecallOptions, RecallResult, SQLiteMemoryStore};

use super::ActionResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallRequest {
    pub query: String,
    pub project: Option<String>,
    pub agent_profile: Option<String>,
    pub workflow_id: Option<String>,
    pub session_id: Option<String>,
    pub scope: Option<String>,
    pub limit: usize,
    pub include_sensitive: bool,
    pub include_superseded: bool,
    pub explain: bool,
}

#[derive(Debug, Clone)]
pub struct RecallReport {
    pub results: Vec<RecallResult>,
}

pub fn recall(store: &SQLiteMemoryStore, request: RecallRequest) -> ActionResult<RecallReport> {
    let results = MemoryRetriever::new(store)
        .recall_with_options(
            &request.query,
            &RecallOptions {
                project: request.project.as_deref(),
                agent_profile: request.agent_profile.as_deref(),
                workflow_id: request.workflow_id.as_deref(),
                session_id: request.session_id.as_deref(),
                scope: request.scope.as_deref(),
                rings: None,
                event_types: None,
                include_sensitive: request.include_sensitive,
                include_superseded: request.include_superseded,
                limit: request.limit,
                explain_ranking: request.explain,
            },
        )
        .map_err(|err| err.to_string())?;
    Ok(RecallReport { results })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use tree_ring_memory_core::MemoryEvent;

    use super::*;

    #[test]
    fn recall_action_returns_ranked_memories() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let event = MemoryEvent::new("Use shared recall action.", "lesson").unwrap();
        store.put(&event).unwrap();

        let report = recall(
            &store,
            RecallRequest {
                query: "shared recall".to_string(),
                project: None,
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                scope: None,
                limit: 8,
                include_sensitive: false,
                include_superseded: false,
                explain: true,
            },
        )
        .unwrap();

        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].memory.id, event.id);
        assert!(report.results[0].ranking.contains_key("textual_match"));
    }

    #[test]
    fn recall_action_filters_multi_agent_context_before_ranking() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        for (agent, workflow, session) in [
            ("researcher", "fanout-7", "attempt-1"),
            ("reviewer", "fanout-7", "attempt-1"),
            ("researcher", "fanout-8", "attempt-2"),
        ] {
            let mut event = MemoryEvent::new("Shared phrase from worker.", "lesson").unwrap();
            event.scope = "agent".to_string();
            event.agent_profile = Some(agent.to_string());
            event.workflow_id = Some(workflow.to_string());
            event.session_id = Some(session.to_string());
            store.put(&event).unwrap();
        }

        let report = recall(
            &store,
            RecallRequest {
                query: "shared phrase worker".to_string(),
                project: None,
                agent_profile: Some("researcher".to_string()),
                workflow_id: Some("fanout-7".to_string()),
                session_id: Some("attempt-1".to_string()),
                scope: Some("agent".to_string()),
                limit: 8,
                include_sensitive: false,
                include_superseded: false,
                explain: false,
            },
        )
        .unwrap();

        assert_eq!(report.results.len(), 1);
        assert_eq!(
            report.results[0].memory.agent_profile.as_deref(),
            Some("researcher")
        );
    }
}
