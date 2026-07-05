use rusqlite::{params, Connection, OptionalExtension, Row};
use std::collections::HashSet;
use std::path::Path;

use tree_ring_memory_core::models::{sqlite_error, MemoryEvent, TreeRingResult};
use tree_ring_memory_core::recall::{search_queries, RecallScorer};

#[derive(Debug, Clone)]
pub struct RecallResult {
    pub memory: MemoryEvent,
    pub score: f64,
    pub ranking: std::collections::BTreeMap<String, f64>,
}

pub struct SQLiteMemoryStore {
    connection: Connection,
}

impl SQLiteMemoryStore {
    pub fn open(path: impl AsRef<Path>) -> TreeRingResult<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| sqlite_error(err.to_string()))?;
        }
        let connection = Connection::open(path).map_err(|err| sqlite_error(err.to_string()))?;
        connection
            .busy_timeout(std::time::Duration::from_millis(30_000))
            .map_err(|err| sqlite_error(err.to_string()))?;
        connection
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=30000;")
            .map_err(|err| sqlite_error(err.to_string()))?;
        let store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn migrate(&self) -> TreeRingResult<()> {
        self.connection
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS memories (
                  id TEXT PRIMARY KEY,
                  created_at TEXT NOT NULL,
                  updated_at TEXT NOT NULL,
                  project TEXT,
                  agent_profile TEXT,
                  scope TEXT NOT NULL,
                  ring TEXT NOT NULL,
                  event_type TEXT NOT NULL,
                  summary TEXT NOT NULL,
                  details TEXT NOT NULL,
                  source_json TEXT NOT NULL,
                  tags_json TEXT NOT NULL,
                  salience REAL NOT NULL,
                  confidence REAL NOT NULL,
                  sensitivity TEXT NOT NULL,
                  retention TEXT NOT NULL,
                  expires_at TEXT,
                  supersedes_json TEXT NOT NULL,
                  superseded_by TEXT,
                  links_json TEXT NOT NULL,
                  review_json TEXT NOT NULL,
                  raw_json TEXT NOT NULL
                );
                CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
                  id UNINDEXED,
                  summary,
                  details,
                  tags,
                  source_ref
                );
                "#,
            )
            .map_err(|err| sqlite_error(err.to_string()))?;
        Ok(())
    }

    pub fn put(&mut self, event: &MemoryEvent) -> TreeRingResult<()> {
        let mut event = event.clone();
        event.validate()?;
        let source_json = serde_json::to_string(&event.source)?;
        let tags_json = serde_json::to_string(&event.tags)?;
        let supersedes_json = serde_json::to_string(&event.supersedes)?;
        let links_json = serde_json::to_string(&event.links)?;
        let review_json = serde_json::to_string(&event.review)?;
        let raw_json = serde_json::to_string(&event)?;

        let transaction = self.connection.transaction().map_err(|err| sqlite_error(err.to_string()))?;
        transaction
            .execute(
                r#"
                INSERT OR REPLACE INTO memories (
                  id, created_at, updated_at, project, agent_profile, scope, ring,
                  event_type, summary, details, source_json, tags_json, salience,
                  confidence, sensitivity, retention, expires_at, supersedes_json,
                  superseded_by, links_json, review_json, raw_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
                params![
                    event.id,
                    event.created_at,
                    event.updated_at,
                    event.project,
                    event.agent_profile,
                    event.scope,
                    event.ring,
                    event.event_type,
                    event.summary,
                    event.details,
                    source_json,
                    tags_json,
                    event.salience,
                    event.confidence,
                    event.sensitivity,
                    event.retention,
                    event.expires_at,
                    supersedes_json,
                    event.superseded_by,
                    links_json,
                    review_json,
                    raw_json,
                ],
            )
            .map_err(|err| sqlite_error(err.to_string()))?;

        transaction
            .execute("DELETE FROM memory_fts WHERE id = ?", params![event.id])
            .map_err(|err| sqlite_error(err.to_string()))?;
        transaction
            .execute(
                "INSERT INTO memory_fts (id, summary, details, tags, source_ref) VALUES (?, ?, ?, ?, ?)",
                params![
                    event.id,
                    event.summary,
                    event.details,
                    event.tags.join(" "),
                    event.source.ref_,
                ],
            )
            .map_err(|err| sqlite_error(err.to_string()))?;
        transaction.commit().map_err(|err| sqlite_error(err.to_string()))?;
        Ok(())
    }

    pub fn get(&self, memory_id: &str) -> TreeRingResult<Option<MemoryEvent>> {
        self.connection
            .query_row(
                "SELECT raw_json FROM memories WHERE id = ?",
                params![memory_id],
                event_from_row,
            )
            .optional()
            .map_err(|err| sqlite_error(err.to_string()))?
            .transpose()
    }

    pub fn list_all(&self, include_superseded: bool) -> TreeRingResult<Vec<MemoryEvent>> {
        let sql = if include_superseded {
            "SELECT raw_json FROM memories ORDER BY created_at DESC"
        } else {
            "SELECT raw_json FROM memories WHERE superseded_by IS NULL ORDER BY created_at DESC"
        };
        let mut statement = self.connection.prepare(sql).map_err(|err| sqlite_error(err.to_string()))?;
        let rows = statement
            .query_map([], event_from_row)
            .map_err(|err| sqlite_error(err.to_string()))?;
        collect_rows(rows)
    }

    pub fn search_text(&self, query: &str, include_superseded: bool) -> TreeRingResult<Vec<MemoryEvent>> {
        if query.trim().is_empty() {
            return self.list_all(include_superseded);
        }
        let Some(fts_query) = format_plain_text_fts_query(query) else {
            return Ok(Vec::new());
        };
        let sql = if include_superseded {
            r#"
            SELECT memories.raw_json
            FROM memory_fts
            JOIN memories ON memories.id = memory_fts.id
            WHERE memory_fts MATCH ?
            ORDER BY rank
            "#
        } else {
            r#"
            SELECT memories.raw_json
            FROM memory_fts
            JOIN memories ON memories.id = memory_fts.id
            WHERE memory_fts MATCH ?
              AND memories.superseded_by IS NULL
            ORDER BY rank
            "#
        };
        let mut statement = self.connection.prepare(sql).map_err(|err| sqlite_error(err.to_string()))?;
        let rows = statement
            .query_map(params![fts_query], event_from_row)
            .map_err(|err| sqlite_error(err.to_string()))?;
        collect_rows(rows)
    }

    pub fn supersede(&mut self, old_id: &str, new_id: &str) -> TreeRingResult<()> {
        let Some(mut old) = self.get(old_id)? else {
            return Ok(());
        };
        old.superseded_by = Some(new_id.to_string());
        let raw_json = serde_json::to_string(&old)?;
        self.connection
            .execute(
                "UPDATE memories SET superseded_by = ?, raw_json = ? WHERE id = ?",
                params![new_id, raw_json, old_id],
            )
            .map_err(|err| sqlite_error(err.to_string()))?;
        Ok(())
    }

    pub fn delete(&mut self, memory_id: &str) -> TreeRingResult<()> {
        let transaction = self.connection.transaction().map_err(|err| sqlite_error(err.to_string()))?;
        transaction
            .execute("DELETE FROM memories WHERE id = ?", params![memory_id])
            .map_err(|err| sqlite_error(err.to_string()))?;
        transaction
            .execute("DELETE FROM memory_fts WHERE id = ?", params![memory_id])
            .map_err(|err| sqlite_error(err.to_string()))?;
        transaction.commit().map_err(|err| sqlite_error(err.to_string()))?;
        Ok(())
    }

    pub fn redact(&mut self, memory_id: &str) -> TreeRingResult<()> {
        let Some(mut event) = self.get(memory_id)? else {
            return Ok(());
        };
        event.redact();
        self.put(&event)?;
        self.connection
            .execute(
                "UPDATE memories SET superseded_by = NULL WHERE id = ?",
                params![memory_id],
            )
            .map_err(|err| sqlite_error(err.to_string()))?;
        Ok(())
    }
}

