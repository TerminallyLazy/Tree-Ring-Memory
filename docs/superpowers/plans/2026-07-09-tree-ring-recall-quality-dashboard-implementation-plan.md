# Tree Ring Recall Quality Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Phase 3 of the evidence spine: deterministic recall-quality diagnostics that prove recall speed and relevance without using private memories, then surface the evidence in `/evidence`.

**Architecture:** Add a non-mutating recall-quality evidence producer in the CLI crate. It creates a deterministic safe fixture store in memory, runs curated recall queries with ranking explanations enabled, writes a compact evidence record under `target/tree-ring-certification/recall-quality/default-fixture-v1.json`, merges that record into `evidence-index.json`, and lets the TUI render the record through the existing evidence browser.

**Tech Stack:** Rust 2021, Clap, serde/serde_json, chrono, existing `tree-ring-memory-core`, existing `tree-ring-memory-sqlite`, Ratatui, shell certification script, cargo tests.

## Global Constraints

- Do not use real user memory or private payloads for recall-quality diagnostics.
- The default evidence path is `target/tree-ring-certification/recall-quality/default-fixture-v1.json`.
- Recall-quality evidence must record returned ids, rank positions, score breakdowns, and latency.
- Query results must support `pass`, `fail`, and `needs_review` statuses.
- The TUI `/evidence` view must show recall-quality evidence without leaking memory summaries or details.
- Do not add new runtime dependencies.
- Do not change recall ranking semantics except where a test explicitly proves an existing bug.
- Do not run hidden background certification from the TUI; `/evidence refresh` remains an explicit confirmation-only workflow.
- Run `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked --all-targets`, `git diff --check`, and `sh scripts/certify-tree-ring.sh` before PR handoff.

---

## File Structure

- Modify `crates/tree-ring-memory-cli/src/evidence.rs`: add `EvidenceStatus::NeedsReview`, shared index helpers, recall-quality payload summary loading, and tests.
- Modify `crates/tree-ring-memory-cli/src/harness_evidence.rs`: reuse shared evidence-index helpers and shared rollup logic.
- Create `crates/tree-ring-memory-cli/src/recall_quality.rs`: deterministic fixture, query runner, evaluation logic, evidence JSON writer, index merge, and focused tests.
- Modify `crates/tree-ring-memory-cli/src/main.rs`: expose `tree-ring recall-quality`, print JSON/human reports, and add CLI contract tests.
- Modify `crates/tree-ring-memory-cli/src/tui/render.rs`: render recall-quality list/detail rows and add a render test.
- Modify `scripts/certify-tree-ring.sh`: run the recall-quality command and assert the evidence record/index are present.
- Modify `README.md`: document the new recall-quality evidence command and certification artifact.

---

### Task 1: Shared Evidence Index And Recall-Quality Reader

**Files:**
- Modify: `crates/tree-ring-memory-cli/src/evidence.rs`
- Modify: `crates/tree-ring-memory-cli/src/harness_evidence.rs`

**Interfaces:**
- Produces: `EvidenceStatus::NeedsReview`.
- Produces: `pub(crate) fn read_or_create_index(evidence_dir: &Path, generated_at: &str) -> Result<EvidenceIndex, String>`.
- Produces: `pub(crate) fn write_index(evidence_dir: &Path, index: &EvidenceIndex) -> Result<PathBuf, String>`.
- Produces: `pub(crate) fn rollup_index_status(index: &EvidenceIndex) -> EvidenceStatus`.
- Produces: `pub struct RecallQualityEvidence` and `pub struct RecallQualityQueryEvidence`.
- Produces: `EvidenceSnapshot { recall_quality: Option<RecallQualityEvidence>, ... }`.
- Consumes: existing `EvidenceIndex`, `EvidenceRecordRef`, and certification reader behavior.

- [ ] **Step 1: Add the failing evidence reader tests**

Add tests in `crates/tree-ring-memory-cli/src/evidence.rs`:

