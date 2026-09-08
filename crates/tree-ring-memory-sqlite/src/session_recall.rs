use std::time::{Duration, Instant};

use rusqlite::{params_from_iter, types::Value};
use tree_ring_memory_core::{models::TreeRingResult, recall::search_queries, RecallScorer};

use crate::{
    ensure_recall_deadline, format_plain_text_fts_or_query, format_plain_text_fts_query, search,
    sqlite_error_from_rusqlite, MemoryRetriever, RecallResult,
};

/// Visibility for an automatic session brief. Correlation fields on shared or
/// agent-scoped memories describe their origin; they are not lifetime limits.
/// Ordinary `RecallOptions` continue to provide exact, conjunctive filtering.
#[derive(Debug, Clone, Copy)]
pub struct SessionRecallScope<'a> {
    pub project: &'a str,
    /// The lifecycle parser's safe project-name alias, for captured memories.
    pub project_alias: Option<&'a str>,
    pub agent_profile: &'a str,
    pub workflow_id: &'a str,
    pub session_id: &'a str,
}

impl MemoryRetriever<'_> {
    /// Recalls shared project context, this agent's durable memories, and only
    /// the matching workflow/session partitions. A missing query selects a
    /// bounded startup brief without requiring invented search keywords.
    pub fn recall_for_session_timeout(
        &self,
        query: Option<&str>,
        scope: &SessionRecallScope<'_>,
        limit: usize,
        timeout: Duration,
    ) -> TreeRingResult<Vec<RecallResult>> {
        self.with_recall_timeout(timeout, |deadline| {
            let query = query.map(str::trim).filter(|query| !query.is_empty());
            if let Some(query) = query {
                for search_query in search_queries(query) {
                    if let Some(fts) = format_plain_text_fts_query(&search_query) {
                        let results =
                            self.session_candidates(Some(&fts), query, scope, limit, deadline)?;
                        if !results.is_empty() {
                            return Ok(results);
                        }
                    }
                }
                if let Some(fts) = format_plain_text_fts_or_query(query) {
                    return self.session_candidates(Some(&fts), query, scope, limit, deadline);
                }
                return Ok(Vec::new());
            }
            self.session_candidates(None, "", scope, limit, deadline)
        })
    }

    fn session_candidates(
        &self,
        fts: Option<&str>,
        query: &str,
        scope: &SessionRecallScope<'_>,
        limit: usize,
        deadline: Instant,
    ) -> TreeRingResult<Vec<RecallResult>> {
        ensure_recall_deadline(Some(deadline))?;
        if limit == 0 {
            return Ok(Vec::new());
        }
        let mut sql = String::from("SELECT memories.raw_json FROM memories ");
        let mut parameters = Vec::new();
        if let Some(fts) = fts {
            sql.push_str(
                "JOIN memory_fts ON memories.id = memory_fts.id WHERE memory_fts MATCH ? AND ",
            );
            parameters.push(Value::Text(fts.to_string()));
        } else {
            sql.push_str("WHERE ");
        }
        // Apply every visibility/lifecycle predicate before the candidate cap.
        // No unscoped global memories or private memories from other workers
        // may crowd out the current project's context.
        sql.push_str(
            "memories.project IN (?, ?) AND memories.sensitivity = 'normal'
             AND memories.superseded_by IS NULL
             AND (memories.expires_at IS NULL OR julianday(memories.expires_at) > julianday('now'))
             AND NOT EXISTS (SELECT 1 FROM redaction_tombstones WHERE memory_id = memories.id)
             AND (
                memories.scope IN ('global', 'project', 'tool', 'eval', 'manual', 'dox', 'revolve')
                OR (memories.scope = 'agent' AND memories.agent_profile = ?)
                OR (memories.scope = 'workflow' AND memories.workflow_id = ?)
                OR (memories.scope = 'session' AND memories.session_id = ?)
             ) ",
        );
        parameters.extend(
            [
                scope.project,
                scope.project_alias.unwrap_or(scope.project),
                scope.agent_profile,
                scope.workflow_id,
                scope.session_id,
            ]
            .map(|value| Value::Text(value.to_string())),
        );
        if fts.is_some() {
            sql.push_str("ORDER BY rank, memories.id ");
        } else {
            sql.push_str(
                "ORDER BY CASE memories.ring WHEN 'scar' THEN 0 WHEN 'heartwood' THEN 1 ELSE 2 END,
                 memories.salience DESC, memories.confidence DESC, memories.created_at DESC, memories.id ",
            );
        }
        sql.push_str("LIMIT ?");
        parameters.push(Value::Integer(
            limit.saturating_mul(128).clamp(256, 2048) as i64
        ));
        let mut statement = self
            .store
            .connection
            .prepare(&sql)
            .map_err(sqlite_error_from_rusqlite)?;
        let rows = statement
            .query_map(params_from_iter(parameters), search::event_from_row)
            .map_err(sqlite_error_from_rusqlite)?;
        let mut results = Vec::new();
        for row in rows {
            ensure_recall_deadline(Some(deadline))?;
            let memory = row.map_err(sqlite_error_from_rusqlite)??;
            let mut score = RecallScorer::score(&memory, query).score;
            if fts.is_none() {
                score += match memory.ring.as_str() {
                    "scar" => 0.2,
                    "heartwood" => 0.15,
                    _ => 0.0,
                };
            }
            results.push(RecallResult {
                memory,
                score,
                ranking: Default::default(),
            });
        }
        ensure_recall_deadline(Some(deadline))?;
        results.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.memory.id.cmp(&right.memory.id))
        });
        results.truncate(limit);
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RecallOptions, SQLiteMemoryStore};
    use tree_ring_memory_core::MemoryEvent;

    fn scope() -> SessionRecallScope<'static> {
        SessionRecallScope {
            project: "project",
            project_alias: Some("project-alias"),
            agent_profile: "codex",
            workflow_id: "new-workflow",
            session_id: "new-session",
        }
    }

    fn memory(id: &str, scope: &str) -> MemoryEvent {
        let mut event = MemoryEvent::new("Use reversible database migrations", "lesson").unwrap();
        event.id = id.to_string();
        event.project = Some("project".to_string());
        event.scope = scope.to_string();
        event.agent_profile = Some("codex".to_string());
        event.workflow_id = Some("old-workflow".to_string());
        event.session_id = Some("old-session".to_string());
        event
    }

    #[test]
    fn new_session_recalls_durable_and_shared_memory_but_respects_private_scopes() {
        let mut store = SQLiteMemoryStore::open(":memory:").unwrap();
        for shared in [
            "project", "global", "manual", "dox", "revolve", "tool", "eval",
        ] {
            let mut event = memory(shared, shared);
            event.agent_profile = Some("other-agent".to_string());
            store.put(&event).unwrap();
        }
        store.put(&memory("durable-agent", "agent")).unwrap();
        store.put(&memory("old-workflow", "workflow")).unwrap();
        store.put(&memory("old-session", "session")).unwrap();
        let mut other_agent = memory("other-agent", "agent");
        other_agent.agent_profile = Some("other-agent".to_string());
        store.put(&other_agent).unwrap();
        let mut workflow = memory("current-workflow", "workflow");
        workflow.workflow_id = Some("new-workflow".to_string());
        workflow.agent_profile = Some("peer-worker".to_string());
        store.put(&workflow).unwrap();
        let mut session = memory("current-session", "session");
        session.session_id = Some("new-session".to_string());
        store.put(&session).unwrap();
        let mut alias = memory("safe-project-alias", "agent");
        alias.project = Some("project-alias".to_string());
        store.put(&alias).unwrap();

        for query in [None, Some("database migrations")] {
            let results = MemoryRetriever::new(&store)
                .recall_for_session_timeout(query, &scope(), 20, Duration::from_secs(1))
                .unwrap();
            let ids: std::collections::BTreeSet<_> =
                results.iter().map(|r| r.memory.id.as_str()).collect();
            assert_eq!(
                ids,
                [
                    "project",
                    "global",
                    "manual",
                    "dox",
                    "revolve",
                    "tool",
                    "eval",
                    "durable-agent",
                    "current-workflow",
                    "current-session",
                    "safe-project-alias"
                ]
                .into_iter()
                .collect()
            );
        }
        // Explicit CLI/API filters still mean exactly what the caller requested.
        let explicit = MemoryRetriever::new(&store)
            .recall_with_options(
                "database migrations",
                &RecallOptions {
                    agent_profile: Some("codex"),
                    session_id: Some("new-session"),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(explicit.len(), 1);
        assert_eq!(explicit[0].memory.id, "current-session");
    }

    #[test]
    fn invisible_memory_cannot_starve_the_brief_before_its_candidate_limit() {
        let mut store = SQLiteMemoryStore::open(":memory:").unwrap();
        let visible = memory("visible", "agent");
        store.put(&visible).unwrap();
        for i in 0..300 {
            let mut hidden = memory(&format!("hidden-{i}"), "agent");
            match i % 6 {
                0 => hidden.project = Some("other-project".to_string()),
                1 => hidden.agent_profile = Some("other-agent".to_string()),
                2 => hidden.sensitivity = "health".to_string(),
                3 => hidden.expires_at = Some("2000-01-01T00:00:00Z".to_string()),
                4 => hidden.superseded_by = Some(visible.id.clone()),
                _ => {
                    hidden.scope = "global".to_string();
                    hidden.project = None;
                }
            }
            hidden.salience = 1.0;
            store.put(&hidden).unwrap();
        }
        let redacted = memory("redacted", "agent");
        store.put(&redacted).unwrap();
        store
            .connection
            .execute(
                "INSERT INTO redaction_tombstones (memory_id) VALUES (?)",
                [&redacted.id],
            )
            .unwrap();
        for query in [None, Some("database migrations")] {
            let results = MemoryRetriever::new(&store)
                .recall_for_session_timeout(query, &scope(), 1, Duration::from_secs(1))
                .unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].memory.id, "visible");
        }
    }

    #[test]
    fn startup_prioritizes_scars_and_heartwood_without_magic_keywords() {
        let mut store = SQLiteMemoryStore::open(":memory:").unwrap();
        for (id, ring) in [
            ("a-recent", "cambium"),
            ("z-scar", "scar"),
            ("y-heartwood", "heartwood"),
        ] {
            let mut event = memory(id, "project");
            event.ring = ring.to_string();
            store.put(&event).unwrap();
        }
        let results = MemoryRetriever::new(&store)
            .recall_for_session_timeout(None, &scope(), 2, Duration::from_secs(1))
            .unwrap();
        assert_eq!(
            results
                .iter()
                .map(|r| r.memory.id.as_str())
                .collect::<Vec<_>>(),
            ["z-scar", "y-heartwood"]
        );
        assert!(MemoryRetriever::new(&store)
            .recall_for_session_timeout(
                Some("nonexistent zebras"),
                &scope(),
                8,
                Duration::from_secs(1)
            )
            .unwrap()
            .is_empty());
        assert!(MemoryRetriever::new(&store)
            .recall_for_session_timeout(None, &scope(), 8, Duration::ZERO)
            .is_err());
    }
}
