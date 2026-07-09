use rusqlite::{
    params, params_from_iter, types::Value, Connection, ErrorCode, OpenFlags, OptionalExtension,
    Row, Transaction,
};
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use tree_ring_memory_core::models::{sqlite_error, MemoryEvent, TreeRingError, TreeRingResult};
use tree_ring_memory_core::recall::{search_queries, RecallScorer};
use tree_ring_memory_core::{
    audit_memories, consolidate_memories, decode_jsonl, encode_jsonl, normalize_import_events,
    plan_maintenance, AuditReport, ConsolidationReport, ConsolidationRequest,
    MaintenanceActionType, MaintenanceFtsReport, MaintenanceReport, MaintenanceRequest,
};

const WRITE_RETRY_ATTEMPTS: usize = 8;
const WRITE_RETRY_INITIAL_DELAY_MS: u64 = 5;
const WRITE_RETRY_MAX_DELAY_MS: u64 = 100;
const EXISTING_ID_QUERY_CHUNK: usize = 500;

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
struct StoredConsolidation {
    id: String,
    created_at: String,
    output_memory_ids: Vec<String>,
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

    pub fn open_read_only(path: impl AsRef<Path>) -> TreeRingResult<Self> {
        let path = path
            .as_ref()
            .canonicalize()
            .map_err(|err| sqlite_error(err.to_string()))?;
        let normalized_path = normalize_sqlite_uri_path(&path.to_string_lossy());
        let uri = format!(
            "file:{}?mode=ro&immutable=1",
            sqlite_uri_path(&normalized_path)
        );
        let connection = Connection::open_with_flags(
            uri,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
        .map_err(sqlite_error_from_rusqlite)?;
        connection
            .busy_timeout(std::time::Duration::from_millis(30_000))
            .map_err(sqlite_error_from_rusqlite)?;
        Ok(Self { connection })
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
                CREATE TABLE IF NOT EXISTS consolidations (
                  id TEXT PRIMARY KEY,
                  created_at TEXT NOT NULL,
                  period_type TEXT NOT NULL,
                  period_key TEXT NOT NULL,
                  source_memory_ids_json TEXT NOT NULL,
                  output_memory_ids_json TEXT NOT NULL,
                  status TEXT NOT NULL,
                  notes TEXT NOT NULL
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
            .prepare(sql)
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

        let ids = events
            .iter()
            .map(|event| event.id.clone())
            .collect::<Vec<_>>();
        let mut known_ids = self.existing_memory_ids(&ids)?;
        let mut batch_events = Vec::new();
        for event in events {
            if known_ids.contains(&event.id) {
                if replace_existing {
                    batch_events.push(event);
                    report.replaced_count += 1;
                } else {
                    report.skipped_duplicate_count += 1;
                }
            } else {
                known_ids.insert(event.id.clone());
                batch_events.push(event);
                report.inserted_count += 1;
            }
        }
        if !batch_events.is_empty() {
            self.put_many(&batch_events)?;
            for event in &batch_events {
                self.apply_supersedes(event)?;
            }
        }
        Ok(report)
    }

    pub fn audit(&self, audit_type: &str) -> TreeRingResult<AuditReport> {
        let events = self.list_all(true)?;
        audit_memories(&events, audit_type)
    }

    pub fn consolidate(
        &mut self,
        request: &ConsolidationRequest,
    ) -> TreeRingResult<ConsolidationReport> {
        let events = self.list_all(false)?;
        let mut report = consolidate_memories(&events, request)?;
        if request.dry_run || report.candidate_count == 0 {
            return Ok(report);
        }

        let source_ids_json =
            serde_json::to_string(&report.source_memory_ids).map_err(TreeRingError::Json)?;
        if !request.force {
            if let Some(existing) = self.find_consolidation(
                report.period_type.as_str(),
                &report.period_key,
                &source_ids_json,
            )? {
                report.id = existing.id;
                report.created_at = existing.created_at;
                report.output_memory_ids = existing.output_memory_ids;
                report.status = "unchanged".to_string();
                report.notes = "Matching consolidation already exists.".to_string();
                report.outputs.clear();
                return Ok(report);
            }
        }

        let previous_outputs = if request.force {
            self.previous_consolidation_outputs(report.period_type.as_str(), &report.period_key)?
        } else {
            Vec::new()
        };
        let output_events = report
            .outputs
            .iter()
            .map(|output| output.memory.clone())
            .collect::<Vec<_>>();
        let supersession_pairs = if request.force {
            consolidation_supersession_pairs(&previous_outputs, &output_events)
        } else {
            Vec::new()
        };
        report.status = "created".to_string();
        report.notes = "Consolidation summaries stored.".to_string();
        self.insert_consolidation_transaction(&report, &output_events, &supersession_pairs)?;
        Ok(report)
    }

    pub fn maintain(&mut self, request: &MaintenanceRequest) -> TreeRingResult<MaintenanceReport> {
        let events = self.list_all(true)?;
        let mut report = plan_maintenance(&events, request);
        report.fts = self.fts_report(false)?;

        let apply_expired = !request.dry_run && request.apply_expired;
        let apply_secret_redactions = !request.dry_run && request.apply_secret_redactions;
        let repair_fts = !request.dry_run && request.repair_fts;
        let needs_transaction = repair_fts
            || report.actions.iter().any(|action| {
                matches!(action.action_type, MaintenanceActionType::DeleteExpired) && apply_expired
                    || matches!(action.action_type, MaintenanceActionType::RedactSecret)
                        && apply_secret_redactions
            });
        if needs_transaction {
            let (applied_indexes, fts_repaired) = retry_locked(|| {
                let transaction = self
                    .connection
                    .transaction()
                    .map_err(sqlite_error_from_rusqlite)?;
                let mut transaction_applied = Vec::new();

                for (index, action) in report.actions.iter().enumerate() {
                    if action.action_type == MaintenanceActionType::RedactSecret
                        && apply_secret_redactions
                        && redact_in_transaction(&transaction, &action.memory_id)?
                    {
                        transaction_applied.push(index);
                    }
                }

                for (index, action) in report.actions.iter().enumerate() {
                    if action.action_type == MaintenanceActionType::DeleteExpired
                        && apply_expired
                        && delete_in_transaction(&transaction, &action.memory_id)?
                    {
                        transaction_applied.push(index);
                    }
                }

                if repair_fts {
                    rebuild_fts_in_transaction(&transaction)?;
                }

                transaction.commit().map_err(sqlite_error_from_rusqlite)?;
                Ok((transaction_applied, repair_fts))
            })?;

            for index in applied_indexes {
                if let Some(action) = report.actions.get_mut(index) {
                    action.applied = true;
                }
            }
            report.applied_action_count = report
                .actions
                .iter()
                .filter(|action| action.applied)
                .count();
            report.fts = self.fts_report(fts_repaired)?;
        }

        report.status = maintenance_status(&report);
        Ok(report)
    }

    fn find_consolidation(
        &self,
        period_type: &str,
        period_key: &str,
        source_ids_json: &str,
    ) -> TreeRingResult<Option<StoredConsolidation>> {
        self.connection
            .query_row(
                r#"
                SELECT id, created_at, output_memory_ids_json
                FROM consolidations
                WHERE period_type = ?
                  AND period_key = ?
                  AND source_memory_ids_json = ?
                  AND status = 'created'
                ORDER BY created_at DESC
                LIMIT 1
                "#,
                params![period_type, period_key, source_ids_json],
                stored_consolidation_from_row,
            )
            .optional()
            .map_err(sqlite_error_from_rusqlite)?
            .transpose()
    }

    fn previous_consolidation_outputs(
        &self,
        period_type: &str,
        period_key: &str,
    ) -> TreeRingResult<Vec<MemoryEvent>> {
        let mut statement = self
            .connection
            .prepare(
                r#"
                SELECT output_memory_ids_json
                FROM consolidations
                WHERE period_type = ?
                  AND period_key = ?
                  AND status = 'created'
                ORDER BY created_at ASC
                "#,
            )
            .map_err(sqlite_error_from_rusqlite)?;
        let rows = statement
            .query_map(params![period_type, period_key], |row| {
                row.get::<_, String>(0)
            })
            .map_err(sqlite_error_from_rusqlite)?;
        let mut output_ids = Vec::new();
        for row in rows {
            let output_ids_json = row.map_err(sqlite_error_from_rusqlite)?;
            let mut parsed = serde_json::from_str::<Vec<String>>(&output_ids_json)
                .map_err(TreeRingError::Json)?;
            output_ids.append(&mut parsed);
        }
        let mut outputs = Vec::new();
        for output_id in output_ids {
            if let Some(output) = self.get(&output_id)? {
                outputs.push(output);
            }
        }
        Ok(outputs)
    }

    fn insert_consolidation_transaction(
        &mut self,
        report: &ConsolidationReport,
        output_events: &[MemoryEvent],
        supersession_pairs: &[(MemoryEvent, String)],
    ) -> TreeRingResult<()> {
        let source_ids_json =
            serde_json::to_string(&report.source_memory_ids).map_err(TreeRingError::Json)?;
        let output_ids_json =
            serde_json::to_string(&report.output_memory_ids).map_err(TreeRingError::Json)?;
        retry_locked(|| {
            let transaction = self
                .connection
                .transaction()
                .map_err(sqlite_error_from_rusqlite)?;
            for event in output_events {
                put_in_transaction(&transaction, event)?;
            }
            for (old, new_id) in supersession_pairs {
                let mut updated = old.clone();
                updated.superseded_by = Some(new_id.clone());
                let raw_json = serde_json::to_string(&updated)?;
                transaction
                    .execute(
                        "UPDATE memories SET superseded_by = ?, raw_json = ? WHERE id = ?",
                        params![new_id, raw_json, &old.id],
                    )
                    .map_err(sqlite_error_from_rusqlite)?;
            }
            transaction
                .execute(
                    r#"
                    INSERT INTO consolidations (
                      id, created_at, period_type, period_key, source_memory_ids_json,
                      output_memory_ids_json, status, notes
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                    params![
                        report.id,
                        report.created_at,
                        report.period_type.as_str(),
                        &report.period_key,
                        source_ids_json,
                        output_ids_json,
                        &report.status,
                        &report.notes
                    ],
                )
                .map_err(sqlite_error_from_rusqlite)?;
            transaction.commit().map_err(sqlite_error_from_rusqlite)?;
            Ok(())
        })
    }

    fn apply_supersedes(&mut self, event: &MemoryEvent) -> TreeRingResult<()> {
        for old_id in &event.supersedes {
            self.supersede(old_id, &event.id)?;
        }
        Ok(())
    }

    fn existing_memory_ids(&self, ids: &[String]) -> TreeRingResult<HashSet<String>> {
        let mut existing = HashSet::new();
        for chunk in ids.chunks(EXISTING_ID_QUERY_CHUNK) {
            if chunk.is_empty() {
                continue;
            }
            let placeholders = std::iter::repeat_n("?", chunk.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!("SELECT id FROM memories WHERE id IN ({placeholders})");
            let mut statement = self
                .connection
                .prepare(&sql)
                .map_err(sqlite_error_from_rusqlite)?;
            let rows = statement
                .query_map(params_from_iter(chunk.iter()), |row| {
                    row.get::<_, String>(0)
                })
                .map_err(sqlite_error_from_rusqlite)?;
            for row in rows {
                existing.insert(row.map_err(sqlite_error_from_rusqlite)?);
            }
        }
        Ok(existing)
    }

    fn fts_report(&self, repaired: bool) -> TreeRingResult<MaintenanceFtsReport> {
        Ok(MaintenanceFtsReport {
            memory_rows: count_query(&self.connection, "SELECT count(*) FROM memories")?,
            fts_rows: count_query(&self.connection, "SELECT count(*) FROM memory_fts")?,
            missing_fts_rows: count_query(
                &self.connection,
                r#"
                SELECT count(*)
                FROM memories
                LEFT JOIN memory_fts ON memories.id = memory_fts.id
                WHERE memory_fts.id IS NULL
                "#,
            )?,
            orphan_fts_rows: count_query(
                &self.connection,
                r#"
                SELECT count(*)
                FROM memory_fts
                LEFT JOIN memories ON memories.id = memory_fts.id
                WHERE memories.id IS NULL
                "#,
            )?,
            repaired,
        })
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

fn consolidation_supersession_pairs(
    previous_outputs: &[MemoryEvent],
    new_outputs: &[MemoryEvent],
) -> Vec<(MemoryEvent, String)> {
    if new_outputs.is_empty() {
        return Vec::new();
    }
    previous_outputs
        .iter()
        .enumerate()
        .map(|(index, old)| {
            let target = best_consolidation_replacement(old, new_outputs)
                .unwrap_or_else(|| &new_outputs[index % new_outputs.len()]);
            (old.clone(), target.id.clone())
        })
        .collect()
}

fn best_consolidation_replacement<'a>(
    old: &MemoryEvent,
    new_outputs: &'a [MemoryEvent],
) -> Option<&'a MemoryEvent> {
    let old_targets = memory_link_targets(old);
    if old_targets.is_empty() {
        return None;
    }
    new_outputs
        .iter()
        .map(|candidate| {
            let candidate_targets = memory_link_targets(candidate);
            let overlap = old_targets.intersection(&candidate_targets).count();
            (overlap, candidate)
        })
        .filter(|(overlap, _candidate)| *overlap > 0)
        .max_by_key(|(overlap, candidate)| (*overlap, std::cmp::Reverse(candidate.id.as_str())))
        .map(|(_overlap, candidate)| candidate)
}

fn memory_link_targets(event: &MemoryEvent) -> HashSet<String> {
    event
        .links
        .iter()
        .filter(|link| link.link_type == "memory")
        .map(|link| link.target.clone())
        .collect()
}

fn maintenance_status(report: &MaintenanceReport) -> String {
    let has_fts_drift = report.fts.missing_fts_rows > 0 || report.fts.orphan_fts_rows > 0;
    let has_unapplied_actions = report.actions.iter().any(|action| !action.applied);
    if !has_fts_drift && !has_unapplied_actions {
        if report.applied_action_count > 0 || report.fts.repaired {
            "applied".to_string()
        } else {
            "clean".to_string()
        }
    } else {
        "planned".to_string()
    }
}

fn count_query(connection: &Connection, sql: &str) -> TreeRingResult<usize> {
    let count: i64 = connection
        .query_row(sql, [], |row| row.get(0))
        .map_err(sqlite_error_from_rusqlite)?;
    Ok(count as usize)
}

fn delete_in_transaction(transaction: &Transaction<'_>, memory_id: &str) -> TreeRingResult<bool> {
    let deleted = transaction
        .execute("DELETE FROM memories WHERE id = ?", params![memory_id])
        .map_err(sqlite_error_from_rusqlite)?;
    transaction
        .execute("DELETE FROM memory_fts WHERE id = ?", params![memory_id])
        .map_err(sqlite_error_from_rusqlite)?;
    Ok(deleted > 0)
}

fn redact_in_transaction(transaction: &Transaction<'_>, memory_id: &str) -> TreeRingResult<bool> {
    let Some(mut event) = transaction
        .query_row(
            "SELECT raw_json FROM memories WHERE id = ?",
            params![memory_id],
            event_from_row,
        )
        .optional()
        .map_err(sqlite_error_from_rusqlite)?
        .transpose()?
    else {
        return Ok(false);
    };
    event.redact();
    put_in_transaction(transaction, &event)?;
    Ok(true)
}

fn rebuild_fts_in_transaction(transaction: &Transaction<'_>) -> TreeRingResult<()> {
    let events = {
        let mut statement = transaction
            .prepare("SELECT raw_json FROM memories ORDER BY created_at DESC")
            .map_err(sqlite_error_from_rusqlite)?;
        let rows = statement
            .query_map([], event_from_row)
            .map_err(sqlite_error_from_rusqlite)?;
        collect_rows(rows)?
    };
    transaction
        .execute("DELETE FROM memory_fts", [])
        .map_err(sqlite_error_from_rusqlite)?;
    let mut insert_fts = transaction
        .prepare("INSERT INTO memory_fts (id, summary, details, tags, source_ref) VALUES (?, ?, ?, ?, ?)")
        .map_err(sqlite_error_from_rusqlite)?;
    for event in events {
        insert_fts
            .execute(params![
                &event.id,
                &event.summary,
                &event.details,
                event.tags.join(" "),
                &event.source.ref_,
            ])
            .map_err(sqlite_error_from_rusqlite)?;
    }
    Ok(())
}

fn stored_consolidation_from_row(
    row: &Row<'_>,
) -> rusqlite::Result<TreeRingResult<StoredConsolidation>> {
    let id: String = row.get(0)?;
    let created_at: String = row.get(1)?;
    let output_ids_json: String = row.get(2)?;
    Ok(serde_json::from_str::<Vec<String>>(&output_ids_json)
        .map(|output_memory_ids| StoredConsolidation {
            id,
            created_at,
            output_memory_ids,
        })
        .map_err(Into::into))
}

fn parent_dir_to_create(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

fn normalize_sqlite_uri_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{rest}").replace('\\', "/")
    } else if let Some(rest) = path.strip_prefix("\\\\?\\") {
        rest.replace('\\', "/")
    } else {
        path.replace('\\', "/")
    }
}

fn sqlite_uri_path(path: &str) -> String {
    path.bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'.' | b'_' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
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
        &std::iter::repeat_n("?", values.len())
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
    fn normalizes_windows_paths_for_sqlite_uri_open() {
        assert_eq!(
            normalize_sqlite_uri_path(r"\\?\C:\Users\lazy\memory.sqlite"),
            "C:/Users/lazy/memory.sqlite"
        );
        assert_eq!(
            normalize_sqlite_uri_path(r"\\?\UNC\server\share\memory.sqlite"),
            "//server/share/memory.sqlite"
        );
        assert_eq!(
            normalize_sqlite_uri_path(r"C:\Users\lazy\memory.sqlite"),
            "C:/Users/lazy/memory.sqlite"
        );
    }

    #[test]
    fn sqlite_uri_path_percent_encodes_only_unsafe_bytes() {
        assert_eq!(
            sqlite_uri_path("/tmp/tree ring/mémoire.sqlite"),
            "/tmp/tree%20ring/m%C3%A9moire.sqlite"
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
    fn import_jsonl_preserves_duplicate_order_within_one_file() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let first = MemoryEvent::new("First duplicate import memory.", "lesson").unwrap();
        let mut second = first.clone();
        second.summary = "Second duplicate import memory.".to_string();
        let duplicate_jsonl = encode_jsonl(&[first.clone(), second.clone()], false).unwrap();

        let skipped = store.import_jsonl(&duplicate_jsonl, false, false).unwrap();

        assert_eq!(skipped.inserted_count, 1);
        assert_eq!(skipped.replaced_count, 0);
        assert_eq!(skipped.skipped_duplicate_count, 1);
        assert_eq!(
            store.get(&first.id).unwrap().unwrap().summary,
            "First duplicate import memory."
        );

        let replaced = store.import_jsonl(&duplicate_jsonl, false, true).unwrap();

        assert_eq!(replaced.inserted_count, 0);
        assert_eq!(replaced.replaced_count, 2);
        assert_eq!(replaced.skipped_duplicate_count, 0);
        assert_eq!(
            store.get(&first.id).unwrap().unwrap().summary,
            "Second duplicate import memory."
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
    fn import_jsonl_applies_supersedes_after_all_imported_rows_are_written() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let old = MemoryEvent::new("Old decision imported after replacement.", "decision").unwrap();
        let mut new = MemoryEvent::new("New decision imported before old.", "decision").unwrap();
        new.supersedes = vec![old.id.clone()];
        let out_of_order_jsonl = encode_jsonl(&[new.clone(), old.clone()], false).unwrap();

        let report = store
            .import_jsonl(&out_of_order_jsonl, false, false)
            .unwrap();

        assert_eq!(report.inserted_count, 2);
        assert_eq!(
            store.get(&old.id).unwrap().unwrap().superseded_by,
            Some(new.id.clone())
        );
        let active_ids: Vec<_> = store
            .list_all(false)
            .unwrap()
            .into_iter()
            .map(|event| event.id)
            .collect();
        assert_eq!(active_ids, vec![new.id]);
    }

    #[test]
    fn audit_uses_all_rows_and_does_not_mutate_storage() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut old = MemoryEvent::new("Old decision.", "decision").unwrap();
        old.superseded_by = Some("mem_missing".to_string());
        let mut sensitive = MemoryEvent::new("Private diagnosis audit.", "lesson").unwrap();
        sensitive.sensitivity = "health".to_string();
        store.put(&old).unwrap();
        store.put(&sensitive).unwrap();

        let before = store.list_all(true).unwrap();
        let report = store.audit("all").unwrap();
        let after = store.list_all(true).unwrap();

        assert_eq!(before, after);
        assert_eq!(report.memory_count, 2);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.audit_type == "sensitive"));
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.audit_type == "supersession"));
    }

