# Tree Ring Evidence Spine Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the Phase 1 Evidence Spine and TUI evidence browser so operators can inspect local certification proof without changing existing memory behavior.

**Architecture:** Add a small file-backed evidence reader in the CLI crate. Update certification to write `evidence-index.json` beside the existing `metrics.json`, then add a TUI Evidence mode that reads current evidence and offers an explicit confirmed certification-refresh next step without running hidden background work.

**Tech Stack:** Rust 2021, serde/serde_json, Ratatui, existing Tree Ring CLI crate, POSIX shell certification script, cargo tests, `scripts/certify-tree-ring.sh`.

## Global Constraints

- Preserve existing `metrics.json` compatibility while adding richer evidence indexes beside it.
- TUI reads existing evidence by default.
- TUI evidence refresh actions require explicit confirmation.
- The first TUI refresh implementation presents the exact external certification command instead of spawning the long-running script inside the TUI.
- Do not add a daemon, background recorder, hidden proof runner, or hidden durable memory writer.
- Do not change the SQLite schema or public JSONL memory schema.
- Do not make core memory logic depend on Codex, Claude Code, OpenCode, Goose, Pi, Agent Zero, or any other harness.
- Do not replace the current certification metrics format.
- Do not redesign the whole TUI layout in this phase.
- Do not claim compatibility unless a local evidence producer generated a pass, fail, or skip record.
- Do not bundle harness probes or recall-quality diagnostics into Phase 1.
- Run `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked --all-targets`, `git diff --check`, and `sh scripts/certify-tree-ring.sh` before PR handoff.

---

## Scope Check

The roadmap spec covers the TUI cockpit, harness matrix, and recall quality dashboard. This plan intentionally implements only Phase 1: evidence index model, certification writer, and TUI evidence browser. Harness probe producers and recall-quality diagnostics remain follow-up phases.

## File Structure

- Create `crates/tree-ring-memory-cli/src/evidence.rs`: typed evidence index, certification metrics reader, snapshot loader, and tests.
- Modify `crates/tree-ring-memory-cli/src/main.rs`: declare the evidence module for CLI/TUI use.
- Modify `scripts/certify-tree-ring.sh`: write `target/tree-ring-certification/evidence-index.json` while preserving `metrics.json` and `summary.md`.
- Modify `crates/tree-ring-memory-cli/src/tui/input.rs`: parse `/evidence`, `/proof`, and `/evidence refresh`.
- Modify `crates/tree-ring-memory-cli/src/tui/actions.rs`: add a confirmed evidence-refresh pending action.
- Modify `crates/tree-ring-memory-cli/src/tui/app.rs`: hold evidence snapshot state, load evidence, and handle refresh confirmation.
- Modify `crates/tree-ring-memory-cli/src/tui/render.rs`: render Evidence mode list and detail panes.
- Modify `docs/architecture/rust-core-status.md`: document the Phase 1 evidence spine after implementation.

---

### Task 1: Add Evidence Index Model And Reader

**Files:**
- Create: `crates/tree-ring-memory-cli/src/evidence.rs`
- Modify: `crates/tree-ring-memory-cli/src/main.rs`

**Interfaces:**
- Produces: `evidence::EvidenceStatus`
- Produces: `evidence::EvidenceRecordRef`
- Produces: `evidence::EvidenceIndex`
- Produces: `evidence::CertificationEvidence`
- Produces: `evidence::EvidenceSnapshot`
- Produces: `evidence::certification_dir_for_project(project_root: &Path) -> PathBuf`
- Produces: `evidence::load_snapshot(evidence_dir: &Path) -> EvidenceSnapshot`
- Consumes: `serde`, `serde_json`, `std::fs`, and `std::path`

- [ ] **Step 1: Add the evidence module declaration**

Modify `crates/tree-ring-memory-cli/src/main.rs` module declarations so they include:

```rust
mod actions;
mod agent_awareness;
mod commands;
mod evidence;
mod integrations;
mod ring_mark;
mod tui;
mod welcome;
```

- [ ] **Step 2: Write the evidence reader and tests**

