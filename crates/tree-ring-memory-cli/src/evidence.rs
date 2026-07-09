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
            match load_index_certification(evidence_dir, &index) {
                Ok(certification) => {
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
                Err(error) => EvidenceSnapshot {
                    root: evidence_dir.to_path_buf(),
                    index_path,
                    index: Some(index),
                    certification: None,
                    status: EvidenceStatus::Error,
                    message: error,
                },
            }
        }
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
            },
            Err(_) if !metrics_path_for_dir(evidence_dir).exists() => EvidenceSnapshot {
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
        assert_eq!(certification.summary_path, Some(evidence_dir.join("summary.md")));
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
        assert!(snapshot.message.contains("failed to load certification payload"));
        assert!(snapshot.message.contains("metrics.json"));
    }
}
