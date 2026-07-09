# Tree Ring Harness Certification Matrix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Phase 2 harness proof records so local compatibility is represented by structured evidence instead of claims.

**Architecture:** Add a small harness evidence producer in the CLI crate that consumes the existing integration scan, writes `target/tree-ring-certification/harness/*.json`, and merges those records into `evidence-index.json`. Expose it through `tree-ring integrations certify`, run it from the repo certification script, and render per-harness status/path rows in `/evidence`.

**Tech Stack:** Rust 2021, Clap, serde/serde_json, chrono workspace dependency, Ratatui, POSIX shell certification script, cargo tests.

## Global Constraints

- Preserve existing `metrics.json` compatibility while adding richer evidence indexes beside it.
- TUI reads existing evidence by default.
- TUI evidence refresh actions require explicit confirmation.
- Do not add a daemon, background recorder, hidden proof runner, hidden durable memory writer, or background transcript capture.
- Do not change the SQLite schema or public JSONL memory schema.
- Do not make core memory logic depend on Codex, Claude Code, OpenCode, Goose, Pi, Agent Zero, or any other harness.
- Do not replace the current certification metrics format.
- Do not claim compatibility unless a local evidence producer generated a pass, fail, or skip record.
- Do not mutate third-party harness configuration by default.
- Do not implement recall-quality diagnostics in this phase.
- Do not implement `tree-ring integrations link` in this phase.
- Keep Tree Ring framework-agnostic: no Agent Zero core changes, no Codex-only assumptions, and no automatic global harness writes.
- Run `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked --all-targets`, `git diff --check`, and `sh scripts/certify-tree-ring.sh` before PR handoff.

---

## Scope Check

This plan implements Evidence Spine Phase 2 only: deterministic non-mutating harness probe evidence for Codex, Claude Code, OpenCode, Goose, Pi, and Agent Zero/A0. It does not implement recall-quality evidence, integration linking, vector search, daemon behavior, background capture, global harness configuration writes, or Agent Zero core changes.

## File Structure

- Create `crates/tree-ring-memory-cli/src/harness_evidence.rs`: harness probe records, status rules, record writers, and evidence-index merge logic.
- Modify `crates/tree-ring-memory-cli/src/main.rs`: add `tree-ring integrations certify` and print human/JSON reports.
- Modify `crates/tree-ring-memory-cli/Cargo.toml`: add `chrono.workspace = true` for Rust-side evidence timestamps.
- Modify `scripts/certify-tree-ring.sh`: run the new harness certification producer against temporary markers and merge its records into the repo certification index.
- Modify `crates/tree-ring-memory-cli/src/tui/render.rs`: render per-harness evidence status/path rows from the index.
- Modify `docs/architecture/rust-core-status.md` and `README.md`: document the new proof command and evidence artifacts after implementation.

---

### Task 1: Add Harness Evidence Producer

**Files:**
- Create: `crates/tree-ring-memory-cli/src/harness_evidence.rs`
- Modify: `crates/tree-ring-memory-cli/src/main.rs`
- Modify: `crates/tree-ring-memory-cli/Cargo.toml`

**Interfaces:**
- Consumes: `integrations::scan_integrations(root: &Path) -> IntegrationScanReport`
- Consumes: `evidence::EvidenceIndex`, `EvidenceRecordRef`, `EvidenceStatus`
- Produces: `harness_evidence::HarnessCertificationRequest`
- Produces: `harness_evidence::HarnessCertificationReport`
- Produces: `harness_evidence::HarnessProbeRecord`
- Produces: `harness_evidence::certify_harnesses(request: HarnessCertificationRequest) -> Result<HarnessCertificationReport, String>`
- Produces record files under `<evidence_dir>/harness/<harness-id>.json`
- Produces or updates `<evidence_dir>/evidence-index.json`

- [ ] **Step 1: Add failing harness evidence tests**

Create `crates/tree-ring-memory-cli/src/harness_evidence.rs` with the module skeleton and these tests:

