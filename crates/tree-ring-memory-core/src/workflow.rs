use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::models::{MemoryEvent, TreeRingError, TreeRingResult};

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
    #[serde(default)]
    pub seed_memories: Vec<MemoryEvent>,
    #[serde(default)]
    pub workspace_files: Vec<WorkflowWorkspaceFile>,
    #[serde(default)]
    pub expected_files: Vec<WorkflowFileExpectation>,
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
            validate_safe_relative_path(
                &format!("workflow scenario workspace_files[{index}].path"),
                &file.path,
            )?;
            if !workspace_paths.insert(file.path.as_str()) {
                return Err(TreeRingError::Validation(format!(
                    "workflow scenario workspace_files[{index}] duplicates path {}",
                    file.path
                )));
            }
        }

        let mut expected_file_pairs = HashSet::new();
        for (index, expectation) in self.expected_files.iter().enumerate() {
            validate_safe_relative_path(
                &format!("workflow scenario expected_files[{index}].path"),
                &expectation.path,
            )?;
            validate_nonblank(
                &format!("workflow scenario expected_files[{index}].contains"),
                &expectation.contains,
            )?;
            if !expected_file_pairs
                .insert((expectation.path.as_str(), expectation.contains.as_str()))
            {
                return Err(TreeRingError::Validation(format!(
                    "workflow scenario expected_files[{index}] duplicates path and contains"
                )));
            }
        }

        for memory in &self.seed_memories {
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
            let (exists, passed) =
                if validate_safe_relative_path("workflow file expectation path", &expectation.path)
                    .is_ok()
                {
                    let path = workspace_root.join(&expectation.path);
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

fn validate_safe_relative_path(field: &str, value: &str) -> TreeRingResult<()> {
    if value.trim().is_empty() {
        return Err(TreeRingError::Validation(format!(
            "{field} must not be empty"
        )));
    }

    let path = Path::new(value);
    if path.is_absolute() {
        return Err(TreeRingError::Validation(format!(
            "{field} must be relative"
        )));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(TreeRingError::Validation(format!(
            "{field} must not contain parent directory components"
        )));
    }

    Ok(())
}
