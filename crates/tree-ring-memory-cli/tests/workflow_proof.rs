use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tempfile::tempdir;
use tree_ring_memory_cli::workflow_proof::{
    run_workflow_proof, CodexWorkflowAgent, WorkflowAgent, WorkflowProofTrialStatus,
};
use tree_ring_memory_core::{WorkflowAgentRequest, WorkflowAgentResponse, WorkflowArm};

const TARGET_MEMORY_ID: &str = "mem_quality_no_background_writer";

struct FakeAgent {
    requests: Mutex<Vec<WorkflowAgentRequest>>,
    cite_unknown_memory: bool,
}

impl FakeAgent {
    fn new(cite_unknown_memory: bool) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            cite_unknown_memory,
        }
    }

    fn requests(&self) -> Vec<WorkflowAgentRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl WorkflowAgent for FakeAgent {
    fn execute(&self, request: &WorkflowAgentRequest) -> Result<WorkflowAgentResponse, String> {
        self.requests.lock().unwrap().push(request.clone());

        let has_target_memory = request
            .memory_context
            .iter()
            .any(|memory| memory.id == TARGET_MEMORY_ID);
        if has_target_memory {
            fs::write(
                request.workspace_root.join("decision.md"),
                "No background writer should run without an explicit request.\n",
            )
            .map_err(|error| error.to_string())?;
        }

        Ok(WorkflowAgentResponse {
            summary: "completed the requested workspace task".to_string(),
            used_memory_ids: if self.cite_unknown_memory {
                vec!["memory-not-in-context".to_string()]
            } else if has_target_memory {
                vec![TARGET_MEMORY_ID.to_string()]
            } else {
                Vec::new()
            },
        })
    }
}

#[test]
fn paired_runner_keeps_controls_and_records_observed_lift() {
    let fixtures = tempdir().unwrap();
    let output = tempdir().unwrap();
    write_no_background_writer_fixture(fixtures.path());
    let agent = FakeAgent::new(false);

    let report = run_workflow_proof(fixtures.path(), output.path(), &agent).unwrap();

    assert_eq!(report.scenario_count, 1);
    assert_eq!(report.trial_count, 3);
    assert_eq!(report.tree_ring_wins_over_no_memory, 1);
    assert_eq!(report.tree_ring_wins_over_raw_memory, 0);
    assert!(report.tree_ring_complete);
    assert_eq!(report.agent_identity, "unspecified-agent");

    let scenario = &report.scenarios[0];
    let no_memory = trial_for(scenario, WorkflowArm::NoMemory);
    assert_eq!(no_memory.status, WorkflowProofTrialStatus::Fail);
    assert!(no_memory.file_checks.iter().any(|check| !check.passed));

    let raw_memory = trial_for(scenario, WorkflowArm::RawMemory);
    assert_eq!(raw_memory.status, WorkflowProofTrialStatus::Pass);
    let tree_ring = trial_for(scenario, WorkflowArm::TreeRing);
    assert_eq!(tree_ring.status, WorkflowProofTrialStatus::Pass);

    let requests = agent.requests();
    assert_eq!(requests.len(), 3);
    let no_memory_request = request_for(&requests, WorkflowArm::NoMemory);
    assert!(no_memory_request.memory_context.is_empty());
    for arm in [WorkflowArm::RawMemory, WorkflowArm::TreeRing] {
        let request = request_for(&requests, arm);
        assert_eq!(
            request
                .memory_context
                .iter()
                .map(|memory| memory.id.as_str())
                .collect::<Vec<_>>(),
            vec![TARGET_MEMORY_ID]
        );
        assert_eq!(
            request.memory_context[0].source_ref,
            "test://workflow-proof"
        );
    }

    for arm in ["no_memory", "raw_memory", "tree_ring"] {
        assert!(output
            .path()
            .join("trials/no-background-writer")
            .join(arm)
            .join("workspace")
            .is_dir());
    }
    let report_json = fs::read_to_string(output.path().join("workflow-proof-report.json")).unwrap();
    assert!(report_json.contains("\"agent_identity\": \"unspecified-agent\""));
    let summary = fs::read_to_string(output.path().join("workflow-proof-summary.md")).unwrap();
    assert!(summary.contains("- agent identity: unspecified-agent"));
}

