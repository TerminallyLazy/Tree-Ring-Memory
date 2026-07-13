use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::{
    ffi::{CString, OsStr},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::Component,
};

use serde::{de::Deserializer, Deserialize, Serialize};
use serde_json::Value;

use crate::models::{
    MemoryEvent, MemoryLink, MemoryReview, MemorySource, TreeRingError, TreeRingResult,
};

const WORKFLOW_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowArm {
    NoMemory,
    RawMemory,
    TreeRing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowScenario {
    pub name: String,
    pub task: String,
    #[serde(default, deserialize_with = "deserialize_seed_memories")]
    pub seed_memories: Vec<MemoryEvent>,
    #[serde(default)]
    pub workspace_files: Vec<WorkflowWorkspaceFile>,
    #[serde(default)]
    pub expected_files: Vec<WorkflowFileExpectation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowSeedMemory {
    id: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    agent_profile: Option<String>,
    #[serde(default = "default_workflow_seed_scope")]
    scope: String,
    #[serde(default = "default_workflow_seed_ring")]
    ring: String,
    event_type: String,
    summary: String,
    #[serde(default)]
    details: String,
    #[serde(default)]
    source: WorkflowSeedMemorySource,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default = "default_workflow_seed_score")]
    salience: f64,
    #[serde(default = "default_workflow_seed_score")]
    confidence: f64,
    #[serde(default = "default_workflow_seed_sensitivity")]
    sensitivity: String,
    #[serde(default = "default_workflow_seed_retention")]
    retention: String,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    supersedes: Vec<String>,
    #[serde(default)]
    superseded_by: Option<String>,
    #[serde(default)]
    links: Vec<WorkflowSeedMemoryLink>,
    #[serde(default)]
    review: WorkflowSeedMemoryReview,
}

