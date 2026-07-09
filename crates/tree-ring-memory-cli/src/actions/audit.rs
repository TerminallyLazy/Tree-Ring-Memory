use std::path::Path;

use tree_ring_memory_core::{audit_memories, AuditReport};
use tree_ring_memory_sqlite::SQLiteMemoryStore;

use super::ActionResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditActionRequest {
    pub audit_type: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuditActionReport {
    pub report: AuditReport,
}

pub fn audit_store(db_path: &Path, request: AuditActionRequest) -> ActionResult<AuditActionReport> {
    let report = if db_path.exists() {
        SQLiteMemoryStore::open_read_only(db_path)
            .and_then(|store| store.audit(&request.audit_type))
            .map_err(|err| err.to_string())?
    } else {
        audit_memories(&[], &request.audit_type).map_err(|err| err.to_string())?
    };
    Ok(AuditActionReport { report })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn audit_action_missing_store_reports_empty_audit_without_creating_store() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join(".tree-ring/memory.sqlite");

        let report = audit_store(
            &db_path,
            AuditActionRequest {
                audit_type: "all".to_string(),
            },
        )
        .unwrap();

        assert_eq!(report.report.memory_count, 0);
        assert!(!db_path.exists());
    }
}
