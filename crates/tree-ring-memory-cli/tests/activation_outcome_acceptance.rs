#![cfg(unix)]

use serde_json::{json, Value};
use std::{fs, path::PathBuf, process::Command};
use tempfile::{tempdir, TempDir};

struct Project {
    temp: TempDir,
    root: PathBuf,
}

impl Project {
    fn new() -> Self {
        let temp = tempdir().unwrap();
        let root = temp.path().join("Activation Outcome Project");
        fs::create_dir_all(root.join(".codex")).unwrap();
        fs::create_dir_all(root.join(".pi")).unwrap();
        let project = Self { temp, root };
        project.run(&["init"]);
        project
    }

    fn run(&self, args: &[&str]) -> Value {
        let output = Command::new(env!("CARGO_BIN_EXE_tree-ring"))
            .current_dir(&self.root)
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", self.temp.path().join("fixture-home"))
            .env_remove("TREE_RING_AGENT_PROFILE")
            .env_remove("TREE_RING_WORKFLOW_ID")
            .env_remove("TREE_RING_SESSION_ID")
            .env_remove("TREE_RING_COORDINATOR_TOKEN")
            .env_remove("TREE_RING_AGENT_ZERO_PLUGIN_MANIFEST")
            .arg("--json")
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn preflight(&self) {
        let result = self.run(&[
            "integrations",
            "preflight",
            "--harness",
            "codex",
            "--agent-profile",
            "outcome-worker",
            "--workflow-id",
            "outcome-flow",
            "--session-id",
            "outcome-session",
        ]);
        assert_eq!(result["state"], "active");
    }

    fn receipt(&self) -> PathBuf {
        let harness = self.root.join(".tree-ring/activation/receipts/codex");
        let worker = fs::read_dir(harness)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        fs::read_dir(worker)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
    }
}

#[test]
fn applied_activation_points_to_a_new_session_while_dry_run_remains_a_plan() {
    let project = Project::new();
    let manifest = fs::read(project.root.join(".tree-ring/activation.json")).unwrap();
    for command in ["activate", "link"] {
        let report = project.run(&["integrations", command, "--harness", "codex"]);
        assert_eq!(report["state"], "configured-awaiting-proof");
        assert_eq!(report["changed_paths"], json!([]));
        let step = report["next_step"].as_str().unwrap();
        assert!(step.contains("new Codex session"), "{step}");
        assert!(step.contains("integrations status --verbose"), "{step}");
        assert!(!step.contains("Apply the reviewed bridge plan"), "{step}");
    }
    let preview = project.run(&[
        "integrations",
        "activate",
        "--harness",
        "codex",
        "--dry-run",
    ]);
    assert_eq!(preview["dry_run"], true);
    assert!(preview["next_step"]
        .as_str()
        .unwrap()
        .contains("Apply the reviewed bridge plan"));
    assert_eq!(
        fs::read(project.root.join(".tree-ring/activation.json")).unwrap(),
        manifest
    );
    assert!(!project.root.join(".tree-ring/activation/receipts").exists());
}

#[test]
fn unchanged_activation_retains_real_preflight_proof_without_writing_a_receipt() {
    let project = Project::new();
    project.preflight();
    let receipt = project.receipt();
    let receipt_bytes = fs::read(&receipt).unwrap();
    let manifest = fs::read(project.root.join(".tree-ring/activation.json")).unwrap();
    for command in ["activate", "link"] {
        let report = project.run(&["integrations", command, "--harness", "codex"]);
        assert_eq!(report["state"], "active");
        assert_eq!(report["changed_paths"], json!([]));
        assert!(report["next_step"]
            .as_str()
            .unwrap()
            .contains("No action required"));
        assert_eq!(fs::read(&receipt).unwrap(), receipt_bytes);
        assert_eq!(
            fs::read(project.root.join(".tree-ring/activation.json")).unwrap(),
            manifest
        );
    }
}

#[test]
fn mismatched_receipt_does_not_turn_successful_configuration_into_active() {
    let project = Project::new();
    project.preflight();
    let receipt = project.receipt();
    let mut content: Value = serde_json::from_slice(&fs::read(&receipt).unwrap()).unwrap();
    content["store_id"] = json!("different-store");
    let mismatched = serde_json::to_vec(&content).unwrap();
    fs::write(&receipt, &mismatched).unwrap();
    let report = project.run(&["integrations", "activate", "--harness", "codex"]);
    assert_eq!(report["state"], "configured-awaiting-proof");
    assert!(report["next_step"]
        .as_str()
        .unwrap()
        .contains("new Codex session"));
    assert_eq!(fs::read(receipt).unwrap(), mismatched);
}

#[test]
fn prior_proof_cannot_hide_bridge_review_trust_or_create_only_deactivation() {
    let project = Project::new();
    project.preflight();
    let deactivation = project.run(&["integrations", "deactivate", "--harness", "codex"]);
    assert_eq!(deactivation["state"], "needs-user-review");
    let trust = project.run(&["integrations", "activate", "--harness", "pi"]);
    assert_eq!(trust["state"], "needs-trust");

    let hooks = project.root.join(".codex/hooks.json");
    let original = fs::read_to_string(&hooks).unwrap();
    let changed = format!("{original}\n");
    fs::write(&hooks, &changed).unwrap();
    let report = project.run(&["integrations", "activate", "--harness", "codex"]);
    assert_eq!(report["state"], "needs-user-review");
    assert_eq!(report["changed_paths"], json!([]));
    assert!(!report["next_step"]
        .as_str()
        .unwrap()
        .contains("No action required"));
    assert_eq!(fs::read_to_string(hooks).unwrap(), changed);
}