Create `crates/tree-ring-memory-cli/src/evidence.rs` with this complete file:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Pass,
    Fail,
    Skip,
    Missing,
    Stale,
    Error,
}

impl EvidenceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Skip => "skip",
            Self::Missing => "missing",
            Self::Stale => "stale",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRecordRef {
    pub category: String,
    pub status: EvidenceStatus,
    pub label: String,
    pub path: PathBuf,
    pub summary_path: Option<PathBuf>,
    pub generated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceIndex {
    pub generated_at: String,
    pub overall_status: EvidenceStatus,
    pub certification: Option<EvidenceRecordRef>,
    #[serde(default)]
    pub harness: BTreeMap<String, EvidenceRecordRef>,
    pub recall_quality: Option<EvidenceRecordRef>,
    #[serde(default)]
    pub missing: Vec<String>,
    #[serde(default)]
    pub stale: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CertificationEvidence {
    pub status: EvidenceStatus,
    pub generated_at: String,
    pub metrics_path: PathBuf,
    pub summary_path: Option<PathBuf>,
    pub release_binary_bytes: Option<u64>,
    pub project_install_kb: Option<u64>,
    pub global_install_kb: Option<u64>,
    pub cli_import_events_per_second: Option<u64>,
    pub recall_avg_ms_10000: Option<f64>,
    pub recall_max_ms_10000: Option<f64>,
    pub recall_avg_ms_30000: Option<f64>,
    pub recall_max_ms_30000: Option<f64>,
    pub agent_zero_status: Option<String>,
    pub agent_zero_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceSnapshot {
    pub root: PathBuf,
    pub index_path: PathBuf,
    pub index: Option<EvidenceIndex>,
    pub certification: Option<CertificationEvidence>,
    pub status: EvidenceStatus,
    pub message: String,
}

pub fn certification_dir_for_project(project_root: &Path) -> PathBuf {
    project_root.join("target").join("tree-ring-certification")
}

pub fn load_snapshot(evidence_dir: &Path) -> EvidenceSnapshot {
    let index_path = evidence_dir.join("evidence-index.json");
    match load_index(&index_path) {
        Ok(index) => {
            let certification = index
                .certification
                .as_ref()
                .and_then(|record| load_certification(evidence_dir, record).ok());
            let message = match &certification {
                Some(certification) => format!(
                    "certification {} at {}",
                    certification.status.as_str(),
                    certification.generated_at
                ),
                None => "evidence index loaded without certification metrics".to_string(),
            };
            EvidenceSnapshot {
                root: evidence_dir.to_path_buf(),
                index_path,
                status: index.overall_status,
                index: Some(index),
                certification,
                message,
            }
        }
        Err(_) if !index_path.exists() => EvidenceSnapshot {
            root: evidence_dir.to_path_buf(),
            index_path,
            index: None,
            certification: None,
            status: EvidenceStatus::Missing,
            message: format!(
                "no evidence index found; run sh scripts/certify-tree-ring.sh to generate {}",
                evidence_dir.display()
            ),
        },
        Err(error) => EvidenceSnapshot {
            root: evidence_dir.to_path_buf(),
            index_path,
            index: None,
            certification: None,
            status: EvidenceStatus::Error,
            message: error,
        },
    }
}

fn load_index(index_path: &Path) -> Result<EvidenceIndex, String> {
    let input = fs::read_to_string(index_path).map_err(|err| err.to_string())?;
    serde_json::from_str(&input).map_err(|err| err.to_string())
}

fn load_certification(
    evidence_dir: &Path,
    record: &EvidenceRecordRef,
) -> Result<CertificationEvidence, String> {
    let metrics_path = resolve_evidence_path(evidence_dir, &record.path);
    let summary_path = record
        .summary_path
        .as_ref()
        .map(|path| resolve_evidence_path(evidence_dir, path));
    let input = fs::read_to_string(&metrics_path).map_err(|err| err.to_string())?;
    let value: Value = serde_json::from_str(&input).map_err(|err| err.to_string())?;
    Ok(CertificationEvidence {
        status: record.status,
        generated_at: value
            .get("created_at")
            .and_then(Value::as_str)
            .unwrap_or(record.generated_at.as_str())
            .to_string(),
        metrics_path,
        summary_path,
        release_binary_bytes: get_u64(&value, &["release_binary_bytes"]),
        project_install_kb: get_u64(&value, &["project_install_kb"]),
        global_install_kb: get_u64(&value, &["global_install_kb"]),
        cli_import_events_per_second: get_u64(&value, &["cli_import", "events_per_second"]),
        recall_avg_ms_10000: get_f64(&value, &["performance", "records_10000", "recall_avg_ms"]),
        recall_max_ms_10000: get_f64(&value, &["performance", "records_10000", "recall_max_ms"]),
        recall_avg_ms_30000: get_f64(&value, &["performance", "records_30000", "recall_avg_ms"]),
        recall_max_ms_30000: get_f64(&value, &["performance", "records_30000", "recall_max_ms"]),
        agent_zero_status: get_string(&value, &["agent_zero", "status"]),
        agent_zero_note: get_string(&value, &["agent_zero", "note"]),
    })
}

fn resolve_evidence_path(evidence_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        evidence_dir.join(path)
    }
}

fn get_u64(value: &Value, path: &[&str]) -> Option<u64> {
    get_value(value, path).and_then(Value::as_u64)
}

fn get_f64(value: &Value, path: &[&str]) -> Option<f64> {
    get_value(value, path).and_then(Value::as_f64)
}

fn get_string(value: &Value, path: &[&str]) -> Option<String> {
    get_value(value, path)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn get_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |current, key| current.get(*key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn evidence_snapshot_reports_missing_index_without_error() {
        let dir = tempdir().unwrap();

        let snapshot = load_snapshot(&certification_dir_for_project(dir.path()));

        assert_eq!(snapshot.status, EvidenceStatus::Missing);
        assert!(snapshot.index.is_none());
        assert!(snapshot.message.contains("certify-tree-ring"));
    }

    #[test]
    fn evidence_snapshot_loads_certification_metrics_from_index() {
        let dir = tempdir().unwrap();
        let evidence_dir = certification_dir_for_project(dir.path());
        fs::create_dir_all(&evidence_dir).unwrap();
        fs::write(evidence_dir.join("summary.md"), "# Summary\n").unwrap();
        fs::write(
            evidence_dir.join("metrics.json"),
            r#"{
              "ok": true,
              "created_at": "2026-07-09T04:22:38Z",
              "release_binary_bytes": 6137088,
              "project_install_kb": 6064,
              "global_install_kb": 6020,
              "cli_import": {"memory_count": 10000, "seconds": 5, "events_per_second": 2000},
              "performance": {
                "records_10000": {"recall_avg_ms": 3.729, "recall_max_ms": 6.539},
                "records_30000": {"recall_avg_ms": 7.978, "recall_max_ms": 14.444},
                "records_50000": null
              },
              "agent_zero": {"status": "skipped", "note": "TREE_RING_AGENT_ZERO_ROOT not set"}
            }"#,
        )
        .unwrap();
        fs::write(
            evidence_dir.join("evidence-index.json"),
            r#"{
              "generated_at": "2026-07-09T04:22:38Z",
              "overall_status": "pass",
              "certification": {
                "category": "certification",
                "status": "pass",
                "label": "Local certification",
                "path": "metrics.json",
                "summary_path": "summary.md",
                "generated_at": "2026-07-09T04:22:38Z"
              },
              "harness": {},
              "recall_quality": null,
              "missing": ["harness", "recall_quality"],
              "stale": []
            }"#,
        )
        .unwrap();

        let snapshot = load_snapshot(&evidence_dir);

        let certification = snapshot.certification.unwrap();
        assert_eq!(snapshot.status, EvidenceStatus::Pass);
        assert_eq!(certification.release_binary_bytes, Some(6_137_088));
        assert_eq!(certification.project_install_kb, Some(6_064));
        assert_eq!(certification.global_install_kb, Some(6_020));
        assert_eq!(certification.cli_import_events_per_second, Some(2_000));
        assert_eq!(certification.recall_avg_ms_10000, Some(3.729));
        assert_eq!(certification.recall_max_ms_30000, Some(14.444));
        assert_eq!(certification.agent_zero_status.as_deref(), Some("skipped"));
    }
}
```

- [ ] **Step 3: Run focused evidence tests**

Run:

```bash
cargo test -p tree-ring-memory-cli evidence --locked
```

Expected: PASS with both `evidence_snapshot_*` tests passing.

- [ ] **Step 4: Commit Task 1**

Run:

```bash
git add crates/tree-ring-memory-cli/src/evidence.rs crates/tree-ring-memory-cli/src/main.rs
git commit -m "Add evidence index reader"
```

---

### Task 2: Write Evidence Index During Certification

**Files:**
- Modify: `scripts/certify-tree-ring.sh`

**Interfaces:**
- Consumes: Task 1 `evidence::EvidenceIndex` JSON shape.
- Produces: `target/tree-ring-certification/evidence-index.json`
- Keeps: `target/tree-ring-certification/metrics.json`
- Keeps: `target/tree-ring-certification/summary.md`

- [ ] **Step 1: Add an index path variable**

In `scripts/certify-tree-ring.sh`, after:

```sh
SUMMARY="$OUT_DIR/summary.md"
METRICS="$OUT_DIR/metrics.json"
LOG="$OUT_DIR/certification.log"
```

change it to:

```sh
SUMMARY="$OUT_DIR/summary.md"
METRICS="$OUT_DIR/metrics.json"
INDEX="$OUT_DIR/evidence-index.json"
LOG="$OUT_DIR/certification.log"
```

- [ ] **Step 2: Write the evidence index after summary generation**

After the existing `cat > "$SUMMARY" <<EOF` block and before `log "certification passed"`, add:

```sh
cat > "$INDEX" <<EOF
{
  "generated_at": "$created_at",
  "overall_status": "pass",
  "certification": {
    "category": "certification",
    "status": "pass",
    "label": "Local certification",
    "path": "metrics.json",
    "summary_path": "summary.md",
    "generated_at": "$created_at"
  },
  "harness": {},
  "recall_quality": null,
  "missing": ["harness", "recall_quality"],
  "stale": []
}
EOF
```

- [ ] **Step 3: Print the index path in the certification completion output**

At the bottom of `scripts/certify-tree-ring.sh`, change:

```sh
printf 'Summary: %s\n' "$SUMMARY"
printf 'Metrics: %s\n' "$METRICS"
```

to:

```sh
printf 'Summary: %s\n' "$SUMMARY"
printf 'Metrics: %s\n' "$METRICS"
printf 'Evidence index: %s\n' "$INDEX"
```

- [ ] **Step 4: Run syntax and model checks**

Run:

```bash
sh -n scripts/certify-tree-ring.sh
cargo test -p tree-ring-memory-cli evidence --locked
```

Expected: shell syntax check exits 0 and evidence tests pass.

- [ ] **Step 5: Run certification and verify the new index**

Run:

```bash
sh scripts/certify-tree-ring.sh
test -f target/tree-ring-certification/evidence-index.json
cargo test -p tree-ring-memory-cli evidence --locked
```

Expected: certification passes, the evidence index file exists, and the evidence tests still pass.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add scripts/certify-tree-ring.sh
git commit -m "Write certification evidence index"
```

---

### Task 3: Add TUI Evidence State And Confirmed Refresh Next Step

**Files:**
- Modify: `crates/tree-ring-memory-cli/src/tui/input.rs`
- Modify: `crates/tree-ring-memory-cli/src/tui/actions.rs`
- Modify: `crates/tree-ring-memory-cli/src/tui/app.rs`

**Interfaces:**
- Consumes: `evidence::certification_dir_for_project(project_root: &Path) -> PathBuf`
- Consumes: `evidence::load_snapshot(evidence_dir: &Path) -> EvidenceSnapshot`
- Produces: `SlashCommand::Evidence(String)`
- Produces: `AppMode::Evidence`
- Produces: `ActionKind::RefreshCertification { command: String }`
- Produces: `App::evidence_snapshot: Option<EvidenceSnapshot>`

- [ ] **Step 1: Add failing slash-command tests**

In `crates/tree-ring-memory-cli/src/tui/input.rs`, add this test to the existing test module:

```rust
#[test]
fn parses_evidence_command_and_refresh_argument() {
    assert_eq!(
        parse_slash_command("/evidence"),
        SlashCommand::Evidence(String::new())
    );
    assert_eq!(
        parse_slash_command("/proof refresh"),
        SlashCommand::Evidence("refresh".to_string())
    );
}
```

Run:

```bash
cargo test -p tree-ring-memory-cli parses_evidence_command_and_refresh_argument --locked
```

Expected: FAIL because `SlashCommand::Evidence` does not exist.

- [ ] **Step 2: Add the Evidence slash command**

In `crates/tree-ring-memory-cli/src/tui/input.rs`, add the enum variant:

```rust
Evidence(String),
```

Add this match arm in `parse_slash_command`:

```rust
"evidence" | "proof" => SlashCommand::Evidence(argument),
```

Update `command_help()` to return:

```rust
"/rings /search <q> /remember <summary> /forget /redact /promote /scar /seed /supersede <old_id> /consolidate /export <file> /sync /integrations /evidence"
```

- [ ] **Step 3: Add failing pending-action tests**

In `crates/tree-ring-memory-cli/src/tui/actions.rs`, add this test to the existing test module:

```rust
#[test]
fn evidence_refresh_is_explicit_pending_value() {
    let pending = PendingAction::refresh_certification("sh scripts/certify-tree-ring.sh");

    assert!(pending.confirmation_prompt().contains("press y"));
    assert!(pending.summary.contains("Refresh certification evidence"));
    assert_eq!(
        pending.kind,
        ActionKind::RefreshCertification {
            command: "sh scripts/certify-tree-ring.sh".to_string()
        }
    );
}
```

Run:

```bash
cargo test -p tree-ring-memory-cli evidence_refresh_is_explicit_pending_value --locked
```

Expected: FAIL because `RefreshCertification` does not exist.

- [ ] **Step 4: Add the pending refresh action**

In `crates/tree-ring-memory-cli/src/tui/actions.rs`, add this `ActionKind` variant:

```rust
RefreshCertification {
    command: String,
},
```

Add this constructor to `impl PendingAction`:

```rust
pub fn refresh_certification(command: &str) -> Self {
    Self {
        kind: ActionKind::RefreshCertification {
            command: command.to_string(),
        },
        memory_id: None,
        summary: "Refresh certification evidence".to_string(),
    }
}
```

- [ ] **Step 5: Add failing TUI app tests**

In `crates/tree-ring-memory-cli/src/tui/app.rs`, add these tests to the existing test module:

```rust
#[test]
fn slash_evidence_opens_missing_evidence_state() {
    let dir = tempdir().unwrap();
    let mut app = app(&dir);

    app.execute_slash_command("/evidence").unwrap();

    assert_eq!(app.mode, AppMode::Evidence);
    let snapshot = app.evidence_snapshot.as_ref().unwrap();
    assert_eq!(snapshot.status, crate::evidence::EvidenceStatus::Missing);
    assert!(app.status.contains("evidence"));
}

#[test]
fn slash_evidence_refresh_requires_confirmation_without_running() {
    let dir = tempdir().unwrap();
    let mut app = app(&dir);

    app.execute_slash_command("/evidence refresh").unwrap();

    assert!(app.pending_action.is_some());
    assert!(app.pending_action.as_ref().unwrap().summary.contains("Refresh certification"));
    confirm(&mut app);
    assert!(app.status.contains("run externally: sh scripts/certify-tree-ring.sh"));
}
```

Run:

```bash
cargo test -p tree-ring-memory-cli slash_evidence --locked
```

Expected: FAIL because `AppMode::Evidence` and `evidence_snapshot` do not exist.

- [ ] **Step 6: Add evidence state to the TUI app**

In `crates/tree-ring-memory-cli/src/tui/app.rs`, add this import near the other crate imports:

```rust
use crate::evidence::{certification_dir_for_project, load_snapshot, EvidenceSnapshot};
```

Add this `AppMode` variant:

```rust
Evidence,
```

Add this field to `pub struct App`:

```rust
pub evidence_snapshot: Option<EvidenceSnapshot>,
```

Initialize it in `App::new`:

```rust
evidence_snapshot: None,
```

Add this match arm in `execute_slash_command`:

```rust
SlashCommand::Evidence(argument) => {
    if argument.eq_ignore_ascii_case("refresh") {
        self.pending_action = Some(PendingAction::refresh_certification(
            "sh scripts/certify-tree-ring.sh",
        ));
    } else {
        self.show_evidence();
    }
}
```

Add this method to `impl App` near `show_integrations`:

```rust
fn show_evidence(&mut self) {
    let project_root = project_root_for_memory_root(&self.root);
    let evidence_dir = certification_dir_for_project(&project_root);
    let snapshot = load_snapshot(&evidence_dir);
    self.status = format!("evidence: {}", snapshot.message);
    self.evidence_snapshot = Some(snapshot);
    self.mode = AppMode::Evidence;
}
```

Add this match branch to `confirm_pending_action`:

```rust
ActionKind::RefreshCertification { command } => {
    self.status = format!("run externally: {command}");
}
```

- [ ] **Step 7: Run focused TUI state tests**

Run:

```bash
cargo test -p tree-ring-memory-cli parses_evidence_command_and_refresh_argument evidence_refresh_is_explicit_pending_value slash_evidence --locked
```

If Cargo rejects multiple positional filters, run:

```bash
cargo test -p tree-ring-memory-cli parses_evidence_command_and_refresh_argument --locked
cargo test -p tree-ring-memory-cli evidence_refresh_is_explicit_pending_value --locked
cargo test -p tree-ring-memory-cli slash_evidence --locked
```

Expected: PASS for the evidence parser, pending action, and TUI state tests.

- [ ] **Step 8: Commit Task 3**

Run:

```bash
git add crates/tree-ring-memory-cli/src/tui/input.rs crates/tree-ring-memory-cli/src/tui/actions.rs crates/tree-ring-memory-cli/src/tui/app.rs
git commit -m "Add TUI evidence state"
```

---

### Task 4: Render Evidence Mode In The TUI

**Files:**
- Modify: `crates/tree-ring-memory-cli/src/tui/render.rs`

**Interfaces:**
- Consumes: `AppMode::Evidence`
- Consumes: `App::evidence_snapshot`
- Consumes: `evidence::EvidenceStatus::as_str()`

- [ ] **Step 1: Add failing render tests**

In `crates/tree-ring-memory-cli/src/tui/render.rs`, add these tests to the existing test module:

```rust
#[test]
fn render_evidence_mode_shows_empty_state_and_refresh_command() {
    let dir = tempdir().unwrap();
    let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
    app.execute_slash_command("/evidence").unwrap();
    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|frame| render(frame, &app)).unwrap();
    let output = terminal.backend().to_string();

    assert!(output.contains("Evidence"));
    assert!(output.contains("missing"));
    assert!(output.contains("certify-tree-ring"));
}

#[test]
fn render_evidence_mode_shows_certification_metrics() {
    let dir = tempdir().unwrap();
    let evidence_dir = dir.path().join("target/tree-ring-certification");
    std::fs::create_dir_all(&evidence_dir).unwrap();
    std::fs::write(evidence_dir.join("summary.md"), "# Summary\n").unwrap();
    std::fs::write(
        evidence_dir.join("metrics.json"),
        r#"{
          "ok": true,
          "created_at": "2026-07-09T04:22:38Z",
          "release_binary_bytes": 6137088,
          "project_install_kb": 6064,
          "global_install_kb": 6020,
          "cli_import": {"events_per_second": 2000},
          "performance": {
            "records_10000": {"recall_avg_ms": 3.729, "recall_max_ms": 6.539},
            "records_30000": {"recall_avg_ms": 7.978, "recall_max_ms": 14.444}
          },
          "agent_zero": {"status": "skipped", "note": "TREE_RING_AGENT_ZERO_ROOT not set"}
        }"#,
    )
    .unwrap();
    std::fs::write(
        evidence_dir.join("evidence-index.json"),
        r#"{
          "generated_at": "2026-07-09T04:22:38Z",
          "overall_status": "pass",
          "certification": {
            "category": "certification",
            "status": "pass",
            "label": "Local certification",
            "path": "metrics.json",
            "summary_path": "summary.md",
            "generated_at": "2026-07-09T04:22:38Z"
          },
          "harness": {},
          "recall_quality": null,
          "missing": ["harness", "recall_quality"],
          "stale": []
        }"#,
    )
    .unwrap();
    let mut app = App::new(dir.path().join(".tree-ring"), None).unwrap();
    app.execute_slash_command("/evidence").unwrap();
    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|frame| render(frame, &app)).unwrap();
    let output = terminal.backend().to_string();

    assert!(output.contains("Local certification"));
    assert!(output.contains("6064 KB"));
    assert!(output.contains("3.729 ms"));
    assert!(output.contains("Agent Zero"));
}
```

Run:

```bash
cargo test -p tree-ring-memory-cli render_evidence_mode --locked
```

Expected: FAIL because render support for `AppMode::Evidence` does not exist.

- [ ] **Step 2: Render evidence in the header mode label**

In `render_header`, add:

```rust
AppMode::Evidence => "evidence",
```

- [ ] **Step 3: Route result rendering to evidence mode**

In `render_results`, add this branch before the integrations branch:

```rust
if app.mode == AppMode::Evidence {
    render_evidence_list(frame, area, app);
    return;
}
```

- [ ] **Step 4: Add evidence list rendering**

Add this function near `render_integrations`:

```rust
fn render_evidence_list(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let items = if let Some(snapshot) = &app.evidence_snapshot {
        let mut rows = Vec::new();
        let status = snapshot.status.as_str();
        rows.push(ListItem::new(Line::from(vec![
            Span::styled("* ", theme::secondary_accent()),
            Span::styled("Certification", theme::selected()),
            Span::styled(format!(" {status}"), theme::dim()),
        ])));
        rows.push(ListItem::new(Line::from(vec![
            Span::styled("  ", theme::dim()),
            Span::styled("Harness probes", theme::dim()),
            Span::styled(" missing", theme::dim()),
        ])));
        rows.push(ListItem::new(Line::from(vec![
            Span::styled("  ", theme::dim()),
            Span::styled("Recall quality", theme::dim()),
            Span::styled(" missing", theme::dim()),
        ])));
        rows
    } else {
        vec![ListItem::new(Line::from(Span::styled(
            "Run /evidence to load proof.",
            theme::dim(),
        )))]
    };
    frame.render_widget(List::new(items).block(theme::panel("Evidence")), area);
}
```

- [ ] **Step 5: Route detail rendering to evidence mode**

At the top of `render_detail`, before the integrations branch, add:

```rust
if app.mode == AppMode::Evidence {
    render_evidence_detail(frame, area, app);
    return;
}
```

- [ ] **Step 6: Add evidence detail rendering**

Add this function near `render_detail`:

```rust
fn render_evidence_detail(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = Vec::new();
    if let Some(snapshot) = &app.evidence_snapshot {
        lines.push(Line::from(vec![
            Span::styled("status ", theme::dim()),
            Span::styled(snapshot.status.as_str(), theme::accent()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("root ", theme::dim()),
            Span::raw(snapshot.root.display().to_string()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("index ", theme::dim()),
            Span::raw(snapshot.index_path.display().to_string()),
        ]));
        lines.push(Line::from(""));
        if let Some(certification) = &snapshot.certification {
            lines.push(Line::from(vec![
                Span::styled("Local certification ", theme::brand()),
                Span::styled(certification.status.as_str(), theme::dim()),
            ]));
            lines.push(Line::from(format!("generated {}", certification.generated_at)));
            if let Some(bytes) = certification.release_binary_bytes {
                lines.push(Line::from(format!("release binary {bytes} bytes")));
            }
            if let Some(project_kb) = certification.project_install_kb {
                lines.push(Line::from(format!("project install {project_kb} KB")));
            }
            if let Some(global_kb) = certification.global_install_kb {
                lines.push(Line::from(format!("global install {global_kb} KB")));
            }
            if let Some(rate) = certification.cli_import_events_per_second {
                lines.push(Line::from(format!("CLI import {rate}/s")));
            }
            if let Some(avg) = certification.recall_avg_ms_10000 {
                let max = certification.recall_max_ms_10000.unwrap_or(avg);
                lines.push(Line::from(format!("10k recall avg {avg:.3} ms max {max:.3} ms")));
            }
            if let Some(avg) = certification.recall_avg_ms_30000 {
                let max = certification.recall_max_ms_30000.unwrap_or(avg);
                lines.push(Line::from(format!("30k recall avg {avg:.3} ms max {max:.3} ms")));
            }
            if let Some(status) = &certification.agent_zero_status {
                lines.push(Line::from(format!(
                    "Agent Zero {} {}",
                    status,
                    certification.agent_zero_note.as_deref().unwrap_or("")
                )));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("metrics ", theme::dim()),
                Span::raw(certification.metrics_path.display().to_string()),
            ]));
        } else {
            lines.push(Line::from(snapshot.message.clone()));
            lines.push(Line::from("Run: sh scripts/certify-tree-ring.sh"));
        }
        lines.push(Line::from(""));
        lines.push(Line::from("Actions: /evidence refresh | /integrations"));
    } else {
        lines.push(Line::from("Run /evidence to load certification proof."));
    }
    let paragraph = Paragraph::new(lines)
        .block(theme::panel("Evidence Detail"))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}
```

- [ ] **Step 7: Run focused render tests**

Run:

```bash
cargo test -p tree-ring-memory-cli render_evidence_mode --locked
```

Expected: PASS with both evidence render tests passing.

- [ ] **Step 8: Commit Task 4**

Run:

```bash
git add crates/tree-ring-memory-cli/src/tui/render.rs
git commit -m "Render evidence browser in TUI"
```

---

### Task 5: Final Docs And Verification

**Files:**
- Modify: `docs/architecture/rust-core-status.md`

**Interfaces:**
- Consumes: completed Phase 1 evidence model, certification writer, TUI state, and render changes.
- Produces: PR-ready branch with final verification evidence.

- [ ] **Step 1: Add architecture status note**

In `docs/architecture/rust-core-status.md`, add this bullet near the certification and TUI status bullets:

```markdown
- The TUI includes `/evidence` for a read-only-first evidence browser backed
  by `target/tree-ring-certification/evidence-index.json` and existing
  certification metrics. Refresh certification is confirmation-gated and
  presents the external command instead of running a hidden background proof
  job.
```

- [ ] **Step 2: Run formatter check**

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

Expected: PASS for CLI, core, SQLite, and doc tests.

- [ ] **Step 4: Run Clippy**

Run:

```bash
cargo clippy --locked --all-targets
```

Expected: PASS with no new warnings.

- [ ] **Step 5: Run whitespace check**

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

Expected: certification passes and writes:

```text
target/tree-ring-certification/summary.md
target/tree-ring-certification/metrics.json
target/tree-ring-certification/evidence-index.json
```

- [ ] **Step 7: Inspect the evidence index**

Run:

```bash
cat target/tree-ring-certification/evidence-index.json
```

Expected: JSON contains `"overall_status": "pass"`, `"path": "metrics.json"`, `"summary_path": "summary.md"`, and `"missing": ["harness", "recall_quality"]`.

- [ ] **Step 8: Commit final docs**

Run:

```bash
git add docs/architecture/rust-core-status.md
git commit -m "Document evidence spine phase one"
```

---

## Plan Self-Review

- Spec coverage: Task 1 covers typed evidence readers. Task 2 covers certification index writing while preserving `metrics.json`. Tasks 3 and 4 cover `/evidence`, read-only default behavior, explicit confirmation, and rendering. Task 5 covers final docs and verification.
- Scope check: Harness probes and recall-quality diagnostics are represented as missing evidence categories only. They are not implemented in Phase 1.
- Type consistency: `EvidenceStatus`, `EvidenceIndex`, `EvidenceRecordRef`, `CertificationEvidence`, `EvidenceSnapshot`, `AppMode::Evidence`, `SlashCommand::Evidence`, and `ActionKind::RefreshCertification` are introduced before later tasks consume them.
- Behavior check: TUI refresh confirmation presents `sh scripts/certify-tree-ring.sh` as an external next step and does not run a hidden background command.