```rust
use crate::evidence::{certification_dir_for_project, EvidenceRecordRef, EvidenceStatus};
use crate::integrations::{scan_integrations, IntegrationMarker, MarkerOrigin};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const CERTIFIED_HARNESS_IDS: &[&str] = &[
    "codex",
    "claude-code",
    "opencode",
    "goose",
    "pi",
    "agent-zero",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessCertificationRequest {
    pub source_root: PathBuf,
    pub evidence_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessCertificationReport {
    pub generated_at: String,
    pub source_root: PathBuf,
    pub evidence_dir: PathBuf,
    pub index_path: PathBuf,
    pub pass_count: usize,
    pub fail_count: usize,
    pub skip_count: usize,
    pub records: Vec<HarnessProbeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessProbeRecord {
    pub schema_version: u8,
    pub harness_id: String,
    pub name: String,
    pub status: EvidenceStatus,
    pub generated_at: String,
    pub source_root: PathBuf,
    pub command: String,
    pub markers: Vec<HarnessProbeMarker>,
    pub guidance: HarnessGuidanceEvidence,
    pub summary: String,
    pub next_step: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessProbeMarker {
    pub path: String,
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessGuidanceEvidence {
    pub agents_md: Option<PathBuf>,
    pub skill_md: Option<PathBuf>,
    pub cli_md: Option<PathBuf>,
    pub recall_guidance: bool,
    pub remember_guidance: bool,
}

pub fn certify_harnesses(
    _request: HarnessCertificationRequest,
) -> Result<HarnessCertificationReport, String> {
    Err("harness certification producer is still in the red phase".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn harness_certification_skips_absent_project_markers_and_indexes_records() {
        let dir = tempdir().unwrap();
        let evidence_dir = certification_dir_for_project(dir.path());

        let report = certify_harnesses(HarnessCertificationRequest {
            source_root: dir.path().to_path_buf(),
            evidence_dir: evidence_dir.clone(),
        })
        .unwrap();

        assert_eq!(report.records.len(), CERTIFIED_HARNESS_IDS.len());
        assert_eq!(report.pass_count, 0);
        assert_eq!(report.fail_count, 0);
        assert_eq!(report.skip_count, CERTIFIED_HARNESS_IDS.len());
        assert!(evidence_dir.join("harness/codex.json").exists());
        let index = std::fs::read_to_string(evidence_dir.join("evidence-index.json")).unwrap();
        assert!(index.contains("\"harness\""));
        assert!(index.contains("\"codex\""));
        assert!(index.contains("\"status\":\"skip\""));
        assert!(!dir.path().join(".codex/generated-by-certify").exists());
    }

    #[test]
    fn harness_certification_passes_project_marker_with_generated_guidance() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codex")).unwrap();
        std::fs::create_dir_all(dir.path().join(".tree-ring")).unwrap();
        std::fs::write(
            dir.path().join(".tree-ring/SKILL.md"),
            "Use `tree-ring recall` before acting and `tree-ring remember` for durable facts.",
        )
        .unwrap();
        std::fs::write(
            dir.path().join(".tree-ring/CLI.md"),
            "`tree-ring recall` and `tree-ring remember` are the portable command surface.",
        )
        .unwrap();
        std::fs::write(
            dir.path().join(".tree-ring/AGENTS.md"),
            "Project guidance delegates to SKILL.md and CLI.md.",
        )
        .unwrap();
        let evidence_dir = certification_dir_for_project(dir.path());

        let report = certify_harnesses(HarnessCertificationRequest {
            source_root: dir.path().to_path_buf(),
            evidence_dir: evidence_dir.clone(),
        })
        .unwrap();

        let codex = report
            .records
            .iter()
            .find(|record| record.harness_id == "codex")
            .unwrap();
        assert_eq!(codex.status, EvidenceStatus::Pass);
        assert!(codex.summary.contains("project marker"));
        assert!(codex.guidance.recall_guidance);
        assert!(codex.guidance.remember_guidance);
        assert!(evidence_dir.join("harness/codex.json").exists());
    }

    #[test]
    fn harness_certification_fails_project_marker_without_generated_guidance() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Claude instructions").unwrap();
        let evidence_dir = certification_dir_for_project(dir.path());

        let report = certify_harnesses(HarnessCertificationRequest {
            source_root: dir.path().to_path_buf(),
            evidence_dir,
        })
        .unwrap();

        let claude = report
            .records
            .iter()
            .find(|record| record.harness_id == "claude-code")
            .unwrap();
        assert_eq!(claude.status, EvidenceStatus::Fail);
        assert!(claude.summary.contains("missing generated Tree Ring guidance"));
        assert!(claude.next_step.contains("tree-ring init"));
    }

    #[test]
    fn harness_certification_preserves_existing_certification_index_entry() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codex")).unwrap();
        let evidence_dir = certification_dir_for_project(dir.path());
        std::fs::create_dir_all(&evidence_dir).unwrap();
        std::fs::write(
            evidence_dir.join("metrics.json"),
            r#"{"ok":true,"created_at":"2026-07-09T05:44:48Z"}"#,
        )
        .unwrap();
        std::fs::write(
            evidence_dir.join("evidence-index.json"),
            r#"{
              "generated_at": "2026-07-09T05:44:48Z",
              "overall_status": "pass",
              "certification": {
                "category": "certification",
                "status": "pass",
                "label": "Local certification",
                "path": "metrics.json",
                "summary_path": "summary.md",
                "generated_at": "2026-07-09T05:44:48Z"
              },
              "harness": {},
              "recall_quality": null,
              "missing": ["harness", "recall_quality"],
              "stale": []
            }"#,
        )
        .unwrap();

        certify_harnesses(HarnessCertificationRequest {
            source_root: dir.path().to_path_buf(),
            evidence_dir: evidence_dir.clone(),
        })
        .unwrap();

        let index = std::fs::read_to_string(evidence_dir.join("evidence-index.json")).unwrap();
        assert!(index.contains("\"certification\""));
        assert!(index.contains("\"metrics.json\""));
        assert!(index.contains("\"harness/codex.json\""));
        assert!(!index.contains("\"harness\", \"recall_quality\""));
    }
}
```

