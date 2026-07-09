use std::path::PathBuf;

use tree_ring_memory_core::{
    collect_dox_memories, collect_revolve_memories, DoxSyncReport, DoxSyncRequest,
    RevolveSyncReport, RevolveSyncRequest,
};
use tree_ring_memory_sqlite::SQLiteMemoryStore;

use super::ActionResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoxSyncActionRequest {
    pub source_root: PathBuf,
    pub project: Option<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DoxSyncActionReport {
    pub report: DoxSyncReport,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevolveSyncActionRequest {
    pub source_root: PathBuf,
    pub project: Option<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RevolveSyncActionReport {
    pub report: RevolveSyncReport,
    pub dry_run: bool,
}

pub fn sync_dox(
    store: Option<&mut SQLiteMemoryStore>,
    request: DoxSyncActionRequest,
) -> ActionResult<DoxSyncActionReport> {
    let mut dox_request = DoxSyncRequest::new(request.source_root);
    dox_request.project = request.project;
    let report = collect_dox_memories(&dox_request).map_err(|err| err.to_string())?;
    if !request.dry_run {
        let store = store.ok_or_else(|| {
            "DOX sync action requires an open writable store when dry_run=false".to_string()
        })?;
        store
            .put_many(&report.events)
            .map_err(|err| err.to_string())?;
    }
    Ok(DoxSyncActionReport {
        report,
        dry_run: request.dry_run,
    })
}

pub fn sync_revolve(
    store: Option<&mut SQLiteMemoryStore>,
    request: RevolveSyncActionRequest,
) -> ActionResult<RevolveSyncActionReport> {
    let mut revolve_request = RevolveSyncRequest::new(request.source_root);
    revolve_request.project = request.project;
    let report = collect_revolve_memories(&revolve_request).map_err(|err| err.to_string())?;
    if !request.dry_run {
        let store = store.ok_or_else(|| {
            "Revolve sync action requires an open writable store when dry_run=false".to_string()
        })?;
        store
            .put_many(&report.events)
            .map_err(|err| err.to_string())?;
    }
    Ok(RevolveSyncActionReport {
        report,
        dry_run: request.dry_run,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn dox_action_dry_run_does_not_write_events() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# Rules\n\nAlways run tests.").unwrap();

        let report = sync_dox(
            None,
            DoxSyncActionRequest {
                source_root: dir.path().to_path_buf(),
                project: Some("tree-ring".to_string()),
                dry_run: true,
            },
        )
        .unwrap();

        assert_eq!(report.report.memory_count, 1);
        assert!(!dir.path().join("memory.sqlite").exists());
    }
}
