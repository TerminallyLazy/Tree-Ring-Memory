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
        apply_dox_preview(store, &report)?;
    }
    Ok(DoxSyncActionReport {
        report,
        dry_run: request.dry_run,
    })
}

/// Persist exactly the candidates returned by a reviewed DOX dry run. Store
/// validation, atomic batch writes, and coordinated policy still apply.
pub fn apply_dox_preview(
    store: &mut SQLiteMemoryStore,
    report: &DoxSyncReport,
) -> ActionResult<()> {
    // Older DOX records have no root provenance. Only a source project's own
    // .tree-ring store can establish that association without guessing which
    // project originally wrote a shared legacy record.
    // Resolve a bare file spelling such as AGENTS.md before taking its parent;
    // its lexical parent is empty, not a canonicalizable project directory.
    let source_root = std::fs::canonicalize(&report.root).ok().and_then(|source| {
        if source.is_file() {
            source.parent().map(|parent| parent.to_path_buf())
        } else {
            Some(source)
        }
    });
    let local_store_project = store.database_path().ok().and_then(|database| {
        // A local-looking directory or database symlink does not establish
        // ownership of an external legacy store. Inspect the resolved target.
        let database = std::fs::canonicalize(database).ok()?;
        let memory_root = database.parent()?;
        if memory_root.file_name()? != ".tree-ring" {
            return None;
        }
        Some(memory_root.parent()?.to_path_buf())
    });
    let allow_legacy_sources = source_root.is_some() && source_root == local_store_project;
    store
        .put_dox_many(&report.events, allow_legacy_sources)
        .map_err(|err| err.to_string())
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

    #[test]
    fn legacy_dox_updates_only_in_its_source_projects_local_store() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# Rules\n\nAlways run tests.").unwrap();
        let preview = sync_dox(
            None,
            DoxSyncActionRequest {
                source_root: dir.path().to_path_buf(),
                project: Some("project".to_string()),
                dry_run: true,
            },
        )
        .unwrap()
        .report;
        let mut legacy = preview.events[0].clone();
        legacy.links.retain(|link| link.link_type != "dox-root");

        for (location, allowed) in [(".tree-ring", true), ("shared-store", false)] {
            let root = dir.path().join(location);
            fs::create_dir(&root).unwrap();
            let mut store = SQLiteMemoryStore::open(root.join("memory.sqlite")).unwrap();
            store.put(&legacy).unwrap();
            let result = apply_dox_preview(&mut store, &preview);
            assert_eq!(result.is_ok(), allowed, "{location}: {result:?}");
            let saved = store.list_all(true).unwrap();
            assert_eq!(saved.len(), 1);
            assert_eq!(
                saved[0],
                if allowed {
                    preview.events[0].clone()
                } else {
                    legacy.clone()
                }
            );
        }
    }

    #[test]
    fn shared_store_rejects_distinct_roots_with_the_same_project_name() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("shared.sqlite")).unwrap();
        for (index, parent) in ["first", "second"].into_iter().enumerate() {
            let root = dir.path().join(parent).join("project");
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join("AGENTS.md"), "# Rules\n\nAlways run tests.").unwrap();
            let preview = sync_dox(
                None,
                DoxSyncActionRequest {
                    source_root: root,
                    project: Some("project".to_string()),
                    dry_run: true,
                },
            )
            .unwrap()
            .report;
            let before = store.list_all(true).unwrap();
            let result = apply_dox_preview(&mut store, &preview);
            if index == 0 {
                result.unwrap();
                apply_dox_preview(&mut store, &preview).unwrap();
                assert_eq!(store.list_all(true).unwrap().len(), 1);
            } else {
                assert!(result.is_err());
                assert_eq!(store.list_all(true).unwrap(), before);
            }
        }
    }
}
