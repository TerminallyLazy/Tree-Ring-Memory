use std::fs;
use std::path::PathBuf;

use serde_json::json;
use tree_ring_memory_core::{decode_jsonl, normalize_import_events};
use tree_ring_memory_sqlite::{ExportReport, ImportReport, SQLiteMemoryStore};

use super::ActionResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportActionRequest {
    pub output: Option<PathBuf>,
    pub include_sensitive: bool,
    pub include_superseded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportActionReport {
    pub jsonl: Option<String>,
    pub output: Option<PathBuf>,
    pub report: ExportReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportActionRequest {
    pub path: PathBuf,
    pub dry_run: bool,
    pub replace_existing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportActionReport {
    pub path: PathBuf,
    pub report: ImportReport,
}

pub fn export_jsonl(
    store: &SQLiteMemoryStore,
    request: ExportActionRequest,
) -> ActionResult<ExportActionReport> {
    let (jsonl, report) = store
        .export_jsonl(request.include_sensitive, request.include_superseded)
        .map_err(|err| err.to_string())?;
    if let Some(output) = request.output {
        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| err.to_string())?;
            }
        }
        fs::write(&output, jsonl).map_err(|err| err.to_string())?;
        Ok(ExportActionReport {
            jsonl: None,
            output: Some(output),
            report,
        })
    } else {
        Ok(ExportActionReport {
            jsonl: Some(jsonl),
            output: None,
            report,
        })
    }
}

pub fn import_jsonl(
    store: Option<&mut SQLiteMemoryStore>,
    request: ImportActionRequest,
) -> ActionResult<ImportActionReport> {
    let input = fs::read_to_string(&request.path).map_err(|err| err.to_string())?;
    let report = if request.dry_run {
        let decoded = decode_jsonl(&input).map_err(|err| err.to_string())?;
        let events = normalize_import_events(decoded.events).map_err(|err| err.to_string())?;
        ImportReport {
            valid_count: events.len(),
            inserted_count: 0,
            replaced_count: 0,
            skipped_duplicate_count: 0,
            dry_run: true,
        }
    } else {
        let store = store.ok_or_else(|| {
            "import action requires an open writable store when dry_run=false".to_string()
        })?;
        store
            .import_jsonl(&input, false, request.replace_existing)
            .map_err(|err| err.to_string())?
    };
    Ok(ImportActionReport {
        path: request.path,
        report,
    })
}

pub fn import_json_payload(report: &ImportActionReport) -> serde_json::Value {
    json!({
        "ok": true,
        "path": report.path,
        "valid_count": report.report.valid_count,
        "inserted_count": report.report.inserted_count,
        "replaced_count": report.report.replaced_count,
        "skipped_duplicate_count": report.report.skipped_duplicate_count,
        "dry_run": report.report.dry_run,
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use tree_ring_memory_core::MemoryEvent;

    use super::*;

    #[test]
    fn export_action_can_return_stdout_jsonl() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        store
            .put(&MemoryEvent::new("Export through shared action.", "lesson").unwrap())
            .unwrap();

        let report = export_jsonl(
            &store,
            ExportActionRequest {
                output: None,
                include_sensitive: false,
                include_superseded: false,
            },
        )
        .unwrap();

        assert_eq!(report.report.memory_count, 1);
        assert!(report.jsonl.unwrap().contains("memory_event"));
    }

    #[test]
    fn import_action_dry_run_validates_without_writing() {
        let dir = tempdir().unwrap();
        let mut source = SQLiteMemoryStore::open(dir.path().join("source.sqlite")).unwrap();
        let target = SQLiteMemoryStore::open(dir.path().join("target.sqlite")).unwrap();
        source
            .put(&MemoryEvent::new("Import through shared action.", "lesson").unwrap())
            .unwrap();
        let (jsonl, _) = source.export_jsonl(false, false).unwrap();
        let input = dir.path().join("input.jsonl");
        fs::write(&input, jsonl).unwrap();

        let report = import_jsonl(
            None,
            ImportActionRequest {
                path: input,
                dry_run: true,
                replace_existing: false,
            },
        )
        .unwrap();

        assert_eq!(report.report.valid_count, 1);
        assert_eq!(target.list_all(true).unwrap().len(), 0);
    }
}
