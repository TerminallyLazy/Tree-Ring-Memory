use rusqlite::{params, OptionalExtension, Transaction};
use sha2::{Digest, Sha256};
use std::time::Duration;

use tree_ring_memory_core::models::{MemoryEvent, TreeRingError, TreeRingResult};
use tree_ring_memory_core::SensitivityGuard;

use crate::search;
use crate::sqlite_error_from_rusqlite;

const WRITE_RETRY_ATTEMPTS: usize = 8;
const WRITE_RETRY_INITIAL_DELAY_MS: u64 = 5;
const WRITE_RETRY_MAX_DELAY_MS: u64 = 100;

pub(crate) const UPSERT_MEMORY_SQL: &str = r#"
    INSERT INTO memories (
      id, created_at, updated_at, project, agent_profile, workflow_id, session_id,
      operation_id, scope, ring, event_type, summary, details, source_json,
      tags_json, salience, confidence, sensitivity, retention, expires_at,
      supersedes_json, superseded_by, links_json, review_json, raw_json
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT(id) DO UPDATE SET
      created_at = excluded.created_at,
      updated_at = excluded.updated_at,
      project = excluded.project,
      agent_profile = excluded.agent_profile,
      workflow_id = excluded.workflow_id,
      session_id = excluded.session_id,
      operation_id = excluded.operation_id,
      scope = excluded.scope,
      ring = excluded.ring,
      event_type = excluded.event_type,
      summary = excluded.summary,
      details = excluded.details,
      source_json = excluded.source_json,
      tags_json = excluded.tags_json,
      salience = excluded.salience,
      confidence = excluded.confidence,
      sensitivity = excluded.sensitivity,
      retention = excluded.retention,
      expires_at = excluded.expires_at,
      supersedes_json = excluded.supersedes_json,
      superseded_by = excluded.superseded_by,
      links_json = excluded.links_json,
      review_json = excluded.review_json,
      raw_json = excluded.raw_json
    "#;

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
    transaction
        .execute(
            "DELETE FROM operation_claims WHERE memory_id = ?",
            params![memory_id],
        )
        .map_err(sqlite_error_from_rusqlite)?;
    transaction
        .execute(
            "DELETE FROM redaction_tombstones WHERE memory_id = ?",
            params![memory_id],
        )
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
    let superseded_by = event.superseded_by.clone();
    event.redact();
    event.superseded_by = superseded_by;
    put_in_transaction(transaction, &event)?;
    transaction
        .execute(
            r#"
            INSERT OR IGNORE INTO redaction_tombstones (memory_id)
            VALUES (?)
            "#,
            params![&event.id],
        )
        .map_err(sqlite_error_from_rusqlite)?;
    Ok(true)
}

pub(crate) fn supersede_in_transaction(
    transaction: &Transaction<'_>,
    old_id: &str,
    new_id: &str,
) -> TreeRingResult<bool> {
    SensitivityGuard::default().check_or_raise(new_id)?;
    let Some(mut event) = transaction
        .query_row(
            "SELECT raw_json FROM memories WHERE id = ?",
            params![old_id],
            search::event_from_row,
        )
        .optional()
        .map_err(sqlite_error_from_rusqlite)?
        .transpose()?
    else {
        return Ok(false);
    };
    event.superseded_by = Some(new_id.to_string());
    let raw_json = serde_json::to_string(&event)?;
    transaction
        .execute(
            "UPDATE memories SET superseded_by = ?, raw_json = ? WHERE id = ?",
            params![new_id, raw_json, old_id],
        )
        .map_err(sqlite_error_from_rusqlite)?;
    Ok(true)
}

