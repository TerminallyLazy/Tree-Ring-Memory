use super::{ActivationState, AdapterCapability, ACTIVATION_PROTOCOL_VERSION};
use serde::Serialize;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

pub const AGENT_ZERO_PLUGIN_ID: &str = "tree_ring_memory";

/// Project paths used by activation. Adapter plans always target this root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationProject {
    pub project_root: PathBuf,
    pub memory_root: PathBuf,
}

impl ActivationProject {
    pub fn from_memory_root(memory_root: impl Into<PathBuf>) -> Result<Self, String> {
        let memory_root = memory_root.into();
        let project_root = memory_root.parent().ok_or_else(|| {
            format!(
                "memory root has no project parent: {}",
                memory_root.display()
            )
        })?;
        let project_root = if project_root.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            project_root.to_path_buf()
        };
        Ok(Self {
            project_root,
            memory_root,
        })
    }

    pub fn from_project_root(project_root: impl Into<PathBuf>) -> Self {
        let project_root = project_root.into();
        Self {
            memory_root: project_root.join(".tree-ring"),
            project_root,
        }
    }
}

/// Environment operations required for detection. Implementations must not write.
pub trait HarnessEnvironment {
    fn executable_version(&self, command: &str) -> Option<String>;
    fn project_path_exists(&self, relative: &Path) -> bool;
    fn read_project_file(&self, relative: &Path) -> Result<Option<String>, String>;
    fn agent_zero_plugin_manifest(&self) -> Option<AgentZeroPluginManifest>;
}

/// Minimal compatibility information supplied by the separate Agent Zero plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentZeroPluginManifest {
    pub plugin_id: String,
    pub protocol_version: u16,
    pub enabled: bool,
}

impl AgentZeroPluginManifest {
    pub fn compatible() -> Self {
        Self {
            plugin_id: AGENT_ZERO_PLUGIN_ID.to_string(),
            protocol_version: ACTIVATION_PROTOCOL_VERSION,
            enabled: true,
        }
    }

