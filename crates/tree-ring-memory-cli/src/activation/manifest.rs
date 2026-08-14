use super::{
    ActivationState, AdapterCapability, SessionIdentity, ACTIVATION_PROTOCOL_VERSION,
    ACTIVATION_SCHEMA_VERSION, RECEIPT_RETENTION_DAYS, RECEIPT_RETENTION_PER_WORKER,
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
}

/// A deliberately minimal, non-sensitive record that an activation occurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivationReceipt {
    pub schema_version: u16,
    pub protocol_version: u16,
    pub receipt_id: String,
    pub harness_id: String,
    pub worker_key_fingerprint: String,
    pub session: SessionIdentity,
    pub state: ActivationState,
    pub recorded_at: DateTime<Utc>,
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

/// Atomically persists a validated activation manifest.
pub fn save_manifest(memory_root: &Path, manifest: &ActivationManifest) -> Result<(), String> {
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
    validate_memory_root(memory_root)?;
    validate_receipt(receipt)?;
    let path = receipt_path(
        memory_root,
        &receipt.harness_id,
        &receipt.worker_key_fingerprint,
    )
    .join(format!("{}.json", receipt.receipt_id));
    atomic_write_json(&path, receipt, AtomicWriteMode::Replace)?;
    Ok(path)
}

/// Removes expired receipts and keeps at most 100 receipts for a harness/worker pair.
pub fn prune_receipts(
    memory_root: &Path,
    harness_id: &str,
    worker_key: &str,
    now: DateTime<Utc>,
) -> Result<usize, String> {
    validate_memory_root(memory_root)?;
    validate_identifier("harness id", harness_id)?;
    validate_identifier("worker key", worker_key)?;
    let directory = receipt_path(memory_root, harness_id, &fingerprint(worker_key));
    if !directory.exists() {
        return Ok(0);
    }

    let expiry = now - Duration::days(RECEIPT_RETENTION_DAYS);
    let mut retained = Vec::new();
    let mut removed = 0;
    for entry in fs::read_dir(&directory).map_err(|err| io_error(&directory, err))? {
        let entry = entry.map_err(|err| io_error(&directory, err))?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let receipt: ActivationReceipt = match read_json(&path).and_then(|receipt| {
            validate_receipt(&receipt)?;
            Ok(receipt)
        }) {
            Ok(receipt) => receipt,
            Err(_) => {
                fs::remove_file(&path).map_err(|err| io_error(&path, err))?;
                removed += 1;
                continue;
            }
        };
        if receipt.recorded_at < expiry {
            fs::remove_file(&path).map_err(|err| io_error(&path, err))?;
            removed += 1;
        } else {
            retained.push((receipt.recorded_at, path));
        }
    }
    retained.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
    for (_, path) in retained.into_iter().skip(RECEIPT_RETENTION_PER_WORKER) {
        fs::remove_file(&path).map_err(|err| io_error(&path, err))?;
        removed += 1;
    }
    Ok(removed)
}

fn manifest_path(memory_root: &Path) -> PathBuf {
    memory_root.join(ACTIVATION_MANIFEST_FILE)
}

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
        }
        for owned in &activation.owned_files {
            validate_project_relative_path(&owned.path)?;
            if !is_sha256(&owned.sha256) {
                return Err("owned bridge file digest must be a SHA-256 hex digest".to_string());
            }
        }
        for owned in &activation.managed_blocks {
            validate_project_relative_path(&owned.path)?;
            validate_identifier("managed block id", &owned.block_id)?;
            if !is_sha256(&owned.sha256) {
                return Err("managed block digest must be a SHA-256 hex digest".to_string());
            }
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
    if !is_sha256(&receipt.worker_key_fingerprint) {
        return Err("worker key fingerprint must be a SHA-256 hex digest".to_string());
    }
    validate_receipt_identity("agent profile", &receipt.session.agent_profile)?;
    validate_receipt_identity("workflow id", &receipt.session.workflow_id)?;
    validate_receipt_identity("session id", &receipt.session.session_id)
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

fn fingerprint_path(project_root: &Path) -> String {
    let path = fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    fingerprint(&path.to_string_lossy())
}

fn fingerprint(value: &str) -> String {
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
            worker_key_fingerprint: fingerprint("worker-1"),
            session: SessionIdentity {
                agent_profile: "implementer".to_string(),
                workflow_id: "activation".to_string(),
                session_id: "session-1".to_string(),
            },
            state: ActivationState::Active,
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
    fn prune_receipts_removes_malformed_and_expired_records_and_keeps_the_latest_hundred() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("project/.tree-ring");
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
                bridge_path: Some("bridges/codex.sh".to_string()),
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
