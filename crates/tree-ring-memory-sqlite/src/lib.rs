use rusqlite::{
    params, params_from_iter, types::Value, Connection, ErrorCode, OptionalExtension,
    TransactionBehavior,
};
use std::collections::HashSet;
use std::path::Path;

use tree_ring_memory_core::models::{sqlite_error, MemoryEvent, TreeRingError, TreeRingResult};
use tree_ring_memory_core::recall::{search_queries, RecallScorer};
use tree_ring_memory_core::{
    audit_memories, consolidate_memories, decode_jsonl, encode_jsonl, normalize_import_events,
    normalize_legacy_private_scope_identity, plan_maintenance, AuditReport, ConsolidationReport,
    ConsolidationRequest, MaintenanceActionType, MaintenanceFtsReport, MaintenanceReport,
    MaintenanceRequest,
};

mod lifecycle;
mod schema;
mod search;
mod write;

const SQLITE_SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone)]
pub struct RecallResult {
    pub memory: MemoryEvent,
    pub score: f64,
    pub ranking: std::collections::BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct RecallOptions<'a> {
    pub project: Option<&'a str>,
    pub agent_profile: Option<&'a str>,
    pub workflow_id: Option<&'a str>,
    pub session_id: Option<&'a str>,
    pub scope: Option<&'a str>,
    pub rings: Option<&'a [String]>,
    pub event_types: Option<&'a [String]>,
    pub include_sensitive: bool,
    pub include_superseded: bool,
    pub limit: usize,
    pub explain_ranking: bool,
}

impl Default for RecallOptions<'_> {
    fn default() -> Self {
        Self {
            project: None,
            agent_profile: None,
            workflow_id: None,
            session_id: None,
            scope: None,
            rings: None,
            event_types: None,
            include_sensitive: false,
            include_superseded: false,
            limit: 8,
            explain_ranking: false,
        }
    }
}

