# Tree Ring Shared Action Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create shared action contracts that keep CLI and TUI durable memory operations behavior-stable while reducing coupling in the largest command, TUI, and storage files.

**Architecture:** Add a focused `actions` layer inside `tree-ring-memory-cli` with request/report structs and operation functions consumed by both command handlers and the TUI. Keep `SQLiteMemoryStore` as the public storage facade while splitting private SQLite helpers into responsibility-focused modules only after action seams are established.

**Tech Stack:** Rust 2021, Clap, Ratatui, serde/serde_json, rusqlite with bundled SQLite, existing `tree-ring-memory-core`, existing `tree-ring-memory-sqlite`, cargo tests, `scripts/certify-tree-ring.sh`.

## Global Constraints

- Keep user-facing behavior stable while simplifying internal boundaries.
- Do not add new user-facing product behavior in this lane.
- Do not change the SQLite schema or public JSONL schema.
- Do not change recall ranking, sensitivity classification, adapter summaries, generated guidance wording, installer behavior, or TUI layout.
- Do not add a daemon, MCP server, background recorder, or hidden durable memory writer.
- Keep `SQLiteMemoryStore` as the public storage facade.
- CLI remains responsible for Clap argument parsing, text and JSON presentation, exit behavior, and help text.
- TUI remains responsible for mode transitions, selection state, confirmation panels, rendering, keyboard input, and slash-command input.
- Existing non-mutating dry-run semantics remain non-mutating.
- Run `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked --all-targets`, `git diff --check`, and `sh scripts/certify-tree-ring.sh` before final PR handoff.

---

## Scope Check

This plan covers one subsystem: the shared action foundation and behavior-preserving simplification of CLI/TUI/storage boundaries. The TUI cockpit, harness certification matrix, and recall quality dashboard are not part of this plan.

## File Structure

- Create `crates/tree-ring-memory-cli/src/actions/mod.rs`: shared action module exports, `ActionResult`, and small string-context helper.
- Create `crates/tree-ring-memory-cli/src/actions/remember.rs`: remember request/report and store operation.
- Create `crates/tree-ring-memory-cli/src/actions/recall.rs`: recall request/report and retriever operation.
- Create `crates/tree-ring-memory-cli/src/actions/export_import.rs`: export/import request/report wrappers around store JSONL behavior and dry-run validation.
- Create `crates/tree-ring-memory-cli/src/actions/audit.rs`: audit request/report behavior, including missing-store read-only behavior.
- Create `crates/tree-ring-memory-cli/src/actions/lifecycle.rs`: consolidation and maintenance action behavior.
- Create `crates/tree-ring-memory-cli/src/actions/adapters.rs`: DOX and Revolve sync action behavior.
- Create `crates/tree-ring-memory-cli/src/actions/integrations.rs`: integration scan action behavior.
- Create `crates/tree-ring-memory-cli/src/commands/mod.rs`: command module exports.
- Create `crates/tree-ring-memory-cli/src/commands/scriptable.rs`: CLI command handlers that call shared actions and keep output behavior stable.
- Modify `crates/tree-ring-memory-cli/src/main.rs`: module declarations, thin dispatch, output formatting delegation, and reduced inline operation semantics.
- Modify `crates/tree-ring-memory-cli/src/tui/app.rs`: call shared actions for matching existing TUI operations.
- Modify `crates/tree-ring-memory-cli/src/tui/actions.rs`: keep confirmation models, but store action request values where useful.
- Modify `crates/tree-ring-memory-sqlite/src/lib.rs`: keep public facade, move private helpers out in later tasks.
- Create `crates/tree-ring-memory-sqlite/src/schema.rs`: private open/migration helpers.
- Create `crates/tree-ring-memory-sqlite/src/write.rs`: private write/delete/redact/supersede helpers.
- Create `crates/tree-ring-memory-sqlite/src/search.rs`: private list/search/recall helpers.
- Create `crates/tree-ring-memory-sqlite/src/import_export.rs`: private store JSONL import/export helpers.
- Create `crates/tree-ring-memory-sqlite/src/lifecycle.rs`: private audit/consolidate/maintenance helpers.
- Modify `docs/architecture/rust-core-status.md`: short note that shared action contracts now back CLI/TUI operations.
- Modify `README.md`: only if command examples or verification guidance need a behavior-stable note.

---

### Task 1: Add Shared Action Foundation For Remember

**Files:**
- Create: `crates/tree-ring-memory-cli/src/actions/mod.rs`
- Create: `crates/tree-ring-memory-cli/src/actions/remember.rs`
- Modify: `crates/tree-ring-memory-cli/src/main.rs`

**Interfaces:**
- Produces: `actions::ActionResult<T> = Result<T, String>`
- Produces: `actions::remember::RememberRequest`
- Produces: `actions::remember::RememberReport`
- Produces: `actions::remember::remember(store: &mut SQLiteMemoryStore, request: RememberRequest) -> ActionResult<RememberReport>`
- Consumes: `tree_ring_memory_core::{MemoryEvent, SensitivityGuard}`
- Consumes: `tree_ring_memory_sqlite::SQLiteMemoryStore`

- [ ] **Step 1: Write the failing remember action tests**

Add this complete file at `crates/tree-ring-memory-cli/src/actions/remember.rs`:

```rust
use tree_ring_memory_core::{MemoryEvent, SensitivityGuard};
use tree_ring_memory_sqlite::SQLiteMemoryStore;

use super::ActionResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RememberRequest {
    pub summary: String,
    pub event_type: String,
    pub ring: String,
    pub scope: String,
    pub project: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RememberReport {
    pub memory: MemoryEvent,
}

pub fn remember(
    store: &mut SQLiteMemoryStore,
    request: RememberRequest,
) -> ActionResult<RememberReport> {
    let guard = SensitivityGuard::default();
    let values = [
        request.summary.as_str(),
        request.event_type.as_str(),
        request.ring.as_str(),
        request.scope.as_str(),
    ]
    .into_iter()
    .chain(request.project.iter().map(String::as_str))
    .chain(request.tags.iter().map(String::as_str));
    let detected_sensitivity = guard
        .detect_text_sensitivity(values)
        .map_err(|err| err.to_string())?;
    let mut event =
        MemoryEvent::new(request.summary, request.event_type).map_err(|err| err.to_string())?;
    event.ring = request.ring;
    event.scope = request.scope;
    event.project = request.project;
    event.tags = request.tags;
    if detected_sensitivity != "normal" {
        event.sensitivity = detected_sensitivity;
    }
    event.validate().map_err(|err| err.to_string())?;
    store.put(&event).map_err(|err| err.to_string())?;
    Ok(RememberReport { memory: event })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn remember_action_stores_memory_with_cli_defaults() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();

        let report = remember(
            &mut store,
            RememberRequest {
                summary: "Use shared actions for durable operations.".to_string(),
                event_type: "lesson".to_string(),
                ring: "cambium".to_string(),
                scope: "project".to_string(),
                project: Some("tree-ring".to_string()),
                tags: vec!["refactor".to_string()],
            },
        )
        .unwrap();

        let stored = store.get(&report.memory.id).unwrap().unwrap();
        assert_eq!(stored.summary, "Use shared actions for durable operations.");
        assert_eq!(stored.ring, "cambium");
        assert_eq!(stored.scope, "project");
        assert_eq!(stored.project.as_deref(), Some("tree-ring"));
        assert_eq!(stored.tags, vec!["refactor"]);
    }

    #[test]
    fn remember_action_classifies_sensitive_input_before_storage() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();

        let report = remember(
            &mut store,
            RememberRequest {
                summary: "Private diagnosis should be guarded.".to_string(),
                event_type: "lesson".to_string(),
                ring: "cambium".to_string(),
                scope: "global".to_string(),
                project: None,
                tags: Vec::new(),
            },
        )
        .unwrap();

        let stored = store.get(&report.memory.id).unwrap().unwrap();
        assert_eq!(stored.sensitivity, "private");
    }
}
```

- [ ] **Step 2: Run the focused test and verify module wiring is missing**

Run:

```bash
cargo test -p tree-ring-memory-cli remember_action --locked
```

Expected: FAIL because `super::ActionResult` and the `actions` module are not declared yet.

- [ ] **Step 3: Add the shared action module**

Create `crates/tree-ring-memory-cli/src/actions/mod.rs`:

```rust
pub mod remember;

pub type ActionResult<T> = Result<T, String>;
```

Modify the module declarations near the top of `crates/tree-ring-memory-cli/src/main.rs`:

```rust
mod actions;
mod agent_awareness;
mod integrations;
mod ring_mark;
mod tui;
mod welcome;
```

- [ ] **Step 4: Run the focused test and verify it passes**

Run:

```bash
cargo test -p tree-ring-memory-cli remember_action --locked
```

Expected: PASS with both `remember_action_*` tests passing.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add crates/tree-ring-memory-cli/src/actions/mod.rs crates/tree-ring-memory-cli/src/actions/remember.rs crates/tree-ring-memory-cli/src/main.rs
git commit -m "Add shared remember action"
```

---

### Task 2: Add Recall, Export, Import, And Audit Actions

**Files:**
- Create: `crates/tree-ring-memory-cli/src/actions/recall.rs`
- Create: `crates/tree-ring-memory-cli/src/actions/export_import.rs`
- Create: `crates/tree-ring-memory-cli/src/actions/audit.rs`
- Modify: `crates/tree-ring-memory-cli/src/actions/mod.rs`

**Interfaces:**
- Consumes: `actions::ActionResult`
- Produces: `actions::recall::RecallRequest`
- Produces: `actions::recall::RecallReport`
- Produces: `actions::recall::recall(store: &SQLiteMemoryStore, request: RecallRequest) -> ActionResult<RecallReport>`
- Produces: `actions::export_import::ExportActionRequest`
- Produces: `actions::export_import::ExportActionReport`
- Produces: `actions::export_import::export_jsonl(store: &SQLiteMemoryStore, request: ExportActionRequest) -> ActionResult<ExportActionReport>`
- Produces: `actions::export_import::ImportActionRequest`
- Produces: `actions::export_import::ImportActionReport`
- Produces: `actions::export_import::import_jsonl(store: &mut SQLiteMemoryStore, request: ImportActionRequest) -> ActionResult<ImportActionReport>`
- Produces: `actions::audit::AuditActionRequest`
- Produces: `actions::audit::AuditActionReport`
- Produces: `actions::audit::audit_store(db_path: &Path, request: AuditActionRequest) -> ActionResult<AuditActionReport>`

- [ ] **Step 1: Add recall action tests and implementation**

Create `crates/tree-ring-memory-cli/src/actions/recall.rs`:

```rust
use tree_ring_memory_sqlite::{MemoryRetriever, RecallResult, SQLiteMemoryStore};

