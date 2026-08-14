use super::{
    adapters::{
        adapter_capability, adapter_version, ActivationProject, AdapterPlan, ManagedBlockUpdate,
        PlannedWrite,
    },
    manifest::{
        bridge_fingerprint, fingerprint_path, validate_manifest, validate_project_relative_path,
        ActivationManifest, HarnessActivation, OwnedBridgeFile, OwnedManagedBlock,
    },
    ActivationState, AdapterCapability, ACTIVATION_PROTOCOL_VERSION,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

#[cfg(test)]
use std::cell::RefCell;

#[cfg(not(unix))]
use std::ffi::OsString;
#[cfg(unix)]
use std::{
    ffi::{CStr, CString, OsString},
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::{
            ffi::{OsStrExt, OsStringExt},
            fs::MetadataExt,
        },
    },
    path::Component,
};
#[cfg(unix)]
use uuid::Uuid;

const CODEX_RETRY: &str = "tree-ring integrations activate --harness codex --accept-managed-block";
const CLAUDE_DESCRIPTION: &str = "Tree Ring Memory managed preflight v1";
const CLAUDE_COMMAND: &str = "tree-ring --root .tree-ring integrations preflight --harness claude-code --input-json-stdin --context-format claude-session-start";

const SKILL_BRIDGE: &str = r#"---
name: tree-ring-memory
description: Use the canonical project-local Tree Ring Memory guidance and preflight contract.
---

# Tree Ring Memory

Read `.tree-ring/SKILL.md`, `.tree-ring/AGENTS.md`, and `.tree-ring/CLI.md` from this project. Before substantive work, use the project-local Tree Ring preflight interface for this harness. Treat configured guidance as non-active until a receipt-producing preflight succeeds.
"#;

const CODEX_BLOCK_BODY: &str = r#"## Tree Ring Memory

Read `.tree-ring/AGENTS.md`, `.tree-ring/SKILL.md`, and `.tree-ring/CLI.md` before substantive work. Use `tree-ring --root .tree-ring integrations preflight --harness codex` with the later preflight identity interface, and do not claim memory is active without a valid project-local recall receipt.
"#;

const PI_EXTENSION: &str = r#"import { spawn } from "node:child_process";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

type PreflightResponse = { context: string };

function invokePreflight(cwd: string, input: string): Promise<PreflightResponse> {
  return new Promise((resolve, reject) => {
    const child = spawn(
      "tree-ring",
      [
        "--root",
        ".tree-ring",
        "integrations",
        "preflight",
        "--harness",
        "pi",
        "--input-json-stdin",
        "--context-format",
        "pi-before-agent-start",
      ],
      { cwd, stdio: ["pipe", "pipe", "pipe"] },
    );
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0) {
        reject(new Error(stderr || `Tree Ring preflight exited with status ${code}`));
        return;
      }
      try {
        const response = JSON.parse(stdout) as PreflightResponse;
        if (typeof response.context !== "string") throw new Error("missing context");
        resolve(response);
      } catch (error) {
        reject(new Error(`Invalid Tree Ring preflight response: ${String(error)}`));
      }
    });
    child.stdin.end(input);
  });
}

function sensitivityRejected(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return message.includes("sensitive-task-hint");
}

async function runPreflightWithStdin(cwd: string, input: string): Promise<PreflightResponse> {
  try {
    return await invokePreflight(cwd, input);
  } catch (error) {
    if (!sensitivityRejected(error)) throw error;
    const safeInput = {
      ...(JSON.parse(input) as Record<string, unknown>),
      task_hint: "project startup constraints",
    };
    return invokePreflight(cwd, JSON.stringify(safeInput));
  }
}

export default function treeRingMemory(pi: ExtensionAPI) {
  pi.on("before_agent_start", async (event, ctx) => {
    const input = JSON.stringify({
      agent_profile: "pi",
      workflow_id: ctx.sessionManager.getSessionFile() ?? "pi-ephemeral",
      session_id: ctx.sessionManager.getSessionFile() ?? "pi-ephemeral",
      task_hint: event.prompt,
    });
    const response = await runPreflightWithStdin(ctx.cwd, input);
    return { message: { customType: "tree-ring-preflight", content: response.context, display: false } };
  });
}
"#;

#[cfg(test)]
#[derive(Default)]
struct TestMutationHooks {
    after_final_root_binding: Option<Box<dyn FnOnce()>>,
    after_final_target_validation: Option<Box<dyn FnOnce()>>,
    fail_directory_sync_at: Vec<usize>,
    directory_syncs: usize,
    fail_fdopendir: bool,
    fail_readdir: bool,
    fail_closedir: bool,
}

#[cfg(test)]
thread_local! {
    static TEST_MUTATION_HOOKS: RefCell<TestMutationHooks> = RefCell::new(TestMutationHooks::default());
}

#[cfg(test)]
fn after_final_root_binding() {
    let action =
        TEST_MUTATION_HOOKS.with(|hooks| hooks.borrow_mut().after_final_root_binding.take());
    if let Some(action) = action {
        action();
    }
}

#[cfg(not(test))]
fn after_final_root_binding() {}

#[cfg(test)]
fn after_final_target_validation() {
    let action =
        TEST_MUTATION_HOOKS.with(|hooks| hooks.borrow_mut().after_final_target_validation.take());
    if let Some(action) = action {
        action();
    }
}

#[cfg(not(test))]
fn after_final_target_validation() {}

#[cfg(test)]
fn fail_directory_sync_now() -> bool {
    TEST_MUTATION_HOOKS.with(|hooks| {
        let mut hooks = hooks.borrow_mut();
        hooks.directory_syncs += 1;
        if let Some(position) = hooks
            .fail_directory_sync_at
            .iter()
            .position(|number| *number == hooks.directory_syncs)
        {
            hooks.fail_directory_sync_at.remove(position);
            true
        } else {
            false
        }
    })
}

#[cfg(not(test))]
fn fail_directory_sync_now() -> bool {
    false
}

#[cfg(all(unix, test))]
fn fdopendir_for_listing(descriptor: libc::c_int) -> *mut libc::DIR {
    let fail = TEST_MUTATION_HOOKS.with(|hooks| {
        let mut hooks = hooks.borrow_mut();
        if hooks.fail_fdopendir {
            hooks.fail_fdopendir = false;
            true
        } else {
            false
        }
    });
    if fail {
        set_errno(libc::EIO);
        std::ptr::null_mut()
    } else {
        unsafe {
            // SAFETY: the caller transfers ownership of a valid duplicated descriptor.
            libc::fdopendir(descriptor)
        }
    }
}

#[cfg(all(unix, not(test)))]
fn fdopendir_for_listing(descriptor: libc::c_int) -> *mut libc::DIR {
    unsafe {
        // SAFETY: the caller transfers ownership of a valid duplicated descriptor.
        libc::fdopendir(descriptor)
    }
}

#[cfg(all(unix, test))]
fn readdir_for_listing(stream: *mut libc::DIR) -> *mut libc::dirent {
    let fail = TEST_MUTATION_HOOKS.with(|hooks| {
        let mut hooks = hooks.borrow_mut();
        let fail = hooks.fail_readdir;
        hooks.fail_readdir = false;
        fail
    });
    if fail {
        set_errno(libc::EIO);
        std::ptr::null_mut()
    } else {
        unsafe {
            // SAFETY: the caller keeps the directory stream open for this call.
            libc::readdir(stream)
        }
    }
}

#[cfg(all(unix, not(test)))]
fn readdir_for_listing(stream: *mut libc::DIR) -> *mut libc::dirent {
    unsafe {
        // SAFETY: the caller keeps the directory stream open for this call.
        libc::readdir(stream)
    }
}

#[cfg(all(unix, test))]
fn closedir_for_listing(stream: *mut libc::DIR) -> libc::c_int {
    let status = unsafe {
        // SAFETY: the caller transfers ownership of its open directory stream.
        libc::closedir(stream)
    };
    let fail = TEST_MUTATION_HOOKS.with(|hooks| {
        let mut hooks = hooks.borrow_mut();
        let fail = hooks.fail_closedir;
        hooks.fail_closedir = false;
        fail
    });
    if status == 0 && fail {
        set_errno(libc::EIO);
        -1
    } else {
        status
    }
}

