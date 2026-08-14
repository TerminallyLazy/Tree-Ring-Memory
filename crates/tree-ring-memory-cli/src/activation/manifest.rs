use super::{
    adapters::ActivationProject, bridge::ProjectFs, ActivationState, AdapterCapability,
    SessionIdentity, ACTIVATION_PROTOCOL_VERSION, ACTIVATION_SCHEMA_VERSION,
    RECEIPT_RETENTION_DAYS, RECEIPT_RETENTION_PER_WORKER,
};
use chrono::{DateTime, Duration, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};
use tree_ring_memory_core::SensitivityGuard;
use uuid::Uuid;

const ACTIVATION_MANIFEST_FILE: &str = "activation.json";
const RECEIPTS_DIRECTORY: &str = "activation/receipts";

/// The persisted activation configuration for one local Tree Ring store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivationManifest {
    pub schema_version: u16,
    pub protocol_version: u16,
    pub store_id: String,
    pub project_root_fingerprint: String,
    pub cli_version: String,
    pub harnesses: BTreeMap<String, HarnessActivation>,
}

/// Configuration owned by a harness. Bridge paths are project-relative only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessActivation {
    pub state: ActivationState,
    pub adapter_capability: AdapterCapability,
    #[serde(default = "default_adapter_version")]
    pub adapter_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bridge_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owned_files: Vec<OwnedBridgeFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub managed_blocks: Vec<OwnedManagedBlock>,
}

/// A complete project-local bridge file that Tree Ring may replace or remove
/// only while its bytes still match this digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedBridgeFile {
    pub path: String,
    pub sha256: String,
}

/// A bounded block inside an otherwise user-owned file. The block identifier
/// selects the exact adapter-owned markers or structured JSON handler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnedManagedBlock {
    pub path: String,
    pub block_id: String,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub leading_separator: String,
}

/// A deliberately minimal, non-sensitive record that an activation occurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivationReceipt {
    pub schema_version: u16,
    pub protocol_version: u16,
    pub receipt_id: String,
    pub harness_id: String,
    pub adapter_version: String,
    pub bridge_fingerprint: String,
    pub store_id: String,
    pub project_root_fingerprint: String,
    pub worker_key_fingerprint: String,
    pub session: SessionIdentity,
    pub state: ActivationState,
    pub query_class: String,
    pub result_count: usize,
    pub selected_memory_ids_sha256: String,
    pub duration_ms: u64,
    pub status: String,
    pub recorded_at: DateTime<Utc>,
}

fn default_adapter_version() -> String {
    "1".to_string()
}