    fn is_compatible(&self) -> bool {
        self.enabled
            && self.plugin_id == AGENT_ZERO_PLUGIN_ID
            && self.protocol_version == ACTIVATION_PROTOCOL_VERSION
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct IntegrationMarker {
    pub path: String,
    pub origin: MarkerOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerOrigin {
    Home,
    Project,
}

impl MarkerOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Home => "home",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationStatus {
    Detected,
    Available,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BridgeWrite {
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedBlockUpdate {
    pub path: PathBuf,
    pub block_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlannedWrite {
    BridgeWrite(BridgeWrite),
    ManagedBlockUpdate(ManagedBlockUpdate),
}

/// A declarative plan; applying it is intentionally owned by a later bridge layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdapterPlan {
    pub harness_id: String,
    pub state: ActivationState,
    pub writes: Vec<PlannedWrite>,
    pub next_step: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeactivationPlan {
    pub harness_id: String,
    pub state: ActivationState,
    pub operations: Vec<DeactivationOperation>,
    pub next_step: String,
}

/// Removal work retained with the same ownership granularity as activation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeactivationOperation {
    BridgeWrite(BridgeWrite),
    ManagedBlockUpdate(ManagedBlockUpdate),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdapterDetection {
    pub id: String,
    pub name: String,
    pub capability: AdapterCapability,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_version: Option<String>,
    pub status: IntegrationStatus,
    pub state: ActivationState,
    pub markers: Vec<IntegrationMarker>,
    pub plan: AdapterPlan,
    pub next_step: String,
}

impl AdapterDetection {
    pub fn is_candidate(&self) -> bool {
        self.status == IntegrationStatus::Detected
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntegrationScanReport {
    pub root: PathBuf,
    pub detected_count: usize,
    pub integrations: Vec<AdapterDetection>,
}

impl IntegrationScanReport {
    pub fn by_id(&self, id: &str) -> Option<&AdapterDetection> {
        self.integrations
            .iter()
            .find(|detection| detection.id == id)
    }
}

pub type AgentIntegration = AdapterDetection;

pub trait HarnessAdapter: Sync {
    fn id(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn capability(&self) -> AdapterCapability;
    fn detect(&self, project: &ActivationProject, env: &dyn HarnessEnvironment)
        -> AdapterDetection;
    fn plan(&self, project: &ActivationProject, detection: &AdapterDetection) -> AdapterPlan;
}

#[derive(Debug, Clone, Copy)]
struct DeclarativeAdapter {
    id: &'static str,
    version: &'static str,
    display_name: &'static str,
    command: &'static str,
    capability: AdapterCapability,
    markers: &'static [&'static str],
    support: AdapterSupport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdapterSupport {
    Maintained,
    AgentZero,
    Unsupported,
}

const ADAPTERS: [DeclarativeAdapter; 7] = [
    DeclarativeAdapter {
        id: "codex",
        version: "1",
        display_name: "Codex",
        command: "codex",
        capability: AdapterCapability::WrapperPreflight,
        markers: &[".codex", "AGENTS.md"],
        support: AdapterSupport::Maintained,
    },
    DeclarativeAdapter {
        id: "claude-code",
        version: "1",
        display_name: "Claude Code",
        command: "claude",
        capability: AdapterCapability::NativePreflight,
        markers: &[".claude", "CLAUDE.md"],
        support: AdapterSupport::Maintained,
    },
    DeclarativeAdapter {
        id: "pi",
        version: "1",
        display_name: "Pi",
        command: "pi",
        capability: AdapterCapability::NativePreflight,
        markers: &[".pi", "pi.toml"],
        support: AdapterSupport::Maintained,
    },
    DeclarativeAdapter {
        id: "agent-zero",
        version: "1",
        display_name: "Agent Zero / A0",
        command: "agent-zero",
        capability: AdapterCapability::NativePreflight,
        markers: &[".a0", "agent-zero", "a0"],
        support: AdapterSupport::AgentZero,
    },
    DeclarativeAdapter {
        id: "hermes",
        version: "0",
        display_name: "Hermes",
        command: "hermes",
        capability: AdapterCapability::GuidanceOnly,
        markers: &[".hermes", "hermes.toml"],
        support: AdapterSupport::Unsupported,
    },
    DeclarativeAdapter {
        id: "opencode",
        version: "0",
        display_name: "OpenCode",
        command: "opencode",
        capability: AdapterCapability::GuidanceOnly,
        markers: &[".opencode", "opencode.json", "opencode.toml"],
        support: AdapterSupport::Unsupported,
    },
    DeclarativeAdapter {
        id: "goose",
        version: "0",
        display_name: "Goose",
        command: "goose",
        capability: AdapterCapability::GuidanceOnly,
        markers: &[".goose", "goosehints"],
        support: AdapterSupport::Unsupported,
    },
];

/// The four adapters with a maintained activation contract.
pub fn maintained_adapters() -> Vec<&'static dyn HarnessAdapter> {
    ADAPTERS
        .iter()
        .filter(|adapter| adapter.support != AdapterSupport::Unsupported)
        .map(|adapter| adapter as &dyn HarnessAdapter)
        .collect()
}

/// Returns the exact activation adapter version registered for a harness.
pub fn adapter_version(id: &str) -> Option<&'static str> {
    registered_adapters()
        .find(|adapter| adapter.id == id)
        .map(|adapter| adapter.version)
}

fn registered_adapters() -> impl Iterator<Item = &'static DeclarativeAdapter> {
    ADAPTERS.iter()
}

/// Detect candidate harnesses without mutating either the project or its store.
pub fn detect_adapters(
    project: &ActivationProject,
    env: &dyn HarnessEnvironment,
) -> IntegrationScanReport {
    let integrations = registered_adapters()
        .map(|adapter| adapter.detect(project, env))
        .collect::<Vec<_>>();
    let detected_count = integrations
        .iter()
        .filter(|detection| detection.is_candidate())
        .count();
    IntegrationScanReport {
        root: project.project_root.clone(),
        detected_count,
        integrations,
    }
}

/// Read-only compatibility alias for `tree-ring integrations scan`.
pub fn scan_integrations(root: &Path) -> IntegrationScanReport {
    let project = ActivationProject::from_project_root(root);
    detect_adapters(
        &project,
        &LocalHarnessEnvironment::new(project.project_root.clone()),
    )
}

pub fn format_markers(markers: &[IntegrationMarker]) -> String {
    markers
        .iter()
        .map(|marker| format!("{}:{}", marker.origin.as_str(), marker.path))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn plan_activation(id: &str, project: &ActivationProject) -> Result<AdapterPlan, String> {
    plan_activation_with_environment(id, project, &EmptyHarnessEnvironment)
}

pub fn plan_activation_with_environment(
    id: &str,
    project: &ActivationProject,
    env: &dyn HarnessEnvironment,
) -> Result<AdapterPlan, String> {
    let adapter = registered_adapters()
        .find(|adapter| adapter.id == id)
        .ok_or_else(|| format!("unknown harness adapter: {id}"))?;
    let detection = adapter.detect(project, env);
    Ok(adapter.plan(project, &detection))
}

pub fn plan_deactivation(
    id: &str,
    project: &ActivationProject,
) -> Result<DeactivationPlan, String> {
    plan_deactivation_with_environment(id, project, &EmptyHarnessEnvironment)
}

pub fn plan_deactivation_with_environment(
    id: &str,
    project: &ActivationProject,
    env: &dyn HarnessEnvironment,
) -> Result<DeactivationPlan, String> {
    let adapter = registered_adapters()
        .find(|adapter| adapter.id == id)
        .ok_or_else(|| format!("unknown harness adapter: {id}"))?;
    let detection = adapter.detect(project, env);
    let operations = adapter
        .plan(project, &detection)
        .writes
        .into_iter()
        .map(DeactivationOperation::from)
        .collect();
    Ok(DeactivationPlan {
        harness_id: id.to_string(),
        state: detection.state,
        operations,
        next_step: "A later bridge layer removes only manifest-recorded Tree Ring-owned material."
            .to_string(),
    })
}

impl HarnessAdapter for DeclarativeAdapter {
    fn id(&self) -> &'static str {
        self.id
    }

    fn version(&self) -> &'static str {
        self.version
    }

    fn display_name(&self) -> &'static str {
        self.display_name
    }

    fn capability(&self) -> AdapterCapability {
        self.capability
    }

    fn detect(
        &self,
        project: &ActivationProject,
        env: &dyn HarnessEnvironment,
    ) -> AdapterDetection {
        let mut markers = self
            .markers
            .iter()
            .filter(|marker| env.project_path_exists(Path::new(marker)))
            .map(|marker| IntegrationMarker {
                path: normalized_relative_path(marker)
                    .expect("static marker paths are normalized")
                    .display()
                    .to_string(),
                origin: MarkerOrigin::Project,
            })
            .collect::<Vec<_>>();
        markers.sort();
        markers.dedup();

        let state = match self.support {
            AdapterSupport::Unsupported => ActivationState::Unsupported,
            AdapterSupport::AgentZero => match env.agent_zero_plugin_manifest() {
                Some(manifest) if manifest.is_compatible() => {
                    ActivationState::ConfiguredAwaitingProof
                }
                _ => ActivationState::NeedsPlugin,
            },
            AdapterSupport::Maintained if self.id == "pi" => ActivationState::NeedsTrust,
            AdapterSupport::Maintained => ActivationState::ConfiguredAwaitingProof,
        };
        let status = if markers.is_empty() {
            IntegrationStatus::Available
        } else {
            IntegrationStatus::Detected
        };
        let mut detection = AdapterDetection {
            id: self.id.to_string(),
            name: self.display_name.to_string(),
            capability: self.capability,
            executable_version: env.executable_version(self.command),
            status,
            state,
            markers,
            plan: AdapterPlan {
                harness_id: self.id.to_string(),
                state,
                writes: Vec::new(),
                next_step: next_step(self.support, state).to_string(),
            },
            next_step: next_step(self.support, state).to_string(),
        };
        detection.plan = self.plan(project, &detection);
        detection
    }

    fn plan(&self, _project: &ActivationProject, detection: &AdapterDetection) -> AdapterPlan {
        let writes = match detection.state {
            ActivationState::Unsupported | ActivationState::NeedsPlugin => Vec::new(),
            _ => adapter_writes(self),
        };
        AdapterPlan {
            harness_id: self.id.to_string(),
            state: detection.state,
            writes,
            next_step: next_step(self.support, detection.state).to_string(),
        }
    }
}

fn next_step(support: AdapterSupport, state: ActivationState) -> &'static str {
    match (support, state) {
        (AdapterSupport::AgentZero, ActivationState::NeedsPlugin) => {
            "Install or enable the compatible separate tree_ring_memory Agent Zero plugin."
        }
        (AdapterSupport::Unsupported, _) => {
            "Author and register a native harness adapter before claiming Tree Ring activation."
        }
        _ => "Apply the reviewed bridge plan, then complete adapter-specific preflight and receipt verification.",
    }
}

fn adapter_writes(adapter: &DeclarativeAdapter) -> Vec<PlannedWrite> {
    match adapter.id {
        "codex" => vec![
            bridge_write(".agents/skills/tree-ring-memory/SKILL.md"),
            managed_block("AGENTS.md", "codex"),
        ],
        "claude-code" => vec![
            bridge_write(".claude/skills/tree-ring-memory/SKILL.md"),
            managed_block(".claude/settings.json", "claude-code"),
        ],
        "pi" => vec![
            bridge_write(".agents/skills/tree-ring-memory/SKILL.md"),
            bridge_write(".pi/extensions/tree-ring-memory.ts"),
        ],
        "agent-zero" => vec![bridge_write(".tree-ring/activation/agent-zero.json")],
        _ => Vec::new(),
    }
}

impl From<PlannedWrite> for DeactivationOperation {
    fn from(write: PlannedWrite) -> Self {
        match write {
            PlannedWrite::BridgeWrite(write) => Self::BridgeWrite(write),
            PlannedWrite::ManagedBlockUpdate(write) => Self::ManagedBlockUpdate(write),
        }
    }
}

fn bridge_write(path: &str) -> PlannedWrite {
    PlannedWrite::BridgeWrite(BridgeWrite {
        path: normalized_relative_path(path).expect("static bridge paths are normalized"),
    })
}

fn managed_block(path: &str, block_id: &str) -> PlannedWrite {
    PlannedWrite::ManagedBlockUpdate(ManagedBlockUpdate {
        path: normalized_relative_path(path).expect("static bridge paths are normalized"),
        block_id: block_id.to_string(),
    })
}

fn normalized_relative_path(path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err("adapter plan paths must be normalized project-relative paths".to_string());
    }
    Ok(path.to_path_buf())
}

struct EmptyHarnessEnvironment;

impl HarnessEnvironment for EmptyHarnessEnvironment {
    fn executable_version(&self, _command: &str) -> Option<String> {
        None
    }

    fn project_path_exists(&self, _relative: &Path) -> bool {
        false
    }

    fn read_project_file(&self, _relative: &Path) -> Result<Option<String>, String> {
        Ok(None)
    }

    fn agent_zero_plugin_manifest(&self) -> Option<AgentZeroPluginManifest> {
        None
    }
}

struct LocalHarnessEnvironment {
    project_root: PathBuf,
}

impl LocalHarnessEnvironment {
    fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }
}

impl HarnessEnvironment for LocalHarnessEnvironment {
    fn executable_version(&self, command: &str) -> Option<String> {
        Command::new(command)
            .arg("--version")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|version| version.trim().to_string())
            .filter(|version| !version.is_empty())
    }