#[cfg(all(unix, not(test)))]
fn closedir_for_listing(stream: *mut libc::DIR) -> libc::c_int {
    unsafe {
        // SAFETY: the caller transfers ownership of its open directory stream.
        libc::closedir(stream)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgePlanResult {
    pub state: ActivationState,
    pub changed_paths: Vec<PathBuf>,
    pub next_step: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeBatchResult {
    pub harness_id: String,
    pub result: BridgePlanResult,
}

#[derive(Debug, Clone)]
struct PreparedFile {
    relative: PathBuf,
    before: Option<Vec<u8>>,
    after: Option<Vec<u8>>,
}

#[derive(Debug)]
struct AppliedFile {
    target: ResolvedTarget,
    publication: FileIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[derive(Debug)]
struct TargetSnapshot {
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct MutationError {
    message: String,
    publication: Option<FileIdentity>,
}

impl MutationError {
    fn before_publication(message: String) -> Self {
        Self {
            message,
            publication: None,
        }
    }

    fn after_publication(message: String, publication: FileIdentity) -> Self {
        Self {
            message,
            publication: Some(publication),
        }
    }
}

#[derive(Debug)]
struct CommitPreparedError {
    message: String,
    applied: Option<Box<AppliedFile>>,
}

impl CommitPreparedError {
    fn before_publication(message: String) -> Self {
        Self {
            message,
            applied: None,
        }
    }

    fn after_publication(message: String, applied: AppliedFile) -> Self {
        Self {
            message,
            applied: Some(Box::new(applied)),
        }
    }
}

#[cfg(unix)]
#[derive(Debug)]
pub(crate) struct ProjectFs {
    root: File,
    project_root: PathBuf,
    root_identity: FileIdentity,
}

#[cfg(unix)]
#[derive(Debug)]
pub(crate) struct ResolvedTarget {
    parent: File,
    name: CString,
    display: PathBuf,
}

#[cfg(unix)]
#[derive(Debug)]
pub(crate) struct ManifestLock {
    file: File,
}

#[cfg(unix)]
impl Drop for ManifestLock {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: the lock file descriptor is owned and remains open for this guard's life.
            libc::flock(self.file.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

#[cfg(unix)]
impl ProjectFs {
    pub(crate) fn open(project: &ActivationProject) -> Result<Self, String> {
        validate_project_shape(project)?;
        let root = open_project_root_no_follow(&project.project_root)?;
        let metadata = root
            .metadata()
            .map_err(|error| io_error(&project.project_root, error))?;
        if !metadata.is_dir() {
            return Err("project root must be a real directory, not a symlink".to_string());
        }
        let project_fs = Self {
            root,
            project_root: project.project_root.clone(),
            root_identity: file_identity(&metadata),
        };
        Ok(project_fs)
    }

    pub(crate) fn lock_manifest(&self) -> Result<ManifestLock, String> {
        self.ensure_root_binding()?;
        let file = self
            .root
            .try_clone()
            .map_err(|error| io_error(Path::new("."), error))?;
        let result = unsafe {
            // SAFETY: `file` owns the stable project-root directory descriptor and remains alive
            // for the returned guard, so path replacement cannot split concurrent writers.
            libc::flock(file.as_raw_fd(), libc::LOCK_EX)
        };
        if result != 0 {
            return Err(io_error(Path::new("."), std::io::Error::last_os_error()));
        }
        let manifest_lock = ManifestLock { file };
        self.ensure_root_binding()?;
        Ok(manifest_lock)
    }

    fn same_root_identity(&self, other: &Self) -> Result<bool, String> {
        self.ensure_root_binding()?;
        other.ensure_root_binding()?;
        Ok(self.root_identity == other.root_identity)
    }

    pub(crate) fn read_optional(&self, relative: &Path) -> Result<Option<Vec<u8>>, String> {
        self.ensure_root_binding()?;
        let Some(target) = self.resolve_target_optional(relative, false)? else {
            return Ok(None);
        };
        let bytes = target.read_optional()?;
        self.ensure_root_binding()?;
        Ok(bytes)
    }

    fn ensure_root_binding(&self) -> Result<(), String> {
        let current = open_project_root_no_follow(&self.project_root)
            .map_err(|_| "activation project root changed during bridge update".to_string())?;
        let metadata = current
            .metadata()
            .map_err(|error| io_error(&self.project_root, error))?;
        if !metadata.is_dir() || file_identity(&metadata) != self.root_identity {
            return Err("activation project root changed during bridge update".to_string());
        }
        Ok(())
    }

    fn resolve_target(
        &self,
        relative: &Path,
        create_parents: bool,
    ) -> Result<ResolvedTarget, String> {
        self.ensure_root_binding()?;
        self.resolve_target_optional(relative, create_parents)?
            .ok_or_else(|| format!("bridge target parent is missing: {}", relative.display()))
    }

    /// Creates a new receipt-like file below the pinned project root. Existing
    /// final entries are never replaced, and post-publication uncertainty is
    /// reported while leaving the published material in place.
    pub(crate) fn create_receipt_file(&self, relative: &Path, bytes: &[u8]) -> Result<(), String> {
        validate_receipt_target(relative)?;
        let target = self.resolve_target(relative, true)?;
        if target.read_optional()?.is_some() {
            return Err(format!(
                "existing target cannot be safely replaced: {}",
                relative.display()
            ));
        }
        self.ensure_root_binding()?;
        let publication = target
            .publish_creation(bytes)
            .map_err(|error| error.message)?;
        if let Err(error) = target.sync_parent() {
            return Err(MutationError::after_publication(error, publication).message);
        }
        self.ensure_root_binding()
    }

    /// Creates one private launcher context at the fixed activation runtime
    /// location. The receipt id determines the name; callers cannot select an
    /// arbitrary filesystem target or replace an existing entry.
    pub(crate) fn create_runtime_context_file(
        &self,
        receipt_id: &str,
        bytes: &[u8],
    ) -> Result<PathBuf, String> {
        let relative = runtime_context_target(receipt_id)?;
        let target = self.resolve_target(&relative, true)?;
        if target.read_optional()?.is_some() {
            return Err("existing runtime context requires user review".to_string());
        }
        self.ensure_root_binding()?;
        let publication = target
            .publish_creation(bytes)
            .map_err(|error| error.message)?;
        if let Err(error) = target.sync_parent() {
            return Err(MutationError::after_publication(error, publication).message);
        }
        self.ensure_root_binding()?;
        Ok(relative)
    }

    fn resolve_target_optional(
        &self,
        relative: &Path,
        create_parents: bool,
    ) -> Result<Option<ResolvedTarget>, String> {
        validate_relative_path_buf(relative)?;
        let mut components = relative.components().peekable();
        let mut directory = self
            .root
            .try_clone()
            .map_err(|error| io_error(relative, error))?;
        while let Some(component) = components.next() {
            let Component::Normal(segment) = component else {
                return Err("bridge path must be normalized and project-relative".to_string());
            };
            if components.peek().is_none() {
                return Ok(Some(ResolvedTarget {
                    parent: directory,
                    name: component_c_string(segment)?,
                    display: relative.to_path_buf(),
                }));
            }
            match open_child_directory(&directory, segment) {
                Ok(next) => directory = next,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create_parents => {
                    return Ok(None);
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    create_child_directory(&directory, segment)?;
                    directory = open_child_directory(&directory, segment)
                        .map_err(|error| io_error(relative, error))?;
                }
                Err(error) => {
                    return Err(format!(
                        "bridge path parent is a symlink or non-directory at {}: {error}",
                        relative.display()
                    ));
                }
            }
        }
        Err("bridge path must include a file name".to_string())
    }

    /// Lists direct children through a retained directory descriptor. Every
    /// component is opened with `O_NOFOLLOW`; a missing directory is distinct
    /// from a symlink or non-directory failure.
    pub(crate) fn directory_entries(
        &self,
        relative: &Path,
    ) -> Result<Option<Vec<OsString>>, String> {
        self.ensure_root_binding()?;
        let Some(directory) = self.resolve_directory_optional(relative)? else {
            return Ok(None);
        };
        let entries = list_directory_entries(&directory)?;
        self.ensure_root_binding()?;
        Ok(Some(entries))
    }

    fn resolve_directory_optional(&self, relative: &Path) -> Result<Option<File>, String> {
        validate_relative_path_buf(relative)?;
        let mut directory = self
            .root
            .try_clone()
            .map_err(|error| io_error(relative, error))?;
        for component in relative.components() {
            let Component::Normal(segment) = component else {
                return Err("bridge path must be normalized and project-relative".to_string());
            };
            match open_child_directory(&directory, segment) {
                Ok(next) => directory = next,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => {
                    return Err(format!(
                        "bridge path parent is a symlink or non-directory at {}: {error}",
                        relative.display()
                    ));
                }
            }
        }
        Ok(Some(directory))
    }

    /// Deletes only a file that was re-opened as a regular no-follow target.
    pub(crate) fn remove_validated_receipt_file(&self, relative: &Path) -> Result<bool, String> {
        validate_receipt_target(relative)?;
        self.ensure_root_binding()?;
        let Some(target) = self.resolve_target_optional(relative, false)? else {
            return Ok(false);
        };
        if target.snapshot_optional()?.is_none() {
            return Ok(false);
        }
        self.ensure_root_binding()?;
        let removed = target.remove_validated_regular_file()?;
        self.ensure_root_binding()?;
        Ok(removed)
    }

    /// Removes only the fixed private runtime file for this receipt id.
    pub(crate) fn remove_runtime_context_file(&self, receipt_id: &str) -> Result<bool, String> {
        let relative = runtime_context_target(receipt_id)?;
        self.ensure_root_binding()?;
        let Some(target) = self.resolve_target_optional(&relative, false)? else {
            return Ok(false);
        };
        if target.snapshot_optional()?.is_none() {
            return Ok(false);
        }
        self.ensure_root_binding()?;
        let removed = target.remove_validated_regular_file()?;
        self.ensure_root_binding()?;
        Ok(removed)
    }
}

#[cfg(unix)]
impl ResolvedTarget {
    pub(crate) fn read_optional(&self) -> Result<Option<Vec<u8>>, String> {
        Ok(self.snapshot_optional()?.map(|snapshot| snapshot.bytes))
    }

    fn snapshot_optional(&self) -> Result<Option<TargetSnapshot>, String> {
        let descriptor = unsafe {
            // SAFETY: the parent descriptor and single-component name are valid for this call.
            libc::openat(
                self.parent.as_raw_fd(),
                self.name.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            )
        };
        if descriptor < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::NotFound {
                return Ok(None);
            }
            return Err(io_error(&self.display, error));
        }
        let mut file = owned_file_descriptor(descriptor, &self.display)?;
        if !file
            .metadata()
            .map_err(|error| io_error(&self.display, error))?
            .is_file()
        {
            return Err(format!(
                "bridge target is not a regular file: {}",
                self.display.display()
            ));
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|error| io_error(&self.display, error))?;
        Ok(Some(TargetSnapshot { bytes }))
    }

    /// Publishes only a new final entry. POSIX has no inode-and-bytes
    /// conditional replacement or unlink primitive, so this writer never
    /// replaces or removes an existing bridge path.
    fn publish_creation(&self, bytes: &[u8]) -> Result<FileIdentity, MutationError> {
        let temp_name = CString::new(format!(
            ".{}.{}.tmp",
            self.name.to_string_lossy(),
            Uuid::new_v4()
        ))
        .map_err(|_| {
            MutationError::before_publication("temporary bridge name contains NUL".to_string())
        })?;
        let descriptor = unsafe {
            // SAFETY: parent and temp name are valid; O_EXCL creates one owned temp file.
            libc::openat(
                self.parent.as_raw_fd(),
                temp_name.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            )
        };
        let mut temp = owned_file_descriptor(descriptor, &self.display)
            .map_err(MutationError::before_publication)?;
        let result = (|| -> Result<FileIdentity, MutationError> {
            temp.write_all(bytes).map_err(|error| {
                MutationError::before_publication(io_error(&self.display, error))
            })?;
            temp.sync_all().map_err(|error| {
                MutationError::before_publication(io_error(&self.display, error))
            })?;
            let metadata = temp.metadata().map_err(|error| {
                MutationError::before_publication(io_error(&self.display, error))
            })?;
            let publication = file_identity(&metadata);
            drop(temp);
            self.ensure_entry_absent()
                .map_err(MutationError::before_publication)?;
            // `linkat` below is create-only. This test hook is deliberately
            // after the final absence check so an adversarial replacement is
            // exercised at the only publication operation, not before it.
            after_final_target_validation();
            let status = unsafe {
                // SAFETY: both names are relative to the same retained parent descriptor.
                // `linkat` fails with EEXIST instead of replacing a concurrent final entry.
                libc::linkat(
                    self.parent.as_raw_fd(),
                    temp_name.as_ptr(),
                    self.parent.as_raw_fd(),
                    self.name.as_ptr(),
                    0,
                )
            };
            if status != 0 {
                return Err(MutationError::before_publication(io_error(
                    &self.display,
                    std::io::Error::last_os_error(),
                )));
            }
            if let Err(error) = unlink_at_if_present(&self.parent, &temp_name, &self.display) {
                return Err(MutationError::after_publication(error, publication));
            }
            Ok(publication)
        })();
        if result.is_err() {
            let _ = unlink_at_if_present(&self.parent, &temp_name, &self.display);
        }
        result
    }

    fn remove_validated_regular_file(&self) -> Result<bool, String> {
        if self.snapshot_optional()?.is_none() {
            return Ok(false);
        }
        unlink_at_if_present(&self.parent, &self.name, &self.display)?;
        self.sync_parent()?;
        Ok(true)
    }

    fn sync_parent(&self) -> Result<(), String> {
        sync_directory(&self.parent, &self.display)
    }

    fn ensure_entry_absent(&self) -> Result<(), String> {
        if self.entry_identity_optional()?.is_none() {
            Ok(())
        } else {
            Err(format!(
                "bridge target already exists and cannot be safely replaced: {}",
                self.display.display()
            ))
        }
    }

    fn entry_identity_optional(&self) -> Result<Option<FileIdentity>, String> {
        let mut stat = unsafe { std::mem::zeroed::<libc::stat>() };
        let status = unsafe {
            // SAFETY: parent descriptor and one-component name remain valid for the call.
            libc::fstatat(
                self.parent.as_raw_fd(),
                self.name.as_ptr(),
                &mut stat,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if status != 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::NotFound {
                return Ok(None);
            }
            return Err(io_error(&self.display, error));
        }
        if stat.st_mode & libc::S_IFMT != libc::S_IFREG {
            return Err(format!(
                "bridge target is not a regular file: {}",
                self.display.display()
            ));
        }
        Ok(Some(FileIdentity {
            device: stat.st_dev as u64,
            inode: stat.st_ino as u64,
        }))
    }
}

#[cfg(unix)]
fn list_directory_entries(directory: &File) -> Result<Vec<OsString>, String> {
    let cloned = directory
        .try_clone()
        .map_err(|error| io_error(Path::new("."), error))?;
    let descriptor = cloned.into_raw_fd();
    let stream = fdopendir_for_listing(descriptor);
    if stream.is_null() {
        let open_error = std::io::Error::last_os_error();
        let close_status = unsafe {
            // SAFETY: fdopendir did not take ownership when it returned null.
            libc::close(descriptor)
        };
        let open_message = io_error(Path::new("."), open_error);
        if close_status != 0 {
            return Err(format!(
                "{open_message}; descriptor cleanup also failed: {}",
                io_error(Path::new("."), std::io::Error::last_os_error())
            ));
        }
        return Err(open_message);
    }
    let mut entries = Vec::new();
    let mut read_error = None;
    loop {
        set_errno(0);
        let entry = readdir_for_listing(stream);
        if entry.is_null() {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(0) {
                read_error = Some(error);
            }
            break;
        }
        let name = unsafe {
            // SAFETY: d_name is NUL-terminated for a successful readdir entry.
            CStr::from_ptr((*entry).d_name.as_ptr())
        };
        let bytes = name.to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        entries.push(OsString::from_vec(bytes.to_vec()));
    }
    let close_status = closedir_for_listing(stream);
    let close_error = if close_status == 0 {
        None
    } else {
        Some(std::io::Error::last_os_error())
    };
    match (read_error, close_error) {
        (Some(read_error), Some(close_error)) => {
            return Err(format!(
                "{}; directory close also failed: {}",
                io_error(Path::new("."), read_error),
                io_error(Path::new("."), close_error)
            ));
        }
        (Some(error), None) | (None, Some(error)) => {
            return Err(io_error(Path::new("."), error));
        }
        (None, None) => {}
    }
    entries.sort();
    Ok(entries)
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "visionos",
    target_os = "freebsd"
))]
fn set_errno(value: libc::c_int) {
    unsafe {
        // SAFETY: libc returns the calling thread's errno storage.
        *libc::__error() = value;
    }
}

#[cfg(any(target_os = "openbsd", target_os = "netbsd", target_os = "android"))]
fn set_errno(value: libc::c_int) {
    unsafe {
        // SAFETY: libc returns the calling thread's errno storage.
        *libc::__errno() = value;
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "hurd",
    target_os = "emscripten",
    target_os = "redox",
    target_os = "dragonfly"
))]
fn set_errno(value: libc::c_int) {
    unsafe {
        // SAFETY: libc returns the calling thread's errno storage.
        *libc::__errno_location() = value;
    }
}

#[cfg(any(target_os = "solaris", target_os = "illumos"))]
fn set_errno(value: libc::c_int) {
    unsafe {
        // SAFETY: libc returns the calling thread's errno storage.
        *libc::___errno() = value;
    }
}

#[cfg(target_os = "aix")]
fn set_errno(value: libc::c_int) {
    unsafe {
        // SAFETY: libc returns the calling thread's errno storage.
        *libc::_Errno() = value;
    }
}

#[cfg(target_os = "haiku")]
fn set_errno(value: libc::c_int) {
    unsafe {
        // SAFETY: libc returns the calling thread's errno storage.
        *libc::_errnop() = value;
    }
}

#[cfg(target_os = "nto")]
fn set_errno(value: libc::c_int) {
    unsafe {
        // SAFETY: libc returns the calling thread's errno storage.
        *libc::__get_errno_ptr() = value;
    }
}

#[cfg(any(target_os = "cygwin", target_os = "nuttx"))]
fn set_errno(value: libc::c_int) {
    unsafe {
        // SAFETY: libc returns the calling thread's errno storage.
        *libc::__errno() = value;
    }
}

#[cfg(unix)]
fn open_project_root_no_follow(path: &Path) -> Result<File, String> {
    if path.as_os_str().is_empty() {
        return Err("project root is empty".to_string());
    }
    let display = path.to_path_buf();
    let path = CString::new(display.as_os_str().as_bytes())
        .map_err(|_| "project root contains NUL".to_string())?;
    let descriptor = unsafe {
        // SAFETY: the full project-root path is NUL-free and O_NOFOLLOW protects its final
        // component while the returned descriptor pins the directory identity.
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY
                | libc::O_CLOEXEC
                | libc::O_DIRECTORY
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK,
        )
    };
    owned_file_descriptor(descriptor, &display)
}

#[cfg(unix)]
fn file_identity(metadata: &std::fs::Metadata) -> FileIdentity {
    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[cfg(unix)]
fn open_child_directory(directory: &File, segment: &OsStr) -> std::io::Result<File> {
    let segment = component_c_string(segment)
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;
    let descriptor = unsafe {
        // SAFETY: directory owns a live descriptor and segment is one path component.
        libc::openat(
            directory.as_raw_fd(),
            segment.as_ptr(),
            libc::O_RDONLY
                | libc::O_CLOEXEC
                | libc::O_DIRECTORY
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK,
        )
    };
    if descriptor < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe {
            // SAFETY: openat returned a new owned descriptor.
            File::from_raw_fd(descriptor)
        })
    }
}

#[cfg(unix)]
fn create_child_directory(directory: &File, segment: &OsStr) -> Result<(), String> {
    let segment = component_c_string(segment)?;
    let status = unsafe {
        // SAFETY: directory owns a live descriptor and segment is one path component.
        libc::mkdirat(directory.as_raw_fd(), segment.as_ptr(), 0o755)
    };
    if status == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        Ok(())
    } else {
        Err(format!("failed to create bridge directory: {error}"))
    }
}

#[cfg(unix)]
fn component_c_string(segment: &OsStr) -> Result<CString, String> {
    CString::new(segment.as_bytes()).map_err(|_| "bridge path component contains NUL".to_string())
}

#[cfg(unix)]
fn validate_receipt_target(relative: &Path) -> Result<(), String> {
    validate_relative_path_buf(relative)?;
    let components = relative.components().collect::<Vec<_>>();
    let is_receipt = components.len() == 6
        && matches!(components[0], Component::Normal(value) if value == OsStr::new(".tree-ring"))
        && matches!(components[1], Component::Normal(value) if value == OsStr::new("activation"))
        && matches!(components[2], Component::Normal(value) if value == OsStr::new("receipts"))
        && matches!(components[3], Component::Normal(_))
        && matches!(components[4], Component::Normal(_))
        && matches!(components[5], Component::Normal(value) if Path::new(value).extension() == Some(OsStr::new("json")));
    if is_receipt {
        Ok(())
    } else {
        Err("activation receipt target is outside the receipt store".to_string())
    }
}

#[cfg(unix)]
fn runtime_context_target(receipt_id: &str) -> Result<PathBuf, String> {
    let valid_id = receipt_id.starts_with("receipt-")
        && receipt_id.len() <= 128
        && receipt_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    if !valid_id {
        return Err("invalid launcher receipt id".to_string());
    }
    Ok(PathBuf::from(".tree-ring/activation/runtime").join(format!("{receipt_id}.md")))
}

#[cfg(unix)]
fn owned_file_descriptor(descriptor: libc::c_int, path: &Path) -> Result<File, String> {
    if descriptor < 0 {
        return Err(io_error(path, std::io::Error::last_os_error()));
    }
    Ok(unsafe {
        // SAFETY: the successful libc call returned a new owned descriptor.
        File::from_raw_fd(descriptor)
    })
}

#[cfg(unix)]
fn unlink_at_if_present(parent: &File, name: &CStr, display: &Path) -> Result<(), String> {
    let status = unsafe {
        // SAFETY: parent owns a live descriptor and name is one child component.
        libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), 0)
    };
    if status == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::NotFound {
        Ok(())
    } else {
        Err(io_error(display, error))
    }
}

#[cfg(unix)]
fn sync_directory(directory: &File, display: &Path) -> Result<(), String> {
    if fail_directory_sync_now() {
        return Err(format!(
            "{}: injected directory sync failure",
            display.display()
        ));
    }
    let status = unsafe {
        // SAFETY: directory owns a live directory descriptor.
        libc::fsync(directory.as_raw_fd())
    };
    if status == 0 {
        Ok(())
    } else {
        Err(io_error(display, std::io::Error::last_os_error()))
    }
}

#[cfg(not(unix))]
#[derive(Debug)]
pub(crate) struct ProjectFs;

#[cfg(not(unix))]
#[derive(Debug)]
pub(crate) struct ResolvedTarget;

#[cfg(not(unix))]
#[derive(Debug)]
pub(crate) struct ManifestLock;

