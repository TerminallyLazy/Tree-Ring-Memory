use crate::evidence::{
    read_or_create_index, rollup_index_status, write_index, EvidenceRecordRef, EvidenceStatus,
};
use crate::integrations::{scan_integrations, IntegrationMarker, MarkerOrigin};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
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
        command: "tree-ring integrations certify --source-root <source_root>".to_string(),
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
    let combined_lower = combined.to_lowercase();
    HarnessGuidanceEvidence {
        agents_md,
        skill_md,
        cli_md,
        recall_guidance: combined_lower.contains("tree-ring recall"),
        remember_guidance: combined_lower.contains("tree-ring remember"),
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

fn merge_harness_index(
    evidence_dir: &Path,
    generated_at: &str,
    records: &[HarnessProbeRecord],
) -> Result<PathBuf, String> {
    let mut index = read_or_create_index(evidence_dir, generated_at)?;
    index.generated_at = generated_at.to_string();
    for record in records {
        index.harness.insert(
            record.harness_id.clone(),
            EvidenceRecordRef {
                category: "harness".to_string(),
                status: record.status,
                label: record.name.clone(),
                path: PathBuf::from(format!("harness/{}.json", record.harness_id)),
                summary_path: None,
                generated_at: record.generated_at.clone(),
            },
        );
    }
    index.missing.retain(|item| item != "harness");
    if index.recall_quality.is_none() && !index.missing.iter().any(|item| item == "recall_quality")
    {
        index.missing.push("recall_quality".to_string());
    }
    index.missing.sort();
    index.missing.dedup();
    index.overall_status = rollup_index_status(&index);
    write_index(evidence_dir, &index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::certification_dir_for_project;
    use std::collections::BTreeMap;
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
        let index: crate::evidence::EvidenceIndex = serde_json::from_str(
            &std::fs::read_to_string(evidence_dir.join("evidence-index.json")).unwrap(),
        )
        .unwrap();
        assert!(index.harness.contains_key("codex"));
        assert_eq!(
            index.harness.get("codex").map(|record| record.status),
            Some(EvidenceStatus::Skip)
        );
        assert!(!dir.path().join(".codex/generated-by-certify").exists());
    }

    #[test]
    fn harness_certification_passes_project_marker_with_generated_guidance() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codex")).unwrap();
        std::fs::create_dir_all(dir.path().join(".tree-ring")).unwrap();
        std::fs::write(
            dir.path().join(".tree-ring/SKILL.md"),
            "Use `TREE-RING RECALL` before acting and `Tree-Ring Remember` for durable facts.",
        )
        .unwrap();
        std::fs::write(
            dir.path().join(".tree-ring/CLI.md"),
            "`Tree-Ring Recall` and `TREE-RING REMEMBER` are the portable command surface.",
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
        assert!(claude
            .summary
            .contains("missing generated Tree Ring guidance"));
        assert!(claude.next_step.contains("tree-ring init"));
    }

    #[test]
    fn harness_certification_preserves_existing_certification_index_entry() {
        let dir = tempdir().unwrap();
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
              "harness": {
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

        certify_harnesses(HarnessCertificationRequest {
            source_root: dir.path().to_path_buf(),
            evidence_dir: evidence_dir.clone(),
        })
        .unwrap();

        let index: crate::evidence::EvidenceIndex = serde_json::from_str(
            &std::fs::read_to_string(evidence_dir.join("evidence-index.json")).unwrap(),
        )
        .unwrap();
        assert!(index.certification.is_some());
        assert_eq!(
            index
                .certification
                .as_ref()
                .map(|record| record.path.clone()),
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
        assert_eq!(index.missing, vec!["recall_quality".to_string()]);
        assert_eq!(index.overall_status, EvidenceStatus::Pass);
    }

    #[test]
    fn harness_certification_home_only_marker_produces_skip_record_with_anti_overclaim_guidance() {
        let generated_at = "2026-07-09T05:44:48Z";
        let source_root = Path::new("/tmp/example project");
        let guidance = HarnessGuidanceEvidence {
            agents_md: None,
            skill_md: None,
            cli_md: None,
            recall_guidance: false,
            remember_guidance: false,
        };
        let integration = crate::integrations::AgentIntegration {
            id: "claude-code",
            name: "Claude Code",
            status: crate::integrations::IntegrationStatus::Detected,
            confidence: 0.7,
            markers: vec![IntegrationMarker {
                path: "/Users/test/.claude".to_string(),
                origin: MarkerOrigin::Home,
            }],
            next_step: "Reference `.tree-ring/SKILL.md` from `CLAUDE.md` or `.claude` project instructions.",
        };

        let record = probe_record(&integration, source_root, generated_at, &guidance);

        assert_eq!(record.status, EvidenceStatus::Skip);
        assert_eq!(
            record.summary,
            "Claude Code only has user-home markers; this project is not certified for that harness."
        );
        assert_eq!(
            record.next_step,
            "Add a project-level harness marker or project instruction file, then rerun `tree-ring integrations certify`."
        );
        assert_eq!(
            record.command,
            "tree-ring integrations certify --source-root <source_root>"
        );
    }

    #[test]
    fn harness_certification_rollup_keeps_certification_status_when_only_skips_are_present() {
        let index = crate::evidence::EvidenceIndex {
            generated_at: "2026-07-09T05:44:48Z".to_string(),
            overall_status: EvidenceStatus::Missing,
            certification: Some(EvidenceRecordRef {
                category: "certification".to_string(),
                status: EvidenceStatus::Pass,
                label: "Local certification".to_string(),
                path: PathBuf::from("metrics.json"),
                summary_path: Some(PathBuf::from("summary.md")),
                generated_at: "2026-07-09T05:44:48Z".to_string(),
            }),
            harness: BTreeMap::from([(
                "codex".to_string(),
                EvidenceRecordRef {
                    category: "harness".to_string(),
                    status: EvidenceStatus::Skip,
                    label: "Codex".to_string(),
                    path: PathBuf::from("harness/codex.json"),
                    summary_path: None,
                    generated_at: "2026-07-09T05:44:48Z".to_string(),
                },
            )]),
            recall_quality: None,
            missing: vec!["recall_quality".to_string()],
            stale: Vec::new(),
        };

        assert_eq!(rollup_index_status(&index), EvidenceStatus::Pass);
    }

    #[test]
    fn harness_certification_rollup_skips_when_only_harness_passes_exist_without_certification() {
        let index = crate::evidence::EvidenceIndex {
            generated_at: "2026-07-09T05:44:48Z".to_string(),
            overall_status: EvidenceStatus::Missing,
            certification: None,
            harness: BTreeMap::from([(
                "codex".to_string(),
                EvidenceRecordRef {
                    category: "harness".to_string(),
                    status: EvidenceStatus::Pass,
                    label: "Codex".to_string(),
                    path: PathBuf::from("harness/codex.json"),
                    summary_path: None,
                    generated_at: "2026-07-09T05:44:48Z".to_string(),
                },
            )]),
            recall_quality: None,
            missing: vec!["recall_quality".to_string()],
            stale: Vec::new(),
        };

        assert_eq!(rollup_index_status(&index), EvidenceStatus::Skip);
    }
}
