use rusqlite::{params, OptionalExtension, Transaction};
use std::time::Duration;

use tree_ring_memory_core::models::{MemoryEvent, TreeRingError, TreeRingResult};

use crate::search;
use crate::sqlite_error_from_rusqlite;

const WRITE_RETRY_ATTEMPTS: usize = 8;
const WRITE_RETRY_INITIAL_DELAY_MS: u64 = 5;
const WRITE_RETRY_MAX_DELAY_MS: u64 = 100;

pub(crate) fn delete_in_transaction(
    transaction: &Transaction<'_>,
    memory_id: &str,
) -> TreeRingResult<bool> {
    let deleted = transaction
        .execute("DELETE FROM memories WHERE id = ?", params![memory_id])
        .map_err(sqlite_error_from_rusqlite)?;
    transaction
        .execute("DELETE FROM memory_fts WHERE id = ?", params![memory_id])
        .map_err(sqlite_error_from_rusqlite)?;
    Ok(deleted > 0)
}

pub(crate) fn redact_in_transaction(
    transaction: &Transaction<'_>,
    memory_id: &str,
) -> TreeRingResult<bool> {
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
        return Ok(false);
    };
    event.redact();
    put_in_transaction(transaction, &event)?;
    Ok(true)
}

pub(crate) fn put_in_transaction(
    transaction: &Transaction<'_>,
    event: &MemoryEvent,
) -> TreeRingResult<()> {
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

pub(crate) fn put_with_statements(
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

pub(crate) fn retry_locked<T>(
    mut operation: impl FnMut() -> TreeRingResult<T>,
) -> TreeRingResult<T> {
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