pub struct MemoryRetriever<'a> {
    store: &'a SQLiteMemoryStore,
}

impl<'a> MemoryRetriever<'a> {
    pub fn new(store: &'a SQLiteMemoryStore) -> Self {
        Self { store }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn recall(
        &self,
        query: &str,
        project: Option<&str>,
        agent_profile: Option<&str>,
        scope: Option<&str>,
        rings: Option<&[String]>,
        event_types: Option<&[String]>,
        include_sensitive: bool,
        include_superseded: bool,
        limit: usize,
        explain_ranking: bool,
    ) -> TreeRingResult<Vec<RecallResult>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut candidates = Vec::new();
        let mut seen_queries = HashSet::new();
        for search_query in search_queries(query) {
            if !seen_queries.insert(search_query.clone()) {
                continue;
            }
            candidates = self.store.search_text(&search_query, include_superseded)?;
            if !candidates.is_empty() {
                break;
            }
        }

        let mut results: Vec<RecallResult> = candidates
            .into_iter()
            .filter(|event| matches_filters(event, project, agent_profile, scope, rings, event_types, include_sensitive))
            .map(|memory| {
                let scored = RecallScorer::score(&memory, query);
                RecallResult {
                    memory,
                    score: scored.score,
                    ranking: if explain_ranking {
                        scored.ranking.factors
                    } else {
                        Default::default()
                    },
                }
            })
            .collect();
        results.sort_by(|left, right| right.score.total_cmp(&left.score));
        results.truncate(limit);
        Ok(results)
    }
}