/// Loads the persisted manifest without creating a directory or file.
pub fn load_manifest(memory_root: &Path) -> Result<ActivationManifest, String> {
    validate_memory_root(memory_root)?;
    let path = manifest_path(memory_root);
    let manifest: ActivationManifest = read_json(&path)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// Loads an existing manifest or creates the first manifest for a store.
pub fn load_or_create_manifest(
    memory_root: &Path,
    project_root: &Path,
    cli_version: &str,
) -> Result<ActivationManifest, String> {
    validate_memory_root(memory_root)?;
    validate_identifier("cli version", cli_version)?;
    let path = manifest_path(memory_root);
    if path.exists() {
        return load_manifest(memory_root);
    }

    let manifest = ActivationManifest {
        schema_version: ACTIVATION_SCHEMA_VERSION,
        protocol_version: ACTIVATION_PROTOCOL_VERSION,
        store_id: Uuid::new_v4().hyphenated().to_string(),
        project_root_fingerprint: fingerprint_path(project_root),
        cli_version: cli_version.to_owned(),
        harnesses: BTreeMap::new(),
    };
    validate_manifest(&manifest)?;
    match atomic_write_json(&path, &manifest, AtomicWriteMode::Create) {
        Ok(()) => Ok(manifest),
        Err(_) if path.exists() => load_manifest(memory_root),
        Err(error) => Err(error),
    }
}

/// Test-only replacement writer for fixtures that exercise changed persisted
/// contracts. Production activation persistence remains creation-only.
#[cfg(test)]
pub(crate) fn save_manifest(
    memory_root: &Path,
    manifest: &ActivationManifest,
) -> Result<(), String> {
    validate_memory_root(memory_root)?;
    validate_manifest(manifest)?;
    atomic_write_json(
        &manifest_path(memory_root),
        manifest,
        AtomicWriteMode::Replace,
    )
}

/// Writes a receipt beneath the store's activation receipt directory.
pub fn write_receipt(memory_root: &Path, receipt: &ActivationReceipt) -> Result<PathBuf, String> {
    validate_receipt(receipt)?;
    let project_fs = open_receipt_store(memory_root)?;
    let _manifest_lock = project_fs.lock_manifest()?;
    let relative = write_receipt_locked(&project_fs, receipt)?;
    let suffix = relative
        .strip_prefix(".tree-ring")
        .expect("receipt relative paths are rooted at .tree-ring");
    Ok(memory_root.join(suffix))
}

/// Removes expired receipts and keeps at most 100 receipts for a harness/worker pair.
pub fn prune_receipts(
    memory_root: &Path,
    harness_id: &str,
    worker_key: &str,
    now: DateTime<Utc>,
) -> Result<usize, String> {
    validate_identifier("harness id", harness_id)?;
    validate_identifier("worker key", worker_key)?;
    let project_fs = open_receipt_store(memory_root)?;
    let _manifest_lock = project_fs.lock_manifest()?;
    prune_receipts_locked(&project_fs, harness_id, worker_key, now)
}

/// Removes expired receipts while the caller holds the activation-contract
/// lock. All receipt traversal remains descriptor-relative to the verified
/// project root, so a replaced directory cannot redirect deletion.
pub(crate) fn prune_receipts_locked(
    project_fs: &ProjectFs,
    harness_id: &str,
    worker_key: &str,
    now: DateTime<Utc>,
) -> Result<usize, String> {
    validate_identifier("harness id", harness_id)?;
    validate_identifier("worker key", worker_key)?;
    let directory = receipt_relative_directory(harness_id, &fingerprint(worker_key));
    let Some(entries) = project_fs.directory_entries(&directory)? else {
        return Ok(0);
    };

    let expiry = now - Duration::days(RECEIPT_RETENTION_DAYS);
    let mut retained = Vec::new();
    let mut removed = 0;
    for name in entries {
        let path = directory.join(&name);
        if Path::new(&name)
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("json")
        {
            continue;
        }
        let receipt: ActivationReceipt =
            match read_receipt_locked(project_fs, &path).and_then(|receipt| {
                validate_receipt(&receipt)?;
                Ok(receipt)
            }) {
                Ok(receipt) => receipt,
                Err(_) => {
                    if project_fs.remove_validated_receipt_file(&path)? {
                        removed += 1;
                    }
                    continue;
                }
            };
        if receipt.recorded_at < expiry {
            if project_fs.remove_validated_receipt_file(&path)? {
                removed += 1;
            }
        } else {
            retained.push((receipt.recorded_at, path));
        }
    }
    retained.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
    for (_, path) in retained.into_iter().skip(RECEIPT_RETENTION_PER_WORKER) {
        if project_fs.remove_validated_receipt_file(&path)? {
            removed += 1;
        }
    }
    Ok(removed)
}

/// Removes receipts that no longer prove the current adapter/store contract.
pub fn invalidate_receipts_for_adapter(
    memory_root: &Path,
    harness_id: &str,
    adapter_version: &str,
    bridge_fingerprint: &str,
    project_root_fingerprint: &str,
    store_id: &str,
) -> Result<usize, String> {
    validate_identifier("harness id", harness_id)?;
    let project_fs = open_receipt_store(memory_root)?;
    let _manifest_lock = project_fs.lock_manifest()?;
    invalidate_receipts_for_adapter_locked(
        &project_fs,
        harness_id,
        adapter_version,
        bridge_fingerprint,
        project_root_fingerprint,
        store_id,
    )
}

/// Removes receipt evidence that does not match the exact adapter contract.
/// This is intentionally called under the same lock used by bridge mutation.
pub(crate) fn invalidate_receipts_for_adapter_locked(
    project_fs: &ProjectFs,
    harness_id: &str,
    adapter_version: &str,
    bridge_fingerprint: &str,
    project_root_fingerprint: &str,
    store_id: &str,
) -> Result<usize, String> {
    validate_identifier("harness id", harness_id)?;
    validate_identifier("adapter version", adapter_version)?;
    if !is_sha256(bridge_fingerprint) || !is_sha256(project_root_fingerprint) {
        return Err("receipt contract fingerprint is invalid".to_string());
    }
    validate_identifier("store id", store_id)?;
    let directory = receipt_harness_directory(harness_id);
    let Some(workers) = project_fs.directory_entries(&directory)? else {
        return Ok(0);
    };
    let mut removed = 0;
    for worker in workers {
        let worker_directory = directory.join(&worker);
        let Some(entries) = project_fs.directory_entries(&worker_directory)? else {
            continue;
        };
        for name in entries {
            let path = worker_directory.join(&name);
            if Path::new(&name)
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("json")
            {
                continue;
            }
            let current = read_receipt_locked(project_fs, &path)
                .and_then(|receipt| {
                    validate_receipt(&receipt)?;
                    Ok(receipt)
                })
                .is_ok_and(|receipt| {
                    receipt.adapter_version == adapter_version
                        && receipt.bridge_fingerprint == bridge_fingerprint
                        && receipt.project_root_fingerprint == project_root_fingerprint
                        && receipt.store_id == store_id
                });
            if !current && project_fs.remove_validated_receipt_file(&path)? {
                removed += 1;
            }
        }
    }
    Ok(removed)
}

/// Removes every receipt for a harness through the same no-follow traversal.
/// A manifest whose adapter record is itself stale cannot leave an otherwise
/// matching historical receipt as activation proof.
pub(crate) fn invalidate_all_receipts_for_adapter_locked(
    project_fs: &ProjectFs,
    harness_id: &str,
) -> Result<usize, String> {
    validate_identifier("harness id", harness_id)?;
    let directory = receipt_harness_directory(harness_id);
    let Some(workers) = project_fs.directory_entries(&directory)? else {
        return Ok(0);
    };
    let mut removed = 0;
    for worker in workers {
        let worker_directory = directory.join(&worker);
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
            if project_fs.remove_validated_receipt_file(&worker_directory.join(name))? {
                removed += 1;
            }
        }
    }
    Ok(removed)
}