#[cfg(not(unix))]
impl ProjectFs {
    pub(crate) fn open(_project: &ActivationProject) -> Result<Self, String> {
        Err("bridge mutation requires descriptor-relative no-follow filesystem support".to_string())
    }

    pub(crate) fn lock_manifest(&self) -> Result<ManifestLock, String> {
        Err("bridge mutation requires descriptor-relative no-follow filesystem support".to_string())
    }

    fn same_root_identity(&self, _other: &Self) -> Result<bool, String> {
        Err("bridge access requires descriptor-relative no-follow filesystem support".to_string())
    }

    pub(crate) fn read_optional(&self, _relative: &Path) -> Result<Option<Vec<u8>>, String> {
        Err("bridge access requires descriptor-relative no-follow filesystem support".to_string())
    }

    fn ensure_root_binding(&self) -> Result<(), String> {
        Err("bridge mutation requires descriptor-relative no-follow filesystem support".to_string())
    }

    fn resolve_target(
        &self,
        _relative: &Path,
        _create_parents: bool,
    ) -> Result<ResolvedTarget, String> {
        Err("bridge mutation requires descriptor-relative no-follow filesystem support".to_string())
    }

    pub(crate) fn create_receipt_file(
        &self,
        _relative: &Path,
        _bytes: &[u8],
    ) -> Result<(), String> {
        Err("bridge mutation requires descriptor-relative no-follow filesystem support".to_string())
    }

    pub(crate) fn create_runtime_context_file(
        &self,
        _receipt_id: &str,
        _bytes: &[u8],
    ) -> Result<PathBuf, String> {
        Err(
            "launcher mutation requires descriptor-relative no-follow filesystem support"
                .to_string(),
        )
    }

    pub(crate) fn directory_entries(
        &self,
        _relative: &Path,
    ) -> Result<Option<Vec<OsString>>, String> {
        Err("bridge access requires descriptor-relative no-follow filesystem support".to_string())
    }

    pub(crate) fn remove_validated_receipt_file(&self, _relative: &Path) -> Result<bool, String> {
        Err("bridge mutation requires descriptor-relative no-follow filesystem support".to_string())
    }

    pub(crate) fn remove_runtime_context_file(&self, _receipt_id: &str) -> Result<bool, String> {
        Err(
            "launcher mutation requires descriptor-relative no-follow filesystem support"
                .to_string(),
        )
    }
}

#[cfg(not(unix))]
impl ResolvedTarget {
    pub(crate) fn read_optional(&self) -> Result<Option<Vec<u8>>, String> {
        Err("bridge access requires descriptor-relative no-follow filesystem support".to_string())
    }

    fn snapshot_optional(&self) -> Result<Option<TargetSnapshot>, String> {
        Err("bridge access requires descriptor-relative no-follow filesystem support".to_string())
    }

    fn publish_creation(&self, _bytes: &[u8]) -> Result<FileIdentity, MutationError> {
        Err(MutationError::before_publication(
            "bridge mutation requires descriptor-relative no-follow filesystem support".to_string(),
        ))
    }

    fn sync_parent(&self) -> Result<(), String> {
        Err("bridge mutation requires descriptor-relative no-follow filesystem support".to_string())
    }
}

#[derive(Debug)]
enum Preparation {
    Ready {
        files: Vec<PreparedFile>,
        activation: HarnessActivation,
    },
    Review {
        next_step: String,
    },
}

/// Applies an adapter plan after every target has passed ownership and format
/// checks. A review result is non-mutating and never leaves a partial bridge.
pub fn apply_bridge_plan(
    project: &ActivationProject,
    manifest: &mut ActivationManifest,
    plan: AdapterPlan,
    accept_managed_block: bool,
) -> Result<BridgePlanResult, String> {
    validate_manifest(manifest)?;
    validate_plan(&plan)?;
    let project_fs = ProjectFs::open(project)?;
    let _manifest_lock = project_fs.lock_manifest()?;
    let (current_manifest, expected_persisted) = reconcile_manifest(&project_fs, manifest)?;

    if matches!(
        plan.state,
        ActivationState::NeedsPlugin | ActivationState::Unsupported
    ) {
        if !plan.writes.is_empty() {
            return Err(format!(
                "{} plan cannot write while in state {:?}",
                plan.harness_id, plan.state
            ));
        }
        let mut next_manifest = current_manifest.clone();
        let activation = blocked_activation(&plan, next_manifest.harnesses.get(&plan.harness_id))?;
        next_manifest
            .harnesses
            .insert(plan.harness_id.clone(), activation);
        if manifest_update_requires_replacement(expected_persisted.as_ref(), &next_manifest) {
            return Ok(creation_only_review_result(&plan.harness_id));
        }
        commit_files_and_manifest(
            &project_fs,
            manifest,
            &current_manifest,
            expected_persisted.as_ref(),
            next_manifest,
            &[],
        )?;
        return Ok(BridgePlanResult {
            state: plan.state,
            changed_paths: Vec::new(),
            next_step: plan.next_step,
        });
    }

    let prepared = prepare_apply(&project_fs, &current_manifest, &plan, accept_managed_block)?;
    let Preparation::Ready { files, activation } = prepared else {
        let Preparation::Review { next_step } = prepared else {
            unreachable!()
        };
        return Ok(BridgePlanResult {
            state: ActivationState::NeedsUserReview,
            changed_paths: Vec::new(),
            next_step,
        });
    };

    let mut next_manifest = current_manifest.clone();
    next_manifest
        .harnesses
        .insert(plan.harness_id.clone(), activation);
    validate_manifest(&next_manifest)?;
    if files_require_existing_entry_mutation(&files)
        || manifest_update_requires_replacement(expected_persisted.as_ref(), &next_manifest)
    {
        return Ok(creation_only_review_result(&plan.harness_id));
    }
    let changed_paths = files
        .iter()
        .filter(|file| file.before != file.after)
        .map(|file| file.relative.clone())
        .collect::<Vec<_>>();
    if changed_paths.is_empty() && next_manifest == current_manifest {
        *manifest = current_manifest;
        return Ok(BridgePlanResult {
            state: applied_state(&plan.harness_id, plan.state),
            changed_paths,
            next_step: plan.next_step,
        });
    }
    commit_files_and_manifest(
        &project_fs,
        manifest,
        &current_manifest,
        expected_persisted.as_ref(),
        next_manifest,
        &files,
    )?;

    let state = applied_state(&plan.harness_id, plan.state);
    Ok(BridgePlanResult {
        state,
        changed_paths,
        next_step: plan.next_step,
    })
}

/// Applies every safe adapter plan beneath one pinned project root and publishes
/// one creation-only activation manifest after all bridge files are ready.
///
/// This is deliberately narrower than repeated `apply_bridge_plan` calls: it
/// exists for first-run init, where publishing the manifest after the first
/// adapter would make later adapters require replacement of a trusted final
/// entry. Existing manifests are accepted only when the complete batch is
/// already semantically identical.
pub fn apply_bridge_plans_create_only(
    project: &ActivationProject,
    manifest: &mut ActivationManifest,
    mut plans: Vec<AdapterPlan>,
) -> Result<Vec<BridgeBatchResult>, String> {
    validate_manifest(manifest)?;
    plans.sort_by(|left, right| left.harness_id.cmp(&right.harness_id));
    for plan in &plans {
        validate_plan(plan)?;
    }
    if plans
        .windows(2)
        .any(|pair| pair[0].harness_id == pair[1].harness_id)
    {
        return Err("duplicate harness plan in activation batch".to_string());
    }

    let project_fs = ProjectFs::open(project)?;
    let _manifest_lock = project_fs.lock_manifest()?;
    let (current_manifest, expected_persisted) = reconcile_manifest(&project_fs, manifest)?;
    let mut next_manifest = current_manifest.clone();
    let mut files = Vec::<PreparedFile>::new();
    let mut outcomes = Vec::with_capacity(plans.len());

    for plan in plans {
        if matches!(
            plan.state,
            ActivationState::NeedsPlugin | ActivationState::Unsupported
        ) {
            if !plan.writes.is_empty() {
                return Err(format!(
                    "{} plan cannot write while in state {:?}",
                    plan.harness_id, plan.state
                ));
            }
            let activation =
                blocked_activation(&plan, next_manifest.harnesses.get(&plan.harness_id))?;
            next_manifest
                .harnesses
                .insert(plan.harness_id.clone(), activation);
            outcomes.push(BridgeBatchResult {
                harness_id: plan.harness_id,
                result: BridgePlanResult {
                    state: plan.state,
                    changed_paths: Vec::new(),
                    next_step: plan.next_step,
                },
            });
            continue;
        }

        match prepare_apply(&project_fs, &next_manifest, &plan, false)? {
            Preparation::Review { next_step } => outcomes.push(BridgeBatchResult {
                harness_id: plan.harness_id,
                result: BridgePlanResult {
                    state: ActivationState::NeedsUserReview,
                    changed_paths: Vec::new(),
                    next_step,
                },
            }),
            Preparation::Ready {
                files: plan_files,
                activation,
            } => {
                let changed_paths = plan_files
                    .iter()
                    .filter(|file| file.before != file.after)
                    .map(|file| file.relative.clone())
                    .collect::<Vec<_>>();
                for file in plan_files {
                    if let Some(existing) = files
                        .iter()
                        .find(|existing| existing.relative == file.relative)
                    {
                        if existing.before != file.before || existing.after != file.after {
                            return Err(
                                "incompatible shared bridge target in activation batch".to_string()
                            );
                        }
                    } else {
                        files.push(file);
                    }
                }
                next_manifest
                    .harnesses
                    .insert(plan.harness_id.clone(), activation);
                let state = applied_state(&plan.harness_id, plan.state);
                outcomes.push(BridgeBatchResult {
                    harness_id: plan.harness_id,
                    result: BridgePlanResult {
                        state,
                        changed_paths,
                        next_step: plan.next_step,
                    },
                });
            }
        }
    }

    validate_manifest(&next_manifest)?;
    if files_require_existing_entry_mutation(&files) {
        return Err(
            "bridge writer will not replace or remove an existing final entry; explicit review is required"
                .to_string(),
        );
    }
    if manifest_update_requires_replacement(expected_persisted.as_ref(), &next_manifest) {
        for outcome in &mut outcomes {
            if current_manifest.harnesses.get(&outcome.harness_id)
                != next_manifest.harnesses.get(&outcome.harness_id)
            {
                outcome.result = creation_only_review_result(&outcome.harness_id);
            } else {
                outcome.result.changed_paths.clear();
            }
        }
        *manifest = current_manifest;
        return Ok(outcomes);
    }
    if expected_persisted.is_some()
        && files.iter().all(|file| file.before == file.after)
        && next_manifest == current_manifest
    {
        *manifest = current_manifest;
        for outcome in &mut outcomes {
            outcome.result.changed_paths.clear();
        }
        return Ok(outcomes);
    }

    commit_files_and_manifest(
        &project_fs,
        manifest,
        &current_manifest,
        expected_persisted.as_ref(),
        next_manifest,
        &files,
    )?;
    Ok(outcomes)
}

/// Validates the two-root preflight topology without following project-root or
/// manifest symlinks. The selected manifest is returned for the subsequent
/// isolated-store preflight; the canonical manifest is read only to prove that
/// the explicit project root is itself a valid Tree Ring project.
pub fn validate_isolated_preflight_roots(
    selected_memory_root: &Path,
    canonical_project_root: &Path,
) -> Result<ActivationManifest, String> {
    fn validate(
        selected_memory_root: &Path,
        canonical_project_root: &Path,
    ) -> Result<ActivationManifest, String> {
        if selected_memory_root.file_name() != Some(OsStr::new(".tree-ring")) {
            return Err("selected memory root is not project-local".to_string());
        }
        let selected_project = ActivationProject::from_memory_root(selected_memory_root)?;
        let canonical_project = ActivationProject::from_project_root(canonical_project_root);
        let selected_fs = ProjectFs::open(&selected_project)?;
        let canonical_fs = ProjectFs::open(&canonical_project)?;
        let selected = load_persisted_manifest(&selected_fs)?
            .ok_or_else(|| "selected activation manifest is unavailable".to_string())?;
        let canonical = load_persisted_manifest(&canonical_fs)?
            .ok_or_else(|| "canonical activation manifest is unavailable".to_string())?;

        if selected.project_root_fingerprint != fingerprint_path(&selected_project.project_root)
            || canonical.project_root_fingerprint
                != fingerprint_path(&canonical_project.project_root)
        {
            return Err("activation manifest project identity mismatch".to_string());
        }
        if selected_fs.same_root_identity(&canonical_fs)? || selected.store_id == canonical.store_id
        {
            return Err("preflight roots are not isolated".to_string());
        }
        Ok(selected)
    }

    validate(selected_memory_root, canonical_project_root)
        .map_err(|_| "isolated preflight roots are invalid".to_string())
}

/// Reads the init manifest through the pinned project descriptor. A missing
/// manifest is returned as `None`; callers may then construct the first
/// in-memory identity for creation-only batch publication.
pub fn load_init_manifest_no_follow(
    project: &ActivationProject,
) -> Result<Option<ActivationManifest>, String> {
    let project_fs = ProjectFs::open(project)?;
    load_persisted_manifest(&project_fs)
}

/// Produces the same validated plan as apply without writing files or the
/// activation manifest.
pub fn preview_bridge_plan(
    project: &ActivationProject,
    manifest: &ActivationManifest,
    plan: AdapterPlan,
    accept_managed_block: bool,
) -> Result<BridgePlanResult, String> {
    validate_manifest(manifest)?;
    validate_plan(&plan)?;
    let project_fs = ProjectFs::open(project)?;
    let (current_manifest, expected_persisted) = reconcile_manifest(&project_fs, manifest)?;
    if matches!(
        plan.state,
        ActivationState::NeedsPlugin | ActivationState::Unsupported
    ) {
        if !plan.writes.is_empty() {
            return Err(format!(
                "{} plan cannot write while in state {:?}",
                plan.harness_id, plan.state
            ));
        }
        let mut next_manifest = current_manifest.clone();
        let activation = blocked_activation(&plan, next_manifest.harnesses.get(&plan.harness_id))?;
        next_manifest
            .harnesses
            .insert(plan.harness_id.clone(), activation);
        if manifest_update_requires_replacement(expected_persisted.as_ref(), &next_manifest) {
            return Ok(creation_only_review_result(&plan.harness_id));
        }
        return Ok(BridgePlanResult {
            state: plan.state,
            changed_paths: Vec::new(),
            next_step: plan.next_step,
        });
    }
    match prepare_apply(&project_fs, &current_manifest, &plan, accept_managed_block)? {
        Preparation::Review { next_step } => Ok(BridgePlanResult {
            state: ActivationState::NeedsUserReview,
            changed_paths: Vec::new(),
            next_step,
        }),
        Preparation::Ready { files, activation } => {
            let mut next_manifest = current_manifest.clone();
            next_manifest
                .harnesses
                .insert(plan.harness_id.clone(), activation);
            if files_require_existing_entry_mutation(&files)
                || manifest_update_requires_replacement(expected_persisted.as_ref(), &next_manifest)
            {
                return Ok(creation_only_review_result(&plan.harness_id));
            }
            Ok(BridgePlanResult {
                state: applied_state(&plan.harness_id, plan.state),
                changed_paths: files
                    .into_iter()
                    .filter(|file| file.before != file.after)
                    .map(|file| file.relative)
                    .collect(),
                next_step: plan.next_step,
            })
        }
    }
}

