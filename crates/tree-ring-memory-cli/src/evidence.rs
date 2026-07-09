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
    NeedsReview,
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
            Self::NeedsReview => "needs_review",
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

#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceSnapshot {
    pub root: PathBuf,
    pub index_path: PathBuf,
    pub index: Option<EvidenceIndex>,
    pub certification: Option<CertificationEvidence>,
    pub recall_quality: Option<RecallQualityEvidence>,
    pub status: EvidenceStatus,
    pub message: String,
}

pub fn certification_dir_for_project(project_root: &Path) -> PathBuf {
    project_root.join("target").join("tree-ring-certification")
}

pub fn load_snapshot(evidence_dir: &Path) -> EvidenceSnapshot {
    let index_path = evidence_dir.join("evidence-index.json");
    match load_index(&index_path) {
        Ok(index) => match (
            load_index_certification(evidence_dir, &index),
            load_index_recall_quality(evidence_dir, &index),
        ) {
            (Ok(certification), Ok(recall_quality)) => {
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
                    recall_quality,
                    message,
                }
            }
            (Err(error), _) | (_, Err(error)) => EvidenceSnapshot {
                root: evidence_dir.to_path_buf(),
                index_path,
                index: Some(index),
                certification: None,
                recall_quality: None,
                status: EvidenceStatus::Error,
                message: error,
            },
        },
        Err(_) if !index_path.exists() => match load_metrics_only_certification(evidence_dir) {
            Ok(certification) => EvidenceSnapshot {
                root: evidence_dir.to_path_buf(),
                index_path,
                index: None,
                status: certification.status,
                message: format!(
                    "certification {} at {}",
                    certification.status.as_str(),
                    certification.generated_at
                ),
                certification: Some(certification),
                recall_quality: None,
            },
            Err(_) if !metrics_path_for_dir(evidence_dir).exists() => EvidenceSnapshot {
                root: evidence_dir.to_path_buf(),
                index_path,
                index: None,
                certification: None,
                recall_quality: None,
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
                recall_quality: None,
                status: EvidenceStatus::Error,
                message: error,
            },
        },
        Err(error) => EvidenceSnapshot {
            root: evidence_dir.to_path_buf(),
            index_path,
            index: None,
            certification: None,
            recall_quality: None,
            status: EvidenceStatus::Error,
            message: error,
        },
    }
}

fn load_index(index_path: &Path) -> Result<EvidenceIndex, String> {
    let input = fs::read_to_string(index_path).map_err(|err| err.to_string())?;
    serde_json::from_str(&input).map_err(|err| err.to_string())
}

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
    if index
        .harness
        .values()
        .any(|record| record.status == EvidenceStatus::Fail)
        || index
            .recall_quality
            .as_ref()
            .is_some_and(|record| record.status == EvidenceStatus::Fail)
    {
        return EvidenceStatus::Fail;
    }
    if index
        .harness
        .values()
        .any(|record| record.status == EvidenceStatus::Error)
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

fn load_index_certification(
    evidence_dir: &Path,
    index: &EvidenceIndex,
) -> Result<Option<CertificationEvidence>, String> {
    match index.certification.as_ref() {
        Some(record) => {
            let metrics_path = resolve_evidence_path(evidence_dir, &record.path);
            load_certification(evidence_dir, record)
                .map(Some)
                .map_err(|error| {
                    format!(
                        "failed to load certification payload {}: {}",
                        metrics_path.display(),
                        error
                    )
                })
        }
        None => Ok(None),
    }
}

fn load_index_recall_quality(
    evidence_dir: &Path,
    index: &EvidenceIndex,
) -> Result<Option<RecallQualityEvidence>, String> {
    match index.recall_quality.as_ref() {
        Some(record) => {
            let record_path = resolve_evidence_path(evidence_dir, &record.path);
            load_recall_quality(evidence_dir, record)
                .map(Some)
                .map_err(|error| {
                    format!(
                        "failed to load recall quality payload {}: {}",
                        record_path.display(),
                        error
                    )
                })
        }
        None => Ok(None),
    }
}

