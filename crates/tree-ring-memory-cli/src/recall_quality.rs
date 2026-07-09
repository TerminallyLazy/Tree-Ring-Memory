use crate::evidence::{
    read_or_create_index, rollup_index_status, write_index, EvidenceRecordRef, EvidenceStatus,
};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tree_ring_memory_core::{MemoryEvent, MemorySource};
use tree_ring_memory_sqlite::{MemoryRetriever, SQLiteMemoryStore};

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueryCase {
    query_id: &'static str,
    query: &'static str,
    expected_top_id: Option<&'static str>,
    max_expected_rank: Option<usize>,
    forbidden_ids: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueryEvaluation {
    status: RecallQualityQueryStatus,
    notes: Vec<String>,
}

pub fn run_recall_quality(request: RecallQualityRequest) -> Result<RecallQualityReport, String> {
    let generated_at = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let mut store =
        SQLiteMemoryStore::open(Path::new(":memory:")).map_err(|err| err.to_string())?;
    let fixtures = fixture_memories()?;
    store.put_many(&fixtures).map_err(|err| err.to_string())?;

    let retriever = MemoryRetriever::new(&store);
    let mut queries = Vec::new();
    for case in query_cases() {
        queries.push(run_case(&retriever, &case)?);
    }
    let summary = summarize(&queries, &fixtures);
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

fn run_case(
    retriever: &MemoryRetriever<'_>,
    case: &QueryCase,
) -> Result<RecallQualityQueryRecord, String> {
    let started_at = Instant::now();
    let returned = retriever
        .recall(
            case.query, None, None, None, None, None, false, false, 5, true,
        )
        .map_err(|err| err.to_string())?;
    let latency_ms = started_at.elapsed().as_secs_f64() * 1000.0;
    let returned: Vec<_> = returned
        .into_iter()
        .enumerate()
        .map(|(index, item)| RecallQualityReturnedMemory {
            id: item.memory.id,
            rank: index + 1,
            ring: item.memory.ring,
            source_ref: item.memory.source.ref_,
            score: item.score,
            ranking: item.ranking,
        })
        .collect();
    let evaluation = evaluate_query(
        case.expected_top_id,
        case.max_expected_rank,
        case.forbidden_ids,
        &returned,
    );

    Ok(RecallQualityQueryRecord {
        query_id: case.query_id.to_string(),
        query: case.query.to_string(),
        status: evaluation.status,
        expected_top_id: case.expected_top_id.map(str::to_string),
        expected_rank: case.max_expected_rank,
        latency_ms,
        returned,
        notes: evaluation.notes,
    })
}

fn evaluate_query(
    expected_top_id: Option<&str>,
    max_expected_rank: Option<usize>,
    forbidden_ids: &[&str],
    returned: &[RecallQualityReturnedMemory],
) -> QueryEvaluation {
    let mut status = RecallQualityQueryStatus::Pass;
    let mut notes = Vec::new();

    if let Some(expected_top_id) = expected_top_id {
        match returned.iter().find(|item| item.id == expected_top_id) {
            Some(item) => {
                if max_expected_rank.is_some_and(|expected_rank| item.rank > expected_rank) {
                    status = RecallQualityQueryStatus::NeedsReview;
                    notes.push(format!(
                        "expected memory {expected_top_id} returned at rank {} which is worse than allowed rank {}",
                        item.rank,
                        max_expected_rank.unwrap_or(item.rank)
                    ));
                }
            }
            None => {
                status = RecallQualityQueryStatus::Fail;
                notes.push(format!("missing expected memory {expected_top_id}"));
            }
        }
    }

    let forbidden_hits: Vec<_> = returned
        .iter()
        .filter(|item| forbidden_ids.contains(&item.id.as_str()))
        .map(|item| item.id.clone())
        .collect();
    if !forbidden_hits.is_empty() {
        status = RecallQualityQueryStatus::Fail;
        notes.push(format!(
            "forbidden memories returned: {}",
            forbidden_hits.join(", ")
        ));
    }

    QueryEvaluation { status, notes }
}

fn summarize(
    queries: &[RecallQualityQueryRecord],
    fixtures: &[MemoryEvent],
) -> RecallQualitySummary {
    let pass_count = queries
        .iter()
        .filter(|query| query.status == RecallQualityQueryStatus::Pass)
        .count();
    let fail_count = queries
        .iter()
        .filter(|query| query.status == RecallQualityQueryStatus::Fail)
        .count();
    let needs_review_count = queries
        .iter()
        .filter(|query| query.status == RecallQualityQueryStatus::NeedsReview)
        .count();
    let total_latency_ms: f64 = queries.iter().map(|query| query.latency_ms).sum();
    let avg_latency_ms = if queries.is_empty() {
        0.0
    } else {
        total_latency_ms / queries.len() as f64
    };
    let max_latency_ms = queries
        .iter()
        .map(|query| query.latency_ms)
        .fold(0.0, f64::max);

    RecallQualitySummary {
        query_count: queries.len(),
        pass_count,
        fail_count,
        needs_review_count,
        avg_latency_ms,
        max_latency_ms,
        fixture_memory_count: fixtures.len(),
        private_payloads_used: fixtures
            .iter()
            .any(|fixture| fixture.sensitivity != "normal"),
    }
}

fn report_status(summary: &RecallQualitySummary) -> EvidenceStatus {
    if summary.fail_count > 0 {
        EvidenceStatus::Fail
    } else if summary.needs_review_count > 0 {
        EvidenceStatus::NeedsReview
    } else {
        EvidenceStatus::Pass
    }
}

fn write_report_and_index(
    evidence_dir: &Path,
    generated_at: &str,
    report: &RecallQualityReport,
) -> Result<(), String> {
    let report_dir = evidence_dir.join("recall-quality");
    fs::create_dir_all(&report_dir).map_err(|err| err.to_string())?;
    let report_json = serde_json::to_string_pretty(report).map_err(|err| err.to_string())?;
    fs::write(&report.record_path, report_json).map_err(|err| err.to_string())?;

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
    if index.harness.is_empty() {
        if !index.missing.iter().any(|item| item == "harness") {
            index.missing.push("harness".to_string());
        }
    } else {
        index.missing.retain(|item| item != "harness");
    }
    index.missing.sort();
    index.missing.dedup();
    index.overall_status = rollup_index_status(&index);
    write_index(evidence_dir, &index)?;
    Ok(())
}

fn query_cases() -> Vec<QueryCase> {
    vec![
        QueryCase {
            query_id: "scar-stale-cache",
            query: "failure stale cache",
            expected_top_id: Some("rq_scar_stale_cache"),
            max_expected_rank: Some(1),
            forbidden_ids: &[],
        },
        QueryCase {
            query_id: "heartwood-sqlite-decision",
            query: "durable local sqlite decision",
            expected_top_id: Some("rq_heartwood_sqlite_decision"),
            max_expected_rank: Some(1),
            forbidden_ids: &[],
        },
        QueryCase {
            query_id: "seed-harness-experiment",
            query: "planning experiment agent harness",
            expected_top_id: Some("rq_seed_harness_experiment"),
            max_expected_rank: Some(1),
            forbidden_ids: &[],
        },
        QueryCase {
            query_id: "sensitive-filter",
            query: "health private payload",
            expected_top_id: None,
            max_expected_rank: None,
            forbidden_ids: &["rq_sensitive_health_note"],
        },
    ]
}

fn fixture_memories() -> Result<Vec<MemoryEvent>, String> {
    Ok(vec![
        fixture_memory(
            "rq_scar_stale_cache",
            "scar",
            "warning",
            "Failure stale cache regression after deploy.",
            "The cache stayed stale until invalidation was added to the failure path.",
            "fixture://recall-quality/scar-stale-cache",
            "normal",
        )?,
        fixture_memory(
            "rq_heartwood_sqlite_decision",
            "heartwood",
            "decision",
            "Durable local sqlite decision for recall storage.",
            "Choose local sqlite as the durable storage decision for offline recall.",
            "fixture://recall-quality/heartwood-sqlite-decision",
            "normal",
        )?,
        fixture_memory(
            "rq_seed_harness_experiment",
            "seed",
            "hypothesis",
            "Planning experiment for agent harness diagnostics.",
            "Run a planning experiment around the agent harness diagnostic runner.",
            "fixture://recall-quality/seed-harness-experiment",
            "normal",
        )?,
        fixture_memory(
            "rq_outer_release_guardrail",
            "outer",
            "lesson",
            "Release guardrail for evidence output before shipping.",
            "Keep release guardrail checks separate from recall-quality diagnostics.",
            "fixture://recall-quality/outer-release-guardrail",
            "normal",
        )?,
        fixture_memory(
            "rq_sensitive_health_note",
            "cambium",
            "lesson",
            "Health private payload stays filtered from safe recall evidence.",
            "This fixture proves sensitive health payload filtering without exposing details.",
            "fixture://recall-quality/sensitive-health-note",
            "health",
        )?,
    ])
}

fn fixture_memory(
    id: &str,
    ring: &str,
    event_type: &str,
    summary: &str,
    details: &str,
    source_ref: &str,
    sensitivity: &str,
) -> Result<MemoryEvent, String> {
    let mut event = MemoryEvent::new(summary, event_type).map_err(|err| err.to_string())?;
    event.id = id.to_string();
    event.project = Some("recall-quality-fixture".to_string());
    event.scope = "project".to_string();
    event.ring = ring.to_string();
    event.details = details.to_string();
    event.source = MemorySource {
        source_type: "fixture".to_string(),
        ref_: source_ref.to_string(),
        quote: String::new(),
    };
    event.tags = vec![
        "recall-quality".to_string(),
        RECALL_QUALITY_QUERY_SET_ID.to_string(),
    ];
    event.salience = 0.92;
    event.confidence = 0.91;
    event.retention = "durable".to_string();
    event.sensitivity = sensitivity.to_string();
    event.validated().map_err(|err| err.to_string())
}

#[cfg(test)]
fn returned_memory_for_test(id: &str, rank: usize) -> RecallQualityReturnedMemory {
    RecallQualityReturnedMemory {
        id: id.to_string(),
        rank,
        ring: "cambium".to_string(),
        source_ref: format!("fixture://{id}"),
        score: 0.5,
        ranking: BTreeMap::from([("fts".to_string(), 0.5)]),
    }
}

#[cfg(test)]
fn evaluate_query_for_test(
    expected_top_id: Option<&str>,
    max_expected_rank: Option<usize>,
    forbidden_ids: &[&str],
    returned: &[RecallQualityReturnedMemory],
) -> QueryEvaluation {
    evaluate_query(expected_top_id, max_expected_rank, forbidden_ids, returned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{EvidenceIndex, EvidenceStatus};
    use std::path::PathBuf;
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
        assert!(evidence_dir
            .join("recall-quality/default-fixture-v1.json")
            .exists());
        let json =
            std::fs::read_to_string(evidence_dir.join("recall-quality/default-fixture-v1.json"))
                .unwrap();
        assert!(json.contains("\"ranking\""));
        assert!(json.contains("\"latency_ms\""));
        assert!(!json.contains("Private bank account note"));

        let index: crate::evidence::EvidenceIndex = serde_json::from_str(
            &std::fs::read_to_string(evidence_dir.join("evidence-index.json")).unwrap(),
        )
        .unwrap();
        assert!(index.recall_quality.is_some());
        assert!(!index.missing.iter().any(|item| item == "recall_quality"));
    }

    #[test]
    fn query_evaluation_distinguishes_fail_and_needs_review() {
        let passing_returned = vec![
            returned_memory_for_test("other", 1),
            returned_memory_for_test("expected", 2),
        ];
        let passing = evaluate_query_for_test(Some("expected"), Some(3), &[], &passing_returned);
        assert_eq!(passing.status, RecallQualityQueryStatus::Pass);

        let review_returned = vec![
            returned_memory_for_test("other", 1),
            returned_memory_for_test("expected", 4),
        ];
        let review = evaluate_query_for_test(Some("expected"), Some(3), &[], &review_returned);
        assert_eq!(review.status, RecallQualityQueryStatus::NeedsReview);

        let failed =
            evaluate_query_for_test(Some("missing"), Some(1), &["other"], &passing_returned);
        assert_eq!(failed.status, RecallQualityQueryStatus::Fail);
        assert!(failed.notes.iter().any(|note| note.contains("missing")));
        assert!(failed.notes.iter().any(|note| note.contains("forbidden")));
    }

    #[test]
    fn recall_quality_index_merge_preserves_existing_certification_and_harness_entries() {
        let dir = tempdir().unwrap();
        let evidence_dir = dir.path().join("target/tree-ring-certification");
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
              "harness": {
                "codex": {
                  "category": "harness",
                  "status": "pass",
                  "label": "Codex",
                  "path": "harness/codex.json",
                  "summary_path": null,
                  "generated_at": "2026-07-09T05:44:48Z"
                },
                "manual-harness": {
                  "category": "harness",
                  "status": "skip",
                  "label": "Manual Harness",
                  "path": "harness/manual.json",
                  "summary_path": null,
                  "generated_at": "2026-07-09T05:44:48Z"
                }
              },
              "recall_quality": null,
              "missing": ["harness", "recall_quality"],
              "stale": []
            }"#,
        )
        .unwrap();

        let report = RecallQualityReport {
            schema_version: 1,
            generated_at: "2026-07-09T06:00:00Z".to_string(),
            query_set_id: RECALL_QUALITY_QUERY_SET_ID.to_string(),
            status: EvidenceStatus::Pass,
            source_root: dir.path().to_path_buf(),
            evidence_dir: evidence_dir.clone(),
            record_path: evidence_dir.join(format!(
                "recall-quality/{RECALL_QUALITY_QUERY_SET_ID}.json"
            )),
            summary: RecallQualitySummary {
                query_count: 4,
                pass_count: 4,
                fail_count: 0,
                needs_review_count: 0,
                avg_latency_ms: 1.0,
                max_latency_ms: 2.0,
                fixture_memory_count: 5,
                private_payloads_used: true,
            },
            queries: Vec::new(),
        };

        write_report_and_index(&evidence_dir, &report.generated_at, &report).unwrap();

        let index: EvidenceIndex = serde_json::from_str(
            &std::fs::read_to_string(evidence_dir.join("evidence-index.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            index.certification.as_ref().map(|record| record.path.clone()),
            Some(PathBuf::from("metrics.json"))
        );
        assert_eq!(
            index.harness.get("codex").map(|record| record.path.clone()),
            Some(PathBuf::from("harness/codex.json"))
        );
        assert_eq!(
            index
                .harness
                .get("manual-harness")
                .map(|record| record.path.clone()),
            Some(PathBuf::from("harness/manual.json"))
        );
        assert_eq!(
            index.recall_quality.as_ref().map(|record| record.path.clone()),
            Some(PathBuf::from(format!(
                "recall-quality/{RECALL_QUALITY_QUERY_SET_ID}.json"
            )))
        );
        assert_eq!(index.missing, Vec::<String>::new());
        assert_eq!(index.overall_status, EvidenceStatus::Pass);
    }
}
