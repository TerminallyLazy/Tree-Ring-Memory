use chrono::{Duration, Utc};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tree_ring_memory_cli::activation::{
    self,
    adapters::{scan_integrations, ActivationProject, IntegrationScanReport},
    bridge::{apply_bridge_plan, deactivate_bridge_plan},
    manifest::{bridge_fingerprint, ActivationManifest, ActivationReceipt},
    preflight::{project_fingerprint, read_receipt_candidates},
    ActivationState, AdapterCapability, PreflightContextFormat, PreflightRequest, SessionIdentity,
    ACTIVATION_PROTOCOL_VERSION, ACTIVATION_SCHEMA_VERSION, RECEIPT_RETENTION_DAYS,
};
use tree_ring_memory_core::sensitivity::SensitivityGuard;
use tree_ring_memory_sqlite::SQLiteMemoryStore;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationScanRequest {
    pub source_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntegrationScanActionReport {
    pub report: IntegrationScanReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationStatusRequest {
    pub source_root: PathBuf,
    pub memory_root: PathBuf,
    pub verbose: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IntegrationStatusEntry {
    pub id: String,
    pub name: String,
    pub state: ActivationState,
    pub capability: AdapterCapability,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub managed_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_age_seconds: Option<i64>,
    pub next_step: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IntegrationStatusActionReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store_id: Option<String>,
    pub integrations: Vec<IntegrationStatusEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationActivationRequest {
    pub harness_id: String,
    pub source_root: PathBuf,
    pub memory_root: PathBuf,
    pub dry_run: bool,
    pub accept_managed_block: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IntegrationLifecycleActionReport {
    pub harness_id: String,
    pub state: ActivationState,
    pub changed_paths: Vec<String>,
    pub dry_run: bool,
    pub next_step: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReceiptVerificationStatus {
    Valid,
    Missing,
    Invalid,
}

/// Receipt validation shared by status and certification. The receipt remains
/// process-local; callers must serialize only the bounded metadata below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReceiptVerification {
    pub status: ReceiptVerificationStatus,
    pub receipt: Option<ActivationReceipt>,
    pub store_id_matches: bool,
    pub project_root_matches: bool,
    pub diagnostic: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightIdentityInput {
    pub agent_profile: Option<String>,
    pub workflow_id: Option<String>,
    pub session_id: Option<String>,
}

pub fn scan(request: IntegrationScanRequest) -> IntegrationScanActionReport {
    IntegrationScanActionReport {
        report: scan_integrations(&request.source_root),
    }
}

/// Reads activation state without opening or migrating the SQLite store.
pub fn status(request: IntegrationStatusRequest) -> Result<IntegrationStatusActionReport, String> {
    let scan = scan_integrations(&request.source_root);
    let manifest = optional_manifest(&request.memory_root)?.filter(|manifest| {
        manifest.project_root_fingerprint == project_fingerprint(&request.source_root)
    });
    let now = Utc::now();
    let integrations = scan
        .integrations
        .into_iter()
        .map(|detected| {
            let activation = manifest
                .as_ref()
                .and_then(|manifest| manifest.harnesses.get(&detected.id));
            let receipt = manifest
                .as_ref()
                .and_then(|manifest| {
                    activation.map(|activation| {
                        verify_activation_receipts_at(
                            &request.memory_root,
                            &detected.id,
                            manifest,
                            activation,
                            Utc::now(),
                        )
                    })
                })
                .and_then(|verification| {
                    (verification.status == ReceiptVerificationStatus::Valid)
                        .then_some(verification.receipt)
                        .flatten()
                });
            let state = receipt
                .as_ref()
                .map(|receipt| receipt.state)
                .or_else(|| activation.map(|activation| activation.state))
                .unwrap_or(detected.state);
            let managed_paths = if request.verbose {
                activation
                    .map(|activation| {
                        activation
                            .owned_files
                            .iter()
                            .map(|owned| owned.path.clone())
                            .chain(
                                activation
                                    .managed_blocks
                                    .iter()
                                    .map(|owned| owned.path.clone()),
                            )
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let receipt_age_seconds = request
                .verbose
                .then(|| {
                    receipt.map(|receipt| {
                        now.signed_duration_since(receipt.recorded_at)
                            .num_seconds()
                            .max(0)
                    })
                })
                .flatten();
            IntegrationStatusEntry {
                id: detected.id,
                name: detected.name,
                state,
                capability: detected.capability,
                managed_paths,
                receipt_age_seconds,
                next_step: next_step_for_state(state, &detected.next_step),
            }
        })
        .collect();
    Ok(IntegrationStatusActionReport {
        store_id: manifest.map(|manifest| manifest.store_id),
        integrations,
    })
}

/// Applies a single adapter plan, or reports its validated read-only dry-run.
pub fn activate(
    request: IntegrationActivationRequest,
) -> Result<IntegrationLifecycleActionReport, String> {
    let project = ActivationProject {
        project_root: request.source_root.clone(),
        memory_root: request.memory_root.clone(),
    };
    let detection = scan_integrations(&request.source_root)
        .by_id(&request.harness_id)
        .cloned()
        .ok_or_else(|| format!("unknown harness adapter: {}", request.harness_id))?;
    if request.dry_run {
        return Ok(IntegrationLifecycleActionReport {
            harness_id: detection.id,
            state: detection.plan.state,
            changed_paths: detection
                .plan
                .writes
                .iter()
                .map(planned_path)
                .collect::<Result<Vec<_>, _>>()?,
            dry_run: true,
            next_step: detection.plan.next_step,
        });
    }
    let mut manifest = activation::load_manifest(&request.memory_root).map_err(|_| {
        "activation manifest is unavailable; run `tree-ring init` before activation".to_string()
    })?;
    ensure_manifest_project(&manifest, &request.source_root)?;
    let result = apply_bridge_plan(
        &project,
        &mut manifest,
        detection.plan,
        request.accept_managed_block,
    )?;
    Ok(IntegrationLifecycleActionReport {
        harness_id: request.harness_id,
        state: result.state,
        changed_paths: relative_paths(result.changed_paths)?,
        dry_run: false,
        next_step: result.next_step,
    })
}

pub fn deactivate(
    harness_id: &str,
    source_root: PathBuf,
    memory_root: PathBuf,
) -> Result<IntegrationLifecycleActionReport, String> {
    let project = ActivationProject {
        project_root: source_root,
        memory_root: memory_root.clone(),
    };
    let mut manifest = activation::load_manifest(&memory_root).map_err(|_| {
        "activation manifest is unavailable; run `tree-ring init` before deactivation".to_string()
    })?;
    ensure_manifest_project(&manifest, &project.project_root)?;
    let result = deactivate_bridge_plan(&project, &mut manifest, harness_id)?;
    Ok(IntegrationLifecycleActionReport {
        harness_id: harness_id.to_string(),
        state: result.state,
        changed_paths: relative_paths(result.changed_paths)?,
        dry_run: false,
        next_step: result.next_step,
    })
}

/// Creates an in-memory manifest suitable for the first create-only bridge
/// publication. The bridge layer persists it atomically with the safe plan.
pub fn new_manifest(project_root: &Path) -> ActivationManifest {
    ActivationManifest {
        schema_version: ACTIVATION_SCHEMA_VERSION,
        protocol_version: ACTIVATION_PROTOCOL_VERSION,
        store_id: Uuid::new_v4().hyphenated().to_string(),
        project_root_fingerprint: project_fingerprint(project_root),
        cli_version: env!("CARGO_PKG_VERSION").to_string(),
        harnesses: Default::default(),
    }
}

/// Validates the complete input contract before callers open the store.
pub fn resolve_preflight_request(
    project: &ActivationProject,
    harness_id: &str,
    identity: PreflightIdentityInput,
    input_json: Option<&str>,
    context_format: PreflightContextFormat,
) -> Result<PreflightRequest, String> {
    if let Some(input) = input_json {
        if identity.agent_profile.is_some()
            || identity.workflow_id.is_some()
            || identity.session_id.is_some()
        {
            return Err(
                "--input-json-stdin cannot be combined with direct identity flags".to_string(),
            );
        }
        return resolve_adapter_stdin(project, harness_id, context_format, input);
    }

    let (Some(agent_profile), Some(workflow_id), Some(session_id)) = (
        identity.agent_profile,
        identity.workflow_id,
        identity.session_id,
    ) else {
        return Err(
            "direct preflight requires --agent-profile, --workflow-id, and --session-id"
                .to_string(),
        );
    };
    if harness_id != "codex" || context_format != PreflightContextFormat::Json {
        return Err(
            "direct identity flags are supported only for Codex JSON preflight".to_string(),
        );
    }
    Ok(PreflightRequest::direct(
        harness_id,
        SessionIdentity {
            agent_profile,
            workflow_id,
            session_id,
        },
        None,
        context_format,
    ))
}

pub fn run_preflight(
    store: &SQLiteMemoryStore,
    project: &ActivationProject,
    manifest: &ActivationManifest,
    request: PreflightRequest,
    context_format: PreflightContextFormat,
) -> Result<String, String> {
    let response = activation::run_preflight(store, project, manifest, request)
        .map_err(|error| error.to_string())?;
    match context_format {
        PreflightContextFormat::ClaudeSessionStart => {
            activation::render_claude_session_start(&response)
        }
        PreflightContextFormat::PiBeforeAgentStart => activation::render_pi_context(&response),
        PreflightContextFormat::Json => activation::render_json_context(&response),
    }
    .map_err(|error| error.to_string())
}

fn resolve_adapter_stdin(
    project: &ActivationProject,
    harness_id: &str,
    context_format: PreflightContextFormat,
    input: &str,
) -> Result<PreflightRequest, String> {
    let value: Value =
        serde_json::from_str(input).map_err(|_| "invalid adapter preflight stdin".to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "invalid adapter preflight stdin".to_string())?;
    reject_derived_or_capability_fields(object)?;
    let sanitized = match harness_id {
        "claude-code" => sanitize_claude_input(project, object)?,
        "pi" => sanitize_pi_input(object)?,
        "agent-zero" => sanitize_agent_zero_input(object)?,
        _ => return Err("unknown harness adapter".to_string()),
    };
    activation::parse_adapter_stdin(
        harness_id,
        context_format,
        &serde_json::to_string(&sanitized).map_err(|_| "invalid adapter preflight stdin")?,
    )
    .map_err(|error| error.to_string())
}

fn sanitize_claude_input(
    project: &ActivationProject,
    object: &Map<String, Value>,
) -> Result<Value, String> {
    let session_id = required_string(object, "session_id")?;
    let cwd = required_string(object, "cwd")?;
    if let Some(agent_type) = object.get("agent_type") {
        let agent_type = agent_type
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "invalid adapter preflight stdin".to_string())?;
        if agent_type != "claude-code" {
            return Err("ambiguous Claude Code adapter identity".to_string());
        }
    }
    ensure_cwd_inside_project(project, cwd)?;
    Ok(serde_json::json!({
        "session_id": session_id,
        "workflow_id": session_id,
    }))
}

fn sanitize_pi_input(object: &Map<String, Value>) -> Result<Value, String> {
    reject_unknown_fields(
        object,
        &["agent_profile", "workflow_id", "session_id", "task_hint"],
    )?;
    let mut sanitized = serde_json::json!({
        "agent_profile": required_string(object, "agent_profile")?,
        "workflow_id": required_string(object, "workflow_id")?,
        "session_id": required_string(object, "session_id")?,
    });
    if let Some(Value::String(hint)) = object.get("task_hint") {
        if !hint.trim().is_empty()
            && SensitivityGuard::default().inspect(hint).sensitivity == "normal"
        {
            sanitized["task_hint"] = Value::String(hint.clone());
        }
    } else if object
        .get("task_hint")
        .is_some_and(|value| !value.is_null())
    {
        return Err("invalid adapter preflight stdin".to_string());
    }
    Ok(sanitized)
}

fn sanitize_agent_zero_input(object: &Map<String, Value>) -> Result<Value, String> {
    reject_unknown_fields(object, &["agent_profile", "workflow_id", "session_id"])?;
    Ok(serde_json::json!({
        "agent_profile": required_string(object, "agent_profile")?,
        "workflow_id": required_string(object, "workflow_id")?,
        "session_id": required_string(object, "session_id")?,
    }))
}

fn reject_unknown_fields(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("invalid adapter preflight stdin fields".to_string());
    }
    Ok(())
}

fn reject_derived_or_capability_fields(object: &Map<String, Value>) -> Result<(), String> {
    const BLOCKED: &[&str] = &[
        "authorization",
        "bridge_fingerprint",
        "capability",
        "capabilities",
        "coordinator_capability",
        "harness_id",
        "memory_root",
        "project_root",
        "project_root_fingerprint",
        "receipt",
        "receipt_id",
        "state",
        "store_id",
        "token",
        "tree_ring_coordinator_token",
    ];
    fn contains_blocked(value: &Value, blocked: &[&str]) -> bool {
        match value {
            Value::Object(object) => object.iter().any(|(key, value)| {
                let key = key.to_ascii_lowercase().replace('-', "_");
                blocked.contains(&key.as_str()) || contains_blocked(value, blocked)
            }),
            Value::Array(values) => values.iter().any(|value| contains_blocked(value, blocked)),
            _ => false,
        }
    }
    if contains_blocked(&Value::Object(object.clone()), BLOCKED) {
        return Err("adapter preflight stdin contains a forbidden field".to_string());
    }
    Ok(())
}

fn required_string<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a str, String> {
    object
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "invalid adapter preflight stdin".to_string())
}

fn ensure_cwd_inside_project(project: &ActivationProject, cwd: &str) -> Result<(), String> {
    let project_root = fs::canonicalize(&project.project_root)
        .map_err(|_| "Claude Code project root is unavailable".to_string())?;
    let candidate = Path::new(cwd);
    let candidate = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        project_root.join(candidate)
    };
    let candidate =
        fs::canonicalize(candidate).map_err(|_| "Claude Code cwd is unavailable".to_string())?;
    if !candidate.starts_with(&project_root) {
        return Err("Claude Code cwd is outside the activation project".to_string());
    }
    Ok(())
}

fn optional_manifest(memory_root: &Path) -> Result<Option<ActivationManifest>, String> {
    if !memory_root.join("activation.json").exists() {
        return Ok(None);
    }
    activation::load_manifest(memory_root).map(Some)
}

fn ensure_manifest_project(
    manifest: &ActivationManifest,
    source_root: &Path,
) -> Result<(), String> {
    if manifest.project_root_fingerprint != project_fingerprint(source_root) {
        return Err("activation manifest does not belong to the requested project".to_string());
    }
    Ok(())
}

pub(crate) fn verify_activation_receipts(
    memory_root: &Path,
    harness_id: &str,
    manifest: &ActivationManifest,
    harness: &activation::HarnessActivation,
) -> ReceiptVerification {
    verify_activation_receipts_at(memory_root, harness_id, manifest, harness, Utc::now())
}

fn verify_activation_receipts_at(
    memory_root: &Path,
    harness_id: &str,
    manifest: &ActivationManifest,
    harness: &activation::HarnessActivation,
    now: chrono::DateTime<Utc>,
) -> ReceiptVerification {
    if harness.bridge_fingerprint != bridge_fingerprint(harness_id, harness) {
        return invalid_receipt(None, "bridge fingerprint does not match adapter contract");
    }
    let mut receipts = Vec::new();
    let candidates = match read_receipt_candidates(memory_root, harness_id) {
        Ok(candidates) => candidates,
        Err(_) => return invalid_receipt(None, "activation receipt directory is unreadable"),
    };
    let found_json = !candidates.is_empty();
    for bytes in candidates {
        if let Ok(receipt) = serde_json::from_slice::<ActivationReceipt>(&bytes) {
            receipts.push(receipt);
        }
    }

    if let Some(receipt) = receipts
        .iter()
        .filter(|receipt| receipt_matches(harness_id, manifest, harness, receipt, now))
        .max_by_key(|receipt| receipt.recorded_at)
        .cloned()
    {
        return ReceiptVerification {
            status: ReceiptVerificationStatus::Valid,
            store_id_matches: true,
            project_root_matches: true,
            receipt: Some(receipt),
            diagnostic: "fresh matching activation receipt",
        };
    }

    if let Some(receipt) = receipts
        .into_iter()
        .max_by_key(|receipt| receipt.recorded_at)
    {
        let diagnostic = receipt_mismatch(harness_id, manifest, harness, &receipt, now);
        return ReceiptVerification {
            status: ReceiptVerificationStatus::Invalid,
            store_id_matches: receipt.store_id == manifest.store_id,
            project_root_matches: receipt.project_root_fingerprint
                == manifest.project_root_fingerprint,
            receipt: Some(receipt),
            diagnostic,
        };
    }
    if found_json {
        return invalid_receipt(None, "activation receipt is malformed");
    }
    ReceiptVerification {
        status: ReceiptVerificationStatus::Missing,
        receipt: None,
        store_id_matches: false,
        project_root_matches: false,
        diagnostic: "no activation receipt",
    }
}

fn receipt_matches(
    harness_id: &str,
    manifest: &ActivationManifest,
    harness: &activation::HarnessActivation,
    receipt: &ActivationReceipt,
    now: chrono::DateTime<Utc>,
) -> bool {
    receipt.schema_version == ACTIVATION_SCHEMA_VERSION
        && receipt.harness_id == harness_id
        && receipt.protocol_version == manifest.protocol_version
        && receipt.adapter_version == harness.adapter_version
        && receipt.bridge_fingerprint == harness.bridge_fingerprint
        && receipt.store_id == manifest.store_id
        && receipt.project_root_fingerprint == manifest.project_root_fingerprint
        && receipt.status == "success"
        && matches!(
            receipt.state,
            ActivationState::Active | ActivationState::ActiveIsolated
        )
        && receipt.recorded_at <= now
        && receipt.recorded_at > now - Duration::days(RECEIPT_RETENTION_DAYS)
}

fn receipt_mismatch(
    harness_id: &str,
    manifest: &ActivationManifest,
    harness: &activation::HarnessActivation,
    receipt: &ActivationReceipt,
    now: chrono::DateTime<Utc>,
) -> &'static str {
    if receipt.schema_version != ACTIVATION_SCHEMA_VERSION {
        "activation receipt schema version mismatch"
    } else if receipt.harness_id != harness_id {
        "activation receipt harness mismatch"
    } else if receipt.protocol_version != manifest.protocol_version {
        "activation receipt protocol mismatch"
    } else if receipt.adapter_version != harness.adapter_version {
        "activation receipt adapter version mismatch"
    } else if receipt.bridge_fingerprint != harness.bridge_fingerprint {
        "activation receipt bridge fingerprint mismatch"
    } else if receipt.store_id != manifest.store_id {
        "activation receipt store mismatch"
    } else if receipt.project_root_fingerprint != manifest.project_root_fingerprint {
        "activation receipt project root mismatch"
    } else if receipt.status != "success" {
        "activation receipt does not record success"
    } else if !matches!(
        receipt.state,
        ActivationState::Active | ActivationState::ActiveIsolated
    ) {
        "activation receipt state is not active"
    } else if receipt.recorded_at > now {
        "activation receipt timestamp is in the future"
    } else {
        "activation receipt is expired"
    }
}

fn invalid_receipt(
    receipt: Option<ActivationReceipt>,
    diagnostic: &'static str,
) -> ReceiptVerification {
    ReceiptVerification {
        status: ReceiptVerificationStatus::Invalid,
        receipt,
        store_id_matches: false,
        project_root_matches: false,
        diagnostic,
    }
}

fn next_step_for_state(state: ActivationState, detected_next_step: &str) -> String {
    match state {
        ActivationState::Active => "No action required for the receipt-backed session.".to_string(),
        ActivationState::ActiveIsolated => {
            "Bind the harness to this project's canonical store before claiming shared use."
                .to_string()
        }
        ActivationState::ConfiguredAwaitingProof => {
            "Run the adapter preflight at the start of a new harness session.".to_string()
        }
        _ => detected_next_step.to_string(),
    }
}

fn planned_path(write: &activation::adapters::PlannedWrite) -> Result<String, String> {
    let path = match write {
        activation::adapters::PlannedWrite::BridgeWrite(write) => &write.path,
        activation::adapters::PlannedWrite::ManagedBlockUpdate(write) => &write.path,
    };
    relative_path(path)
}

fn relative_paths(paths: Vec<PathBuf>) -> Result<Vec<String>, String> {
    paths.into_iter().map(|path| relative_path(&path)).collect()
}

fn relative_path(path: &Path) -> Result<String, String> {
    if path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err("activation report path must be project-relative".to_string());
    }
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| "activation report path must be UTF-8".to_string())
}

pub(crate) fn project_root_fingerprint(path: &Path) -> String {
    project_fingerprint(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    fn receipt_fixture() -> (
        tempfile::TempDir,
        ActivationProject,
        ActivationManifest,
        activation::HarnessActivation,
        ActivationReceipt,
    ) {
        let temp = tempdir().unwrap();
        let project_root = temp.path().join("project");
        fs::create_dir_all(&project_root).unwrap();
        let project = ActivationProject::from_project_root(&project_root);
        fs::create_dir_all(&project.memory_root).unwrap();
        let mut harness = activation::HarnessActivation {
            state: ActivationState::ConfiguredAwaitingProof,
            adapter_capability: AdapterCapability::NativePreflight,
            adapter_version: "1".to_string(),
            bridge_fingerprint: String::new(),
            bridge_path: Some(".agents/skills/tree-ring-memory/SKILL.md".to_string()),
            owned_files: Vec::new(),
            managed_blocks: Vec::new(),
        };
        harness.bridge_fingerprint = bridge_fingerprint("pi", &harness);
        let mut manifest = new_manifest(&project_root);
        manifest.store_id = "store-test".to_string();
        manifest.harnesses.insert("pi".to_string(), harness.clone());
        let receipt = ActivationReceipt {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            protocol_version: ACTIVATION_PROTOCOL_VERSION,
            receipt_id: "receipt-test".to_string(),
            harness_id: "pi".to_string(),
            adapter_version: harness.adapter_version.clone(),
            bridge_fingerprint: harness.bridge_fingerprint.clone(),
            store_id: manifest.store_id.clone(),
            project_root_fingerprint: manifest.project_root_fingerprint.clone(),
            worker_key_fingerprint: "a".repeat(64),
            session: SessionIdentity {
                agent_profile: "pi".to_string(),
                workflow_id: "workflow".to_string(),
                session_id: "session".to_string(),
            },
            state: ActivationState::Active,
            query_class: "startup_fallback".to_string(),
            result_count: 0,
            selected_memory_ids_sha256: "b".repeat(64),
            duration_ms: 1,
            status: "success".to_string(),
            recorded_at: Utc::now(),
        };
        (temp, project, manifest, harness, receipt)
    }

    #[test]
    fn integration_action_scans_project_markers() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# Rules").unwrap();

        let report = scan(IntegrationScanRequest {
            source_root: dir.path().to_path_buf(),
        });

        assert!(report.report.detected_count > 0);
    }

    #[test]
    fn agent_zero_stdin_rejects_task_hints_and_capabilities() {
        let dir = tempdir().unwrap();
        let project = ActivationProject::from_project_root(dir.path());
        for input in [
            r#"{"agent_profile":"a","workflow_id":"w","session_id":"s","task_hint":"prompt"}"#,
            r#"{"agent_profile":"a","workflow_id":"w","session_id":"s","capability":"secret"}"#,
        ] {
            assert!(resolve_preflight_request(
                &project,
                "agent-zero",
                PreflightIdentityInput {
                    agent_profile: None,
                    workflow_id: None,
                    session_id: None,
                },
                Some(input),
                PreflightContextFormat::Json,
            )
            .is_err());
        }
    }

    #[test]
    fn claude_stdin_rejects_an_out_of_project_cwd() {
        let project_dir = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let project = ActivationProject::from_project_root(project_dir.path());
        let input = serde_json::json!({
            "session_id": "session-1",
            "cwd": outside.path(),
            "transcript_path": "/ignored/private/transcript",
        });
        assert!(resolve_preflight_request(
            &project,
            "claude-code",
            PreflightIdentityInput {
                agent_profile: None,
                workflow_id: None,
                session_id: None,
            },
            Some(&input.to_string()),
            PreflightContextFormat::ClaudeSessionStart,
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn receipt_discovery_rejects_a_symlinked_json_file_without_following_it() {
        let (temp, project, manifest, harness, receipt) = receipt_fixture();
        let worker = project
            .memory_root
            .join("activation/receipts/pi")
            .join(&receipt.worker_key_fingerprint);
        fs::create_dir_all(&worker).unwrap();
        let outside = temp.path().join("outside-receipt.json");
        fs::write(&outside, serde_json::to_vec(&receipt).unwrap()).unwrap();
        symlink(&outside, worker.join("receipt-test.json")).unwrap();

        let verification =
            verify_activation_receipts(&project.memory_root, "pi", &manifest, &harness);

        assert_eq!(verification.status, ReceiptVerificationStatus::Invalid);
        assert_eq!(
            verification.diagnostic,
            "activation receipt directory is unreadable"
        );
        assert!(verification.receipt.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn receipt_discovery_rejects_a_symlink_directory_cycle() {
        let (_temp, project, manifest, harness, _receipt) = receipt_fixture();
        let harness_directory = project.memory_root.join("activation/receipts/pi");
        fs::create_dir_all(&harness_directory).unwrap();
        symlink(".", harness_directory.join("cycle")).unwrap();

        let verification =
            verify_activation_receipts(&project.memory_root, "pi", &manifest, &harness);

        assert_eq!(verification.status, ReceiptVerificationStatus::Invalid);
        assert_eq!(
            verification.diagnostic,
            "activation receipt directory is unreadable"
        );
    }
}