fn load_metrics_only_certification(evidence_dir: &Path) -> Result<CertificationEvidence, String> {
    let metrics_path = metrics_path_for_dir(evidence_dir);
    let input = fs::read_to_string(&metrics_path).map_err(|err| err.to_string())?;
    let value: Value = serde_json::from_str(&input).map_err(|err| err.to_string())?;
    Ok(CertificationEvidence {
        status: EvidenceStatus::Pass,
        generated_at: value
            .get("created_at")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        metrics_path,
        summary_path: summary_path_for_dir(evidence_dir),
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

fn load_recall_quality(
    evidence_dir: &Path,
    record: &EvidenceRecordRef,
) -> Result<RecallQualityEvidence, String> {
    let record_path = resolve_evidence_path(evidence_dir, &record.path);
    let input = fs::read_to_string(&record_path).map_err(|err| err.to_string())?;
    let value: Value = serde_json::from_str(&input).map_err(|err| err.to_string())?;
    let queries = value
        .get("queries")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|query| RecallQualityQueryEvidence {
                    query_id: get_string(query, &["query_id"]).unwrap_or_default(),
                    query: get_string(query, &["query"]).unwrap_or_default(),
                    status: get_string(query, &["status"]).unwrap_or_default(),
                    expected_top_id: get_string(query, &["expected_top_id"]),
                    expected_rank: get_u64(query, &["expected_rank"]),
                    latency_ms: get_f64(query, &["latency_ms"]),
                    returned_ids: query
                        .get("returned")
                        .and_then(Value::as_array)
                        .map(|returned| {
                            returned
                                .iter()
                                .filter_map(|item| get_string(item, &["id"]))
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(RecallQualityEvidence {
        status: record.status,
        generated_at: get_string(&value, &["generated_at"])
            .unwrap_or_else(|| record.generated_at.clone()),
        record_path,
        query_set_id: get_string(&value, &["query_set_id"]).unwrap_or_default(),
        query_count: get_u64(&value, &["summary", "query_count"]).unwrap_or(0),
        pass_count: get_u64(&value, &["summary", "pass_count"]).unwrap_or(0),
        fail_count: get_u64(&value, &["summary", "fail_count"]).unwrap_or(0),
        needs_review_count: get_u64(&value, &["summary", "needs_review_count"]).unwrap_or(0),
        avg_latency_ms: get_f64(&value, &["summary", "avg_latency_ms"]),
        max_latency_ms: get_f64(&value, &["summary", "max_latency_ms"]),
        queries,
    })
}

fn metrics_path_for_dir(evidence_dir: &Path) -> PathBuf {
    evidence_dir.join("metrics.json")
}

fn summary_path_for_dir(evidence_dir: &Path) -> Option<PathBuf> {
    let summary_path = evidence_dir.join("summary.md");
    summary_path.exists().then_some(summary_path)
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
    path.iter()
        .try_fold(value, |current, key| current.get(*key))
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

    #[test]
    fn evidence_snapshot_loads_metrics_only_certification_without_index() {
        let dir = tempdir().unwrap();
        let evidence_dir = certification_dir_for_project(dir.path());
        fs::create_dir_all(&evidence_dir).unwrap();
        fs::write(evidence_dir.join("summary.md"), "# Summary\n").unwrap();
        fs::write(
            evidence_dir.join("metrics.json"),
            r#"{
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

        let snapshot = load_snapshot(&evidence_dir);

        let certification = snapshot.certification.unwrap();
        assert_eq!(snapshot.status, EvidenceStatus::Pass);
        assert!(snapshot.index.is_none());
        assert_eq!(
            certification.summary_path,
            Some(evidence_dir.join("summary.md"))
        );
        assert_eq!(certification.generated_at, "2026-07-09T04:22:38Z");
        assert_eq!(certification.release_binary_bytes, Some(6_137_088));
        assert_eq!(certification.cli_import_events_per_second, Some(2_000));
    }

    #[test]
    fn evidence_snapshot_errors_when_indexed_certification_payload_is_missing() {
        let dir = tempdir().unwrap();
        let evidence_dir = certification_dir_for_project(dir.path());
        fs::create_dir_all(&evidence_dir).unwrap();
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

        assert_eq!(snapshot.status, EvidenceStatus::Error);
        assert!(snapshot.certification.is_none());
        assert!(snapshot
            .message
            .contains("failed to load certification payload"));
        assert!(snapshot.message.contains("metrics.json"));
    }

    #[test]
    fn evidence_snapshot_loads_recall_quality_record_from_index() {
        let dir = tempdir().unwrap();
        let evidence_dir = certification_dir_for_project(dir.path());
        fs::create_dir_all(evidence_dir.join("recall-quality")).unwrap();
        fs::write(
            evidence_dir.join("metrics.json"),
            r#"{"ok":true,"created_at":"2026-07-09T04:22:38Z"}"#,
        )
        .unwrap();
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
        assert_eq!(
            recall_quality.queries[0].returned_ids,
            vec!["rq_scar_stale_cache"]
        );
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
}