    fn project_path_exists(&self, relative: &Path) -> bool {
        self.project_root.join(relative).exists()
    }

    fn read_project_file(&self, relative: &Path) -> Result<Option<String>, String> {
        let path = self.project_root.join(relative);
        match std::fs::read_to_string(&path) {
            Ok(content) => Ok(Some(content)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("failed to read {}: {error}", path.display())),
        }
    }

    fn agent_zero_plugin_manifest(&self) -> Option<AgentZeroPluginManifest> {
        // Core does not infer a plugin manifest from a generic project marker.
        // The separate plugin supplies this capability through a future runtime
        // environment; until then scan must remain conservatively needs-plugin.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Default)]
    struct FakeEnvironment {
        executable_versions: BTreeMap<String, String>,
        paths: BTreeSet<PathBuf>,
        files: BTreeMap<PathBuf, String>,
        agent_zero: Option<AgentZeroPluginManifest>,
    }

    impl HarnessEnvironment for FakeEnvironment {
        fn executable_version(&self, command: &str) -> Option<String> {
            self.executable_versions.get(command).cloned()
        }

        fn project_path_exists(&self, relative: &Path) -> bool {
            self.paths.contains(relative)
        }

        fn read_project_file(&self, relative: &Path) -> Result<Option<String>, String> {
            Ok(self.files.get(relative).cloned())
        }

        fn agent_zero_plugin_manifest(&self) -> Option<AgentZeroPluginManifest> {
            self.agent_zero.clone()
        }
    }

    fn project() -> ActivationProject {
        ActivationProject::from_memory_root("/tmp/tree-ring-project/.tree-ring").unwrap()
    }

    #[test]
    fn an_empty_codex_marker_is_detected_as_a_candidate_but_never_active() {
        let mut env = FakeEnvironment::default();
        env.paths.insert(PathBuf::from(".codex"));

        let report = detect_adapters(&project(), &env);
        let codex = report.by_id("codex").unwrap();

        assert!(codex.is_candidate());
        assert_ne!(codex.state, ActivationState::Active);
        assert!(!codex.plan.writes.is_empty());
    }

    #[test]
    fn an_available_executable_never_transitions_a_harness_to_active() {
        let mut env = FakeEnvironment::default();
        env.executable_versions
            .insert("codex".to_string(), "1.2.3".to_string());

        let report = detect_adapters(&project(), &env);
        let codex = report.by_id("codex").unwrap();

        assert_eq!(codex.executable_version.as_deref(), Some("1.2.3"));
        assert_ne!(codex.state, ActivationState::Active);
        assert_eq!(codex.state, ActivationState::ConfiguredAwaitingProof);
    }

    #[test]
    fn unknown_harnesses_are_explicitly_unsupported_without_bridge_writes() {
        let plan = plan_activation("hermes", &project()).unwrap();
        assert_eq!(plan.state, ActivationState::Unsupported);
        assert!(plan.writes.is_empty());
    }

    #[test]
    fn missing_agent_zero_plugin_requires_the_separate_plugin_without_core_mutation() {
        let detection = detect_adapters(&project(), &FakeEnvironment::default());
        let agent_zero = detection.by_id("agent-zero").unwrap();
        assert_eq!(agent_zero.state, ActivationState::NeedsPlugin);
        assert!(agent_zero.plan.writes.is_empty());
    }

    #[test]
    fn a_compatible_agent_zero_plugin_only_receives_its_protocol_binding_plan() {
        let env = FakeEnvironment {
            agent_zero: Some(AgentZeroPluginManifest::compatible()),
            ..FakeEnvironment::default()
        };
        let agent_zero = detect_adapters(&project(), &env)
            .by_id("agent-zero")
            .unwrap()
            .clone();
        assert_eq!(agent_zero.state, ActivationState::ConfiguredAwaitingProof);
        assert_eq!(
            agent_zero.plan.writes,
            vec![bridge_write(".tree-ring/activation/agent-zero.json")]
        );
    }

    #[test]
    fn registry_has_exactly_four_maintained_adapters_and_three_explicitly_unsupported_ones() {
        let maintained = maintained_adapters();
        assert_eq!(maintained.len(), 4);
        assert_eq!(
            maintained
                .iter()
                .map(|adapter| adapter.id())
                .collect::<Vec<_>>(),
            vec!["codex", "claude-code", "pi", "agent-zero"]
        );
        assert!(maintained.iter().all(|adapter| adapter.version() == "1"));
        assert!(maintained
            .iter()
            .all(|adapter| adapter_version(adapter.id()) == Some("1")));
        for id in ["hermes", "opencode", "goose"] {
            assert_eq!(
                plan_activation(id, &project()).unwrap().state,
                ActivationState::Unsupported
            );
        }
    }

    #[test]
    fn project_root_is_derived_from_the_memory_root() {
        let project = ActivationProject::from_memory_root("workspace/.tree-ring").unwrap();
        assert_eq!(project.project_root, PathBuf::from("workspace"));
        assert_eq!(
            ActivationProject::from_memory_root(".tree-ring")
                .unwrap()
                .project_root,
            PathBuf::from(".")
        );
        assert_eq!(
            ActivationProject::from_memory_root("/").unwrap_err(),
            "memory root has no project parent: /"
        );
    }

    #[test]
    fn plans_only_use_normalized_project_relative_paths() {
        for adapter in maintained_adapters() {
            let plan = plan_activation(adapter.id(), &project()).unwrap();
            for write in plan.writes {
                let path = match write {
                    PlannedWrite::BridgeWrite(write) => write.path,
                    PlannedWrite::ManagedBlockUpdate(write) => write.path,
                };
                assert!(!path.is_absolute());
                assert!(!path
                    .components()
                    .any(|component| component == Component::ParentDir));
            }
        }
    }

    #[test]
    fn detection_exposes_project_relative_markers_for_an_absolute_project_root() {
        let mut env = FakeEnvironment::default();
        env.paths.insert(PathBuf::from(".codex"));
        let project =
            ActivationProject::from_memory_root("/private/tmp/tree-ring-project/.tree-ring")
                .unwrap();

        let report = detect_adapters(&project, &env);
        let marker = report.by_id("codex").unwrap().markers.first().unwrap();

        assert_eq!(marker.path, ".codex");
        assert!(!Path::new(&marker.path).is_absolute());
    }

    #[test]
    fn deactivation_retains_managed_block_ownership() {
        let plan = plan_deactivation("codex", &project()).unwrap();

        assert_eq!(plan.state, ActivationState::ConfiguredAwaitingProof);
        assert!(plan.operations.iter().any(|operation| matches!(
            operation,
            DeactivationOperation::BridgeWrite(BridgeWrite { path })
                if path == Path::new(".agents/skills/tree-ring-memory/SKILL.md")
        )));
        assert!(plan.operations.iter().any(|operation| matches!(
            operation,
            DeactivationOperation::ManagedBlockUpdate(ManagedBlockUpdate { path, block_id })
                if path == Path::new("AGENTS.md") && block_id == "codex"
        )));
        assert!(!plan.operations.iter().any(|operation| matches!(
            operation,
            DeactivationOperation::BridgeWrite(BridgeWrite { path }) if path == Path::new("AGENTS.md")
        )));
    }

    #[test]
    fn missing_agent_zero_plugin_blocks_deactivation_without_operations() {
        let plan = plan_deactivation("agent-zero", &project()).unwrap();

        assert_eq!(plan.state, ActivationState::NeedsPlugin);
        assert!(plan.operations.is_empty());
    }

    #[test]
    fn compatible_agent_zero_plugin_plans_only_its_binding_deactivation() {
        let env = FakeEnvironment {
            agent_zero: Some(AgentZeroPluginManifest::compatible()),
            ..FakeEnvironment::default()
        };
        let plan = plan_deactivation_with_environment("agent-zero", &project(), &env).unwrap();

        assert_eq!(plan.state, ActivationState::ConfiguredAwaitingProof);
        assert_eq!(
            plan.operations,
            vec![DeactivationOperation::BridgeWrite(BridgeWrite {
                path: PathBuf::from(".tree-ring/activation/agent-zero.json"),
            })]
        );
    }
}
