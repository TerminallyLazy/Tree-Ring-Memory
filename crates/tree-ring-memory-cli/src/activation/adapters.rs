use super::{ActivationState, AdapterCapability, ACTIVATION_PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::process::Command;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

pub const AGENT_ZERO_PLUGIN_ID: &str = "tree_ring_memory";
pub const AGENT_ZERO_PLUGIN_MANIFEST_ENV: &str = "TREE_RING_AGENT_ZERO_PLUGIN_MANIFEST";

const AGENT_ZERO_CAPABILITY_FILE: &str = "activation-capability.json";
const AGENT_ZERO_CAPABILITY_KIND: &str = "tree-ring-agent-zero-plugin-capability";
const AGENT_ZERO_CAPABILITY_CONTRACTS: &[(&str, &str, &str)] = &[
    ("3.1.0", "0.14.0", "0.14"),
    ("3.2.0", "0.15.3", "0.15"),
    ("3.3.0", "0.15.3", "0.15"),
    ("3.3.1", "0.15.3", "0.15"),
];
const MAX_AGENT_ZERO_CAPABILITY_BYTES: u64 = 16 * 1024;

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
        // Keep the root and its derived project parent in the same lexical
        // form. The CLI default is `.tree-ring`; without this, it derives `.`
        // as the project root but compares `.tree-ring` with `./.tree-ring`
        // during the project-local safety validation.
        let memory_root = if memory_root
            .parent()
            .is_some_and(|parent| parent.as_os_str().is_empty())
        {
            project_root.join(&memory_root)
        } else {
            memory_root
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
    fn home_path_exists(&self, _relative: &Path) -> bool {
        false
    }
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentZeroCapabilityDocument {
    schema_version: u16,
    kind: String,
    plugin_id: String,
    plugin_version: String,
    activation_protocol_version: u16,
    tree_ring_version: AgentZeroTreeRingVersion,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentZeroTreeRingVersion {
    min: String,
    minor: String,
}

/// Reads the capability that the separately installed Agent Zero plugin
/// explicitly presents to a Tree Ring child process. It never scans project
/// markers or generic Agent Zero paths: only the plugin-owned, absolute,
/// no-follow descriptor named by the environment is considered.
pub fn agent_zero_plugin_manifest_for_project(
    project_root: &Path,
) -> Option<AgentZeroPluginManifest> {
    let descriptor = std::env::var_os(AGENT_ZERO_PLUGIN_MANIFEST_ENV).map(PathBuf::from)?;
    read_agent_zero_plugin_manifest(project_root, &descriptor)
}

fn read_agent_zero_plugin_manifest(
    project_root: &Path,
    descriptor: &Path,
) -> Option<AgentZeroPluginManifest> {
    if !descriptor.is_absolute()
        || descriptor.file_name().and_then(|name| name.to_str()) != Some(AGENT_ZERO_CAPABILITY_FILE)
    {
        return None;
    }

    // Reject a descriptor symlink before canonicalizing. Canonicalization is
    // then used only to compare the resolved external plugin location with
    // the project root and to find its sibling plugin.yaml.
    let metadata = fs::symlink_metadata(descriptor).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_AGENT_ZERO_CAPABILITY_BYTES {
        return None;
    }
    let descriptor = fs::canonicalize(descriptor).ok()?;
    if descriptor.file_name().and_then(|name| name.to_str()) != Some(AGENT_ZERO_CAPABILITY_FILE) {
        return None;
    }
    let project_root = fs::canonicalize(project_root).ok()?;
    if descriptor.starts_with(&project_root) {
        return None;
    }

    let capability = serde_json::from_slice::<AgentZeroCapabilityDocument>(
        &read_regular_file_no_follow(&descriptor)?,
    )
    .ok()?;
    let trusted_contract = AGENT_ZERO_CAPABILITY_CONTRACTS.iter().any(
        |(plugin_version, minimum_version, minor_version)| {
            capability.plugin_version == *plugin_version
                && capability.tree_ring_version.min == *minimum_version
                && capability.tree_ring_version.minor == *minor_version
        },
    );
    if capability.schema_version != 1
        || capability.kind != AGENT_ZERO_CAPABILITY_KIND
        || capability.plugin_id != AGENT_ZERO_PLUGIN_ID
        || capability.activation_protocol_version != ACTIVATION_PROTOCOL_VERSION
        || !trusted_contract
        || !capability.enabled
    {
        return None;
    }

    let plugin_yaml = descriptor.parent()?.join("plugin.yaml");
    let plugin_yaml = read_regular_file_no_follow(&plugin_yaml)?;
    plugin_yaml_matches_capability(
        std::str::from_utf8(&plugin_yaml).ok()?,
        &capability.plugin_version,
    )
    .then_some(AgentZeroPluginManifest::compatible())
}

/// Opens one regular capability file without following its final path entry.
/// Non-Unix platforms fail closed because the rest of activation persistence
/// already requires descriptor-relative no-follow support there as well.
#[cfg(unix)]
fn read_regular_file_no_follow(path: &Path) -> Option<Vec<u8>> {
    let before = fs::symlink_metadata(path).ok()?;
    if !before.file_type().is_file() || before.len() > MAX_AGENT_ZERO_CAPABILITY_BYTES {
        return None;
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .ok()?;
    let after = file.metadata().ok()?;
    if !after.file_type().is_file()
        || after.len() > MAX_AGENT_ZERO_CAPABILITY_BYTES
        || before.dev() != after.dev()
        || before.ino() != after.ino()
    {
        return None;
    }
    let mut contents = Vec::with_capacity(usize::try_from(after.len()).ok()?);
    file.take(MAX_AGENT_ZERO_CAPABILITY_BYTES + 1)
        .read_to_end(&mut contents)
        .ok()?;
    (contents.len() as u64 <= MAX_AGENT_ZERO_CAPABILITY_BYTES).then_some(contents)
}

#[cfg(not(unix))]
fn read_regular_file_no_follow(_path: &Path) -> Option<Vec<u8>> {
    None
}

fn plugin_yaml_matches_capability(plugin_yaml: &str, expected_version: &str) -> bool {
    let mut name = None;
    let mut version = None;
    for line in plugin_yaml.lines() {
        if line.starts_with([' ', '\t']) {
            continue;
        }
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_matches(['\'', '"']);
        match key.trim() {
            "name" if name.replace(value).is_none() => {}
            "version" if version.replace(value).is_none() => {}
            "name" | "version" => return false,
            _ => {}
        }
    }
    name == Some(AGENT_ZERO_PLUGIN_ID) && version == Some(expected_version)
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
    home_markers: &'static [&'static str],
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
        capability: AdapterCapability::GuidanceOnly,
        markers: &[".codex", "AGENTS.md"],
        home_markers: &[".codex"],
        support: AdapterSupport::Maintained,
    },
    DeclarativeAdapter {
        id: "claude-code",
        version: "1",
        display_name: "Claude Code",
        command: "claude",
        capability: AdapterCapability::WrapperPreflight,
        markers: &[".claude", "CLAUDE.md"],
        home_markers: &[".claude"],
        support: AdapterSupport::Maintained,
    },
    DeclarativeAdapter {
        id: "pi",
        version: "1",
        display_name: "Pi",
        command: "pi",
        capability: AdapterCapability::NativePreflight,
        markers: &[".pi", "pi.toml"],
        home_markers: &[".pi"],
        support: AdapterSupport::Maintained,
    },
    DeclarativeAdapter {
        id: "agent-zero",
        version: "1",
        display_name: "Agent Zero / A0",
        command: "agent-zero",
        capability: AdapterCapability::NativePreflight,
        // Project `.a0`-style paths are not evidence that the separate
        // tree_ring_memory plugin is installed or enabled. The plugin may
        // prove its capability only through the explicit descriptor below.
        markers: &[],
        home_markers: &[],
        support: AdapterSupport::AgentZero,
    },
    DeclarativeAdapter {
        id: "hermes",
        version: "0",
        display_name: "Hermes",
        command: "hermes",
        capability: AdapterCapability::GuidanceOnly,
        markers: &[".hermes", "hermes.toml"],
        home_markers: &[".hermes"],
        support: AdapterSupport::Unsupported,
    },
    DeclarativeAdapter {
        id: "opencode",
        version: "0",
        display_name: "OpenCode",
        command: "opencode",
        capability: AdapterCapability::GuidanceOnly,
        markers: &[".opencode", "opencode.json", "opencode.toml"],
        home_markers: &[".opencode"],
        support: AdapterSupport::Unsupported,
    },
    DeclarativeAdapter {
        id: "goose",
        version: "0",
        display_name: "Goose",
        command: "goose",
        capability: AdapterCapability::GuidanceOnly,
        markers: &[".goose", "goosehints"],
        home_markers: &[".goose"],
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

/// Returns the activation capability registered for a harness.
pub(crate) fn adapter_capability(id: &str) -> Option<AdapterCapability> {
    registered_adapters()
        .find(|adapter| adapter.id == id)
        .map(|adapter| adapter.capability)
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
        markers.extend(
            self.home_markers
                .iter()
                .filter(|marker| env.home_path_exists(Path::new(marker)))
                .map(|marker| IntegrationMarker {
                    path: normalized_relative_path(marker)
                        .expect("static home marker paths are normalized")
                        .display()
                        .to_string(),
                    origin: MarkerOrigin::Home,
                }),
        );
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
        // Agent Zero receives a passive, core-owned project binding even
        // before its separate plugin proves availability. Keeping the
        // persisted record at NeedsPlugin makes this creation-only bootstrap
        // inert; a verified plugin descriptor is required later for
        // configured status and receipt-producing preflight.
        let state = if self.support == AdapterSupport::AgentZero {
            ActivationState::NeedsPlugin
        } else {
            detection.state
        };
        let writes = match state {
            ActivationState::Unsupported | ActivationState::NeedsPlugin
                if self.support != AdapterSupport::AgentZero =>
            {
                Vec::new()
            }
            _ => adapter_writes(self),
        };
        AdapterPlan {
            harness_id: self.id.to_string(),
            state,
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
    home_root: Option<PathBuf>,
}

impl LocalHarnessEnvironment {
    fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            home_root: std::env::var_os("HOME").map(PathBuf::from),
        }
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

    fn home_path_exists(&self, relative: &Path) -> bool {
        self.home_root
            .as_ref()
            .is_some_and(|home| home.join(relative).exists())
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
        agent_zero_plugin_manifest_for_project(&self.project_root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
    };
    use tempfile::tempdir;

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    #[derive(Default)]
    struct FakeEnvironment {
        executable_versions: BTreeMap<String, String>,
        paths: BTreeSet<PathBuf>,
        home_paths: BTreeSet<PathBuf>,
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

        fn home_path_exists(&self, relative: &Path) -> bool {
            self.home_paths.contains(relative)
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
    fn missing_agent_zero_plugin_requires_the_separate_plugin_but_plans_a_passive_binding() {
        let mut env = FakeEnvironment::default();
        env.paths.insert(PathBuf::from(".a0"));
        let detection = detect_adapters(&project(), &env);
        let agent_zero = detection.by_id("agent-zero").unwrap();
        assert_eq!(agent_zero.state, ActivationState::NeedsPlugin);
        assert!(agent_zero.markers.is_empty());
        assert_eq!(agent_zero.status, IntegrationStatus::Available);
        assert_eq!(
            agent_zero.plan.writes,
            vec![bridge_write(".tree-ring/activation/agent-zero.json")]
        );
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
        assert_eq!(agent_zero.plan.state, ActivationState::NeedsPlugin);
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
    fn only_claude_code_advertises_a_tested_wrapper_preflight() {
        let report = detect_adapters(&project(), &FakeEnvironment::default());

        assert_eq!(
            report.by_id("claude-code").unwrap().capability,
            AdapterCapability::WrapperPreflight
        );
        for harness_id in ["codex", "pi", "agent-zero"] {
            assert_ne!(
                report.by_id(harness_id).unwrap().capability,
                AdapterCapability::WrapperPreflight,
                "{harness_id} must not advertise a generic launch wrapper"
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
    fn detection_distinguishes_project_and_home_markers_without_exposing_home_paths() {
        let mut env = FakeEnvironment::default();
        env.paths.insert(PathBuf::from("CLAUDE.md"));
        env.home_paths.insert(PathBuf::from(".claude"));

        let report = detect_adapters(&project(), &env);
        let claude = report.by_id("claude-code").unwrap();

        assert!(claude.markers.contains(&IntegrationMarker {
            path: "CLAUDE.md".to_string(),
            origin: MarkerOrigin::Project,
        }));
        assert!(claude.markers.contains(&IntegrationMarker {
            path: ".claude".to_string(),
            origin: MarkerOrigin::Home,
        }));
        assert!(claude
            .markers
            .iter()
            .all(|marker| !Path::new(&marker.path).is_absolute()));
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
    fn missing_agent_zero_plugin_retains_ownership_for_passive_binding_deactivation() {
        let plan = plan_deactivation("agent-zero", &project()).unwrap();

        assert_eq!(plan.state, ActivationState::NeedsPlugin);
        assert_eq!(
            plan.operations,
            vec![DeactivationOperation::BridgeWrite(BridgeWrite {
                path: PathBuf::from(".tree-ring/activation/agent-zero.json"),
            })]
        );
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

    #[test]
    fn agent_zero_capability_descriptor_is_external_exact_and_no_follow() {
        let temp = tempdir().unwrap();
        let project = temp.path().join("project");
        let plugin = temp.path().join("plugin");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&plugin).unwrap();

        let descriptor = write_capability_descriptor(&plugin, true);
        assert_eq!(
            read_agent_zero_plugin_manifest(&project, &descriptor),
            Some(AgentZeroPluginManifest::compatible())
        );

        fs::write(
            plugin.join("plugin.yaml"),
            "name: tree_ring_memory\nversion: 3.3.1\n",
        )
        .unwrap();
        fs::write(
            &descriptor,
            capability_document(true).replace(
                "\"plugin_version\":\"3.2.0\"",
                "\"plugin_version\":\"3.3.1\"",
            ),
        )
        .unwrap();
        assert_eq!(
            read_agent_zero_plugin_manifest(&project, &descriptor),
            Some(AgentZeroPluginManifest::compatible())
        );

        fs::write(
            plugin.join("plugin.yaml"),
            "name: tree_ring_memory\nversion: 3.1.0\n",
        )
        .unwrap();
        fs::write(
            &descriptor,
            r#"{"schema_version":1,"kind":"tree-ring-agent-zero-plugin-capability","plugin_id":"tree_ring_memory","plugin_version":"3.1.0","activation_protocol_version":1,"tree_ring_version":{"min":"0.14.0","minor":"0.14"},"enabled":true}"#,
        )
        .unwrap();
        assert_eq!(
            read_agent_zero_plugin_manifest(&project, &descriptor),
            Some(AgentZeroPluginManifest::compatible())
        );

        fs::write(&descriptor, capability_document(true)).unwrap();
        fs::write(
            plugin.join("plugin.yaml"),
            "name: tree_ring_memory\nversion: 3.2.0\n",
        )
        .unwrap();
        fs::write(
            &descriptor,
            capability_document(true).replace(
                "\"min\":\"0.15.3\",\"minor\":\"0.15\"",
                "\"min\":\"0.14.0\",\"minor\":\"0.14\"",
            ),
        )
        .unwrap();
        assert!(read_agent_zero_plugin_manifest(&project, &descriptor).is_none());

        fs::write(&descriptor, capability_document(false)).unwrap();
        assert!(read_agent_zero_plugin_manifest(&project, &descriptor).is_none());

        fs::write(
            &descriptor,
            capability_document(true).replace("\"schema_version\":1", "\"schema_version\":2"),
        )
        .unwrap();
        assert!(read_agent_zero_plugin_manifest(&project, &descriptor).is_none());

        fs::write(&descriptor, capability_document(true)).unwrap();
        fs::write(
            plugin.join("plugin.yaml"),
            "name: tree_ring_memory\nversion: 3.0.1\n",
        )
        .unwrap();
        assert!(read_agent_zero_plugin_manifest(&project, &descriptor).is_none());
        fs::write(
            plugin.join("plugin.yaml"),
            "name: tree_ring_memory\nversion: 3.2.0\n",
        )
        .unwrap();
        assert!(
            read_agent_zero_plugin_manifest(&project, Path::new(AGENT_ZERO_CAPABILITY_FILE))
                .is_none()
        );

        let inside_project = project.join("plugin");
        fs::create_dir_all(&inside_project).unwrap();
        let inside_descriptor = write_capability_descriptor(&inside_project, true);
        assert!(read_agent_zero_plugin_manifest(&project, &inside_descriptor).is_none());

        #[cfg(unix)]
        {
            fs::write(&descriptor, capability_document(true)).unwrap();
            let symlink_parent = temp.path().join("symlink-plugin");
            fs::create_dir_all(&symlink_parent).unwrap();
            fs::write(
                symlink_parent.join("plugin.yaml"),
                "name: tree_ring_memory\nversion: 3.2.0\n",
            )
            .unwrap();
            let symlink_descriptor = symlink_parent.join(AGENT_ZERO_CAPABILITY_FILE);
            symlink(&descriptor, &symlink_descriptor).unwrap();
            assert!(read_agent_zero_plugin_manifest(&project, &symlink_descriptor).is_none());
        }
    }

    fn write_capability_descriptor(plugin: &Path, enabled: bool) -> PathBuf {
        fs::write(
            plugin.join("plugin.yaml"),
            "name: tree_ring_memory\nversion: 3.2.0\n",
        )
        .unwrap();
        let descriptor = plugin.join(AGENT_ZERO_CAPABILITY_FILE);
        fs::write(&descriptor, capability_document(enabled)).unwrap();
        descriptor
    }

    fn capability_document(enabled: bool) -> String {
        format!(
            r#"{{"schema_version":1,"kind":"tree-ring-agent-zero-plugin-capability","plugin_id":"tree_ring_memory","plugin_version":"3.2.0","activation_protocol_version":1,"tree_ring_version":{{"min":"0.15.3","minor":"0.15"}},"enabled":{enabled}}}"#
        )
    }
}