```rust
#[test]
fn evidence_snapshot_loads_recall_quality_record_from_index() {
    let dir = tempdir().unwrap();
    let evidence_dir = certification_dir_for_project(dir.path());
    fs::create_dir_all(evidence_dir.join("recall-quality")).unwrap();
    fs::write(evidence_dir.join("metrics.json"), r#"{"ok":true,"created_at":"2026-07-09T04:22:38Z"}"#).unwrap();
    fs::write(
        evidence_dir.join("recall-quality/default-fixture-v1.json"),
        r#"{
          "schema_version": 1,
          "generated_at": "2026-07-09T06:00:00Z",
          "query_set_id": "default-fixture-v1",
          "status": "pass",
          "summary": {
            "query_count": 2,
            "pass_count": 2,
            "fail_count": 0,
            "needs_review_count": 0,
            "avg_latency_ms": 1.25,
            "max_latency_ms": 2.5
          },
          "queries": [
            {
              "query_id": "scar-stale-cache",
              "query": "failure stale cache",
              "status": "pass",
              "expected_top_id": "rq_scar_stale_cache",
              "expected_rank": 1,
              "latency_ms": 0.9,
              "returned": [
                {
                  "id": "rq_scar_stale_cache",
                  "rank": 1,
                  "ring": "scar",
                  "source_ref": "recall-quality/scar-stale-cache",
                  "score": 1.2,
                  "ranking": {"textual_match": 1.0}
                }
              ],
              "notes": []
            }
          ]
        }"#,
    )
    .unwrap();
    fs::write(
        evidence_dir.join("evidence-index.json"),
        r#"{
          "generated_at": "2026-07-09T06:00:00Z",
          "overall_status": "pass",
          "certification": {
            "category": "certification",
            "status": "pass",
            "label": "Local certification",
            "path": "metrics.json",
            "summary_path": null,
            "generated_at": "2026-07-09T04:22:38Z"
          },
          "harness": {},
          "recall_quality": {
            "category": "recall_quality",
            "status": "pass",
            "label": "Recall quality",
            "path": "recall-quality/default-fixture-v1.json",
            "summary_path": null,
            "generated_at": "2026-07-09T06:00:00Z"
          },
          "missing": [],
          "stale": []
        }"#,
    )
    .unwrap();

    let snapshot = load_snapshot(&evidence_dir);

    let recall_quality = snapshot.recall_quality.unwrap();
    assert_eq!(recall_quality.status, EvidenceStatus::Pass);
    assert_eq!(recall_quality.query_set_id, "default-fixture-v1");
    assert_eq!(recall_quality.query_count, 2);
    assert_eq!(recall_quality.pass_count, 2);
    assert_eq!(recall_quality.fail_count, 0);
    assert_eq!(recall_quality.needs_review_count, 0);
    assert_eq!(recall_quality.avg_latency_ms, Some(1.25));
    assert_eq!(recall_quality.max_latency_ms, Some(2.5));
    assert_eq!(recall_quality.queries[0].query_id, "scar-stale-cache");
    assert_eq!(recall_quality.queries[0].returned_ids, vec!["rq_scar_stale_cache"]);
}

#[test]
fn rollup_index_status_marks_recall_quality_needs_review() {
    let mut index = EvidenceIndex {
        generated_at: "2026-07-09T06:00:00Z".to_string(),
        overall_status: EvidenceStatus::Pass,
        certification: Some(EvidenceRecordRef {
            category: "certification".to_string(),
            status: EvidenceStatus::Pass,
            label: "Local certification".to_string(),
            path: PathBuf::from("metrics.json"),
            summary_path: None,
            generated_at: "2026-07-09T06:00:00Z".to_string(),
        }),
        harness: BTreeMap::new(),
        recall_quality: Some(EvidenceRecordRef {
            category: "recall_quality".to_string(),
            status: EvidenceStatus::NeedsReview,
            label: "Recall quality".to_string(),
            path: PathBuf::from("recall-quality/default-fixture-v1.json"),
            summary_path: None,
            generated_at: "2026-07-09T06:00:00Z".to_string(),
        }),
        missing: Vec::new(),
        stale: Vec::new(),
    };
    assert_eq!(rollup_index_status(&index), EvidenceStatus::NeedsReview);
    index.recall_quality.as_mut().unwrap().status = EvidenceStatus::Fail;
    assert_eq!(rollup_index_status(&index), EvidenceStatus::Fail);
}
```

- [ ] **Step 2: Run the focused failing tests**

Run:

```bash
cargo test -p tree-ring-memory-cli evidence_snapshot_loads_recall_quality_record_from_index rollup_index_status_marks_recall_quality_needs_review --locked
```

Expected: FAIL because the recall-quality reader and `NeedsReview` status do not exist yet.

- [ ] **Step 3: Add shared status, reader, and index helper implementation**

Update `EvidenceStatus`:

```rust
pub enum EvidenceStatus {
    Pass,
    Fail,
    Skip,
    Missing,
    Stale,
    NeedsReview,
    Error,
}
```

Add to `EvidenceStatus::as_str()`:

```rust
Self::NeedsReview => "needs_review",
```

