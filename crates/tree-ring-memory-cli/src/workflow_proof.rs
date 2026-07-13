use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tree_ring_memory_core::{
    evaluate_workspace, now_iso, parse_workflow_scenario, MemoryEvent, WorkflowAgentRequest,
    WorkflowAgentResponse, WorkflowArm, WorkflowFileCheckReport, WorkflowMemoryContext,
    WorkflowScenario,
};
use tree_ring_memory_sqlite::{MemoryRetriever, SQLiteMemoryStore};

const REPORT_SCHEMA_VERSION: u8 = 1;
const RECALL_LIMIT: usize = 8;
const CODEX_SCHEMA_FILE: &str = ".tree-ring-workflow-schema.json";
const CODEX_RESPONSE_FILE: &str = ".tree-ring-workflow-response.json";

pub trait WorkflowAgent {
    fn execute(&self, request: &WorkflowAgentRequest) -> Result<WorkflowAgentResponse, String>;
}

pub struct CodexWorkflowAgent {
    binary: PathBuf,
    model: Option<String>,
}

impl CodexWorkflowAgent {
    pub fn new(binary: PathBuf, model: Option<String>) -> Self {
        Self { binary, model }
    }
}

impl WorkflowAgent for CodexWorkflowAgent {
    fn execute(&self, request: &WorkflowAgentRequest) -> Result<WorkflowAgentResponse, String> {
        fs::create_dir_all(&request.workspace_root)
            .map_err(|error| format!("workspace_create_error: {error}"))?;

        let schema_path = request.workspace_root.join(CODEX_SCHEMA_FILE);
        let response_path = request.workspace_root.join(CODEX_RESPONSE_FILE);
        let schema = serde_json::to_string_pretty(&codex_response_schema())
            .map_err(|error| format!("codex_schema_encode_error: {error}"))?;
        fs::write(&schema_path, schema)
            .map_err(|error| format!("codex_schema_write_error: {error}"))?;
        if response_path.exists() {
            fs::remove_file(&response_path)
                .map_err(|error| format!("codex_response_cleanup_error: {error}"))?;
        }

        let prompt = codex_prompt(request)?;
        let mut command = Command::new(&self.binary);
        command
            .arg("exec")
            .arg("--ephemeral")
            .arg("--sandbox")
            .arg("workspace-write")
            .arg("--cd")
            .arg(&request.workspace_root)
            .arg("--output-schema")
            .arg(&schema_path)
            .arg("--output-last-message")
            .arg(&response_path);
        if let Some(model) = &self.model {
            command.arg("--model").arg(model);
        }
        let status = command
            .arg(prompt)
            .status()
            .map_err(|error| format!("codex_exec_spawn_error: {error}"))?;
        if !status.success() {
            return Err(format!("codex_exec_failed: {status}"));
        }

        let output = fs::read_to_string(&response_path)
            .map_err(|error| format!("codex_response_read_error: {error}"))?;
        let response = serde_json::from_str::<WorkflowAgentResponse>(&output)
            .map_err(|error| format!("codex_response_parse_error: {error}"))?;
        response
            .validate()
            .map_err(|error| format!("codex_response_validation_error: {error}"))?;
        Ok(response)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowProofTrialStatus {
    Pass,
    Fail,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowProofTrialReport {
    pub arm: WorkflowArm,
    pub workspace: String,
    pub memory_context: Vec<WorkflowMemoryContext>,
    pub agent_response: Option<WorkflowAgentResponse>,
    pub file_checks: Vec<WorkflowFileCheckReport>,
    pub status: WorkflowProofTrialStatus,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowProofScenarioReport {
    pub name: String,
    pub scenario_id: String,
    pub trials: Vec<WorkflowProofTrialReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowProofArmSummary {
    pub arm: WorkflowArm,
    pub pass_count: usize,
    pub fail_count: usize,
    pub error_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowProofReport {
    pub schema_version: u8,
    pub generated_at: String,
    pub scenario_count: usize,
    pub trial_count: usize,
    pub arm_summaries: Vec<WorkflowProofArmSummary>,
    pub scenarios: Vec<WorkflowProofScenarioReport>,
    pub tree_ring_wins_over_no_memory: usize,
    pub tree_ring_wins_over_raw_memory: usize,
    pub tree_ring_complete: bool,
}

pub fn run_workflow_proof(
    fixture_dir: &Path,
    output_dir: &Path,
    agent: &impl WorkflowAgent,
) -> Result<WorkflowProofReport, String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("output_directory_create_error: {error}"))?;

    let fixture_paths = sorted_fixture_paths(fixture_dir)?;
    if fixture_paths.is_empty() {
        return Err("no workflow scenario JSON files found".to_string());
    }

    let mut scenario_ids = BTreeSet::new();
    let mut scenarios = Vec::with_capacity(fixture_paths.len());
    for path in fixture_paths {
        let input = fs::read_to_string(&path)
            .map_err(|error| format!("workflow_fixture_read_error {}: {error}", path.display()))?;
        let scenario = parse_workflow_scenario(&input)
            .map_err(|error| format!("workflow_fixture_parse_error {}: {error}", path.display()))?;
        let scenario_id = unique_scenario_id(&scenario.name, &mut scenario_ids);
        scenarios.push(run_scenario(&scenario, &scenario_id, output_dir, agent)?);
    }

    let report = summarize(scenarios);
    write_reports(output_dir, &report)?;
    Ok(report)
}

fn run_scenario(
    scenario: &WorkflowScenario,
    scenario_id: &str,
    output_dir: &Path,
    agent: &impl WorkflowAgent,
) -> Result<WorkflowProofScenarioReport, String> {
    let raw_memory = visible_seed_memories(scenario);
    let tree_ring_memory = recalled_memories(scenario)?;
    let mut trials = Vec::with_capacity(3);

    for arm in [
        WorkflowArm::NoMemory,
        WorkflowArm::RawMemory,
        WorkflowArm::TreeRing,
    ] {
        let memory_context = match arm {
            WorkflowArm::NoMemory => Vec::new(),
            WorkflowArm::RawMemory => raw_memory.clone(),
            WorkflowArm::TreeRing => tree_ring_memory.clone(),
        };
        trials.push(run_trial(
            scenario,
            scenario_id,
            arm,
            memory_context,
            output_dir,
            agent,
        )?);
    }

    Ok(WorkflowProofScenarioReport {
        name: scenario.name.clone(),
        scenario_id: scenario_id.to_string(),
        trials,
    })
}

fn run_trial(
    scenario: &WorkflowScenario,
    scenario_id: &str,
    arm: WorkflowArm,
    memory_context: Vec<WorkflowMemoryContext>,
    output_dir: &Path,
    agent: &impl WorkflowAgent,
) -> Result<WorkflowProofTrialReport, String> {
    let workspace = output_dir
        .join("trials")
        .join(scenario_id)
        .join(arm_directory(&arm))
        .join("workspace");
    materialize_workspace(&workspace, scenario)?;

    let request = WorkflowAgentRequest::new(
        scenario_id.to_string(),
        arm.clone(),
        scenario.task.clone(),
        workspace.clone(),
        memory_context.clone(),
    );
    let mut errors = Vec::new();
    let agent_response = match agent.execute(&request) {
        Ok(response) => {
            if let Err(error) = response.validate() {
                errors.push(format!("agent_response_validation_error: {error}"));
            }
            let known_memory_ids = memory_context
                .iter()
                .map(|memory| memory.id.as_str())
                .collect::<BTreeSet<_>>();
            for memory_id in &response.used_memory_ids {
                if !known_memory_ids.contains(memory_id.as_str()) {
                    errors.push(format!("used_memory_id_not_in_context: {memory_id}"));
                }
            }
            Some(response)
        }
        Err(error) => {
            errors.push(format!("agent_execution_error: {error}"));
            None
        }
    };
    let file_checks = evaluate_workspace(scenario, &workspace);
    let status = if errors.is_empty() {
        if file_checks.iter().all(|check| check.passed) {
            WorkflowProofTrialStatus::Pass
        } else {
            WorkflowProofTrialStatus::Fail
        }
    } else {
        WorkflowProofTrialStatus::Error
    };

    Ok(WorkflowProofTrialReport {
        arm,
        workspace: workspace
            .strip_prefix(output_dir)
            .unwrap_or(&workspace)
            .display()
            .to_string(),
        memory_context,
        agent_response,
        file_checks,
        status,
        errors,
    })
}

fn visible_seed_memories(scenario: &WorkflowScenario) -> Vec<WorkflowMemoryContext> {
    scenario
        .seed_memories
        .iter()
        .filter(|memory| memory.sensitivity == "normal" && memory.superseded_by.is_none())
        .map(project_memory)
        .collect()
}

fn recalled_memories(scenario: &WorkflowScenario) -> Result<Vec<WorkflowMemoryContext>, String> {
    let mut store = SQLiteMemoryStore::open(":memory:")
        .map_err(|error| format!("workflow_store_open_error: {error}"))?;
    store
        .put_many(&scenario.seed_memories)
        .map_err(|error| format!("workflow_seed_write_error: {error}"))?;
    let recalled = MemoryRetriever::new(&store)
        .recall(
            &scenario.task,
            None,
            None,
            None,
            None,
            None,
            false,
            false,
            RECALL_LIMIT,
            false,
        )
        .map_err(|error| format!("workflow_recall_error: {error}"))?;
    Ok(recalled
        .into_iter()
        .map(|result| project_memory(&result.memory))
        .collect())
}

fn project_memory(memory: &MemoryEvent) -> WorkflowMemoryContext {
    WorkflowMemoryContext {
        id: memory.id.clone(),
        summary: memory.summary.clone(),
        details: memory.details.clone(),
        ring: memory.ring.clone(),
        event_type: memory.event_type.clone(),
        source_ref: memory.source.ref_.clone(),
        confidence: memory.confidence,
    }
}

fn materialize_workspace(workspace: &Path, scenario: &WorkflowScenario) -> Result<(), String> {
    if workspace.exists() {
        fs::remove_dir_all(workspace)
            .map_err(|error| format!("workspace_cleanup_error {}: {error}", workspace.display()))?;
    }
    fs::create_dir_all(workspace)
        .map_err(|error| format!("workspace_create_error {}: {error}", workspace.display()))?;

    for file in &scenario.workspace_files {
        let path = workspace.join(&file.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "workspace_parent_create_error {}: {error}",
                    parent.display()
                )
            })?;
        }
        fs::write(&path, &file.content)
            .map_err(|error| format!("workspace_file_write_error {}: {error}", path.display()))?;
    }
    Ok(())
}

fn sorted_fixture_paths(fixture_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(fixture_dir).map_err(|error| {
        format!(
            "fixture_directory_read_error {}: {error}",
            fixture_dir.display()
        )
    })?;
    let mut paths = entries
        .map(|entry| {
            entry.map(|entry| entry.path()).map_err(|error| {
                format!(
                    "fixture_directory_read_error {}: {error}",
                    fixture_dir.display()
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.is_file() && path.extension() == Some(OsStr::new("json")));
    paths.sort();
    Ok(paths)
}

fn unique_scenario_id(name: &str, used: &mut BTreeSet<String>) -> String {
    let base = safe_scenario_name(name);
    let mut candidate = base.clone();
    let mut suffix = 2;
    while !used.insert(candidate.clone()) {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }
    candidate
}

fn safe_scenario_name(name: &str) -> String {
    let name = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let name = name.trim_matches('-');
    if name.is_empty() {
        "scenario".to_string()
    } else {
        name.to_string()
    }
}

fn arm_directory(arm: &WorkflowArm) -> &'static str {
    match arm {
        WorkflowArm::NoMemory => "no_memory",
        WorkflowArm::RawMemory => "raw_memory",
        WorkflowArm::TreeRing => "tree_ring",
    }
}

fn summarize(scenarios: Vec<WorkflowProofScenarioReport>) -> WorkflowProofReport {
    let arm_summaries = [
        WorkflowArm::NoMemory,
        WorkflowArm::RawMemory,
        WorkflowArm::TreeRing,
    ]
    .into_iter()
    .map(|arm| summarize_arm(&scenarios, arm))
    .collect::<Vec<_>>();
    let tree_ring_wins_over_no_memory = scenarios
        .iter()
        .filter(|scenario| observed_win(scenario, WorkflowArm::NoMemory))
        .count();
    let tree_ring_wins_over_raw_memory = scenarios
        .iter()
        .filter(|scenario| observed_win(scenario, WorkflowArm::RawMemory))
        .count();
    let tree_ring_complete = !scenarios.is_empty()
        && scenarios.iter().all(|scenario| {
            status_for(scenario, &WorkflowArm::TreeRing) == Some(WorkflowProofTrialStatus::Pass)
        });
    let scenario_count = scenarios.len();
    let trial_count = scenarios.iter().map(|scenario| scenario.trials.len()).sum();

    WorkflowProofReport {
        schema_version: REPORT_SCHEMA_VERSION,
        generated_at: now_iso(),
        scenario_count,
        trial_count,
        arm_summaries,
        scenarios,
        tree_ring_wins_over_no_memory,
        tree_ring_wins_over_raw_memory,
        tree_ring_complete,
    }
}

fn summarize_arm(
    scenarios: &[WorkflowProofScenarioReport],
    arm: WorkflowArm,
) -> WorkflowProofArmSummary {
    let mut pass_count = 0;
    let mut fail_count = 0;
    let mut error_count = 0;
    for scenario in scenarios {
        match status_for(scenario, &arm) {
            Some(WorkflowProofTrialStatus::Pass) => pass_count += 1,
            Some(WorkflowProofTrialStatus::Fail) => fail_count += 1,
            Some(WorkflowProofTrialStatus::Error) => error_count += 1,
            None => error_count += 1,
        }
    }
    WorkflowProofArmSummary {
        arm,
        pass_count,
        fail_count,
        error_count,
    }
}

fn observed_win(scenario: &WorkflowProofScenarioReport, control: WorkflowArm) -> bool {
    status_for(scenario, &WorkflowArm::TreeRing) == Some(WorkflowProofTrialStatus::Pass)
        && status_for(scenario, &control) == Some(WorkflowProofTrialStatus::Fail)
}

fn status_for(
    scenario: &WorkflowProofScenarioReport,
    arm: &WorkflowArm,
) -> Option<WorkflowProofTrialStatus> {
    scenario
        .trials
        .iter()
        .find(|trial| &trial.arm == arm)
        .map(|trial| trial.status.clone())
}

fn write_reports(output_dir: &Path, report: &WorkflowProofReport) -> Result<(), String> {
    let json = serde_json::to_string_pretty(report)
        .map_err(|error| format!("workflow_report_encode_error: {error}"))?;
    fs::write(output_dir.join("workflow-proof-report.json"), json)
        .map_err(|error| format!("workflow_report_write_error: {error}"))?;
    fs::write(
        output_dir.join("workflow-proof-summary.md"),
        markdown_summary(report),
    )
    .map_err(|error| format!("workflow_summary_write_error: {error}"))?;
    Ok(())
}

fn markdown_summary(report: &WorkflowProofReport) -> String {
    let mut lines = vec![
        "# Tree Ring Workflow Proof Summary".to_string(),
        String::new(),
        format!("- Tree Ring complete: {}", report.tree_ring_complete),
        format!(
            "- observed Tree Ring wins over no-memory: {}",
            report.tree_ring_wins_over_no_memory
        ),
        format!(
            "- observed Tree Ring wins over raw-memory: {}",
            report.tree_ring_wins_over_raw_memory
        ),
        format!("- scenarios: {}", report.scenario_count),
        format!("- trials: {}", report.trial_count),
        String::new(),
        "## Arm summaries".to_string(),
        String::new(),
    ];
    for summary in &report.arm_summaries {
        lines.push(format!(
            "- {}: pass={}, fail={}, error={}",
            arm_directory(&summary.arm),
            summary.pass_count,
            summary.fail_count,
            summary.error_count
        ));
    }
    lines.extend([String::new(), "## Scenarios".to_string(), String::new()]);
    for scenario in &report.scenarios {
        let statuses = scenario
            .trials
            .iter()
            .map(|trial| {
                format!(
                    "{}={}",
                    arm_directory(&trial.arm),
                    status_label(&trial.status)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("- `{}`: {statuses}", scenario.name));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn status_label(status: &WorkflowProofTrialStatus) -> &'static str {
    match status {
        WorkflowProofTrialStatus::Pass => "pass",
        WorkflowProofTrialStatus::Fail => "fail",
        WorkflowProofTrialStatus::Error => "error",
    }
}

fn codex_response_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["summary", "used_memory_ids"],
        "properties": {
            "summary": { "type": "string" },
            "used_memory_ids": {
                "type": "array",
                "items": { "type": "string" }
            }
        }
    })
}

fn codex_prompt(request: &WorkflowAgentRequest) -> Result<String, String> {
    let memory_context = serde_json::to_string_pretty(&request.memory_context)
        .map_err(|error| format!("codex_prompt_encode_error: {error}"))?;
    Ok(format!(
        "work only in the workspace.\n\
use source/task files over memory when they conflict.\n\
do not seek validators or fixtures.\n\
return only the response schema fields `summary` and `used_memory_ids`.\n\n\
task:\n{}\n\n\
memory_context:\n{}",
        request.task, memory_context
    ))
}
