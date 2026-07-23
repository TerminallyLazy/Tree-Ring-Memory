use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[cfg(unix)]
use std::path::PathBuf;

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

fn json_field_scenario(json_fields: Value) -> WorkflowScenario {
    parse_workflow_scenario(
        &serde_json::to_string(&json!({
            "name": "structured workspace evaluation",
            "task": "Check the generated JSON file.",
            "workspace_files": [],
            "expected_files": [{
                "path": "decision.json",
                "json_fields": json_fields
            }]
        }))
        .unwrap(),
    )
    .unwrap()
}

#[cfg(unix)]
fn physical_path(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("test path must resolve to its physical location")
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
    assert_eq!(
        scenario.expected_files[0].contains.as_deref(),
        Some("safe action")
    );
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
fn keeps_json_field_validators_out_of_agent_request() {
    let scenario = json_field_scenario(json!({
        "/decision/status": "must-not-leak",
        "/metadata/requires_review": false
    }));
    let request = WorkflowAgentRequest::new(
        scenario.name.clone(),
        WorkflowArm::TreeRing,
        scenario.task.clone(),
        "/tmp/trial".into(),
        Vec::new(),
    );
    let serialized = serde_json::to_string(&request).unwrap();

    assert!(scenario.expected_files[0].json_fields.is_some());
    assert!(!serialized.contains("/decision/status"));
    assert!(!serialized.contains("must-not-leak"));
    assert!(!serialized.contains("expected_files"));
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
fn permits_multiple_distinct_checks_for_the_same_expected_file() {
    let input = VALID_SCENARIO.replace(
        "[{\"path\": \"decision.md\", \"contains\": \"safe action\"}]",
        "[\
          {\"path\": \"decision.md\", \"contains\": \"safe action\"},\
          {\"path\": \"decision.md\", \"contains\": \"durable rationale\"}\
        ]",
    );

    assert!(parse_workflow_scenario(&input).is_ok());
}

#[test]
fn rejects_mixed_or_missing_file_check_modes() {
    let mut mixed = scenario_value();
    mixed["expected_files"] = json!([{
        "path": "decision.json",
        "contains": "approved",
        "json_fields": {"/status": "approved"}
    }]);
    let mixed_error = parse_workflow_scenario(&serde_json::to_string(&mixed).unwrap())
        .unwrap_err()
        .to_string();
    assert!(mixed_error.contains("exactly one check mode"));

    let mut missing = scenario_value();
    missing["expected_files"] = json!([{"path": "decision.json"}]);
    let missing_error = parse_workflow_scenario(&serde_json::to_string(&missing).unwrap())
        .unwrap_err()
        .to_string();
    assert!(missing_error.contains("exactly one check mode"));
}

#[test]
fn rejects_empty_json_field_check_configurations() {
    let mut scenario = scenario_value();
    scenario["expected_files"] = json!([{
        "path": "decision.json",
        "json_fields": {}
    }]);

    let error = parse_workflow_scenario(&serde_json::to_string(&scenario).unwrap())
        .unwrap_err()
        .to_string();

    assert!(error.contains("json_fields requires at least one JSON pointer"));
}

#[test]
fn rejects_malformed_json_pointer_configurations() {
    for pointer in ["decision/status", "/decision/~2status", "/decision/~"] {
        let mut json_fields = serde_json::Map::new();
        json_fields.insert(pointer.to_string(), json!("approved"));
        let mut scenario = scenario_value();
        scenario["expected_files"] = json!([{
            "path": "decision.json",
            "json_fields": json_fields
        }]);

        assert!(
            parse_workflow_scenario(&serde_json::to_string(&scenario).unwrap()).is_err(),
            "{pointer:?} must be rejected"
        );
    }
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
fn workflow_seed_conversion_preserves_multi_agent_context() {
    let mut seed = valid_seed_memory();
    seed["agent_profile"] = json!("reviewer");
    seed["workflow_id"] = json!("workflow-42");
    seed["session_id"] = json!("session-7");
    seed["operation_id"] = json!("operation-3");
    seed["scope"] = json!("workflow");

    let scenario = parse_workflow_scenario(&scenario_with_seed_memory(seed)).unwrap();
    let memory = &scenario.seed_memories[0];

    assert_eq!(memory.agent_profile.as_deref(), Some("reviewer"));
    assert_eq!(memory.workflow_id.as_deref(), Some("workflow-42"));
    assert_eq!(memory.session_id.as_deref(), Some("session-7"));
    assert_eq!(memory.operation_id.as_deref(), Some("operation-3"));
    memory.validate().unwrap();
}

#[test]
fn rejects_blank_and_cross_sensitivity_duplicate_seed_memory_ids() {
    let mut blank_id = valid_seed_memory();
    blank_id["id"] = json!("   ");
    let blank_error = parse_workflow_scenario(&scenario_with_seed_memory(blank_id))
        .unwrap_err()
        .to_string();
    assert!(blank_error.contains("seed_memories[0].id"));

    let mut normal = valid_seed_memory();
    normal["id"] = json!("mem_shared");
    normal["sensitivity"] = json!("normal");
    let mut private = valid_seed_memory();
    private["id"] = json!("mem_shared");
    private["sensitivity"] = json!("private");
    let mut scenario = scenario_value();
    scenario["seed_memories"] = json!([normal, private]);

    let duplicate_error = parse_workflow_scenario(&serde_json::to_string(&scenario).unwrap())
        .unwrap_err()
        .to_string();
    assert!(duplicate_error.contains("duplicates memory id mem_shared"));
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

#[cfg(unix)]
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

    let reports = evaluate_workspace(&scenario, &physical_path(workspace.path()));

    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].path, "decision.md");
    assert!(reports[0].exists);
    assert!(reports[0].passed);
    assert_eq!(reports[1].path, "missing.md");
    assert!(!reports[1].exists);
    assert!(!reports[1].passed);
}

#[cfg(unix)]
#[test]
fn retains_legacy_contains_file_checks() {
    let scenario = parse_workflow_scenario(VALID_SCENARIO).unwrap();
    let workspace = tempdir().unwrap();
    fs::write(
        workspace.path().join("decision.md"),
        "Choose the safe action with durable rationale.",
    )
    .unwrap();

    let reports = evaluate_workspace(&scenario, &physical_path(workspace.path()));

    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].contains.as_deref(), Some("safe action"));
    assert!(reports[0].json_fields.is_none());
    assert!(reports[0].passed);
}