Run:

```bash
cargo test -p tree-ring-memory-cli harness_certification --locked
```

Expected: FAIL because the producer returns the red-phase error and the module is not declared.

- [ ] **Step 2: Add module declaration and chrono dependency**

Modify `crates/tree-ring-memory-cli/src/main.rs` module declarations:

```rust
mod actions;
mod agent_awareness;
mod commands;
mod evidence;
mod harness_evidence;
mod integrations;
mod ring_mark;
mod tui;
mod welcome;
```

Modify `crates/tree-ring-memory-cli/Cargo.toml` dependencies:

```toml
chrono.workspace = true
```

- [ ] **Step 3: Implement harness evidence status rules**

Replace the red-phase `certify_harnesses` implementation with these helper functions and logic:

```rust
pub fn certify_harnesses(
    request: HarnessCertificationRequest,
) -> Result<HarnessCertificationReport, String> {
    let generated_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let report = scan_integrations(&request.source_root);
    let guidance = inspect_guidance(&request.source_root);
    let harness_dir = request.evidence_dir.join("harness");
    fs::create_dir_all(&harness_dir).map_err(|err| err.to_string())?;

    let mut records = Vec::new();
    for harness_id in CERTIFIED_HARNESS_IDS {
        let integration = report
            .integrations
            .iter()
            .find(|integration| integration.id == *harness_id)
            .ok_or_else(|| format!("missing integration definition for {harness_id}"))?;
        let record = probe_record(integration, &request.source_root, &generated_at, &guidance);
        let path = harness_dir.join(format!("{harness_id}.json"));
        let json = serde_json::to_string_pretty(&record).map_err(|err| err.to_string())?;
        fs::write(&path, json).map_err(|err| err.to_string())?;
        records.push(record);
    }

    let index_path = merge_harness_index(&request.evidence_dir, &generated_at, &records)?;
    let pass_count = records
        .iter()
        .filter(|record| record.status == EvidenceStatus::Pass)
        .count();
    let fail_count = records
        .iter()
        .filter(|record| record.status == EvidenceStatus::Fail)
        .count();
    let skip_count = records
        .iter()
        .filter(|record| record.status == EvidenceStatus::Skip)
        .count();

    Ok(HarnessCertificationReport {
        generated_at,
        source_root: request.source_root,
        evidence_dir: request.evidence_dir,
        index_path,
        pass_count,
        fail_count,
        skip_count,
        records,
    })
}

fn probe_record(
    integration: &crate::integrations::AgentIntegration,
    source_root: &Path,
    generated_at: &str,
    guidance: &HarnessGuidanceEvidence,
) -> HarnessProbeRecord {
    let project_marker = integration
        .markers
        .iter()
        .any(|marker| marker.origin == MarkerOrigin::Project);
    let home_marker = integration
        .markers
        .iter()
        .any(|marker| marker.origin == MarkerOrigin::Home);
    let guidance_ready = guidance.recall_guidance && guidance.remember_guidance;
    let (status, summary, next_step) = if project_marker && guidance_ready {
        (
            EvidenceStatus::Pass,
            format!(
                "{} has a project marker and generated Tree Ring recall/remember guidance.",
                integration.name
            ),
            integration.next_step.to_string(),
        )
    } else if project_marker {
        (
            EvidenceStatus::Fail,
            format!(
                "{} has a project marker but is missing generated Tree Ring guidance.",
                integration.name
            ),
            "Run `tree-ring init`, then reference `.tree-ring/SKILL.md` and `.tree-ring/CLI.md` from the harness project instructions.".to_string(),
        )
    } else if home_marker {
        (
            EvidenceStatus::Skip,
            format!(
                "{} only has user-home markers; this project is not certified for that harness.",
                integration.name
            ),
            "Add a project-level harness marker or project instruction file, then rerun `tree-ring integrations certify`.".to_string(),
        )
    } else {
        (
            EvidenceStatus::Skip,
            format!(
                "{} was not detected for this project, so no compatibility claim is made.",
                integration.name
            ),
            integration.next_step.to_string(),
        )
    };

    HarnessProbeRecord {
        schema_version: 1,
        harness_id: integration.id.to_string(),
        name: integration.name.to_string(),
        status,
        generated_at: generated_at.to_string(),
        source_root: source_root.to_path_buf(),
        command: format!(
            "tree-ring integrations certify --source-root {}",
            shell_display(source_root)
        ),
        markers: integration.markers.iter().map(marker_from_scan).collect(),
        guidance: guidance.clone(),
        summary,
        next_step,
    }
}

fn inspect_guidance(source_root: &Path) -> HarnessGuidanceEvidence {
    let agents_md = existing_path(source_root.join(".tree-ring/AGENTS.md"));
    let skill_md = existing_path(source_root.join(".tree-ring/SKILL.md"));
    let cli_md = existing_path(source_root.join(".tree-ring/CLI.md"));
    let combined = [agents_md.as_ref(), skill_md.as_ref(), cli_md.as_ref()]
        .into_iter()
        .flatten()
        .filter_map(|path| fs::read_to_string(path).ok())
        .collect::<Vec<_>>()
        .join("\n");
    HarnessGuidanceEvidence {
        agents_md,
        skill_md,
        cli_md,
        recall_guidance: combined.contains("tree-ring recall"),
        remember_guidance: combined.contains("tree-ring remember"),
    }
}

fn existing_path(path: PathBuf) -> Option<PathBuf> {
    path.exists().then_some(path)
}

fn marker_from_scan(marker: &IntegrationMarker) -> HarnessProbeMarker {
    HarnessProbeMarker {
        path: marker.path.clone(),
        origin: marker.origin.as_str().to_string(),
    }
}

fn shell_display(path: &Path) -> String {
    path.display().to_string()
}
```

