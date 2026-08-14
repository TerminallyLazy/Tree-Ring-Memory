use crate::actions::integrations::{
    project_root_fingerprint, verify_activation_receipts, ReceiptVerificationStatus,
};
use crate::evidence::{
    atomic_write, publish_indexed_evidence, rollup_index_status, EvidenceRecordRef, EvidenceStatus,
    HARNESS_ACTIVATION_SUMMARY_FILE,
};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tree_ring_memory_cli::activation::adapters::{
    adapter_version, scan_integrations, AdapterDetection, IntegrationMarker, MarkerOrigin,
};
use tree_ring_memory_cli::activation::{ActivationManifest, ActivationState, AdapterCapability};

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
    pub memory_root: PathBuf,
    pub evidence_dir: PathBuf,
    pub live: bool,
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
    pub activation: HarnessActivationEvidence,
    pub markers: Vec<HarnessProbeMarker>,
    pub guidance: HarnessGuidanceEvidence,
    pub summary: String,
    pub next_step: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessActivationEvidence {
    pub adapter_version: String,
    pub adapter_capability: AdapterCapability,
    pub state: ActivationState,
    pub receipt_recorded_at: Option<String>,
    pub receipt_age_seconds: Option<i64>,
    pub store_id_matches: bool,
    pub project_root_matches: bool,
    pub diagnostic: String,
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
    let manifest = inspect_manifest(&request.memory_root);
    let mut records = Vec::new();
    for harness_id in CERTIFIED_HARNESS_IDS {
        let integration = report
            .integrations
            .iter()
            .find(|integration| integration.id == *harness_id)
            .ok_or_else(|| format!("missing integration definition for {harness_id}"))?;
        let record = probe_record(
            integration,
            &request.source_root,
            &request.memory_root,
            &generated_at,
            &guidance,
            &manifest,
            request.live,
        );
        records.push(record);
    }

    let index_path = publish_harness_evidence(&request.evidence_dir, &generated_at, &records)?;
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
    integration: &AdapterDetection,
    source_root: &Path,
    memory_root: &Path,
    generated_at: &str,
    guidance: &HarnessGuidanceEvidence,
    manifest: &ManifestInspection,
    live: bool,
) -> HarnessProbeRecord {
    let expected_adapter_version = adapter_version(&integration.id).unwrap_or("unknown");
    let expected_project_fingerprint = project_root_fingerprint(source_root);
    let result = if live && integration.executable_version.is_none() {
        activation_result(
            EvidenceStatus::Skip,
            expected_adapter_version,
            integration.capability,
            integration.state,
            None,
            [false, false],
            "not installed locally",
        )
    } else {
        match manifest {
            ManifestInspection::Missing => activation_result(
                EvidenceStatus::Skip,
                expected_adapter_version,
                integration.capability,
                configured_state_without_receipt(integration.state),
                None,
                [false, false],
                "activation manifest not found",
            ),
            ManifestInspection::Invalid => activation_result(
                EvidenceStatus::Fail,
                expected_adapter_version,
                integration.capability,
                ActivationState::Failed,
                None,
                [false, false],
                "activation manifest is malformed or invalid",
            ),
            ManifestInspection::Valid(manifest) => {
                let root_matches =
                    manifest.project_root_fingerprint == expected_project_fingerprint;
                let Some(harness) = manifest.harnesses.get(&integration.id) else {
                    return finish_record(
                        integration,
                        generated_at,
                        guidance,
                        activation_result(
                            EvidenceStatus::Skip,
                            expected_adapter_version,
                            integration.capability,
                            configured_state_without_receipt(integration.state),
                            None,
                            [false, root_matches],
                            "harness has no activation record",
                        ),
                    );
                };
                if !root_matches {
                    activation_result(
                        EvidenceStatus::Fail,
                        &harness.adapter_version,
                        harness.adapter_capability,
                        ActivationState::Failed,
                        None,
                        [false, false],
                        "activation manifest belongs to a different project root",
                    )
                } else if harness.adapter_version != expected_adapter_version {
                    activation_result(
                        EvidenceStatus::Fail,
                        &harness.adapter_version,
                        harness.adapter_capability,
                        ActivationState::Failed,
                        None,
                        [false, true],
                        "activation adapter version does not match the registered adapter",
                    )
                } else if harness.adapter_capability != integration.capability {
                    activation_result(
                        EvidenceStatus::Fail,
                        &harness.adapter_version,
                        harness.adapter_capability,
                        ActivationState::Failed,
                        None,
                        [false, true],
                        "activation adapter capability does not match the registered adapter",
                    )
                } else if integration.state == ActivationState::Unsupported {
                    activation_result(
                        EvidenceStatus::Skip,
                        &harness.adapter_version,
                        harness.adapter_capability,
                        ActivationState::Unsupported,
                        None,
                        [false, true],
                        "adapter is unsupported",
                    )
                } else {
                    let verification =
                        verify_activation_receipts(memory_root, &integration.id, manifest, harness);
                    let receipt_metadata = verification.receipt.as_ref().map(|receipt| {
                        (
                            receipt
                                .recorded_at
                                .to_rfc3339_opts(SecondsFormat::Secs, true),
                            Utc::now()
                                .signed_duration_since(receipt.recorded_at)
                                .num_seconds()
                                .max(0),
                            receipt.state,
                        )
                    });
                    match verification.status {
                        ReceiptVerificationStatus::Valid => {
                            let state = receipt_metadata
                                .as_ref()
                                .map(|(_, _, state)| *state)
                                .unwrap_or(ActivationState::Failed);
                            let evidence_status = if state == ActivationState::Active {
                                EvidenceStatus::Pass
                            } else {
                                EvidenceStatus::Skip
                            };
                            activation_result(
                                evidence_status,
                                &harness.adapter_version,
                                harness.adapter_capability,
                                state,
                                receipt_metadata,
                                [
                                    verification.store_id_matches,
                                    verification.project_root_matches,
                                ],
                                if state == ActivationState::ActiveIsolated {
                                    "fresh receipt proves isolated activation only"
                                } else {
                                    verification.diagnostic
                                },
                            )
                        }
                        ReceiptVerificationStatus::Missing => activation_result(
                            EvidenceStatus::Skip,
                            &harness.adapter_version,
                            harness.adapter_capability,
                            configured_state_without_receipt(harness.state),
                            None,
                            [false, true],
                            verification.diagnostic,
                        ),
                        ReceiptVerificationStatus::Invalid => activation_result(
                            EvidenceStatus::Fail,
                            &harness.adapter_version,
                            harness.adapter_capability,
                            ActivationState::Failed,
                            receipt_metadata,
                            [
                                verification.store_id_matches,
                                verification.project_root_matches,
                            ],
                            verification.diagnostic,
                        ),
                    }
                }
            }
        }
    };

    finish_record(integration, generated_at, guidance, result)
}