    #[test]
    fn audit_rejects_unknown_type() {
        let dir = tempdir().unwrap();
        let store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let err = store.audit("unknown").unwrap_err().to_string();

        assert!(err.contains("unsupported audit_type"));
    }

    #[test]
    fn consolidation_dry_run_writes_nothing() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut event = MemoryEvent::new("Use deterministic consolidation.", "decision").unwrap();
        event.project = Some("core".to_string());
        store.put(&event).unwrap();
        let request = ConsolidationRequest {
            period_type: tree_ring_memory_core::ConsolidationPeriod::Manual,
            period_key: Some("manual-test".to_string()),
            project: Some("core".to_string()),
            dry_run: true,
            force: false,
        };

        let report = store.consolidate(&request).unwrap();
        let rows: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM memories", [], |row| row.get(0))
            .unwrap();
        let records: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM consolidations", [], |row| row.get(0))
            .unwrap();

        assert_eq!(report.status, "dry_run");
        assert_eq!(report.candidate_count, 1);
        assert_eq!(rows, 1);
        assert_eq!(records, 0);
    }

    #[test]
    fn consolidation_empty_writes_no_rows_or_records() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let request = ConsolidationRequest {
            period_type: tree_ring_memory_core::ConsolidationPeriod::Manual,
            period_key: Some("manual-empty".to_string()),
            project: Some("core".to_string()),
            dry_run: false,
            force: false,
        };

        let report = store.consolidate(&request).unwrap();
        let rows: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM memories", [], |row| row.get(0))
            .unwrap();
        let records: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM consolidations", [], |row| row.get(0))
            .unwrap();

        assert_eq!(report.status, "empty");
        assert_eq!(report.candidate_count, 0);
        assert_eq!(rows, 0);
        assert_eq!(records, 0);
    }

    #[test]
    fn consolidation_is_idempotent_for_same_source_set() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut event = MemoryEvent::new("Use deterministic consolidation.", "decision").unwrap();
        event.project = Some("core".to_string());
        store.put(&event).unwrap();
        let request = ConsolidationRequest {
            period_type: tree_ring_memory_core::ConsolidationPeriod::Manual,
            period_key: Some("manual-test".to_string()),
            project: Some("core".to_string()),
            dry_run: false,
            force: false,
        };

        let first = store.consolidate(&request).unwrap();
        let second = store.consolidate(&request).unwrap();
        let records: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM consolidations", [], |row| row.get(0))
            .unwrap();

        assert_eq!(first.status, "created");
        assert_eq!(second.status, "unchanged");
        assert_eq!(second.output_memory_ids, first.output_memory_ids);
        assert!(second.outputs.is_empty());
        assert_eq!(records, 1);
    }

    #[test]
    fn forced_consolidation_supersedes_prior_summary() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut event = MemoryEvent::new("Use deterministic consolidation.", "decision").unwrap();
        event.project = Some("core".to_string());
        store.put(&event).unwrap();
        let mut request = ConsolidationRequest {
            period_type: tree_ring_memory_core::ConsolidationPeriod::Manual,
            period_key: Some("manual-test".to_string()),
            project: Some("core".to_string()),
            dry_run: false,
            force: false,
        };
        let first = store.consolidate(&request).unwrap();
        request.force = true;

        let second = store.consolidate(&request).unwrap();
        let old = store.get(&first.output_memory_ids[0]).unwrap().unwrap();
        let records: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM consolidations", [], |row| row.get(0))
            .unwrap();

        assert_eq!(second.status, "created");
        assert_eq!(
            old.superseded_by.as_deref(),
            Some(second.output_memory_ids[0].as_str())
        );
        assert_eq!(records, 2);
    }

    #[test]
    fn forced_consolidation_supersedes_prior_summary_when_source_set_changes() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut first_event =
            MemoryEvent::new("Use deterministic consolidation.", "decision").unwrap();
        first_event.project = Some("core".to_string());
        store.put(&first_event).unwrap();
        let mut request = ConsolidationRequest {
            period_type: tree_ring_memory_core::ConsolidationPeriod::Manual,
            period_key: Some("manual-test".to_string()),
            project: Some("core".to_string()),
            dry_run: false,
            force: false,
        };
        let first = store.consolidate(&request).unwrap();

        let mut second_event = MemoryEvent::new(
            "Keep forced consolidation replacing old summaries.",
            "decision",
        )
        .unwrap();
        second_event.project = Some("core".to_string());
        store.put(&second_event).unwrap();
        request.force = true;

        let second = store.consolidate(&request).unwrap();
        let old = store.get(&first.output_memory_ids[0]).unwrap().unwrap();

        assert_eq!(second.status, "created");
        assert_eq!(
            old.superseded_by.as_deref(),
            Some(second.output_memory_ids[0].as_str())
        );
        assert!(second.source_memory_ids.contains(&first_event.id));
        assert!(second.source_memory_ids.contains(&second_event.id));
    }

    #[test]
    fn forced_consolidation_maps_multiple_prior_outputs_to_matching_new_outputs() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut decision =
            MemoryEvent::new("Use deterministic consolidation.", "decision").unwrap();
        decision.project = Some("core".to_string());
        let mut lesson = MemoryEvent::new("Keep source-linked summaries.", "lesson").unwrap();
        lesson.project = Some("core".to_string());
        store.put(&decision).unwrap();
        store.put(&lesson).unwrap();
        let mut request = ConsolidationRequest {
            period_type: tree_ring_memory_core::ConsolidationPeriod::Manual,
            period_key: Some("manual-multi-output".to_string()),
            project: Some("core".to_string()),
            dry_run: false,
            force: false,
        };
        let first = store.consolidate(&request).unwrap();
        let output_id_for_source = |report: &ConsolidationReport, source_id: &str| {
            report
                .outputs
                .iter()
                .find(|output| {
                    output
                        .memory
                        .links
                        .iter()
                        .any(|link| link.link_type == "memory" && link.target == source_id)
                })
                .map(|output| output.memory.id.clone())
                .unwrap()
        };
        let old_decision_output_id = output_id_for_source(&first, &decision.id);
        let old_lesson_output_id = output_id_for_source(&first, &lesson.id);
        request.force = true;

        let second = store.consolidate(&request).unwrap();
        let new_decision_output_id = output_id_for_source(&second, &decision.id);
        let new_lesson_output_id = output_id_for_source(&second, &lesson.id);
        let old_decision_output = store.get(&old_decision_output_id).unwrap().unwrap();
        let old_lesson_output = store.get(&old_lesson_output_id).unwrap().unwrap();

        assert_eq!(second.status, "created");
        assert_eq!(
            old_decision_output.superseded_by.as_deref(),
            Some(new_decision_output_id.as_str())
        );
        assert_eq!(
            old_lesson_output.superseded_by.as_deref(),
            Some(new_lesson_output_id.as_str())
        );
        assert_ne!(new_decision_output_id, new_lesson_output_id);
    }

    #[test]
    fn consolidation_summarizes_sensitive_without_payload_leakage() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut event = MemoryEvent::new("Private diagnosis payload.", "lesson").unwrap();
        event.project = Some("core".to_string());
        event.sensitivity = "health".to_string();
        store.put(&event).unwrap();
        let request = ConsolidationRequest {
            period_type: tree_ring_memory_core::ConsolidationPeriod::Manual,
            period_key: Some("manual-sensitive".to_string()),
            project: Some("core".to_string()),
            dry_run: false,
            force: false,
        };

        let report = store.consolidate(&request).unwrap();
        let output = store.get(&report.output_memory_ids[0]).unwrap().unwrap();

        assert_eq!(output.sensitivity, "private");
        assert!(output.review.needs_review);
        assert!(!output.summary.contains("diagnosis"));
        assert!(!output.details.contains("diagnosis"));
    }

    #[test]
    fn maintenance_dry_run_reports_expired_and_secret_without_mutating() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let expired = expired_maintenance_memory("Temporary cache", "ephemeral", "cambium");
        let mut secret = MemoryEvent::new("Secret-like memory", "lesson").unwrap();
        secret.details = "Use sk-proj-abcdefghijklmnopqrstuvwxyz1234567890".to_string();
        store.put(&expired).unwrap();
        store.put(&secret).unwrap();

        let report = store.maintain(&MaintenanceRequest::default()).unwrap();

        assert_eq!(report.status, "planned");
        assert_eq!(report.planned_action_count, 2);
        assert_eq!(report.applied_action_count, 0);
        assert_action_type(&report, &expired.id, MaintenanceActionType::DeleteExpired);
        assert_action_type(&report, &secret.id, MaintenanceActionType::RedactSecret);
        assert!(store.get(&expired.id).unwrap().is_some());
        assert_eq!(
            store.get(&secret.id).unwrap().unwrap().summary,
            "Secret-like memory"
        );
    }

    #[test]
    fn maintenance_apply_expired_deletes_eligible_and_preserves_protected() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let expired = expired_maintenance_memory("Temporary cache", "ephemeral", "cambium");
        let protected = expired_maintenance_memory("Protected scar", "ephemeral", "scar");
        store.put(&expired).unwrap();
        store.put(&protected).unwrap();

        let report = store
            .maintain(&MaintenanceRequest {
                dry_run: false,
                apply_expired: true,
                ..MaintenanceRequest::default()
            })
            .unwrap();

        assert_eq!(report.status, "planned");
        assert_eq!(report.applied_action_count, 1);
        assert!(store.get(&expired.id).unwrap().is_none());
        assert!(store.get(&protected.id).unwrap().is_some());
        let protected_action = report
            .actions
            .iter()
            .find(|action| action.memory_id == protected.id)
            .unwrap();
        assert_eq!(
            protected_action.action_type,
            MaintenanceActionType::ReviewExpiredProtected
        );
        assert!(!protected_action.applied);
    }

    #[test]
    fn maintenance_apply_secret_redactions_redacts_secret_like_memory() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let raw_secret = "sk-proj-abcdefghijklmnopqrstuvwxyz1234567890";
        let mut event = MemoryEvent::new("Secret-like memory", "lesson").unwrap();
        event.details = format!("Use {raw_secret}");
        store.put(&event).unwrap();

        let report = store
            .maintain(&MaintenanceRequest {
                dry_run: false,
                apply_secret_redactions: true,
                ..MaintenanceRequest::default()
            })
            .unwrap();

        let redacted = store.get(&event.id).unwrap().unwrap();
        assert_eq!(report.status, "applied");
        assert_eq!(report.applied_action_count, 1);
        assert_eq!(redacted.summary, "[REDACTED]");
        assert!(!serde_json::to_string(&redacted)
            .unwrap()
            .contains(raw_secret));
        assert!(store.search_text(raw_secret, true).unwrap().is_empty());
    }

    #[test]
    fn maintenance_detects_and_repairs_fts_drift() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let event = MemoryEvent::new("Repair missing FTS row.", "lesson").unwrap();
        store.put(&event).unwrap();
        store
            .connection()
            .execute("DELETE FROM memory_fts WHERE id = ?", params![&event.id])
            .unwrap();
        store
            .connection()
            .execute(
                "INSERT INTO memory_fts (id, summary, details, tags, source_ref) VALUES (?, ?, ?, ?, ?)",
                params!["mem_orphan", "orphan", "", "", ""],
            )
            .unwrap();

        let dry_run = store.maintain(&MaintenanceRequest::default()).unwrap();
        assert_eq!(dry_run.status, "planned");
        assert_eq!(dry_run.fts.missing_fts_rows, 1);
        assert_eq!(dry_run.fts.orphan_fts_rows, 1);

        let repaired = store
            .maintain(&MaintenanceRequest {
                dry_run: false,
                repair_fts: true,
                ..MaintenanceRequest::default()
            })
            .unwrap();

        assert_eq!(repaired.status, "applied");
        assert!(repaired.fts.repaired);
        assert_eq!(repaired.fts.missing_fts_rows, 0);
        assert_eq!(repaired.fts.orphan_fts_rows, 0);
        assert_eq!(
            store.search_text("Repair missing", false).unwrap()[0].id,
            event.id
        );
    }

    #[test]
    fn maintenance_project_filter_limits_planned_actions() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut included = expired_maintenance_memory("UI cache", "ephemeral", "cambium");
        included.project = Some("ui".to_string());
        let mut excluded = expired_maintenance_memory("CLI cache", "ephemeral", "cambium");
        excluded.project = Some("cli".to_string());
        store.put(&included).unwrap();
        store.put(&excluded).unwrap();

        let report = store
            .maintain(&MaintenanceRequest {
                project: Some("ui".to_string()),
                ..MaintenanceRequest::default()
            })
            .unwrap();

        assert_eq!(report.memory_count, 1);
        assert_eq!(report.planned_action_count, 1);
        assert_eq!(report.actions[0].memory_id, included.id);
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

    fn expired_maintenance_memory(summary: &str, retention: &str, ring: &str) -> MemoryEvent {
        let mut event = MemoryEvent::new(summary, "lesson").unwrap();
        event.retention = retention.to_string();
        event.ring = ring.to_string();
        event.expires_at = Some("2000-01-01T00:00:00Z".to_string());
        event
    }

    fn assert_action_type(
        report: &MaintenanceReport,
        memory_id: &str,
        action_type: MaintenanceActionType,
    ) {
        assert!(report
            .actions
            .iter()
            .any(|action| action.memory_id == memory_id && action.action_type == action_type));
    }
}