- [ ] **Step 4: Implement evidence-index merge**

Add this merge logic to `harness_evidence.rs`:

```rust
fn merge_harness_index(
    evidence_dir: &Path,
    generated_at: &str,
    records: &[HarnessProbeRecord],
) -> Result<PathBuf, String> {
    fs::create_dir_all(evidence_dir).map_err(|err| err.to_string())?;
    let index_path = evidence_dir.join("evidence-index.json");
    let mut index = read_or_create_index(evidence_dir, generated_at)?;
    index.generated_at = generated_at.to_string();
    index.harness = records
        .iter()
        .map(|record| {
            (
                record.harness_id.clone(),
                EvidenceRecordRef {
                    category: "harness".to_string(),
                    status: record.status,
                    label: record.name.clone(),
                    path: PathBuf::from(format!("harness/{}.json", record.harness_id)),
                    summary_path: None,
                    generated_at: record.generated_at.clone(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    index.missing.retain(|item| item != "harness");
    if index.recall_quality.is_none() && !index.missing.iter().any(|item| item == "recall_quality") {
        index.missing.push("recall_quality".to_string());
    }
    index.missing.sort();
    index.missing.dedup();
    index.overall_status = rollup_status(&index);
    let json = serde_json::to_string_pretty(&index).map_err(|err| err.to_string())?;
    fs::write(&index_path, json).map_err(|err| err.to_string())?;
    Ok(index_path)
}

fn read_or_create_index(evidence_dir: &Path, generated_at: &str) -> Result<crate::evidence::EvidenceIndex, String> {
    let index_path = evidence_dir.join("evidence-index.json");
    if index_path.exists() {
        let input = fs::read_to_string(&index_path).map_err(|err| err.to_string())?;
        return serde_json::from_str(&input).map_err(|err| err.to_string());
    }
    Ok(crate::evidence::EvidenceIndex {
        generated_at: generated_at.to_string(),
        overall_status: EvidenceStatus::Missing,
        certification: certification_record_from_metrics(evidence_dir, generated_at),
        harness: BTreeMap::new(),
        recall_quality: None,
        missing: vec!["recall_quality".to_string()],
        stale: Vec::new(),
    })
}

fn certification_record_from_metrics(
    evidence_dir: &Path,
    generated_at: &str,
) -> Option<EvidenceRecordRef> {
    let metrics_path = evidence_dir.join("metrics.json");
    if !metrics_path.exists() {
        return None;
    }
    let summary_path = evidence_dir.join("summary.md");
    Some(EvidenceRecordRef {
        category: "certification".to_string(),
        status: EvidenceStatus::Pass,
        label: "Local certification".to_string(),
        path: PathBuf::from("metrics.json"),
        summary_path: summary_path.exists().then_some(PathBuf::from("summary.md")),
        generated_at: generated_at.to_string(),
    })
}

fn rollup_status(index: &crate::evidence::EvidenceIndex) -> EvidenceStatus {
    if index.harness.values().any(|record| record.status == EvidenceStatus::Fail) {
        return EvidenceStatus::Fail;
    }
    if index.harness.values().any(|record| record.status == EvidenceStatus::Error) {
        return EvidenceStatus::Error;
    }
    if index.harness.values().any(|record| record.status == EvidenceStatus::Skip) {
        return EvidenceStatus::Skip;
    }
    if let Some(certification) = &index.certification {
        return certification.status;
    }
    if index.harness.is_empty() {
        EvidenceStatus::Missing
    } else {
        EvidenceStatus::Pass
    }
}
```