#[cfg(unix)]
#[test]
fn evaluates_json_field_expectations_with_exact_json_pointer_values() {
    let scenario = parse_workflow_scenario(
        r#"{
          "name": "structured workspace evaluation",
          "task": "Check the generated JSON file.",
          "workspace_files": [],
          "expected_files": [{
            "path": "decision.json",
            "json_fields": {
              "/decision/status": "approved",
              "/decision/retry_count": 0,
              "/metadata/requires_review": false
            }
          }]
        }"#,
    )
    .unwrap();
    let workspace = tempdir().unwrap();
    fs::write(
        workspace.path().join("decision.json"),
        r#"{
          "decision": {"status": "approved", "retry_count": 0},
          "metadata": {"requires_review": false}
        }"#,
    )
    .unwrap();

    let reports = evaluate_workspace(&scenario, &physical_path(workspace.path()));

    assert_eq!(reports.len(), 1);
    assert!(reports[0].exists);
    assert!(reports[0].passed);
    assert!(reports[0].contains.is_none());
    assert_eq!(
        reports[0].json_fields,
        Some(
            [
                ("/decision/retry_count".to_string(), json!(0)),
                ("/decision/status".to_string(), json!("approved")),
                ("/metadata/requires_review".to_string(), json!(false)),
            ]
            .into_iter()
            .collect::<BTreeMap<_, _>>(),
        )
    );
    let serialized_report = serde_json::to_string(&reports[0]).unwrap();
    assert!(
        serialized_report.find("/decision/retry_count").unwrap()
            < serialized_report.find("/decision/status").unwrap()
    );
    assert!(serialized_report.contains("\"json_fields\""));
}

#[cfg(unix)]
#[test]
fn marks_json_field_check_failed_when_a_pointer_is_missing() {
    let scenario = json_field_scenario(json!({
        "/decision/status": "approved",
        "/decision/owner": "release-team"
    }));
    let workspace = tempdir().unwrap();
    fs::write(
        workspace.path().join("decision.json"),
        r#"{"decision": {"status": "approved"}}"#,
    )
    .unwrap();

    let reports = evaluate_workspace(&scenario, &physical_path(workspace.path()));

    assert!(reports[0].exists);
    assert!(!reports[0].passed);
}

