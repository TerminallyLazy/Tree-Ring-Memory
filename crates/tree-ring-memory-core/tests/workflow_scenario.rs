use std::fs;

use serde_json::{json, Value};
use tempfile::tempdir;
use tree_ring_memory_core::{
    evaluate_workspace, parse_workflow_scenario, WorkflowAgentRequest, WorkflowAgentResponse,
    WorkflowArm, WorkflowFileExpectation, WorkflowMemoryContext, WorkflowScenario,
};

const VALID_SCENARIO: &str = r#"{
  "name": "safe workflow",
  "task": "Prepare decision.md from the workspace.",
  "seed_memories": [],
  "workspace_files": [{"path": "proposal.md", "content": "draft"}],
  "expected_files": [{"path": "decision.md", "contains": "safe action"}]
}"#;

fn scenario_value() -> Value {
    serde_json::from_str(VALID_SCENARIO).unwrap()
}

fn scenario_with_seed_memory(seed_memory: Value) -> String {
    let mut scenario = scenario_value();
    scenario["seed_memories"] = json!([seed_memory]);
    serde_json::to_string(&scenario).unwrap()
}

fn valid_seed_memory() -> Value {
    json!({
        "id": "mem_seed",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z",
        "event_type": "decision",
        "summary": "Preserve the approved durable action.",
        "source": {
            "type": "manual",
            "ref": "docs/seed.md",
            "quote": "approved action"
        },
        "links": [{"type": "supports", "target": "decision.md"}],
        "review": {"needs_review": false}
    })
}

#[test]
fn parses_safe_workflow_fixture_and_keeps_validator_out_of_agent_request() {
    let scenario = parse_workflow_scenario(VALID_SCENARIO).unwrap();
    let request = WorkflowAgentRequest::new(
        scenario.name.clone(),
        WorkflowArm::NoMemory,
        scenario.task.clone(),
        "/tmp/trial".into(),
        vec![WorkflowMemoryContext {
            id: "mem_1".to_string(),
            summary: "Use the approved decision format.".to_string(),
            details: "Keep the durable action explicit.".to_string(),
            ring: "heartwood".to_string(),
            event_type: "decision".to_string(),
            source_ref: "notes/approval.md".to_string(),
            confidence: 0.9,
        }],
    );

    let serialized = serde_json::to_string(&request).unwrap();
    let request_fields = serde_json::to_value(&request).unwrap();

    assert_eq!(scenario.workspace_files[0].path, "proposal.md");
    assert_eq!(scenario.expected_files[0].contains, "safe action");
    assert!(!serialized.contains("safe action"));
    assert_eq!(
        request_fields
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec![
            "arm",
            "memory_context",
            "scenario_id",
            "schema_version",
            "task",
            "workspace_root",
        ]
    );
}

#[test]
fn accepts_nested_relative_paths_and_rejects_unsafe_paths() {
    let nested = VALID_SCENARIO
        .replace("proposal.md", "inputs/proposal.md")
        .replace("decision.md", "out/decision.md");
    assert!(parse_workflow_scenario(&nested).is_ok());

    for unsafe_path in ["../escape.txt", "/absolute.txt", ""] {
        let input = VALID_SCENARIO.replace("proposal.md", unsafe_path);
        assert!(
            parse_workflow_scenario(&input).is_err(),
            "{unsafe_path:?} must be rejected"
        );
    }
}

#[test]
fn rejects_unknown_fixture_fields() {
    let input = VALID_SCENARIO.replace("\n}", ",\n  \"unexpected\": \"must not be accepted\"\n}");

    assert!(parse_workflow_scenario(&input).is_err());
}

#[test]
fn rejects_invalid_scenario_contract_values() {
    let missing_expected_files = VALID_SCENARIO.replace(
        "\n  \"expected_files\": [{\"path\": \"decision.md\", \"contains\": \"safe action\"}]",
        "\n  \"expected_files\": []",
    );
    assert!(parse_workflow_scenario(&missing_expected_files).is_err());

    let duplicate_workspace_path = VALID_SCENARIO.replace(
        "[{\"path\": \"proposal.md\", \"content\": \"draft\"}]",
        "[{\"path\": \"proposal.md\", \"content\": \"draft\"}, {\"path\": \"proposal.md\", \"content\": \"revised\"}]",
    );
    assert!(parse_workflow_scenario(&duplicate_workspace_path).is_err());

    let duplicate_expectation = VALID_SCENARIO.replace(
        "[{\"path\": \"decision.md\", \"contains\": \"safe action\"}]",
        "[{\"path\": \"decision.md\", \"contains\": \"safe action\"}, {\"path\": \"decision.md\", \"contains\": \"safe action\"}]",
    );
    assert!(parse_workflow_scenario(&duplicate_expectation).is_err());

    let blank_expected_content = VALID_SCENARIO.replace("safe action", "   ");
    assert!(parse_workflow_scenario(&blank_expected_content).is_err());
}

#[test]
fn rejects_invalid_seed_memories() {
    let input = VALID_SCENARIO.replace(
        "\"seed_memories\": []",
        r#""seed_memories": [{
          "id": "mem_bad",
          "created_at": "2026-01-01T00:00:00Z",
          "updated_at": "2026-01-01T00:00:00Z",
          "event_type": "decision",
          "summary": ""
        }]"#,
    );

    assert!(parse_workflow_scenario(&input).is_err());
}

