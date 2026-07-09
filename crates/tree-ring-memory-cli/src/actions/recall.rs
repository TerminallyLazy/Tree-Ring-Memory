use tree_ring_memory_sqlite::{MemoryRetriever, RecallResult, SQLiteMemoryStore};

use super::ActionResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallRequest {
    pub query: String,
    pub project: Option<String>,
    pub limit: usize,
    pub include_sensitive: bool,
    pub include_superseded: bool,
    pub explain: bool,
}

#[derive(Debug, Clone)]
pub struct RecallReport {
    pub results: Vec<RecallResult>,
}

pub fn recall(
    store: &SQLiteMemoryStore,
    request: RecallRequest,
) -> ActionResult<RecallReport> {
    let results = MemoryRetriever::new(store)
        .recall(
            &request.query,
            request.project.as_deref(),
            None,
            None,
            None,
            None,
            request.include_sensitive,
            request.include_superseded,
            request.limit,
            request.explain,
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
}
