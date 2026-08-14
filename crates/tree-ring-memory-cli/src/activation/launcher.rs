use super::{
    adapters::{adapter_capability, ActivationProject},
    bridge::ProjectFs,
    manifest::ActivationManifest,
    preflight::{commit_prepared_preflight, prepare_preflight, ActivationError, PreflightRequest},
    AdapterCapability,
};
use std::{
    ffi::OsString,
    fmt, io,
    path::{Path, PathBuf},
    process::{Child, Command},
};
use tree_ring_memory_sqlite::SQLiteMemoryStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchRequest {
    harness_id: String,
    task_hint: Option<String>,
}

impl LaunchRequest {
    pub fn new(harness_id: impl Into<String>, task_hint: Option<String>) -> Self {
        Self {
            harness_id: harness_id.into(),
            task_hint,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchError(String);

impl LaunchError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for LaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for LaunchError {}

impl From<ActivationError> for LaunchError {
    fn from(error: ActivationError) -> Self {
        Self(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChildOutcome {
    exit_code: Option<i32>,
}

trait LaunchedChild {
    fn wait(&mut self) -> io::Result<ChildOutcome>;
}

trait ClaudeSpawner {
    type Child: LaunchedChild;

    fn spawn(&mut self, command: &mut Command) -> io::Result<Self::Child>;
}

struct SystemClaudeSpawner;

struct SystemClaudeChild(Child);

impl ClaudeSpawner for SystemClaudeSpawner {
    type Child = SystemClaudeChild;

    fn spawn(&mut self, command: &mut Command) -> io::Result<Self::Child> {
        command.spawn().map(SystemClaudeChild)
    }
}

impl LaunchedChild for SystemClaudeChild {
    fn wait(&mut self) -> io::Result<ChildOutcome> {
        self.0.wait().map(|status| ChildOutcome {
            exit_code: status.code(),
        })
    }
}

/// Launches the fixed Claude Code executable with one private, prepared Tree
/// Ring context file and returns Claude's exit code.
pub fn launch_with_preflight(
    store: &SQLiteMemoryStore,
    project: &ActivationProject,
    manifest: &ActivationManifest,
    request: LaunchRequest,
    arguments: &[OsString],
) -> Result<i32, LaunchError> {
    launch_with_spawner(
        store,
        project,
        manifest,
        request,
        arguments,
        &mut SystemClaudeSpawner,
    )
}

fn launch_with_spawner<S: ClaudeSpawner>(
    store: &SQLiteMemoryStore,
    project: &ActivationProject,
    manifest: &ActivationManifest,
    request: LaunchRequest,
    arguments: &[OsString],
    spawner: &mut S,
) -> Result<i32, LaunchError> {
    launch_with_spawner_and_cleanup(
        store,
        project,
        manifest,
        request,
        arguments,
        spawner,
        cleanup_runtime,
    )
}

fn launch_with_spawner_and_cleanup<S, C>(
    store: &SQLiteMemoryStore,
    project: &ActivationProject,
    manifest: &ActivationManifest,
    request: LaunchRequest,
    arguments: &[OsString],
    spawner: &mut S,
    cleanup: C,
) -> Result<i32, LaunchError>
where
    S: ClaudeSpawner,
    C: FnOnce(&ProjectFs, &str) -> Result<(), LaunchError>,
{
    if adapter_capability(&request.harness_id) != Some(AdapterCapability::WrapperPreflight) {
        return Err(LaunchError::new(format!(
            "harness {} does not provide a wrapper preflight",
            request.harness_id
        )));
    }
    if request.harness_id != "claude-code" {
        return Err(LaunchError::new(
            "only the Claude Code wrapper is supported",
        ));
    }

    let prepared = prepare_preflight(
        store,
        project,
        manifest,
        PreflightRequest::claude_wrapper(request.task_hint),
    )?;
    let receipt_id = prepared.receipt_id().to_string();
    let runtime_project = ActivationProject::from_memory_root(project.memory_root.clone())
        .map_err(LaunchError::new)?;
    let project_fs = ProjectFs::open(&runtime_project).map_err(LaunchError::new)?;
    let relative_context =
        match project_fs.create_runtime_context_file(&receipt_id, prepared.context().as_bytes()) {
            Ok(path) => path,
            Err(error) => {
                return match project_fs.remove_runtime_context_file(&receipt_id) {
                    Ok(_) => Err(LaunchError::new(format!(
                        "failed to create private Claude context: {error}"
                    ))),
                    Err(cleanup_error) => Err(cleanup_review_error(error, cleanup_error)),
                };
            }
        };
    let context_path = absolute_context_path(&runtime_project.project_root, &relative_context);
    let mut command = claude_command(&project.project_root, &context_path, arguments);

    let mut child = match spawner.spawn(&mut command) {
        Ok(child) => child,
        Err(error) => {
            let spawn_error = LaunchError::new(format!("failed to spawn Claude Code: {error}"));
            return match cleanup(&project_fs, &receipt_id) {
                Ok(()) => Err(spawn_error),
                Err(cleanup_error) => Err(combine_launch_errors(spawn_error, cleanup_error)),
            };
        }
    };

    let commit_result = commit_prepared_preflight(store, project, manifest, prepared).map_err(
        |error| {
            LaunchError::new(format!(
                "failed to commit launch receipt; receipt state may be indeterminate and requires user review: {error}"
            ))
        },
    );
    let wait_result = child.wait();
    let cleanup_result = cleanup(&project_fs, &receipt_id);
    let outcome_result = wait_result
        .map_err(|error| LaunchError::new(format!("failed to wait for Claude Code: {error}")))
        .and_then(|outcome| {
            outcome
                .exit_code
                .ok_or_else(|| LaunchError::new("Claude Code terminated without an exit code"))
        });

    let mut failures = Vec::new();
    if let Err(error) = commit_result {
        failures.push(error);
    }
    if let Err(error) = cleanup_result {
        failures.push(error);
    }
    let outcome = match outcome_result {
        Ok(outcome) => Some(outcome),
        Err(error) => {
            failures.push(error);
            None
        }
    };
    let mut errors = failures.into_iter();
    if let Some(primary) = errors.next() {
        return Err(errors.fold(primary, combine_launch_errors));
    }
    Ok(outcome.expect("successful outcome was checked above"))
}

fn claude_command(project_root: &Path, context_path: &Path, arguments: &[OsString]) -> Command {
    let mut command = Command::new("claude");
    command
        .current_dir(project_root)
        .arg("--append-system-prompt-file")
        .arg(context_path)
        .arg("--")
        .args(arguments);
    command
}

fn absolute_context_path(runtime_root: &Path, relative: &Path) -> PathBuf {
    if runtime_root.is_absolute() {
        runtime_root.join(relative)
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(runtime_root)
            .join(relative)
    }
}

fn cleanup_runtime(project_fs: &ProjectFs, receipt_id: &str) -> Result<(), LaunchError> {
    project_fs
        .remove_runtime_context_file(receipt_id)
        .map(|_| ())
        .map_err(|error| {
            LaunchError::new(format!(
                "private Claude context cleanup failed; runtime state is indeterminate and requires user review: {error}"
            ))
        })
}

fn cleanup_review_error(primary: String, cleanup: String) -> LaunchError {
    LaunchError::new(format!(
        "failed to create private Claude context: {primary}; cleanup failed and runtime state is indeterminate and requires user review: {cleanup}"
    ))
}

fn combine_launch_errors(primary: LaunchError, additional: LaunchError) -> LaunchError {
    LaunchError::new(format!("{primary}; additionally, {additional}"))
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
        preflight::project_fingerprint,
        ActivationState, AdapterCapability, ACTIVATION_PROTOCOL_VERSION, ACTIVATION_SCHEMA_VERSION,
    };
    use std::{
        cell::RefCell, collections::BTreeMap, ffi::OsString, fs, io, os::unix::fs::PermissionsExt,
        path::PathBuf, process::Command, rc::Rc,
    };
    use tree_ring_memory_core::{MemoryEvent, MemorySource};
    use tree_ring_memory_sqlite::SQLiteMemoryStore;

    struct Fixture {
        _temp: tempfile::TempDir,
        project: ActivationProject,
        store: SQLiteMemoryStore,
        manifest: ActivationManifest,
    }

    fn fixture() -> Fixture {
        let temp = tempfile::tempdir().unwrap();
        let project_root = temp.path().join("project");
        let project = ActivationProject::from_project_root(&project_root);
        fs::create_dir_all(&project.memory_root).unwrap();
        let mut activation = HarnessActivation {
            state: ActivationState::ConfiguredAwaitingProof,
            adapter_capability: AdapterCapability::WrapperPreflight,
            adapter_version: "1".to_string(),
            bridge_fingerprint: String::new(),
            bridge_path: Some(".claude/skills/tree-ring-memory/SKILL.md".to_string()),
            owned_files: vec![OwnedBridgeFile {
                path: ".claude/skills/tree-ring-memory/SKILL.md".to_string(),
                sha256: "b".repeat(64),
            }],
            managed_blocks: Vec::new(),
        };
        activation.bridge_fingerprint = bridge_fingerprint("claude-code", &activation);
        let manifest = ActivationManifest {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            protocol_version: ACTIVATION_PROTOCOL_VERSION,
            store_id: "store-launcher-test".to_string(),
            project_root_fingerprint: project_fingerprint(&project_root),
            cli_version: env!("CARGO_PKG_VERSION").to_string(),
            harnesses: BTreeMap::from([("claude-code".to_string(), activation)]),
        };
        save_manifest(&project.memory_root, &manifest).unwrap();
        let mut store = SQLiteMemoryStore::open(project.memory_root.join("memory.sqlite")).unwrap();
        let mut memory = MemoryEvent::new(
            "Tree Ring preflight context must stay out of process arguments",
            "lesson",
        )
        .unwrap();
        memory.project = Some("project".to_string());
        memory.agent_profile = Some("claude-code".to_string());
        memory.workflow_id = Some("workflow-1".to_string());
        memory.session_id = Some("session-1".to_string());
        memory.scope = "agent".to_string();
        memory.source = MemorySource {
            source_type: "agent".to_string(),
            ref_: "docs/constraints.md".to_string(),
            quote: String::new(),
        };
        store.put(&memory).unwrap();
        Fixture {
            _temp: temp,
            project,
            store,
            manifest,
        }
    }

    fn request_for(harness_id: &str) -> LaunchRequest {
        LaunchRequest::new(harness_id, Some("Tree Ring preflight context".to_string()))
    }

    fn runtime_context_files(project: &ActivationProject) -> Vec<PathBuf> {
        let directory = project.memory_root.join("activation/runtime");
        let Ok(entries) = fs::read_dir(directory) else {
            return Vec::new();
        };
        entries.map(|entry| entry.unwrap().path()).collect()
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CapturedCommand {
        program: OsString,
        args: Vec<OsString>,
        current_dir: Option<PathBuf>,
        context_path: PathBuf,
        context: String,
        mode: u32,
        receipt_count_at_spawn: usize,
    }

    struct FakeChild {
        wait_result: io::Result<ChildOutcome>,
    }

    impl LaunchedChild for FakeChild {
        fn wait(&mut self) -> io::Result<ChildOutcome> {
            self.wait_result
                .as_ref()
                .map(Clone::clone)
                .map_err(|error| io::Error::new(error.kind(), error.to_string()))
        }
    }

    struct CapturingSpawner {
        project: ActivationProject,
        captured: Rc<RefCell<Option<CapturedCommand>>>,
        spawn_error: Option<io::ErrorKind>,
        wait_result: io::Result<ChildOutcome>,
        on_spawn: Option<Box<dyn FnOnce()>>,
    }

    impl ClaudeSpawner for CapturingSpawner {
        type Child = FakeChild;

        fn spawn(&mut self, command: &mut Command) -> io::Result<Self::Child> {
            let args = command.get_args().map(OsString::from).collect::<Vec<_>>();
            let context_index = args
                .iter()
                .position(|arg| arg == "--append-system-prompt-file")
                .expect("launcher must include the context flag");
            let context_path = PathBuf::from(&args[context_index + 1]);
            let metadata = fs::metadata(&context_path).unwrap();
            *self.captured.borrow_mut() = Some(CapturedCommand {
                program: command.get_program().to_os_string(),
                args,
                current_dir: command.get_current_dir().map(PathBuf::from),
                context: fs::read_to_string(&context_path).unwrap(),
                mode: metadata.permissions().mode() & 0o777,
                context_path,
                receipt_count_at_spawn: receipt_files(&self.project.memory_root).len(),
            });
            if let Some(on_spawn) = self.on_spawn.take() {
                on_spawn();
            }
            if let Some(kind) = self.spawn_error {
                return Err(io::Error::new(kind, "injected spawn failure"));
            }
            Ok(FakeChild {
                wait_result: self
                    .wait_result
                    .as_ref()
                    .map(Clone::clone)
                    .map_err(|error| io::Error::new(error.kind(), error.to_string())),
            })
        }
    }

    fn spawner(
        project: &ActivationProject,
        wait_result: io::Result<ChildOutcome>,
    ) -> (CapturingSpawner, Rc<RefCell<Option<CapturedCommand>>>) {
        let captured = Rc::new(RefCell::new(None));
        (
            CapturingSpawner {
                project: project.clone(),
                captured: Rc::clone(&captured),
                spawn_error: None,
                wait_result,
                on_spawn: None,
            },
            captured,
        )
    }

    #[test]
    fn claude_launcher_injects_preflight_context_from_a_private_file() {
        let fixture = fixture();
        let (mut spawner, captured) =
            spawner(&fixture.project, Ok(ChildOutcome { exit_code: Some(0) }));

        let exit_code = launch_with_spawner(
            &fixture.store,
            &fixture.project,
            &fixture.manifest,
            request_for("claude-code"),
            &[OsString::from("--model"), OsString::from("sonnet")],
            &mut spawner,
        )
        .unwrap();

        let child = captured.borrow().clone().unwrap();
        assert_eq!(exit_code, 0);
        assert_eq!(child.program, OsString::from("claude"));
        assert_eq!(
            child.current_dir.as_deref(),
            Some(fixture.project.project_root.as_path())
        );
        let context_argument = child.context_path.as_os_str();
        assert!(child.args.windows(2).any(|pair| {
            pair[0] == "--append-system-prompt-file" && pair[1] == context_argument
        }));
        assert_eq!(
            &child.args[child.args.len() - 3..],
            [
                OsString::from("--"),
                OsString::from("--model"),
                OsString::from("sonnet")
            ]
        );
        assert_eq!(child.mode, 0o600);
        assert!(child
            .context
            .contains("Tree Ring Memory scoped preflight recall"));
        assert!(!child.args.iter().any(|arg| arg
            .to_string_lossy()
            .contains("must stay out of process arguments")));
        assert_eq!(child.receipt_count_at_spawn, 0);
        assert_eq!(receipt_files(&fixture.project.memory_root).len(), 1);
        assert!(runtime_context_files(&fixture.project).is_empty());
    }

    #[test]
    fn wrapper_rejects_an_unsupported_harness_without_writing_context() {
        let fixture = fixture();
        let (mut spawner, _captured) =
            spawner(&fixture.project, Ok(ChildOutcome { exit_code: Some(0) }));

        for harness_id in ["codex", "pi", "agent-zero"] {
            let error = launch_with_spawner(
                &fixture.store,
                &fixture.project,
                &fixture.manifest,
                request_for(harness_id),
                &[],
                &mut spawner,
            )
            .unwrap_err();

            assert!(error
                .to_string()
                .contains("does not provide a wrapper preflight"));
        }
        assert!(runtime_context_files(&fixture.project).is_empty());
        assert!(receipt_files(&fixture.project.memory_root).is_empty());
    }

    #[test]
    fn spawn_failure_removes_context_and_writes_no_receipt() {
        let fixture = fixture();
        let (mut spawner, _captured) =
            spawner(&fixture.project, Ok(ChildOutcome { exit_code: Some(0) }));
        spawner.spawn_error = Some(io::ErrorKind::NotFound);

        let error = launch_with_spawner(
            &fixture.store,
            &fixture.project,
            &fixture.manifest,
            request_for("claude-code"),
            &[],
            &mut spawner,
        )
        .unwrap_err();

        assert!(error.to_string().contains("failed to spawn Claude Code"));
        assert!(runtime_context_files(&fixture.project).is_empty());
        assert!(receipt_files(&fixture.project.memory_root).is_empty());
    }

    #[test]
    fn spawn_failure_preserves_spawn_and_cleanup_errors() {
        let fixture = fixture();
        let (mut spawner, _captured) =
            spawner(&fixture.project, Ok(ChildOutcome { exit_code: Some(0) }));
        spawner.spawn_error = Some(io::ErrorKind::NotFound);

        let error = launch_with_spawner_and_cleanup(
            &fixture.store,
            &fixture.project,
            &fixture.manifest,
            request_for("claude-code"),
            &[],
            &mut spawner,
            |_project_fs, _receipt_id| {
                Err(LaunchError::new(
                    "private Claude context cleanup failed; injected cleanup failure",
                ))
            },
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("failed to spawn Claude Code: injected spawn failure"));
        assert!(error.contains("injected cleanup failure"));
        assert!(receipt_files(&fixture.project.memory_root).is_empty());
    }

    #[test]
    fn abnormal_child_outcome_removes_context_and_surfaces_failure() {
        let fixture = fixture();
        let (mut spawner, _captured) =
            spawner(&fixture.project, Ok(ChildOutcome { exit_code: None }));

        let error = launch_with_spawner(
            &fixture.store,
            &fixture.project,
            &fixture.manifest,
            request_for("claude-code"),
            &[],
            &mut spawner,
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("terminated without an exit code"));
        assert!(runtime_context_files(&fixture.project).is_empty());
        assert_eq!(receipt_files(&fixture.project.memory_root).len(), 1);
    }

    #[test]
    fn child_wait_error_removes_context_and_surfaces_failure() {
        let fixture = fixture();
        let (mut spawner, _captured) = spawner(
            &fixture.project,
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected wait failure",
            )),
        );

        let error = launch_with_spawner(
            &fixture.store,
            &fixture.project,
            &fixture.manifest,
            request_for("claude-code"),
            &[],
            &mut spawner,
        )
        .unwrap_err();

        assert!(error.to_string().contains("failed to wait for Claude Code"));
        assert!(runtime_context_files(&fixture.project).is_empty());
        assert_eq!(receipt_files(&fixture.project.memory_root).len(), 1);
    }

    #[test]
    fn contract_change_after_spawn_blocks_receipt_and_removes_context() {
        let fixture = fixture();
        let (mut spawner, _captured) =
            spawner(&fixture.project, Ok(ChildOutcome { exit_code: Some(0) }));
        let memory_root = fixture.project.memory_root.clone();
        let mut changed = fixture.manifest.clone();
        changed
            .harnesses
            .get_mut("claude-code")
            .unwrap()
            .adapter_capability = AdapterCapability::NativePreflight;
        spawner.on_spawn = Some(Box::new(move || {
            save_manifest(&memory_root, &changed).unwrap();
        }));

        let error = launch_with_spawner(
            &fixture.store,
            &fixture.project,
            &fixture.manifest,
            request_for("claude-code"),
            &[],
            &mut spawner,
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("failed to commit launch receipt"));
        assert!(runtime_context_files(&fixture.project).is_empty());
        assert!(receipt_files(&fixture.project.memory_root).is_empty());
    }

    #[test]
    fn post_wait_failure_preserves_commit_indeterminacy_and_cleanup_error() {
        let fixture = fixture();
        let (mut spawner, _captured) =
            spawner(&fixture.project, Ok(ChildOutcome { exit_code: Some(0) }));
        let memory_root = fixture.project.memory_root.clone();
        let mut changed = fixture.manifest.clone();
        changed
            .harnesses
            .get_mut("claude-code")
            .unwrap()
            .adapter_capability = AdapterCapability::NativePreflight;
        spawner.on_spawn = Some(Box::new(move || {
            save_manifest(&memory_root, &changed).unwrap();
        }));

        let error = launch_with_spawner_and_cleanup(
            &fixture.store,
            &fixture.project,
            &fixture.manifest,
            request_for("claude-code"),
            &[],
            &mut spawner,
            |_project_fs, _receipt_id| {
                Err(LaunchError::new(
                    "private Claude context cleanup failed; injected cleanup failure",
                ))
            },
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("failed to commit launch receipt"));
        assert!(error.contains("receipt state may be indeterminate"));
        assert!(error.contains("injected cleanup failure"));
        assert!(receipt_files(&fixture.project.memory_root).is_empty());
    }
}