/// Reads a persisted manifest through a descriptor-relative no-follow path.
/// Callers that need an activation decision should hold `ProjectFs::lock_manifest`.
pub(crate) fn load_manifest_locked(project_fs: &ProjectFs) -> Result<ActivationManifest, String> {
    let bytes = project_fs
        .read_optional(Path::new(".tree-ring/activation.json"))?
        .ok_or_else(|| "activation manifest is unavailable".to_string())?;
    let manifest: ActivationManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid activation JSON: {error}"))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// Writes a validated receipt using an O_NOFOLLOW, descriptor-relative target.
/// The caller must hold the activation-contract lock.
pub(crate) fn write_receipt_locked(
    project_fs: &ProjectFs,
    receipt: &ActivationReceipt,
) -> Result<PathBuf, String> {
    validate_receipt(receipt)?;
    let relative = receipt_relative_path(receipt)?;
    let bytes = serde_json::to_vec_pretty(receipt)
        .map_err(|error| format!("failed to serialize activation receipt: {error}"))?;
    project_fs.create_receipt_file(&relative, &bytes)?;
    Ok(relative)
}

fn read_receipt_locked(
    project_fs: &ProjectFs,
    relative: &Path,
) -> Result<ActivationReceipt, String> {
    let bytes = project_fs
        .read_optional(relative)?
        .ok_or_else(|| "activation receipt disappeared".to_string())?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid activation receipt JSON: {error}"))
}

fn open_receipt_store(memory_root: &Path) -> Result<ProjectFs, String> {
    validate_memory_root(memory_root)?;
    let project = ActivationProject::from_memory_root(memory_root.to_path_buf())?;
    let project_fs = ProjectFs::open(&project)?;
    if project_fs
        .directory_entries(Path::new(".tree-ring"))?
        .is_none()
    {
        return Err("activation metadata root is unavailable".to_string());
    }
    Ok(project_fs)
}

fn receipt_harness_directory(harness_id: &str) -> PathBuf {
    PathBuf::from(".tree-ring")
        .join(RECEIPTS_DIRECTORY)
        .join(harness_id)
}

fn receipt_relative_directory(harness_id: &str, worker_fingerprint: &str) -> PathBuf {
    receipt_harness_directory(harness_id).join(worker_fingerprint)
}

fn receipt_relative_path(receipt: &ActivationReceipt) -> Result<PathBuf, String> {
    validate_identifier("harness id", &receipt.harness_id)?;
    validate_identifier("receipt id", &receipt.receipt_id)?;
    if !is_sha256(&receipt.worker_key_fingerprint) {
        return Err("receipt worker key fingerprint is invalid".to_string());
    }
    Ok(
        receipt_relative_directory(&receipt.harness_id, &receipt.worker_key_fingerprint)
            .join(format!("{}.json", receipt.receipt_id)),
    )
}

/// Stable digest of only the project-relative, adapter-owned bridge contract.
pub fn bridge_fingerprint(harness_id: &str, activation: &HarnessActivation) -> String {
    let mut hasher = Sha256::new();
    hash_part(&mut hasher, harness_id);
    hash_part(&mut hasher, &activation.adapter_version);
    let mut files = activation.owned_files.clone();
    files.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.sha256.cmp(&right.sha256))
    });
    for owned in files {
        hash_part(&mut hasher, "file");
        hash_part(&mut hasher, &owned.path);
        hash_part(&mut hasher, &owned.sha256);
    }
    let mut blocks = activation.managed_blocks.clone();
    blocks.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.block_id.cmp(&right.block_id))
            .then(left.sha256.cmp(&right.sha256))
            .then(left.leading_separator.cmp(&right.leading_separator))
    });
    for owned in blocks {
        hash_part(&mut hasher, "block");
        hash_part(&mut hasher, &owned.path);
        hash_part(&mut hasher, &owned.block_id);
        hash_part(&mut hasher, &owned.sha256);
        hash_part(&mut hasher, &owned.leading_separator);
    }
    format!("{:x}", hasher.finalize())
}