#[cfg(unix)]
#[test]
fn marks_json_field_check_failed_when_a_value_is_wrong() {
    let scenario = json_field_scenario(json!({"/decision/status": "approved"}));
    let workspace = tempdir().unwrap();
    fs::write(
        workspace.path().join("decision.json"),
        r#"{"decision": {"status": "rejected"}}"#,
    )
    .unwrap();

    let reports = evaluate_workspace(&scenario, &physical_path(workspace.path()));

    assert!(reports[0].exists);
    assert!(!reports[0].passed);
}

#[cfg(unix)]
#[test]
fn marks_json_field_check_failed_when_file_is_not_json() {
    let scenario = json_field_scenario(json!({"/decision/status": "approved"}));
    let workspace = tempdir().unwrap();
    fs::write(workspace.path().join("decision.json"), "not JSON").unwrap();

    let reports = evaluate_workspace(&scenario, &physical_path(workspace.path()));

    assert!(reports[0].exists);
    assert!(!reports[0].passed);
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
            contains: Some("safe action".to_string()),
            json_fields: None,
        }],
    };

    let reports = evaluate_workspace(&scenario, &workspace);

    assert_eq!(reports.len(), 1);
    assert!(!reports[0].exists);
    assert!(!reports[0].passed);
}

#[test]
fn rejects_root_prefix_and_backslash_separator_paths() {
    for unsafe_path in [
        "/escape.txt",
        r"\\escape.txt",
        "C:escape.txt",
        r"out\decision.md",
    ] {
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
                contains: Some("safe action".to_string()),
                json_fields: None,
            }],
        };

        let reports = evaluate_workspace(&scenario, workspace.path());

        assert_eq!(reports.len(), 1);
        assert!(!reports[0].exists, "{unsafe_path:?} must not be read");
        assert!(!reports[0].passed, "{unsafe_path:?} must not pass");
    }
}

#[test]
fn workflow_proof_fixtures_parse_and_cover_the_required_scenarios() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/workflow-proof");
    let entries = fs::read_dir(&fixture_dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", fixture_dir.display()));
    let mut scenario_names = BTreeSet::new();

    for entry in entries {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let input = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            let scenario = parse_workflow_scenario(&input)
                .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
            assert!(
                !scenario.expected_files.is_empty(),
                "{} must define an expected file",
                path.display()
            );
            scenario_names.insert(scenario.name);
        }
    }

    for required_name in [
        "no-background-writer",
        "stale-cli-contract",
        "scar-recovery",
    ] {
        assert!(
            scenario_names.contains(required_name),
            "missing workflow proof scenario {required_name}"
        );
    }
}

#[test]
fn workspace_evaluation_rejects_portable_root_and_backslash_separator_paths() {
    let workspace = tempdir().unwrap();
    fs::create_dir_all(workspace.path().join("out")).unwrap();
    fs::write(workspace.path().join(r"out\decision.md"), "safe action").unwrap();
    let scenario = WorkflowScenario {
        name: "unsafe direct construction".to_string(),
        task: "Do not escape the workspace.".to_string(),
        seed_memories: Vec::new(),
        workspace_files: Vec::new(),
        expected_files: vec![
            WorkflowFileExpectation {
                path: "/escape.txt".to_string(),
                contains: Some("safe action".to_string()),
                json_fields: None,
            },
            WorkflowFileExpectation {
                path: r"out\decision.md".to_string(),
                contains: Some("safe action".to_string()),
                json_fields: None,
            },
        ],
    };

    let reports = evaluate_workspace(&scenario, workspace.path());

    assert_eq!(reports.len(), 2);
    assert!(reports.iter().all(|report| !report.exists));
    assert!(reports.iter().all(|report| !report.passed));
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

#[test]
fn rejects_empty_path_components_that_create_lexical_aliases() {
    let mut scenario = scenario_value();
    scenario["workspace_files"] = json!([
        {"path": "out//decision.md", "content": "draft"},
        {"path": "out/decision.md", "content": "revised"}
    ]);

    assert!(parse_workflow_scenario(&serde_json::to_string(&scenario).unwrap()).is_err());
}