type ActivationResult = (EvidenceStatus, HarnessActivationEvidence);

fn activation_result(
    status: EvidenceStatus,
    adapter_version: &str,
    adapter_capability: AdapterCapability,
    state: ActivationState,
    receipt: Option<(String, i64, ActivationState)>,
    contract_matches: [bool; 2],
    diagnostic: &'static str,
) -> ActivationResult {
    let receipt_recorded_at = receipt.as_ref().map(|(timestamp, _, _)| timestamp.clone());
    let receipt_age_seconds = receipt.map(|(_, age, _)| age);
    (
        status,
        HarnessActivationEvidence {
            adapter_version: adapter_version.to_string(),
            adapter_capability,
            state,
            receipt_recorded_at,
            receipt_age_seconds,
            store_id_matches: contract_matches[0],
            project_root_matches: contract_matches[1],
            diagnostic: diagnostic.to_string(),
        },
    )
}

fn finish_record(
    integration: &AdapterDetection,
    generated_at: &str,
    guidance: &HarnessGuidanceEvidence,
    result: ActivationResult,
) -> HarnessProbeRecord {
    let (status, activation) = result;
    let summary = format!("{}: {}.", integration.name, activation.diagnostic);
    let next_step = next_step_for_state(activation.state, &integration.next_step);
    HarnessProbeRecord {
        schema_version: 1,
        harness_id: integration.id.to_string(),
        name: integration.name.to_string(),
        status,
        generated_at: generated_at.to_string(),
        source_root: PathBuf::from("<source-root>"),
        command: "tree-ring integrations certify --source-root <source_root>".to_string(),
        activation,
        markers: integration.markers.iter().map(marker_from_scan).collect(),
        guidance: guidance.clone(),
        summary,
        next_step,
    }
}