fn hash_part(hasher: &mut Sha256, value: &str) {
    hasher.update(value.len().to_be_bytes());
    hasher.update(value.as_bytes());
}

#[cfg(test)]
pub(crate) fn receipt_files(memory_root: &Path) -> Vec<PathBuf> {
    let directory = memory_root.join(RECEIPTS_DIRECTORY);
    let mut files = Vec::new();
    let mut pending = vec![directory];
    while let Some(path) = pending.pop() {
        let Ok(entries) = fs::read_dir(path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn manifest_path(memory_root: &Path) -> PathBuf {
    memory_root.join(ACTIVATION_MANIFEST_FILE)
}

#[cfg(test)]
fn receipt_path(memory_root: &Path, harness_id: &str, worker_fingerprint: &str) -> PathBuf {
    memory_root
        .join(RECEIPTS_DIRECTORY)
        .join(harness_id)
        .join(worker_fingerprint)
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|err| io_error(path, err))?;
    serde_json::from_slice(&bytes)
        .map_err(|err| format!("invalid activation JSON at {}: {err}", path.display()))
}

#[derive(Clone, Copy)]
enum AtomicWriteMode {
    #[cfg(test)]
    Replace,
    Create,
}

fn atomic_write_json<T: Serialize>(
    path: &Path,
    value: &T,
    mode: AtomicWriteMode,
) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|err| format!("failed to serialize activation JSON: {err}"))?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("activation output path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|err| io_error(parent, err))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "activation output path has no UTF-8 file name: {}",
                path.display()
            )
        })?;
    let temp_path = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|err| io_error(&temp_path, err))?;
        file.write_all(&bytes)
            .map_err(|err| io_error(&temp_path, err))?;
        file.sync_all().map_err(|err| io_error(&temp_path, err))?;
        drop(file);
        match mode {
            #[cfg(test)]
            AtomicWriteMode::Replace => {
                fs::rename(&temp_path, path).map_err(|err| io_error(path, err))
            }
            AtomicWriteMode::Create => {
                fs::hard_link(&temp_path, path).map_err(|err| io_error(path, err))?;
                fs::remove_file(&temp_path).map_err(|err| io_error(&temp_path, err))
            }
        }
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn validate_memory_root(memory_root: &Path) -> Result<(), String> {
    if memory_root.file_name().and_then(|name| name.to_str()) == Some(".tree-ring") {
        Ok(())
    } else {
        Err(format!(
            "activation metadata root must be a project .tree-ring directory: {}",
            memory_root.display()
        ))
    }
}