impl From<WorkflowSeedMemory> for MemoryEvent {
    fn from(seed_memory: WorkflowSeedMemory) -> Self {
        Self {
            id: seed_memory.id,
            created_at: seed_memory.created_at,
            updated_at: seed_memory.updated_at,
            project: seed_memory.project,
            agent_profile: seed_memory.agent_profile,
            scope: seed_memory.scope,
            ring: seed_memory.ring,
            event_type: seed_memory.event_type,
            summary: seed_memory.summary,
            details: seed_memory.details,
            source: seed_memory.source.into(),
            tags: seed_memory.tags,
            salience: seed_memory.salience,
            confidence: seed_memory.confidence,
            sensitivity: seed_memory.sensitivity,
            retention: seed_memory.retention,
            expires_at: seed_memory.expires_at,
            supersedes: seed_memory.supersedes,
            superseded_by: seed_memory.superseded_by,
            links: seed_memory.links.into_iter().map(Into::into).collect(),
            review: seed_memory.review.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowSeedMemorySource {
    #[serde(rename = "type", default = "default_workflow_seed_source_type")]
    source_type: String,
    #[serde(rename = "ref", default)]
    ref_: String,
    #[serde(default)]
    quote: String,
}

impl Default for WorkflowSeedMemorySource {
    fn default() -> Self {
        Self {
            source_type: default_workflow_seed_source_type(),
            ref_: String::new(),
            quote: String::new(),
        }
    }
}

impl From<WorkflowSeedMemorySource> for MemorySource {
    fn from(source: WorkflowSeedMemorySource) -> Self {
        Self {
            source_type: source.source_type,
            ref_: source.ref_,
            quote: source.quote,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowSeedMemoryLink {
    #[serde(rename = "type")]
    link_type: String,
    target: String,
}

impl From<WorkflowSeedMemoryLink> for MemoryLink {
    fn from(link: WorkflowSeedMemoryLink) -> Self {
        Self {
            link_type: link.link_type,
            target: link.target,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowSeedMemoryReview {
    #[serde(default)]
    needs_review: bool,
    #[serde(default)]
    review_reason: Option<String>,
    #[serde(default)]
    reviewed_at: Option<String>,
    #[serde(default)]
    reviewed_by: Option<String>,
}

impl From<WorkflowSeedMemoryReview> for MemoryReview {
    fn from(review: WorkflowSeedMemoryReview) -> Self {
        Self {
            needs_review: review.needs_review,
            review_reason: review.review_reason,
            reviewed_at: review.reviewed_at,
            reviewed_by: review.reviewed_by,
        }
    }
}

impl WorkflowScenario {
    pub fn validate(&self) -> TreeRingResult<()> {
        validate_nonblank("workflow scenario name", &self.name)?;
        validate_nonblank("workflow scenario task", &self.task)?;

        if self.expected_files.is_empty() {
            return Err(TreeRingError::Validation(
                "workflow scenario requires at least one expected_file".to_string(),
            ));
        }

        let mut workspace_paths = HashSet::new();
        for (index, file) in self.workspace_files.iter().enumerate() {
            let path_key = canonical_workflow_path(
                &format!("workflow scenario workspace_files[{index}].path"),
                &file.path,
            )?;
            if !workspace_paths.insert(path_key) {
                return Err(TreeRingError::Validation(format!(
                    "workflow scenario workspace_files[{index}] duplicates path {}",
                    file.path
                )));
            }
        }

        let mut expected_file_checks = HashSet::new();
        for (index, expectation) in self.expected_files.iter().enumerate() {
            let path_key = canonical_workflow_path(
                &format!("workflow scenario expected_files[{index}].path"),
                &expectation.path,
            )?;
            let check_key = match expectation
                .check_mode(&format!("workflow scenario expected_files[{index}]"))?
            {
                WorkflowFileCheckMode::Contains(contains) => format!("contains:{contains}"),
                WorkflowFileCheckMode::JsonFields(json_fields) => {
                    format!("json_fields:{}", serde_json::to_string(json_fields)?)
                }
            };
            if !expected_file_checks.insert((path_key, check_key)) {
                return Err(TreeRingError::Validation(format!(
                    "workflow scenario expected_files[{index}] duplicates path and check"
                )));
            }
        }

        let mut seed_memory_ids = HashSet::new();
        for (index, memory) in self.seed_memories.iter().enumerate() {
            validate_nonblank(
                &format!("workflow scenario seed_memories[{index}].id"),
                &memory.id,
            )?;
            if !seed_memory_ids.insert(memory.id.as_str()) {
                return Err(TreeRingError::Validation(format!(
                    "workflow scenario seed_memories[{index}] duplicates memory id {}",
                    memory.id
                )));
            }
            memory.validate()?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowWorkspaceFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowFileExpectation {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contains: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_fields: Option<BTreeMap<String, Value>>,
}

impl WorkflowFileExpectation {
    fn check_mode(&self, field: &str) -> TreeRingResult<WorkflowFileCheckMode<'_>> {
        match (&self.contains, &self.json_fields) {
            (Some(contains), None) => {
                validate_nonblank(&format!("{field}.contains"), contains)?;
                Ok(WorkflowFileCheckMode::Contains(contains))
            }
            (None, Some(json_fields)) => {
                if json_fields.is_empty() {
                    return Err(TreeRingError::Validation(format!(
                        "{field}.json_fields requires at least one JSON pointer"
                    )));
                }
                for pointer in json_fields.keys() {
                    validate_json_pointer(&format!("{field}.json_fields"), pointer)?;
                }
                Ok(WorkflowFileCheckMode::JsonFields(json_fields))
            }
            _ => Err(TreeRingError::Validation(format!(
                "{field} requires exactly one check mode: contains or json_fields"
            ))),
        }
    }
}

enum WorkflowFileCheckMode<'a> {
    Contains(&'a str),
    JsonFields(&'a BTreeMap<String, Value>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowMemoryContext {
    pub id: String,
    pub summary: String,
    pub details: String,
    pub ring: String,
    pub event_type: String,
    pub source_ref: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAgentRequest {
    pub schema_version: u8,
    pub scenario_id: String,
    pub arm: WorkflowArm,
    pub task: String,
    pub workspace_root: PathBuf,
    pub memory_context: Vec<WorkflowMemoryContext>,
}

impl WorkflowAgentRequest {
    pub fn new(
        scenario_id: String,
        arm: WorkflowArm,
        task: String,
        workspace_root: PathBuf,
        memory_context: Vec<WorkflowMemoryContext>,
    ) -> Self {
        Self {
            schema_version: WORKFLOW_SCHEMA_VERSION,
            scenario_id,
            arm,
            task,
            workspace_root,
            memory_context,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowAgentResponse {
    pub summary: String,
    #[serde(default)]
    pub used_memory_ids: Vec<String>,
}

impl WorkflowAgentResponse {
    pub fn validate(&self) -> TreeRingResult<()> {
        validate_nonblank("workflow agent response summary", &self.summary)?;

        let mut used_memory_ids = HashSet::new();
        for (index, memory_id) in self.used_memory_ids.iter().enumerate() {
            validate_nonblank(
                &format!("workflow agent response used_memory_ids[{index}]"),
                memory_id,
            )?;
            if !used_memory_ids.insert(memory_id.as_str()) {
                return Err(TreeRingError::Validation(format!(
                    "workflow agent response used_memory_ids[{index}] duplicates memory id {memory_id}"
                )));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowFileCheckReport {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contains: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json_fields: Option<BTreeMap<String, Value>>,
    pub exists: bool,
    pub passed: bool,
}

pub fn parse_workflow_scenario(input: &str) -> TreeRingResult<WorkflowScenario> {
    let scenario: WorkflowScenario = serde_json::from_str(input)?;
    scenario.validate()?;
    Ok(scenario)
}

pub fn evaluate_workspace(
    scenario: &WorkflowScenario,
    workspace_root: &Path,
) -> Vec<WorkflowFileCheckReport> {
    scenario
        .expected_files
        .iter()
        .map(|expectation| {
            let (exists, passed) = if let Ok(path_key) =
                canonical_workflow_path("workflow file expectation path", &expectation.path)
            {
                match read_regular_workspace_file(workspace_root, &path_key) {
                    Some(content) => (true, evaluate_file_content(expectation, &content)),
                    None => (false, false),
                }
            } else {
                (false, false)
            };

            WorkflowFileCheckReport {
                path: expectation.path.clone(),
                contains: expectation.contains.clone(),
                json_fields: expectation.json_fields.clone(),
                exists,
                passed,
            }
        })
        .collect()
}

fn read_regular_workspace_file(workspace_root: &Path, path_key: &str) -> Option<String> {
    let mut file = open_regular_workspace_file(workspace_root, path_key)?;
    let mut content = String::new();
    file.read_to_string(&mut content).ok()?;
    Some(content)
}

#[cfg(unix)]
fn open_regular_workspace_file(workspace_root: &Path, path_key: &str) -> Option<fs::File> {
    let mut directory = open_workspace_directory_no_follow(workspace_root)?;
    let mut segments = path_key.split('/').peekable();

    while let Some(segment) = segments.next() {
        let is_final = segments.peek().is_none();
        let file = open_child_no_follow(&directory, OsStr::new(segment), is_final)?;

        if is_final {
            let metadata = file.metadata().ok()?;
            // A link count above one can make the output name an alias for data outside the
            // workspace. Workflow artifacts do not need hard-link semantics, so reject them.
            return (metadata.is_file() && metadata.nlink() == 1).then_some(file);
        }

        if !file.metadata().ok()?.is_dir() {
            return None;
        }
        directory = file;
    }

    None
}

#[cfg(not(unix))]
fn open_regular_workspace_file(_workspace_root: &Path, _path_key: &str) -> Option<fs::File> {
    // The evaluator fails closed until a descriptor-relative, no-follow implementation is
    // available for this platform. A pathname-based fallback would reintroduce a TOCTOU read.
    None
}

#[cfg(unix)]
fn open_workspace_directory_no_follow(workspace_root: &Path) -> Option<fs::File> {
    if workspace_root.as_os_str().is_empty() {
        return None;
    }

    let is_absolute = workspace_root.is_absolute();
    let mut directory = open_trusted_directory_anchor(is_absolute)?;

    for component in workspace_root.components() {
        match component {
            Component::RootDir if is_absolute => {}
            Component::CurDir => {}
            Component::Normal(segment) => {
                let next_directory = open_child_no_follow(&directory, segment, false)?;
                if !next_directory.metadata().ok()?.is_dir() {
                    return None;
                }
                directory = next_directory;
            }
            Component::RootDir | Component::ParentDir | Component::Prefix(_) => return None,
        }
    }

    Some(directory)
}

#[cfg(unix)]
fn open_trusted_directory_anchor(is_absolute: bool) -> Option<fs::File> {
    let anchor = CString::new(if is_absolute { "/" } else { "." }).ok()?;
    let flags =
        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NONBLOCK;
    let descriptor = unsafe {
        // SAFETY: `anchor` is one of the NUL-terminated paths "/" or ".", and the returned
        // descriptor is immediately owned by `File`, which closes it on every return path.
        // Workspace paths are never opened by pathname.
        libc::open(anchor.as_ptr(), flags)
    };
    let directory = file_from_descriptor(descriptor)?;
    directory.metadata().ok()?.is_dir().then_some(directory)
}

#[cfg(unix)]
fn open_child_no_follow(directory: &fs::File, segment: &OsStr, is_final: bool) -> Option<fs::File> {
    let segment = CString::new(segment.as_bytes()).ok()?;
    let mut flags = libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK;
    if !is_final {
        flags |= libc::O_DIRECTORY;
    }
    let descriptor = unsafe {
        // SAFETY: `directory` owns a live directory descriptor, and `segment` is a NUL-terminated
        // single path component. The returned descriptor is immediately owned by `File`.
        libc::openat(directory.as_raw_fd(), segment.as_ptr(), flags)
    };
    file_from_descriptor(descriptor)
}

#[cfg(unix)]
fn file_from_descriptor(descriptor: libc::c_int) -> Option<fs::File> {
    if descriptor < 0 {
        return None;
    }

    Some(unsafe {
        // SAFETY: `open` and `openat` return an owned descriptor when it is non-negative.
        fs::File::from_raw_fd(descriptor)
    })
}

fn validate_nonblank(field: &str, value: &str) -> TreeRingResult<()> {
    if value.trim().is_empty() {
        return Err(TreeRingError::Validation(format!("{field} is required")));
    }
    Ok(())
}

fn evaluate_file_content(expectation: &WorkflowFileExpectation, content: &str) -> bool {
    match expectation.check_mode("workflow file expectation") {
        Ok(WorkflowFileCheckMode::Contains(contains)) => content.contains(contains),
        Ok(WorkflowFileCheckMode::JsonFields(json_fields)) => {
            serde_json::from_str::<Value>(content)
                .map(|document| {
                    json_fields.iter().all(|(pointer, expected_value)| {
                        document
                            .pointer(pointer)
                            .is_some_and(|actual_value| actual_value == expected_value)
                    })
                })
                .unwrap_or(false)
        }
        Err(_) => false,
    }
}

fn validate_json_pointer(field: &str, pointer: &str) -> TreeRingResult<()> {
    if pointer.is_empty() {
        return Ok(());
    }
    if !pointer.starts_with('/') {
        return Err(TreeRingError::Validation(format!(
            "{field} JSON pointer {pointer:?} must be empty or start with /"
        )));
    }

    let mut characters = pointer.chars();
    while let Some(character) = characters.next() {
        if character == '~' {
            match characters.next() {
                Some('0' | '1') => {}
                _ => {
                    return Err(TreeRingError::Validation(format!(
                        "{field} JSON pointer {pointer:?} has an invalid ~ escape"
                    )));
                }
            }
        }
    }

    Ok(())
}

fn deserialize_seed_memories<'de, D>(deserializer: D) -> Result<Vec<MemoryEvent>, D::Error>
where
    D: Deserializer<'de>,
{
    Vec::<WorkflowSeedMemory>::deserialize(deserializer)
        .map(|seed_memories| seed_memories.into_iter().map(Into::into).collect())
}

fn default_workflow_seed_scope() -> String {
    "global".to_string()
}

fn default_workflow_seed_ring() -> String {
    "cambium".to_string()
}

fn default_workflow_seed_score() -> f64 {
    0.5
}

fn default_workflow_seed_sensitivity() -> String {
    "normal".to_string()
}

fn default_workflow_seed_retention() -> String {
    "normal".to_string()
}

fn default_workflow_seed_source_type() -> String {
    "manual".to_string()
}

fn canonical_workflow_path(field: &str, value: &str) -> TreeRingResult<String> {
    if value.trim().is_empty() {
        return Err(TreeRingError::Validation(format!(
            "{field} must not be empty"
        )));
    }

    if value.starts_with('/') || value.starts_with('\\') {
        return Err(TreeRingError::Validation(format!(
            "{field} must be relative"
        )));
    }
    if has_windows_drive_prefix(value) {
        return Err(TreeRingError::Validation(format!(
            "{field} must not use a Windows drive prefix"
        )));
    }
    if value.contains('\\') {
        return Err(TreeRingError::Validation(format!(
            "{field} must use forward slash separators"
        )));
    }

    let mut segments = Vec::new();
    for segment in value.split('/') {
        if segment.is_empty() {
            return Err(TreeRingError::Validation(format!(
                "{field} must not contain empty path components"
            )));
        }
        if segment == "." {
            return Err(TreeRingError::Validation(format!(
                "{field} must not contain current directory components"
            )));
        }
        if segment == ".." {
            return Err(TreeRingError::Validation(format!(
                "{field} must not contain parent directory components"
            )));
        }
        segments.push(segment);
    }

    Ok(segments.join("/"))
}

fn has_windows_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[cfg(unix)]
    use std::io::Read;

    #[cfg(unix)]
    use std::path::{Path, PathBuf};

    use serde_json::json;
    use tempfile::tempdir;

    use super::{evaluate_workspace, WorkflowFileExpectation, WorkflowScenario};

    #[cfg(unix)]
    use super::{open_regular_workspace_file, open_workspace_directory_no_follow};

    fn scenario_with_expectations(
        expected_files: Vec<WorkflowFileExpectation>,
    ) -> WorkflowScenario {
        WorkflowScenario {
            name: "workspace evaluation".to_string(),
            task: "Check the generated files.".to_string(),
            seed_memories: Vec::new(),
            workspace_files: Vec::new(),
            expected_files,
        }
    }

    #[cfg(not(unix))]
    #[test]
    fn evaluate_workspace_fails_closed_without_descriptor_relative_support() {
        let workspace = tempdir().unwrap();
        fs::write(
            workspace.path().join("decision.md"),
            "Choose the safe action.",
        )
        .unwrap();
        let scenario = scenario_with_expectations(vec![WorkflowFileExpectation {
            path: "decision.md".to_string(),
            contains: Some("safe action".to_string()),
            json_fields: None,
        }]);

        let reports = evaluate_workspace(&scenario, workspace.path());

        assert_eq!(reports.len(), 1);
        assert!(!reports[0].exists);
        assert!(!reports[0].passed);
    }

    #[cfg(unix)]
    fn physical_path(path: &Path) -> PathBuf {
        fs::canonicalize(path).expect("test path must resolve to its physical location")
    }

    #[cfg(unix)]
    #[test]
    fn evaluate_workspace_rejects_a_final_symlinked_expected_file() {
        use std::os::unix::fs::symlink;

        let workspace = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let external_file = outside.path().join("decision.md");
        fs::write(&external_file, "Choose the safe action.").unwrap();
        symlink(&external_file, workspace.path().join("decision.md")).unwrap();
        let scenario = scenario_with_expectations(vec![WorkflowFileExpectation {
            path: "decision.md".to_string(),
            contains: Some("safe action".to_string()),
            json_fields: None,
        }]);

        let reports = evaluate_workspace(&scenario, &physical_path(workspace.path()));

        assert_eq!(reports.len(), 1);
        assert!(!reports[0].exists);
        assert!(!reports[0].passed);
    }

    #[cfg(unix)]
    #[test]
    fn evaluate_workspace_rejects_a_hard_linked_expected_file() {
        let root = tempdir().unwrap();
        let workspace_path = root.path().join("workspace");
        let outside_path = root.path().join("outside");
        fs::create_dir(&workspace_path).unwrap();
        fs::create_dir(&outside_path).unwrap();
        let external_file = outside_path.join("decision.md");
        fs::write(&external_file, "Choose the safe action.").unwrap();
        fs::hard_link(&external_file, workspace_path.join("decision.md")).unwrap();
        let scenario = scenario_with_expectations(vec![WorkflowFileExpectation {
            path: "decision.md".to_string(),
            contains: Some("safe action".to_string()),
            json_fields: None,
        }]);

        let reports = evaluate_workspace(&scenario, &physical_path(&workspace_path));

        assert_eq!(reports.len(), 1);
        assert!(!reports[0].exists);
        assert!(!reports[0].passed);
    }

    #[cfg(unix)]
    #[test]
    fn evaluate_workspace_rejects_a_symlinked_workspace_root() {
        use std::os::unix::fs::symlink;

        let workspace = tempdir().unwrap();
        let parent = tempdir().unwrap();
        let workspace_path = physical_path(workspace.path());
        let parent_path = physical_path(parent.path());
        fs::write(
            workspace_path.join("decision.md"),
            "Choose the safe action.",
        )
        .unwrap();
        let linked_workspace = parent_path.join("linked-workspace");
        symlink(&workspace_path, &linked_workspace).unwrap();
        let scenario = scenario_with_expectations(vec![WorkflowFileExpectation {
            path: "decision.md".to_string(),
            contains: Some("safe action".to_string()),
            json_fields: None,
        }]);

        let reports = evaluate_workspace(&scenario, &linked_workspace);

        assert_eq!(reports.len(), 1);
        assert!(!reports[0].exists);
        assert!(!reports[0].passed);
    }

    #[cfg(unix)]
    #[test]
    fn evaluate_workspace_rejects_a_symlinked_ancestor_of_workspace_root() {
        use std::os::unix::fs::symlink;

        let root = tempdir().unwrap();
        let root_path = physical_path(root.path());
        let trusted_parent = root_path.join("trusted-parent");
        let outside_parent = root_path.join("outside-parent");
        fs::create_dir(&trusted_parent).unwrap();
        fs::create_dir(&outside_parent).unwrap();
        let outside_workspace = outside_parent.join("workspace");
        fs::create_dir(&outside_workspace).unwrap();
        fs::write(
            outside_workspace.join("decision.md"),
            "Choose the safe action.",
        )
        .unwrap();
        symlink(&outside_parent, trusted_parent.join("route")).unwrap();
        let routed_workspace = trusted_parent.join("route/workspace");
        let scenario = scenario_with_expectations(vec![WorkflowFileExpectation {
            path: "decision.md".to_string(),
            contains: Some("safe action".to_string()),
            json_fields: None,
        }]);

        let reports = evaluate_workspace(&scenario, &routed_workspace);

        assert_eq!(reports.len(), 1);
        assert!(!reports[0].exists);
        assert!(!reports[0].passed);
    }

    #[cfg(unix)]
    #[test]
    fn workspace_directory_open_rejects_relative_parent_traversal() {
        assert!(open_workspace_directory_no_follow(Path::new("../outside")).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn evaluate_workspace_rejects_a_symlinked_parent_of_a_structured_expected_file() {
        use std::os::unix::fs::symlink;

        let workspace = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let workspace_path = physical_path(workspace.path());
        fs::write(
            outside.path().join("decision.json"),
            r#"{"decision": {"status": "approved"}}"#,
        )
        .unwrap();
        symlink(outside.path(), workspace_path.join("out")).unwrap();
        let scenario = scenario_with_expectations(vec![WorkflowFileExpectation {
            path: "out/decision.json".to_string(),
            contains: None,
            json_fields: Some([("/decision/status".to_string(), json!("approved"))].into()),
        }]);

        let reports = evaluate_workspace(&scenario, &workspace_path);

        assert_eq!(reports.len(), 1);
        assert!(!reports[0].exists);
        assert!(!reports[0].passed);
    }

    #[cfg(unix)]
    #[test]
    fn approved_workspace_file_stays_bound_after_its_path_is_replaced() {
        use std::os::unix::fs::symlink;

        let workspace = tempdir().unwrap();
        let workspace_path = physical_path(workspace.path());
        let outside = tempdir().unwrap();
        let decision_path = workspace_path.join("decision.md");
        fs::write(&decision_path, "approved workspace content").unwrap();
        let external_file = outside.path().join("decision.md");
        fs::write(&external_file, "external replacement content").unwrap();

        let mut approved_file =
            open_regular_workspace_file(&workspace_path, "decision.md").expect("regular file");
        fs::rename(&decision_path, workspace_path.join("decision.previous.md")).unwrap();
        symlink(&external_file, &decision_path).unwrap();

        let mut content = String::new();
        approved_file.read_to_string(&mut content).unwrap();

        assert_eq!(content, "approved workspace content");
    }

    #[cfg(unix)]
    #[test]
    fn evaluate_workspace_accepts_regular_nested_files_for_both_check_modes() {
        let workspace = tempdir().unwrap();
        let workspace_path = physical_path(workspace.path());
        fs::create_dir_all(workspace_path.join("out")).unwrap();
        fs::write(
            workspace_path.join("out/decision.md"),
            "Choose the safe action.",
        )
        .unwrap();
        fs::write(
            workspace_path.join("out/decision.json"),
            r#"{"decision": {"status": "approved"}}"#,
        )
        .unwrap();
        let scenario = scenario_with_expectations(vec![
            WorkflowFileExpectation {
                path: "out/decision.md".to_string(),
                contains: Some("safe action".to_string()),
                json_fields: None,
            },
            WorkflowFileExpectation {
                path: "out/decision.json".to_string(),
                contains: None,
                json_fields: Some([("/decision/status".to_string(), json!("approved"))].into()),
            },
        ]);

        let reports = evaluate_workspace(&scenario, &workspace_path);

        assert_eq!(reports.len(), 2);
        assert!(reports.iter().all(|report| report.exists));
        assert!(reports.iter().all(|report| report.passed));
    }
}
