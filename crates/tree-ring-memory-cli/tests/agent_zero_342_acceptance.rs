#![cfg(unix)]

use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};
use tempfile::{tempdir, TempDir};

struct Project {
    _temp: TempDir,
    root: PathBuf,
    descriptor: PathBuf,
}

impl Project {
    fn new() -> Self {
        let temp = tempdir().unwrap();
        let root = temp.path().join("Agent Zero Project");
        let plugin = temp.path().join("installed-plugin");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&plugin).unwrap();
        let project = Self {
            _temp: temp,
            root,
            descriptor: plugin.join("activation-capability.json"),
        };
        project.plugin("3.4.2", "3.4.2", "0.15.12", true);
        project
    }

    fn plugin(
        &self,
        manifest_version: &str,
        descriptor_version: &str,
        minimum: &str,
        enabled: bool,
    ) {
        fs::write(
            self.descriptor.with_file_name("plugin.yaml"),
            format!("name: tree_ring_memory\nversion: {manifest_version}\n"),
        )
        .unwrap();
        fs::write(
            &self.descriptor,
            serde_json::to_vec(&json!({
                "schema_version": 1,
                "kind": "tree-ring-agent-zero-plugin-capability",
                "plugin_id": "tree_ring_memory",
                "plugin_version": descriptor_version,
                "activation_protocol_version": 1,
                "tree_ring_version": {"min": minimum, "minor": "0.15"},
                "enabled": enabled
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tree-ring"));
        command
            .current_dir(&self.root)
            .env("PATH", "")
            .env("HOME", self.root.join("fixture-home"))
            .env_remove("TREE_RING_AGENT_PROFILE")
            .env_remove("TREE_RING_WORKFLOW_ID")
            .env_remove("TREE_RING_SESSION_ID")
            .env_remove("TREE_RING_OPERATION_ID")
            .env_remove("TREE_RING_COORDINATOR_TOKEN")
            .env("TREE_RING_AGENT_ZERO_PLUGIN_MANIFEST", &self.descriptor)
            .args(["--json", "--root", ".tree-ring"])
            .args(args);
        command
    }

    fn run(&self, args: &[&str]) -> Value {
        successful_json(&self.command(args).output().unwrap())
    }

    fn preflight(&self, session: &str) -> Output {
        let mut child = self
            .command(&[
                "integrations",
                "preflight",
                "--harness",
                "agent-zero",
                "--input-json-stdin",
                "--context-format",
                "json",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(
                &serde_json::to_vec(&json!({
                    "agent_profile": "agent-zero-342-worker",
                    "workflow_id": "agent-zero-342-flow",
                    "session_id": session
                }))
                .unwrap(),
            )
            .unwrap();
        child.wait_with_output().unwrap()
    }

    fn receipts(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn collect(path: &Path, receipts: &mut BTreeMap<PathBuf, Vec<u8>>) {
            let Ok(entries) = fs::read_dir(path) else {
                return;
            };
            for entry in entries {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    collect(&path, receipts);
                } else {
                    receipts.insert(path.clone(), fs::read(path).unwrap());
                }
            }
        }
        let mut receipts = BTreeMap::new();
        collect(
            &self.root.join(".tree-ring/activation/receipts"),
            &mut receipts,
        );
        receipts
    }
}

fn successful_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "stdout={}; stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn agent_zero_state(report: &Value) -> &str {
    report["integrations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["id"] == "agent-zero" || record["harness_id"] == "agent-zero")
        .unwrap()["state"]
        .as_str()
        .unwrap()
}

#[test]
fn agent_zero_342_with_01512_contract_activates_only_after_real_preflight() {
    let project = Project::new();
    assert_eq!(
        agent_zero_state(&project.run(&["init"])),
        "configured-awaiting-proof"
    );
    let activation = project.run(&["integrations", "activate", "--harness", "agent-zero"]);
    assert_eq!(activation["state"], "needs-plugin");
    assert!(project.receipts().is_empty());
    assert_eq!(
        agent_zero_state(&project.run(&["integrations", "status", "--verbose"])),
        "configured-awaiting-proof"
    );
    let manifest_path = project.root.join(".tree-ring/activation.json");
    let manifest_before = fs::read(&manifest_path).unwrap();
    let manifest: Value = serde_json::from_slice(&manifest_before).unwrap();
    assert_eq!(manifest["harnesses"]["agent-zero"]["state"], "needs-plugin");

    let preflight = successful_json(&project.preflight("valid-session"));
    assert_eq!(preflight["state"], "active");
    assert_eq!(preflight["receipt"]["harness_id"], "agent-zero");
    assert!(preflight["receipt"].get("session").is_none());
    let receipts = project.receipts();
    assert_eq!(receipts.len(), 1);
    let persisted: Value = serde_json::from_slice(receipts.values().next().unwrap()).unwrap();
    assert_eq!(
        persisted["session"]["agent_profile"],
        "agent-zero-342-worker"
    );
    assert_eq!(persisted["session"]["workflow_id"], "agent-zero-342-flow");
    assert_eq!(persisted["session"]["session_id"], "valid-session");
    assert_eq!(persisted["receipt_id"], preflight["receipt"]["receipt_id"]);
    assert_eq!(persisted["store_id"], preflight["receipt"]["store_id"]);
    assert_eq!(persisted["state"], "active");
    assert_eq!(fs::read(manifest_path).unwrap(), manifest_before);
    assert_eq!(
        agent_zero_state(&project.run(&["integrations", "status", "--verbose"])),
        "active"
    );
}

#[test]
fn mismatched_or_disabled_342_plugin_cannot_reuse_or_create_activation_proof() {
    let project = Project::new();
    project.run(&["init"]);
    successful_json(&project.preflight("previous-valid-session"));
    let receipts = project.receipts();
    assert_eq!(receipts.len(), 1);

    for (name, manifest_version, descriptor_version, minimum, enabled) in [
        ("sibling-version", "3.4.1", "3.4.2", "0.15.12", true),
        ("unsupported-version", "3.4.3", "3.4.3", "0.15.12", true),
        ("older-minimum", "3.4.2", "3.4.2", "0.15.11", true),
        ("different-minimum", "3.4.2", "3.4.2", "0.15.13", true),
        ("disabled", "3.4.2", "3.4.2", "0.15.12", false),
    ] {
        project.plugin(manifest_version, descriptor_version, minimum, enabled);
        assert_eq!(
            agent_zero_state(&project.run(&["init"])),
            "needs-plugin",
            "{name}"
        );
        assert_eq!(
            project.run(&["integrations", "activate", "--harness", "agent-zero"])["state"],
            "needs-plugin",
            "{name}"
        );
        assert_eq!(
            agent_zero_state(&project.run(&["integrations", "status", "--verbose"])),
            "needs-plugin",
            "{name}"
        );
        let output = project.preflight(name);
        assert!(!output.status.success(), "{name}");
        assert!(output.stdout.is_empty(), "{name}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("not eligible for preflight"),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(project.receipts(), receipts, "{name}");
    }
}