#[test]
fn stale_cli_fixture_injects_current_contract_and_omits_superseded_contract() {
    let fixtures = tempdir().unwrap();
    let output = tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/workflow-proof/stale-cli-contract.json");
    fs::copy(&fixture, fixtures.path().join("stale-cli-contract.json"))
        .unwrap_or_else(|error| panic!("copy {}: {error}", fixture.display()));
    let agent = FakeAgent::new(false);

    let report = run_workflow_proof(fixtures.path(), output.path(), &agent).unwrap();

    let scenario = &report.scenarios[0];
    let raw_memory = trial_for(scenario, WorkflowArm::RawMemory);
    assert_eq!(
        memory_ids(&raw_memory.memory_context),
        ["mem_workflow_current_cli_contract"]
    );

    let tree_ring = trial_for(scenario, WorkflowArm::TreeRing);
    assert_eq!(
        memory_ids(&tree_ring.memory_context),
        ["mem_workflow_current_cli_contract"]
    );
    assert!(!memory_ids(&tree_ring.memory_context).contains(&"mem_workflow_stale_cli_contract"));
}

#[test]
fn unknown_cited_memory_is_recorded_as_a_trial_error() {
    let fixtures = tempdir().unwrap();
    let output = tempdir().unwrap();
    write_no_background_writer_fixture(fixtures.path());
    let agent = FakeAgent::new(true);

    let report = run_workflow_proof(fixtures.path(), output.path(), &agent).unwrap();

    let scenario = &report.scenarios[0];
    let tree_ring = trial_for(scenario, WorkflowArm::TreeRing);
    assert_eq!(tree_ring.status, WorkflowProofTrialStatus::Error);
    assert!(tree_ring
        .errors
        .iter()
        .any(|error| error.contains("memory-not-in-context")));
    assert_eq!(report.tree_ring_wins_over_no_memory, 0);
    assert!(!report.tree_ring_complete);
}

#[cfg(unix)]
#[test]
fn codex_adapter_uses_request_context_and_required_model() {
    use std::os::unix::fs::PermissionsExt;

    let workspace = tempdir().unwrap();
    let binary = workspace.path().join("fake-codex");
    write_fake_codex(&binary, TARGET_MEMORY_ID);
    let mut permissions = fs::metadata(&binary).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&binary, permissions).unwrap();

    let request = WorkflowAgentRequest::new(
        "adapter scenario".to_string(),
        WorkflowArm::TreeRing,
        "Create the requested decision file.".to_string(),
        workspace.path().to_path_buf(),
        vec![tree_ring_memory_core::WorkflowMemoryContext {
            id: TARGET_MEMORY_ID.to_string(),
            summary: "No background writer".to_string(),
            details: "Require an explicit request before starting one.".to_string(),
            ring: "cambium".to_string(),
            event_type: "decision".to_string(),
            source_ref: "test://workflow-proof".to_string(),
            confidence: 0.95,
        }],
    );

    let agent = CodexWorkflowAgent::new(binary.clone(), "test-model".to_string()).unwrap();
    assert_eq!(agent.evidence_identity(), "codex:test-model");
    let response = agent.execute(&request).unwrap();

    assert_eq!(response.used_memory_ids, vec![TARGET_MEMORY_ID]);
    let arguments = fake_codex_arguments(&binary);
    assert_eq!(
        arguments
            .iter()
            .take(10)
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec![
            "exec",
            "--ephemeral",
            "--sandbox",
            "workspace-write",
            "--cd",
            workspace.path().to_str().unwrap(),
            "--output-schema",
            workspace
                .path()
                .join(".tree-ring-workflow-schema.json")
                .to_str()
                .unwrap(),
            "--output-last-message",
            workspace
                .path()
                .join(".tree-ring-workflow-response.json")
                .to_str()
                .unwrap(),
        ]
    );
    assert_eq!(
        arguments[10..12]
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["--model", "test-model"]
    );
    assert_eq!(arguments.len(), 13);
    let prompt = &arguments[12];
    assert!(prompt.contains("work only in the workspace"));
    assert!(prompt.contains("use source/task files over memory when they conflict"));
    assert!(prompt.contains("do not seek validators or fixtures"));
    assert!(prompt.contains("Create the requested decision file."));
    assert!(prompt.contains("memory_context"));
    assert!(!prompt.contains("expected_files"));
    assert!(workspace
        .path()
        .join(".tree-ring-workflow-schema.json")
        .is_file());
}

#[test]
fn codex_adapter_rejects_blank_model_at_construction() {
    let error = CodexWorkflowAgent::new(PathBuf::from("codex"), " \t ".to_string())
        .err()
        .expect("blank model must be rejected");

    assert_eq!(error, "codex workflow model is required");
}

