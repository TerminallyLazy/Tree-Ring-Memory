use rusqlite::{
    params, params_from_iter, types::Value, Connection, ErrorCode, OptionalExtension, Row,
    Transaction,
};
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use tree_ring_memory_core::models::{sqlite_error, MemoryEvent, TreeRingError, TreeRingResult};
use tree_ring_memory_core::recall::{search_queries, RecallScorer};
use tree_ring_memory_core::{decode_jsonl, encode_jsonl, normalize_import_events};

const WRITE_RETRY_ATTEMPTS: usize = 8;
const WRITE_RETRY_INITIAL_DELAY_MS: u64 = 5;
const WRITE_RETRY_MAX_DELAY_MS: u64 = 100;

#[derive(Debug, Clone)]
pub struct RecallResult {
    pub memory: MemoryEvent,
    pub score: f64,
    pub ranking: std::collections::BTreeMap<String, f64>,
}

pub struct SQLiteMemoryStore {
    connection: Connection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReport {
    pub memory_count: usize,
    pub sensitive_included: bool,
    pub superseded_included: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub valid_count: usize,
    pub inserted_count: usize,
    pub replaced_count: usize,
    pub skipped_duplicate_count: usize,
    pub dry_run: bool,
}

impl SQLiteMemoryStore {
    pub fn open(path: impl AsRef<Path>) -> TreeRingResult<Self> {
        let path = path.as_ref();
        if let Some(parent) = parent_dir_to_create(path) {
            std::fs::create_dir_all(parent).map_err(|err| sqlite_error(err.to_string()))?;
        }
        let connection = Connection::open(path).map_err(sqlite_error_from_rusqlite)?;
        connection
            .busy_timeout(std::time::Duration::from_millis(30_000))
            .map_err(sqlite_error_from_rusqlite)?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=30000;",
            )
            .map_err(sqlite_error_from_rusqlite)?;
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
            .map_err(sqlite_error_from_rusqlite)?;
        Ok(())
    }

    pub fn put(&mut self, event: &MemoryEvent) -> TreeRingResult<()> {
        retry_locked(|| {
            let transaction = self
                .connection
                .transaction()
                .map_err(sqlite_error_from_rusqlite)?;
            put_in_transaction(&transaction, event)?;
            transaction.commit().map_err(sqlite_error_from_rusqlite)?;
            Ok(())
        })
    }

