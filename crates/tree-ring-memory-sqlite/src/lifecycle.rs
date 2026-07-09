use rusqlite::{params, Connection, Row, Transaction};

use tree_ring_memory_core::models::TreeRingResult;

use crate::search;
use crate::sqlite_error_from_rusqlite;
use crate::StoredConsolidation;

pub(crate) fn count_query(connection: &Connection, sql: &str) -> TreeRingResult<usize> {
    let count: i64 = connection
        .query_row(sql, [], |row| row.get(0))
        .map_err(sqlite_error_from_rusqlite)?;
    Ok(count as usize)
}

pub(crate) fn rebuild_fts_in_transaction(transaction: &Transaction<'_>) -> TreeRingResult<()> {
    let events = {
        let mut statement = transaction
            .prepare("SELECT raw_json FROM memories ORDER BY created_at DESC")
            .map_err(sqlite_error_from_rusqlite)?;
        let rows = statement
            .query_map([], search::event_from_row)
            .map_err(sqlite_error_from_rusqlite)?;
        search::collect_rows(rows)?
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

pub(crate) fn stored_consolidation_from_row(
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