/// Removes only hash-matching complete files and exact managed blocks recorded
/// for one harness. Canonical store files, receipts, and other harness owners
/// are never targets.
pub fn deactivate_bridge_plan(
    project: &ActivationProject,
    manifest: &mut ActivationManifest,
    harness_id: &str,
) -> Result<BridgePlanResult, String> {
    validate_manifest(manifest)?;
    let project_fs = ProjectFs::open(project)?;
    let _manifest_lock = project_fs.lock_manifest()?;
    let (current_manifest, expected_persisted) = reconcile_manifest(&project_fs, manifest)?;
    let Some(current) = current_manifest.harnesses.get(harness_id).cloned() else {
        return Ok(BridgePlanResult {
            state: deactivated_state(harness_id),
            changed_paths: Vec::new(),
            next_step: "No manifest-recorded Tree Ring bridge material was present.".to_string(),
        });
    };

    let mut files = Vec::new();
    let mut retained_files = Vec::new();
    for owned in &current.owned_files {
        let relative = PathBuf::from(&owned.path);
        let before = project_fs.read_optional(&relative)?;
        let other_owner = current_manifest
            .harnesses
            .iter()
            .any(|(other_id, activation)| {
                other_id != harness_id
                    && activation
                        .owned_files
                        .iter()
                        .any(|other| other.path == owned.path && other.sha256 == owned.sha256)
            });
        match before {
            Some(bytes) if sha256(&bytes) == owned.sha256 && !other_owner => {
                files.push(PreparedFile {
                    relative,
                    before: Some(bytes),
                    after: None,
                });
            }
            Some(bytes) if sha256(&bytes) == owned.sha256 && other_owner => {}
            None => {}
            _ => retained_files.push(owned.clone()),
        }
    }

    let mut retained_blocks = Vec::new();
    for owned in &current.managed_blocks {
        let relative = PathBuf::from(&owned.path);
        let before = project_fs.read_optional(&relative)?;
        let Some(before_bytes) = before else {
            continue;
        };
        let removal = if relative == Path::new("AGENTS.md") {
            remove_markdown_block(
                &before_bytes,
                &owned.block_id,
                &owned.sha256,
                &owned.leading_separator,
            )?
        } else if relative == Path::new(".claude/settings.json") {
            remove_claude_handler(&before_bytes, &owned.sha256)?
        } else {
            return Err(format!(
                "unsupported managed bridge target in manifest: {}",
                relative.display()
            ));
        };
        match removal {
            Some(after) => files.push(PreparedFile {
                relative,
                before: Some(before_bytes),
                after: Some(after),
            }),
            None => retained_blocks.push(owned.clone()),
        }
    }

    let mut next_manifest = current_manifest.clone();
    let next = next_manifest
        .harnesses
        .get_mut(harness_id)
        .expect("cloned manifest retained harness");
    next.state = deactivated_state(harness_id);
    next.bridge_path = None;
    next.owned_files = retained_files;
    next.managed_blocks = retained_blocks;
    if let Some(first) = next
        .owned_files
        .first()
        .map(|owned| owned.path.clone())
        .or_else(|| next.managed_blocks.first().map(|owned| owned.path.clone()))
    {
        next.bridge_path = Some(first);
    }
    let (adapter_capability, adapter_version) = adapter_contract(harness_id)?;
    next.adapter_capability = adapter_capability;
    next.adapter_version = adapter_version.to_string();
    next.bridge_fingerprint = bridge_fingerprint(harness_id, next);

    if files_require_existing_entry_mutation(&files)
        || manifest_update_requires_replacement(expected_persisted.as_ref(), &next_manifest)
    {
        return Ok(creation_only_review_result(harness_id));
    }
    let changed_paths = files
        .iter()
        .filter(|file| file.before != file.after)
        .map(|file| file.relative.clone())
        .collect::<Vec<_>>();
    commit_files_and_manifest(
        &project_fs,
        manifest,
        &current_manifest,
        expected_persisted.as_ref(),
        next_manifest,
        &files,
    )?;
    let needs_review = !next_manifest_ownership_empty(manifest, harness_id);
    Ok(BridgePlanResult {
        state: if needs_review {
            ActivationState::NeedsUserReview
        } else {
            deactivated_state(harness_id)
        },
        changed_paths,
        next_step: if needs_review {
            "Tree Ring preserved changed or shared bridge material for explicit review.".to_string()
        } else {
            "Tree Ring-owned bridge material was removed; store guidance and receipts were retained."
                .to_string()
        },
    })
}

fn prepare_apply(
    project_fs: &ProjectFs,
    manifest: &ActivationManifest,
    plan: &AdapterPlan,
    accept_managed_block: bool,
) -> Result<Preparation, String> {
    let existing = manifest.harnesses.get(&plan.harness_id);
    let mut files = Vec::new();
    let mut owned_files = existing
        .map(|activation| activation.owned_files.clone())
        .unwrap_or_default();
    let mut managed_blocks = existing
        .map(|activation| activation.managed_blocks.clone())
        .unwrap_or_default();

    for write in &plan.writes {
        let prepared = match write {
            PlannedWrite::BridgeWrite(write) => {
                let desired = render_complete_file(&plan.harness_id, &write.path, manifest)?;
                prepare_complete_file(
                    project_fs,
                    manifest,
                    &plan.harness_id,
                    &write.path,
                    desired,
                    &mut owned_files,
                )?
            }
            PlannedWrite::ManagedBlockUpdate(write) if write.path == Path::new("AGENTS.md") => {
                prepare_markdown_file(
                    project_fs,
                    &plan.harness_id,
                    write,
                    accept_managed_block,
                    &mut owned_files,
                    &mut managed_blocks,
                )?
            }
            PlannedWrite::ManagedBlockUpdate(write)
                if write.path == Path::new(".claude/settings.json") =>
            {
                prepare_claude_settings(project_fs, write, &mut owned_files, &mut managed_blocks)?
            }
            PlannedWrite::ManagedBlockUpdate(write) => {
                return Err(format!(
                    "unsupported managed bridge target: {}",
                    write.path.display()
                ));
            }
        };
        let Some(prepared) = prepared else {
            return Ok(Preparation::Review {
                next_step: review_step(&plan.harness_id),
            });
        };
        files.push(prepared);
    }

    owned_files.sort_by(|left, right| left.path.cmp(&right.path));
    owned_files.dedup_by(|left, right| left.path == right.path);
    managed_blocks.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.block_id.cmp(&right.block_id))
    });
    managed_blocks
        .dedup_by(|left, right| left.path == right.path && left.block_id == right.block_id);
    let bridge_path = owned_files
        .first()
        .map(|owned| owned.path.clone())
        .or_else(|| managed_blocks.first().map(|owned| owned.path.clone()));
    let (adapter_capability, adapter_version) = adapter_contract(&plan.harness_id)?;
    let mut activation = HarnessActivation {
        state: applied_state(&plan.harness_id, plan.state),
        adapter_capability,
        adapter_version: adapter_version.to_string(),
        bridge_fingerprint: String::new(),
        bridge_path,
        owned_files,
        managed_blocks,
    };
    activation.bridge_fingerprint = bridge_fingerprint(&plan.harness_id, &activation);
    Ok(Preparation::Ready { files, activation })
}

fn prepare_complete_file(
    project_fs: &ProjectFs,
    manifest: &ActivationManifest,
    harness_id: &str,
    relative: &Path,
    desired: Vec<u8>,
    owned_files: &mut Vec<OwnedBridgeFile>,
) -> Result<Option<PreparedFile>, String> {
    let before = project_fs.read_optional(relative)?;
    let path = relative_string(relative)?;
    let desired_hash = sha256(&desired);
    let own = owned_files.iter().find(|owned| owned.path == path).cloned();
    let other_owners = manifest
        .harnesses
        .iter()
        .filter(|(other_id, _)| other_id.as_str() != harness_id)
        .filter_map(|(_, activation)| {
            activation
                .owned_files
                .iter()
                .find(|owned| owned.path == path)
        })
        .collect::<Vec<_>>();

    let permitted = match (&before, own.as_ref()) {
        (None, None) => true,
        (None, Some(_)) => false,
        (Some(bytes), Some(owned)) => {
            sha256(bytes) == owned.sha256
                && (other_owners.is_empty()
                    || other_owners
                        .iter()
                        .all(|other| other.sha256 == owned.sha256 && desired_hash == owned.sha256))
        }
        (Some(bytes), None) => {
            !other_owners.is_empty()
                && other_owners
                    .iter()
                    .all(|other| sha256(bytes) == other.sha256 && desired_hash == other.sha256)
        }
    };
    if !permitted {
        return Ok(None);
    }
    upsert_owned_file(owned_files, path, desired_hash);
    Ok(Some(PreparedFile {
        relative: relative.to_path_buf(),
        before,
        after: Some(desired),
    }))
}

fn prepare_markdown_file(
    project_fs: &ProjectFs,
    harness_id: &str,
    write: &ManagedBlockUpdate,
    accept_managed_block: bool,
    owned_files: &mut Vec<OwnedBridgeFile>,
    managed_blocks: &mut Vec<OwnedManagedBlock>,
) -> Result<Option<PreparedFile>, String> {
    let before = project_fs.read_optional(&write.path)?;
    let path = relative_string(&write.path)?;
    let block = markdown_block(harness_id);
    if before.is_none() {
        if owned_files.iter().any(|owned| owned.path == path)
            || managed_blocks
                .iter()
                .any(|owned| owned.path == path && owned.block_id == write.block_id)
        {
            return Ok(None);
        }
        let bytes = block.into_bytes();
        upsert_owned_file(owned_files, path, sha256(&bytes));
        return Ok(Some(PreparedFile {
            relative: write.path.clone(),
            before: None,
            after: Some(bytes),
        }));
    }
    if let Some(owned) = owned_files.iter().find(|owned| owned.path == path) {
        let bytes = before.as_ref().expect("checked above");
        if sha256(bytes) != owned.sha256 {
            return Ok(None);
        }
        let desired = block.into_bytes();
        upsert_owned_file(owned_files, path, sha256(&desired));
        return Ok(Some(PreparedFile {
            relative: write.path.clone(),
            before,
            after: Some(desired),
        }));
    }

    let before_bytes = before.as_ref().expect("checked above");
    let before_text = std::str::from_utf8(before_bytes).map_err(|_| {
        format!(
            "managed Markdown bridge is not UTF-8: {}",
            write.path.display()
        )
    })?;
    let markers = locate_markdown_block(before_text, harness_id)?;
    let (after, leading_separator) = if let Some((start, end)) = markers {
        let existing_block = &before_text[start..end];
        let recorded = managed_blocks
            .iter()
            .find(|owned| owned.path == path && owned.block_id == write.block_id);
        if recorded.is_some_and(|owned| sha256(existing_block.as_bytes()) != owned.sha256) {
            return Ok(None);
        }
        (
            replace_range(before_text, start, end, &block).into_bytes(),
            recorded
                .map(|owned| owned.leading_separator.clone())
                .unwrap_or_default(),
        )
    } else {
        if !accept_managed_block {
            return Ok(None);
        }
        let (content, separator) = append_markdown_block(before_text, &block);
        (content.into_bytes(), separator)
    };
    upsert_managed_block(
        managed_blocks,
        path,
        write.block_id.clone(),
        sha256(block.as_bytes()),
        leading_separator,
    );
    Ok(Some(PreparedFile {
        relative: write.path.clone(),
        before,
        after: Some(after),
    }))
}

fn prepare_claude_settings(
    project_fs: &ProjectFs,
    write: &ManagedBlockUpdate,
    owned_files: &mut Vec<OwnedBridgeFile>,
    managed_blocks: &mut Vec<OwnedManagedBlock>,
) -> Result<Option<PreparedFile>, String> {
    let before = project_fs.read_optional(&write.path)?;
    let path = relative_string(&write.path)?;
    let handler = claude_handler();
    let handler_hash = sha256(&serde_json::to_vec(&handler).map_err(json_error)?);
    if before.is_none() {
        if owned_files.iter().any(|owned| owned.path == path)
            || managed_blocks
                .iter()
                .any(|owned| owned.path == path && owned.block_id == write.block_id)
        {
            return Ok(None);
        }
        let mut root = Map::new();
        insert_claude_handler(&mut root, handler)?;
        let bytes = pretty_json(&Value::Object(root))?;
        upsert_owned_file(owned_files, path, sha256(&bytes));
        return Ok(Some(PreparedFile {
            relative: write.path.clone(),
            before: None,
            after: Some(bytes),
        }));
    }
    if let Some(owned) = owned_files.iter().find(|owned| owned.path == path) {
        let bytes = before.as_ref().expect("checked above");
        if sha256(bytes) != owned.sha256 {
            return Ok(None);
        }
        let mut root = parse_json_object(bytes)?;
        let state = match inspect_claude_handler(&root) {
            Ok(state) => state,
            Err(_) => return Ok(None),
        };
        match state {
            ClaudeHandlerState::Exact => {}
            ClaudeHandlerState::Absent => {
                if insert_claude_handler(&mut root, handler).is_err() {
                    return Ok(None);
                }
            }
            ClaudeHandlerState::Conflict => return Ok(None),
        }
        let desired = pretty_json(&Value::Object(root))?;
        upsert_owned_file(owned_files, path, sha256(&desired));
        return Ok(Some(PreparedFile {
            relative: write.path.clone(),
            before,
            after: Some(desired),
        }));
    }

    let before_bytes = before.as_ref().expect("checked above");
    let mut root = match parse_json_object(before_bytes) {
        Ok(root) => root,
        Err(_) => return Ok(None),
    };
    let state = match inspect_claude_handler(&root) {
        Ok(state) => state,
        Err(_) => return Ok(None),
    };
    match state {
        ClaudeHandlerState::Conflict => return Ok(None),
        ClaudeHandlerState::Exact => {}
        ClaudeHandlerState::Absent => {
            if insert_claude_handler(&mut root, handler).is_err() {
                return Ok(None);
            }
        }
    }
    let after = pretty_json(&Value::Object(root))?;
    upsert_managed_block(
        managed_blocks,
        path,
        write.block_id.clone(),
        handler_hash,
        String::new(),
    );
    Ok(Some(PreparedFile {
        relative: write.path.clone(),
        before,
        after: Some(after),
    }))
}

fn render_complete_file(
    harness_id: &str,
    path: &Path,
    manifest: &ActivationManifest,
) -> Result<Vec<u8>, String> {
    match (harness_id, path.to_str()) {
        ("codex" | "pi", Some(".agents/skills/tree-ring-memory/SKILL.md"))
        | ("claude-code", Some(".claude/skills/tree-ring-memory/SKILL.md")) => {
            Ok(SKILL_BRIDGE.as_bytes().to_vec())
        }
        ("pi", Some(".pi/extensions/tree-ring-memory.ts")) => Ok(PI_EXTENSION.as_bytes().to_vec()),
        ("agent-zero", Some(".tree-ring/activation/agent-zero.json")) => {
            let binding = json!({
                "protocol_version": ACTIVATION_PROTOCOL_VERSION,
                "store_id": manifest.store_id,
                "project_root_fingerprint": manifest.project_root_fingerprint,
                "memory_root": ".tree-ring",
                "command_protocol": {
                    "command": "tree-ring",
                    "arguments": [
                        "--root", ".tree-ring", "integrations", "preflight",
                        "--harness", "agent-zero", "--input-json-stdin",
                        "--context-format", "json"
                    ],
                    "stdin": "json",
                    "stdout": "json"
                }
            });
            pretty_json(&binding)
        }
        _ => Err(format!(
            "unexpected complete bridge target for {harness_id}: {}",
            path.display()
        )),
    }
}