    pub fn put_many(&mut self, events: &[MemoryEvent]) -> TreeRingResult<()> {
        retry_locked(|| {
            let transaction = self
                .connection
                .transaction()
                .map_err(sqlite_error_from_rusqlite)?;
            {
                let mut insert_memory = transaction
                    .prepare(
                        r#"
                        INSERT OR REPLACE INTO memories (
                          id, created_at, updated_at, project, agent_profile, scope, ring,
                          event_type, summary, details, source_json, tags_json, salience,
                          confidence, sensitivity, retention, expires_at, supersedes_json,
                          superseded_by, links_json, review_json, raw_json
                        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                        "#,
                    )
                    .map_err(sqlite_error_from_rusqlite)?;
                let mut delete_fts = transaction
                    .prepare("DELETE FROM memory_fts WHERE id = ?")
                    .map_err(sqlite_error_from_rusqlite)?;
                let mut insert_fts = transaction
                    .prepare("INSERT INTO memory_fts (id, summary, details, tags, source_ref) VALUES (?, ?, ?, ?, ?)")
                    .map_err(sqlite_error_from_rusqlite)?;

                for event in events {
                    put_with_statements(
                        event,
                        &mut insert_memory,
                        &mut delete_fts,
                        &mut insert_fts,
                    )?;
                }
            }
            transaction.commit().map_err(sqlite_error_from_rusqlite)?;
            Ok(())
        })
    }

    pub fn get(&self, memory_id: &str) -> TreeRingResult<Option<MemoryEvent>> {
        self.connection
            .query_row(
                "SELECT raw_json FROM memories WHERE id = ?",
                params![memory_id],
                event_from_row,
            )
            .optional()
            .map_err(sqlite_error_from_rusqlite)?
            .transpose()
    }

    pub fn list_all(&self, include_superseded: bool) -> TreeRingResult<Vec<MemoryEvent>> {
        let sql = if include_superseded {
            "SELECT raw_json FROM memories ORDER BY created_at DESC"
        } else {
            "SELECT raw_json FROM memories WHERE superseded_by IS NULL ORDER BY created_at DESC"
        };
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(sqlite_error_from_rusqlite)?;
        let rows = statement
            .query_map([], event_from_row)
            .map_err(sqlite_error_from_rusqlite)?;
        collect_rows(rows)
    }

    pub fn search_text(
        &self,
        query: &str,
        include_superseded: bool,
    ) -> TreeRingResult<Vec<MemoryEvent>> {
        self.search_text_limited(query, include_superseded, None)
    }

    pub fn search_text_limited(
        &self,
        query: &str,
        include_superseded: bool,
        limit: Option<usize>,
    ) -> TreeRingResult<Vec<MemoryEvent>> {
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
        let mut sql = sql.to_string();
        if limit.is_some() {
            sql.push_str(" LIMIT ?");
        }
        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(sqlite_error_from_rusqlite)?;
        let rows = if let Some(limit) = limit {
            statement
                .query_map(params![fts_query, limit as i64], event_from_row)
                .map_err(sqlite_error_from_rusqlite)?
        } else {
            statement
                .query_map(params![fts_query], event_from_row)
                .map_err(sqlite_error_from_rusqlite)?
        };
        collect_rows(rows)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search_text_filtered_limited(
        &self,
        query: &str,
        project: Option<&str>,
        agent_profile: Option<&str>,
        scope: Option<&str>,
        rings: Option<&[String]>,
        event_types: Option<&[String]>,
        include_sensitive: bool,
        include_superseded: bool,
        limit: Option<usize>,
    ) -> TreeRingResult<Vec<MemoryEvent>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        if rings.is_some_and(|rings| rings.is_empty())
            || event_types.is_some_and(|event_types| event_types.is_empty())
        {
            return Ok(Vec::new());
        }
        let Some(fts_query) = format_plain_text_fts_query(query) else {
            return Ok(Vec::new());
        };

        let mut sql = String::from(
            r#"
            SELECT memories.raw_json
            FROM memory_fts
            JOIN memories ON memories.id = memory_fts.id
            WHERE memory_fts MATCH ?
            "#,
        );
        let mut parameters = vec![Value::Text(fts_query)];

        if !include_superseded {
            sql.push_str(" AND memories.superseded_by IS NULL");
        }
        if let Some(project) = project {
            sql.push_str(" AND memories.project = ?");
            parameters.push(Value::Text(project.to_string()));
        }
        if let Some(agent_profile) = agent_profile {
            sql.push_str(" AND memories.agent_profile = ?");
            parameters.push(Value::Text(agent_profile.to_string()));
        }
        if let Some(scope) = scope {
            sql.push_str(" AND memories.scope = ?");
            parameters.push(Value::Text(scope.to_string()));
        }
        if let Some(rings) = rings {
            push_in_filter(&mut sql, &mut parameters, "memories.ring", rings);
        }
        if let Some(event_types) = event_types {
            push_in_filter(
                &mut sql,
                &mut parameters,
                "memories.event_type",
                event_types,
            );
        }
        if !include_sensitive {
            sql.push_str(" AND memories.sensitivity = 'normal'");
        }
        sql.push_str(" ORDER BY rank");
        if let Some(limit) = limit {
            sql.push_str(" LIMIT ?");
            parameters.push(Value::Integer(limit as i64));
        }

        let mut statement = self
            .connection
            .prepare(&sql)
            .map_err(sqlite_error_from_rusqlite)?;
        let rows = statement
            .query_map(params_from_iter(parameters), event_from_row)
            .map_err(sqlite_error_from_rusqlite)?;
        collect_rows(rows)
    }

    pub fn supersede(&mut self, old_id: &str, new_id: &str) -> TreeRingResult<()> {
        let Some(mut old) = self.get(old_id)? else {
            return Ok(());
        };
        old.superseded_by = Some(new_id.to_string());
        let raw_json = serde_json::to_string(&old)?;
        retry_locked(|| {
            self.connection
                .execute(
                    "UPDATE memories SET superseded_by = ?, raw_json = ? WHERE id = ?",
                    params![new_id, raw_json, old_id],
                )
                .map_err(sqlite_error_from_rusqlite)?;
            Ok(())
        })
    }

    pub fn delete(&mut self, memory_id: &str) -> TreeRingResult<()> {
        retry_locked(|| {
            let transaction = self
                .connection
                .transaction()
                .map_err(sqlite_error_from_rusqlite)?;
            transaction
                .execute("DELETE FROM memories WHERE id = ?", params![memory_id])
                .map_err(sqlite_error_from_rusqlite)?;
            transaction
                .execute("DELETE FROM memory_fts WHERE id = ?", params![memory_id])
                .map_err(sqlite_error_from_rusqlite)?;
            transaction.commit().map_err(sqlite_error_from_rusqlite)?;
            Ok(())
        })
    }

    pub fn redact(&mut self, memory_id: &str) -> TreeRingResult<()> {
        let Some(mut event) = self.get(memory_id)? else {
            return Ok(());
        };
        event.redact();
        self.put(&event)
    }

    pub fn export_jsonl(
        &self,
        include_sensitive: bool,
        include_superseded: bool,
    ) -> TreeRingResult<(String, ExportReport)> {
        let events: Vec<_> = self
            .list_all(include_superseded)?
            .into_iter()
            .filter(|event| include_sensitive || event.sensitivity == "normal")
            .collect();
        let jsonl = encode_jsonl(&events, include_sensitive)?;
        let report = ExportReport {
            memory_count: events.len(),
            sensitive_included: include_sensitive,
            superseded_included: include_superseded,
        };
        Ok((jsonl, report))
    }

    pub fn import_jsonl(
        &mut self,
        input: &str,
        dry_run: bool,
        replace_existing: bool,
    ) -> TreeRingResult<ImportReport> {
        let decoded = decode_jsonl(input)?;
        let events = normalize_import_events(decoded.events)?;
        let mut report = ImportReport {
            valid_count: events.len(),
            inserted_count: 0,
            replaced_count: 0,
            skipped_duplicate_count: 0,
            dry_run,
        };
        if dry_run {
            return Ok(report);
        }

        for event in events {
            if self.get(&event.id)?.is_some() {
                if replace_existing {
                    self.put(&event)?;
                    self.apply_supersedes(&event)?;
                    report.replaced_count += 1;
                } else {
                    report.skipped_duplicate_count += 1;
                }
            } else {
                self.put(&event)?;
                self.apply_supersedes(&event)?;
                report.inserted_count += 1;
            }
        }
        Ok(report)
    }

    fn apply_supersedes(&mut self, event: &MemoryEvent) -> TreeRingResult<()> {
        for old_id in &event.supersedes {
            self.supersede(old_id, &event.id)?;
        }
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
            let candidate_limit = Some(limit.saturating_mul(128).clamp(256, 2048));
            candidates = self.store.search_text_filtered_limited(
                &search_query,
                project,
                agent_profile,
                scope,
                rings,
                event_types,
                include_sensitive,
                include_superseded,
                candidate_limit,
            )?;
            if !candidates.is_empty() {
                break;
            }
        }

        let mut results: Vec<RecallResult> = candidates
            .into_iter()
            .filter(|event| {
                matches_filters(
                    event,
                    project,
                    agent_profile,
                    scope,
                    rings,
                    event_types,
                    include_sensitive,
                )
            })
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

fn parent_dir_to_create(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

fn sqlite_error_from_rusqlite(error: rusqlite::Error) -> TreeRingError {
    let is_locked = matches!(
        &error,
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(failure.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    );
    if is_locked {
        TreeRingError::StorageLocked(error.to_string())
    } else {
        sqlite_error(error.to_string())
    }
}

fn retry_locked<T>(mut operation: impl FnMut() -> TreeRingResult<T>) -> TreeRingResult<T> {
    let mut delay = Duration::from_millis(WRITE_RETRY_INITIAL_DELAY_MS);
    for attempt in 0..WRITE_RETRY_ATTEMPTS {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) if is_sqlite_lock_error(&error) && attempt + 1 < WRITE_RETRY_ATTEMPTS => {
                std::thread::sleep(delay);
                delay = (delay * 2).min(Duration::from_millis(WRITE_RETRY_MAX_DELAY_MS));
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("retry loop either returns a value or the final error")
}

fn is_sqlite_lock_error(error: &TreeRingError) -> bool {
    matches!(error, TreeRingError::StorageLocked(_))
}

fn push_in_filter(
    sql: &mut String,
    parameters: &mut Vec<Value>,
    column_name: &str,
    values: &[String],
) {
    sql.push_str(" AND ");
    sql.push_str(column_name);
    sql.push_str(" IN (");
    sql.push_str(
        &std::iter::repeat("?")
            .take(values.len())
            .collect::<Vec<_>>()
            .join(", "),
    );
    sql.push(')');
    parameters.extend(values.iter().cloned().map(Value::Text));
}

fn put_in_transaction(transaction: &Transaction<'_>, event: &MemoryEvent) -> TreeRingResult<()> {
    let mut insert_memory = transaction
        .prepare(
            r#"
            INSERT OR REPLACE INTO memories (
              id, created_at, updated_at, project, agent_profile, scope, ring,
              event_type, summary, details, source_json, tags_json, salience,
              confidence, sensitivity, retention, expires_at, supersedes_json,
              superseded_by, links_json, review_json, raw_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .map_err(sqlite_error_from_rusqlite)?;
    let mut delete_fts = transaction
        .prepare("DELETE FROM memory_fts WHERE id = ?")
        .map_err(sqlite_error_from_rusqlite)?;
    let mut insert_fts = transaction
        .prepare("INSERT INTO memory_fts (id, summary, details, tags, source_ref) VALUES (?, ?, ?, ?, ?)")
        .map_err(sqlite_error_from_rusqlite)?;
    put_with_statements(event, &mut insert_memory, &mut delete_fts, &mut insert_fts)
}

fn put_with_statements(
    event: &MemoryEvent,
    insert_memory: &mut rusqlite::Statement<'_>,
    delete_fts: &mut rusqlite::Statement<'_>,
    insert_fts: &mut rusqlite::Statement<'_>,
) -> TreeRingResult<()> {
    event.validate()?;
    let source_json = serde_json::to_string(&event.source)?;
    let tags_json = serde_json::to_string(&event.tags)?;
    let supersedes_json = serde_json::to_string(&event.supersedes)?;
    let links_json = serde_json::to_string(&event.links)?;
    let review_json = serde_json::to_string(&event.review)?;
    let raw_json = serde_json::to_string(event)?;

    insert_memory
        .execute(params![
            &event.id,
            &event.created_at,
            &event.updated_at,
            event.project.as_deref(),
            event.agent_profile.as_deref(),
            &event.scope,
            &event.ring,
            &event.event_type,
            &event.summary,
            &event.details,
            source_json,
            tags_json,
            event.salience,
            event.confidence,
            &event.sensitivity,
            &event.retention,
            event.expires_at.as_deref(),
            supersedes_json,
            event.superseded_by.as_deref(),
            links_json,
            review_json,
            raw_json,
        ])
        .map_err(sqlite_error_from_rusqlite)?;

    delete_fts
        .execute(params![&event.id])
        .map_err(sqlite_error_from_rusqlite)?;
    insert_fts
        .execute(params![
            &event.id,
            &event.summary,
            &event.details,
            event.tags.join(" "),
            &event.source.ref_,
        ])
        .map_err(sqlite_error_from_rusqlite)?;
    Ok(())
}

fn collect_rows<I>(rows: I) -> TreeRingResult<Vec<MemoryEvent>>
where
    I: IntoIterator<Item = rusqlite::Result<TreeRingResult<MemoryEvent>>>,
{
    rows.into_iter()
        .map(|row| {
            row.map_err(sqlite_error_from_rusqlite)
                .and_then(|event| event)
        })
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
    "a", "an", "and", "about", "are", "for", "in", "is", "not", "of", "on", "or", "the", "to",
    "what",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
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
        let journal_mode: String = store
            .connection()
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        let busy_timeout: i64 = store
            .connection()
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();

        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
        assert!(busy_timeout >= 30_000);
    }

    #[test]
    fn plain_relative_sqlite_path_has_no_parent_to_create() {
        assert!(parent_dir_to_create(Path::new("memory.sqlite")).is_none());
        assert_eq!(
            parent_dir_to_create(Path::new("relative/memory.sqlite")),
            Some(Path::new("relative"))
        );
    }

    #[test]
    fn store_searches_fts() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut scar =
            MemoryEvent::new("Avoid stale cache without invalidation.", "warning").unwrap();
        scar.ring = "scar".to_string();
        let mut decision = MemoryEvent::new("Use local SQLite for v0.1.", "decision").unwrap();
        decision.ring = "heartwood".to_string();
        store.put(&scar).unwrap();
        store.put(&decision).unwrap();

        let results = store.search_text("stale cache", false).unwrap();

        assert_eq!(results[0].ring, "scar");
    }

    #[test]
    fn put_many_inserts_memory_and_fts_rows_in_one_batch() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let events: Vec<_> = (0..5)
            .map(|index| MemoryEvent::new(format!("Batch memory {index}"), "lesson").unwrap())
            .collect();

        store.put_many(&events).unwrap();

        let memory_count: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM memories", [], |row| row.get(0))
            .unwrap();
        let fts_count: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM memory_fts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(memory_count, 5);
        assert_eq!(fts_count, 5);
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
            .recall(
                "failure stale cache",
                None,
                None,
                None,
                None,
                None,
                false,
                false,
                8,
                true,
            )
            .unwrap();

        assert_eq!(results[0].memory.ring, "scar");
        assert!(!results
            .iter()
            .any(|result| result.memory.sensitivity == "financial"));
    }

    #[test]
    fn recall_filters_project_before_candidate_limit() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut events = Vec::new();
        for index in 0..300 {
            let mut event = MemoryEvent::new(
                format!("Shared bottleneck recall distractor {index}"),
                "lesson",
            )
            .unwrap();
            event.project = Some("other-project".to_string());
            events.push(event);
        }
        let mut target = MemoryEvent::new("Shared bottleneck recall target", "lesson").unwrap();
        target.project = Some("target-project".to_string());
        let target_id = target.id.clone();
        events.push(target);
        store.put_many(&events).unwrap();

        let results = MemoryRetriever::new(&store)
            .recall(
                "shared bottleneck recall",
                Some("target-project"),
                None,
                None,
                None,
                None,
                false,
                false,
                1,
                false,
            )
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory.id, target_id);
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
        assert!(store
            .search_text("sk-proj-abcdefghijklmnopqrstuvwxyz1234567890", true)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn export_jsonl_excludes_sensitive_and_superseded_by_default() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let normal = MemoryEvent::new("Normal export memory.", "lesson").unwrap();
        let mut sensitive = MemoryEvent::new("Private diagnosis export memory.", "lesson").unwrap();
        sensitive.sensitivity = "health".to_string();
        let mut superseded = MemoryEvent::new("Superseded export memory.", "lesson").unwrap();
        superseded.superseded_by = Some(normal.id.clone());
        store.put(&normal).unwrap();
        store.put(&sensitive).unwrap();
        store.put(&superseded).unwrap();

        let (jsonl, report) = store.export_jsonl(false, false).unwrap();

        assert_eq!(report.memory_count, 1);
        assert!(jsonl.contains("Normal export memory."));
        assert!(!jsonl.contains("Private diagnosis"));
        assert!(!jsonl.contains("Superseded export memory."));
    }

    #[test]
    fn import_jsonl_dry_run_validates_without_writing() {
        let source_dir = tempdir().unwrap();
        let target_dir = tempdir().unwrap();
        let mut source = SQLiteMemoryStore::open(source_dir.path().join("memory.sqlite")).unwrap();
        let event = MemoryEvent::new("Dry run import memory.", "lesson").unwrap();
        source.put(&event).unwrap();
        let (jsonl, _) = source.export_jsonl(false, false).unwrap();
        let mut target = SQLiteMemoryStore::open(target_dir.path().join("memory.sqlite")).unwrap();

        let report = target.import_jsonl(&jsonl, true, false).unwrap();

        assert_eq!(report.valid_count, 1);
        assert_eq!(report.inserted_count, 0);
        assert!(target.list_all(false).unwrap().is_empty());
    }

    #[test]
    fn import_jsonl_skips_duplicates_unless_replace_is_enabled() {
        let source_dir = tempdir().unwrap();
        let target_dir = tempdir().unwrap();
        let mut source = SQLiteMemoryStore::open(source_dir.path().join("memory.sqlite")).unwrap();
        let original = MemoryEvent::new("Original import memory.", "lesson").unwrap();
        source.put(&original).unwrap();
        let (jsonl, _) = source.export_jsonl(false, false).unwrap();
        let mut target = SQLiteMemoryStore::open(target_dir.path().join("memory.sqlite")).unwrap();

        let first = target.import_jsonl(&jsonl, false, false).unwrap();
        let duplicate = target.import_jsonl(&jsonl, false, false).unwrap();

        assert_eq!(first.inserted_count, 1);
        assert_eq!(duplicate.inserted_count, 0);
        assert_eq!(duplicate.skipped_duplicate_count, 1);

        let mut replacement = original.clone();
        replacement.summary = "Replacement import memory.".to_string();
        let replacement_jsonl = encode_jsonl(&[replacement.clone()], false).unwrap();
        let replaced = target
            .import_jsonl(&replacement_jsonl, false, true)
            .unwrap();

        assert_eq!(replaced.replaced_count, 1);
        assert_eq!(
            target.get(&replacement.id).unwrap().unwrap().summary,
            "Replacement import memory."
        );
    }

    #[test]
    fn import_jsonl_reclassifies_sensitive_and_blocks_secrets() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut health =
            MemoryEvent::new("Private diagnosis imported as normal.", "lesson").unwrap();
        health.sensitivity = "normal".to_string();
        let health_jsonl = encode_jsonl(&[health.clone()], false).unwrap();

        let report = store.import_jsonl(&health_jsonl, false, false).unwrap();

        assert_eq!(report.inserted_count, 1);
        assert_eq!(
            store.get(&health.id).unwrap().unwrap().sensitivity,
            "health"
        );

        let secret = MemoryEvent::new(
            "Imported secret sk-proj-abcdefghijklmnopqrstuvwxyz1234567890 must fail.",
            "lesson",
        )
        .unwrap();
        let secret_jsonl = encode_jsonl(&[secret], false).unwrap();

        let err = store
            .import_jsonl(&secret_jsonl, false, false)
            .unwrap_err()
            .to_string();

        assert!(err.contains("blocked"));
    }

    #[test]
    fn import_jsonl_applies_supersedes_to_existing_target_memory() {
        let source_dir = tempdir().unwrap();
        let target_dir = tempdir().unwrap();
        let mut source = SQLiteMemoryStore::open(source_dir.path().join("memory.sqlite")).unwrap();
        let mut target = SQLiteMemoryStore::open(target_dir.path().join("memory.sqlite")).unwrap();
        let old = MemoryEvent::new("Old imported decision.", "decision").unwrap();
        let mut new = MemoryEvent::new("New imported decision.", "decision").unwrap();
        new.supersedes = vec![old.id.clone()];
        target.put(&old).unwrap();
        source.put(&new).unwrap();
        let (jsonl, _) = source.export_jsonl(false, false).unwrap();

        let report = target.import_jsonl(&jsonl, false, false).unwrap();

        assert_eq!(report.inserted_count, 1);
        assert_eq!(
            target.get(&old.id).unwrap().unwrap().superseded_by,
            Some(new.id.clone())
        );
        let active_ids: Vec<_> = target
            .list_all(false)
            .unwrap()
            .into_iter()
            .map(|event| event.id)
            .collect();
        assert_eq!(active_ids, vec![new.id]);
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

    #[test]
    fn concurrent_writers_do_not_orphan_fts_rows() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        SQLiteMemoryStore::open(&db_path).unwrap();

        let handles: Vec<_> = (0..4)
            .map(|worker_id| {
                let db_path = db_path.clone();
                thread::spawn(move || {
                    let mut store = SQLiteMemoryStore::open(db_path).unwrap();
                    for index in 0..20 {
                        let mut event = MemoryEvent::new(
                            format!("Concurrent memory {worker_id}-{index}"),
                            "lesson",
                        )
                        .unwrap();
                        event.project = Some("concurrency".to_string());
                        store.put(&event).unwrap();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let store = SQLiteMemoryStore::open(&db_path).unwrap();
        let memory_count: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM memories", [], |row| row.get(0))
            .unwrap();
        let fts_count: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM memory_fts", [], |row| row.get(0))
            .unwrap();

        assert_eq!(memory_count, 80);
        assert_eq!(fts_count, 80);
    }
}