pub struct SQLiteMemoryStore {
    connection: Connection,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum PutOutcome {
    Created,
    Existing(MemoryEvent),
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
        let connection = schema::open_connection(path.as_ref())?;
        let store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_read_only(path: impl AsRef<Path>) -> TreeRingResult<Self> {
        let connection = schema::open_read_only_connection(path.as_ref())?;
        Ok(Self { connection })
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn migrate(&self) -> TreeRingResult<()> {
        if schema::user_version(&self.connection)? >= SQLITE_SCHEMA_VERSION {
            return Ok(());
        }
        write::retry_locked(|| {
            let transaction = rusqlite::Transaction::new_unchecked(
                &self.connection,
                TransactionBehavior::Immediate,
            )
            .map_err(sqlite_error_from_rusqlite)?;
            if schema::user_version(&transaction)? >= SQLITE_SCHEMA_VERSION {
                transaction.commit().map_err(sqlite_error_from_rusqlite)?;
                return Ok(());
            }
            transaction
                .execute_batch(
                    r#"
                CREATE TABLE IF NOT EXISTS memories (
                  id TEXT PRIMARY KEY,
                  created_at TEXT NOT NULL,
                  updated_at TEXT NOT NULL,
                  project TEXT,
                  agent_profile TEXT,
                  workflow_id TEXT,
                  session_id TEXT,
                  operation_id TEXT,
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
                CREATE TABLE IF NOT EXISTS operation_claims (
                  namespace_hash BLOB PRIMARY KEY NOT NULL
                    CHECK(length(namespace_hash) = 32),
                  memory_id TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS redaction_tombstones (
                  memory_id TEXT PRIMARY KEY NOT NULL
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
            for (column, definition) in [
                ("workflow_id", "TEXT"),
                ("session_id", "TEXT"),
                ("operation_id", "TEXT"),
            ] {
                if !schema::memory_column_exists(&transaction, column)? {
                    transaction
                        .execute(
                            &format!("ALTER TABLE memories ADD COLUMN {column} {definition}"),
                            [],
                        )
                        .map_err(sqlite_error_from_rusqlite)?;
                }
            }
            normalize_legacy_private_scope_rows(&transaction)?;
            transaction
                .execute_batch(
                    r#"
                CREATE INDEX IF NOT EXISTS idx_memories_project
                  ON memories(project);
                CREATE INDEX IF NOT EXISTS idx_memories_agent_profile
                  ON memories(agent_profile);
                CREATE INDEX IF NOT EXISTS idx_memories_workflow_id
                  ON memories(workflow_id);
                CREATE INDEX IF NOT EXISTS idx_memories_session_id
                  ON memories(session_id);
                CREATE UNIQUE INDEX IF NOT EXISTS idx_memories_operation_namespace
                  ON memories(
                    COALESCE(project, ''),
                    COALESCE(workflow_id, ''),
                    COALESCE(agent_profile, ''),
                    operation_id
                  )
                  WHERE operation_id IS NOT NULL;
                CREATE INDEX IF NOT EXISTS idx_operation_claims_memory_id
                  ON operation_claims(memory_id);
                INSERT OR IGNORE INTO redaction_tombstones (memory_id)
                  SELECT id
                  FROM memories
                  WHERE summary = '[REDACTED]';
                "#,
                )
                .map_err(sqlite_error_from_rusqlite)?;
            transaction
                .pragma_update(None, "user_version", SQLITE_SCHEMA_VERSION)
                .map_err(sqlite_error_from_rusqlite)?;
            transaction.commit().map_err(sqlite_error_from_rusqlite)?;
            Ok(())
        })
    }

    pub fn put(&mut self, event: &MemoryEvent) -> TreeRingResult<()> {
        write::retry_locked(|| {
            let transaction = self
                .connection
                .transaction()
                .map_err(sqlite_error_from_rusqlite)?;
            write::put_in_transaction(&transaction, event)?;
            transaction.commit().map_err(sqlite_error_from_rusqlite)?;
            Ok(())
        })
    }

    pub fn put_idempotent(&mut self, event: &MemoryEvent) -> TreeRingResult<PutOutcome> {
        let Some(operation_id) = event.operation_id.as_deref() else {
            self.put(event)?;
            return Ok(PutOutcome::Created);
        };
        event.validate()?;
        write::retry_locked(|| {
            let transaction = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sqlite_error_from_rusqlite)?;
            if let Some(existing) =
                find_memory_by_operation_namespace(&transaction, event, operation_id)?
            {
                transaction.commit().map_err(sqlite_error_from_rusqlite)?;
                return Ok(PutOutcome::Existing(existing));
            }
            write::put_in_transaction(&transaction, event)?;
            transaction.commit().map_err(sqlite_error_from_rusqlite)?;
            Ok(PutOutcome::Created)
        })
    }

    pub fn put_many(&mut self, events: &[MemoryEvent]) -> TreeRingResult<()> {
        write::retry_locked(|| {
            let transaction = self
                .connection
                .transaction()
                .map_err(sqlite_error_from_rusqlite)?;
            {
                let mut insert_memory = transaction
                    .prepare(write::UPSERT_MEMORY_SQL)
                    .map_err(sqlite_error_from_rusqlite)?;
                let mut delete_fts = transaction
                    .prepare("DELETE FROM memory_fts WHERE id = ?")
                    .map_err(sqlite_error_from_rusqlite)?;
                let mut insert_fts = transaction
                    .prepare("INSERT INTO memory_fts (id, summary, details, tags, source_ref) VALUES (?, ?, ?, ?, ?)")
                    .map_err(sqlite_error_from_rusqlite)?;

                for event in events {
                    write::prepare_memory_write(&transaction, event)?;
                    write::put_with_statements(
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
                search::event_from_row,
            )
            .optional()
            .map_err(sqlite_error_from_rusqlite)?
            .transpose()
    }

    pub fn list_all(&self, include_superseded: bool) -> TreeRingResult<Vec<MemoryEvent>> {
        list_all_on_connection(&self.connection, include_superseded)
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
                .query_map(params![fts_query, limit as i64], search::event_from_row)
                .map_err(sqlite_error_from_rusqlite)?
        } else {
            statement
                .query_map(params![fts_query], search::event_from_row)
                .map_err(sqlite_error_from_rusqlite)?
        };
        search::collect_rows(rows)
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
        self.search_text_filtered_with_context_limited(
            query,
            project,
            agent_profile,
            None,
            None,
            scope,
            rings,
            event_types,
            include_sensitive,
            include_superseded,
            limit,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn search_text_filtered_with_context_limited(
        &self,
        query: &str,
        project: Option<&str>,
        agent_profile: Option<&str>,
        workflow_id: Option<&str>,
        session_id: Option<&str>,
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
        self.search_fts_filtered_limited(
            &fts_query,
            project,
            agent_profile,
            workflow_id,
            session_id,
            scope,
            rings,
            event_types,
            include_sensitive,
            include_superseded,
            limit,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn search_fts_filtered_limited(
        &self,
        fts_query: &str,
        project: Option<&str>,
        agent_profile: Option<&str>,
        workflow_id: Option<&str>,
        session_id: Option<&str>,
        scope: Option<&str>,
        rings: Option<&[String]>,
        event_types: Option<&[String]>,
        include_sensitive: bool,
        include_superseded: bool,
        limit: Option<usize>,
    ) -> TreeRingResult<Vec<MemoryEvent>> {
        let mut sql = String::from(
            r#"
            SELECT memories.raw_json
            FROM memory_fts
            JOIN memories ON memories.id = memory_fts.id
            WHERE memory_fts MATCH ?
            "#,
        );
        let mut parameters = vec![Value::Text(fts_query.to_string())];

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
        if let Some(workflow_id) = workflow_id {
            sql.push_str(" AND memories.workflow_id = ?");
            parameters.push(Value::Text(workflow_id.to_string()));
        }
        if let Some(session_id) = session_id {
            sql.push_str(" AND memories.session_id = ?");
            parameters.push(Value::Text(session_id.to_string()));
        }
        if let Some(scope) = scope {
            sql.push_str(" AND memories.scope = ?");
            parameters.push(Value::Text(scope.to_string()));
        }
        if let Some(rings) = rings {
            search::push_in_filter(&mut sql, &mut parameters, "memories.ring", rings);
        }
        if let Some(event_types) = event_types {
            search::push_in_filter(
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
            .query_map(params_from_iter(parameters), search::event_from_row)
            .map_err(sqlite_error_from_rusqlite)?;
        search::collect_rows(rows)
    }

    pub fn supersede(&mut self, old_id: &str, new_id: &str) -> TreeRingResult<()> {
        write::retry_locked(|| {
            let transaction = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sqlite_error_from_rusqlite)?;
            write::supersede_in_transaction(&transaction, old_id, new_id)?;
            transaction.commit().map_err(sqlite_error_from_rusqlite)?;
            Ok(())
        })
    }

    pub fn delete(&mut self, memory_id: &str) -> TreeRingResult<()> {
        write::retry_locked(|| {
            let transaction = self
                .connection
                .transaction()
                .map_err(sqlite_error_from_rusqlite)?;
            write::delete_in_transaction(&transaction, memory_id)?;
            transaction.commit().map_err(sqlite_error_from_rusqlite)?;
            Ok(())
        })
    }

    pub fn redact(&mut self, memory_id: &str) -> TreeRingResult<()> {
        write::retry_locked(|| {
            let transaction = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sqlite_error_from_rusqlite)?;
            write::redact_in_transaction(&transaction, memory_id)?;
            transaction.commit().map_err(sqlite_error_from_rusqlite)?;
            Ok(())
        })
    }

    pub fn change_ring(
        &mut self,
        memory_id: &str,
        ring: &str,
        event_type: &str,
    ) -> TreeRingResult<Option<MemoryEvent>> {
        write::retry_locked(|| {
            let transaction = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sqlite_error_from_rusqlite)?;
            let Some(mut event) = transaction
                .query_row(
                    "SELECT raw_json FROM memories WHERE id = ?",
                    params![memory_id],
                    search::event_from_row,
                )
                .optional()
                .map_err(sqlite_error_from_rusqlite)?
                .transpose()?
            else {
                transaction.commit().map_err(sqlite_error_from_rusqlite)?;
                return Ok(None);
            };
            let tombstoned = write::is_redaction_tombstoned(&transaction, memory_id)?;
            event.ring = ring.to_string();
            if !tombstoned {
                event.event_type = event_type.to_string();
            }
            event.updated_at = tree_ring_memory_core::now_iso();
            if ring == "heartwood" {
                event.retention = "durable".to_string();
            }
            write::put_in_transaction(&transaction, &event)?;
            transaction.commit().map_err(sqlite_error_from_rusqlite)?;
            Ok(Some(event))
        })
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

        let (inserted_count, replaced_count, skipped_duplicate_count) =
            write::retry_locked(|| {
                let transaction = self
                    .connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(sqlite_error_from_rusqlite)?;
                let mut inserted_count = 0;
                let mut replaced_count = 0;
                let mut skipped_duplicate_count = 0;
                let mut written_events = Vec::new();
                for event in &events {
                    let exists: bool = transaction
                        .query_row(
                            "SELECT EXISTS(SELECT 1 FROM memories WHERE id = ?)",
                            params![&event.id],
                            |row| row.get(0),
                        )
                        .map_err(sqlite_error_from_rusqlite)?;
                    if exists && !replace_existing {
                        skipped_duplicate_count += 1;
                        continue;
                    }
                    write::put_in_transaction(&transaction, event)?;
                    written_events.push(event);
                    if exists {
                        replaced_count += 1;
                    } else {
                        inserted_count += 1;
                    }
                }
                for event in written_events {
                    for old_id in &event.supersedes {
                        write::supersede_in_transaction(&transaction, old_id, &event.id)?;
                    }
                }
                transaction.commit().map_err(sqlite_error_from_rusqlite)?;
                Ok((inserted_count, replaced_count, skipped_duplicate_count))
            })?;
        report.inserted_count = inserted_count;
        report.replaced_count = replaced_count;
        report.skipped_duplicate_count = skipped_duplicate_count;
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
        if request.dry_run {
            let events = self.list_all(false)?;
            return consolidate_memories(&events, request);
        }
        write::retry_locked(|| {
            let transaction = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sqlite_error_from_rusqlite)?;
            let events = list_all_on_connection(&transaction, false)?;
            let mut report = consolidate_memories(&events, request)?;
            if report.candidate_count == 0 {
                transaction.commit().map_err(sqlite_error_from_rusqlite)?;
                return Ok(report);
            }

            let source_ids_json =
                serde_json::to_string(&report.source_memory_ids).map_err(TreeRingError::Json)?;
            if !request.force {
                if let Some(existing) = find_consolidation_on_connection(
                    &transaction,
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
                    transaction.commit().map_err(sqlite_error_from_rusqlite)?;
                    return Ok(report);
                }
            }

            let output_events = report
                .outputs
                .iter()
                .map(|output| output.memory.clone())
                .collect::<Vec<_>>();
            let supersession_pairs = if request.force {
                let previous_outputs = previous_consolidation_outputs_on_connection(
                    &transaction,
                    report.period_type.as_str(),
                    &report.period_key,
                    request,
                )?;
                consolidation_supersession_pairs(&previous_outputs, &output_events)
            } else {
                Vec::new()
            };
            report.status = "created".to_string();
            report.notes = "Consolidation summaries stored.".to_string();
            for event in &output_events {
                write::put_in_transaction(&transaction, event)?;
            }
            for (old_id, new_id) in &supersession_pairs {
                write::supersede_in_transaction(&transaction, old_id, new_id)?;
            }
            insert_consolidation_record(&transaction, &report, &source_ids_json)?;
            transaction.commit().map_err(sqlite_error_from_rusqlite)?;
            Ok(report)
        })
    }

    pub fn maintain(&mut self, request: &MaintenanceRequest) -> TreeRingResult<MaintenanceReport> {
        let apply_expired = !request.dry_run && request.apply_expired;
        let apply_secret_redactions = !request.dry_run && request.apply_secret_redactions;
        let repair_fts = !request.dry_run && request.repair_fts;
        let needs_transaction = apply_expired || apply_secret_redactions || repair_fts;
        let mut report = if needs_transaction {
            write::retry_locked(|| {
                let transaction = self
                    .connection
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(sqlite_error_from_rusqlite)?;
                let events = list_all_on_connection(&transaction, true)?;
                let mut transaction_report = plan_maintenance(&events, request);
                transaction_report.fts = fts_report_on_connection(&transaction, false)?;

                for action in &mut transaction_report.actions {
                    if action.action_type == MaintenanceActionType::RedactSecret
                        && apply_secret_redactions
                        && write::redact_in_transaction(&transaction, &action.memory_id)?
                    {
                        action.applied = true;
                    }
                }

                for action in &mut transaction_report.actions {
                    if action.action_type == MaintenanceActionType::DeleteExpired
                        && apply_expired
                        && write::delete_in_transaction(&transaction, &action.memory_id)?
                    {
                        action.applied = true;
                    }
                }

                if repair_fts {
                    lifecycle::rebuild_fts_in_transaction(&transaction)?;
                }
                transaction_report.applied_action_count = transaction_report
                    .actions
                    .iter()
                    .filter(|action| action.applied)
                    .count();
                transaction_report.fts = fts_report_on_connection(&transaction, repair_fts)?;
                transaction.commit().map_err(sqlite_error_from_rusqlite)?;
                Ok(transaction_report)
            })?
        } else {
            let events = self.list_all(true)?;
            let mut report = plan_maintenance(&events, request);
            report.fts = self.fts_report(false)?;
            report
        };

        report.status = maintenance_status(&report);
        Ok(report)
    }

    fn fts_report(&self, repaired: bool) -> TreeRingResult<MaintenanceFtsReport> {
        fts_report_on_connection(&self.connection, repaired)
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
        self.recall_with_options(
            query,
            &RecallOptions {
                project,
                agent_profile,
                workflow_id: None,
                session_id: None,
                scope,
                rings,
                event_types,
                include_sensitive,
                include_superseded,
                limit,
                explain_ranking,
            },
        )
    }

    pub fn recall_with_options(
        &self,
        query: &str,
        options: &RecallOptions<'_>,
    ) -> TreeRingResult<Vec<RecallResult>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut candidates = Vec::new();
        let mut seen_queries = HashSet::new();
        let candidate_limit = Some(options.limit.saturating_mul(128).clamp(256, 2048));
        for search_query in search_queries(query) {
            if !seen_queries.insert(search_query.clone()) {
                continue;
            }
            candidates = self.store.search_text_filtered_with_context_limited(
                &search_query,
                options.project,
                options.agent_profile,
                options.workflow_id,
                options.session_id,
                options.scope,
                options.rings,
                options.event_types,
                options.include_sensitive,
                options.include_superseded,
                candidate_limit,
            )?;
            if !candidates.is_empty() {
                break;
            }
        }
        if candidates.is_empty() {
            if let Some(fts_query) = format_plain_text_fts_or_query(query) {
                candidates = self.store.search_fts_filtered_limited(
                    &fts_query,
                    options.project,
                    options.agent_profile,
                    options.workflow_id,
                    options.session_id,
                    options.scope,
                    options.rings,
                    options.event_types,
                    options.include_sensitive,
                    options.include_superseded,
                    candidate_limit,
                )?;
            }
        }

        let mut results: Vec<RecallResult> = candidates
            .into_iter()
            .filter(|event| {
                matches_filters(
                    event,
                    options.project,
                    options.agent_profile,
                    options.workflow_id,
                    options.session_id,
                    options.scope,
                    options.rings,
                    options.event_types,
                    options.include_sensitive,
                )
            })
            .map(|memory| {
                let scored = RecallScorer::score(&memory, query);
                RecallResult {
                    memory,
                    score: scored.score,
                    ranking: if options.explain_ranking {
                        scored.ranking.factors
                    } else {
                        Default::default()
                    },
                }
            })
            .collect();
        results.sort_by(|left, right| right.score.total_cmp(&left.score));
        results.truncate(options.limit);
        Ok(results)
    }
}

#[allow(clippy::too_many_arguments)]
fn matches_filters(
    event: &MemoryEvent,
    project: Option<&str>,
    agent_profile: Option<&str>,
    workflow_id: Option<&str>,
    session_id: Option<&str>,
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
    if workflow_id.is_some_and(|workflow| event.workflow_id.as_deref() != Some(workflow)) {
        return false;
    }
    if session_id.is_some_and(|session| event.session_id.as_deref() != Some(session)) {
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

fn consolidation_supersession_pairs(
    previous_outputs: &[MemoryEvent],
    new_outputs: &[MemoryEvent],
) -> Vec<(String, String)> {
    if new_outputs.is_empty() {
        return Vec::new();
    }
    previous_outputs
        .iter()
        .enumerate()
        .map(|(index, old)| {
            let target = best_consolidation_replacement(old, new_outputs)
                .unwrap_or_else(|| &new_outputs[index % new_outputs.len()]);
            (old.id.clone(), target.id.clone())
        })
        .collect()
}

fn find_memory_by_operation_namespace(
    connection: &Connection,
    event: &MemoryEvent,
    operation_id: &str,
) -> TreeRingResult<Option<MemoryEvent>> {
    let active = connection
        .query_row(
            r#"
            SELECT raw_json
            FROM memories
            WHERE COALESCE(project, '') = COALESCE(?, '')
              AND COALESCE(workflow_id, '') = COALESCE(?, '')
              AND COALESCE(agent_profile, '') = COALESCE(?, '')
              AND operation_id = ?
            LIMIT 1
            "#,
            params![
                event.project.as_deref(),
                event.workflow_id.as_deref(),
                event.agent_profile.as_deref(),
                operation_id
            ],
            search::event_from_row,
        )
        .optional()
        .map_err(sqlite_error_from_rusqlite)?
        .transpose()?;
    if active.is_some() {
        return Ok(active);
    }
    let claim_hash = write::operation_namespace_hash(event, operation_id);
    connection
        .query_row(
            r#"
            SELECT memories.raw_json
            FROM operation_claims
            JOIN memories ON memories.id = operation_claims.memory_id
            WHERE operation_claims.namespace_hash = ?
            LIMIT 1
            "#,
            params![claim_hash.as_slice()],
            search::event_from_row,
        )
        .optional()
        .map_err(sqlite_error_from_rusqlite)?
        .transpose()
}

fn list_all_on_connection(
    connection: &Connection,
    include_superseded: bool,
) -> TreeRingResult<Vec<MemoryEvent>> {
    let sql = if include_superseded {
        "SELECT raw_json FROM memories ORDER BY created_at DESC"
    } else {
        "SELECT raw_json FROM memories WHERE superseded_by IS NULL ORDER BY created_at DESC"
    };
    let mut statement = connection
        .prepare(sql)
        .map_err(sqlite_error_from_rusqlite)?;
    let rows = statement
        .query_map([], search::event_from_row)
        .map_err(sqlite_error_from_rusqlite)?;
    search::collect_rows(rows)
}

fn fts_report_on_connection(
    connection: &Connection,
    repaired: bool,
) -> TreeRingResult<MaintenanceFtsReport> {
    Ok(MaintenanceFtsReport {
        memory_rows: lifecycle::count_query(connection, "SELECT count(*) FROM memories")?,
        fts_rows: lifecycle::count_query(connection, "SELECT count(*) FROM memory_fts")?,
        missing_fts_rows: lifecycle::count_query(
            connection,
            r#"
            SELECT count(*)
            FROM memories
            LEFT JOIN memory_fts ON memories.id = memory_fts.id
            WHERE memory_fts.id IS NULL
            "#,
        )?,
        orphan_fts_rows: lifecycle::count_query(
            connection,
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

fn previous_consolidation_outputs_on_connection(
    connection: &Connection,
    period_type: &str,
    period_key: &str,
    request: &ConsolidationRequest,
) -> TreeRingResult<Vec<MemoryEvent>> {
    let mut statement = connection
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
        let mut parsed =
            serde_json::from_str::<Vec<String>>(&output_ids_json).map_err(TreeRingError::Json)?;
        output_ids.append(&mut parsed);
    }
    drop(statement);

    let mut outputs = Vec::new();
    for output_id in output_ids {
        if let Some(output) = connection
            .query_row(
                "SELECT raw_json FROM memories WHERE id = ?",
                params![output_id],
                search::event_from_row,
            )
            .optional()
            .map_err(sqlite_error_from_rusqlite)?
            .transpose()?
        {
            if consolidation_context_matches(&output, request) {
                outputs.push(output);
            }
        }
    }
    Ok(outputs)
}

fn consolidation_context_matches(event: &MemoryEvent, request: &ConsolidationRequest) -> bool {
    request
        .project
        .as_ref()
        .is_none_or(|project| event.project.as_ref() == Some(project))
        && request
            .agent_profile
            .as_ref()
            .is_none_or(|profile| event.agent_profile.as_ref() == Some(profile))
        && request
            .workflow_id
            .as_ref()
            .is_none_or(|workflow| event.workflow_id.as_ref() == Some(workflow))
        && request
            .session_id
            .as_ref()
            .is_none_or(|session| event.session_id.as_ref() == Some(session))
}

fn insert_consolidation_record(
    connection: &Connection,
    report: &ConsolidationReport,
    source_ids_json: &str,
) -> TreeRingResult<()> {
    let output_ids_json =
        serde_json::to_string(&report.output_memory_ids).map_err(TreeRingError::Json)?;
    connection
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
    Ok(())
}

fn find_consolidation_on_connection(
    connection: &Connection,
    period_type: &str,
    period_key: &str,
    source_ids_json: &str,
) -> TreeRingResult<Option<StoredConsolidation>> {
    connection
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
            lifecycle::stored_consolidation_from_row,
        )
        .optional()
        .map_err(sqlite_error_from_rusqlite)?
        .transpose()
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

fn normalize_legacy_private_scope_rows(
    transaction: &rusqlite::Transaction<'_>,
) -> TreeRingResult<()> {
    let rows = {
        let mut statement = transaction
            .prepare(
                r#"
                SELECT raw_json
                FROM memories
                WHERE scope IN ('agent', 'workflow', 'session')
                "#,
            )
            .map_err(sqlite_error_from_rusqlite)?;
        let mapped = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(sqlite_error_from_rusqlite)?;
        mapped
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite_error_from_rusqlite)?
    };

    for raw_json in rows {
        let mut event: MemoryEvent = serde_json::from_str(&raw_json)?;
        normalize_legacy_private_scope_identity(&mut event)?;
        event.validate()?;
        transaction
            .execute(
                r#"
                UPDATE memories
                SET agent_profile = ?,
                    workflow_id = ?,
                    session_id = ?,
                    review_json = ?,
                    raw_json = ?
                WHERE id = ?
                "#,
                params![
                    event.agent_profile.as_deref(),
                    event.workflow_id.as_deref(),
                    event.session_id.as_deref(),
                    serde_json::to_string(&event.review)?,
                    serde_json::to_string(&event)?,
                    &event.id,
                ],
            )
            .map_err(sqlite_error_from_rusqlite)?;
    }
    Ok(())
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

fn format_plain_text_fts_query(query: &str) -> Option<String> {
    format_plain_text_fts_query_with_operator(query, " AND ")
}

fn format_plain_text_fts_or_query(query: &str) -> Option<String> {
    format_plain_text_fts_query_with_operator(query, " OR ")
}

fn format_plain_text_fts_query_with_operator(query: &str, operator: &str) -> Option<String> {
    let terms: Vec<String> = tree_ring_memory_core::recall::terms(query)
        .into_iter()
        .filter(|term| !SEARCH_FILLER_TERMS.contains(&term.as_str()))
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() {
        return None;
    }
    Some(terms.join(operator))
}

const SEARCH_FILLER_TERMS: &[&str] = &[
    "a", "an", "and", "about", "are", "for", "in", "is", "not", "of", "on", "or", "the", "to",
    "what",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, Barrier},
        thread,
    };
    use tempfile::tempdir;
    use tree_ring_memory_core::models::MemorySource;

    #[test]
    fn public_store_facade_still_covers_write_search_export_import_and_maintenance() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        let mut store = SQLiteMemoryStore::open(&db_path).unwrap();
        let event = MemoryEvent::new("Facade preservation memory.", "lesson").unwrap();
        store.put(&event).unwrap();

        assert!(store.get(&event.id).unwrap().is_some());
        assert_eq!(
            store
                .search_text("facade preservation", false)
                .unwrap()
                .len(),
            1
        );

        let (jsonl, export_report) = store.export_jsonl(false, false).unwrap();
        assert_eq!(export_report.memory_count, 1);

        let target_dir = tempdir().unwrap();
        let mut target = SQLiteMemoryStore::open(target_dir.path().join("memory.sqlite")).unwrap();
        let import_report = target.import_jsonl(&jsonl, false, false).unwrap();
        assert_eq!(import_report.inserted_count, 1);

        let audit = store.audit("all").unwrap();
        assert_eq!(audit.memory_count, 1);

        let maintenance = store.maintain(&MaintenanceRequest::default()).unwrap();
        assert_eq!(maintenance.memory_count, 1);
    }

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
    fn read_only_store_sees_committed_rows_in_live_wal() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        let mut writer = SQLiteMemoryStore::open(&db_path).unwrap();
        writer
            .connection()
            .execute_batch("PRAGMA wal_autocheckpoint=0; PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        let event = MemoryEvent::new("Visible from the live WAL.", "lesson").unwrap();

        writer.put(&event).unwrap();

        let wal_path = db_path.with_extension("sqlite-wal");
        assert!(std::fs::metadata(&wal_path).unwrap().len() > 0);
        let reader = SQLiteMemoryStore::open_read_only(&db_path).unwrap();
        assert_eq!(reader.get(&event.id).unwrap().unwrap().id, event.id);
        let query_only: i64 = reader
            .connection()
            .query_row("PRAGMA query_only", [], |row| row.get(0))
            .unwrap();
        assert_eq!(query_only, 1);
        assert!(reader
            .connection()
            .execute("CREATE TABLE forbidden_write (id INTEGER)", [])
            .is_err());
    }

    #[test]
    fn migrate_adds_multi_agent_columns_and_indexes_to_legacy_database() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        create_legacy_database(&db_path);

        let store = SQLiteMemoryStore::open(&db_path).unwrap();

        for column in ["workflow_id", "session_id", "operation_id"] {
            assert!(schema::memory_column_exists(store.connection(), column).unwrap());
        }
        let indexes: HashSet<String> = {
            let mut statement = store
                .connection()
                .prepare("SELECT name FROM sqlite_master WHERE type = 'index'")
                .unwrap();
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        for index in [
            "idx_memories_project",
            "idx_memories_agent_profile",
            "idx_memories_workflow_id",
            "idx_memories_session_id",
            "idx_memories_operation_namespace",
        ] {
            assert!(indexes.contains(index), "missing index {index}");
        }
    }

    #[test]
    fn migration_normalizes_and_round_trips_identity_less_legacy_private_scope() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        create_legacy_database(&db_path);
        let mut legacy =
            MemoryEvent::new("Legacy workflow memory remains portable.", "lesson").unwrap();
        legacy.id = "mem_legacy_workflow_portability".to_string();
        legacy.scope = "workflow".to_string();
        insert_legacy_event(&db_path, &legacy);

        let store = SQLiteMemoryStore::open(&db_path).unwrap();
        let migrated = store.get(&legacy.id).unwrap().unwrap();
        let workflow_id = migrated.workflow_id.as_deref().unwrap();
        assert!(workflow_id.starts_with("legacy-workflow-"));
        assert!(migrated.review.needs_review);
        let stored_workflow_id: String = store
            .connection()
            .query_row(
                "SELECT workflow_id FROM memories WHERE id = ?",
                params![&legacy.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_workflow_id, workflow_id);

        let (jsonl, report) = store.export_jsonl(false, false).unwrap();
        assert_eq!(report.memory_count, 1);
        let target_dir = tempdir().unwrap();
        let mut target = SQLiteMemoryStore::open(target_dir.path().join("memory.sqlite")).unwrap();
        assert_eq!(
            target
                .import_jsonl(&jsonl, false, false)
                .unwrap()
                .inserted_count,
            1
        );
        assert_eq!(
            target
                .get(&legacy.id)
                .unwrap()
                .unwrap()
                .workflow_id
                .as_deref(),
            Some(workflow_id)
        );
    }

    #[test]
    fn migration_normalizes_blank_legacy_agent_identity() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        create_legacy_database(&db_path);
        let mut legacy =
            MemoryEvent::new("Legacy blank agent identity is normalized.", "lesson").unwrap();
        legacy.id = "mem_legacy_blank_agent_migration".to_string();
        legacy.scope = "agent".to_string();
        legacy.agent_profile = Some(" \t ".to_string());
        insert_legacy_event(&db_path, &legacy);

        let store = SQLiteMemoryStore::open(&db_path).unwrap();
        let migrated = store.get(&legacy.id).unwrap().unwrap();

        assert!(migrated
            .agent_profile
            .as_deref()
            .is_some_and(|identity| identity.starts_with("legacy-agent-")));
        assert!(migrated.review.needs_review);
        migrated.validate().unwrap();
    }

    #[test]
    fn read_only_legacy_rows_are_normalized_in_memory_without_migration() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        create_legacy_database(&db_path);
        let mut legacy = MemoryEvent::new(
            "Legacy session memory is readable without mutation.",
            "lesson",
        )
        .unwrap();
        legacy.id = "mem_legacy_read_only_session".to_string();
        legacy.scope = "session".to_string();
        insert_legacy_event(&db_path, &legacy);

        let store = SQLiteMemoryStore::open_read_only(&db_path).unwrap();
        let loaded = store.get(&legacy.id).unwrap().unwrap();

        assert!(loaded
            .session_id
            .as_deref()
            .is_some_and(|identity| identity.starts_with("legacy-session-")));
        assert!(loaded.review.needs_review);
        assert!(store.export_jsonl(false, false).is_ok());
        assert!(!schema::memory_column_exists(store.connection(), "session_id").unwrap());
    }

    #[test]
    fn concurrent_open_serializes_legacy_schema_migration() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        create_legacy_database(&db_path);
        let barrier = Arc::new(Barrier::new(3));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let db_path = db_path.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    SQLiteMemoryStore::open(db_path)
                })
            })
            .collect();

        barrier.wait();
        for handle in handles {
            handle.join().unwrap().unwrap();
        }

        let store = SQLiteMemoryStore::open(&db_path).unwrap();
        for column in ["workflow_id", "session_id", "operation_id"] {
            assert!(schema::memory_column_exists(store.connection(), column).unwrap());
        }
    }

    #[test]
    fn current_schema_open_does_not_wait_for_writer_lock() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        let mut writer = SQLiteMemoryStore::open(&db_path).unwrap();
        assert_eq!(
            schema::user_version(writer.connection()).unwrap(),
            SQLITE_SCHEMA_VERSION
        );
        let transaction = writer
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        let handle = thread::spawn(move || {
            let result = SQLiteMemoryStore::open(&db_path)
                .and_then(|store| schema::user_version(store.connection()));
            result_tx.send(result).unwrap();
        });

        let opened_version = result_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("current-schema open waited for the writer lock")
            .unwrap();
        assert_eq!(opened_version, SQLITE_SCHEMA_VERSION);
        transaction.commit().unwrap();
        handle.join().unwrap();
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
    fn put_idempotent_returns_existing_without_replacing_a_different_id() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut first = MemoryEvent::new("First operation result.", "lesson").unwrap();
        first.project = Some("core".to_string());
        first.workflow_id = Some("workflow-1".to_string());
        first.agent_profile = Some("agent-a".to_string());
        first.operation_id = Some("operation-1".to_string());
        let mut duplicate = first.clone();
        duplicate.id = "mem_duplicate_operation".to_string();
        duplicate.summary = "A retry with a different id.".to_string();

        assert_eq!(store.put_idempotent(&first).unwrap(), PutOutcome::Created);
        let outcome = store.put_idempotent(&duplicate).unwrap();

        match outcome {
            PutOutcome::Existing(existing) => assert_eq!(existing.id, first.id),
            PutOutcome::Created => panic!("duplicate operation unexpectedly created a row"),
        }
        assert!(store.get(&duplicate.id).unwrap().is_none());
        assert!(store.put(&duplicate).is_err());
        assert_eq!(
            store.get(&first.id).unwrap().unwrap().summary,
            "First operation result."
        );
    }

    #[test]
    fn concurrent_put_idempotent_claims_operation_once() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        SQLiteMemoryStore::open(&db_path).unwrap();
        let barrier = Arc::new(Barrier::new(3));
        let handles: Vec<_> = (0..2)
            .map(|index| {
                let db_path = db_path.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    let mut event =
                        MemoryEvent::new(format!("Concurrent operation {index}."), "lesson")
                            .unwrap();
                    event.project = Some("core".to_string());
                    event.workflow_id = Some("workflow-1".to_string());
                    event.agent_profile = Some("agent-a".to_string());
                    event.operation_id = Some("operation-1".to_string());
                    let mut store = SQLiteMemoryStore::open(db_path).unwrap();
                    barrier.wait();
                    store.put_idempotent(&event).unwrap()
                })
            })
            .collect();

        barrier.wait();
        let outcomes: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();

        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(outcome, PutOutcome::Created))
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(outcome, PutOutcome::Existing(_)))
                .count(),
            1
        );
        let store = SQLiteMemoryStore::open(&db_path).unwrap();
        let rows: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM memories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 1);
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
    fn recall_filters_workflow_and_session_before_candidate_limit() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut events = Vec::new();
        for index in 0..300 {
            let mut event = MemoryEvent::new(
                format!("Workflow partition bottleneck distractor {index}"),
                "lesson",
            )
            .unwrap();
            event.workflow_id = Some("other-workflow".to_string());
            event.session_id = Some("target-session".to_string());
            events.push(event);
        }
        let mut workflow_target =
            MemoryEvent::new("Workflow partition bottleneck target", "lesson").unwrap();
        workflow_target.workflow_id = Some("target-workflow".to_string());
        workflow_target.session_id = Some("target-session".to_string());
        let workflow_target_id = workflow_target.id.clone();
        events.push(workflow_target);
        for index in 0..300 {
            let mut event = MemoryEvent::new(
                format!("Session partition bottleneck distractor {index}"),
                "lesson",
            )
            .unwrap();
            event.workflow_id = Some("target-workflow".to_string());
            event.session_id = Some("other-session".to_string());
            events.push(event);
        }
        let mut session_target =
            MemoryEvent::new("Session partition bottleneck target", "lesson").unwrap();
        session_target.workflow_id = Some("target-workflow".to_string());
        session_target.session_id = Some("target-session".to_string());
        let session_target_id = session_target.id.clone();
        events.push(session_target);
        store.put_many(&events).unwrap();
        let options = RecallOptions {
            workflow_id: Some("target-workflow"),
            session_id: Some("target-session"),
            limit: 1,
            ..RecallOptions::default()
        };

        let workflow_results = MemoryRetriever::new(&store)
            .recall_with_options("workflow partition bottleneck", &options)
            .unwrap();
        let session_results = MemoryRetriever::new(&store)
            .recall_with_options("session partition bottleneck", &options)
            .unwrap();

        assert_eq!(workflow_results[0].memory.id, workflow_target_id);
        assert_eq!(session_results[0].memory.id, session_target_id);
    }

    #[test]
    fn recall_strict_all_term_match_wins_before_fallback() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut partial =
            MemoryEvent::new("Cache invalidation rollout reminder.", "lesson").unwrap();
        partial.salience = 1.0;
        partial.confidence = 1.0;
        let partial_id = partial.id.clone();
        let mut strict =
            MemoryEvent::new("Cache invalidation canary rollout reminder.", "lesson").unwrap();
        strict.salience = 0.1;
        strict.confidence = 0.1;
        let strict_id = strict.id.clone();
        store.put(&partial).unwrap();
        store.put(&strict).unwrap();

        let results = MemoryRetriever::new(&store)
            .recall(
                "cache invalidation canary",
                None,
                None,
                None,
                None,
                None,
                false,
                false,
                8,
                false,
            )
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory.id, strict_id);
        assert!(!results.iter().any(|result| result.memory.id == partial_id));
    }

    #[test]
    fn recall_falls_back_to_partial_term_match_after_strict_misses() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let target = MemoryEvent::new(
            "Quality adapter recalled approved fixture evidence.",
            "lesson",
        )
        .unwrap();
        let target_id = target.id.clone();
        store.put(&target).unwrap();

        let results = MemoryRetriever::new(&store)
            .recall(
                "quality adapter missing transcript",
                None,
                None,
                None,
                None,
                None,
                false,
                false,
                8,
                false,
            )
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].memory.id, target_id);
    }

    #[test]
    fn recall_fallback_keeps_default_filters_before_candidate_limit() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut events = Vec::new();
        for index in 0..300 {
            let mut event = MemoryEvent::new(
                format!("Workflow fallback recall distractor {index}"),
                "lesson",
            )
            .unwrap();
            event.project = Some("other-project".to_string());
            events.push(event);
        }
        let mut sensitive =
            MemoryEvent::new("Workflow fallback recall sensitive target", "lesson").unwrap();
        sensitive.project = Some("target-project".to_string());
        sensitive.sensitivity = "private".to_string();
        events.push(sensitive);
        let mut superseded =
            MemoryEvent::new("Workflow fallback recall superseded target", "lesson").unwrap();
        superseded.project = Some("target-project".to_string());
        superseded.superseded_by = Some("replacement-memory".to_string());
        events.push(superseded);
        let mut target = MemoryEvent::new("Workflow fallback recall target", "lesson").unwrap();
        target.project = Some("target-project".to_string());
        let target_id = target.id.clone();
        events.push(target);
        store.put_many(&events).unwrap();

        let results = MemoryRetriever::new(&store)
            .recall(
                "workflow fallback recall impossible",
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
    fn redaction_preserves_operation_claim_against_exact_retry() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut event = MemoryEvent::new("Payload that must stay forgotten.", "lesson").unwrap();
        event.project = Some("core".to_string());
        event.workflow_id = Some("workflow-1".to_string());
        event.agent_profile = Some("agent-a".to_string());
        event.operation_id = Some("operation-1".to_string());
        assert_eq!(store.put_idempotent(&event).unwrap(), PutOutcome::Created);

        store.redact(&event.id).unwrap();
        let mut retry = event.clone();
        retry.id = "mem_exact_retry_after_redaction".to_string();
        let outcome = store.put_idempotent(&retry).unwrap();

        match outcome {
            PutOutcome::Existing(existing) => {
                assert_eq!(existing.id, event.id);
                assert_eq!(existing.summary, "[REDACTED]");
                assert!(existing.operation_id.is_none());
            }
            PutOutcome::Created => panic!("redacted operation claim was unexpectedly released"),
        }
        assert!(store.get(&retry.id).unwrap().is_none());
        assert!(store.put(&retry).is_err());
        let scrubbed_columns: [Option<String>; 5] = store
            .connection()
            .query_row(
                r#"
                SELECT project, agent_profile, workflow_id, session_id, operation_id
                FROM memories
                WHERE id = ?
                "#,
                params![&event.id],
                |row| {
                    Ok([
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ])
                },
            )
            .unwrap();
        assert_eq!(scrubbed_columns, [None, None, None, None, None]);
        let (claim_hash, claim_memory_id): (Vec<u8>, String) = store
            .connection()
            .query_row(
                "SELECT namespace_hash, memory_id FROM operation_claims",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(claim_hash.len(), 32);
        assert_eq!(claim_memory_id, event.id);
        assert!(!claim_hash
            .windows("operation-1".len())
            .any(|window| window == b"operation-1"));
        let rows: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM memories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }

    #[test]
    fn redacted_id_cannot_be_resurrected_by_put_or_replacement_import() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let original = MemoryEvent::new(
            "Payload that replacement import must not restore.",
            "lesson",
        )
        .unwrap();
        store.put(&original).unwrap();
        store.redact(&original.id).unwrap();

        let mut replacement = original.clone();
        replacement.summary = "Resurrected payload.".to_string();
        assert!(store.put(&replacement).is_err());

        let jsonl = encode_jsonl(&[replacement], false).unwrap();
        assert!(store.import_jsonl(&jsonl, false, true).is_err());

        let retained = store.get(&original.id).unwrap().unwrap();
        assert_eq!(retained.summary, "[REDACTED]");
        assert_eq!(retained.event_type, "redacted");
        let tombstones: i64 = store
            .connection()
            .query_row(
                "SELECT count(*) FROM redaction_tombstones WHERE memory_id = ?",
                params![&original.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tombstones, 1);
    }

    #[test]
    fn replacing_active_operation_preserves_old_namespace_until_hard_delete() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut original = MemoryEvent::new("Original operation payload.", "lesson").unwrap();
        original.project = Some("core".to_string());
        original.workflow_id = Some("workflow-1".to_string());
        original.agent_profile = Some("agent-a".to_string());
        original.operation_id = Some("operation-a".to_string());
        store.put(&original).unwrap();

        let mut replacement = original.clone();
        replacement.summary = "Replacement operation payload.".to_string();
        replacement.operation_id = Some("operation-b".to_string());
        store.put(&replacement).unwrap();

        let mut retry = original.clone();
        retry.id = "mem_retry_replaced_operation".to_string();
        let outcome = store.put_idempotent(&retry).unwrap();
        match outcome {
            PutOutcome::Existing(existing) => {
                assert_eq!(existing.id, original.id);
                assert_eq!(existing.operation_id.as_deref(), Some("operation-b"));
            }
            PutOutcome::Created => panic!("replaced operation namespace was unexpectedly released"),
        }
        assert!(store.get(&retry.id).unwrap().is_none());

        store.delete(&original.id).unwrap();
        assert_eq!(store.put_idempotent(&retry).unwrap(), PutOutcome::Created);
    }

    #[test]
    fn hard_delete_removes_redacted_operation_claim() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut event = MemoryEvent::new("Deleted operation payload.", "lesson").unwrap();
        event.project = Some("core".to_string());
        event.operation_id = Some("operation-delete".to_string());
        store.put(&event).unwrap();
        store.redact(&event.id).unwrap();

        store.delete(&event.id).unwrap();

        let claims: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM operation_claims", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(claims, 0);
        let tombstones: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM redaction_tombstones", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(tombstones, 0);
        let mut retry = event.clone();
        retry.id = "mem_retry_after_hard_delete".to_string();
        assert_eq!(store.put_idempotent(&retry).unwrap(), PutOutcome::Created);
    }

    #[test]
    fn concurrent_redact_and_supersede_preserve_both_monotonic_changes() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        let mut setup = SQLiteMemoryStore::open(&db_path).unwrap();
        let event = MemoryEvent::new("Secret payload must not be resurrected.", "lesson").unwrap();
        let event_id = event.id.clone();
        setup.put(&event).unwrap();
        drop(setup);

        let barrier = Arc::new(Barrier::new(3));
        let redact_handle = {
            let db_path = db_path.clone();
            let event_id = event_id.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let mut store = SQLiteMemoryStore::open(db_path).unwrap();
                barrier.wait();
                store.redact(&event_id).unwrap();
            })
        };
        let supersede_handle = {
            let db_path = db_path.clone();
            let event_id = event_id.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let mut store = SQLiteMemoryStore::open(db_path).unwrap();
                barrier.wait();
                store.supersede(&event_id, "mem_replacement").unwrap();
            })
        };

        barrier.wait();
        redact_handle.join().unwrap();
        supersede_handle.join().unwrap();

        let store = SQLiteMemoryStore::open(&db_path).unwrap();
        let updated = store.get(&event_id).unwrap().unwrap();
        assert_eq!(updated.summary, "[REDACTED]");
        assert_eq!(updated.superseded_by.as_deref(), Some("mem_replacement"));
        assert!(!serde_json::to_string(&updated)
            .unwrap()
            .contains("Secret payload"));
    }

    #[test]
    fn concurrent_ring_change_reloads_redacted_row_under_write_lock() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        let mut setup = SQLiteMemoryStore::open(&db_path).unwrap();
        let event = MemoryEvent::new("Sensitive payload must stay gone.", "lesson").unwrap();
        let event_id = event.id.clone();
        setup.put(&event).unwrap();
        drop(setup);

        let barrier = Arc::new(Barrier::new(3));
        let redact_handle = {
            let db_path = db_path.clone();
            let event_id = event_id.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let mut store = SQLiteMemoryStore::open(db_path).unwrap();
                barrier.wait();
                store.redact(&event_id).unwrap();
            })
        };
        let promote_handle = {
            let db_path = db_path.clone();
            let event_id = event_id.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let mut store = SQLiteMemoryStore::open(db_path).unwrap();
                barrier.wait();
                store
                    .change_ring(&event_id, "heartwood", "decision")
                    .unwrap();
            })
        };

        barrier.wait();
        redact_handle.join().unwrap();
        promote_handle.join().unwrap();

        let store = SQLiteMemoryStore::open(&db_path).unwrap();
        let updated = store.get(&event_id).unwrap().unwrap();
        assert_eq!(updated.summary, "[REDACTED]");
        assert_eq!(updated.ring, "heartwood");
        assert_eq!(updated.retention, "durable");
        assert!(!serde_json::to_string(&updated)
            .unwrap()
            .contains("Sensitive payload"));
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
    fn import_rolls_back_all_rows_when_operation_namespace_conflicts() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut existing = MemoryEvent::new("Existing operation claimant.", "lesson").unwrap();
        existing.project = Some("core".to_string());
        existing.workflow_id = Some("workflow-1".to_string());
        existing.agent_profile = Some("agent-a".to_string());
        existing.operation_id = Some("operation-1".to_string());
        store.put(&existing).unwrap();
        let new_event = MemoryEvent::new("This import must roll back.", "lesson").unwrap();
        let mut conflict = existing.clone();
        conflict.id = "mem_import_operation_conflict".to_string();
        conflict.summary = "Conflicting imported operation.".to_string();
        let jsonl = encode_jsonl(&[new_event.clone(), conflict.clone()], false).unwrap();

        assert!(store.import_jsonl(&jsonl, false, false).is_err());

        assert!(store.get(&new_event.id).unwrap().is_none());
        assert!(store.get(&conflict.id).unwrap().is_none());
        assert_eq!(
            store.get(&existing.id).unwrap().unwrap().summary,
            "Existing operation claimant."
        );
    }

    #[test]
    fn import_cannot_bypass_redacted_operation_claim() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut redacted = MemoryEvent::new("Original operation payload.", "lesson").unwrap();
        redacted.project = Some("core".to_string());
        redacted.workflow_id = Some("workflow-1".to_string());
        redacted.agent_profile = Some("agent-a".to_string());
        redacted.operation_id = Some("operation-1".to_string());
        store.put(&redacted).unwrap();
        store.redact(&redacted.id).unwrap();

        let first = MemoryEvent::new("Earlier import row must roll back.", "lesson").unwrap();
        let mut retry = redacted.clone();
        retry.id = "mem_import_retry_after_redaction".to_string();
        let jsonl = encode_jsonl(&[first.clone(), retry.clone()], false).unwrap();

        assert!(store.import_jsonl(&jsonl, false, false).is_err());

        assert!(store.get(&first.id).unwrap().is_none());
        assert!(store.get(&retry.id).unwrap().is_none());
        assert_eq!(
            store.get(&redacted.id).unwrap().unwrap().summary,
            "[REDACTED]"
        );
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
            agent_profile: None,
            workflow_id: None,
            session_id: None,
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
            agent_profile: None,
            workflow_id: None,
            session_id: None,
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
            agent_profile: None,
            workflow_id: None,
            session_id: None,
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
    fn concurrent_consolidation_claim_creates_one_record_and_output_set() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        let mut setup = SQLiteMemoryStore::open(&db_path).unwrap();
        let mut event =
            MemoryEvent::new("Consolidate this source exactly once.", "decision").unwrap();
        event.project = Some("core".to_string());
        setup.put(&event).unwrap();
        drop(setup);

        let mut request = ConsolidationRequest::new("manual").unwrap();
        request.period_key = Some("manual-concurrent".to_string());
        request.project = Some("core".to_string());
        let barrier = Arc::new(Barrier::new(3));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let db_path = db_path.clone();
                let request = request.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    let mut store = SQLiteMemoryStore::open(db_path).unwrap();
                    barrier.wait();
                    store.consolidate(&request).unwrap()
                })
            })
            .collect();

        barrier.wait();
        let reports: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();

        assert_eq!(
            reports
                .iter()
                .filter(|report| report.status == "created")
                .count(),
            1
        );
        assert_eq!(
            reports
                .iter()
                .filter(|report| report.status == "unchanged")
                .count(),
            1
        );
        assert_eq!(reports[0].output_memory_ids, reports[1].output_memory_ids);
        let store = SQLiteMemoryStore::open(&db_path).unwrap();
        let records: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM consolidations", [], |row| row.get(0))
            .unwrap();
        let rows: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM memories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(records, 1);
        assert_eq!(rows as usize, 1 + reports[0].output_memory_ids.len());
    }

    #[test]
    fn consolidation_reloads_sources_after_waiting_for_concurrent_redaction() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        let mut writer = SQLiteMemoryStore::open(&db_path).unwrap();
        let mut event =
            MemoryEvent::new("Source being redacted before consolidation.", "lesson").unwrap();
        event.project = Some("core".to_string());
        event.tags = vec!["stale_sensitive_marker".to_string()];
        writer.put(&event).unwrap();
        let mut consolidator = SQLiteMemoryStore::open(&db_path).unwrap();
        let mut request = ConsolidationRequest::new("manual").unwrap();
        request.period_key = Some("manual-redaction-race".to_string());
        request.project = Some("core".to_string());

        let transaction = writer
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        write::redact_in_transaction(&transaction, &event.id).unwrap();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let handle = thread::spawn(move || {
            started_tx.send(()).unwrap();
            consolidator.consolidate(&request).unwrap()
        });
        started_rx.recv().unwrap();
        thread::sleep(std::time::Duration::from_millis(100));
        transaction.commit().unwrap();

        let report = handle.join().unwrap();
        let store = SQLiteMemoryStore::open(&db_path).unwrap();
        for output_id in report.output_memory_ids {
            let output = store.get(&output_id).unwrap().unwrap();
            assert!(!output
                .tags
                .iter()
                .any(|tag| tag == "stale_sensitive_marker"));
            assert!(!serde_json::to_string(&output)
                .unwrap()
                .contains("stale_sensitive_marker"));
        }
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
            agent_profile: None,
            workflow_id: None,
            session_id: None,
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
            agent_profile: None,
            workflow_id: None,
            session_id: None,
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
    fn forced_consolidation_does_not_supersede_other_project_outputs() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut project_a = MemoryEvent::new("Project A decision.", "decision").unwrap();
        project_a.project = Some("project-a".to_string());
        let mut project_b = MemoryEvent::new("Project B decision.", "decision").unwrap();
        project_b.project = Some("project-b".to_string());
        store.put(&project_a).unwrap();
        store.put(&project_b).unwrap();
        let mut request_a = ConsolidationRequest::new("manual").unwrap();
        request_a.period_key = Some("manual-project-isolation".to_string());
        request_a.project = Some("project-a".to_string());
        let mut request_b = request_a.clone();
        request_b.project = Some("project-b".to_string());
        let first_a = store.consolidate(&request_a).unwrap();
        let first_b = store.consolidate(&request_b).unwrap();

        request_a.force = true;
        let second_a = store.consolidate(&request_a).unwrap();

        let old_a = store.get(&first_a.output_memory_ids[0]).unwrap().unwrap();
        let old_b = store.get(&first_b.output_memory_ids[0]).unwrap().unwrap();
        assert_eq!(
            old_a.superseded_by.as_deref(),
            Some(second_a.output_memory_ids[0].as_str())
        );
        assert!(old_b.superseded_by.is_none());
    }

    #[test]
    fn forced_consolidation_does_not_supersede_other_workflow_outputs() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let mut workflow_a = MemoryEvent::new("Workflow A decision.", "decision").unwrap();
        workflow_a.project = Some("core".to_string());
        workflow_a.scope = "workflow".to_string();
        workflow_a.workflow_id = Some("workflow-a".to_string());
        let mut workflow_b = MemoryEvent::new("Workflow B decision.", "decision").unwrap();
        workflow_b.project = Some("core".to_string());
        workflow_b.scope = "workflow".to_string();
        workflow_b.workflow_id = Some("workflow-b".to_string());
        store.put(&workflow_a).unwrap();
        store.put(&workflow_b).unwrap();
        let mut request_a = ConsolidationRequest::new("manual").unwrap();
        request_a.period_key = Some("manual-workflow-isolation".to_string());
        request_a.project = Some("core".to_string());
        request_a.workflow_id = Some("workflow-a".to_string());
        let mut request_b = request_a.clone();
        request_b.workflow_id = Some("workflow-b".to_string());
        let first_a = store.consolidate(&request_a).unwrap();
        let first_b = store.consolidate(&request_b).unwrap();

        request_a.force = true;
        let second_a = store.consolidate(&request_a).unwrap();

        let old_a = store.get(&first_a.output_memory_ids[0]).unwrap().unwrap();
        let old_b = store.get(&first_b.output_memory_ids[0]).unwrap().unwrap();
        assert_eq!(
            old_a.superseded_by.as_deref(),
            Some(second_a.output_memory_ids[0].as_str())
        );
        assert!(old_b.superseded_by.is_none());
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
            agent_profile: None,
            workflow_id: None,
            session_id: None,
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
            agent_profile: None,
            workflow_id: None,
            session_id: None,
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
    fn maintenance_replans_after_waiting_for_concurrent_writer() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("memory.sqlite");
        let mut writer = SQLiteMemoryStore::open(&db_path).unwrap();
        let mut maintainer = SQLiteMemoryStore::open(&db_path).unwrap();
        let expired =
            expired_maintenance_memory("Concurrent expired cache", "ephemeral", "cambium");
        let transaction = writer
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        write::put_in_transaction(&transaction, &expired).unwrap();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let handle = thread::spawn(move || {
            started_tx.send(()).unwrap();
            maintainer
                .maintain(&MaintenanceRequest {
                    dry_run: false,
                    apply_expired: true,
                    ..MaintenanceRequest::default()
                })
                .unwrap()
        });
        started_rx.recv().unwrap();
        thread::sleep(std::time::Duration::from_millis(100));
        transaction.commit().unwrap();

        let report = handle.join().unwrap();
        let store = SQLiteMemoryStore::open(&db_path).unwrap();
        assert_eq!(report.applied_action_count, 1);
        assert!(store.get(&expired.id).unwrap().is_none());
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

    fn create_legacy_database(path: &Path) {
        let legacy = Connection::open(path).unwrap();
        legacy
            .execute_batch(
                r#"
                CREATE TABLE memories (
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
                "#,
            )
            .unwrap();
    }

    fn insert_legacy_event(path: &Path, event: &MemoryEvent) {
        let legacy = Connection::open(path).unwrap();
        legacy
            .execute(
                r#"
                INSERT INTO memories (
                  id, created_at, updated_at, project, agent_profile, scope, ring,
                  event_type, summary, details, source_json, tags_json, salience,
                  confidence, sensitivity, retention, expires_at, supersedes_json,
                  superseded_by, links_json, review_json, raw_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
                params![
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
                    serde_json::to_string(&event.source).unwrap(),
                    serde_json::to_string(&event.tags).unwrap(),
                    event.salience,
                    event.confidence,
                    &event.sensitivity,
                    &event.retention,
                    event.expires_at.as_deref(),
                    serde_json::to_string(&event.supersedes).unwrap(),
                    event.superseded_by.as_deref(),
                    serde_json::to_string(&event.links).unwrap(),
                    serde_json::to_string(&event.review).unwrap(),
                    serde_json::to_string(event).unwrap(),
                ],
            )
            .unwrap();
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