fn configured_state_without_receipt(state: ActivationState) -> ActivationState {
    match state {
        ActivationState::Active | ActivationState::ActiveIsolated => {
            ActivationState::ConfiguredAwaitingProof
        }
        state => state,
    }
}

fn next_step_for_state(state: ActivationState, detected: &str) -> String {
    match state {
        ActivationState::Active => "No action required for the receipt-backed session.".to_string(),
        ActivationState::ActiveIsolated => {
            "Bind the harness to this project's canonical store before claiming shared use."
                .to_string()
        }
        ActivationState::ConfiguredAwaitingProof => {
            "Run the adapter preflight at the start of a new harness session.".to_string()
        }
        _ => detected.to_string(),
    }
}

enum ManifestInspection {
    Missing,
    Valid(ActivationManifest),
    Invalid,
}

fn inspect_manifest(memory_root: &Path) -> ManifestInspection {
    if !memory_root.join("activation.json").exists() {
        return ManifestInspection::Missing;
    }
    match tree_ring_memory_cli::activation::load_manifest(memory_root) {
        Ok(manifest) => ManifestInspection::Valid(manifest),
        Err(_) => ManifestInspection::Invalid,
    }
}

fn inspect_guidance(source_root: &Path) -> HarnessGuidanceEvidence {
    let agents_md = existing_relative_path(source_root, ".tree-ring/AGENTS.md");
    let skill_md = existing_relative_path(source_root, ".tree-ring/SKILL.md");
    let cli_md = existing_relative_path(source_root, ".tree-ring/CLI.md");
    let combined = [agents_md.as_ref(), skill_md.as_ref(), cli_md.as_ref()]
        .into_iter()
        .flatten()
        .filter_map(|path| fs::read_to_string(source_root.join(path)).ok())
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

fn existing_relative_path(source_root: &Path, relative: &str) -> Option<PathBuf> {
    source_root
        .join(relative)
        .exists()
        .then(|| PathBuf::from(relative))
}

fn marker_from_scan(marker: &IntegrationMarker) -> HarnessProbeMarker {
    HarnessProbeMarker {
        path: if marker.origin == MarkerOrigin::Home {
            marker
                .path
                .rsplit(['/', '\\'])
                .next()
                .map(|name| format!("<home>/{name}"))
                .unwrap_or_else(|| "<home-marker>".to_string())
        } else if Path::new(&marker.path).is_absolute() {
            "<project-marker>".to_string()
        } else {
            marker.path.clone()
        },
        origin: marker.origin.as_str().to_string(),
    }
}

fn publish_harness_evidence(
    evidence_dir: &Path,
    generated_at: &str,
    records: &[HarnessProbeRecord],
) -> Result<PathBuf, String> {
    publish_indexed_evidence(evidence_dir, generated_at, |index| {
        let harness_dir = evidence_dir.join("harness");
        fs::create_dir_all(&harness_dir).map_err(|err| err.to_string())?;
        for record in records {
            let path = harness_dir.join(format!("{}.json", record.harness_id));
            let json = serde_json::to_string_pretty(record).map_err(|err| err.to_string())?;
            atomic_write(&path, json.as_bytes())?;
        }
        atomic_write(
            &evidence_dir.join(HARNESS_ACTIVATION_SUMMARY_FILE),
            render_harness_summary(generated_at, records).as_bytes(),
        )?;

        index.generated_at = generated_at.to_string();
        for record in records {
            index.harness.insert(
                record.harness_id.clone(),
                EvidenceRecordRef {
                    category: "harness".to_string(),
                    status: record.status,
                    label: record.name.clone(),
                    path: PathBuf::from(format!("harness/{}.json", record.harness_id)),
                    summary_path: Some(PathBuf::from(HARNESS_ACTIVATION_SUMMARY_FILE)),
                    generated_at: record.generated_at.clone(),
                },
            );
        }
        index.missing.retain(|item| item != "harness");
        if index.recall_quality.is_none()
            && !index.missing.iter().any(|item| item == "recall_quality")
        {
            index.missing.push("recall_quality".to_string());
        }
        index.missing.sort();
        index.missing.dedup();
        index.overall_status = rollup_index_status(index);
        Ok(())
    })
}

fn render_harness_summary(generated_at: &str, records: &[HarnessProbeRecord]) -> String {
    let mut lines = vec![
        "# Harness activation evidence".to_string(),
        String::new(),
        format!("Generated: {generated_at}"),
        String::new(),
        "| Harness | Evidence | Activation state | Diagnostic |".to_string(),
        "| --- | --- | --- | --- |".to_string(),
    ];
    for record in records {
        lines.push(format!(
            "| {} | {} | {:?} | {} |",
            record.name,
            record.status.as_str(),
            record.activation.state,
            record.activation.diagnostic.replace('|', "-")
        ));
    }
    lines.push(String::new());
    lines.push(
        "Markers are detection context only. A pass requires a fresh receipt matching the current adapter and project store contract."
            .to_string(),
    );
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::certification_dir_for_project;
    use chrono::Duration;
    use std::collections::BTreeMap;
    use tempfile::tempdir;
    use tree_ring_memory_cli::activation::manifest::bridge_fingerprint;
    use tree_ring_memory_cli::activation::{
        save_manifest, write_receipt, ActivationReceipt, HarnessActivation, SessionIdentity,
        ACTIVATION_PROTOCOL_VERSION, ACTIVATION_SCHEMA_VERSION,
    };

    fn request(project: &Path) -> HarnessCertificationRequest {
        HarnessCertificationRequest {
            source_root: project.to_path_buf(),
            memory_root: project.join(".tree-ring"),
            evidence_dir: certification_dir_for_project(project),
            live: false,
        }
    }

    fn write_activation(
        project: &Path,
        harness_id: &str,
        state: ActivationState,
    ) -> ActivationManifest {
        let memory_root = project.join(".tree-ring");
        fs::create_dir_all(&memory_root).unwrap();
        let detection = scan_integrations(project)
            .by_id(harness_id)
            .cloned()
            .unwrap();
        let mut harness = HarnessActivation {
            state,
            adapter_capability: detection.capability,
            adapter_version: adapter_version(harness_id).unwrap().to_string(),
            bridge_fingerprint: String::new(),
            bridge_path: None,
            owned_files: Vec::new(),
            managed_blocks: Vec::new(),
        };
        harness.bridge_fingerprint = bridge_fingerprint(harness_id, &harness);
        let manifest = ActivationManifest {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            protocol_version: ACTIVATION_PROTOCOL_VERSION,
            store_id: "test-store".to_string(),
            project_root_fingerprint: project_root_fingerprint(project),
            cli_version: env!("CARGO_PKG_VERSION").to_string(),
            harnesses: BTreeMap::from([(harness_id.to_string(), harness)]),
        };
        save_manifest(&memory_root, &manifest).unwrap();
        manifest
    }

    fn matching_receipt(
        manifest: &ActivationManifest,
        harness_id: &str,
        state: ActivationState,
    ) -> ActivationReceipt {
        let harness = manifest.harnesses.get(harness_id).unwrap();
        ActivationReceipt {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            protocol_version: manifest.protocol_version,
            receipt_id: "receipt-1".to_string(),
            harness_id: harness_id.to_string(),
            adapter_version: harness.adapter_version.clone(),
            bridge_fingerprint: harness.bridge_fingerprint.clone(),
            store_id: manifest.store_id.clone(),
            project_root_fingerprint: manifest.project_root_fingerprint.clone(),
            worker_key_fingerprint: "a".repeat(64),
            session: SessionIdentity {
                agent_profile: "worker-a".to_string(),
                workflow_id: "workflow-a".to_string(),
                session_id: "session-a".to_string(),
            },
            state,
            query_class: "startup_fallback".to_string(),
            result_count: 1,
            selected_memory_ids_sha256: "b".repeat(64),
            duration_ms: 1,
            status: "success".to_string(),
            recorded_at: Utc::now() - Duration::seconds(5),
        }
    }

    fn record<'a>(
        report: &'a HarnessCertificationReport,
        harness_id: &str,
    ) -> &'a HarnessProbeRecord {
        report
            .records
            .iter()
            .find(|record| record.harness_id == harness_id)
            .unwrap()
    }

    #[test]
    fn harness_certification_passes_only_a_fresh_matching_receipt() {
        let dir = tempdir().unwrap();
        let manifest = write_activation(
            dir.path(),
            "claude-code",
            ActivationState::ConfiguredAwaitingProof,
        );
        write_receipt(
            &dir.path().join(".tree-ring"),
            &matching_receipt(&manifest, "claude-code", ActivationState::Active),
        )
        .unwrap();

        let report = certify_harnesses(request(dir.path())).unwrap();
        let claude = record(&report, "claude-code");

        assert_eq!(claude.status, EvidenceStatus::Pass);
        assert_eq!(claude.activation.state, ActivationState::Active);
        assert!(claude.activation.store_id_matches);
        assert!(claude.activation.project_root_matches);
        assert!(claude.activation.receipt_age_seconds.is_some());
        assert_eq!(
            claude.activation.diagnostic,
            "fresh matching activation receipt"
        );
    }

    #[test]
    fn harness_certification_rejects_expired_or_mismatched_receipts() {
        enum Mutation {
            Expired,
            Adapter,
            Fingerprint,
            Store,
            Root,
        }
        for mutation in [
            Mutation::Expired,
            Mutation::Adapter,
            Mutation::Fingerprint,
            Mutation::Store,
            Mutation::Root,
        ] {
            let dir = tempdir().unwrap();
            let manifest = write_activation(
                dir.path(),
                "codex",
                ActivationState::ConfiguredAwaitingProof,
            );
            let mut receipt = matching_receipt(&manifest, "codex", ActivationState::Active);
            match mutation {
                Mutation::Expired => receipt.recorded_at = Utc::now() - Duration::days(31),
                Mutation::Adapter => receipt.adapter_version = "wrong-adapter".to_string(),
                Mutation::Fingerprint => receipt.bridge_fingerprint = "c".repeat(64),
                Mutation::Store => receipt.store_id = "wrong-store".to_string(),
                Mutation::Root => receipt.project_root_fingerprint = "d".repeat(64),
            }
            write_receipt(&dir.path().join(".tree-ring"), &receipt).unwrap();

            let report = certify_harnesses(request(dir.path())).unwrap();
            let codex = record(&report, "codex");
            assert_eq!(codex.status, EvidenceStatus::Fail);
            assert_eq!(codex.activation.state, ActivationState::Failed);
            assert!(!codex
                .activation
                .diagnostic
                .contains(dir.path().to_str().unwrap()));
        }
    }

    #[test]
    fn harness_certification_fails_malformed_receipt_with_redacted_cause() {
        let dir = tempdir().unwrap();
        write_activation(
            dir.path(),
            "codex",
            ActivationState::ConfiguredAwaitingProof,
        );
        let receipt_dir = dir
            .path()
            .join(".tree-ring/activation/receipts/codex/worker");
        fs::create_dir_all(&receipt_dir).unwrap();
        fs::write(receipt_dir.join("broken.json"), b"{not-json").unwrap();

        let report = certify_harnesses(request(dir.path())).unwrap();
        let codex = record(&report, "codex");

        assert_eq!(codex.status, EvidenceStatus::Fail);
        assert_eq!(
            codex.activation.diagnostic,
            "activation receipt is malformed"
        );
        let serialized = serde_json::to_string(codex).unwrap();
        assert!(!serialized.contains(dir.path().to_str().unwrap()));
        assert!(!serialized.contains("not-json"));
    }

    #[test]
    fn harness_certification_downgrades_manifest_active_without_receipt() {
        let dir = tempdir().unwrap();
        write_activation(dir.path(), "codex", ActivationState::Active);

        let report = certify_harnesses(request(dir.path())).unwrap();
        let codex = record(&report, "codex");

        assert_eq!(codex.status, EvidenceStatus::Skip);
        assert_eq!(
            codex.activation.state,
            ActivationState::ConfiguredAwaitingProof
        );
        assert_eq!(codex.activation.diagnostic, "no activation receipt");
    }

    #[test]
    fn harness_certification_keeps_active_isolated_distinct_and_nonpassing() {
        let dir = tempdir().unwrap();
        let manifest = write_activation(
            dir.path(),
            "agent-zero",
            ActivationState::ConfiguredAwaitingProof,
        );
        write_receipt(
            &dir.path().join(".tree-ring"),
            &matching_receipt(&manifest, "agent-zero", ActivationState::ActiveIsolated),
        )
        .unwrap();

        let report = certify_harnesses(request(dir.path())).unwrap();
        let agent_zero = record(&report, "agent-zero");

        assert_eq!(agent_zero.status, EvidenceStatus::Skip);
        assert_eq!(agent_zero.activation.state, ActivationState::ActiveIsolated);
        assert_eq!(
            agent_zero.activation.diagnostic,
            "fresh receipt proves isolated activation only"
        );
    }

    #[test]
    fn harness_certification_live_missing_executable_is_exact_skip() {
        let integration = AdapterDetection {
            id: "claude-code".to_string(),
            name: "Claude Code".to_string(),
            capability: AdapterCapability::WrapperPreflight,
            executable_version: None,
            status: tree_ring_memory_cli::activation::adapters::IntegrationStatus::Available,
            state: ActivationState::ConfiguredAwaitingProof,
            markers: Vec::new(),
            plan: tree_ring_memory_cli::activation::adapters::AdapterPlan {
                harness_id: "claude-code".to_string(),
                state: ActivationState::ConfiguredAwaitingProof,
                writes: Vec::new(),
                next_step: "Install the optional executable.".to_string(),
            },
            next_step: "Install the optional executable.".to_string(),
        };
        let record = probe_record(
            &integration,
            Path::new("/project"),
            Path::new("/project/.tree-ring"),
            "2026-08-14T00:00:00Z",
            &HarnessGuidanceEvidence {
                agents_md: None,
                skill_md: None,
                cli_md: None,
                recall_guidance: false,
                remember_guidance: false,
            },
            &ManifestInspection::Missing,
            true,
        );

        assert_eq!(record.status, EvidenceStatus::Skip);
        assert_eq!(record.activation.diagnostic, "not installed locally");
        assert_eq!(record.summary, "Claude Code: not installed locally.");
    }

    #[test]
    fn harness_certification_publishes_redacted_markdown_summary() {
        let dir = tempdir().unwrap();
        let report = certify_harnesses(request(dir.path())).unwrap();
        let summary_path = report.evidence_dir.join(HARNESS_ACTIVATION_SUMMARY_FILE);
        let summary = fs::read_to_string(summary_path).unwrap();

        assert!(summary.contains("# Harness activation evidence"));
        assert!(summary.contains("Markers are detection context only"));
        assert!(!summary.contains(dir.path().to_str().unwrap()));
        let index: crate::evidence::EvidenceIndex =
            serde_json::from_str(&fs::read_to_string(report.index_path).unwrap()).unwrap();
        assert_eq!(
            index.harness["codex"].summary_path,
            Some(PathBuf::from(HARNESS_ACTIVATION_SUMMARY_FILE))
        );
    }

    #[test]
    fn harness_certification_skips_absent_project_markers_and_indexes_records() {
        let dir = tempdir().unwrap();
        let evidence_dir = certification_dir_for_project(dir.path());

        let report = certify_harnesses(HarnessCertificationRequest {
            source_root: dir.path().to_path_buf(),
            memory_root: dir.path().join(".tree-ring"),
            evidence_dir: evidence_dir.clone(),
            live: false,
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
    fn harness_certification_skips_non_active_project_marker_with_generated_guidance() {
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
            memory_root: dir.path().join(".tree-ring"),
            evidence_dir: evidence_dir.clone(),
            live: false,
        })
        .unwrap();

        let codex = report
            .records
            .iter()
            .find(|record| record.harness_id == "codex")
            .unwrap();
        assert_eq!(codex.status, EvidenceStatus::Skip);
        assert_eq!(
            codex.activation.state,
            ActivationState::ConfiguredAwaitingProof
        );
        assert_eq!(codex.activation.diagnostic, "activation manifest not found");
        assert!(codex.guidance.recall_guidance);
        assert!(codex.guidance.remember_guidance);
        assert!(evidence_dir.join("harness/codex.json").exists());
    }

    #[test]
    fn harness_certification_marker_without_generated_guidance_still_awaits_receipt() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("CLAUDE.md"), "# Claude instructions").unwrap();
        let evidence_dir = certification_dir_for_project(dir.path());

        let report = certify_harnesses(HarnessCertificationRequest {
            source_root: dir.path().to_path_buf(),
            memory_root: dir.path().join(".tree-ring"),
            evidence_dir,
            live: false,
        })
        .unwrap();

        let claude = report
            .records
            .iter()
            .find(|record| record.harness_id == "claude-code")
            .unwrap();
        assert_eq!(claude.status, EvidenceStatus::Skip);
        assert_eq!(
            claude.activation.state,
            ActivationState::ConfiguredAwaitingProof
        );
        assert_eq!(
            claude.activation.diagnostic,
            "activation manifest not found"
        );
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
            memory_root: dir.path().join(".tree-ring"),
            evidence_dir: evidence_dir.clone(),
            live: false,
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
        let integration = AdapterDetection {
            id: "claude-code".to_string(),
            name: "Claude Code".to_string(),
            capability: tree_ring_memory_cli::activation::AdapterCapability::NativePreflight,
            executable_version: None,
            status: tree_ring_memory_cli::activation::adapters::IntegrationStatus::Detected,
            state: tree_ring_memory_cli::activation::ActivationState::ConfiguredAwaitingProof,
            markers: vec![IntegrationMarker {
                path: "/Users/test/.claude".to_string(),
                origin: MarkerOrigin::Home,
            }],
            plan: tree_ring_memory_cli::activation::adapters::AdapterPlan {
                harness_id: "claude-code".to_string(),
                state: tree_ring_memory_cli::activation::ActivationState::ConfiguredAwaitingProof,
                writes: Vec::new(),
                next_step: "Reference `.tree-ring/SKILL.md` from `CLAUDE.md` or `.claude` project instructions.".to_string(),
            },
            next_step: "Reference `.tree-ring/SKILL.md` from `CLAUDE.md` or `.claude` project instructions.".to_string(),
        };

        let record = probe_record(
            &integration,
            source_root,
            Path::new("/tmp/example project/.tree-ring"),
            generated_at,
            &guidance,
            &ManifestInspection::Missing,
            false,
        );

        assert_eq!(record.status, EvidenceStatus::Skip);
        assert_eq!(
            record.summary,
            "Claude Code: activation manifest not found."
        );
        assert_eq!(
            record.next_step,
            "Run the adapter preflight at the start of a new harness session."
        );
        assert_eq!(
            record.command,
            "tree-ring integrations certify --source-root <source_root>"
        );
    }

    #[test]
    fn blocking_adapter_states_never_certify_marker_and_guidance_as_active() {
        let guidance = HarnessGuidanceEvidence {
            agents_md: Some(PathBuf::from(".tree-ring/AGENTS.md")),
            skill_md: Some(PathBuf::from(".tree-ring/SKILL.md")),
            cli_md: Some(PathBuf::from(".tree-ring/CLI.md")),
            recall_guidance: true,
            remember_guidance: true,
        };
        for (id, state) in [
            (
                "agent-zero",
                tree_ring_memory_cli::activation::ActivationState::NeedsPlugin,
            ),
            (
                "opencode",
                tree_ring_memory_cli::activation::ActivationState::Unsupported,
            ),
            (
                "goose",
                tree_ring_memory_cli::activation::ActivationState::Unsupported,
            ),
            (
                "codex",
                tree_ring_memory_cli::activation::ActivationState::NeedsTrust,
            ),
            (
                "claude-code",
                tree_ring_memory_cli::activation::ActivationState::ConfiguredAwaitingProof,
            ),
        ] {
            let integration = AdapterDetection {
                id: id.to_string(),
                name: id.to_string(),
                capability: tree_ring_memory_cli::activation::AdapterCapability::GuidanceOnly,
                executable_version: None,
                status: tree_ring_memory_cli::activation::adapters::IntegrationStatus::Detected,
                state,
                markers: vec![IntegrationMarker {
                    path: format!(".{id}"),
                    origin: MarkerOrigin::Project,
                }],
                plan: tree_ring_memory_cli::activation::adapters::AdapterPlan {
                    harness_id: id.to_string(),
                    state,
                    writes: Vec::new(),
                    next_step: "Resolve activation before certification.".to_string(),
                },
                next_step: "Resolve activation before certification.".to_string(),
            };

            let record = probe_record(
                &integration,
                Path::new("/tmp/project"),
                Path::new("/tmp/project/.tree-ring"),
                "now",
                &guidance,
                &ManifestInspection::Missing,
                false,
            );

            assert_eq!(record.status, EvidenceStatus::Skip, "{id}");
            assert_ne!(record.activation.state, ActivationState::Active, "{id}");
            assert_eq!(
                record.activation.diagnostic, "activation manifest not found",
                "{id}"
            );
        }
    }

    #[test]
    fn harness_certification_never_passes_active_marker_without_receipt() {
        let guidance = HarnessGuidanceEvidence {
            agents_md: Some(PathBuf::from(".tree-ring/AGENTS.md")),
            skill_md: Some(PathBuf::from(".tree-ring/SKILL.md")),
            cli_md: Some(PathBuf::from(".tree-ring/CLI.md")),
            recall_guidance: true,
            remember_guidance: true,
        };
        let integration = AdapterDetection {
            id: "codex".to_string(),
            name: "Codex".to_string(),
            capability: tree_ring_memory_cli::activation::AdapterCapability::GuidanceOnly,
            executable_version: Some("1.0".to_string()),
            status: tree_ring_memory_cli::activation::adapters::IntegrationStatus::Detected,
            state: tree_ring_memory_cli::activation::ActivationState::Active,
            markers: vec![IntegrationMarker {
                path: ".codex".to_string(),
                origin: MarkerOrigin::Project,
            }],
            plan: tree_ring_memory_cli::activation::adapters::AdapterPlan {
                harness_id: "codex".to_string(),
                state: tree_ring_memory_cli::activation::ActivationState::Active,
                writes: Vec::new(),
                next_step: "Run preflight.".to_string(),
            },
            next_step: "Run preflight.".to_string(),
        };

        let record = probe_record(
            &integration,
            Path::new("/tmp/project"),
            Path::new("/tmp/project/.tree-ring"),
            "2026-08-14T00:00:00Z",
            &guidance,
            &ManifestInspection::Missing,
            false,
        );

        assert_ne!(record.status, EvidenceStatus::Pass);
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