use super::ActionResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallRequest {
    pub query: String,
    pub project: Option<String>,
    pub limit: usize,
    pub include_sensitive: bool,
    pub include_superseded: bool,
    pub explain: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecallReport {
    pub results: Vec<RecallResult>,
}

pub fn recall(store: &SQLiteMemoryStore, request: RecallRequest) -> ActionResult<RecallReport> {
    let results = MemoryRetriever::new(store)
        .recall(
            &request.query,
            request.project.as_deref(),
            None,
            None,
            None,
            None,
            request.include_sensitive,
            request.include_superseded,
            request.limit,
            request.explain,
        )
        .map_err(|err| err.to_string())?;
    Ok(RecallReport { results })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use tree_ring_memory_core::MemoryEvent;

    use super::*;

    #[test]
    fn recall_action_returns_ranked_memories() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let event = MemoryEvent::new("Use shared recall action.", "lesson").unwrap();
        store.put(&event).unwrap();

        let report = recall(
            &store,
            RecallRequest {
                query: "shared recall".to_string(),
                project: None,
                limit: 8,
                include_sensitive: false,
                include_superseded: false,
                explain: true,
            },
        )
        .unwrap();

        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].memory.id, event.id);
        assert!(report.results[0].ranking.contains_key("text"));
    }
}
```

- [ ] **Step 2: Add export/import action tests and implementation**

Create `crates/tree-ring-memory-cli/src/actions/export_import.rs`:

```rust
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
    store: &mut SQLiteMemoryStore,
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
        let mut target = SQLiteMemoryStore::open(dir.path().join("target.sqlite")).unwrap();
        source
            .put(&MemoryEvent::new("Import through shared action.", "lesson").unwrap())
            .unwrap();
        let (jsonl, _) = source.export_jsonl(false, false).unwrap();
        let input = dir.path().join("input.jsonl");
        fs::write(&input, jsonl).unwrap();

        let report = import_jsonl(
            &mut target,
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
```

- [ ] **Step 3: Add audit action tests and implementation**

Create `crates/tree-ring-memory-cli/src/actions/audit.rs`:

```rust
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
```

- [ ] **Step 4: Export the new action modules**

Replace `crates/tree-ring-memory-cli/src/actions/mod.rs` with:

```rust
pub mod audit;
pub mod export_import;
pub mod recall;
pub mod remember;

pub type ActionResult<T> = Result<T, String>;
```

- [ ] **Step 5: Run focused action tests**

Run:

```bash
cargo test -p tree-ring-memory-cli actions --locked
```

Expected: PASS with remember, recall, export/import, and audit action tests passing.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add crates/tree-ring-memory-cli/src/actions
git commit -m "Add shared scriptable actions"
```

---

### Task 3: Wire CLI Remember, Recall, Export, Import, And Audit Through Actions

**Files:**
- Create: `crates/tree-ring-memory-cli/src/commands/mod.rs`
- Create: `crates/tree-ring-memory-cli/src/commands/scriptable.rs`
- Modify: `crates/tree-ring-memory-cli/src/main.rs`

**Interfaces:**
- Consumes: `actions::remember::remember`
- Consumes: `actions::recall::recall`
- Consumes: `actions::export_import::{export_jsonl, import_jsonl, import_json_payload}`
- Consumes: `actions::audit::audit_store`
- Produces: `commands::scriptable::print_recall_report(report: actions::recall::RecallReport, json_output: bool) -> Result<(), String>`
- Produces: `commands::scriptable::print_export_report(report: actions::export_import::ExportActionReport, json_output: bool) -> Result<(), String>`
- Produces: `commands::scriptable::print_import_report(report: actions::export_import::ImportActionReport, json_output: bool) -> Result<(), String>`

- [ ] **Step 1: Add CLI parity tests for action-backed commands**

Append these tests to the existing `#[cfg(test)] mod tests` in `crates/tree-ring-memory-cli/src/main.rs`:

```rust
#[test]
fn remember_and_recall_output_stays_stable_after_action_extraction() {
    let dir = tempdir().unwrap();
    let root = dir.path().join(".tree-ring");

    run(Cli::parse_from([
        "tree-ring",
        "--root",
        root.to_str().unwrap(),
        "remember",
        "Use action-backed CLI behavior.",
        "--event-type",
        "lesson",
        "--scope",
        "project",
        "--project",
        "tree-ring",
    ]))
    .unwrap();

    let store = SQLiteMemoryStore::open(root.join("memory.sqlite")).unwrap();
    let memories = store.list_all(false).unwrap();
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].summary, "Use action-backed CLI behavior.");
}

#[test]
fn import_dry_run_still_does_not_create_store_rows_after_action_extraction() {
    let dir = tempdir().unwrap();
    let root = dir.path().join(".tree-ring");
    let source_path = dir.path().join("source.sqlite");
    let mut source = SQLiteMemoryStore::open(&source_path).unwrap();
    source
        .put(&MemoryEvent::new("Dry-run import action parity.", "lesson").unwrap())
        .unwrap();
    let (jsonl, _) = source.export_jsonl(false, false).unwrap();
    let jsonl_path = dir.path().join("memories.jsonl");
    fs::write(&jsonl_path, jsonl).unwrap();

    run(Cli::parse_from([
        "tree-ring",
        "--root",
        root.to_str().unwrap(),
        "import",
        jsonl_path.to_str().unwrap(),
        "--dry-run",
    ]))
    .unwrap();

    assert!(!root.join("memory.sqlite").exists());
}
```

- [ ] **Step 2: Run the parity tests before rewiring**

Run:

```bash
cargo test -p tree-ring-memory-cli action_extraction --locked
```

Expected: PASS before rewiring, proving the tests capture current behavior.

- [ ] **Step 3: Add command output helpers**

Create `crates/tree-ring-memory-cli/src/commands/mod.rs`:

```rust
pub mod scriptable;
```

Create `crates/tree-ring-memory-cli/src/commands/scriptable.rs`:

```rust
use serde_json::json;

use crate::actions::export_import::{
    import_json_payload, ExportActionReport, ImportActionReport,
};
use crate::actions::recall::RecallReport;

pub fn print_recall_report(report: RecallReport, json_output: bool) -> Result<(), String> {
    if json_output {
        let payload: Vec<_> = report
            .results
            .into_iter()
            .map(|result| {
                json!({
                    "memory": result.memory,
                    "score": result.score,
                    "ranking": result.ranking,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string(&payload).map_err(|err| err.to_string())?
        );
    } else {
        for result in report.results {
            println!(
                "{} [{}] {} score={:.3}",
                result.memory.id, result.memory.ring, result.memory.summary, result.score
            );
        }
    }
    Ok(())
}

pub fn print_export_report(
    report: ExportActionReport,
    json_output: bool,
) -> Result<(), String> {
    if let Some(jsonl) = report.jsonl {
        print!("{jsonl}");
        return Ok(());
    }
    let Some(output) = report.output else {
        return Err("export action did not return output path or JSONL".to_string());
    };
    if json_output {
        println!(
            "{}",
            json!({
                "ok": true,
                "path": output,
                "memory_count": report.report.memory_count,
                "sensitive_included": report.report.sensitive_included,
                "superseded_included": report.report.superseded_included,
            })
        );
    } else {
        println!(
            "Tree Ring Memory export complete: {} memories -> {}",
            report.report.memory_count,
            output.display()
        );
    }
    Ok(())
}

pub fn print_import_report(
    report: ImportActionReport,
    json_output: bool,
) -> Result<(), String> {
    if json_output {
        println!("{}", import_json_payload(&report));
    } else {
        println!(
            "Tree Ring Memory import complete: valid={} inserted={} replaced={} skipped_duplicates={} dry_run={}",
            report.report.valid_count,
            report.report.inserted_count,
            report.report.replaced_count,
            report.report.skipped_duplicate_count,
            report.report.dry_run
        );
    }
    Ok(())
}
```

- [ ] **Step 4: Declare the command module**

Modify the module declarations in `crates/tree-ring-memory-cli/src/main.rs`:

```rust
mod actions;
mod agent_awareness;
mod commands;
mod integrations;
mod ring_mark;
mod tui;
mod welcome;
```

- [ ] **Step 5: Replace CLI remember, recall, export, import dry-run, import write, and audit semantics with actions**

Use these imports near the top of `crates/tree-ring-memory-cli/src/main.rs`:

```rust
use actions::audit::{audit_store, AuditActionRequest};
use actions::export_import::{
    export_jsonl as export_action, import_jsonl as import_action, ImportActionRequest,
    ExportActionRequest,
};
use actions::recall::{recall as recall_action, RecallRequest};
use actions::remember::{remember as remember_action, RememberRequest};
```

Replace the import dry-run pre-store branch body with:

```rust
if let Command::Import {
    path,
    dry_run: true,
    replace_existing,
} = cli.command
{
    let mut store = SQLiteMemoryStore::open(&db_path).map_err(|err| err.to_string())?;
    let report = import_action(
        &mut store,
        ImportActionRequest {
            path,
            dry_run: true,
            replace_existing,
        },
    )?;
    commands::scriptable::print_import_report(report, cli.json)?;
    return Ok(());
}
```

Replace the audit pre-store branch body with:

```rust
if let Command::Audit { audit_type } = &cli.command {
    let report = audit_store(
        &db_path,
        AuditActionRequest {
            audit_type: audit_type.clone(),
        },
    )?;
    print_audit_report(&report.report, cli.json)?;
    return Ok(());
}
```

Replace the `Command::Remember` match arm body with:

```rust
let report = remember_action(
    &mut store,
    RememberRequest {
        summary,
        event_type,
        ring,
        scope,
        project,
        tags,
    },
)?;
if cli.json {
    println!(
        "{}",
        serde_json::to_string(&report.memory).map_err(|err| err.to_string())?
    );
} else {
    println!("{}", report.memory.id);
}
```

Replace the `Command::Recall` match arm body with:

```rust
let report = recall_action(
    &store,
    RecallRequest {
        query,
        project,
        limit,
        include_sensitive,
        include_superseded: false,
        explain: false,
    },
)?;
commands::scriptable::print_recall_report(report, cli.json)?;
```

Replace the `Command::Export` match arm body with:

```rust
let report = export_action(
    &store,
    ExportActionRequest {
        output,
        include_sensitive,
        include_superseded,
    },
)?;
commands::scriptable::print_export_report(report, cli.json)?;
```

Replace the `Command::Import` writable match arm body with:

```rust
let report = import_action(
    &mut store,
    ImportActionRequest {
        path,
        dry_run,
        replace_existing,
    },
)?;
commands::scriptable::print_import_report(report, cli.json)?;
```

- [ ] **Step 6: Remove now-unused imports**

Remove these imports from `crates/tree-ring-memory-cli/src/main.rs` when the compiler reports they are unused:

```rust
use std::fs;
use tree_ring_memory_core::{decode_jsonl, normalize_import_events, audit_memories};
use tree_ring_memory_sqlite::MemoryRetriever;
```

Keep `std::fs` if later code in tests or command paths still uses it.

- [ ] **Step 7: Run focused CLI tests**

Run:

```bash
cargo test -p tree-ring-memory-cli remember_json_emits_memory_payload recall_filters action_extraction import_dry_run audit_missing_root --locked
```

Expected: PASS. If the filter selects zero tests for `action_extraction`, run:

```bash
cargo test -p tree-ring-memory-cli remember_and_recall_output_stays_stable_after_action_extraction import_dry_run_still_does_not_create_store_rows_after_action_extraction --locked
```

Expected: PASS.

- [ ] **Step 8: Commit Task 3**

Run:

```bash
git add crates/tree-ring-memory-cli/src/actions crates/tree-ring-memory-cli/src/commands crates/tree-ring-memory-cli/src/main.rs
git commit -m "Wire scriptable CLI through shared actions"
```

---

### Task 4: Update Existing TUI Remember And Export To Use Shared Actions

**Files:**
- Modify: `crates/tree-ring-memory-cli/src/tui/app.rs`
- Modify: `crates/tree-ring-memory-cli/src/tui/actions.rs`

**Interfaces:**
- Consumes: `actions::remember::{remember, RememberRequest}`
- Consumes: `actions::export_import::{export_jsonl, ExportActionRequest}`
- Keeps: `tui::actions::PendingAction`
- Keeps: `tui::app::App::execute_slash_command(&mut self, input: &str) -> Result<(), String>`

- [ ] **Step 1: Add focused TUI parity tests**

Append these tests to `crates/tree-ring-memory-cli/src/tui/app.rs` inside the existing test module:

```rust
#[test]
fn slash_remember_uses_shared_action_and_keeps_status_shape() {
    let dir = tempdir().unwrap();
    let mut app = app(&dir);

    app.execute_slash_command("/remember Use shared TUI remember action")
        .unwrap();

    assert!(app.status.starts_with("remembered mem_"));
    let memories = app.store.list_all(false).unwrap();
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].summary, "Use shared TUI remember action");
    assert_eq!(memories[0].ring, "cambium");
}

#[test]
fn confirmed_export_uses_shared_action_and_keeps_default_filters() {
    let dir = tempdir().unwrap();
    let mut app = app(&dir);
    app.execute_slash_command("/remember Export through shared TUI action")
        .unwrap();
    app.execute_slash_command("/export shared.jsonl").unwrap();
    confirm(&mut app);

    let output = dir.path().join(".tree-ring/exports/shared.jsonl");
    let jsonl = fs::read_to_string(output).unwrap();
    assert!(jsonl.contains("tree_ring_memory_export"));
    assert!(jsonl.contains("Export through shared TUI action"));
}
```

- [ ] **Step 2: Run focused TUI tests before changing code**

Run:

```bash
cargo test -p tree-ring-memory-cli slash_remember_uses_shared_action confirmed_export_uses_shared_action --locked
```

Expected: PASS before rewiring, proving current behavior.

- [ ] **Step 3: Import shared actions in TUI app**

Add these imports to `crates/tree-ring-memory-cli/src/tui/app.rs`:

```rust
use crate::actions::export_import::{export_jsonl, ExportActionRequest};
use crate::actions::remember::{remember, RememberRequest};
```

- [ ] **Step 4: Replace `remember_summary` internals**

Replace the body of `fn remember_summary(&mut self, summary: String) -> Result<(), String>` with:

```rust
if summary.trim().is_empty() {
    self.status = "remember requires a summary".to_string();
    return Ok(());
}
let report = remember(
    &mut self.store,
    RememberRequest {
        summary: summary.trim().to_string(),
        event_type: "lesson".to_string(),
        ring: "cambium".to_string(),
        scope: "project".to_string(),
        project: None,
        tags: Vec::new(),
    },
)?;
self.status = format!("remembered {}", report.memory.id);
self.refresh_store()
```

- [ ] **Step 5: Replace confirmed export internals**

Inside `confirm_pending_action`, replace the `ActionKind::Export` branch with:

```rust
ActionKind::Export {
    output,
    include_sensitive,
    include_superseded,
} => {
    if output.exists() {
        self.status = format!("export refused existing file {}", output.display());
    } else {
        let report = export_jsonl(
            &self.store,
            ExportActionRequest {
                output: Some(output.clone()),
                include_sensitive,
                include_superseded,
            },
        )?;
        self.status = format!(
            "exported {} memories to {}",
            report.report.memory_count,
            output.display()
        );
    }
}
```

- [ ] **Step 6: Remove unused imports from TUI app**

Remove these imports from `crates/tree-ring-memory-cli/src/tui/app.rs` if the compiler reports they are unused:

```rust
use std::fs;
use tree_ring_memory_core::SensitivityGuard;
```

Keep `std::fs` inside the test module because export tests read generated JSONL.

- [ ] **Step 7: Run focused and full TUI tests**

Run:

```bash
cargo test -p tree-ring-memory-cli slash_remember_uses_shared_action confirmed_export_uses_shared_action tui::app --locked
```

Expected: PASS.

- [ ] **Step 8: Commit Task 4**

Run:

```bash
git add crates/tree-ring-memory-cli/src/tui/app.rs crates/tree-ring-memory-cli/src/tui/actions.rs
git commit -m "Use shared actions in TUI write flows"
```

---

### Task 5: Add Lifecycle, Adapter, And Integration Actions

**Files:**
- Create: `crates/tree-ring-memory-cli/src/actions/lifecycle.rs`
- Create: `crates/tree-ring-memory-cli/src/actions/adapters.rs`
- Create: `crates/tree-ring-memory-cli/src/actions/integrations.rs`
- Modify: `crates/tree-ring-memory-cli/src/actions/mod.rs`
- Modify: `crates/tree-ring-memory-cli/src/main.rs`
- Modify: `crates/tree-ring-memory-cli/src/tui/app.rs`

**Interfaces:**
- Produces: `actions::lifecycle::ConsolidateActionRequest`
- Produces: `actions::lifecycle::consolidate(store: &mut SQLiteMemoryStore, request: ConsolidateActionRequest) -> ActionResult<ConsolidationReport>`
- Produces: `actions::lifecycle::MaintainActionRequest`
- Produces: `actions::lifecycle::maintain(db_path: &Path, store: Option<&mut SQLiteMemoryStore>, request: MaintainActionRequest) -> ActionResult<MaintenanceReport>`
- Produces: `actions::adapters::DoxSyncActionRequest`
- Produces: `actions::adapters::sync_dox(store: &mut SQLiteMemoryStore, request: DoxSyncActionRequest) -> ActionResult<DoxSyncActionReport>`
- Produces: `actions::adapters::RevolveSyncActionRequest`
- Produces: `actions::adapters::sync_revolve(store: &mut SQLiteMemoryStore, request: RevolveSyncActionRequest) -> ActionResult<RevolveSyncActionReport>`
- Produces: `actions::integrations::scan(request: IntegrationScanRequest) -> IntegrationScanActionReport`

- [ ] **Step 1: Add lifecycle action implementation and tests**

Create `crates/tree-ring-memory-cli/src/actions/lifecycle.rs`:

```rust
use std::path::Path;

use tree_ring_memory_core::{
    consolidate_memories, plan_maintenance, ConsolidationPeriod, ConsolidationReport,
    ConsolidationRequest, MaintenanceReport, MaintenanceRequest,
};
use tree_ring_memory_sqlite::SQLiteMemoryStore;

use super::ActionResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolidateActionRequest {
    pub period_type: String,
    pub period_key: Option<String>,
    pub project: Option<String>,
    pub dry_run: bool,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintainActionRequest {
    pub project: Option<String>,
    pub include_superseded: bool,
    pub apply_expired: bool,
    pub apply_secret_redactions: bool,
    pub repair_fts: bool,
}

pub fn consolidation_request(
    request: ConsolidateActionRequest,
) -> ActionResult<ConsolidationRequest> {
    Ok(ConsolidationRequest {
        period_type: ConsolidationPeriod::parse(&request.period_type)
            .map_err(|err| err.to_string())?,
        period_key: request.period_key,
        project: request.project,
        dry_run: request.dry_run,
        force: request.force,
    })
}

pub fn consolidate(
    store: &mut SQLiteMemoryStore,
    request: ConsolidateActionRequest,
) -> ActionResult<ConsolidationReport> {
    let request = consolidation_request(request)?;
    store.consolidate(&request).map_err(|err| err.to_string())
}

pub fn consolidate_dry_run_from_path(
    db_path: &Path,
    request: ConsolidateActionRequest,
) -> ActionResult<ConsolidationReport> {
    let request = consolidation_request(request)?;
    if db_path.exists() {
        let store = SQLiteMemoryStore::open_read_only(db_path).map_err(|err| err.to_string())?;
        let events = store.list_all(false).map_err(|err| err.to_string())?;
        consolidate_memories(&events, &request).map_err(|err| err.to_string())
    } else {
        consolidate_memories(&[], &request).map_err(|err| err.to_string())
    }
}

pub fn maintenance_request(request: MaintainActionRequest) -> MaintenanceRequest {
    MaintenanceRequest {
        dry_run: !(request.apply_expired || request.apply_secret_redactions || request.repair_fts),
        apply_expired: request.apply_expired,
        apply_secret_redactions: request.apply_secret_redactions,
        repair_fts: request.repair_fts,
        include_superseded: request.include_superseded,
        project: request.project,
    }
}

pub fn maintain(
    db_path: &Path,
    store: Option<&mut SQLiteMemoryStore>,
    request: MaintainActionRequest,
) -> ActionResult<MaintenanceReport> {
    let request = maintenance_request(request);
    if request.dry_run && !db_path.exists() {
        return Ok(plan_maintenance(&[], &request));
    }
    if let Some(store) = store {
        return store.maintain(&request).map_err(|err| err.to_string());
    }
    let mut store = SQLiteMemoryStore::open_read_only(db_path).map_err(|err| err.to_string())?;
    store.maintain(&request).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn maintenance_action_missing_store_is_non_mutating_dry_run() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join(".tree-ring/memory.sqlite");

        let report = maintain(
            &db_path,
            None,
            MaintainActionRequest {
                project: None,
                include_superseded: false,
                apply_expired: false,
                apply_secret_redactions: false,
                repair_fts: false,
            },
        )
        .unwrap();

        assert_eq!(report.memory_count, 0);
        assert!(!db_path.exists());
    }
}
```

- [ ] **Step 2: Add adapter action implementation and tests**

Create `crates/tree-ring-memory-cli/src/actions/adapters.rs`:

```rust
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
    store: &mut SQLiteMemoryStore,
    request: DoxSyncActionRequest,
) -> ActionResult<DoxSyncActionReport> {
    let mut dox_request = DoxSyncRequest::new(request.source_root);
    dox_request.project = request.project;
    let report = collect_dox_memories(&dox_request).map_err(|err| err.to_string())?;
    if !request.dry_run {
        store.put_many(&report.events).map_err(|err| err.to_string())?;
    }
    Ok(DoxSyncActionReport {
        report,
        dry_run: request.dry_run,
    })
}

pub fn sync_revolve(
    store: &mut SQLiteMemoryStore,
    request: RevolveSyncActionRequest,
) -> ActionResult<RevolveSyncActionReport> {
    let mut revolve_request = RevolveSyncRequest::new(request.source_root);
    revolve_request.project = request.project;
    let report = collect_revolve_memories(&revolve_request).map_err(|err| err.to_string())?;
    if !request.dry_run {
        store.put_many(&report.events).map_err(|err| err.to_string())?;
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
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();

        let report = sync_dox(
            &mut store,
            DoxSyncActionRequest {
                source_root: dir.path().to_path_buf(),
                project: Some("tree-ring".to_string()),
                dry_run: true,
            },
        )
        .unwrap();

        assert_eq!(report.report.memory_count, 1);
        assert_eq!(store.list_all(true).unwrap().len(), 0);
    }
}
```

- [ ] **Step 3: Add integration scan action implementation and tests**

Create `crates/tree-ring-memory-cli/src/actions/integrations.rs`:

```rust
use std::path::PathBuf;

use crate::integrations::{scan_integrations, IntegrationScanReport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationScanRequest {
    pub source_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntegrationScanActionReport {
    pub report: IntegrationScanReport,
}

pub fn scan(request: IntegrationScanRequest) -> IntegrationScanActionReport {
    IntegrationScanActionReport {
        report: scan_integrations(&request.source_root),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn integration_action_scans_project_markers() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# Rules").unwrap();

        let report = scan(IntegrationScanRequest {
            source_root: dir.path().to_path_buf(),
        });

        assert!(report.report.detected_count > 0);
    }
}
```

- [ ] **Step 4: Export the lifecycle, adapter, and integration modules**

Replace `crates/tree-ring-memory-cli/src/actions/mod.rs` with:

```rust
pub mod adapters;
pub mod audit;
pub mod export_import;
pub mod integrations;
pub mod lifecycle;
pub mod recall;
pub mod remember;

pub type ActionResult<T> = Result<T, String>;
```

- [ ] **Step 5: Wire CLI branches through lifecycle and adapter actions**

In `crates/tree-ring-memory-cli/src/main.rs`, replace calls to local helper functions for consolidation, maintenance, DOX, Revolve, and integrations scan with action calls from:

```rust
use actions::adapters::{
    sync_dox, sync_revolve, DoxSyncActionRequest, RevolveSyncActionRequest,
};
use actions::integrations::{scan as integration_scan_action, IntegrationScanRequest};
use actions::lifecycle::{
    consolidate, consolidate_dry_run_from_path, maintain, ConsolidateActionRequest,
    MaintainActionRequest,
};
```

Replace the integrations pre-store branch body with:

```rust
let report = integration_scan_action(IntegrationScanRequest {
    source_root: source_root.clone(),
});
print_integration_report(&report.report, cli.json)?;
return Ok(());
```

Replace consolidation dry-run pre-store behavior with:

```rust
let report = consolidate_dry_run_from_path(
    &db_path,
    ConsolidateActionRequest {
        period_type: period_type.clone(),
        period_key: period_key.clone(),
        project: project.clone(),
        dry_run: true,
        force: *force,
    },
)?;
print_consolidation_report(&report, cli.json)?;
return Ok(());
```

Replace maintenance dry-run pre-store behavior with:

```rust
let report = maintain(
    &db_path,
    None,
    MaintainActionRequest {
        project: project.clone(),
        include_superseded: *include_superseded,
        apply_expired: *apply_expired,
        apply_secret_redactions: *apply_secret_redactions,
        repair_fts: *repair_fts,
    },
)?;
print_maintenance_report(&report, cli.json)?;
return Ok(());
```

Replace writable `Command::Consolidate`, `Command::Maintain`, `Command::Dox`, and `Command::Revolve` match arm bodies with calls to `consolidate`, `maintain`, `sync_dox`, and `sync_revolve`, then call existing print functions with the returned reports.

- [ ] **Step 6: Update TUI `/integrations` to call the integration action**

In `crates/tree-ring-memory-cli/src/tui/app.rs`, import:

```rust
use crate::actions::integrations::{scan as scan_integrations_action, IntegrationScanRequest};
```

Replace `show_integrations` scan line with:

```rust
let report = scan_integrations_action(IntegrationScanRequest { source_root: root });
self.status = format!(
    "integration scan: {} detected under {}",
    report.report.detected_count,
    report.report.root.display()
);
self.integration_report = Some(report.report);
self.mode = AppMode::Integrations;
```

- [ ] **Step 7: Run focused action, CLI, and TUI tests**

Run:

```bash
cargo test -p tree-ring-memory-cli actions::lifecycle actions::adapters actions::integrations integrations_scan_is_read_only slash_integrations --locked
```

Expected: PASS.

- [ ] **Step 8: Commit Task 5**

Run:

```bash
git add crates/tree-ring-memory-cli/src/actions crates/tree-ring-memory-cli/src/main.rs crates/tree-ring-memory-cli/src/tui/app.rs
git commit -m "Add lifecycle and integration actions"
```

---

### Task 6: Split Private SQLite Internals Behind The Existing Store Facade

**Files:**
- Modify: `crates/tree-ring-memory-sqlite/src/lib.rs`
- Create: `crates/tree-ring-memory-sqlite/src/schema.rs`
- Create: `crates/tree-ring-memory-sqlite/src/write.rs`
- Create: `crates/tree-ring-memory-sqlite/src/search.rs`
- Create: `crates/tree-ring-memory-sqlite/src/import_export.rs`
- Create: `crates/tree-ring-memory-sqlite/src/lifecycle.rs`

**Interfaces:**
- Keeps: `SQLiteMemoryStore::open(path) -> TreeRingResult<Self>`
- Keeps: `SQLiteMemoryStore::open_read_only(path) -> TreeRingResult<Self>`
- Keeps: `SQLiteMemoryStore::put`, `put_many`, `get`, `list_all`, `search_text`, `search_text_filtered_limited`, `supersede`, `delete`, `redact`, `export_jsonl`, `import_jsonl`, `audit`, `consolidate`, `maintain`
- Produces only crate-private helpers in new files.

- [ ] **Step 1: Add facade preservation tests**

Add this test to `crates/tree-ring-memory-sqlite/src/lib.rs` inside the existing test module:

```rust
#[test]
fn public_store_facade_still_covers_write_search_export_import_and_maintenance() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("memory.sqlite");
    let mut store = SQLiteMemoryStore::open(&db_path).unwrap();
    let event = MemoryEvent::new("Facade preservation memory.", "lesson").unwrap();
    store.put(&event).unwrap();

    assert!(store.get(&event.id).unwrap().is_some());
    assert_eq!(store.search_text("facade preservation", false).unwrap().len(), 1);

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
```

- [ ] **Step 2: Run the facade preservation test before moving helpers**

Run:

```bash
cargo test -p tree-ring-memory-sqlite public_store_facade_still_covers --locked
```

Expected: PASS before moving helpers.

- [ ] **Step 3: Move schema/open helpers into `schema.rs`**

Create `crates/tree-ring-memory-sqlite/src/schema.rs` with crate-private functions moved from `lib.rs`:

```rust
use rusqlite::{Connection, OpenFlags};
use std::path::{Component, Path};

use tree_ring_memory_core::models::{sqlite_error, TreeRingResult};

use crate::sqlite_error_from_rusqlite;

pub(crate) fn open_connection(path: &Path) -> TreeRingResult<Connection> {
    if let Some(parent) = parent_dir_to_create(path) {
        std::fs::create_dir_all(parent).map_err(|err| sqlite_error(err.to_string()))?;
    }
    let connection = Connection::open(path).map_err(sqlite_error_from_rusqlite)?;
    configure_connection(&connection)?;
    Ok(connection)
}

pub(crate) fn open_read_only_connection(path: &Path) -> TreeRingResult<Connection> {
    let path = path
        .canonicalize()
        .map_err(|err| sqlite_error(err.to_string()))?;
    let uri = sqlite_uri_for_path(&path);
    let connection = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(sqlite_error_from_rusqlite)?;
    configure_connection(&connection)?;
    Ok(connection)
}

fn configure_connection(connection: &Connection) -> TreeRingResult<()> {
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

fn parent_dir_to_create(path: &Path) -> Option<&Path> {
    if path.components().count() <= 1 {
        return None;
    }
    path.parent().filter(|parent| !parent.as_os_str().is_empty())
}

fn sqlite_uri_for_path(path: &Path) -> String {
    let mut encoded = String::from("file:");
    for component in path.components() {
        match component {
            Component::RootDir => encoded.push('/'),
            Component::Normal(value) => {
                if !encoded.ends_with('/') {
                    encoded.push('/');
                }
                encoded.push_str(&percent_encode_path_segment(&value.to_string_lossy()));
            }
            _ => {}
        }
    }
    encoded.push_str("?mode=ro");
    encoded
}

fn percent_encode_path_segment(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}
```

In `lib.rs`, add:

```rust
mod schema;
```

Then replace open/open_read_only connection setup with `schema::open_connection(path)?` and `schema::open_read_only_connection(path)?`.

- [ ] **Step 4: Move write helpers into `write.rs`**

Create `crates/tree-ring-memory-sqlite/src/write.rs` by moving these private helpers from `lib.rs` without changing function bodies:

```rust
pub(crate) fn delete_in_transaction(transaction: &Transaction<'_>, memory_id: &str) -> TreeRingResult<bool>
pub(crate) fn redact_in_transaction(transaction: &Transaction<'_>, memory_id: &str) -> TreeRingResult<bool>
pub(crate) fn put_in_transaction(transaction: &Transaction<'_>, event: &MemoryEvent) -> TreeRingResult<()>
pub(crate) fn put_with_statements(
    event: &MemoryEvent,
    insert_memory: &mut rusqlite::Statement<'_>,
    delete_fts: &mut rusqlite::Statement<'_>,
    insert_fts: &mut rusqlite::Statement<'_>,
) -> TreeRingResult<()>
pub(crate) fn retry_locked<T>(operation: impl FnMut() -> TreeRingResult<T>) -> TreeRingResult<T>
```

Use the existing signatures from `lib.rs`, make them `pub(crate)`, and update `lib.rs` call sites to `write::delete_in_transaction`, `write::redact_in_transaction`, `write::put_in_transaction`, `write::put_with_statements`, and `write::retry_locked`.

- [ ] **Step 5: Move search helpers into `search.rs`**

Create `crates/tree-ring-memory-sqlite/src/search.rs` by moving these private helpers from `lib.rs` without changing function bodies:

```rust
pub(crate) fn event_from_row(row: &Row<'_>) -> rusqlite::Result<TreeRingResult<MemoryEvent>>
pub(crate) fn collect_rows<I>(rows: I) -> TreeRingResult<Vec<MemoryEvent>>
where
    I: IntoIterator<Item = rusqlite::Result<TreeRingResult<MemoryEvent>>>
pub(crate) fn push_in_filter(
    sql: &mut String,
    parameters: &mut Vec<Value>,
    column_name: &str,
    values: &[String],
)
```

Use the existing signatures from `lib.rs`, make them `pub(crate)`, and update `lib.rs` call sites through `search::`.

- [ ] **Step 6: Move JSONL import/export helpers into `import_export.rs`**

Create `crates/tree-ring-memory-sqlite/src/import_export.rs` only if the helper boundary stays cleaner than keeping methods on `SQLiteMemoryStore`. The two candidate methods are:

```rust
fn apply_supersedes(&mut self, event: &MemoryEvent) -> TreeRingResult<()>
fn existing_memory_ids(&self, ids: &[String]) -> TreeRingResult<HashSet<String>>
```

If moving those methods requires exposing store internals, keep them in `lib.rs` and record that decision in the task commit message. The acceptance criterion is a narrower `lib.rs`, not a forced split that damages readability.

- [ ] **Step 7: Move lifecycle helpers into `lifecycle.rs`**

Create `crates/tree-ring-memory-sqlite/src/lifecycle.rs` by moving these private helpers from `lib.rs` without changing function bodies:

```rust
pub(crate) fn count_query(connection: &Connection, sql: &str) -> TreeRingResult<usize>
pub(crate) fn rebuild_fts_in_transaction(transaction: &Transaction<'_>) -> TreeRingResult<()>
pub(crate) fn consolidation_from_row(row: &Row<'_>) -> rusqlite::Result<TreeRingResult<StoredConsolidation>>
```

Use the existing signatures from `lib.rs`, make them `pub(crate)`, and update `lib.rs` call sites through `lifecycle::`.

- [ ] **Step 8: Run sqlite tests after each helper group**

After each helper group, run:

```bash
cargo test -p tree-ring-memory-sqlite --locked
```

Expected: PASS after each group. If a helper move creates lifetime or privacy churn that spreads beyond the helper group, revert that helper move and leave it in `lib.rs` for a smaller follow-up plan.

- [ ] **Step 9: Commit Task 6**

Run:

```bash
git add crates/tree-ring-memory-sqlite/src
git commit -m "Split sqlite store internals"
```

---

### Task 7: Final Verification, Docs, And PR Handoff

**Files:**
- Modify: `docs/architecture/rust-core-status.md`
- Modify: `README.md` only if the final implementation changes internal verification guidance

**Interfaces:**
- Consumes: completed action modules, CLI wiring, TUI wiring, and SQLite helper split.
- Produces: final verification evidence and PR-ready branch.

- [ ] **Step 1: Add a short architecture status note**

In `docs/architecture/rust-core-status.md`, add this bullet near the current Rust CLI/TUI status bullets:

```markdown
- CLI and TUI durable operations now share action request/report contracts for
  behavior-preserving command execution. This keeps CLI output ownership,
  TUI state/render ownership, and storage ownership separate while preparing
  the TUI cockpit and integration-link workflows.
```

- [ ] **Step 2: Run formatter**

Run:

```bash
cargo fmt --check
```

Expected: PASS.

- [ ] **Step 3: Run full test suite**

Run:

```bash
cargo test --locked
```

Expected: PASS for all CLI, core, sqlite, and doc tests.

- [ ] **Step 4: Run Clippy**

Run:

```bash
cargo clippy --locked --all-targets
```

Expected: PASS with no new warnings.

- [ ] **Step 5: Run diff whitespace check**

Run:

```bash
git diff --check
```

Expected: no output and exit code 0.

- [ ] **Step 6: Run certification**

Run:

```bash
sh scripts/certify-tree-ring.sh
```

Expected: certification passes and writes `target/tree-ring-certification/summary.md` plus `target/tree-ring-certification/metrics.json`.

- [ ] **Step 7: Review size reduction and boundaries**

Run:

```bash
wc -l crates/tree-ring-memory-cli/src/main.rs crates/tree-ring-memory-cli/src/tui/app.rs crates/tree-ring-memory-sqlite/src/lib.rs crates/tree-ring-memory-cli/src/actions/*.rs crates/tree-ring-memory-cli/src/commands/*.rs crates/tree-ring-memory-sqlite/src/*.rs
```

Expected: `main.rs`, `tui/app.rs`, or `sqlite/lib.rs` is thinner than before, or the final PR description explains which file remains large and why the responsibility boundary is still improved.

- [ ] **Step 8: Commit final docs and verification note**

Run:

```bash
git add docs/architecture/rust-core-status.md README.md
git commit -m "Document shared action foundation"
```

If `README.md` has no changes, run:

```bash
git add docs/architecture/rust-core-status.md
git commit -m "Document shared action foundation"
```

- [ ] **Step 9: Push and open PR**

Run:

```bash
git status --short --branch
git push -u origin codex/shared-action-foundation
gh pr create --base main --head codex/shared-action-foundation --title "Add shared action foundation" --body "## Summary
- add shared action request/report contracts for durable CLI and TUI operations
- wire existing CLI and TUI flows through those actions without changing user-facing behavior
- split private SQLite helpers behind the existing SQLiteMemoryStore facade
- document the shared action foundation as the base for the TUI cockpit lane

## Verification
- cargo fmt --check
- cargo test --locked
- cargo clippy --locked --all-targets
- git diff --check
- sh scripts/certify-tree-ring.sh"
```

Expected: branch pushed and PR opened against `main`.

---

## Plan Self-Review

- Spec coverage: Tasks 1-5 cover shared actions and CLI/TUI adoption. Task 6 covers SQLite facade-preserving internal split. Task 7 covers docs and certification.
- Incomplete-marker scan: no incomplete markers remain.
- Type consistency: request/report names used by later tasks are introduced before use.
- Scope check: no TUI cockpit features, harness matrix, recall-quality dashboard, schema changes, or behavior changes are included in this plan.