fn validate_plan(plan: &AdapterPlan) -> Result<(), String> {
    let expected = if matches!(
        plan.state,
        ActivationState::NeedsPlugin | ActivationState::Unsupported
    ) {
        Vec::new()
    } else {
        match plan.harness_id.as_str() {
            "codex" => vec![
                ("file", ".agents/skills/tree-ring-memory/SKILL.md", ""),
                ("block", "AGENTS.md", "codex"),
            ],
            "claude-code" => vec![
                ("file", ".claude/skills/tree-ring-memory/SKILL.md", ""),
                ("block", ".claude/settings.json", "claude-code"),
            ],
            "pi" => vec![
                ("file", ".agents/skills/tree-ring-memory/SKILL.md", ""),
                ("file", ".pi/extensions/tree-ring-memory.ts", ""),
            ],
            "agent-zero" => vec![("file", ".tree-ring/activation/agent-zero.json", "")],
            "hermes" | "opencode" | "goose" => Vec::new(),
            other => return Err(format!("unknown harness adapter: {other}")),
        }
    };
    let actual = plan
        .writes
        .iter()
        .map(|write| match write {
            PlannedWrite::BridgeWrite(write) => {
                validate_relative_path_buf(&write.path)?;
                Ok(("file", relative_str(&write.path)?, ""))
            }
            PlannedWrite::ManagedBlockUpdate(write) => {
                validate_relative_path_buf(&write.path)?;
                Ok(("block", relative_str(&write.path)?, write.block_id.as_str()))
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    if actual != expected {
        return Err(format!("unexpected bridge writes for {}", plan.harness_id));
    }
    Ok(())
}

fn validate_project_shape(project: &ActivationProject) -> Result<(), String> {
    if project.memory_root != project.project_root.join(".tree-ring") {
        return Err(
            "activation project memory root must be the project-local .tree-ring".to_string(),
        );
    }
    Ok(())
}

fn validate_relative_path_buf(path: &Path) -> Result<(), String> {
    let value = path
        .to_str()
        .ok_or_else(|| "bridge path must be valid UTF-8".to_string())?;
    validate_project_relative_path(value)
}

fn relative_str(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| "bridge path must be valid UTF-8".to_string())
}

fn relative_string(path: &Path) -> Result<String, String> {
    Ok(relative_str(path)?.to_string())
}

fn markdown_block(harness_id: &str) -> String {
    format!(
        "<!-- tree-ring:begin {harness_id} v1 -->\n{CODEX_BLOCK_BODY}<!-- tree-ring:end {harness_id} -->\n"
    )
}

fn locate_markdown_block(
    content: &str,
    harness_id: &str,
) -> Result<Option<(usize, usize)>, String> {
    let begin = format!("<!-- tree-ring:begin {harness_id} v1 -->");
    let end = format!("<!-- tree-ring:end {harness_id} -->");
    let begins = content.match_indices(&begin).collect::<Vec<_>>();
    let ends = content.match_indices(&end).collect::<Vec<_>>();
    match (begins.as_slice(), ends.as_slice()) {
        ([], []) => Ok(None),
        ([(start, _)], [(end_start, _)]) if start < end_start => {
            let mut end_offset = end_start + end.len();
            if content[end_offset..].starts_with('\n') {
                end_offset += 1;
            }
            Ok(Some((*start, end_offset)))
        }
        _ => Err(format!(
            "conflicting or incomplete Tree Ring managed markers for {harness_id}"
        )),
    }
}

fn append_markdown_block(content: &str, block: &str) -> (String, String) {
    if content.is_empty() {
        return (block.to_string(), String::new());
    }
    let separator = if content.ends_with("\n\n") {
        ""
    } else if content.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    (
        format!("{content}{separator}{block}"),
        separator.to_string(),
    )
}

fn replace_range(content: &str, start: usize, end: usize, replacement: &str) -> String {
    format!("{}{}{}", &content[..start], replacement, &content[end..])
}

fn remove_markdown_block(
    bytes: &[u8],
    block_id: &str,
    expected_hash: &str,
    leading_separator: &str,
) -> Result<Option<Vec<u8>>, String> {
    let content = match std::str::from_utf8(bytes) {
        Ok(content) => content,
        Err(_) => return Ok(None),
    };
    let Some((start, end)) = locate_markdown_block(content, block_id)? else {
        return Ok(None);
    };
    if !content.is_char_boundary(start) || !content.is_char_boundary(end) {
        return Ok(None);
    }
    if sha256(&content.as_bytes()[start..end]) != expected_hash {
        return Ok(None);
    }
    let Some(removal_start) = start.checked_sub(leading_separator.len()) else {
        return Ok(None);
    };
    if !content.is_char_boundary(removal_start) {
        return Ok(None);
    }
    if &content[removal_start..start] != leading_separator {
        return Ok(None);
    }
    Ok(Some(
        replace_range(content, removal_start, end, "").into_bytes(),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClaudeHandlerState {
    Absent,
    Exact,
    Conflict,
}

fn claude_handler() -> Value {
    json!({
        "type": "command",
        "command": CLAUDE_COMMAND,
        "description": CLAUDE_DESCRIPTION
    })
}

fn inspect_claude_handler(root: &Map<String, Value>) -> Result<ClaudeHandlerState, String> {
    validate_claude_hook_shape(root)?;
    let expected = claude_handler();
    let mut exact = 0usize;
    let mut conflict = false;
    if let Some(entries) = root
        .get("hooks")
        .and_then(Value::as_object)
        .and_then(|hooks| hooks.get("SessionStart"))
        .and_then(Value::as_array)
    {
        for entry in entries {
            let handlers = entry
                .get("hooks")
                .and_then(Value::as_array)
                .expect("validated SessionStart handler array");
            for handler in handlers {
                let object = handler
                    .as_object()
                    .expect("validated SessionStart handler object");
                let description = object.get("description").and_then(Value::as_str);
                let command = object.get("command").and_then(Value::as_str);
                let claims_description = description == Some(CLAUDE_DESCRIPTION);
                let claims_command = command.is_some_and(|command| {
                    command.contains("tree-ring")
                        && command.contains("integrations preflight")
                        && command.contains("--harness claude-code")
                });
                if claims_description || claims_command {
                    if handler == &expected {
                        exact += 1;
                    } else {
                        conflict = true;
                    }
                }
            }
        }
    }
    if conflict || exact > 1 {
        Ok(ClaudeHandlerState::Conflict)
    } else if exact == 1 {
        Ok(ClaudeHandlerState::Exact)
    } else {
        Ok(ClaudeHandlerState::Absent)
    }
}

fn validate_claude_hook_shape(root: &Map<String, Value>) -> Result<(), String> {
    let Some(hooks) = root.get("hooks") else {
        return Ok(());
    };
    let hooks = hooks
        .as_object()
        .ok_or_else(|| "Claude settings hooks must be an object".to_string())?;
    if let Some(session_start) = hooks.get("SessionStart") {
        let entries = session_start
            .as_array()
            .ok_or_else(|| "Claude SessionStart hooks must be an array".to_string())?;
        for entry in entries {
            let entry = entry
                .as_object()
                .ok_or_else(|| "Claude SessionStart entry must be an object".to_string())?;
            let handlers = entry
                .get("hooks")
                .and_then(Value::as_array)
                .ok_or_else(|| "Claude SessionStart entry hooks must be an array".to_string())?;
            if handlers.iter().any(|handler| !handler.is_object()) {
                return Err("Claude SessionStart command handler must be an object".to_string());
            }
        }
    }
    Ok(())
}

fn insert_claude_handler(root: &mut Map<String, Value>, handler: Value) -> Result<(), String> {
    if !root.contains_key("hooks") {
        root.insert("hooks".to_string(), Value::Object(Map::new()));
    }
    let hooks = root
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "Claude settings hooks must be an object".to_string())?;
    if !hooks.contains_key("SessionStart") {
        hooks.insert("SessionStart".to_string(), Value::Array(Vec::new()));
    }
    let session_start = hooks
        .get_mut("SessionStart")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "Claude SessionStart hooks must be an array".to_string())?;
    session_start.push(json!({"matcher": "", "hooks": [handler]}));
    Ok(())
}

fn remove_claude_handler(bytes: &[u8], expected_hash: &str) -> Result<Option<Vec<u8>>, String> {
    let mut root = match parse_json_object(bytes) {
        Ok(root) => root,
        Err(_) => return Ok(None),
    };
    if validate_claude_hook_shape(&root).is_err() {
        return Ok(None);
    }
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return Ok(None);
    };
    let Some(entries) = hooks.get_mut("SessionStart").and_then(Value::as_array_mut) else {
        return Ok(None);
    };
    let mut removed = 0usize;
    for entry in entries.iter_mut() {
        let Some(handlers) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
            continue;
        };
        handlers.retain(|handler| {
            let matches = handler.get("description").and_then(Value::as_str)
                == Some(CLAUDE_DESCRIPTION)
                && serde_json::to_vec(handler)
                    .map(|bytes| sha256(&bytes) == expected_hash)
                    .unwrap_or(false);
            if matches {
                removed += 1;
            }
            !matches
        });
    }
    if removed != 1 {
        return Ok(None);
    }
    entries.retain(|entry| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .is_none_or(|handlers| !handlers.is_empty())
    });
    if entries.is_empty() {
        hooks.remove("SessionStart");
    }
    if hooks.is_empty() {
        root.remove("hooks");
    }
    pretty_json(&Value::Object(root)).map(Some)
}

fn parse_json_object(bytes: &[u8]) -> Result<Map<String, Value>, String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("invalid Claude settings JSON: {error}"))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| "Claude settings root must be an object".to_string())
}

fn pretty_json(value: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(json_error)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn json_error(error: serde_json::Error) -> String {
    format!("failed to render bridge JSON: {error}")
}

fn upsert_owned_file(owned: &mut Vec<OwnedBridgeFile>, path: String, digest: String) {
    if let Some(existing) = owned.iter_mut().find(|owned| owned.path == path) {
        existing.sha256 = digest;
    } else {
        owned.push(OwnedBridgeFile {
            path,
            sha256: digest,
        });
    }
}

fn upsert_managed_block(
    owned: &mut Vec<OwnedManagedBlock>,
    path: String,
    block_id: String,
    digest: String,
    leading_separator: String,
) {
    if let Some(existing) = owned
        .iter_mut()
        .find(|owned| owned.path == path && owned.block_id == block_id)
    {
        existing.sha256 = digest;
        existing.leading_separator = leading_separator;
    } else {
        owned.push(OwnedManagedBlock {
            path,
            block_id,
            sha256: digest,
            leading_separator,
        });
    }
}

fn reconcile_manifest(
    project_fs: &ProjectFs,
    supplied: &ActivationManifest,
) -> Result<(ActivationManifest, Option<ActivationManifest>), String> {
    let persisted = load_persisted_manifest(project_fs)?;
    let Some(persisted_manifest) = persisted else {
        return Ok((supplied.clone(), None));
    };
    if persisted_manifest.schema_version != supplied.schema_version
        || persisted_manifest.protocol_version != supplied.protocol_version
        || persisted_manifest.store_id != supplied.store_id
        || persisted_manifest.project_root_fingerprint != supplied.project_root_fingerprint
    {
        return Err(
            "activation manifest identity changed while preparing bridge update".to_string(),
        );
    }
    Ok((persisted_manifest.clone(), Some(persisted_manifest)))
}

fn load_persisted_manifest(project_fs: &ProjectFs) -> Result<Option<ActivationManifest>, String> {
    let Some(bytes) = project_fs.read_optional(Path::new(".tree-ring/activation.json"))? else {
        return Ok(None);
    };
    let manifest: ActivationManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid activation JSON: {error}"))?;
    validate_manifest(&manifest)?;
    Ok(Some(manifest))
}

fn ensure_expected_manifest(
    project_fs: &ProjectFs,
    expected: Option<&ActivationManifest>,
) -> Result<(), String> {
    let current = load_persisted_manifest(project_fs)?;
    if current.as_ref() == expected {
        Ok(())
    } else {
        Err("activation manifest changed concurrently; bridge update was not committed".to_string())
    }
}

fn persist_manifest(
    project_fs: &ProjectFs,
    expected: Option<&ActivationManifest>,
    manifest: &ActivationManifest,
) -> Result<Option<AppliedFile>, CommitPreparedError> {
    validate_manifest(manifest).map_err(CommitPreparedError::before_publication)?;
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|error| {
        CommitPreparedError::before_publication(format!(
            "failed to serialize activation JSON: {error}"
        ))
    })?;
    let relative = PathBuf::from(".tree-ring/activation.json");
    let before = project_fs
        .read_optional(&relative)
        .map_err(CommitPreparedError::before_publication)?;
    let current = match before.as_deref() {
        Some(bytes) => Some(serde_json::from_slice::<ActivationManifest>(bytes).map_err(
            |error| {
                CommitPreparedError::before_publication(format!("invalid activation JSON: {error}"))
            },
        )?),
        None => None,
    };
    if current.as_ref() != expected {
        return Err(CommitPreparedError::before_publication(
            "activation manifest changed concurrently; bridge update was not committed".to_string(),
        ));
    }
    if before.is_some() {
        if current.as_ref() == Some(manifest) {
            // Preserve an already-valid manifest byte-for-byte. In particular,
            // do not rewrite a user-formatted but semantically unchanged file.
            return Ok(None);
        }
        return Err(CommitPreparedError::before_publication(
            "activation manifest already exists and cannot be safely replaced".to_string(),
        ));
    }
    let prepared = PreparedFile {
        relative,
        before,
        after: Some(bytes),
    };
    commit_prepared_file(project_fs, &prepared)
}

fn commit_files_and_manifest(
    project_fs: &ProjectFs,
    manifest: &mut ActivationManifest,
    original_manifest: &ActivationManifest,
    expected_persisted: Option<&ActivationManifest>,
    next_manifest: ActivationManifest,
    files: &[PreparedFile],
) -> Result<(), String> {
    project_fs.ensure_root_binding()?;
    ensure_expected_manifest(project_fs, expected_persisted)?;
    if files_require_existing_entry_mutation(files) {
        return Err(
            "bridge writer will not replace or remove an existing final entry; explicit review is required"
                .to_string(),
        );
    }
    if manifest_update_requires_replacement(expected_persisted, &next_manifest) {
        return Err(
            "activation manifest already exists and cannot be safely replaced; explicit review is required"
                .to_string(),
        );
    }
    let mut applied = Vec::new();
    for file in files.iter().filter(|file| file.before != file.after) {
        match commit_prepared_file(project_fs, file) {
            Ok(Some(applied_file)) => applied.push(applied_file),
            Ok(None) => {}
            Err(error) => {
                if let Some(applied_file) = error.applied {
                    applied.push(*applied_file);
                }
                if applied.is_empty() {
                    *manifest = original_manifest.clone();
                    return Err(error.message);
                }
                return Err(indeterminate_commit_error(
                    manifest,
                    original_manifest,
                    &next_manifest,
                    error.message,
                    &applied,
                ));
            }
        }
    }
    if let Err(error) = project_fs.ensure_root_binding() {
        if applied.is_empty() {
            *manifest = original_manifest.clone();
            return Err(error);
        }
        return Err(indeterminate_commit_error(
            manifest,
            original_manifest,
            &next_manifest,
            error,
            &applied,
        ));
    }
    let manifest_applied = match persist_manifest(project_fs, expected_persisted, &next_manifest) {
        Ok(applied_file) => applied_file,
        Err(error) => {
            if let Some(applied_file) = error.applied {
                applied.push(*applied_file);
            }
            if applied.is_empty() {
                *manifest = original_manifest.clone();
                return Err(error.message);
            }
            return Err(indeterminate_commit_error(
                manifest,
                original_manifest,
                &next_manifest,
                error.message,
                &applied,
            ));
        }
    };
    if let Err(error) = project_fs.ensure_root_binding() {
        if let Some(applied_file) = manifest_applied {
            applied.push(applied_file);
        }
        if applied.is_empty() {
            *manifest = original_manifest.clone();
            return Err(error);
        }
        return Err(indeterminate_commit_error(
            manifest,
            original_manifest,
            &next_manifest,
            error,
            &applied,
        ));
    }
    *manifest = next_manifest;
    Ok(())
}