Add these structs near `CertificationEvidence`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RecallQualityEvidence {
    pub status: EvidenceStatus,
    pub generated_at: String,
    pub record_path: PathBuf,
    pub query_set_id: String,
    pub query_count: u64,
    pub pass_count: u64,
    pub fail_count: u64,
    pub needs_review_count: u64,
    pub avg_latency_ms: Option<f64>,
    pub max_latency_ms: Option<f64>,
    pub queries: Vec<RecallQualityQueryEvidence>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecallQualityQueryEvidence {
    pub query_id: String,
    pub query: String,
    pub status: String,
    pub expected_top_id: Option<String>,
    pub expected_rank: Option<u64>,
    pub latency_ms: Option<f64>,
    pub returned_ids: Vec<String>,
}
```

Add `pub recall_quality: Option<RecallQualityEvidence>` to `EvidenceSnapshot` and initialize it in every `EvidenceSnapshot` literal. When an index is present, load both certification and recall quality:

```rust
let certification = load_index_certification(evidence_dir, &index)?;
let recall_quality = load_index_recall_quality(evidence_dir, &index)?;
```

Add helpers:

```rust
pub(crate) fn read_or_create_index(
    evidence_dir: &Path,
    generated_at: &str,
) -> Result<EvidenceIndex, String> {
    let index_path = evidence_dir.join("evidence-index.json");
    if index_path.exists() {
        let input = fs::read_to_string(&index_path).map_err(|err| err.to_string())?;
        return serde_json::from_str(&input).map_err(|err| err.to_string());
    }
    Ok(EvidenceIndex {
        generated_at: generated_at.to_string(),
        overall_status: EvidenceStatus::Missing,
        certification: certification_record_from_metrics(evidence_dir, generated_at),
        harness: BTreeMap::new(),
        recall_quality: None,
        missing: vec!["harness".to_string(), "recall_quality".to_string()],
        stale: Vec::new(),
    })
}

pub(crate) fn write_index(evidence_dir: &Path, index: &EvidenceIndex) -> Result<PathBuf, String> {
    fs::create_dir_all(evidence_dir).map_err(|err| err.to_string())?;
    let index_path = evidence_dir.join("evidence-index.json");
    let json = serde_json::to_string_pretty(index).map_err(|err| err.to_string())?;
    fs::write(&index_path, json).map_err(|err| err.to_string())?;
    Ok(index_path)
}

pub(crate) fn rollup_index_status(index: &EvidenceIndex) -> EvidenceStatus {
    if index.harness.values().any(|record| record.status == EvidenceStatus::Fail)
        || index
            .recall_quality
            .as_ref()
            .is_some_and(|record| record.status == EvidenceStatus::Fail)
    {
        return EvidenceStatus::Fail;
    }
    if index.harness.values().any(|record| record.status == EvidenceStatus::Error)
        || index
            .recall_quality
            .as_ref()
            .is_some_and(|record| record.status == EvidenceStatus::Error)
    {
        return EvidenceStatus::Error;
    }
    if index
        .recall_quality
        .as_ref()
        .is_some_and(|record| record.status == EvidenceStatus::NeedsReview)
    {
        return EvidenceStatus::NeedsReview;
    }
    if let Some(certification) = &index.certification {
        return certification.status;
    }
    if index.harness.is_empty() && index.recall_quality.is_none() {
        EvidenceStatus::Missing
    } else {
        EvidenceStatus::Skip
    }
}
```

Move the private `certification_record_from_metrics` behavior from `harness_evidence.rs` into `evidence.rs` as a private helper used by `read_or_create_index`.

Add `load_index_recall_quality()` and `load_recall_quality()` using `serde_json::Value`, `get_u64`, `get_f64`, `get_string`, and `resolve_evidence_path()`. Parse only safe metadata: `query_set_id`, summary counts, latency, `query_id`, `query`, `status`, `expected_top_id`, `expected_rank`, and returned `id`s. Do not parse or display memory summaries/details.

- [ ] **Step 4: Update harness evidence to use shared helpers**

In `crates/tree-ring-memory-cli/src/harness_evidence.rs`, replace private `read_or_create_index`, `certification_record_from_metrics`, and `rollup_status` with imports from `crate::evidence`:

```rust
use crate::evidence::{
    read_or_create_index, rollup_index_status, write_index, EvidenceRecordRef, EvidenceStatus,
};
```

In `merge_harness_index()`, replace the final rollup/write block with:

```rust
index.overall_status = rollup_index_status(&index);
write_index(evidence_dir, &index)
```

Update existing harness rollup tests to call `rollup_index_status(&index)`.

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p tree-ring-memory-cli evidence::tests harness_evidence::tests --locked
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/tree-ring-memory-cli/src/evidence.rs crates/tree-ring-memory-cli/src/harness_evidence.rs
git commit -m "Add recall quality evidence reader"
```

---

### Task 2: Recall-Quality Diagnostic Runner And CLI

**Files:**
- Create: `crates/tree-ring-memory-cli/src/recall_quality.rs`
- Modify: `crates/tree-ring-memory-cli/src/main.rs`

**Interfaces:**
- Consumes: `read_or_create_index`, `write_index`, `rollup_index_status`, `EvidenceRecordRef`, and `EvidenceStatus` from Task 1.
- Produces: `pub struct RecallQualityRequest { pub source_root: PathBuf, pub evidence_dir: PathBuf }`.
- Produces: `pub struct RecallQualityReport`.
- Produces: `pub fn run_recall_quality(request: RecallQualityRequest) -> Result<RecallQualityReport, String>`.
- Produces CLI command: `tree-ring recall-quality --source-root <path> --out-dir <path>`.

- [ ] **Step 1: Write failing runner tests**

Create `crates/tree-ring-memory-cli/src/recall_quality.rs` with test scaffolding and tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn recall_quality_writes_safe_evidence_and_updates_index() {
        let dir = tempdir().unwrap();
        let evidence_dir = dir.path().join("target/tree-ring-certification");

        let report = run_recall_quality(RecallQualityRequest {
            source_root: dir.path().to_path_buf(),
            evidence_dir: evidence_dir.clone(),
        })
        .unwrap();

        assert_eq!(report.query_set_id, "default-fixture-v1");
        assert_eq!(report.status, EvidenceStatus::Pass);
        assert_eq!(report.summary.fail_count, 0);
        assert!(report.summary.pass_count >= 3);
        assert!(evidence_dir.join("recall-quality/default-fixture-v1.json").exists());
        let json = std::fs::read_to_string(evidence_dir.join("recall-quality/default-fixture-v1.json")).unwrap();
        assert!(json.contains("\"ranking\""));
        assert!(json.contains("\"latency_ms\""));
        assert!(!json.contains("Private bank account note"));

        let index: crate::evidence::EvidenceIndex =
            serde_json::from_str(&std::fs::read_to_string(evidence_dir.join("evidence-index.json")).unwrap()).unwrap();
        assert!(index.recall_quality.is_some());
        assert!(!index.missing.iter().any(|item| item == "recall_quality"));
    }

    #[test]
    fn query_evaluation_distinguishes_fail_and_needs_review() {
        let returned = vec![
            returned_memory_for_test("other", 1),
            returned_memory_for_test("expected", 2),
        ];
        let review = evaluate_query_for_test(
            Some("expected"),
            Some(3),
            &[],
            &returned,
        );
        assert_eq!(review.status, RecallQualityQueryStatus::NeedsReview);

        let failed = evaluate_query_for_test(
            Some("missing"),
            Some(1),
            &["other"],
            &returned,
        );
        assert_eq!(failed.status, RecallQualityQueryStatus::Fail);
        assert!(failed.notes.iter().any(|note| note.contains("missing")));
        assert!(failed.notes.iter().any(|note| note.contains("forbidden")));
    }
}
```

The test helper names can be `pub(crate)` or private test-only helpers. They must exercise the same evaluation function used by the production runner.

- [ ] **Step 2: Run failing runner tests**

Run:

```bash
cargo test -p tree-ring-memory-cli recall_quality --locked
```

Expected: FAIL because the module and CLI command are not wired yet.

- [ ] **Step 3: Implement the diagnostic types and deterministic fixture**

Add production structs with `Serialize`, `Deserialize`, and `PartialEq` where useful:

```rust
pub const RECALL_QUALITY_QUERY_SET_ID: &str = "default-fixture-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecallQualityRequest {
    pub source_root: PathBuf,
    pub evidence_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecallQualityReport {
    pub schema_version: u8,
    pub generated_at: String,
    pub query_set_id: String,
    pub status: EvidenceStatus,
    pub source_root: PathBuf,
    pub evidence_dir: PathBuf,
    pub record_path: PathBuf,
    pub summary: RecallQualitySummary,
    pub queries: Vec<RecallQualityQueryRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecallQualitySummary {
    pub query_count: usize,
    pub pass_count: usize,
    pub fail_count: usize,
    pub needs_review_count: usize,
    pub avg_latency_ms: f64,
    pub max_latency_ms: f64,
    pub fixture_memory_count: usize,
    pub private_payloads_used: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallQualityQueryStatus {
    Pass,
    Fail,
    NeedsReview,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecallQualityQueryRecord {
    pub query_id: String,
    pub query: String,
    pub status: RecallQualityQueryStatus,
    pub expected_top_id: Option<String>,
    pub expected_rank: Option<usize>,
    pub latency_ms: f64,
    pub returned: Vec<RecallQualityReturnedMemory>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecallQualityReturnedMemory {
    pub id: String,
    pub rank: usize,
    pub ring: String,
    pub source_ref: String,
    pub score: f64,
    pub ranking: BTreeMap<String, f64>,
}
```

Build fixture memories with stable ids, safe text, stable project `recall-quality-fixture`, stable source refs, and normal sensitivity. Include at least:

```rust
rq_scar_stale_cache
rq_heartwood_sqlite_decision
rq_seed_harness_experiment
rq_outer_release_guardrail
rq_sensitive_health_note
```

Set `rq_sensitive_health_note.sensitivity = "health"` so the sensitive-filter query can prove it is absent without writing or displaying its details.

- [ ] **Step 4: Implement the runner and index merge**

Implement `run_recall_quality()`:

```rust
pub fn run_recall_quality(request: RecallQualityRequest) -> Result<RecallQualityReport, String> {
    let generated_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let mut store = SQLiteMemoryStore::open(Path::new(":memory:")).map_err(|err| err.to_string())?;
    let fixtures = fixture_memories()?;
    store.put_many(&fixtures).map_err(|err| err.to_string())?;

    let mut queries = Vec::new();
    for case in query_cases() {
        queries.push(run_case(&store, case)?);
    }
    let summary = summarize(&queries, fixtures.len());
    let status = report_status(&summary);
    let record_path = request
        .evidence_dir
        .join("recall-quality")
        .join(format!("{RECALL_QUALITY_QUERY_SET_ID}.json"));
    let report = RecallQualityReport {
        schema_version: 1,
        generated_at: generated_at.clone(),
        query_set_id: RECALL_QUALITY_QUERY_SET_ID.to_string(),
        status,
        source_root: request.source_root,
        evidence_dir: request.evidence_dir.clone(),
        record_path: record_path.clone(),
        summary,
        queries,
    };
    write_report_and_index(&request.evidence_dir, &generated_at, &report)?;
    Ok(report)
}
```

Use `MemoryRetriever::new(&store).recall(..., include_sensitive=false, include_superseded=false, limit=5, explain_ranking=true)` for every case. Use `Instant::now()` for latency.

Use this default query set:

```rust
QueryCase {
    query_id: "scar-stale-cache",
    query: "failure stale cache",
    expected_top_id: Some("rq_scar_stale_cache"),
    max_expected_rank: Some(1),
    forbidden_ids: &[],
}
QueryCase {
    query_id: "heartwood-sqlite-decision",
    query: "durable local sqlite decision",
    expected_top_id: Some("rq_heartwood_sqlite_decision"),
    max_expected_rank: Some(1),
    forbidden_ids: &[],
}
QueryCase {
    query_id: "seed-harness-experiment",
    query: "planning experiment agent harness",
    expected_top_id: Some("rq_seed_harness_experiment"),
    max_expected_rank: Some(1),
    forbidden_ids: &[],
}
QueryCase {
    query_id: "sensitive-filter",
    query: "health private payload",
    expected_top_id: None,
    max_expected_rank: None,
    forbidden_ids: &["rq_sensitive_health_note"],
}
```

Write the report with pretty JSON. Merge the index:

```rust
let mut index = read_or_create_index(evidence_dir, generated_at)?;
index.generated_at = generated_at.to_string();
index.recall_quality = Some(EvidenceRecordRef {
    category: "recall_quality".to_string(),
    status: report.status,
    label: "Recall quality".to_string(),
    path: PathBuf::from(format!("recall-quality/{RECALL_QUALITY_QUERY_SET_ID}.json")),
    summary_path: None,
    generated_at: generated_at.to_string(),
});
index.missing.retain(|item| item != "recall_quality");
if index.harness.is_empty() && !index.missing.iter().any(|item| item == "harness") {
    index.missing.push("harness".to_string());
}
index.missing.sort();
index.missing.dedup();
index.overall_status = rollup_index_status(&index);
write_index(evidence_dir, &index)?;
```

- [ ] **Step 5: Wire the CLI command**

In `main.rs`, add:

```rust
use recall_quality::{run_recall_quality, RecallQualityReport, RecallQualityRequest};
mod recall_quality;
```

Add a command variant:

```rust
#[command(about = "write non-private recall quality evidence")]
RecallQuality {
    #[arg(long, default_value = ".", help = "project root used for default evidence output")]
    source_root: PathBuf,
    #[arg(
        long,
        help = "evidence output directory; defaults to <source-root>/target/tree-ring-certification"
    )]
    out_dir: Option<PathBuf>,
},
```

Handle it before opening the writable memory store:

```rust
if let Command::RecallQuality {
    source_root,
    out_dir,
} = &cli.command
{
    let evidence_dir = out_dir
        .clone()
        .unwrap_or_else(|| evidence::certification_dir_for_project(source_root));
    let report = run_recall_quality(RecallQualityRequest {
        source_root: source_root.clone(),
        evidence_dir,
    })?;
    print_recall_quality_report(&report, cli.json)?;
    return Ok(());
}
```

Add a printer:

```rust
fn print_recall_quality_report(
    report: &RecallQualityReport,
    json_output: bool,
) -> Result<(), String> {
    if json_output {
        println!("{}", json!({"ok": true, "report": report}));
    } else {
        println!(
            "Tree Ring Memory recall quality: status={} queries={} pass={} fail={} needs_review={} avg={:.3}ms max={:.3}ms evidence={}",
            report.status.as_str(),
            report.summary.query_count,
            report.summary.pass_count,
            report.summary.fail_count,
            report.summary.needs_review_count,
            report.summary.avg_latency_ms,
            report.summary.max_latency_ms,
            report.evidence_dir.display()
        );
        for query in &report.queries {
            println!(
                "{} [{:?}] latency={:.3}ms returned={}",
                query.query_id,
                query.status,
                query.latency_ms,
                query.returned.iter().map(|item| item.id.as_str()).collect::<Vec<_>>().join(",")
            );
        }
    }
    Ok(())
}
```

Add command tests in `main.rs` for JSON and human output, mirroring the existing harness certification tests.

- [ ] **Step 6: Run focused tests and manual command**

Run:

```bash
cargo test -p tree-ring-memory-cli recall_quality --locked
cargo test -p tree-ring-memory-cli recall_quality_json_output_contract recall_quality_human_output_contract --locked
cargo run -p tree-ring-memory-cli -- --json recall-quality --source-root . --out-dir target/tree-ring-certification
```

Expected: PASS and command JSON contains `"query_set_id":"default-fixture-v1"` plus `"status":"pass"`.

- [ ] **Step 7: Commit**

```bash
git add crates/tree-ring-memory-cli/src/recall_quality.rs crates/tree-ring-memory-cli/src/main.rs
git commit -m "Add recall quality diagnostic runner"
```

---

### Task 3: Certification Script And Documentation

**Files:**
- Modify: `scripts/certify-tree-ring.sh`
- Modify: `README.md`

**Interfaces:**
- Consumes: `tree-ring recall-quality --source-root <path> --out-dir <path>`.
- Produces: certification artifacts `recall-quality.json` and `recall-quality/default-fixture-v1.json`.

- [ ] **Step 1: Add script assertions**

In `scripts/certify-tree-ring.sh`, after the harness certification assertions, add:

```sh
"$BIN" --json recall-quality --source-root "$scan_root" --out-dir "$OUT_DIR" \
  > "$OUT_DIR/recall-quality.json"
require_file "$OUT_DIR/recall-quality/default-fixture-v1.json"
grep -E '"status"[[:space:]]*:[[:space:]]*"pass"' "$OUT_DIR/recall-quality.json" > /dev/null \
  || fail "recall quality command did not report pass status"
grep -E '"fail_count"[[:space:]]*:[[:space:]]*0' "$OUT_DIR/recall-quality.json" > /dev/null \
  || fail "recall quality command reported failing queries"
grep -F '"recall_quality": {' "$INDEX" > /dev/null \
  || fail "evidence index did not include recall quality record"
grep -F '"missing": []' "$INDEX" > /dev/null \
  || fail "evidence index still reports missing evidence categories"
```

Update the final output:

```sh
printf 'Recall quality evidence: %s\n' "$OUT_DIR/recall-quality/default-fixture-v1.json"
```

- [ ] **Step 2: Update README command docs**

In `README.md`, near the `integrations certify` text, add:

```markdown
- `recall-quality` writes non-private recall diagnostics under
  `target/tree-ring-certification/recall-quality/default-fixture-v1.json`
  and merges the result into `evidence-index.json`. It uses deterministic
  safe fixture memories, records returned ids, rank positions, score factors,
  and latency, and marks each query as `pass`, `fail`, or `needs_review`.

```bash
tree-ring recall-quality --source-root .
```
```

In the certification script section, add that `scripts/certify-tree-ring.sh` now runs the recall-quality diagnostics in addition to install-size, recall-speed, CLI, adapter, and harness checks.

- [ ] **Step 3: Run focused checks**

Run:

```bash
cargo run -p tree-ring-memory-cli -- --json recall-quality --source-root . --out-dir target/tree-ring-certification
sh scripts/certify-tree-ring.sh
```

Expected: both commands pass. Certification output includes `Recall quality evidence:`.

- [ ] **Step 4: Commit**

```bash
git add scripts/certify-tree-ring.sh README.md
git commit -m "Certify recall quality evidence"
```

---

### Task 4: TUI Recall-Quality Dashboard Rendering

**Files:**
- Modify: `crates/tree-ring-memory-cli/src/tui/render.rs`

**Interfaces:**
- Consumes: `EvidenceSnapshot.recall_quality` from Task 1.
- Produces: `/evidence` list and detail rendering for recall-quality status, query counts, latency, and returned ids.

- [ ] **Step 1: Add failing render test**

Add a test near the existing evidence render tests:

```rust
#[test]
fn render_evidence_mode_shows_recall_quality_records() {
    let dir = tempdir().unwrap();
    let evidence_dir = dir.path().join("target/tree-ring-certification");
    std::fs::create_dir_all(evidence_dir.join("recall-quality")).unwrap();
    std::fs::write(evidence_dir.join("metrics.json"), r#"{"ok":true,"created_at":"2026-07-09T05:44:48Z"}"#).unwrap();
    std::fs::write(
        evidence_dir.join("recall-quality/default-fixture-v1.json"),
        r#"{
          "schema_version": 1,
          "generated_at": "2026-07-09T06:00:00Z",
          "query_set_id": "default-fixture-v1",
          "status": "pass",
          "summary": {
            "query_count": 4,
            "pass_count": 4,
            "fail_count": 0,
            "needs_review_count": 0,
            "avg_latency_ms": 0.5,
            "max_latency_ms": 1.0
          },
          "queries": [
            {
              "query_id": "scar-stale-cache",
              "query": "failure stale cache",
              "status": "pass",
              "expected_top_id": "rq_scar_stale_cache",
              "expected_rank": 1,
              "latency_ms": 0.25,
              "returned": [
                {"id":"rq_scar_stale_cache","rank":1,"ring":"scar","source_ref":"recall-quality/scar-stale-cache","score":1.2,"ranking":{"textual_match":1.0}}
              ],
              "notes": []
            }
          ]
        }"#,
    )
    .unwrap();
    std::fs::write(
        evidence_dir.join("evidence-index.json"),
        r#"{
          "generated_at": "2026-07-09T06:00:00Z",
          "overall_status": "pass",
          "certification": {
            "category": "certification",
            "status": "pass",
            "label": "Local certification",
            "path": "metrics.json",
            "summary_path": null,
            "generated_at": "2026-07-09T05:44:48Z"
          },
          "harness": {},
          "recall_quality": {
            "category": "recall_quality",
            "status": "pass",
            "label": "Recall quality",
            "path": "recall-quality/default-fixture-v1.json",
            "summary_path": null,
            "generated_at": "2026-07-09T06:00:00Z"
          },
          "missing": [],
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

    assert!(output.contains("Recall quality"));
    assert!(output.contains("default-fixture-v1"));
    assert!(output.contains("queries 4"));
    assert!(output.contains("avg 0.500 ms"));
    assert!(output.contains("scar-stale-cache"));
    assert!(output.contains("rq_scar_stale_cache"));
}
```

- [ ] **Step 2: Run failing render test**

Run:

```bash
cargo test -p tree-ring-memory-cli render_evidence_mode_shows_recall_quality_records --locked
```

Expected: FAIL because the TUI still only shows recall quality as loaded/missing.

- [ ] **Step 3: Render recall-quality status in the evidence list**

In `render_evidence_list()`, when `index.recall_quality` is present, show the actual status instead of only `loaded`:

```rust
let recall = if let Some(record) = &index.recall_quality {
    format!(" {}", record.status.as_str())
} else if index.missing.iter().any(|item| item == "recall_quality") {
    " missing".to_string()
} else {
    " none".to_string()
};
```

Update the list row to use owned `String` status text.

- [ ] **Step 4: Render recall-quality details**

In `render_evidence_detail()`, after the harness matrix block, add:

```rust
if let Some(recall_quality) = &snapshot.recall_quality {
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Recall quality ", theme::brand()),
        Span::styled(recall_quality.status.as_str(), theme::dim()),
    ]));
    lines.push(Line::from(format!(
        "{} queries {} pass {} fail {} review {}",
        recall_quality.query_set_id,
        recall_quality.query_count,
        recall_quality.pass_count,
        recall_quality.fail_count,
        recall_quality.needs_review_count
    )));
    if let Some(avg) = recall_quality.avg_latency_ms {
        let max = recall_quality.max_latency_ms.unwrap_or(avg);
        lines.push(Line::from(format!("avg {avg:.3} ms max {max:.3} ms")));
    }
    lines.push(Line::from(vec![
        Span::styled("record ", theme::dim()),
        Span::raw(truncate(&recall_quality.record_path.display().to_string(), 52)),
    ]));
    for query in recall_quality.queries.iter().take(4) {
        let returned = if query.returned_ids.is_empty() {
            "-".to_string()
        } else {
            query.returned_ids.join(",")
        };
        lines.push(Line::from(format!(
            "{} [{}] rank {:?} {:.3}ms",
            truncate(&query.query_id, 28),
            query.status,
            query.expected_rank,
            query.latency_ms.unwrap_or(0.0)
        )));
        lines.push(Line::from(Span::styled(
            truncate(&format!("returned {returned}"), 70),
            theme::dim(),
        )));
    }
}
```

Do not render memory summaries or details from the diagnostic payload.

- [ ] **Step 5: Run focused TUI tests**

Run:

```bash
cargo test -p tree-ring-memory-cli render_evidence_mode --locked
```

Expected: PASS with the new recall-quality render test and existing certification/harness evidence render tests passing.

- [ ] **Step 6: Commit**

```bash
git add crates/tree-ring-memory-cli/src/tui/render.rs
git commit -m "Render recall quality evidence in TUI"
```

---

### Task 5: Final Verification And PR Handoff

**Files:**
- Modify only if verification reveals a real defect in files touched by Tasks 1-4.

**Interfaces:**
- Consumes completed recall-quality runner, certification script, docs, and TUI rendering.
- Produces PR-ready branch with verification evidence.

- [ ] **Step 1: Run formatting**

Run:

```bash
cargo fmt --check
```

Expected: PASS.

- [ ] **Step 2: Run full tests**

Run:

```bash
cargo test --locked
```

Expected: PASS.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy --locked --all-targets
```