#[test]
fn rejects_unknown_seed_memory_fields_at_every_level() {
    let mut unknown_memory_field = valid_seed_memory();
    unknown_memory_field["unexpected"] = json!(true);
    assert!(parse_workflow_scenario(&scenario_with_seed_memory(unknown_memory_field)).is_err());

    let mut unknown_source_field = valid_seed_memory();
    unknown_source_field["source"]["unexpected"] = json!(true);
    assert!(parse_workflow_scenario(&scenario_with_seed_memory(unknown_source_field)).is_err());

    let mut unknown_link_field = valid_seed_memory();
    unknown_link_field["links"][0]["unexpected"] = json!(true);
    assert!(parse_workflow_scenario(&scenario_with_seed_memory(unknown_link_field)).is_err());

    let mut unknown_review_field = valid_seed_memory();
    unknown_review_field["review"]["unexpected"] = json!(true);
    assert!(parse_workflow_scenario(&scenario_with_seed_memory(unknown_review_field)).is_err());
}

#[test]
fn uses_the_documented_workflow_arm_serde_names() {
    assert_eq!(
        serde_json::to_string(&WorkflowArm::NoMemory).unwrap(),
        "\"no_memory\""
    );
    assert_eq!(
        serde_json::to_string(&WorkflowArm::RawMemory).unwrap(),
        "\"raw_memory\""
    );
    assert_eq!(
        serde_json::to_string(&WorkflowArm::TreeRing).unwrap(),
        "\"tree_ring\""
    );
}

#[test]
fn validates_agent_responses_without_validating_context_membership() {
    let valid = WorkflowAgentResponse {
        summary: "Created decision.md.".to_string(),
        used_memory_ids: vec!["mem_1".to_string(), "mem_2".to_string()],
    };
    assert!(valid.validate().is_ok());

    for response in [
        WorkflowAgentResponse {
            summary: " ".to_string(),
            used_memory_ids: Vec::new(),
        },
        WorkflowAgentResponse {
            summary: "Done".to_string(),
            used_memory_ids: vec![" ".to_string()],
        },
        WorkflowAgentResponse {
            summary: "Done".to_string(),
            used_memory_ids: vec!["mem_1".to_string(), "mem_1".to_string()],
        },
    ] {
        assert!(response.validate().is_err());
    }

    assert!(serde_json::from_str::<WorkflowAgentResponse>(
        r#"{"summary":"Done","unexpected":true}"#
    )
    .is_err());
}

#[test]
fn evaluates_expected_files_in_fixture_order() {
    let scenario = parse_workflow_scenario(
        r#"{
          "name": "workspace evaluation",
          "task": "Check the generated files.",
          "workspace_files": [],
          "expected_files": [
            {"path": "decision.md", "contains": "safe action"},
            {"path": "missing.md", "contains": "must exist"}
          ]
        }"#,
    )
    .unwrap();
    let workspace = tempdir().unwrap();
    fs::write(
        workspace.path().join("decision.md"),
        "Choose the safe action.",
    )
    .unwrap();

    let reports = evaluate_workspace(&scenario, workspace.path());

    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].path, "decision.md");
    assert!(reports[0].exists);
    assert!(reports[0].passed);
    assert_eq!(reports[1].path, "missing.md");
    assert!(!reports[1].exists);
    assert!(!reports[1].passed);
}

#[test]
fn workspace_evaluation_does_not_follow_unsafe_paths_from_manually_built_scenarios() {
    let root = tempdir().unwrap();
    let workspace = root.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    fs::write(root.path().join("escape.md"), "safe action").unwrap();
    let scenario = WorkflowScenario {
        name: "unsafe direct construction".to_string(),
        task: "Do not escape the workspace.".to_string(),
        seed_memories: Vec::new(),
        workspace_files: Vec::new(),
        expected_files: vec![WorkflowFileExpectation {
            path: "../escape.md".to_string(),
            contains: "safe action".to_string(),
        }],
    };

    let reports = evaluate_workspace(&scenario, &workspace);

    assert_eq!(reports.len(), 1);
    assert!(!reports[0].exists);
    assert!(!reports[0].passed);
}

#[test]
fn rejects_windows_root_and_prefix_paths() {
    for unsafe_path in [r"\\escape.txt", "C:escape.txt"] {
        let mut scenario = scenario_value();
        scenario["workspace_files"][0]["path"] = json!(unsafe_path);
        let input = serde_json::to_string(&scenario).unwrap();

        assert!(
            parse_workflow_scenario(&input).is_err(),
            "{unsafe_path:?} must be rejected"
        );
    }
}

#[test]
fn workspace_evaluation_rejects_windows_root_and_prefix_paths() {
    let workspace = tempdir().unwrap();

    for unsafe_path in [r"\\escape.txt", "C:escape.txt"] {
        fs::write(workspace.path().join(unsafe_path), "safe action").unwrap();
        let scenario = WorkflowScenario {
            name: "unsafe direct construction".to_string(),
            task: "Do not escape the workspace.".to_string(),
            seed_memories: Vec::new(),
            workspace_files: Vec::new(),
            expected_files: vec![WorkflowFileExpectation {
                path: unsafe_path.to_string(),
                contains: "safe action".to_string(),
            }],
        };

        let reports = evaluate_workspace(&scenario, workspace.path());

        assert_eq!(reports.len(), 1);
        assert!(!reports[0].exists, "{unsafe_path:?} must not be read");
        assert!(!reports[0].passed, "{unsafe_path:?} must not pass");
    }
}

#[test]
fn rejects_dot_path_components_that_create_lexical_aliases() {
    let mut scenario = scenario_value();
    scenario["workspace_files"] = json!([
        {"path": "out/./decision.md", "content": "draft"},
        {"path": "out/decision.md", "content": "revised"}
    ]);

    assert!(parse_workflow_scenario(&serde_json::to_string(&scenario).unwrap()).is_err());
}