pub(crate) fn validate_manifest(manifest: &ActivationManifest) -> Result<(), String> {
    if manifest.schema_version != ACTIVATION_SCHEMA_VERSION {
        return Err(format!(
            "unsupported activation schema version: {}",
            manifest.schema_version
        ));
    }
    if manifest.protocol_version != ACTIVATION_PROTOCOL_VERSION {
        return Err(format!(
            "unsupported activation protocol version: {}",
            manifest.protocol_version
        ));
    }
    validate_identifier("store id", &manifest.store_id)?;
    if !is_sha256(&manifest.project_root_fingerprint) {
        return Err("project root fingerprint must be a SHA-256 hex digest".to_string());
    }
    validate_identifier("cli version", &manifest.cli_version)?;
    for (harness_id, activation) in &manifest.harnesses {
        validate_identifier("harness id", harness_id)?;
        if let Some(bridge_path) = &activation.bridge_path {
            validate_project_relative_path(bridge_path)?;
            if !activation
                .owned_files
                .iter()
                .any(|owned| owned.path == *bridge_path)
                && !activation
                    .managed_blocks
                    .iter()
                    .any(|owned| owned.path == *bridge_path)
            {
                return Err(format!(
                    "bridge path is not recorded ownership for {harness_id}"
                ));
            }
        }
        for owned in &activation.owned_files {
            validate_project_relative_path(&owned.path)?;
            if !allowed_owned_file(harness_id, &owned.path) {
                return Err(format!(
                    "invalid complete-file ownership target for {harness_id}: {}",
                    owned.path
                ));
            }
            if !is_sha256(&owned.sha256) {
                return Err("owned bridge file digest must be a SHA-256 hex digest".to_string());
            }
        }
        for owned in &activation.managed_blocks {
            validate_project_relative_path(&owned.path)?;
            validate_identifier("managed block id", &owned.block_id)?;
            if !allowed_managed_block(harness_id, &owned.path, &owned.block_id) {
                return Err(format!(
                    "invalid managed-block ownership target for {harness_id}: {}#{}",
                    owned.path, owned.block_id
                ));
            }
            if !is_sha256(&owned.sha256) {
                return Err("managed block digest must be a SHA-256 hex digest".to_string());
            }
            if !matches!(owned.leading_separator.as_str(), "" | "\n" | "\n\n") {
                return Err("invalid managed block leading separator".to_string());
            }
        }
        validate_identifier("adapter version", &activation.adapter_version)?;
        if !activation.bridge_fingerprint.is_empty() && !is_sha256(&activation.bridge_fingerprint) {
            return Err("bridge fingerprint must be a SHA-256 hex digest".to_string());
        }
        reject_duplicate_ownership(harness_id, activation)?;
    }
    Ok(())
}

fn allowed_owned_file(harness_id: &str, path: &str) -> bool {
    match harness_id {
        "codex" => matches!(
            path,
            ".agents/skills/tree-ring-memory/SKILL.md" | "AGENTS.md"
        ),
        "claude-code" => matches!(
            path,
            ".claude/skills/tree-ring-memory/SKILL.md" | ".claude/settings.json"
        ),
        "pi" => matches!(
            path,
            ".agents/skills/tree-ring-memory/SKILL.md" | ".pi/extensions/tree-ring-memory.ts"
        ),
        "agent-zero" => path == ".tree-ring/activation/agent-zero.json",
        _ => false,
    }
}

fn allowed_managed_block(harness_id: &str, path: &str, block_id: &str) -> bool {
    matches!(
        (harness_id, path, block_id),
        ("codex", "AGENTS.md", "codex") | ("claude-code", ".claude/settings.json", "claude-code")
    )
}

fn reject_duplicate_ownership(
    harness_id: &str,
    activation: &HarnessActivation,
) -> Result<(), String> {
    let mut files = std::collections::BTreeSet::new();
    for owned in &activation.owned_files {
        if !files.insert(&owned.path) {
            return Err(format!(
                "duplicate complete-file ownership for {harness_id}"
            ));
        }
    }
    let mut blocks = std::collections::BTreeSet::new();
    for owned in &activation.managed_blocks {
        if !blocks.insert((&owned.path, &owned.block_id)) {
            return Err(format!(
                "duplicate managed-block ownership for {harness_id}"
            ));
        }
        if files.contains(&owned.path) {
            return Err(format!(
                "conflicting complete-file and managed-block ownership for {harness_id}"
            ));
        }
    }
    Ok(())
}

fn validate_receipt(receipt: &ActivationReceipt) -> Result<(), String> {
    if receipt.schema_version != ACTIVATION_SCHEMA_VERSION {
        return Err(format!(
            "unsupported receipt schema version: {}",
            receipt.schema_version
        ));
    }
    if receipt.protocol_version != ACTIVATION_PROTOCOL_VERSION {
        return Err(format!(
            "unsupported receipt protocol version: {}",
            receipt.protocol_version
        ));
    }
    validate_identifier("receipt id", &receipt.receipt_id)?;
    validate_identifier("harness id", &receipt.harness_id)?;
    validate_identifier("adapter version", &receipt.adapter_version)?;
    if !is_sha256(&receipt.bridge_fingerprint) {
        return Err("receipt bridge fingerprint must be a SHA-256 hex digest".to_string());
    }
    validate_identifier("store id", &receipt.store_id)?;
    if !is_sha256(&receipt.project_root_fingerprint) {
        return Err("receipt project root fingerprint must be a SHA-256 hex digest".to_string());
    }
    if !is_sha256(&receipt.worker_key_fingerprint) {
        return Err("worker key fingerprint must be a SHA-256 hex digest".to_string());
    }
    validate_receipt_identity("agent profile", &receipt.session.agent_profile)?;
    validate_receipt_identity("workflow id", &receipt.session.workflow_id)?;
    validate_receipt_identity("session id", &receipt.session.session_id)?;
    if !matches!(
        receipt.state,
        ActivationState::Active | ActivationState::ActiveIsolated
    ) {
        return Err("receipt state is not active".to_string());
    }
    if !matches!(
        receipt.query_class.as_str(),
        "task_hint" | "startup_fallback"
    ) {
        return Err("invalid receipt query class".to_string());
    }
    if receipt.result_count > 8 {
        return Err("receipt result count exceeds preflight limit".to_string());
    }
    if !is_sha256(&receipt.selected_memory_ids_sha256) {
        return Err("selected memory IDs digest must be a SHA-256 hex digest".to_string());
    }
    if receipt.status != "success" {
        return Err("invalid receipt status".to_string());
    }
    Ok(())
}