Expected: PASS with no new warnings.

- [ ] **Step 4: Run whitespace diff check**

Run:

```bash
git diff --check
```

Expected: no output.

- [ ] **Step 5: Run certification**

Run:

```bash
sh scripts/certify-tree-ring.sh
```

Expected: PASS. Output includes `Evidence index:`, `Harness evidence:`, and `Recall quality evidence:`.

- [ ] **Step 6: Inspect evidence artifacts**

Run:

```bash
test -f target/tree-ring-certification/recall-quality/default-fixture-v1.json
grep -F '"recall_quality": {' target/tree-ring-certification/evidence-index.json
grep -F '"missing": []' target/tree-ring-certification/evidence-index.json
```

Expected: all commands pass.

- [ ] **Step 7: Commit verification fixes if any**

If Tasks 1-4 did not need follow-up fixes, skip this step. Otherwise:

```bash
git add <changed-files>
git commit -m "Stabilize recall quality certification"
```

- [ ] **Step 8: Push and open PR to main**

Run:

```bash
git push -u origin codex/recall-quality-dashboard
gh pr create --base main --head codex/recall-quality-dashboard --title "Add recall quality evidence dashboard" --body-file <generated-pr-body>
```

Expected: PR created against `main`.

---

## Self-Review

- Spec coverage: Task 2 creates curated recall query sets, deterministic fixture memory, returned ids, rank positions, score breakdowns, latency, and pass/fail/needs_review query status. Task 1 and Task 2 merge recall-quality into `/evidence` artifacts. Task 4 surfaces ranking evidence in the TUI without payload leakage. Task 3 wires certification. Task 5 verifies.
- Privacy check: The default runner uses safe fixture memory only; TUI parsing renders ids, query ids, statuses, rank, and latency only.
- Scope check: No real memory store, background daemon, hidden TUI refresh execution, vector search changes, or ranking semantic changes are included.
- Type consistency: `EvidenceStatus::NeedsReview`, `RecallQualityReport`, `RecallQualitySummary`, `RecallQualityQueryRecord`, `RecallQualityReturnedMemory`, and `EvidenceSnapshot.recall_quality` are introduced before use.
- Placeholder scan: No task uses placeholder tokens, "similar to", or unspecified validation language.
