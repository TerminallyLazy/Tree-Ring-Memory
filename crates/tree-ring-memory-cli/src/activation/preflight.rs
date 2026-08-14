use super::{
    adapters::{adapter_capability, adapter_version, ActivationProject},
    bridge::ProjectFs,
    manifest::{
        bridge_fingerprint, fingerprint, fingerprint_path,
        invalidate_all_receipts_for_adapter_locked, invalidate_receipts_for_adapter_locked,
        load_manifest_locked, prune_receipts_locked, validate_manifest, write_receipt_locked,
        ActivationManifest, ActivationReceipt,
    },
    ActivationState, SessionIdentity, ACTIVATION_PROTOCOL_VERSION, ACTIVATION_SCHEMA_VERSION,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fmt,
    path::Path,
    time::{Duration as StdDuration, Instant},
};
use tree_ring_memory_core::SensitivityGuard;
use tree_ring_memory_sqlite::{MemoryRetriever, RecallOptions, RecallResult, SQLiteMemoryStore};
use uuid::Uuid;

const FALLBACK_QUERY: &str = "project startup constraints";
const MAX_RESULTS: usize = 8;
const MAX_CONTEXT_BYTES: usize = 32 * 1024;
const PREFLIGHT_TIMEOUT_MS: u64 = 10_000;
const STORAGE_ERROR: &str = "activation preflight storage unavailable";
const CONTRACT_ERROR: &str = "invalid preflight harness contract";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreflightContextFormat {
    ClaudeSessionStart,
    PiBeforeAgentStart,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightRequest {
    pub harness_id: String,
    pub identity: SessionIdentity,
    pub task_hint: Option<String>,
    pub context_format: PreflightContextFormat,
    input_contract: PreflightInputContract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreflightInputContract {
    DirectIdentityFlags,
    AdapterStdin,
    ClaudeWrapper,
}

impl PreflightRequest {
    /// Constructs the direct identity request supported by the Codex wrapper.
    /// Adapter-owned hooks must go through `parse_adapter_stdin` instead.
    pub fn direct(
        harness_id: impl Into<String>,
        identity: SessionIdentity,
        task_hint: Option<String>,
        context_format: PreflightContextFormat,
    ) -> Self {
        Self {
            harness_id: harness_id.into(),
            identity,
            task_hint,
            context_format,
            input_contract: PreflightInputContract::DirectIdentityFlags,
        }
    }

    fn adapter_stdin(
        harness_id: impl Into<String>,
        identity: SessionIdentity,
        task_hint: Option<String>,
        context_format: PreflightContextFormat,
    ) -> Self {
        Self {
            harness_id: harness_id.into(),
            identity,
            task_hint,
            context_format,
            input_contract: PreflightInputContract::AdapterStdin,
        }
    }

    /// Constructs the launcher-owned Claude Code request. The wrapper binds a
    /// fresh trusted identity rather than widening the direct Codex contract.
    pub(crate) fn claude_wrapper(task_hint: Option<String>) -> Self {
        let invocation_id = Uuid::new_v4().hyphenated().to_string();
        Self {
            harness_id: "claude-code".to_string(),
            identity: SessionIdentity {
                agent_profile: "claude-code".to_string(),
                workflow_id: format!("claude-launch-{invocation_id}"),
                session_id: format!("claude-session-{invocation_id}"),
            },
            task_hint,
            context_format: PreflightContextFormat::Json,
            input_contract: PreflightInputContract::ClaudeWrapper,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivationReceiptSummary {
    pub schema_version: u16,
    pub protocol_version: u16,
    pub receipt_id: String,
    pub harness_id: String,
    pub adapter_version: String,
    pub bridge_fingerprint: String,
    pub store_id: String,
    pub project_root_fingerprint: String,
    pub worker_key_fingerprint: String,
    pub query_class: String,
    pub result_count: usize,
    pub selected_memory_ids_sha256: String,
    pub duration_ms: u64,
    pub status: String,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightResponse {
    pub state: ActivationState,
    pub context: String,
    pub receipt: ActivationReceiptSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationError(String);

impl ActivationError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ActivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ActivationError {}

/// Runs bounded, identity-scoped recall and records proof only after output renders.
pub fn run_preflight(
    store: &SQLiteMemoryStore,
    project: &ActivationProject,
    manifest: &ActivationManifest,
    request: PreflightRequest,
) -> Result<PreflightResponse, ActivationError> {
    let prepared = prepare_preflight(store, project, manifest, request)?;
    commit_prepared_preflight(store, project, manifest, prepared)
}

/// Opaque preflight material that has passed recall and render checks but has
/// not yet produced a durable activation receipt.
pub(crate) struct PreparedPreflight {
    request: PreflightRequest,
    snapshot: PreflightSnapshot,
    receipt: ActivationReceipt,
    response: PreflightResponse,
}

impl PreparedPreflight {
    pub(crate) fn context(&self) -> &str {
        &self.response.context
    }

    pub(crate) fn receipt_id(&self) -> &str {
        &self.receipt.receipt_id
    }
}

/// Prepares safe, rendered preflight material without persisting a receipt.
/// Only sibling activation integrations can cross this transactional boundary.
pub(crate) fn prepare_preflight(
    store: &SQLiteMemoryStore,
    project: &ActivationProject,
    manifest: &ActivationManifest,
    request: PreflightRequest,
) -> Result<PreparedPreflight, ActivationError> {
    prepare_preflight_with_timeout(
        store,
        project,
        manifest,
        request,
        StdDuration::from_millis(PREFLIGHT_TIMEOUT_MS),
    )
}

fn prepare_preflight_with_timeout(
    store: &SQLiteMemoryStore,
    project: &ActivationProject,
    manifest: &ActivationManifest,
    request: PreflightRequest,
    timeout: StdDuration,
) -> Result<PreparedPreflight, ActivationError> {
    validate_request_contract(&request)?;
    validate_identity(&request.identity)?;
    validate_manifest(manifest)
        .map_err(|_| ActivationError::new("activation manifest is invalid"))?;
    ensure_store_path_matches(store, project)?;

    let receipt_project =
        ActivationProject::from_memory_root(project.memory_root.clone()).map_err(storage_error)?;
    let project_fs = ProjectFs::open(&receipt_project).map_err(storage_error)?;
    let snapshot = prepare_preflight_contract(&project_fs, project, manifest, &request)?;

    let started = Instant::now();
    let (query, query_class) = safe_query(request.task_hint.as_deref());
    let results = match MemoryRetriever::new(store).recall_with_options_timeout(
        query,
        &RecallOptions {
            project: Some(&snapshot.project_name),
            agent_profile: Some(&request.identity.agent_profile),
            workflow_id: Some(&request.identity.workflow_id),
            session_id: Some(&request.identity.session_id),
            scope: None,
            rings: None,
            event_types: None,
            include_sensitive: false,
            include_superseded: false,
            limit: MAX_RESULTS,
            explain_ranking: false,
        },
        timeout,
    ) {
        Ok(results) => results,
        Err(_) if started.elapsed() >= timeout => {
            return Err(ActivationError::new("preflight timeout"));
        }
        Err(_) => return Err(ActivationError::new("scoped recall failed")),
    };
    let safe_results = safe_results(results);
    let context = render_safe_recall_context(&safe_results)?;
    let selected_memory_ids_sha256 = selected_memory_ids_digest(&safe_results);
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if started.elapsed() >= timeout {
        return Err(ActivationError::new("preflight timeout"));
    }

    let receipt = ActivationReceipt {
        schema_version: ACTIVATION_SCHEMA_VERSION,
        protocol_version: ACTIVATION_PROTOCOL_VERSION,
        receipt_id: format!("receipt-{}", Uuid::new_v4()),
        harness_id: request.harness_id.clone(),
        adapter_version: snapshot.contract.adapter_version.clone(),
        bridge_fingerprint: snapshot.contract.bridge_fingerprint.clone(),
        store_id: snapshot.manifest.store_id.clone(),
        project_root_fingerprint: snapshot.manifest.project_root_fingerprint.clone(),
        worker_key_fingerprint: fingerprint(&request.identity.agent_profile),
        session: request.identity.clone(),
        state: snapshot.state,
        query_class: query_class.to_string(),
        result_count: safe_results.len(),
        selected_memory_ids_sha256,
        duration_ms,
        status: "success".to_string(),
        recorded_at: Utc::now(),
    };
    let response = PreflightResponse {
        state: snapshot.state,
        context,
        receipt: receipt_summary(&receipt),
    };

    // Force construction of the complete adapter payload before any receipt exists.
    render_for_format(&response, request.context_format)?;
    Ok(PreparedPreflight {
        request,
        snapshot,
        receipt,
        response,
    })
}

/// Revalidates the complete activation contract and commits the receipt for
/// previously prepared material.
pub(crate) fn commit_prepared_preflight(
    store: &SQLiteMemoryStore,
    project: &ActivationProject,
    manifest: &ActivationManifest,
    prepared: PreparedPreflight,
) -> Result<PreflightResponse, ActivationError> {
    let receipt_project =
        ActivationProject::from_memory_root(project.memory_root.clone()).map_err(storage_error)?;
    let project_fs = ProjectFs::open(&receipt_project).map_err(storage_error)?;
    run_pre_commit_hook(&prepared.request);
    commit_receipt(
        &project_fs,
        store,
        project,
        manifest,
        &prepared.snapshot,
        &prepared.request,
        &prepared.receipt,
    )?;
    Ok(prepared.response)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CurrentActivationContract {
    adapter_capability: super::AdapterCapability,
    adapter_version: String,
    bridge_fingerprint: String,
    manifest_matches_registry: bool,
    eligible_for_preflight: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreflightSnapshot {
    manifest: ActivationManifest,
    contract: CurrentActivationContract,
    state: ActivationState,
    project_name: String,
}

fn prepare_preflight_contract(
    project_fs: &ProjectFs,
    project: &ActivationProject,
    supplied: &ActivationManifest,
    request: &PreflightRequest,
) -> Result<PreflightSnapshot, ActivationError> {
    let _manifest_lock = project_fs.lock_manifest().map_err(storage_error)?;
    let persisted = load_manifest_locked(project_fs).map_err(storage_error)?;
    let contract = current_activation_contract(&persisted, &request.harness_id)?;
    invalidate_current_receipts(project_fs, &persisted, &request.harness_id, &contract)?;
    ensure_current_and_eligible(project_fs, &request.harness_id, &contract)?;
    let state = match preflight_state(project, &persisted) {
        Ok(state) => state,
        Err(error) => {
            invalidate_all_receipts_for_adapter_locked(project_fs, &request.harness_id)
                .map_err(storage_error)?;
            return Err(error);
        }
    };
    let project_name = match project_scope_name(project) {
        Ok(name) => name,
        Err(error) => {
            invalidate_all_receipts_for_adapter_locked(project_fs, &request.harness_id)
                .map_err(storage_error)?;
            return Err(error);
        }
    };
    ensure_persisted_manifest_matches(&persisted, supplied)?;
    Ok(PreflightSnapshot {
        manifest: persisted,
        contract,
        state,
        project_name,
    })
}

fn commit_receipt(
    project_fs: &ProjectFs,
    store: &SQLiteMemoryStore,
    project: &ActivationProject,
    supplied: &ActivationManifest,
    snapshot: &PreflightSnapshot,
    request: &PreflightRequest,
    receipt: &ActivationReceipt,
) -> Result<(), ActivationError> {
    let _manifest_lock = project_fs.lock_manifest().map_err(storage_error)?;
    let persisted = load_manifest_locked(project_fs).map_err(storage_error)?;
    ensure_store_path_matches(store, project)?;
    let contract = current_activation_contract(&persisted, &request.harness_id)?;
    invalidate_current_receipts(project_fs, &persisted, &request.harness_id, &contract)?;
    ensure_current_and_eligible(project_fs, &request.harness_id, &contract)?;
    let state = match preflight_state(project, &persisted) {
        Ok(state) => state,
        Err(error) => {
            invalidate_all_receipts_for_adapter_locked(project_fs, &request.harness_id)
                .map_err(storage_error)?;
            return Err(error);
        }
    };
    if persisted != *supplied
        || persisted != snapshot.manifest
        || contract != snapshot.contract
        || state != snapshot.state
        || receipt.adapter_version != contract.adapter_version
        || receipt.bridge_fingerprint != contract.bridge_fingerprint
        || receipt.store_id != persisted.store_id
        || receipt.project_root_fingerprint != persisted.project_root_fingerprint
    {
        return Err(ActivationError::new(
            "activation contract changed while preparing receipt",
        ));
    }
    prune_receipts_locked(
        project_fs,
        &receipt.harness_id,
        &receipt.session.agent_profile,
        Utc::now(),
    )
    .map_err(storage_error)?;
    write_receipt_locked(project_fs, receipt).map_err(storage_error)?;
    Ok(())
}

fn current_activation_contract(
    manifest: &ActivationManifest,
    harness_id: &str,
) -> Result<CurrentActivationContract, ActivationError> {
    let activation = manifest
        .harnesses
        .get(harness_id)
        .ok_or_else(|| ActivationError::new("harness has no activation record"))?;
    let adapter_version = adapter_version(harness_id)
        .ok_or_else(|| ActivationError::new("unknown harness adapter"))?
        .to_string();
    let adapter_capability = adapter_capability(harness_id)
        .ok_or_else(|| ActivationError::new("unknown harness adapter"))?;
    let mut registry_activation = activation.clone();
    registry_activation.adapter_version = adapter_version.clone();
    registry_activation.adapter_capability = adapter_capability;
    let bridge_fingerprint = bridge_fingerprint(harness_id, &registry_activation);
    Ok(CurrentActivationContract {
        manifest_matches_registry: activation.adapter_capability == adapter_capability
            && activation.adapter_version == adapter_version
            && activation.bridge_fingerprint == bridge_fingerprint,
        eligible_for_preflight: matches!(
            activation.state,
            ActivationState::ConfiguredAwaitingProof
                | ActivationState::NeedsTrust
                | ActivationState::Active
                | ActivationState::ActiveIsolated
        ),
        adapter_capability,
        adapter_version,
        bridge_fingerprint,
    })
}

fn invalidate_current_receipts(
    project_fs: &ProjectFs,
    manifest: &ActivationManifest,
    harness_id: &str,
    contract: &CurrentActivationContract,
) -> Result<(), ActivationError> {
    invalidate_receipts_for_adapter_locked(
        project_fs,
        harness_id,
        &contract.adapter_version,
        &contract.bridge_fingerprint,
        &manifest.project_root_fingerprint,
        &manifest.store_id,
    )
    .map_err(storage_error)?;
    Ok(())
}

fn ensure_current_and_eligible(
    project_fs: &ProjectFs,
    harness_id: &str,
    contract: &CurrentActivationContract,
) -> Result<(), ActivationError> {
    if !contract.manifest_matches_registry {
        invalidate_all_receipts_for_adapter_locked(project_fs, harness_id)
            .map_err(storage_error)?;
        return Err(ActivationError::new(
            "stale adapter version or bridge fingerprint",
        ));
    }
    if !contract.eligible_for_preflight {
        invalidate_all_receipts_for_adapter_locked(project_fs, harness_id)
            .map_err(storage_error)?;
        return Err(ActivationError::new(
            "harness is not eligible for preflight",
        ));
    }
    Ok(())
}

pub fn render_claude_session_start(
    response: &PreflightResponse,
) -> Result<String, ActivationError> {
    serde_json::to_string(&serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": response.context,
        }
    }))
    .map_err(|_| ActivationError::new("failed to serialize context"))
}

pub fn render_pi_context(response: &PreflightResponse) -> Result<String, ActivationError> {
    render_structured_context(response)
}

pub fn render_json_context(response: &PreflightResponse) -> Result<String, ActivationError> {
    render_structured_context(response)
}

fn render_structured_context(response: &PreflightResponse) -> Result<String, ActivationError> {
    serde_json::to_string(&serde_json::json!({
        "context": response.context,
        "state": response.state,
        "receipt": response.receipt,
    }))
    .map_err(|_| ActivationError::new("failed to serialize context"))
}

fn render_for_format(
    response: &PreflightResponse,
    format: PreflightContextFormat,
) -> Result<String, ActivationError> {
    match format {
        PreflightContextFormat::ClaudeSessionStart => render_claude_session_start(response),
        PreflightContextFormat::PiBeforeAgentStart => render_pi_context(response),
        PreflightContextFormat::Json => render_json_context(response),
    }
}

/// Parses only harness-owned JSON stdin. Codex uses direct identity flags instead.
pub fn parse_adapter_stdin(
    harness_id: &str,
    context_format: PreflightContextFormat,
    input: &str,
) -> Result<PreflightRequest, ActivationError> {
    if !matches_adapter_stdin_contract(harness_id, context_format) {
        return Err(ActivationError::new(CONTRACT_ERROR));
    }
    let value: serde_json::Value = serde_json::from_str(input)
        .map_err(|_| ActivationError::new("invalid adapter preflight stdin"))?;
    let object = value
        .as_object()
        .ok_or_else(|| ActivationError::new("invalid adapter preflight stdin"))?;
    let string = |name: &str| {
        object
            .get(name)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| ActivationError::new("invalid adapter preflight stdin"))
    };
    let task_hint = match object.get("task_hint") {
        Some(serde_json::Value::String(value)) => Some(value.clone()),
        Some(serde_json::Value::Null) | None => None,
        Some(_) => return Err(ActivationError::new("invalid adapter preflight stdin")),
    };
    let identity = match harness_id {
        "claude-code" => {
            let session_id = string("session_id")?;
            let workflow_id = object
                .get("workflow_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(session_id);
            SessionIdentity {
                agent_profile: "claude-code".to_string(),
                workflow_id: normalize_stdin_identity("workflow", workflow_id),
                session_id: normalize_stdin_identity("session", session_id),
            }
        }
        "pi" | "agent-zero" => SessionIdentity {
            agent_profile: normalize_stdin_identity("agent", string("agent_profile")?),
            workflow_id: normalize_stdin_identity("workflow", string("workflow_id")?),
            session_id: normalize_stdin_identity("session", string("session_id")?),
        },
        _ => return Err(ActivationError::new("unknown harness adapter")),
    };
    Ok(PreflightRequest::adapter_stdin(
        harness_id,
        identity,
        task_hint,
        context_format,
    ))
}

fn validate_request_contract(request: &PreflightRequest) -> Result<(), ActivationError> {
    let valid = match request.input_contract {
        PreflightInputContract::DirectIdentityFlags => {
            request.harness_id == "codex" && request.context_format == PreflightContextFormat::Json
        }
        PreflightInputContract::AdapterStdin => {
            matches_adapter_stdin_contract(&request.harness_id, request.context_format)
        }
        PreflightInputContract::ClaudeWrapper => {
            request.harness_id == "claude-code"
                && request.context_format == PreflightContextFormat::Json
                && request.identity.agent_profile == "claude-code"
                && request.identity.workflow_id.starts_with("claude-launch-")
                && request.identity.session_id.starts_with("claude-session-")
        }
    };
    valid
        .then_some(())
        .ok_or_else(|| ActivationError::new(CONTRACT_ERROR))
}

fn matches_adapter_stdin_contract(harness_id: &str, format: PreflightContextFormat) -> bool {
    matches!(
        (harness_id, format),
        ("claude-code", PreflightContextFormat::ClaudeSessionStart)
            | ("pi", PreflightContextFormat::PiBeforeAgentStart)
            | ("agent-zero", PreflightContextFormat::Json)
    )
}

fn normalize_stdin_identity(label: &str, value: &str) -> String {
    if identity_component_is_safe(value) {
        value.to_string()
    } else {
        format!("{label}-{}", &fingerprint(value)[..24])
    }
}

fn validate_identity(identity: &SessionIdentity) -> Result<(), ActivationError> {
    for value in [
        &identity.agent_profile,
        &identity.workflow_id,
        &identity.session_id,
    ] {
        if !identity_component_is_safe(value) {
            return Err(ActivationError::new("unsafe preflight identity"));
        }
    }
    Ok(())
}

fn identity_component_is_safe(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 256
        && !value.chars().any(char::is_control)
        && !value.contains(['/', '\\'])
        && !matches!(value, "." | "..")
        && SensitivityGuard::default().inspect(value).sensitivity == "normal"
}

fn ensure_persisted_manifest_matches(
    persisted: &ActivationManifest,
    supplied: &ActivationManifest,
) -> Result<(), ActivationError> {
    if persisted != supplied {
        return Err(ActivationError::new("activation manifest/store mismatch"));
    }
    Ok(())
}

fn storage_error(_error: String) -> ActivationError {
    ActivationError::new(STORAGE_ERROR)
}

fn ensure_store_path_matches(
    store: &SQLiteMemoryStore,
    project: &ActivationProject,
) -> Result<(), ActivationError> {
    let actual = store
        .database_path()
        .map_err(|_| ActivationError::new("configured memory store mismatch"))?;
    let actual = std::fs::canonicalize(actual)
        .map_err(|_| ActivationError::new("configured memory store mismatch"))?;
    let expected = std::fs::canonicalize(project.memory_root.join("memory.sqlite"))
        .map_err(|_| ActivationError::new("configured memory store mismatch"))?;
    if actual != expected {
        return Err(ActivationError::new("configured memory store mismatch"));
    }
    Ok(())
}

/// Enumerates receipt candidates through the pinned, descriptor-relative
/// activation filesystem. The fixed two-level layout prevents recursive
/// traversal and every directory/file open rejects symlinks.
pub fn read_receipt_candidates(
    memory_root: &Path,
    harness_id: &str,
) -> Result<Vec<Vec<u8>>, String> {
    let receipt_project = ActivationProject::from_memory_root(memory_root.to_path_buf())?;
    let project_fs = ProjectFs::open(&receipt_project)?;
    let harness_directory =
        std::path::PathBuf::from(".tree-ring/activation/receipts").join(harness_id);
    let Some(workers) = project_fs.directory_entries(&harness_directory)? else {
        return Ok(Vec::new());
    };
    let mut candidates = Vec::new();
    for worker in workers {
        let worker_directory = harness_directory.join(worker);
        let Some(entries) = project_fs.directory_entries(&worker_directory)? else {
            continue;
        };
        for name in entries {
            if Path::new(&name)
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("json")
            {
                continue;
            }
            candidates.push(
                project_fs
                    .read_optional(&worker_directory.join(name))?
                    .unwrap_or_default(),
            );
        }
    }
    Ok(candidates)
}

fn preflight_state(
    project: &ActivationProject,
    manifest: &ActivationManifest,
) -> Result<ActivationState, ActivationError> {
    let configured_root = canonical_or_original(&project.memory_root);
    let mounted_root = canonical_or_original(&project.project_root.join(".tree-ring"));
    let identity_root = configured_identity_root(project)?;
    if fingerprint_path(identity_root) != manifest.project_root_fingerprint {
        return Err(ActivationError::new("project root mismatch"));
    }
    Ok(if configured_root == mounted_root {
        ActivationState::Active
    } else {
        ActivationState::ActiveIsolated
    })
}

fn configured_identity_root(project: &ActivationProject) -> Result<&Path, ActivationError> {
    let configured_root = canonical_or_original(&project.memory_root);
    let mounted_root = canonical_or_original(&project.project_root.join(".tree-ring"));
    if configured_root == mounted_root {
        Ok(&project.project_root)
    } else {
        project
            .memory_root
            .parent()
            .ok_or_else(|| ActivationError::new("configured memory root mismatch"))
    }
}

fn project_scope_name(project: &ActivationProject) -> Result<String, ActivationError> {
    configured_identity_root(project)?
        .file_name()
        .and_then(OsStr::to_str)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ActivationError::new("project identity is unavailable"))
}

fn canonical_or_original(path: &Path) -> std::path::PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn safe_query(task_hint: Option<&str>) -> (&str, &'static str) {
    task_hint
        .map(str::trim)
        .filter(|hint| !hint.is_empty())
        .filter(|hint| SensitivityGuard::default().inspect(hint).sensitivity == "normal")
        .map_or((FALLBACK_QUERY, "startup_fallback"), |hint| {
            (hint, "task_hint")
        })
}

fn safe_results(mut results: Vec<RecallResult>) -> Vec<RecallResult> {
    let guard = SensitivityGuard::default();
    results.retain(|result| {
        result.memory.sensitivity == "normal"
            && identity_component_is_safe(&result.memory.id)
            && guard.inspect(&result.memory.summary).sensitivity == "normal"
            && !result.memory.summary.chars().any(char::is_control)
    });
    results.sort_by(|left, right| left.memory.id.cmp(&right.memory.id));
    results
}

fn render_safe_recall_context(results: &[RecallResult]) -> Result<String, ActivationError> {
    let mut context = String::from("Tree Ring Memory scoped preflight recall:\n");
    if results.is_empty() {
        context.push_str("- No safe memories matched this scoped query.\n");
    } else {
        for result in results {
            context.push_str("- [");
            context.push_str(&result.memory.id);
            context.push_str("] ");
            context.push_str(&result.memory.summary);
            if safe_source_reference(&result.memory.source.ref_) {
                context.push_str(" (source: ");
                context.push_str(&result.memory.source.ref_);
                context.push(')');
            }
            context.push('\n');
        }
    }
    context.push_str(
        "Project source and instructions remain authoritative; verify recalled guidance against them.",
    );
    if context.len() > MAX_CONTEXT_BYTES {
        return Err(ActivationError::new(
            "context exceeds safe serialization limit",
        ));
    }
    Ok(context)
}

fn safe_source_reference(reference: &str) -> bool {
    !reference.is_empty()
        && reference.len() <= 512
        && !Path::new(reference).is_absolute()
        && !reference.chars().any(char::is_control)
        && SensitivityGuard::default().inspect(reference).sensitivity == "normal"
}

fn selected_memory_ids_digest(results: &[RecallResult]) -> String {
    let mut ids = results
        .iter()
        .map(|result| result.memory.id.as_str())
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    let mut hasher = Sha256::new();
    for id in ids {
        hasher.update(id.len().to_be_bytes());
        hasher.update(id.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn receipt_summary(receipt: &ActivationReceipt) -> ActivationReceiptSummary {
    ActivationReceiptSummary {
        schema_version: receipt.schema_version,
        protocol_version: receipt.protocol_version,
        receipt_id: receipt.receipt_id.clone(),
        harness_id: receipt.harness_id.clone(),
        adapter_version: receipt.adapter_version.clone(),
        bridge_fingerprint: receipt.bridge_fingerprint.clone(),
        store_id: receipt.store_id.clone(),
        project_root_fingerprint: receipt.project_root_fingerprint.clone(),
        worker_key_fingerprint: receipt.worker_key_fingerprint.clone(),
        query_class: receipt.query_class.clone(),
        result_count: receipt.result_count,
        selected_memory_ids_sha256: receipt.selected_memory_ids_sha256.clone(),
        duration_ms: receipt.duration_ms,
        status: receipt.status.clone(),
        recorded_at: receipt.recorded_at,
    }
}

#[cfg(not(test))]
fn run_pre_commit_hook(_request: &PreflightRequest) {}

#[cfg(test)]
type PreCommitHook = Box<dyn FnOnce() + Send>;

#[cfg(test)]
static PRE_COMMIT_HOOK: std::sync::Mutex<Option<(String, PreCommitHook)>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
fn run_pre_commit_hook(request: &PreflightRequest) {
    let hook = {
        let mut guard = PRE_COMMIT_HOOK
            .lock()
            .expect("pre-commit hook mutex poisoned");
        if guard
            .as_ref()
            .is_some_and(|(session_id, _)| session_id == &request.identity.session_id)
        {
            guard.take().map(|(_, hook)| hook)
        } else {
            None
        }
    };
    if let Some(hook) = hook {
        hook();
    }
}

#[cfg(test)]
fn install_pre_commit_hook(session_id: impl Into<String>, hook: impl FnOnce() + Send + 'static) {
    *PRE_COMMIT_HOOK
        .lock()
        .expect("pre-commit hook mutex poisoned") = Some((session_id.into(), Box::new(hook)));
}

pub fn project_fingerprint(project_root: &Path) -> String {
    fingerprint_path(project_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activation::{
        adapters::ActivationProject,
        manifest::{
            bridge_fingerprint, receipt_files, save_manifest, ActivationManifest,
            HarnessActivation, OwnedBridgeFile,
        },
        ActivationState, AdapterCapability, SessionIdentity, ACTIVATION_PROTOCOL_VERSION,
        ACTIVATION_SCHEMA_VERSION,
    };
    use std::{collections::BTreeMap, fs};
    #[cfg(unix)]
    use std::{
        ffi::OsString,
        os::unix::{ffi::OsStringExt, fs::symlink},
    };
    use tree_ring_memory_core::{MemoryEvent, MemorySource};
    use tree_ring_memory_sqlite::SQLiteMemoryStore;

    fn fixture() -> (
        tempfile::TempDir,
        ActivationProject,
        SQLiteMemoryStore,
        ActivationManifest,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path().join("project");
        let project = ActivationProject::from_project_root(&project_root);
        fs::create_dir_all(&project.memory_root).unwrap();
        let mut activation = HarnessActivation {
            state: ActivationState::ConfiguredAwaitingProof,
            adapter_capability: AdapterCapability::NativePreflight,
            adapter_version: "1".to_string(),
            bridge_fingerprint: String::new(),
            bridge_path: Some(".agents/skills/tree-ring-memory/SKILL.md".to_string()),
            owned_files: vec![OwnedBridgeFile {
                path: ".agents/skills/tree-ring-memory/SKILL.md".to_string(),
                sha256: "b".repeat(64),
            }],
            managed_blocks: Vec::new(),
        };
        activation.bridge_fingerprint = bridge_fingerprint("pi", &activation);
        let manifest = ActivationManifest {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            protocol_version: ACTIVATION_PROTOCOL_VERSION,
            store_id: "store-test".to_string(),
            project_root_fingerprint: project_fingerprint(&project_root),
            cli_version: env!("CARGO_PKG_VERSION").to_string(),
            harnesses: BTreeMap::from([("pi".to_string(), activation)]),
        };
        save_manifest(&project.memory_root, &manifest).unwrap();
        let mut store = SQLiteMemoryStore::open(project.memory_root.join("memory.sqlite")).unwrap();
        let mut safe =
            MemoryEvent::new("project startup constraint uses local receipts", "lesson").unwrap();
        safe.project = Some("project".to_string());
        safe.agent_profile = Some("pi".to_string());
        safe.workflow_id = Some("workflow-1".to_string());
        safe.session_id = Some("session-1".to_string());
        safe.scope = "agent".to_string();
        safe.source = MemorySource {
            source_type: "agent".to_string(),
            ref_: "docs/constraints.md".to_string(),
            quote: String::new(),
        };
        store.put(&safe).unwrap();
        let mut sensitive = safe.clone();
        sensitive.id = "mem-sensitive-fixture".to_string();
        sensitive.summary = "sensitive fixture project startup constraint".to_string();
        sensitive.sensitivity = "health".to_string();
        store.put(&sensitive).unwrap();
        (temp, project, store, manifest)
    }

    fn fixture_request() -> PreflightRequest {
        PreflightRequest::adapter_stdin(
            "pi",
            SessionIdentity {
                agent_profile: "pi".to_string(),
                workflow_id: "workflow-1".to_string(),
                session_id: "session-1".to_string(),
            },
            Some("project startup constraints".to_string()),
            PreflightContextFormat::PiBeforeAgentStart,
        )
    }

    #[test]
    fn preflight_injects_only_safe_recall_and_writes_a_matching_receipt() {
        let (_temp, project, store, manifest) = fixture();

        let response = run_preflight(&store, &project, &manifest, fixture_request()).unwrap();

        assert!(response.context.contains("project startup constraint"));
        assert!(!response.context.contains("sensitive fixture"));
        assert_eq!(response.receipt.query_class, "task_hint");
        assert_eq!(response.receipt.result_count, 1);
        assert_eq!(response.receipt.store_id, manifest.store_id);
        assert_eq!(receipt_files(&project.memory_root).len(), 1);
    }

    #[test]
    fn sensitive_task_hint_uses_exact_fallback_without_persisting_input() {
        let (_temp, project, store, manifest) = fixture();
        let secret = "Use sk-proj-abcdefghijklmnopqrstuvwxyz1234567890";
        let mut request = fixture_request();
        request.task_hint = Some(secret.to_string());

        let response = run_preflight(&store, &project, &manifest, request).unwrap();

        assert_eq!(response.receipt.query_class, "startup_fallback");
        let receipt_json = fs::read_to_string(&receipt_files(&project.memory_root)[0]).unwrap();
        assert!(!receipt_json.contains(secret));
        assert!(!receipt_json.contains("project startup constraint uses local receipts"));
    }

    #[test]
    fn zero_result_recall_writes_valid_proof_without_inventing_memory_context() {
        let (_temp, project, store, manifest) = fixture();
        let mut request = fixture_request();
        request.identity.session_id = "session-with-no-memories".to_string();

        let response = run_preflight(&store, &project, &manifest, request).unwrap();

        assert_eq!(response.receipt.result_count, 0);
        assert!(response.context.contains("No safe memories matched"));
        assert_eq!(receipt_files(&project.memory_root).len(), 1);
    }

    #[test]
    fn adapter_renderers_emit_exact_single_json_values_without_identity_fields() {
        let (_temp, project, store, manifest) = fixture();
        let response = run_preflight(&store, &project, &manifest, fixture_request()).unwrap();

        let claude: serde_json::Value =
            serde_json::from_str(&render_claude_session_start(&response).unwrap()).unwrap();
        assert_eq!(
            claude,
            serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "SessionStart",
                    "additionalContext": response.context,
                }
            })
        );
        let pi = render_pi_context(&response).unwrap();
        let json = render_json_context(&response).unwrap();
        assert_eq!(pi, json);
        let structured: serde_json::Value = serde_json::from_str(&pi).unwrap();
        assert_eq!(structured["state"], "active");
        assert_eq!(structured["receipt"]["result_count"], 1);
        assert!(structured.get("session").is_none());
        assert!(!pi.contains("workflow-1"));
        assert!(!pi.contains("session-1"));
    }

    #[test]
    fn malformed_adapter_stdin_is_rejected_without_echo_or_receipt() {
        let (_temp, project, _store, _manifest) = fixture();
        let malformed = r#"{"task_hint":"private prompt","session_id":7}"#;

        let error =
            parse_adapter_stdin("pi", PreflightContextFormat::PiBeforeAgentStart, malformed)
                .unwrap_err();

        assert_eq!(error.to_string(), "invalid adapter preflight stdin");
        assert!(!error.to_string().contains("private prompt"));
        assert!(receipt_files(&project.memory_root).is_empty());
    }

    #[test]
    fn harness_contract_mismatch_fails_before_recall_or_receipt() {
        let (_temp, project, store, manifest) = fixture();
        let request = PreflightRequest::direct(
            "pi",
            SessionIdentity {
                agent_profile: "pi".to_string(),
                workflow_id: "workflow-1".to_string(),
                session_id: "session-1".to_string(),
            },
            Some("project startup constraints".to_string()),
            PreflightContextFormat::Json,
        );

        let error = run_preflight(&store, &project, &manifest, request).unwrap_err();

        assert_eq!(error.to_string(), CONTRACT_ERROR);
        assert!(receipt_files(&project.memory_root).is_empty());
    }

    #[test]
    fn parser_binds_each_adapter_to_its_owned_event_format() {
        let pi = r#"{"agent_profile":"pi","workflow_id":"workflow","session_id":"session"}"#;
        let agent_zero =
            r#"{"agent_profile":"agent-zero","workflow_id":"workflow","session_id":"session"}"#;
        let claude = r#"{"session_id":"session"}"#;

        assert!(parse_adapter_stdin(
            "claude-code",
            PreflightContextFormat::ClaudeSessionStart,
            claude,
        )
        .is_ok());
        assert!(parse_adapter_stdin("pi", PreflightContextFormat::PiBeforeAgentStart, pi).is_ok());
        assert!(
            parse_adapter_stdin("agent-zero", PreflightContextFormat::Json, agent_zero).is_ok()
        );
        assert_eq!(
            parse_adapter_stdin("pi", PreflightContextFormat::Json, pi)
                .unwrap_err()
                .to_string(),
            CONTRACT_ERROR
        );
        assert_eq!(
            parse_adapter_stdin("codex", PreflightContextFormat::Json, "{}")
                .unwrap_err()
                .to_string(),
            CONTRACT_ERROR
        );
        assert!(validate_request_contract(&PreflightRequest::direct(
            "codex",
            SessionIdentity {
                agent_profile: "codex".to_string(),
                workflow_id: "workflow".to_string(),
                session_id: "session".to_string(),
            },
            None,
            PreflightContextFormat::Json,
        ))
        .is_ok());
    }

    #[test]
    fn adapter_stdin_fingerprints_unsafe_identity_and_never_persists_raw_paths() {
        let (_temp, project, store, mut manifest) = fixture();
        let raw_path = "/private/tmp/pi/session.jsonl";
        let input = serde_json::json!({
            "agent_profile": "pi",
            "workflow_id": raw_path,
            "session_id": raw_path,
            "task_hint": "project startup constraints",
        })
        .to_string();
        let request =
            parse_adapter_stdin("pi", PreflightContextFormat::PiBeforeAgentStart, &input).unwrap();
        assert!(request.identity.workflow_id.starts_with("workflow-"));
        assert!(!request.identity.workflow_id.contains('/'));
        manifest.harnesses.get_mut("pi").unwrap().state = ActivationState::ConfiguredAwaitingProof;
        save_manifest(&project.memory_root, &manifest).unwrap();

        run_preflight(&store, &project, &manifest, request).unwrap();

        let receipt_json = fs::read_to_string(&receipt_files(&project.memory_root)[0]).unwrap();
        assert!(!receipt_json.contains(raw_path));
        assert!(!receipt_json.contains("/private/"));
    }

    #[test]
    fn oversized_context_fails_before_any_receipt_is_written() {
        let (_temp, project, mut store, manifest) = fixture();
        let mut huge = MemoryEvent::new(
            format!(
                "project startup constraints {}",
                "x".repeat(MAX_CONTEXT_BYTES)
            ),
            "lesson",
        )
        .unwrap();
        huge.project = Some("project".to_string());
        huge.agent_profile = Some("pi".to_string());
        huge.workflow_id = Some("workflow-1".to_string());
        huge.session_id = Some("session-1".to_string());
        huge.scope = "agent".to_string();
        store.put(&huge).unwrap();

        let error = run_preflight(&store, &project, &manifest, fixture_request()).unwrap_err();

        assert!(error.to_string().contains("context"));
        assert!(receipt_files(&project.memory_root).is_empty());
    }

    #[test]
    fn delayed_sqlite_recall_is_interrupted_at_the_preflight_deadline() {
        let (_temp, project, store, manifest) = fixture();
        let database = project.memory_root.join("memory.sqlite");
        drop(store);

        let journal = rusqlite::Connection::open(&database).unwrap();
        let mode: String = journal
            .query_row("PRAGMA journal_mode=DELETE", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "delete");
        drop(journal);

        let store = SQLiteMemoryStore::open_read_only(&database).unwrap();
        let locker = rusqlite::Connection::open(&database).unwrap();
        locker.execute_batch("BEGIN EXCLUSIVE").unwrap();
        let started = Instant::now();

        let error = prepare_preflight_with_timeout(
            &store,
            &project,
            &manifest,
            fixture_request(),
            StdDuration::from_millis(50),
        )
        .err()
        .expect("locked recall must time out");
        let elapsed = started.elapsed();
        locker.execute_batch("ROLLBACK").unwrap();

        assert_eq!(error.to_string(), "preflight timeout");
        assert!(elapsed < StdDuration::from_secs(2), "elapsed: {elapsed:?}");
        assert!(receipt_files(&project.memory_root).is_empty());
    }

    #[test]
    fn registry_version_mismatch_invalidates_prior_receipts_and_fails_closed() {
        let (_temp, project, store, mut manifest) = fixture();
        run_preflight(&store, &project, &manifest, fixture_request()).unwrap();
        assert_eq!(receipt_files(&project.memory_root).len(), 1);
        let activation = manifest.harnesses.get_mut("pi").unwrap();
        activation.adapter_version = "2".to_string();
        activation.bridge_fingerprint = bridge_fingerprint("pi", activation);
        save_manifest(&project.memory_root, &manifest).unwrap();

        let error = run_preflight(&store, &project, &manifest, fixture_request()).unwrap_err();

        assert!(error.to_string().contains("stale adapter version"));
        assert!(receipt_files(&project.memory_root).is_empty());
    }

    #[test]
    fn bridge_mutation_between_prepare_and_commit_invalidates_old_receipts() {
        let (_temp, project, store, manifest) = fixture();
        run_preflight(&store, &project, &manifest, fixture_request()).unwrap();
        assert_eq!(receipt_files(&project.memory_root).len(), 1);

        let memory_root = project.memory_root.clone();
        install_pre_commit_hook("mutation-between-prepare-and-commit", move || {
            let lock_project = ActivationProject::from_memory_root(memory_root.clone()).unwrap();
            let project_fs = ProjectFs::open(&lock_project).unwrap();
            let _activation_lock = project_fs.lock_manifest().unwrap();
            let mut changed = crate::activation::load_manifest(&memory_root).unwrap();
            let activation = changed.harnesses.get_mut("pi").unwrap();
            activation.owned_files[0].sha256 = "c".repeat(64);
            activation.bridge_fingerprint = bridge_fingerprint("pi", activation);
            save_manifest(&memory_root, &changed).unwrap();
        });
        let mut request = fixture_request();
        request.identity.session_id = "mutation-between-prepare-and-commit".to_string();

        let error = run_preflight(&store, &project, &manifest, request).unwrap_err();

        assert_eq!(
            error.to_string(),
            "activation contract changed while preparing receipt"
        );
        assert!(receipt_files(&project.memory_root).is_empty());
    }

    #[test]
    fn persisted_bridge_contract_mismatch_removes_old_receipt_before_recall() {
        let (_temp, project, store, manifest) = fixture();
        run_preflight(&store, &project, &manifest, fixture_request()).unwrap();
        assert_eq!(receipt_files(&project.memory_root).len(), 1);
        let mut changed = manifest.clone();
        let activation = changed.harnesses.get_mut("pi").unwrap();
        activation.owned_files[0].sha256 = "d".repeat(64);
        activation.bridge_fingerprint = bridge_fingerprint("pi", activation);
        save_manifest(&project.memory_root, &changed).unwrap();

        let error = run_preflight(&store, &project, &manifest, fixture_request()).unwrap_err();

        assert_eq!(error.to_string(), "activation manifest/store mismatch");
        assert!(receipt_files(&project.memory_root).is_empty());
    }

    #[test]
    fn supplied_store_identity_mismatch_writes_no_receipt() {
        let (_temp, project, store, manifest) = fixture();
        let mut mismatched = manifest.clone();
        mismatched.store_id = "different-store".to_string();

        let error = run_preflight(&store, &project, &mismatched, fixture_request()).unwrap_err();

        assert!(error.to_string().contains("manifest/store mismatch"));
        assert!(receipt_files(&project.memory_root).is_empty());
    }

    #[test]
    fn sqlite_store_path_mismatch_writes_no_receipt() {
        let (_temp, project, _store, manifest) = fixture();
        let other = tempfile::tempdir().unwrap();
        let other_store = SQLiteMemoryStore::open(other.path().join("memory.sqlite")).unwrap();

        let error =
            run_preflight(&other_store, &project, &manifest, fixture_request()).unwrap_err();

        assert!(error.to_string().contains("memory store mismatch"));
        assert!(receipt_files(&project.memory_root).is_empty());
    }

    #[test]
    fn pathless_sqlite_store_writes_no_receipt() {
        let (_temp, project, _store, manifest) = fixture();
        let in_memory = SQLiteMemoryStore::open(":memory:").unwrap();

        let error = run_preflight(&in_memory, &project, &manifest, fixture_request()).unwrap_err();

        assert!(error.to_string().contains("memory store mismatch"));
        assert!(receipt_files(&project.memory_root).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_receipt_directory_fails_closed_without_touching_outside() {
        let (temp, project, store, manifest) = fixture();
        let outside = temp.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        let sentinel = outside.join("sentinel.json");
        fs::write(&sentinel, b"outside receipt sentinel").unwrap();
        symlink(&outside, project.memory_root.join("activation")).unwrap();

        let error = run_preflight(&store, &project, &manifest, fixture_request()).unwrap_err();

        assert_eq!(error.to_string(), STORAGE_ERROR);
        assert_eq!(fs::read(&sentinel).unwrap(), b"outside receipt sentinel");
        assert!(!outside.join("receipts").exists());
    }

    #[test]
    fn storage_failures_use_a_fixed_path_free_adapter_diagnostic() {
        let (_temp, project, store, manifest) = fixture();
        fs::write(project.memory_root.join("activation"), b"not a directory").unwrap();

        let error = run_preflight(&store, &project, &manifest, fixture_request()).unwrap_err();

        assert_eq!(error.to_string(), STORAGE_ERROR);
        assert!(!error
            .to_string()
            .contains(&project.memory_root.display().to_string()));
        assert!(receipt_files(&project.memory_root).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_project_basename_fails_closed_before_unscoped_recall() {
        // APFS rejects invalid byte sequences, so exercise the trust boundary
        // before filesystem access rather than relying on filesystem-specific
        // creation behavior.
        let project_root = std::path::PathBuf::from(OsString::from_vec(b"project-\xff".to_vec()));
        let project = ActivationProject {
            memory_root: project_root.join(".tree-ring"),
            project_root,
        };

        assert_eq!(
            project_scope_name(&project).unwrap_err().to_string(),
            "project identity is unavailable"
        );
    }

    #[test]
    fn project_root_mismatch_writes_no_receipt() {
        let (_temp, project, store, mut manifest) = fixture();
        manifest.project_root_fingerprint = "d".repeat(64);
        save_manifest(&project.memory_root, &manifest).unwrap();

        let error = run_preflight(&store, &project, &manifest, fixture_request()).unwrap_err();

        assert!(error.to_string().contains("project root mismatch"));
        assert!(receipt_files(&project.memory_root).is_empty());
    }

    #[test]
    fn configured_nonmounted_store_is_active_isolated_and_never_copies_data() {
        let temp = tempfile::tempdir().unwrap();
        let mounted_root = temp.path().join("mounted-project");
        fs::create_dir_all(&mounted_root).unwrap();
        let isolated_project_root = temp.path().join("agent-zero-store");
        let memory_root = isolated_project_root.join(".tree-ring");
        fs::create_dir_all(&memory_root).unwrap();
        let project = ActivationProject {
            project_root: mounted_root.clone(),
            memory_root: memory_root.clone(),
        };
        let mut activation = HarnessActivation {
            state: ActivationState::ConfiguredAwaitingProof,
            adapter_capability: AdapterCapability::NativePreflight,
            adapter_version: "1".to_string(),
            bridge_fingerprint: String::new(),
            bridge_path: Some(".tree-ring/activation/agent-zero.json".to_string()),
            owned_files: vec![OwnedBridgeFile {
                path: ".tree-ring/activation/agent-zero.json".to_string(),
                sha256: "e".repeat(64),
            }],
            managed_blocks: Vec::new(),
        };
        activation.bridge_fingerprint = bridge_fingerprint("agent-zero", &activation);
        let manifest = ActivationManifest {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            protocol_version: ACTIVATION_PROTOCOL_VERSION,
            store_id: "isolated-store".to_string(),
            project_root_fingerprint: project_fingerprint(&isolated_project_root),
            cli_version: env!("CARGO_PKG_VERSION").to_string(),
            harnesses: BTreeMap::from([("agent-zero".to_string(), activation)]),
        };
        save_manifest(&memory_root, &manifest).unwrap();
        let mut store = SQLiteMemoryStore::open(memory_root.join("memory.sqlite")).unwrap();
        let mut local = MemoryEvent::new("isolated local startup constraint", "lesson").unwrap();
        local.project = Some("agent-zero-store".to_string());
        local.agent_profile = Some("agent-zero".to_string());
        local.workflow_id = Some("workflow-1".to_string());
        local.session_id = Some("session-1".to_string());
        local.scope = "agent".to_string();
        store.put(&local).unwrap();
        let request = PreflightRequest::adapter_stdin(
            "agent-zero",
            SessionIdentity {
                agent_profile: "agent-zero".to_string(),
                workflow_id: "workflow-1".to_string(),
                session_id: "session-1".to_string(),
            },
            None,
            PreflightContextFormat::Json,
        );

        let response = run_preflight(&store, &project, &manifest, request).unwrap();

        assert_eq!(response.state, ActivationState::ActiveIsolated);
        assert_eq!(response.receipt.result_count, 1);
        assert!(response
            .context
            .contains("isolated local startup constraint"));
        assert_eq!(receipt_files(&memory_root).len(), 1);
        assert!(!mounted_root.join(".tree-ring").exists());
    }

    #[test]
    fn preflight_prunes_expired_receipt_before_recording_new_proof() {
        let (_temp, project, store, manifest) = fixture();
        run_preflight(&store, &project, &manifest, fixture_request()).unwrap();
        let path = receipt_files(&project.memory_root).pop().unwrap();
        let mut receipt: crate::activation::ActivationReceipt =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        receipt.recorded_at = Utc::now() - chrono::Duration::days(31);
        fs::write(&path, serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();

        run_preflight(&store, &project, &manifest, fixture_request()).unwrap();

        let receipts = receipt_files(&project.memory_root);
        assert_eq!(receipts.len(), 1);
        assert_ne!(receipts[0], path);
    }
}