fn validate_receipt_identity(label: &str, value: &str) -> Result<(), String> {
    validate_identifier(label, value)?;
    if SensitivityGuard::default().inspect(value).sensitivity != "normal" {
        return Err(format!("sensitive {label}"));
    }
    Ok(())
}

fn validate_identifier(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.len() > 256
        || value.chars().any(char::is_control)
        || value.contains(['/', '\\'])
        || matches!(value, "." | "..")
    {
        return Err(format!("invalid {label}"));
    }
    if value.contains("trcap_v1_")
        || SensitivityGuard::default().inspect(value).sensitivity == "secret"
    {
        return Err(format!("unsafe {label}"));
    }
    Ok(())
}

pub(crate) fn validate_project_relative_path(value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err("invalid bridge path".to_string());
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("bridge path must be project-relative".to_string());
    }
    Ok(())
}

pub(crate) fn fingerprint_path(project_root: &Path) -> String {
    let path = fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    fingerprint(&path.to_string_lossy())
}

pub(crate) fn fingerprint(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn io_error(path: &Path, error: std::io::Error) -> String {
    format!("{}: {error}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::{
        fs,
        sync::{Arc, Barrier},
        thread,
    };

    fn fixture_receipt(recorded_at: DateTime<Utc>) -> ActivationReceipt {
        ActivationReceipt {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            protocol_version: ACTIVATION_PROTOCOL_VERSION,
            receipt_id: format!("receipt-{}", Uuid::new_v4()),
            harness_id: "codex".to_string(),
            adapter_version: "1".to_string(),
            bridge_fingerprint: "b".repeat(64),
            store_id: "store-test".to_string(),
            project_root_fingerprint: "a".repeat(64),
            worker_key_fingerprint: fingerprint("worker-1"),
            session: SessionIdentity {
                agent_profile: "implementer".to_string(),
                workflow_id: "activation".to_string(),
                session_id: "session-1".to_string(),
            },
            state: ActivationState::Active,
            query_class: "task_hint".to_string(),
            result_count: 1,
            selected_memory_ids_sha256: "c".repeat(64),
            duration_ms: 1,
            status: "success".to_string(),
            recorded_at,
        }
    }

    #[test]
    fn manifest_assigns_a_stable_store_id_and_fingerprints_the_project() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project/.tree-ring");
        let project = temp.path().join("project");
        let first = load_or_create_manifest(&root, &project, "0.14.0").unwrap();
        let second = load_or_create_manifest(&root, &project, "0.14.0").unwrap();
        assert_eq!(first.store_id, second.store_id);
        assert_eq!(
            first.project_root_fingerprint,
            second.project_root_fingerprint
        );
        assert!(!first
            .project_root_fingerprint
            .contains(project.to_str().unwrap()));
    }

    #[test]
    fn receipt_json_excludes_prompt_context_and_capability_material() {
        let receipt = fixture_receipt(Utc::now());
        let json = serde_json::to_string(&receipt).unwrap();
        assert!(!json.contains("user prompt"));
        assert!(!json.contains("recalled summary"));
        assert!(!json.contains("trcap_v1_"));
    }

    #[test]
    fn receipt_identity_rejects_every_non_normal_sensitivity_classification() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project/.tree-ring");
        for (field, value) in [
            ("agent_profile", "medical"),
            ("workflow_id", "bank account"),
            ("session_id", "lawsuit"),
            ("agent_profile", "passport"),
            ("workflow_id", "sk-proj-aaaaaaaaaaaaaaaaaaaa"),
        ] {
            let mut receipt = fixture_receipt(Utc::now());
            match field {
                "agent_profile" => receipt.session.agent_profile = value.to_string(),
                "workflow_id" => receipt.session.workflow_id = value.to_string(),
                "session_id" => receipt.session.session_id = value.to_string(),
                _ => unreachable!(),
            }
            assert!(write_receipt(&root, &receipt).is_err(), "{field}: {value}");
        }
    }

    #[test]
    fn malformed_manifest_fails_without_replacing_the_file() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project/.tree-ring");
        fs::create_dir_all(&root).unwrap();
        let path = manifest_path(&root);
        fs::write(&path, b"not json").unwrap();
        assert!(load_or_create_manifest(&root, temp.path(), "0.14.0").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"not json");
    }

    #[test]
    fn loading_a_missing_manifest_does_not_create_store_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project/.tree-ring");
        assert!(load_manifest(&root).is_err());
        assert!(!root.exists());
    }

    #[test]
    fn concurrent_first_create_returns_the_persisted_manifest_identity() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project/.tree-ring");
        let project = temp.path().join("project");
        let barrier = Arc::new(Barrier::new(8));
        let mut workers = Vec::new();
        for _ in 0..8 {
            let root = root.clone();
            let project = project.clone();
            let barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(move || {
                barrier.wait();
                load_or_create_manifest(&root, &project, "0.14.0").unwrap()
            }));
        }
        let manifests = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        let persisted = load_manifest(&root).unwrap();
        assert!(manifests.iter().all(|manifest| manifest == &persisted));
    }

    #[test]
    fn atomic_overwrite_replaces_complete_json_without_temp_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project/.tree-ring");
        let project = temp.path().join("project");
        let manifest = load_or_create_manifest(&root, &project, "0.14.0").unwrap();
        let path = manifest_path(&root);
        atomic_write_json(&path, &manifest, AtomicWriteMode::Replace).unwrap();
        assert_eq!(load_manifest(&root).unwrap(), manifest);
        assert!(fs::read_dir(&root).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
    }

    #[test]
    fn preexisting_activation_records_default_version_and_require_new_bridge_proof() {
        let json = serde_json::json!({
            "schema_version": ACTIVATION_SCHEMA_VERSION,
            "protocol_version": ACTIVATION_PROTOCOL_VERSION,
            "store_id": "store-test",
            "project_root_fingerprint": "a".repeat(64),
            "cli_version": "0.14.0",
            "harnesses": {
                "codex": {
                    "state": "configured-awaiting-proof",
                    "adapter_capability": "wrapper-preflight",
                    "bridge_path": null,
                    "owned_files": [],
                    "managed_blocks": []
                }
            }
        });

        let manifest: ActivationManifest = serde_json::from_value(json).unwrap();
        let activation = manifest.harnesses.get("codex").unwrap();

        assert_eq!(activation.adapter_version, "1");
        assert!(activation.bridge_fingerprint.is_empty());
        assert!(validate_manifest(&manifest).is_ok());
    }

    #[test]
    fn bridge_fingerprint_is_order_independent_and_version_bound() {
        let mut activation = HarnessActivation {
            state: ActivationState::ConfiguredAwaitingProof,
            adapter_capability: AdapterCapability::NativePreflight,
            adapter_version: "1".to_string(),
            bridge_fingerprint: String::new(),
            bridge_path: Some(".pi/extensions/tree-ring-memory.ts".to_string()),
            owned_files: vec![
                OwnedBridgeFile {
                    path: ".pi/extensions/tree-ring-memory.ts".to_string(),
                    sha256: "a".repeat(64),
                },
                OwnedBridgeFile {
                    path: ".agents/skills/tree-ring-memory/SKILL.md".to_string(),
                    sha256: "b".repeat(64),
                },
            ],
            managed_blocks: Vec::new(),
        };
        let first = bridge_fingerprint("pi", &activation);
        activation.owned_files.reverse();
        assert_eq!(bridge_fingerprint("pi", &activation), first);
        activation.adapter_version = "2".to_string();
        assert_ne!(bridge_fingerprint("pi", &activation), first);
    }

    #[test]
    fn receipt_invalidation_removes_only_stale_adapter_contracts() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project/.tree-ring");
        fs::create_dir_all(&root).unwrap();
        let receipt = fixture_receipt(Utc::now());
        write_receipt(&root, &receipt).unwrap();
        assert_eq!(receipt_files(&root).len(), 1);
        assert_eq!(
            invalidate_receipts_for_adapter(
                &root,
                "codex",
                &receipt.adapter_version,
                &receipt.bridge_fingerprint,
                &receipt.project_root_fingerprint,
                &receipt.store_id,
            )
            .unwrap(),
            0
        );
        assert_eq!(receipt_files(&root).len(), 1);
        assert_eq!(
            invalidate_receipts_for_adapter(
                &root,
                "codex",
                "2",
                &"d".repeat(64),
                &receipt.project_root_fingerprint,
                &receipt.store_id,
            )
            .unwrap(),
            1
        );
        assert!(receipt_files(&root).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn receipt_operations_reject_symlinked_directory_components_without_escape() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project/.tree-ring");
        fs::create_dir_all(&root).unwrap();
        let outside = temp.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        let sentinel = outside.join("sentinel.json");
        fs::write(&sentinel, b"outside sentinel").unwrap();
        symlink(&outside, root.join("activation")).unwrap();
        let receipt = fixture_receipt(Utc::now());

        assert!(write_receipt(&root, &receipt).is_err());
        assert!(prune_receipts(&root, "codex", "worker-1", Utc::now()).is_err());
        assert!(invalidate_receipts_for_adapter(
            &root,
            "codex",
            &receipt.adapter_version,
            &receipt.bridge_fingerprint,
            &receipt.project_root_fingerprint,
            &receipt.store_id,
        )
        .is_err());
        assert_eq!(fs::read(&sentinel).unwrap(), b"outside sentinel");
        assert!(!outside.join("receipts").exists());
    }

    #[cfg(unix)]
    #[test]
    fn receipt_file_symlink_is_never_read_or_deleted() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project/.tree-ring");
        fs::create_dir_all(&root).unwrap();
        let receipt = fixture_receipt(Utc::now());
        let directory = receipt_path(&root, "codex", &fingerprint("worker-1"));
        fs::create_dir_all(&directory).unwrap();
        let sentinel = temp.path().join("outside-receipt.json");
        fs::write(&sentinel, b"outside receipt").unwrap();
        symlink(&sentinel, directory.join("symlink.json")).unwrap();

        assert!(prune_receipts(&root, "codex", "worker-1", Utc::now()).is_err());
        assert!(invalidate_receipts_for_adapter(
            &root,
            "codex",
            &receipt.adapter_version,
            &receipt.bridge_fingerprint,
            &receipt.project_root_fingerprint,
            &receipt.store_id,
        )
        .is_err());
        assert_eq!(fs::read(&sentinel).unwrap(), b"outside receipt");
        assert!(directory.join("symlink.json").exists());
    }

    #[test]
    fn prune_receipts_removes_malformed_and_expired_records_and_keeps_the_latest_hundred() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project/.tree-ring");
        fs::create_dir_all(&root).unwrap();
        let now = Utc::now();
        let mut expired = fixture_receipt(now - Duration::days(RECEIPT_RETENTION_DAYS + 1));
        expired.receipt_id = "expired".to_string();
        write_receipt(&root, &expired).unwrap();
        let directory = receipt_path(&root, "codex", &fingerprint("worker-1"));
        fs::write(directory.join("malformed.json"), b"not a receipt").unwrap();
        for offset in 0..101 {
            let mut receipt = fixture_receipt(now - Duration::seconds(offset));
            receipt.receipt_id = format!("receipt-{offset}");
            write_receipt(&root, &receipt).unwrap();
        }
        assert_eq!(prune_receipts(&root, "codex", "worker-1", now).unwrap(), 3);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 100);
        assert!(!directory.join("malformed.json").exists());
    }

    #[test]
    fn rejects_unsafe_identifiers_and_absolute_bridge_paths() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project/.tree-ring");
        let mut receipt = fixture_receipt(Utc::now());
        receipt.harness_id = "\n".to_string();
        assert!(write_receipt(&root, &receipt).is_err());
        receipt.harness_id = "codex".to_string();
        receipt.session.session_id = format!("trcap_v1_{}", "a".repeat(64));
        assert!(write_receipt(&root, &receipt).is_err());

        let mut manifest = load_or_create_manifest(&root, temp.path(), "0.14.0").unwrap();
        manifest.harnesses.insert(
            "codex".to_string(),
            HarnessActivation {
                state: ActivationState::Active,
                adapter_capability: AdapterCapability::NativePreflight,
                adapter_version: "1".to_string(),
                bridge_fingerprint: bridge_fingerprint(
                    "codex",
                    &HarnessActivation {
                        state: ActivationState::Active,
                        adapter_capability: AdapterCapability::NativePreflight,
                        adapter_version: "1".to_string(),
                        bridge_fingerprint: String::new(),
                        bridge_path: None,
                        owned_files: Vec::new(),
                        managed_blocks: Vec::new(),
                    },
                ),
                bridge_path: None,
                owned_files: Vec::new(),
                managed_blocks: Vec::new(),
            },
        );
        assert!(validate_manifest(&manifest).is_ok());
        manifest.harnesses.get_mut("codex").unwrap().bridge_path =
            Some(temp.path().display().to_string());
        assert!(validate_manifest(&manifest).is_err());
    }
}
