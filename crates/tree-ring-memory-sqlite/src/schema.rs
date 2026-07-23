use rusqlite::{Connection, OpenFlags};
use std::path::Path;

use tree_ring_memory_core::models::{sqlite_error, TreeRingResult};

use crate::sqlite_error_from_rusqlite;

pub(crate) fn open_connection(path: &Path) -> TreeRingResult<Connection> {
    if let Some(parent) = parent_dir_to_create(path) {
        std::fs::create_dir_all(parent).map_err(|err| sqlite_error(err.to_string()))?;
    }
    let connection = Connection::open(path).map_err(sqlite_error_from_rusqlite)?;
    configure_writable_connection(&connection)?;
    Ok(connection)
}

pub(crate) fn open_read_only_connection(path: &Path) -> TreeRingResult<Connection> {
    let path = path
        .canonicalize()
        .map_err(|err| sqlite_error(err.to_string()))?;
    let normalized_path = normalize_sqlite_uri_path(&path.to_string_lossy());
    let uri = format!("file:{}?mode=ro", sqlite_uri_path(&normalized_path));
    let connection = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(sqlite_error_from_rusqlite)?;
    configure_read_only_connection(&connection)?;
    Ok(connection)
}

fn configure_writable_connection(connection: &Connection) -> TreeRingResult<()> {
    connection
        .busy_timeout(std::time::Duration::from_millis(30_000))
        .map_err(sqlite_error_from_rusqlite)?;
    connection
        .execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=30000;",
        )
        .map_err(sqlite_error_from_rusqlite)?;
    Ok(())
}

fn configure_read_only_connection(connection: &Connection) -> TreeRingResult<()> {
    connection
        .busy_timeout(std::time::Duration::from_millis(30_000))
        .map_err(sqlite_error_from_rusqlite)?;
    connection
        .execute_batch("PRAGMA query_only=ON; PRAGMA busy_timeout=30000;")
        .map_err(sqlite_error_from_rusqlite)?;
    Ok(())
}

pub(crate) fn parent_dir_to_create(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

pub(crate) fn normalize_sqlite_uri_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{rest}").replace('\\', "/")
    } else if let Some(rest) = path.strip_prefix("\\\\?\\") {
        rest.replace('\\', "/")
    } else {
        path.replace('\\', "/")
    }
}

pub(crate) fn sqlite_uri_path(path: &str) -> String {
    path.bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'.' | b'_' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

pub(crate) fn memory_column_exists(
    connection: &Connection,
    column_name: &str,
) -> TreeRingResult<bool> {
    let mut statement = connection
        .prepare("PRAGMA table_info(memories)")
        .map_err(sqlite_error_from_rusqlite)?;
    let mut rows = statement.query([]).map_err(sqlite_error_from_rusqlite)?;
    while let Some(row) = rows.next().map_err(sqlite_error_from_rusqlite)? {
        let existing_name: String = row.get(1).map_err(sqlite_error_from_rusqlite)?;
        if existing_name == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn user_version(connection: &Connection) -> TreeRingResult<i64> {
    connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(sqlite_error_from_rusqlite)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