fn matches_filters(
    event: &MemoryEvent,
    project: Option<&str>,
    agent_profile: Option<&str>,
    scope: Option<&str>,
    rings: Option<&[String]>,
    event_types: Option<&[String]>,
    include_sensitive: bool,
) -> bool {
    if project.is_some_and(|project| event.project.as_deref() != Some(project)) {
        return false;
    }
    if agent_profile.is_some_and(|profile| event.agent_profile.as_deref() != Some(profile)) {
        return false;
    }
    if scope.is_some_and(|scope| event.scope != scope) {
        return false;
    }
    if rings.is_some_and(|rings| !rings.contains(&event.ring)) {
        return false;
    }
    if event_types.is_some_and(|event_types| !event_types.contains(&event.event_type)) {
        return false;
    }
    if !include_sensitive && event.sensitivity != "normal" {
        return false;
    }
    true
}

fn event_from_row(row: &Row<'_>) -> rusqlite::Result<TreeRingResult<MemoryEvent>> {
    let raw_json: String = row.get(0)?;
    Ok(serde_json::from_str::<MemoryEvent>(&raw_json).map_err(Into::into))
}

fn collect_rows<I>(rows: I) -> TreeRingResult<Vec<MemoryEvent>>
where
    I: IntoIterator<Item = rusqlite::Result<TreeRingResult<MemoryEvent>>>,
{
    rows.into_iter()
        .map(|row| row.map_err(|err| sqlite_error(err.to_string())).and_then(|event| event))
        .collect()
}

fn format_plain_text_fts_query(query: &str) -> Option<String> {
    let terms: Vec<String> = tree_ring_memory_core::recall::terms(query)
        .into_iter()
        .filter(|term| !SEARCH_FILLER_TERMS.contains(&term.as_str()))
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() {
        return None;
    }
    Some(terms.join(" AND "))
}

