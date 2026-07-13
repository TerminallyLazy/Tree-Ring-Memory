use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{de::Deserializer, Deserialize, Serialize};

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

        let mut expected_file_pairs = HashSet::new();
        for (index, expectation) in self.expected_files.iter().enumerate() {
            let path_key = canonical_workflow_path(
                &format!("workflow scenario expected_files[{index}].path"),
                &expectation.path,
            )?;
            validate_nonblank(
                &format!("workflow scenario expected_files[{index}].contains"),
                &expectation.contains,
            )?;
            if !expected_file_pairs.insert((path_key, expectation.contains.clone())) {
                return Err(TreeRingError::Validation(format!(
                    "workflow scenario expected_files[{index}] duplicates path and contains"
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
    pub contains: String,
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
    pub contains: String,
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
                let path = workspace_root.join(path_key);
                let exists = path.is_file();
                let passed = exists
                    && fs::read_to_string(&path)
                        .map(|content| content.contains(&expectation.contains))
                        .unwrap_or(false);
                (exists, passed)
            } else {
                (false, false)
            };

            WorkflowFileCheckReport {
                path: expectation.path.clone(),
                contains: expectation.contains.clone(),
                exists,
                passed,
            }
        })
        .collect()
}

fn validate_nonblank(field: &str, value: &str) -> TreeRingResult<()> {
    if value.trim().is_empty() {
        return Err(TreeRingError::Validation(format!("{field} is required")));
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