fn commit_prepared_file(
    project_fs: &ProjectFs,
    file: &PreparedFile,
) -> Result<Option<AppliedFile>, CommitPreparedError> {
    if file.before == file.after {
        return Ok(None);
    }
    if file.before.is_some() || file.after.is_none() {
        return Err(CommitPreparedError::before_publication(format!(
            "bridge writer will not replace or remove an existing final entry: {}",
            file.relative.display()
        )));
    }
    project_fs
        .ensure_root_binding()
        .map_err(CommitPreparedError::before_publication)?;
    let target = project_fs
        .resolve_target(&file.relative, file.after.is_some())
        .map_err(CommitPreparedError::before_publication)?;
    let current = target
        .snapshot_optional()
        .map_err(CommitPreparedError::before_publication)?;
    if current.is_some() {
        return Err(CommitPreparedError::before_publication(format!(
            "bridge target already exists and cannot be safely replaced: {}",
            file.relative.display()
        )));
    }
    project_fs
        .ensure_root_binding()
        .map_err(CommitPreparedError::before_publication)?;
    after_final_root_binding();
    project_fs
        .ensure_root_binding()
        .map_err(CommitPreparedError::before_publication)?;
    let bytes = file
        .after
        .as_deref()
        .expect("creation-only file has desired bytes");
    let publication = target.publish_creation(bytes);
    let publication = match publication {
        Ok(publication) => publication,
        Err(error) => {
            let applied = error.publication.map(|publication| AppliedFile {
                target,
                publication,
            });
            return Err(CommitPreparedError {
                message: error.message,
                applied: applied.map(Box::new),
            });
        }
    };
    let applied = AppliedFile {
        target,
        publication,
    };
    if let Err(error) = applied.target.sync_parent() {
        return Err(CommitPreparedError::after_publication(error, applied));
    }
    Ok(Some(applied))
}