fn trial_for(
    scenario: &tree_ring_memory_cli::workflow_proof::WorkflowProofScenarioReport,
    arm: WorkflowArm,
) -> &tree_ring_memory_cli::workflow_proof::WorkflowProofTrialReport {
    scenario
        .trials
        .iter()
        .find(|trial| trial.arm == arm)
        .unwrap_or_else(|| panic!("missing {arm:?} trial"))
}

fn request_for(requests: &[WorkflowAgentRequest], arm: WorkflowArm) -> &WorkflowAgentRequest {
    requests
        .iter()
        .find(|request| request.arm == arm)
        .unwrap_or_else(|| panic!("missing {arm:?} request"))
}

fn memory_ids(memories: &[tree_ring_memory_core::WorkflowMemoryContext]) -> Vec<&str> {
    memories.iter().map(|memory| memory.id.as_str()).collect()
}

fn write_no_background_writer_fixture(fixture_dir: &Path) {
    fs::write(
        fixture_dir.join("no-background-writer.json"),
        r#"{
  "name": "no background writer",
  "task": "Create decision.md describing whether to start a background writer.",
  "seed_memories": [
    {
      "id": "mem_quality_no_background_writer",
      "created_at": "2026-07-12T00:00:00Z",
      "updated_at": "2026-07-12T00:00:00Z",
      "scope": "global",
      "ring": "cambium",
      "event_type": "decision",
      "summary": "No background writer without an explicit request.",
      "details": "The workflow owner must explicitly request a background writer.",
      "source": {
        "type": "test",
        "ref": "test://workflow-proof",
        "quote": "This quote must never reach the agent request."
      },
      "salience": 0.9,
      "confidence": 0.95,
      "sensitivity": "normal",
      "retention": "normal"
    },
    {
      "id": "mem_sensitive_hidden",
      "created_at": "2026-07-12T00:00:00Z",
      "updated_at": "2026-07-12T00:00:00Z",
      "scope": "global",
      "ring": "cambium",
      "event_type": "note",
      "summary": "Sensitive memory must not be exposed.",
      "details": "Hidden from normal-memory proof arms.",
      "source": {
        "type": "test",
        "ref": "test://sensitive"
      },
      "salience": 0.9,
      "confidence": 0.95,
      "sensitivity": "private",
      "retention": "normal"
    },
    {
      "id": "mem_superseded_hidden",
      "created_at": "2026-07-12T00:00:00Z",
      "updated_at": "2026-07-12T00:00:00Z",
      "scope": "global",
      "ring": "cambium",
      "event_type": "note",
      "summary": "Superseded memory must not be exposed.",
      "details": "Hidden from normal-memory proof arms.",
      "source": {
        "type": "test",
        "ref": "test://superseded"
      },
      "salience": 0.9,
      "confidence": 0.95,
      "sensitivity": "normal",
      "retention": "normal",
      "superseded_by": "mem_quality_no_background_writer"
    }
  ],
  "workspace_files": [
    {
      "path": "task.md",
      "content": "Decide whether a background writer should run."
    }
  ],
  "expected_files": [
    {
      "path": "decision.md",
      "contains": "No background writer"
    }
  ]
}"#,
    )
    .unwrap();
}

#[cfg(unix)]
fn write_fake_codex(binary: &Path, used_memory_id: &str) {
    let used_memory_ids = if used_memory_id.is_empty() {
        "[]".to_string()
    } else {
        format!("[\"{used_memory_id}\"]")
    };
    fs::write(
        binary,
        format!(
            "#!/bin/sh\n\
capture=\"$0.args\"\n\
output=\"\"\n\
for argument in \"$@\"; do\n\
  printf '%s\\n---ARG---\\n' \"$argument\" >> \"$capture\"\n\
done\n\
while [ \"$#\" -gt 0 ]; do\n\
  if [ \"$1\" = \"--output-last-message\" ]; then\n\
    shift\n\
    output=\"$1\"\n\
  fi\n\
  shift\n\
done\n\
printf '%s' '{{\"summary\":\"adapter response\",\"used_memory_ids\":{used_memory_ids}}}' > \"$output\"\n"
        ),
    )
    .unwrap();
}

#[cfg(unix)]
fn fake_codex_arguments(binary: &Path) -> Vec<String> {
    let capture = PathBuf::from(format!("{}.args", binary.display()));
    let arguments = fs::read_to_string(capture).unwrap();
    arguments
        .split("\n---ARG---\n")
        .filter(|argument| !argument.is_empty())
        .map(|argument| argument.trim_end_matches('\n').to_string())
        .collect()
}