pub(crate) fn ensure_operation_claim_available(
    transaction: &Transaction<'_>,
    event: &MemoryEvent,
) -> TreeRingResult<()> {
    let Some(operation_id) = event.operation_id.as_deref() else {
        return Ok(());
    };
    let claim_hash = operation_namespace_hash(event, operation_id);
    let claimed: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM operation_claims WHERE namespace_hash = ?)",
            params![claim_hash.as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite_error_from_rusqlite)?;
    if claimed {
        return Err(TreeRingError::Validation(
            "operation_id was already used by a redacted memory".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn prepare_memory_write(
    transaction: &Transaction<'_>,
    event: &MemoryEvent,
) -> TreeRingResult<()> {
    validate_memory_for_storage(event)?;
    let tombstoned: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM redaction_tombstones WHERE memory_id = ?)",
            params![&event.id],
            |row| row.get(0),
        )
        .map_err(sqlite_error_from_rusqlite)?;
    if tombstoned && !is_sanitized_redaction(event) {
        return Err(TreeRingError::Validation(
            "memory id was redacted and cannot be replaced; hard-delete it before reuse"
                .to_string(),
        ));
    }

    let existing = transaction
        .query_row(
            "SELECT raw_json FROM memories WHERE id = ?",
            params![&event.id],
            search::event_from_row,
        )
        .optional()
        .map_err(sqlite_error_from_rusqlite)?
        .transpose()?;
    if let Some(existing) = existing {
        preserve_replaced_operation_claim(transaction, &existing, event)?;
    }
    ensure_operation_claim_available(transaction, event)
}

pub(crate) fn validate_memory_for_storage(event: &MemoryEvent) -> TreeRingResult<()> {
    SensitivityGuard::default().detect_memory_event_sensitivity(event)?;
    event.validate()
}

pub(crate) fn is_redaction_tombstoned(
    transaction: &Transaction<'_>,
    memory_id: &str,
) -> TreeRingResult<bool> {
    transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM redaction_tombstones WHERE memory_id = ?)",
            params![memory_id],
            |row| row.get(0),
        )
        .map_err(sqlite_error_from_rusqlite)
}

fn preserve_replaced_operation_claim(
    transaction: &Transaction<'_>,
    existing: &MemoryEvent,
    replacement: &MemoryEvent,
) -> TreeRingResult<()> {
    let Some(existing_operation_id) = existing.operation_id.as_deref() else {
        return Ok(());
    };
    let existing_hash = operation_namespace_hash(existing, existing_operation_id);
    let replacement_hash = replacement
        .operation_id
        .as_deref()
        .map(|operation_id| operation_namespace_hash(replacement, operation_id));
    if replacement_hash.as_ref() == Some(&existing_hash) {
        return Ok(());
    }
    transaction
        .execute(
            r#"
            INSERT OR IGNORE INTO operation_claims (namespace_hash, memory_id)
            VALUES (?, ?)
            "#,
            params![existing_hash.as_slice(), &existing.id],
        )
        .map_err(sqlite_error_from_rusqlite)?;
    Ok(())
}

fn is_sanitized_redaction(event: &MemoryEvent) -> bool {
    event.summary == "[REDACTED]"
        && event.details.is_empty()
        && event.project.is_none()
        && event.agent_profile.is_none()
        && event.workflow_id.is_none()
        && event.session_id.is_none()
        && event.operation_id.is_none()
        && event.event_type == "redacted"
        && event.tags.is_empty()
        && event.source.source_type == "manual"
        && event.source.ref_.is_empty()
        && event.source.quote.is_empty()
        && event.supersedes.is_empty()
        && event.links.is_empty()
        && !event.review.needs_review
        && event.review.review_reason.is_none()
        && event.review.reviewed_at.is_none()
        && event.review.reviewed_by.is_none()
        && event.sensitivity == "private"
}

pub(crate) fn operation_namespace_hash(event: &MemoryEvent, operation_id: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"tree-ring-operation-namespace-v1");
    for value in [
        event.project.as_deref(),
        event.workflow_id.as_deref(),
        event.agent_profile.as_deref(),
        Some(operation_id),
    ] {
        match value {
            Some(value) => {
                hasher.update([1]);
                hasher.update((value.len() as u64).to_be_bytes());
                hasher.update(value.as_bytes());
            }
            None => hasher.update([0]),
        }
    }
    hasher.finalize().into()
}

pub(crate) fn put_in_transaction(
    transaction: &Transaction<'_>,
    event: &MemoryEvent,
) -> TreeRingResult<()> {
    prepare_memory_write(transaction, event)?;
    let mut insert_memory = transaction
        .prepare(UPSERT_MEMORY_SQL)
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
            event.workflow_id.as_deref(),
            event.session_id.as_deref(),
            event.operation_id.as_deref(),
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