Run:

```bash
cargo test -p tree-ring-memory-cli harness_certification --locked
cargo fmt --check
```

Expected: PASS for harness certification tests and formatter.

- [ ] **Step 5: Commit Task 1**

Run:

```bash
git add crates/tree-ring-memory-cli/Cargo.toml crates/tree-ring-memory-cli/src/main.rs crates/tree-ring-memory-cli/src/harness_evidence.rs
git commit -m "Add harness evidence producer"
```

---

### Task 2: Add `integrations certify` CLI Command

**Files:**
- Modify: `crates/tree-ring-memory-cli/src/main.rs`

**Interfaces:**
- Consumes: `harness_evidence::certify_harnesses`
- Produces: `tree-ring integrations certify --source-root <path> [--out-dir <path>]`
- Produces JSON output shape `{"ok":true,"report":HarnessCertificationReport}` when `--json` is set

- [ ] **Step 1: Add failing CLI tests**

Add these tests to the existing `#[cfg(test)] mod tests` in `crates/tree-ring-memory-cli/src/main.rs`:

```rust
#[test]
fn integrations_certify_writes_harness_evidence_without_memory_store() {
    let dir = tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".codex")).unwrap();
    fs::create_dir_all(dir.path().join(".tree-ring")).unwrap();
    fs::write(
        dir.path().join(".tree-ring/SKILL.md"),
        "Use `tree-ring recall` and `tree-ring remember`.",
    )
    .unwrap();
    fs::write(
        dir.path().join(".tree-ring/CLI.md"),
        "`tree-ring recall` and `tree-ring remember` are available.",
    )
    .unwrap();
    let root = dir.path().join(".tree-ring-memory");
    let out_dir = dir.path().join("proof");

    run(Cli::parse_from([
        "tree-ring",
        "--root",
        root.to_str().unwrap(),
        "--json",
        "integrations",
        "certify",
        "--source-root",
        dir.path().to_str().unwrap(),
        "--out-dir",
        out_dir.to_str().unwrap(),
    ]))
    .unwrap();

    assert!(!root.join("memory.sqlite").exists());
    assert!(out_dir.join("harness/codex.json").exists());
    let index = fs::read_to_string(out_dir.join("evidence-index.json")).unwrap();
    assert!(index.contains("\"codex\""));
    assert!(index.contains("\"status\":\"pass\""));
}

#[test]
fn integrations_certify_defaults_to_project_certification_dir() {
    let dir = tempdir().unwrap();

    run(Cli::parse_from([
        "tree-ring",
        "integrations",
        "certify",
        "--source-root",
        dir.path().to_str().unwrap(),
    ]))
    .unwrap();

    assert!(dir
        .path()
        .join("target/tree-ring-certification/harness/codex.json")
        .exists());
}
```

Run:

```bash
cargo test -p tree-ring-memory-cli integrations_certify --locked
```

Expected: FAIL because `IntegrationCommand::Certify` does not exist.

- [ ] **Step 2: Add the CLI variant**

Modify `IntegrationCommand` in `main.rs`:

```rust
#[derive(Debug, Subcommand)]
enum IntegrationCommand {
    #[command(about = "scan a project root for known agent-framework markers")]
    Scan {
        #[arg(long, default_value = ".", help = "project root to scan")]
        source_root: PathBuf,
    },
    #[command(about = "write non-mutating harness certification evidence")]
    Certify {
        #[arg(long, default_value = ".", help = "project root to certify")]
        source_root: PathBuf,
        #[arg(
            long,
            help = "evidence output directory; defaults to <source-root>/target/tree-ring-certification"
        )]
        out_dir: Option<PathBuf>,
    },
}
```

- [ ] **Step 3: Route `integrations certify` before opening the store**

Add imports near the existing integration imports:

```rust
use harness_evidence::{certify_harnesses, HarnessCertificationReport, HarnessCertificationRequest};
```

Add this branch in `run` immediately after the existing `integrations scan` early return:

```rust
if let Command::Integrations {
    command:
        IntegrationCommand::Certify {
            source_root,
            out_dir,
        },
} = &cli.command
{
    let evidence_dir = out_dir
        .clone()
        .unwrap_or_else(|| evidence::certification_dir_for_project(source_root));
    let report = certify_harnesses(HarnessCertificationRequest {
        source_root: source_root.clone(),
        evidence_dir,
    })?;
    print_harness_certification_report(&report, cli.json)?;
    return Ok(());
}
```

Add this match arm in the final `match cli.command` unreachable section:

```rust
Command::Integrations { .. } => {
    unreachable!("integrations commands return before opening the scriptable store")
}
```

Replace the existing `integrations scan returns before opening the scriptable store` text with the generic text above.

- [ ] **Step 4: Add report printing**

Add this helper near `print_integration_report`:

```rust
fn print_harness_certification_report(
    report: &HarnessCertificationReport,
    json_output: bool,
) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            json!({
                "ok": true,
                "report": report,
            })
        );
    } else {
        println!(
            "Tree Ring Memory harness certification: pass={} fail={} skip={} evidence={}",
            report.pass_count,
            report.fail_count,
            report.skip_count,
            report.evidence_dir.display()
        );
        for record in &report.records {
            println!(
                "{} [{}] {}",
                record.name,
                record.status.as_str(),
                record.summary
            );
            println!("  next: {}", record.next_step);
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Run focused command tests**

Run:

```bash
cargo test -p tree-ring-memory-cli integrations_certify --locked
cargo test -p tree-ring-memory-cli harness_certification --locked
```

Expected: PASS.

- [ ] **Step 6: Commit Task 2**

Run:

```bash
git add crates/tree-ring-memory-cli/src/main.rs
git commit -m "Add harness certification CLI"
```

---

### Task 3: Run Harness Certification From The Repo Certification Script

**Files:**
- Modify: `scripts/certify-tree-ring.sh`

**Interfaces:**
- Consumes: `tree-ring integrations certify --source-root "$scan_root" --out-dir "$OUT_DIR"`
- Produces: `target/tree-ring-certification/harness/*.json`
- Updates: `target/tree-ring-certification/evidence-index.json` with harness record refs

- [ ] **Step 1: Add harness markers and generated guidance to the certification scan root**

In `scripts/certify-tree-ring.sh`, keep the existing `scan_root` and `scan_home` setup, then add generated guidance files before the existing integration scan:

```sh
mkdir -p "$scan_root/.tree-ring"
cat > "$scan_root/.tree-ring/SKILL.md" <<'EOF'
Use `tree-ring recall` before acting on project assumptions.
Use `tree-ring remember` only for durable, non-secret project facts.
EOF
cat > "$scan_root/.tree-ring/CLI.md" <<'EOF'
The portable command surface is `tree-ring recall` and `tree-ring remember`.
EOF
cat > "$scan_root/.tree-ring/AGENTS.md" <<'EOF'
Project harnesses should reference SKILL.md and CLI.md for Tree Ring Memory.
EOF
```

- [ ] **Step 2: Run the harness certification producer after the initial evidence index is written**

Move the existing `cat > "$INDEX" <<EOF` evidence-index block so it still runs after `summary.md`, then immediately after that block add:

```sh
"$BIN" --json integrations certify --source-root "$scan_root" --out-dir "$OUT_DIR" \
  > "$OUT_DIR/harness-certification.json"
require_file "$OUT_DIR/harness/codex.json"
require_file "$OUT_DIR/harness/claude-code.json"
require_file "$OUT_DIR/harness/opencode.json"
require_file "$OUT_DIR/harness/goose.json"
require_file "$OUT_DIR/harness/pi.json"
require_file "$OUT_DIR/harness/agent-zero.json"
grep -F '"harness"' "$INDEX" > /dev/null \
  || fail "evidence index did not include harness records"
grep -F '"codex"' "$INDEX" > /dev/null \
  || fail "evidence index did not include Codex harness record"
```

Keep this call before `log "certification passed"`.

- [ ] **Step 3: Print the harness evidence path**

At the bottom of the script, after:

```sh
printf 'Evidence index: %s\n' "$INDEX"
```

add:

```sh
printf 'Harness evidence: %s\n' "$OUT_DIR/harness"
```

- [ ] **Step 4: Run script checks**

Run:

```bash
sh -n scripts/certify-tree-ring.sh
cargo test -p tree-ring-memory-cli harness_certification --locked
sh scripts/certify-tree-ring.sh
test -f target/tree-ring-certification/harness/codex.json
grep -F '"codex"' target/tree-ring-certification/evidence-index.json
```

Expected: shell syntax passes, focused tests pass, certification passes, `harness/codex.json` exists, and `evidence-index.json` contains a `codex` harness entry.

- [ ] **Step 5: Commit Task 3**

Run:

```bash
git add scripts/certify-tree-ring.sh
git commit -m "Write harness evidence during certification"
```

---

### Task 4: Render Harness Evidence In `/evidence`

**Files:**
- Modify: `crates/tree-ring-memory-cli/src/tui/render.rs`

**Interfaces:**
- Consumes: `EvidenceSnapshot.index.harness: BTreeMap<String, EvidenceRecordRef>`
- Renders: per-harness status and relative evidence path

- [ ] **Step 1: Add failing render test**

Add this test to `crates/tree-ring-memory-cli/src/tui/render.rs`:

```rust
#[test]
fn render_evidence_mode_shows_harness_matrix_records() {
    let dir = tempdir().unwrap();
    let evidence_dir = dir.path().join("target/tree-ring-certification");
    std::fs::create_dir_all(evidence_dir.join("harness")).unwrap();
    std::fs::write(evidence_dir.join("summary.md"), "# Summary\n").unwrap();
    std::fs::write(
        evidence_dir.join("metrics.json"),
        r#"{"ok":true,"created_at":"2026-07-09T05:44:48Z"}"#,
    )
    .unwrap();
    std::fs::write(evidence_dir.join("harness/codex.json"), "{}").unwrap();
    std::fs::write(evidence_dir.join("harness/claude-code.json"), "{}").unwrap();
    std::fs::write(
        evidence_dir.join("evidence-index.json"),
        r#"{
          "generated_at": "2026-07-09T05:44:48Z",
          "overall_status": "fail",
          "certification": {
            "category": "certification",
            "status": "pass",
            "label": "Local certification",
            "path": "metrics.json",
            "summary_path": "summary.md",
            "generated_at": "2026-07-09T05:44:48Z"
          },
          "harness": {
            "codex": {
              "category": "harness",
              "status": "pass",
              "label": "Codex",
              "path": "harness/codex.json",
              "summary_path": null,
              "generated_at": "2026-07-09T05:44:48Z"
            },
            "claude-code": {
              "category": "harness",
              "status": "fail",
              "label": "Claude Code",
              "path": "harness/claude-code.json",
              "summary_path": null,
              "generated_at": "2026-07-09T05:44:48Z"
            }
          },
          "recall_quality": null,
          "missing": ["recall_quality"],
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

    assert!(output.contains("Codex"));
    assert!(output.contains("pass"));
    assert!(output.contains("Claude Code"));
    assert!(output.contains("fail"));
    assert!(output.contains("harness/codex.json"));
}
```

Run:

```bash
cargo test -p tree-ring-memory-cli render_evidence_mode_shows_harness_matrix_records --locked
```

Expected: FAIL because the current renderer only shows `Harness probes loaded`.

- [ ] **Step 2: Render per-harness rows in the Evidence list**

Update `render_evidence_list` so the harness section expands records when `index.harness` is non-empty:

```rust
if let Some(index) = snapshot.index.as_ref() {
    if index.harness.is_empty() {
        rows.push(ListItem::new(Line::from(vec![
            Span::styled("  ", theme::dim()),
            Span::styled("Harness probes", theme::dim()),
            Span::styled(harness_status, theme::dim()),
        ])));
    } else {
        for record in index.harness.values().take(6) {
            rows.push(ListItem::new(Line::from(vec![
                Span::styled("  ", theme::dim()),
                Span::styled(format!("{:<18}", record.label), theme::selected()),
                Span::styled(format!(" {}", record.status.as_str()), theme::dim()),
            ])));
        }
    }
}
```

Keep the existing recall-quality row.

- [ ] **Step 3: Render harness paths in the Evidence detail pane**

In `render_evidence_detail`, after certification metrics/path lines and before the actions line, add:

```rust
if let Some(index) = snapshot.index.as_ref() {
    if !index.harness.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("Harness matrix", theme::brand())));
        for record in index.harness.values().take(6) {
            lines.push(Line::from(format!(
                "{} {} {}",
                record.label,
                record.status.as_str(),
                record.path.display()
            )));
        }
    }
}
```

- [ ] **Step 4: Run focused render tests**

Run:

```bash
cargo test -p tree-ring-memory-cli render_evidence_mode --locked
```

Expected: PASS with the existing evidence render tests and the new harness matrix render test.

- [ ] **Step 5: Commit Task 4**

Run:

```bash
git add crates/tree-ring-memory-cli/src/tui/render.rs
git commit -m "Render harness evidence matrix"
```

---

### Task 5: Document Harness Certification And Verify

**Files:**
- Modify: `README.md`
- Modify: `docs/architecture/rust-core-status.md`

**Interfaces:**
- Consumes: `tree-ring integrations certify`
- Consumes: `target/tree-ring-certification/harness/*.json`
- Produces: PR-ready final verification evidence

- [ ] **Step 1: Update README command coverage**

In `README.md`, near the existing `integrations scan` command documentation, add:

```markdown
tree-ring integrations certify --source-root .
```

Add this concise behavior note near the integration-scan explanation:

```markdown
- `integrations certify` writes non-mutating harness evidence under
  `target/tree-ring-certification/harness/` and updates
  `target/tree-ring-certification/evidence-index.json`. Pass, fail, and skip
  states are evidence records, not broad compatibility claims.
```

- [ ] **Step 2: Update architecture status**

In `docs/architecture/rust-core-status.md`, update the certification/TUI bullets to include:

```markdown
- `tree-ring integrations certify` turns integration-scan markers into
  non-mutating harness evidence records for Codex, Claude Code, OpenCode,
  Goose, Pi, and Agent Zero/A0. Records live under
  `target/tree-ring-certification/harness/` and are indexed by
  `evidence-index.json`; skip states are explicit and are not counted as
  compatibility passes.
```

- [ ] **Step 3: Run full verification**

Run:

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets
git diff --check
sh scripts/certify-tree-ring.sh
```

Expected: all pass.

- [ ] **Step 4: Inspect generated harness evidence**

Run:

```bash
test -f target/tree-ring-certification/harness/codex.json
test -f target/tree-ring-certification/harness/claude-code.json
test -f target/tree-ring-certification/harness/opencode.json
test -f target/tree-ring-certification/harness/goose.json
test -f target/tree-ring-certification/harness/pi.json
test -f target/tree-ring-certification/harness/agent-zero.json
grep -F '"codex"' target/tree-ring-certification/evidence-index.json
grep -F '"recall_quality"' target/tree-ring-certification/evidence-index.json
```

Expected: all harness evidence files exist, `evidence-index.json` contains harness entries, and `recall_quality` remains present as the missing follow-up category.

- [ ] **Step 5: Commit Task 5**

Run:

```bash
git add README.md docs/architecture/rust-core-status.md
git commit -m "Document harness certification matrix"
```

---

## Plan Self-Review

- Spec coverage: Task 1 creates non-mutating harness evidence producers for the Phase 2 harness set. Task 2 exposes the producer as a repeatable command. Task 3 runs it from certification and updates the evidence index. Task 4 surfaces per-harness status/path in `/evidence`. Task 5 documents and verifies the workflow.
- Scope check: Recall-quality diagnostics, integration linking, daemon/background behavior, global harness writes, and Agent Zero core modifications are excluded.
- Type consistency: `HarnessCertificationRequest`, `HarnessCertificationReport`, `HarnessProbeRecord`, `HarnessGuidanceEvidence`, and `certify_harnesses` are defined in Task 1 before CLI, script, TUI, and docs tasks consume them.
- Compatibility check: `metrics.json` remains unchanged; harness proof is added through `harness/*.json` plus `evidence-index.json` harness refs.
- Truthfulness check: skip states are explicit records, failed project markers do not become passes, and home-only markers do not certify the current project.