/// Never roll a published path back: no POSIX primitive can condition that
/// remove/replace on the exact published inode and bytes. A second directory
/// sync is non-destructive and lets us surface a separate recovery durability
/// failure without pretending the original in-memory manifest still matches
/// disk.
fn preserve_published_files(files: &[AppliedFile]) -> Result<(), String> {
    let failures = files
        .iter()
        .rev()
        .filter_map(|file| {
            file.target.sync_parent().err().map(|error| {
                format!(
                    "{} (published inode {}:{})",
                    error, file.publication.device, file.publication.inode
                )
            })
        })
        .collect::<Vec<_>>();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

fn indeterminate_manifest(
    original: &ActivationManifest,
    next: &ActivationManifest,
) -> ActivationManifest {
    let mut indeterminate = next.clone();
    for (harness_id, activation) in &mut indeterminate.harnesses {
        if original.harnesses.get(harness_id) != Some(activation) {
            activation.state = ActivationState::NeedsUserReview;
        }
    }
    indeterminate
}

fn indeterminate_commit_error(
    manifest: &mut ActivationManifest,
    original: &ActivationManifest,
    next: &ActivationManifest,
    primary_error: String,
    applied: &[AppliedFile],
) -> String {
    *manifest = indeterminate_manifest(original, next);
    match preserve_published_files(applied) {
        Ok(()) => format!(
            "{primary_error}; published bridge material was preserved because safe rollback is unavailable; bridge state is indeterminate and requires user review"
        ),
        Err(reconciliation_error) => format!(
            "{primary_error}; recovery durability check also failed: {reconciliation_error}; published bridge material was preserved and bridge state is indeterminate"
        ),
    }
}

fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn blocked_activation(
    plan: &AdapterPlan,
    current: Option<&HarnessActivation>,
) -> Result<HarnessActivation, String> {
    let (adapter_capability, adapter_version) = adapter_contract(&plan.harness_id)?;
    let mut activation = current.cloned().unwrap_or(HarnessActivation {
        state: plan.state,
        adapter_capability,
        adapter_version: adapter_version.to_string(),
        bridge_fingerprint: String::new(),
        bridge_path: None,
        owned_files: Vec::new(),
        managed_blocks: Vec::new(),
    });
    activation.state = plan.state;
    activation.adapter_capability = adapter_capability;
    activation.adapter_version = adapter_version.to_string();
    activation.bridge_fingerprint = bridge_fingerprint(&plan.harness_id, &activation);
    Ok(activation)
}

fn adapter_contract(harness_id: &str) -> Result<(AdapterCapability, &'static str), String> {
    let capability = adapter_capability(harness_id)
        .ok_or_else(|| format!("unknown harness adapter: {harness_id}"))?;
    let version = adapter_version(harness_id)
        .ok_or_else(|| format!("unknown harness adapter: {harness_id}"))?;
    Ok((capability, version))
}

fn applied_state(harness_id: &str, planned: ActivationState) -> ActivationState {
    if harness_id == "pi" {
        ActivationState::NeedsTrust
    } else {
        planned
    }
}

fn deactivated_state(harness_id: &str) -> ActivationState {
    if harness_id == "pi" {
        ActivationState::NeedsTrust
    } else {
        ActivationState::ConfiguredAwaitingProof
    }
}

fn review_step(harness_id: &str) -> String {
    if harness_id == "codex" {
        CODEX_RETRY.to_string()
    } else {
        format!("Review the existing {harness_id} bridge configuration before retrying activation.")
    }
}

/// There is no portable POSIX operation that says "replace/remove this final
/// pathname only if it is still this exact inode with these exact bytes".
/// Keep the bridge writer creation-only until such a primitive is available.
fn files_require_existing_entry_mutation(files: &[PreparedFile]) -> bool {
    files
        .iter()
        .any(|file| file.before != file.after && (file.before.is_some() || file.after.is_none()))
}

/// The activation manifest is also a final entry. Treat its replacement with
/// the same fail-closed boundary as a bridge file.
fn manifest_update_requires_replacement(
    persisted: Option<&ActivationManifest>,
    next: &ActivationManifest,
) -> bool {
    persisted.is_some_and(|current| current != next)
}

fn creation_only_review_result(harness_id: &str) -> BridgePlanResult {
    BridgePlanResult {
        state: ActivationState::NeedsUserReview,
        changed_paths: Vec::new(),
        next_step: format!(
            "Tree Ring will not replace or remove an existing {harness_id} bridge entry; review and reconcile it before retrying."
        ),
    }
}

fn next_manifest_ownership_empty(manifest: &ActivationManifest, harness_id: &str) -> bool {
    manifest.harnesses.get(harness_id).is_none_or(|activation| {
        activation.owned_files.is_empty() && activation.managed_blocks.is_empty()
    })
}

fn io_error(path: &Path, error: std::io::Error) -> String {
    format!("{}: {error}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activation::{
        adapters::{plan_activation, ActivationProject, AdapterPlan, BridgeWrite, PlannedWrite},
        manifest::{
            save_manifest, ActivationManifest, HarnessActivation, OwnedBridgeFile,
            OwnedManagedBlock,
        },
        ActivationState, AdapterCapability, ACTIVATION_PROTOCOL_VERSION, ACTIVATION_SCHEMA_VERSION,
    };
    use serde_json::{json, Value};
    use std::{
        collections::BTreeMap,
        fs,
        path::Path,
        sync::{Arc, Barrier},
        thread,
    };
    use tempfile::TempDir;

    fn fixture() -> (TempDir, ActivationProject, ActivationManifest) {
        let temp = tempfile::tempdir().unwrap();
        let project = ActivationProject::from_project_root(temp.path());
        fs::create_dir_all(&project.memory_root).unwrap();
        fs::write(
            project.memory_root.join("AGENTS.md"),
            "# Canonical guidance\n",
        )
        .unwrap();
        fs::write(project.memory_root.join("SKILL.md"), "# Canonical skill\n").unwrap();
        fs::write(project.memory_root.join("CLI.md"), "# Canonical CLI\n").unwrap();
        let manifest = ActivationManifest {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            protocol_version: ACTIVATION_PROTOCOL_VERSION,
            store_id: "store-test".to_string(),
            project_root_fingerprint: "a".repeat(64),
            cli_version: "0.14.0".to_string(),
            harnesses: BTreeMap::new(),
        };
        (temp, project, manifest)
    }

    fn read(path: impl AsRef<Path>) -> String {
        fs::read_to_string(path).unwrap()
    }

    fn write(path: impl AsRef<Path>, content: &str) {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn after_final_root_binding_for_test(action: impl FnOnce() + 'static) {
        TEST_MUTATION_HOOKS.with(|hooks| {
            hooks.borrow_mut().after_final_root_binding = Some(Box::new(action));
        });
    }

    fn after_final_target_validation_for_test(action: impl FnOnce() + 'static) {
        TEST_MUTATION_HOOKS.with(|hooks| {
            hooks.borrow_mut().after_final_target_validation = Some(Box::new(action));
        });
    }

    fn fail_directory_sync_at_for_test(number: usize) {
        fail_directory_syncs_at_for_test(&[number]);
    }

    fn fail_directory_syncs_at_for_test(numbers: &[usize]) {
        TEST_MUTATION_HOOKS.with(|hooks| {
            let mut hooks = hooks.borrow_mut();
            hooks.fail_directory_sync_at = numbers.to_vec();
            hooks.directory_syncs = 0;
        });
    }

    #[cfg(unix)]
    fn fail_fdopendir_for_test() {
        TEST_MUTATION_HOOKS.with(|hooks| hooks.borrow_mut().fail_fdopendir = true);
    }

    #[cfg(unix)]
    fn fail_readdir_for_test() {
        TEST_MUTATION_HOOKS.with(|hooks| hooks.borrow_mut().fail_readdir = true);
    }

    #[cfg(unix)]
    fn fail_closedir_for_test() {
        TEST_MUTATION_HOOKS.with(|hooks| hooks.borrow_mut().fail_closedir = true);
    }

    fn plan(harness: &str, project: &ActivationProject) -> AdapterPlan {
        plan_activation(harness, project).unwrap()
    }

    fn verified_agent_zero_plan() -> AdapterPlan {
        AdapterPlan {
            harness_id: "agent-zero".to_string(),
            state: ActivationState::ConfiguredAwaitingProof,
            writes: vec![PlannedWrite::BridgeWrite(BridgeWrite {
                path: ".tree-ring/activation/agent-zero.json".into(),
            })],
            next_step: "verified separate plugin".to_string(),
        }
    }

    #[test]
    fn unmanaged_agents_file_requires_explicit_review_without_partial_writes() {
        let (_temp, project, mut manifest) = fixture();
        write(project.project_root.join("AGENTS.md"), "# Team contract\n");

        let result =
            apply_bridge_plan(&project, &mut manifest, plan("codex", &project), false).unwrap();

        assert_eq!(result.state, ActivationState::NeedsUserReview);
        assert_eq!(
            read(project.project_root.join("AGENTS.md")),
            "# Team contract\n"
        );
        assert!(!project
            .project_root
            .join(".agents/skills/tree-ring-memory/SKILL.md")
            .exists());
        assert_eq!(
            result.next_step,
            "tree-ring integrations activate --harness codex --accept-managed-block"
        );
        assert!(manifest.harnesses.is_empty());
    }

    #[test]
    fn absent_agents_file_becomes_hash_recorded_owned_file_and_retries_idempotently() {
        let (_temp, project, mut manifest) = fixture();
        let codex = plan("codex", &project);

        let first = apply_bridge_plan(&project, &mut manifest, codex.clone(), false).unwrap();
        let agents = read(project.project_root.join("AGENTS.md"));
        let skill = read(
            project
                .project_root
                .join(".agents/skills/tree-ring-memory/SKILL.md"),
        );
        let serialized = serde_json::to_value(&manifest).unwrap();
        let second = apply_bridge_plan(&project, &mut manifest, codex, false).unwrap();

        assert_eq!(first.state, ActivationState::ConfiguredAwaitingProof);
        assert_eq!(second.state, ActivationState::ConfiguredAwaitingProof);
        assert!(second.changed_paths.is_empty());
        assert_eq!(
            agents.matches("<!-- tree-ring:begin codex v1 -->").count(),
            1
        );
        assert!(skill.contains(".tree-ring/SKILL.md"));
        assert_eq!(
            serialized["harnesses"]["codex"]["owned_files"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        for owned in serialized["harnesses"]["codex"]["owned_files"]
            .as_array()
            .unwrap()
        {
            assert_eq!(owned["sha256"].as_str().unwrap().len(), 64);
        }
    }

    #[test]
    fn init_batch_publishes_all_safe_adapters_in_one_deterministic_manifest() {
        let (_temp, project, mut manifest) = fixture();
        let plans = vec![
            plan("pi", &project),
            plan("codex", &project),
            plan("claude-code", &project),
        ];

        let first = apply_bridge_plans_create_only(&project, &mut manifest, plans.clone()).unwrap();

        assert_eq!(
            first
                .iter()
                .map(|outcome| outcome.harness_id.as_str())
                .collect::<Vec<_>>(),
            ["claude-code", "codex", "pi"]
        );
        assert_eq!(
            manifest
                .harnesses
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["claude-code", "codex", "pi"]
        );
        assert_eq!(manifest.harnesses["pi"].state, ActivationState::NeedsTrust);
        let shared = ".agents/skills/tree-ring-memory/SKILL.md";
        assert!(manifest.harnesses["codex"]
            .owned_files
            .iter()
            .any(|owned| owned.path == shared));
        assert!(manifest.harnesses["pi"]
            .owned_files
            .iter()
            .any(|owned| owned.path == shared));
        assert_eq!(
            crate::activation::load_manifest(&project.memory_root).unwrap(),
            manifest
        );

        let bytes = fs::read(project.memory_root.join("activation.json")).unwrap();
        let second = apply_bridge_plans_create_only(&project, &mut manifest, plans).unwrap();
        assert!(second
            .iter()
            .all(|outcome| outcome.result.changed_paths.is_empty()));
        assert_eq!(
            fs::read(project.memory_root.join("activation.json")).unwrap(),
            bytes
        );
    }

    #[test]
    fn empty_init_batch_still_creation_publishes_the_store_manifest() {
        let (_temp, project, mut manifest) = fixture();

        let outcomes = apply_bridge_plans_create_only(&project, &mut manifest, Vec::new()).unwrap();

        assert!(outcomes.is_empty());
        assert_eq!(
            crate::activation::load_manifest(&project.memory_root).unwrap(),
            manifest
        );
    }

    #[test]
    fn init_batch_never_extends_a_preexisting_partial_manifest() {
        let (_temp, project, mut manifest) = fixture();
        apply_bridge_plan(&project, &mut manifest, plan("codex", &project), false).unwrap();
        let manifest_bytes = fs::read(project.memory_root.join("activation.json")).unwrap();

        let outcomes = apply_bridge_plans_create_only(
            &project,
            &mut manifest,
            vec![
                plan("pi", &project),
                plan("codex", &project),
                plan("claude-code", &project),
            ],
        )
        .unwrap();

        assert_eq!(outcomes[1].harness_id, "codex");
        assert_eq!(
            outcomes[1].result.state,
            ActivationState::ConfiguredAwaitingProof
        );
        for harness in ["claude-code", "pi"] {
            assert_eq!(
                outcomes
                    .iter()
                    .find(|outcome| outcome.harness_id == harness)
                    .unwrap()
                    .result
                    .state,
                ActivationState::NeedsUserReview
            );
        }
        assert_eq!(
            fs::read(project.memory_root.join("activation.json")).unwrap(),
            manifest_bytes
        );
        assert!(!project
            .project_root
            .join(".claude/skills/tree-ring-memory/SKILL.md")
            .exists());
        assert!(!project
            .project_root
            .join(".pi/extensions/tree-ring-memory.ts")
            .exists());
    }

    #[test]
    fn init_batch_rejects_duplicate_adapter_plans_before_writing() {
        let (_temp, project, mut manifest) = fixture();
        let codex = plan("codex", &project);

        let error =
            apply_bridge_plans_create_only(&project, &mut manifest, vec![codex.clone(), codex])
                .unwrap_err();

        assert_eq!(error, "duplicate harness plan in activation batch");
        assert!(!project.memory_root.join("activation.json").exists());
        assert!(manifest.harnesses.is_empty());
    }

    #[test]
    fn init_batch_late_durability_failure_marks_every_staged_adapter_for_review() {
        let (_temp, project, mut manifest) = fixture();
        fail_directory_sync_at_for_test(2);

        let error = apply_bridge_plans_create_only(
            &project,
            &mut manifest,
            vec![
                plan("codex", &project),
                plan("claude-code", &project),
                plan("pi", &project),
            ],
        )
        .unwrap_err();

        assert!(error.contains("injected directory sync failure"));
        assert!(error.contains("indeterminate"));
        assert!(!project.memory_root.join("activation.json").exists());
        for harness in ["claude-code", "codex", "pi"] {
            assert_eq!(
                manifest.harnesses[harness].state,
                ActivationState::NeedsUserReview
            );
        }
    }

    #[test]
    fn isolated_preflight_roots_require_two_valid_project_local_manifests() {
        let temp = tempfile::tempdir().unwrap();
        let canonical_project = ActivationProject::from_project_root(temp.path().join("canonical"));
        let selected_project = ActivationProject::from_project_root(temp.path().join("selected"));
        for project in [&canonical_project, &selected_project] {
            fs::create_dir_all(&project.memory_root).unwrap();
        }
        let make_manifest = |project: &ActivationProject, store_id: &str| ActivationManifest {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            protocol_version: ACTIVATION_PROTOCOL_VERSION,
            store_id: store_id.to_string(),
            project_root_fingerprint: fingerprint_path(&project.project_root),
            cli_version: "0.14.0".to_string(),
            harnesses: BTreeMap::new(),
        };
        let canonical_manifest = make_manifest(&canonical_project, "canonical-store");
        let selected_manifest = make_manifest(&selected_project, "selected-store");
        save_manifest(&canonical_project.memory_root, &canonical_manifest).unwrap();
        save_manifest(&selected_project.memory_root, &selected_manifest).unwrap();

        assert_eq!(
            validate_isolated_preflight_roots(
                &selected_project.memory_root,
                &canonical_project.project_root,
            )
            .unwrap(),
            selected_manifest
        );
        assert_eq!(
            validate_isolated_preflight_roots(
                &selected_project.project_root,
                &canonical_project.project_root,
            )
            .unwrap_err(),
            "isolated preflight roots are invalid"
        );
    }

    #[test]
    fn accepted_agents_block_requires_review_when_the_final_file_already_exists() {
        let (_temp, project, mut manifest) = fixture();
        write(project.project_root.join("AGENTS.md"), "# Team contract\n");

        let result =
            apply_bridge_plan(&project, &mut manifest, plan("codex", &project), true).unwrap();

        assert_eq!(result.state, ActivationState::NeedsUserReview);
        assert_eq!(
            read(project.project_root.join("AGENTS.md")),
            "# Team contract\n"
        );
        assert!(manifest.harnesses.is_empty());
    }

    #[test]
    fn changed_owned_file_is_never_overwritten_on_retry() {
        let (_temp, project, mut manifest) = fixture();
        apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap();
        let extension = project
            .project_root
            .join(".pi/extensions/tree-ring-memory.ts");
        write(&extension, "// owner changed this file\n");

        let result =
            apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap();

        assert_eq!(result.state, ActivationState::NeedsUserReview);
        assert_eq!(read(extension), "// owner changed this file\n");
    }

    #[test]
    fn claude_settings_merge_is_planned_but_existing_json_is_never_replaced() {
        let (_temp, project, mut manifest) = fixture();
        let settings_path = project.project_root.join(".claude/settings.json");
        let original = json!({
            "permissions": {"allow": ["Read", "Bash(git status)"]},
            "hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "guard"}]}]},
            "custom": [1, {"nested": true}]
        });
        write(
            &settings_path,
            &serde_json::to_string_pretty(&original).unwrap(),
        );

        let result = apply_bridge_plan(
            &project,
            &mut manifest,
            plan("claude-code", &project),
            false,
        )
        .unwrap();

        assert_eq!(result.state, ActivationState::NeedsUserReview);
        assert_eq!(
            serde_json::from_str::<Value>(&read(&settings_path)).unwrap(),
            original
        );
        assert!(!project
            .project_root
            .join(".claude/skills/tree-ring-memory/SKILL.md")
            .exists());
    }

    #[test]
    fn invalid_or_conflicting_claude_settings_require_review_without_any_write() {
        for contents in [
            "not json".to_string(),
            "[]".to_string(),
            serde_json::to_string(&json!({"hooks": 5})).unwrap(),
            serde_json::to_string(&json!({
                "hooks": {"SessionStart": [{"hooks": [{
                    "type": "command",
                    "command": "some-other-command",
                    "description": "Tree Ring Memory managed preflight v1"
                }]}]}
            }))
            .unwrap(),
        ] {
            let (_temp, project, mut manifest) = fixture();
            let settings_path = project.project_root.join(".claude/settings.json");
            write(&settings_path, &contents);

            let result = apply_bridge_plan(
                &project,
                &mut manifest,
                plan("claude-code", &project),
                false,
            )
            .unwrap();

            assert_eq!(result.state, ActivationState::NeedsUserReview);
            assert_eq!(read(settings_path), contents);
            assert!(!project
                .project_root
                .join(".claude/skills/tree-ring-memory/SKILL.md")
                .exists());
            assert!(manifest.harnesses.is_empty());
        }
    }

    #[test]
    fn preview_is_a_zero_write_dry_run() {
        let (_temp, project, manifest) = fixture();
        let before = serde_json::to_value(&manifest).unwrap();
        let pi = plan("pi", &project);

        assert_eq!(pi.state, ActivationState::NeedsTrust);
        let result = preview_bridge_plan(&project, &manifest, pi, false).unwrap();

        assert_eq!(result.state, ActivationState::NeedsTrust);
        assert_eq!(result.changed_paths.len(), 2);
        assert_eq!(serde_json::to_value(&manifest).unwrap(), before);
        assert!(!project.project_root.join(".agents").exists());
        assert!(!project.project_root.join(".pi").exists());
    }

    #[test]
    fn pi_extension_uses_stdin_fallback_and_returns_non_display_context() {
        let (_temp, project, mut manifest) = fixture();

        let result =
            apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap();
        let source = read(
            project
                .project_root
                .join(".pi/extensions/tree-ring-memory.ts"),
        );

        assert_eq!(result.state, ActivationState::NeedsTrust);
        assert!(source.contains("pi.on(\"before_agent_start\", async (event, ctx) =>"));
        assert!(source.contains("task_hint: event.prompt"));
        assert!(source.contains("child.stdin.end(input)"));
        assert!(source.contains("task_hint: \"project startup constraints\""));
        assert!(source.contains("customType: \"tree-ring-preflight\""));
        assert!(source.contains("content: response.context"));
        assert!(source.contains("display: false"));
        assert!(!source.contains("npm install"));
        assert!(!source.contains("detached: true"));
    }

    #[test]
    fn agent_zero_requires_verified_plugin_plan_and_writes_only_protocol_binding() {
        let (_temp, project, mut manifest) = fixture();
        let blocked = plan("agent-zero", &project);

        let blocked_result = apply_bridge_plan(&project, &mut manifest, blocked, false).unwrap();
        assert_eq!(blocked_result.state, ActivationState::NeedsPlugin);
        assert!(blocked_result.changed_paths.is_empty());
        let blocked_activation = manifest.harnesses.get("agent-zero").unwrap();
        assert_eq!(
            blocked_activation.adapter_capability,
            adapter_capability("agent-zero").unwrap()
        );
        assert_eq!(
            blocked_activation.adapter_version,
            adapter_version("agent-zero").unwrap()
        );
        assert!(!project
            .project_root
            .join(".tree-ring/activation/agent-zero.json")
            .exists());

        let (_temp, project, mut manifest) = fixture();
        apply_bridge_plan(&project, &mut manifest, verified_agent_zero_plan(), false).unwrap();
        let activation = manifest.harnesses.get("agent-zero").unwrap();
        assert_eq!(activation.adapter_version, "1");
        assert_eq!(
            activation.bridge_fingerprint,
            bridge_fingerprint("agent-zero", activation)
        );
        let binding: Value = serde_json::from_str(&read(
            project
                .project_root
                .join(".tree-ring/activation/agent-zero.json"),
        ))
        .unwrap();

        assert_eq!(binding["protocol_version"], ACTIVATION_PROTOCOL_VERSION);
        assert_eq!(binding["store_id"], "store-test");
        assert_eq!(binding["project_root_fingerprint"], "a".repeat(64));
        assert_eq!(binding["memory_root"], ".tree-ring");
        assert_eq!(binding["command_protocol"]["stdin"], "json");
        assert_eq!(binding["command_protocol"]["stdout"], "json");
        assert!(!project.project_root.join(".a0").exists());
    }

    #[test]
    fn creation_only_review_preserves_non_tree_ring_text_canonical_store_and_receipts() {
        let (_temp, project, mut manifest) = fixture();
        write(project.project_root.join("AGENTS.md"), "# Team contract\n");
        let receipt_path = project
            .memory_root
            .join("activation/receipts/codex/worker/receipt.json");
        write(&receipt_path, "{}\n");

        let result =
            apply_bridge_plan(&project, &mut manifest, plan("codex", &project), true).unwrap();

        assert_eq!(result.state, ActivationState::NeedsUserReview);
        assert_eq!(
            read(project.project_root.join("AGENTS.md")),
            "# Team contract\n"
        );
        assert!(receipt_path.exists());
        assert!(project.memory_root.join("AGENTS.md").exists());
        assert!(project.memory_root.join("SKILL.md").exists());
        assert!(project.memory_root.join("CLI.md").exists());
        assert!(!project.memory_root.join("activation.json").exists());
        assert!(!project
            .project_root
            .join(".agents/skills/tree-ring-memory/SKILL.md")
            .exists());
        assert!(manifest.harnesses.is_empty());
    }

    #[test]
    fn deactivation_never_deletes_changed_owned_or_unrecorded_lookalike_files() {
        let (_source_temp, source_project, mut source_manifest) = fixture();
        apply_bridge_plan(
            &source_project,
            &mut source_manifest,
            plan("pi", &source_project),
            false,
        )
        .unwrap();
        let generated = read(
            source_project
                .project_root
                .join(".pi/extensions/tree-ring-memory.ts"),
        );

        let (_temp, project, mut manifest) = fixture();
        let extension = project
            .project_root
            .join(".pi/extensions/tree-ring-memory.ts");
        write(&extension, &generated);
        deactivate_bridge_plan(&project, &mut manifest, "pi").unwrap();
        assert_eq!(read(&extension), generated);

        let review =
            apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap();
        assert_eq!(review.state, ActivationState::NeedsUserReview);
        fs::remove_file(&extension).unwrap();
        apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap();
        write(&extension, "// user changed owned bridge\n");
        deactivate_bridge_plan(&project, &mut manifest, "pi").unwrap();
        assert_eq!(read(extension), "// user changed owned bridge\n");
    }

    #[test]
    fn creation_only_deactivation_preserves_recorded_bridge_material_for_review() {
        let (_temp, project, mut manifest) = fixture();
        apply_bridge_plan(&project, &mut manifest, plan("codex", &project), false).unwrap();

        let result = deactivate_bridge_plan(&project, &mut manifest, "codex").unwrap();

        assert_eq!(result.state, ActivationState::NeedsUserReview);
        assert!(project
            .project_root
            .join(".agents/skills/tree-ring-memory/SKILL.md")
            .exists());
        assert!(project.project_root.join("AGENTS.md").exists());
        assert!(!manifest.harnesses["codex"].owned_files.is_empty());
    }

    #[test]
    fn claude_deactivation_removes_only_the_exact_recorded_handler() {
        let (_temp, project, mut manifest) = fixture();
        let settings_path = project.project_root.join(".claude/settings.json");
        let original = json!({
            "permissions": {"allow": ["Read"]},
            "hooks": {"SessionStart": [{"matcher": "compact", "hooks": [{"type": "command", "command": "keep-me"}]}]}
        });
        write(
            &settings_path,
            &serde_json::to_string_pretty(&original).unwrap(),
        );
        apply_bridge_plan(
            &project,
            &mut manifest,
            plan("claude-code", &project),
            false,
        )
        .unwrap();

        let result = deactivate_bridge_plan(&project, &mut manifest, "claude-code").unwrap();
        let after: Value = serde_json::from_str(&read(settings_path)).unwrap();

        assert_eq!(result.state, ActivationState::ConfiguredAwaitingProof);
        assert_eq!(after, original);
        assert!(!project
            .project_root
            .join(".claude/skills/tree-ring-memory/SKILL.md")
            .exists());
    }

    #[test]
    fn unsafe_or_unexpected_plan_paths_are_rejected_before_writes() {
        let (_temp, project, mut manifest) = fixture();
        for path in ["../outside", "/tmp/outside", "AGENTS.md"] {
            let malicious = AdapterPlan {
                harness_id: "pi".to_string(),
                state: ActivationState::ConfiguredAwaitingProof,
                writes: vec![PlannedWrite::BridgeWrite(BridgeWrite { path: path.into() })],
                next_step: String::new(),
            };
            assert!(apply_bridge_plan(&project, &mut manifest, malicious, false).is_err());
        }
        assert!(!project.project_root.join(".agents").exists());
        assert!(!Path::new("/tmp/outside").exists());
    }

    #[cfg(unix)]
    #[test]
    fn project_local_manifest_never_follows_a_tree_ring_symlink() {
        use std::os::unix::fs::symlink;

        let project_temp = tempfile::tempdir().unwrap();
        let outside_temp = tempfile::tempdir().unwrap();
        symlink(outside_temp.path(), project_temp.path().join(".tree-ring")).unwrap();
        let project = ActivationProject::from_project_root(project_temp.path());
        let mut manifest = ActivationManifest {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            protocol_version: ACTIVATION_PROTOCOL_VERSION,
            store_id: "store-test".to_string(),
            project_root_fingerprint: "a".repeat(64),
            cli_version: "0.14.0".to_string(),
            harnesses: BTreeMap::new(),
        };

        let error =
            apply_bridge_plan(&project, &mut manifest, plan("codex", &project), false).unwrap_err();

        assert!(error.contains("symlink"));
        assert!(!outside_temp.path().join("activation.json").exists());
        assert!(!project_temp.path().join("AGENTS.md").exists());
    }

    #[test]
    fn malformed_manifest_ownership_can_never_delete_non_adapter_or_store_material() {
        for relative in [
            ".tree-ring/AGENTS.md",
            ".tree-ring/SKILL.md",
            ".tree-ring/CLI.md",
            ".tree-ring/activation/receipts/codex/worker/receipt.json",
            "unrelated.txt",
        ] {
            let (_temp, project, mut manifest) = fixture();
            let path = project.project_root.join(relative);
            write(&path, "must survive\n");
            manifest.harnesses.insert(
                "codex".to_string(),
                HarnessActivation {
                    state: ActivationState::ConfiguredAwaitingProof,
                    adapter_capability: AdapterCapability::WrapperPreflight,
                    adapter_version: "1".to_string(),
                    bridge_fingerprint: String::new(),
                    bridge_path: Some(relative.to_string()),
                    owned_files: vec![OwnedBridgeFile {
                        path: relative.to_string(),
                        sha256: sha256(b"must survive\n"),
                    }],
                    managed_blocks: Vec::new(),
                },
            );

            let error = deactivate_bridge_plan(&project, &mut manifest, "codex").unwrap_err();

            assert!(error.contains("ownership") || error.contains("bridge"));
            assert_eq!(read(path), "must survive\n", "{relative}");
        }

        let (_temp, project, mut manifest) = fixture();
        manifest.harnesses.insert(
            "codex".to_string(),
            HarnessActivation {
                state: ActivationState::ConfiguredAwaitingProof,
                adapter_capability: AdapterCapability::WrapperPreflight,
                adapter_version: "1".to_string(),
                bridge_fingerprint: String::new(),
                bridge_path: Some(".tree-ring/AGENTS.md".to_string()),
                owned_files: Vec::new(),
                managed_blocks: vec![OwnedManagedBlock {
                    path: ".tree-ring/AGENTS.md".to_string(),
                    block_id: "codex".to_string(),
                    sha256: "a".repeat(64),
                    leading_separator: String::new(),
                }],
            },
        );
        assert!(deactivate_bridge_plan(&project, &mut manifest, "codex").is_err());
        assert!(project.memory_root.join("AGENTS.md").exists());
    }

    #[test]
    fn agent_zero_missing_plugin_preserves_the_owned_binding_when_manifest_replacement_is_refused()
    {
        let (_temp, project, mut manifest) = fixture();
        apply_bridge_plan(&project, &mut manifest, verified_agent_zero_plan(), false).unwrap();
        let binding = project
            .project_root
            .join(".tree-ring/activation/agent-zero.json");
        let ownership = manifest.harnesses["agent-zero"].owned_files.clone();

        let result =
            apply_bridge_plan(&project, &mut manifest, plan("agent-zero", &project), false)
                .unwrap();

        assert_eq!(result.state, ActivationState::NeedsUserReview);
        assert!(binding.exists());
        assert_eq!(manifest.harnesses["agent-zero"].owned_files, ownership);
        assert_eq!(
            manifest.harnesses["agent-zero"].bridge_path.as_deref(),
            Some(".tree-ring/activation/agent-zero.json")
        );
    }

    #[test]
    fn missing_formerly_owned_agents_and_claude_settings_require_review_without_recreation() {
        let (_temp, project, mut manifest) = fixture();
        apply_bridge_plan(&project, &mut manifest, plan("codex", &project), false).unwrap();
        fs::remove_file(project.project_root.join("AGENTS.md")).unwrap();
        let result =
            apply_bridge_plan(&project, &mut manifest, plan("codex", &project), false).unwrap();
        assert_eq!(result.state, ActivationState::NeedsUserReview);
        assert!(!project.project_root.join("AGENTS.md").exists());

        let (_temp, project, mut manifest) = fixture();
        apply_bridge_plan(
            &project,
            &mut manifest,
            plan("claude-code", &project),
            false,
        )
        .unwrap();
        fs::remove_file(project.project_root.join(".claude/settings.json")).unwrap();
        let result = apply_bridge_plan(
            &project,
            &mut manifest,
            plan("claude-code", &project),
            false,
        )
        .unwrap();
        assert_eq!(result.state, ActivationState::NeedsUserReview);
        assert!(!project.project_root.join(".claude/settings.json").exists());
    }

    #[test]
    fn claude_only_plans_handlers_in_the_exact_session_start_hook_location() {
        let (_temp, project, mut manifest) = fixture();
        let settings = project.project_root.join(".claude/settings.json");
        write(
            &settings,
            &serde_json::to_string_pretty(&json!({
                "custom": claude_handler(),
                "hooks": {"SessionStart": []}
            }))
            .unwrap(),
        );

        let result = apply_bridge_plan(
            &project,
            &mut manifest,
            plan("claude-code", &project),
            false,
        )
        .unwrap();
        let merged: Value = serde_json::from_str(&read(&settings)).unwrap();

        assert_eq!(result.state, ActivationState::NeedsUserReview);
        assert_eq!(merged["custom"], claude_handler());
        assert_eq!(merged["hooks"]["SessionStart"].as_array().unwrap().len(), 0);

        let (_temp, project, mut manifest) = fixture();
        let settings = project.project_root.join(".claude/settings.json");
        let malformed = serde_json::to_string(&json!({
            "custom": claude_handler(),
            "hooks": {"SessionStart": [{"hooks": "not-an-array"}]}
        }))
        .unwrap();
        write(&settings, &malformed);
        let result = apply_bridge_plan(
            &project,
            &mut manifest,
            plan("claude-code", &project),
            false,
        )
        .unwrap();
        assert_eq!(result.state, ActivationState::NeedsUserReview);
        assert_eq!(read(settings), malformed);
    }

    #[test]
    fn concurrent_disjoint_activations_preserve_the_published_manifest_and_review_the_second() {
        let (_temp, project, manifest) = fixture();
        let barrier = Arc::new(Barrier::new(2));
        let mut workers = Vec::new();
        for harness in ["codex", "pi"] {
            let project = project.clone();
            let mut worker_manifest = manifest.clone();
            let barrier = Arc::clone(&barrier);
            workers.push(thread::spawn(move || {
                let adapter_plan = plan(harness, &project);
                barrier.wait();
                apply_bridge_plan(&project, &mut worker_manifest, adapter_plan, false).unwrap()
            }));
        }
        let states = workers
            .into_iter()
            .map(|worker| worker.join().unwrap().state)
            .collect::<Vec<_>>();

        let persisted = crate::activation::load_manifest(&project.memory_root).unwrap();
        assert_eq!(persisted.harnesses.len(), 1);
        assert!(states.contains(&ActivationState::NeedsUserReview));
    }

    #[cfg(unix)]
    #[test]
    fn creation_only_publication_stays_on_the_retained_parent_descriptor() {
        use std::os::unix::fs::symlink;

        let (_temp, project, _manifest) = fixture();
        fs::create_dir_all(project.project_root.join(".agents/skills/tree-ring-memory")).unwrap();
        let project_fs = ProjectFs::open(&project).unwrap();
        let target = project_fs
            .resolve_target(Path::new(".agents/skills/tree-ring-memory/SKILL.md"), false)
            .unwrap();
        let parked = project.project_root.join(".agents-parked");
        fs::rename(project.project_root.join(".agents"), &parked).unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), project.project_root.join(".agents")).unwrap();

        target.publish_creation(b"tree-ring\n").unwrap();
        target.sync_parent().unwrap();

        assert!(!outside
            .path()
            .join("skills/tree-ring-memory/SKILL.md")
            .exists());
        assert_eq!(
            read(parked.join("skills/tree-ring-memory/SKILL.md")),
            "tree-ring\n"
        );
    }

    #[test]
    fn pi_deactivation_keeps_needs_trust_when_creation_only_safety_requires_review() {
        let (_temp, project, mut manifest) = fixture();
        let absent = deactivate_bridge_plan(&project, &mut manifest, "pi").unwrap();
        assert_eq!(absent.state, ActivationState::NeedsTrust);

        apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap();
        let deactivated = deactivate_bridge_plan(&project, &mut manifest, "pi").unwrap();
        assert_eq!(deactivated.state, ActivationState::NeedsUserReview);
        assert_eq!(manifest.harnesses["pi"].state, ActivationState::NeedsTrust);
    }

    #[test]
    fn codex_block_removal_keeps_preexisting_trailing_blank_lines_exactly() {
        for original in ["# Team\n", "# Team\n\n", "# Team\n\n\n"] {
            let block = markdown_block("codex");
            let (with_block, separator) = append_markdown_block(original, &block);
            let removed = remove_markdown_block(
                with_block.as_bytes(),
                "codex",
                &sha256(block.as_bytes()),
                &separator,
            )
            .unwrap()
            .unwrap();
            assert_eq!(String::from_utf8(removed).unwrap(), original);
        }
    }

    #[test]
    fn markdown_removal_rejects_separator_offset_inside_utf8_character() {
        let block = markdown_block("codex");
        let content = format!("é{block}");

        let removed =
            remove_markdown_block(content.as_bytes(), "codex", &sha256(block.as_bytes()), "x")
                .unwrap();

        assert!(removed.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn directory_listing_preserves_fdopendir_error_when_cleanup_succeeds() {
        let directory = tempfile::tempdir().unwrap();
        let file = File::open(directory.path()).unwrap();
        fail_fdopendir_for_test();

        let error = list_directory_entries(&file).unwrap_err();

        assert!(error.contains("Input/output error"));
        assert!(!error.contains("descriptor cleanup also failed"));
    }

    #[cfg(unix)]
    #[test]
    fn directory_listing_resets_errno_and_propagates_read_and_close_errors() {
        let directory = tempfile::tempdir().unwrap();
        let file = File::open(directory.path()).unwrap();

        set_errno(libc::EIO);
        assert!(list_directory_entries(&file).unwrap().is_empty());

        fail_readdir_for_test();
        assert!(list_directory_entries(&file)
            .unwrap_err()
            .contains("Input/output error"));

        fail_closedir_for_test();
        assert!(list_directory_entries(&file)
            .unwrap_err()
            .contains("Input/output error"));

        fail_readdir_for_test();
        fail_closedir_for_test();
        assert!(list_directory_entries(&file)
            .unwrap_err()
            .contains("directory close also failed"));
    }

    #[cfg(unix)]
    #[test]
    fn final_entry_replacement_at_create_publication_is_never_followed_or_overwritten() {
        use std::os::unix::fs::symlink;

        let (_temp, project, _manifest) = fixture();
        let target_path = project.project_root.join("managed-target");
        let outside = tempfile::tempdir().unwrap();
        let outside_marker = outside.path().join("must-not-change");
        write(&outside_marker, "outside\n");
        let project_fs = ProjectFs::open(&project).unwrap();
        let prepared = PreparedFile {
            relative: PathBuf::from("managed-target"),
            before: None,
            after: Some(b"tree-ring\n".to_vec()),
        };
        let replacement = target_path.clone();
        let outside_path = outside_marker.clone();
        after_final_target_validation_for_test(move || {
            symlink(&outside_path, &replacement).unwrap();
        });

        let error = commit_prepared_file(&project_fs, &prepared).unwrap_err();

        assert!(error.message.contains("File exists"));
        assert!(fs::symlink_metadata(&target_path)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(read(outside_marker), "outside\n");
    }

    #[test]
    fn existing_entry_replacement_and_removal_are_refused_before_publication() {
        let (_temp, project, _manifest) = fixture();
        let target_path = project.project_root.join("managed-target");
        write(&target_path, "before\n");
        let project_fs = ProjectFs::open(&project).unwrap();
        for after in [Some(b"tree-ring\n".to_vec()), None] {
            let error = commit_prepared_file(
                &project_fs,
                &PreparedFile {
                    relative: PathBuf::from("managed-target"),
                    before: Some(b"before\n".to_vec()),
                    after,
                },
            )
            .unwrap_err();
            assert!(error.message.contains("will not replace or remove"));
            assert_eq!(read(&target_path), "before\n");
        }
    }

    #[cfg(unix)]
    #[test]
    fn root_swap_after_final_binding_aborts_before_create_publication() {
        use std::os::unix::fs::symlink;

        let (_temp, project, _manifest) = fixture();
        let root = project.project_root.clone();
        let parked = root
            .parent()
            .unwrap()
            .join(format!("tree-ring-parked-{}", Uuid::new_v4()));
        let parked_for_hook = parked.clone();
        let outside = tempfile::tempdir().unwrap();
        let outside_path = outside.path().to_path_buf();
        let project_fs = ProjectFs::open(&project).unwrap();
        let prepared = PreparedFile {
            relative: PathBuf::from("root-race-target"),
            before: None,
            after: Some(b"tree-ring\n".to_vec()),
        };
        after_final_root_binding_for_test(move || {
            fs::rename(&root, &parked_for_hook).unwrap();
            symlink(&outside_path, &root).unwrap();
        });

        let error = commit_prepared_file(&project_fs, &prepared).unwrap_err();

        assert!(error.message.contains("project root changed"));
        assert!(!outside.path().join("root-race-target").exists());
        assert!(!parked.join("root-race-target").exists());
        fs::remove_file(&project.project_root).unwrap();
        fs::remove_dir_all(parked).unwrap();
    }

    #[test]
    fn preservation_after_publication_never_overwrites_a_user_edit() {
        let (_temp, project, _manifest) = fixture();
        let target_path = project.project_root.join("created-target");
        let project_fs = ProjectFs::open(&project).unwrap();
        let prepared = PreparedFile {
            relative: PathBuf::from("created-target"),
            before: None,
            after: Some(b"tree-ring\n".to_vec()),
        };
        let applied = commit_prepared_file(&project_fs, &prepared)
            .unwrap()
            .expect("the file was created");
        write(&target_path, "user edit\n");

        preserve_published_files(&[applied]).unwrap();

        assert_eq!(read(target_path), "user edit\n");
    }

    #[test]
    fn bridge_sync_failure_preserves_publication_and_marks_manifest_indeterminate() {
        let (_temp, project, mut manifest) = fixture();
        fail_directory_sync_at_for_test(1);

        let error =
            apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap_err();

        assert!(error.contains("injected directory sync failure"));
        assert!(error.contains("indeterminate"));
        assert!(project
            .project_root
            .join(".agents/skills/tree-ring-memory/SKILL.md")
            .exists());
        assert_eq!(
            manifest.harnesses["pi"].state,
            ActivationState::NeedsUserReview
        );
        assert!(!project.memory_root.join("activation.json").exists());
    }

    #[test]
    fn bridge_sync_and_recovery_sync_failures_are_both_reported_without_resetting_manifest() {
        let (_temp, project, mut manifest) = fixture();
        fail_directory_syncs_at_for_test(&[1, 2]);

        let error =
            apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap_err();

        assert_eq!(error.matches("injected directory sync failure").count(), 2);
        assert!(error.contains("recovery durability check also failed"));
        assert!(project
            .project_root
            .join(".agents/skills/tree-ring-memory/SKILL.md")
            .exists());
        assert_eq!(
            manifest.harnesses["pi"].state,
            ActivationState::NeedsUserReview
        );
    }

    #[test]
    fn manifest_sync_and_recovery_sync_failures_preserve_published_manifest_and_mark_memory_review()
    {
        let (_temp, project, mut manifest) = fixture();
        // Pi publishes two bridge entries before the activation manifest.
        fail_directory_syncs_at_for_test(&[3, 4]);

        let error =
            apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap_err();

        assert_eq!(error.matches("injected directory sync failure").count(), 2);
        assert!(error.contains("recovery durability check also failed"));
        assert!(project.memory_root.join("activation.json").exists());
        assert_eq!(
            manifest.harnesses["pi"].state,
            ActivationState::NeedsUserReview
        );
        let on_disk = crate::activation::load_manifest(&project.memory_root).unwrap();
        assert_eq!(on_disk.harnesses["pi"].state, ActivationState::NeedsTrust);
    }

    #[test]
    fn existing_manifest_replacement_is_refused_without_changing_disk_or_memory() {
        let (_temp, project, mut manifest) = fixture();
        let original = manifest.clone();
        let original_bytes = serde_json::to_vec_pretty(&original).unwrap();
        fs::write(project.memory_root.join("activation.json"), &original_bytes).unwrap();
        let project_fs = ProjectFs::open(&project).unwrap();
        let mut next = original.clone();
        next.cli_version = "0.14.0-test".to_string();

        let error = commit_files_and_manifest(
            &project_fs,
            &mut manifest,
            &original,
            Some(&original),
            next,
            &[],
        )
        .unwrap_err();

        assert!(error.contains("cannot be safely replaced"));
        assert_eq!(
            fs::read(project.memory_root.join("activation.json")).unwrap(),
            original_bytes
        );
        assert_eq!(manifest, original);
    }
}
