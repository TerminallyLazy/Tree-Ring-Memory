use super::{
    adapters::{adapter_version, ActivationProject},
    manifest::{
        bridge_fingerprint, fingerprint, fingerprint_path, invalidate_receipts_for_adapter,
        load_manifest, prune_receipts, validate_manifest, write_receipt, ActivationManifest,
        ActivationReceipt,
    },
    ActivationState, SessionIdentity, ACTIVATION_PROTOCOL_VERSION, ACTIVATION_SCHEMA_VERSION,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fmt, path::Path, time::Instant};
use tree_ring_memory_core::SensitivityGuard;
use tree_ring_memory_sqlite::{MemoryRetriever, RecallOptions, RecallResult, SQLiteMemoryStore};
use uuid::Uuid;

const FALLBACK_QUERY: &str = "project startup constraints";
const MAX_RESULTS: usize = 8;
const MAX_CONTEXT_BYTES: usize = 32 * 1024;
const PREFLIGHT_TIMEOUT_MS: u64 = 10_000;

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

impl From<String> for ActivationError {
    fn from(error: String) -> Self {
        Self(error)
    }
}

/// Runs bounded, identity-scoped recall and records proof only after output renders.
pub fn run_preflight(
    store: &SQLiteMemoryStore,
    project: &ActivationProject,
    manifest: &ActivationManifest,
    request: PreflightRequest,
) -> Result<PreflightResponse, ActivationError> {
    let started = Instant::now();
    validate_manifest(manifest).map_err(ActivationError::from)?;
    ensure_store_path_matches(store, project)?;
    ensure_persisted_manifest_matches(project, manifest)?;
    validate_identity(&request.identity)?;

    let activation = manifest
        .harnesses
        .get(&request.harness_id)
        .ok_or_else(|| ActivationError::new("harness has no activation record"))?;
    let current_version = adapter_version(&request.harness_id)
        .ok_or_else(|| ActivationError::new("unknown harness adapter"))?;
    let current_bridge_fingerprint = bridge_fingerprint(&request.harness_id, activation);
    invalidate_receipts_for_adapter(
        &project.memory_root,
        &request.harness_id,
        &activation.adapter_version,
        &current_bridge_fingerprint,
        &manifest.project_root_fingerprint,
        &manifest.store_id,
    )
    .map_err(ActivationError::from)?;
    if activation.adapter_version != current_version {
        return Err(ActivationError::new("stale adapter version"));
    }
    if activation.bridge_fingerprint != current_bridge_fingerprint {
        return Err(ActivationError::new("stale bridge fingerprint"));
    }
    if !matches!(
        activation.state,
        ActivationState::ConfiguredAwaitingProof
            | ActivationState::NeedsTrust
            | ActivationState::Active
            | ActivationState::ActiveIsolated
    ) {
        return Err(ActivationError::new(
            "harness is not eligible for preflight",
        ));
    }

    let state = preflight_state(project, manifest)?;
    let (query, query_class) = safe_query(request.task_hint.as_deref());
    let project_name = configured_identity_root(project)?
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty());
    let results = MemoryRetriever::new(store)
        .recall_with_options(
            query,
            &RecallOptions {
                project: project_name,
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
        )
        .map_err(|_| ActivationError::new("scoped recall failed"))?;
    let safe_results = safe_results(results);
    let context = render_safe_recall_context(&safe_results)?;
    let selected_memory_ids_sha256 = selected_memory_ids_digest(&safe_results);
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if duration_ms > PREFLIGHT_TIMEOUT_MS {
        return Err(ActivationError::new("preflight timeout"));
    }

    let receipt = ActivationReceipt {
        schema_version: ACTIVATION_SCHEMA_VERSION,
        protocol_version: ACTIVATION_PROTOCOL_VERSION,
        receipt_id: format!("receipt-{}", Uuid::new_v4()),
        harness_id: request.harness_id,
        adapter_version: activation.adapter_version.clone(),
        bridge_fingerprint: activation.bridge_fingerprint.clone(),
        store_id: manifest.store_id.clone(),
        project_root_fingerprint: manifest.project_root_fingerprint.clone(),
        worker_key_fingerprint: fingerprint(&request.identity.agent_profile),
        session: request.identity,
        state,
        query_class: query_class.to_string(),
        result_count: safe_results.len(),
        selected_memory_ids_sha256,
        duration_ms,
        status: "success".to_string(),
        recorded_at: Utc::now(),
    };
    let response = PreflightResponse {
        state,
        context,
        receipt: receipt_summary(&receipt),
    };

    // Force construction of the complete adapter payload before any receipt exists.
    render_for_format(&response, request.context_format)?;
    prune_receipts(
        &project.memory_root,
        &receipt.harness_id,
        &receipt.session.agent_profile,
        Utc::now(),
    )
    .map_err(ActivationError::from)?;
    write_receipt(&project.memory_root, &receipt).map_err(ActivationError::from)?;
    Ok(response)
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
    if harness_id == "codex" {
        return Err(ActivationError::new(
            "codex preflight requires direct identity flags",
        ));
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
    Ok(PreflightRequest {
        harness_id: harness_id.to_string(),
        identity,
        task_hint,
        context_format,
    })
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
    project: &ActivationProject,
    manifest: &ActivationManifest,
) -> Result<(), ActivationError> {
    let persisted = load_manifest(&project.memory_root)
        .map_err(|_| ActivationError::new("activation manifest is unavailable"))?;
    if persisted.schema_version != manifest.schema_version
        || persisted.protocol_version != manifest.protocol_version
        || persisted.store_id != manifest.store_id
        || persisted.project_root_fingerprint != manifest.project_root_fingerprint
        || persisted.harnesses != manifest.harnesses
    {
        return Err(ActivationError::new("activation manifest/store mismatch"));
    }
    Ok(())
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

#[cfg(test)]
pub(crate) fn project_fingerprint(project_root: &Path) -> String {
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
        PreflightRequest {
            harness_id: "pi".to_string(),
            identity: SessionIdentity {
                agent_profile: "pi".to_string(),
                workflow_id: "workflow-1".to_string(),
                session_id: "session-1".to_string(),
            },
            task_hint: Some("project startup constraints".to_string()),
            context_format: PreflightContextFormat::PiBeforeAgentStart,
        }
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
    fn stale_adapter_version_invalidates_prior_receipts_and_fails_closed() {
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
        let request = PreflightRequest {
            harness_id: "agent-zero".to_string(),
            identity: SessionIdentity {
                agent_profile: "agent-zero".to_string(),
                workflow_id: "workflow-1".to_string(),
                session_id: "session-1".to_string(),
            },
            task_hint: None,
            context_format: PreflightContextFormat::Json,
        };

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
