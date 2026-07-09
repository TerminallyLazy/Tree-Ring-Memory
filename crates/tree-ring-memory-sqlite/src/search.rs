use rusqlite::{types::Value, Result as SqliteResult, Row};

use tree_ring_memory_core::models::{MemoryEvent, TreeRingResult};

use crate::sqlite_error_from_rusqlite;

pub(crate) fn event_from_row(row: &Row<'_>) -> SqliteResult<TreeRingResult<MemoryEvent>> {
    let raw_json: String = row.get(0)?;
    Ok(serde_json::from_str::<MemoryEvent>(&raw_json).map_err(Into::into))
}

pub(crate) fn collect_rows<I>(rows: I) -> TreeRingResult<Vec<MemoryEvent>>
where
    I: IntoIterator<Item = SqliteResult<TreeRingResult<MemoryEvent>>>,
{
    rows.into_iter()
        .map(|row| {
            row.map_err(sqlite_error_from_rusqlite)
                .and_then(|event| event)
        })
        .collect()
}

pub(crate) fn push_in_filter(
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