const SEARCH_FILLER_TERMS: &[&str] = &[
    "a", "an", "and", "about", "are", "for", "in", "is", "not", "of", "on", "or", "the", "to", "what",
];

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tree_ring_memory_core::models::MemorySource;

    #[test]
    fn store_inserts_and_gets_memory() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut event = MemoryEvent::new("SQLite stores portable memory.", "lesson").unwrap();
        event.scope = "project".to_string();
        event.project = Some("demo".to_string());
        event.source = MemorySource {
            source_type: "manual".to_string(),
            ref_: "test".to_string(),
            quote: String::new(),
        };

        store.put(&event).unwrap();
        let loaded = store.get(&event.id).unwrap().unwrap();

        assert_eq!(loaded.summary, "SQLite stores portable memory.");
        assert_eq!(loaded.source.ref_, "test");
    }

    #[test]
    fn store_enables_wal_and_busy_timeout() {
        let dir = tempdir().unwrap();
        let store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let journal_mode: String = store.connection().query_row("PRAGMA journal_mode", [], |row| row.get(0)).unwrap();
        let busy_timeout: i64 = store.connection().query_row("PRAGMA busy_timeout", [], |row| row.get(0)).unwrap();

        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
        assert!(busy_timeout >= 30_000);
    }

    #[test]
    fn store_searches_fts() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut scar = MemoryEvent::new("Avoid stale cache without invalidation.", "warning").unwrap();
        scar.ring = "scar".to_string();
        let mut decision = MemoryEvent::new("Use local SQLite for v0.1.", "decision").unwrap();
        decision.ring = "heartwood".to_string();
        store.put(&scar).unwrap();
        store.put(&decision).unwrap();

        let results = store.search_text("stale cache", false).unwrap();

        assert_eq!(results[0].ring, "scar");
    }

    #[test]
    fn recall_filters_sensitive_and_boosts_scars() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let normal = MemoryEvent::new("Normal memory", "lesson").unwrap();
        let mut sensitive = MemoryEvent::new("Private bank account note", "lesson").unwrap();
        sensitive.sensitivity = "financial".to_string();
        let mut scar = MemoryEvent::new("Avoid stale frontend cache.", "warning").unwrap();
        scar.ring = "scar".to_string();
        scar.confidence = 0.7;
        store.put(&normal).unwrap();
        store.put(&sensitive).unwrap();
        store.put(&scar).unwrap();

        let results = MemoryRetriever::new(&store)
            .recall("failure stale cache", None, None, None, None, None, false, false, 8, true)
            .unwrap();

        assert_eq!(results[0].memory.ring, "scar");
        assert!(!results.iter().any(|result| result.memory.sensitivity == "financial"));
    }

    #[test]
    fn redact_clears_fts_and_raw_payload() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut event = MemoryEvent::new("Legacy memory with secret metadata.", "lesson").unwrap();
        event.source.ref_ = "sk-proj-abcdefghijklmnopqrstuvwxyz1234567890".to_string();
        event.sensitivity = "secret".to_string();
        store.put(&event).unwrap();

        store.redact(&event.id).unwrap();

        let redacted = store.get(&event.id).unwrap().unwrap();
        assert_eq!(redacted.summary, "[REDACTED]");
        assert_eq!(redacted.sensitivity, "private");
        assert!(store.search_text("sk-proj-abcdefghijklmnopqrstuvwxyz1234567890", true).unwrap().is_empty());
    }

    #[test]
    fn fts_rows_match_memory_rows_after_mutations() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let old = MemoryEvent::new("Use polling.", "decision").unwrap();
        let mut new = MemoryEvent::new("Use snapshot invalidation.", "decision").unwrap();
        new.supersedes = vec![old.id.clone()];
        store.put(&old).unwrap();
        store.put(&new).unwrap();
        store.supersede(&old.id, &new.id).unwrap();
        store.redact(&old.id).unwrap();
        store.delete(&new.id).unwrap();

        let mismatch_count: i64 = store
            .connection()
            .query_row(
                r#"
                SELECT
                  (SELECT count(*) FROM memories LEFT JOIN memory_fts ON memories.id = memory_fts.id WHERE memory_fts.id IS NULL)
                  +
                  (SELECT count(*) FROM memory_fts LEFT JOIN memories ON memories.id = memory_fts.id WHERE memories.id IS NULL)
                "#,
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(mismatch_count, 0);
    }
}
