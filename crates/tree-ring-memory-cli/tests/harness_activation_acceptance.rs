#![cfg(unix)]

use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fs,
    io::Write,
    os::unix::fs::{symlink, MetadataExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};
use tempfile::tempdir;

const FIXTURE_TEXTS: [&str; 4] = [
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/harness-activation/codex.json"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/harness-activation/claude-code.json"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/harness-activation/pi.json"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/harness-activation/agent-zero.json"
    )),
];
const PROJECT_NAME: &str = "fixture-project";
const WORKFLOW_ID: &str = "fixture-flow";
const SEEDED_MEMORY: &str =
    "fixture project startup constraints require deterministic source safe evidence";
const RAW_TASK_HINT: &str = "fixture project startup constraints";
const CAPABILITY_SENTINEL: &str = "fixture-coordinator-capability-must-not-persist";

#[test]
fn shipped_fixtures_declare_only_project_local_versioned_activation_contracts() {
    let fixture_map = fixtures();
    assert_eq!(
        fixture_map.keys().map(String::as_str).collect::<Vec<_>>(),
        ["agent-zero", "claude-code", "codex", "pi"]
    );

    let expected = BTreeMap::from([
        (
            "agent-zero",
            (
                "needs-plugin",
                vec![".tree-ring/activation/agent-zero.json"],
            ),
        ),
        (
            "claude-code",
            (
                "configured-awaiting-proof",
                vec![
                    ".claude/skills/tree-ring-memory/SKILL.md",
                    ".claude/settings.json",
                ],
            ),
        ),
        (
            "codex",
            (
                "configured-awaiting-proof",
                vec![".agents/skills/tree-ring-memory/SKILL.md", "AGENTS.md"],
            ),
        ),
        (
            "pi",
            (
                "needs-trust",
                vec![
                    ".agents/skills/tree-ring-memory/SKILL.md",
                    ".pi/extensions/tree-ring-memory.ts",
                ],
            ),
        ),
    ]);

    for (id, fixture) in fixture_map {
        assert_eq!(fixture["schema_version"], 1, "{id}");
        assert_eq!(fixture["harness_id"], id, "{id}");
        assert_eq!(fixture["adapter_version"], "1", "{id}");
        assert_eq!(
            fixture["expected_activation_state_before_proof"],
            expected[id.as_str()].0,
            "{id}"
        );
        assert_eq!(
            strings(&fixture["expected_bridge_paths"]),
            expected[id.as_str()].1,
            "{id}"
        );
        assert_eq!(fixture["seeded_memory"]["summary"], SEEDED_MEMORY);

        let serialized = serde_json::to_string(&fixture).unwrap();
        for forbidden in ["/Users/", "${HOME}", "$HOME", "~/", "\\Users\\"] {
            assert!(
                !serialized.contains(forbidden),
                "{id} depends on {forbidden}"
            );
        }
        for path in strings(&fixture["marker_paths"])
            .into_iter()
            .chain(strings(&fixture["expected_bridge_paths"]))
        {
            assert!(
                !Path::new(path).is_absolute(),
                "{id} path is not local: {path}"
            );
            assert!(!path.split('/').any(|part| part == ".."), "{id}: {path}");
        }
        let live_env = fixture["live_executable_env"].as_str().unwrap();
        assert!(live_env.starts_with("TREE_RING_LIVE_"), "{id}");
        assert!(live_env.ends_with("_EXECUTABLE"), "{id}");
    }

    let isolated_fixtures = fixtures();
    let codex = &isolated_fixtures["codex"];
    assert_eq!(codex["isolated_root"]["selected_project"], "isolated-store");
    assert_eq!(codex["isolated_root"]["memory_root"], ".tree-ring");
    assert_eq!(
        codex["isolated_root"]["canonical_project_arg"],
        "--canonical-project-root"
    );
    assert_eq!(codex["isolated_root"]["expected_state"], "active-isolated");
    assert_eq!(codex["isolated_root"]["copy_sqlite"], false);
}

#[test]
fn default_relative_root_initializes_from_the_project_root() {
    let temp = tempdir().unwrap();
    let project = temp.path().join("relative-default-root");
    let empty_path = temp.path().join("empty-path");
    fs::create_dir_all(project.join(".codex")).unwrap();
    fs::create_dir_all(&empty_path).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tree-ring"))
        .current_dir(&project)
        .env("PATH", OsString::from(empty_path.as_os_str()))
        .env("HOME", project.join("fixture-home"))
        .env_remove("TREE_RING_AGENT_PROFILE")
        .env_remove("TREE_RING_WORKFLOW_ID")
        .env_remove("TREE_RING_SESSION_ID")
        .env_remove("TREE_RING_COORDINATOR_TOKEN")
        .arg("--json")
        .arg("init")
        .output()
        .unwrap();

    assert_success("default relative root init", &output);
    let report = output_json("default relative root init", &output);
    assert_eq!(report["ok"], true);
    assert_eq!(
        record_by_id(&report["integrations"], "codex")["state"],
        "configured-awaiting-proof"
    );
    assert!(project.join(".tree-ring/memory.sqlite").exists());
    assert!(project
        .join(".agents/skills/tree-ring-memory/SKILL.md")
        .exists());

    let preflight = Command::new(env!("CARGO_BIN_EXE_tree-ring"))
        .current_dir(&project)
        .env("PATH", OsString::from(empty_path.as_os_str()))
        .env("HOME", project.join("fixture-home"))
        .env_remove("TREE_RING_AGENT_PROFILE")
        .env_remove("TREE_RING_WORKFLOW_ID")
        .env_remove("TREE_RING_SESSION_ID")
        .env_remove("TREE_RING_COORDINATOR_TOKEN")
        .arg("--json")
        .args([
            "integrations",
            "preflight",
            "--harness",
            "codex",
            "--agent-profile",
            "smoke-worker",
            "--workflow-id",
            "smoke-flow",
            "--session-id",
            "smoke-session",
        ])
        .output()
        .unwrap();
    assert_success("default relative root preflight", &preflight);
    assert_eq!(
        output_json("default relative root preflight", &preflight)["state"],
        "active"
    );

    let status = Command::new(env!("CARGO_BIN_EXE_tree-ring"))
        .current_dir(&project)
        .env("PATH", OsString::from(empty_path.as_os_str()))
        .env("HOME", project.join("fixture-home"))
        .env_remove("TREE_RING_AGENT_PROFILE")
        .env_remove("TREE_RING_WORKFLOW_ID")
        .env_remove("TREE_RING_SESSION_ID")
        .env_remove("TREE_RING_COORDINATOR_TOKEN")
        .arg("--json")
        .args(["integrations", "status"])
        .output()
        .unwrap();
    assert_success("default relative root status", &status);
    let status_report = output_json("default relative root status", &status);
    assert_eq!(
        record_by_id(&status_report["integrations"], "codex")["state"],
        "active"
    );
}

#[test]
fn init_preflight_status_and_certify_prove_same_store_workers_without_private_receipt_data() {
    let temp = tempdir().unwrap();
    let project = temp.path().join(PROJECT_NAME);
    let empty_path = temp.path().join("empty-path");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&empty_path).unwrap();
    install_fixture_markers(&project);

    let root = project.join(".tree-ring");
    let init = tree_ring(&root, &project, &empty_path)
        .arg("init")
        .output()
        .unwrap();
    assert_success("canonical init", &init);
    let init_json = output_json("canonical init", &init);
    assert_eq!(init_json["ok"], true);
    assert_initial_states(&init_json["integrations"]);
    assert_manifest_contracts(&root);
    let before_proof_status = integration_status(&root, &project, &empty_path);
    assert_status_state(&before_proof_status, "agent-zero", "needs-plugin");
    let no_plugin_agent_zero = json!({
        "agent_profile": "agent-zero-worker",
        "workflow_id": WORKFLOW_ID,
        "session_id": "agent-zero-session"
    });
    let no_plugin_preflight = adapter_preflight_output(
        &root,
        &project,
        &empty_path,
        "agent-zero",
        "json",
        &no_plugin_agent_zero,
    );
    assert!(!no_plugin_preflight.status.success());
    assert!(String::from_utf8_lossy(&no_plugin_preflight.stderr)
        .contains("harness activation state is not eligible"));
    assert!(receipt_documents(&root).is_empty());

    seed_for_identity(
        &root,
        &project,
        &empty_path,
        "worker-a",
        WORKFLOW_ID,
        "codex-session-a",
        "seed-codex-a",
    );
    seed_for_identity(
        &root,
        &project,
        &empty_path,
        "worker-b",
        WORKFLOW_ID,
        "codex-session-b",
        "seed-codex-b",
    );
    seed_for_identity(
        &root,
        &project,
        &empty_path,
        "claude-code",
        "claude-session-b",
        "claude-session-b",
        "seed-claude-b",
    );
    seed_for_identity(
        &root,
        &project,
        &empty_path,
        "pi-worker-b",
        WORKFLOW_ID,
        "pi-session-b",
        "seed-pi-b",
    );

    let codex_a = codex_preflight(&root, &project, &empty_path, "worker-a", "codex-session-a");
    let codex_b = codex_preflight(&root, &project, &empty_path, "worker-b", "codex-session-b");
    assert_eq!(codex_a["state"], "active");
    assert_eq!(codex_b["state"], "active");
    assert!(codex_a["context"].as_str().unwrap().contains(SEEDED_MEMORY));
    assert!(codex_b["context"].as_str().unwrap().contains(SEEDED_MEMORY));
    assert_ne!(
        codex_a["receipt"]["receipt_id"],
        codex_b["receipt"]["receipt_id"]
    );
    assert_eq!(
        codex_a["receipt"]["store_id"],
        codex_b["receipt"]["store_id"]
    );

    let claude_input = json!({
        "session_id": "claude-session-b",
        "cwd": project,
        "agent_type": "claude-code",
        "transcript_path": "/ignored/by/adapter"
    });
    let claude = adapter_preflight(
        &root,
        &project,
        &empty_path,
        "claude-code",
        "claude-session-start",
        &claude_input,
    );
    assert!(claude["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .contains(SEEDED_MEMORY));

    let pi_input = json!({
        "agent_profile": "pi-worker-b",
        "workflow_id": WORKFLOW_ID,
        "session_id": "pi-session-b",
        "task_hint": RAW_TASK_HINT
    });
    let pi = adapter_preflight(
        &root,
        &project,
        &empty_path,
        "pi",
        "pi-before-agent-start",
        &pi_input,
    );
    assert_eq!(pi["state"], "active");
    assert!(pi["context"].as_str().unwrap().contains(SEEDED_MEMORY));

    let before_agent_zero = receipt_documents(&root);
    let agent_zero_input = json!({
        "agent_profile": "agent-zero-worker",
        "workflow_id": WORKFLOW_ID,
        "session_id": "agent-zero-session",
        "capability": CAPABILITY_SENTINEL
    });
    let rejected = adapter_preflight_output(
        &root,
        &project,
        &empty_path,
        "agent-zero",
        "json",
        &agent_zero_input,
    );
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("forbidden field"));
    assert_eq!(receipt_documents(&root), before_agent_zero);

    let receipts = receipt_documents(&root);
    assert_eq!(receipts.len(), 4);
    let persisted = receipts
        .values()
        .map(|document| serde_json::from_str::<Value>(document).unwrap())
        .collect::<Vec<_>>();
    let codex_receipts = persisted
        .iter()
        .filter(|receipt| receipt["harness_id"] == "codex")
        .collect::<Vec<_>>();
    assert_eq!(codex_receipts.len(), 2);
    assert_ne!(
        codex_receipts[0]["session"]["session_id"],
        codex_receipts[1]["session"]["session_id"]
    );
    assert_ne!(
        codex_receipts[0]["worker_key_fingerprint"],
        codex_receipts[1]["worker_key_fingerprint"]
    );
    let store_ids = persisted
        .iter()
        .map(|receipt| receipt["store_id"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        store_ids.len(),
        1,
        "all proven harnesses use one canonical store"
    );
    for document in receipts.values() {
        for forbidden in [
            SEEDED_MEMORY,
            RAW_TASK_HINT,
            CAPABILITY_SENTINEL,
            "transcript_path",
        ] {
            assert!(!document.contains(forbidden), "receipt leaked {forbidden}");
        }
        let receipt: Value = serde_json::from_str(document).unwrap();
        assert!(receipt.get("task_hint").is_none());
        assert!(receipt.get("capability").is_none());
        assert!(receipt.get("capabilities").is_none());
    }

    let status = integration_status(&root, &project, &empty_path);
    assert_status_state(&status, "codex", "active");
    assert_status_state(&status, "claude-code", "active");
    assert_status_state(&status, "pi", "active");
    assert_status_state(&status, "agent-zero", "needs-plugin");

    let evidence = temp.path().join("evidence");
    let certification = tree_ring(&root, &project, &empty_path)
        .arg("integrations")
        .arg("certify")
        .arg("--source-root")
        .arg(&project)
        .arg("--out-dir")
        .arg(&evidence)
        .output()
        .unwrap();
    assert_success("certify", &certification);
    let certification = output_json("certify", &certification);
    for id in ["codex", "claude-code", "pi"] {
        let record = record_by_id(&certification["report"]["records"], id);
        assert_eq!(record["status"], "pass", "{id}: {record}");
        assert_eq!(record["activation"]["state"], "active", "{id}");
        assert_eq!(record["activation"]["store_id_matches"], true, "{id}");
    }
    let agent_zero = record_by_id(&certification["report"]["records"], "agent-zero");
    assert_eq!(agent_zero["status"], "skip");
    assert_eq!(agent_zero["activation"]["state"], "needs-plugin");
}

#[test]
fn agent_zero_plugin_descriptor_bootstraps_passive_binding_then_receipt_backed_active_status() {
    let temp = tempdir().unwrap();
    let project = temp.path().join("agent-zero-project");
    let empty_path = temp.path().join("empty-path");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&empty_path).unwrap();
    let descriptor = install_agent_zero_plugin(temp.path());
    let root = project.join(".tree-ring");

    let init = tree_ring(&root, &project, &empty_path)
        .env("TREE_RING_AGENT_ZERO_PLUGIN_MANIFEST", &descriptor)
        .arg("init")
        .output()
        .unwrap();
    assert_success("Agent Zero descriptor init", &init);
    let init = output_json("Agent Zero descriptor init", &init);
    assert_eq!(
        record_by_id(&init["integrations"], "agent-zero")["state"],
        "configured-awaiting-proof"
    );

    let activation = &manifest(&root)["harnesses"]["agent-zero"];
    assert_eq!(activation["state"], "needs-plugin");
    assert_eq!(
        activation["bridge_path"],
        ".tree-ring/activation/agent-zero.json"
    );
    assert!(root.join("activation/agent-zero.json").is_file());

    // Re-running init is a no-replacement operation over the owned passive
    // binding, so an already initialized project remains usable.
    let repeat_init = tree_ring(&root, &project, &empty_path)
        .env("TREE_RING_AGENT_ZERO_PLUGIN_MANIFEST", &descriptor)
        .arg("init")
        .output()
        .unwrap();
    assert_success("repeat Agent Zero descriptor init", &repeat_init);
    assert_eq!(
        record_by_id(
            &output_json("repeat Agent Zero descriptor init", &repeat_init)["integrations"],
            "agent-zero"
        )["state"],
        "configured-awaiting-proof"
    );

    let configured =
        integration_status_with_agent_zero_plugin(&root, &project, &empty_path, &descriptor);
    assert_status_state(&configured, "agent-zero", "configured-awaiting-proof");

    let input = json!({
        "agent_profile": "agent-zero-worker",
        "workflow_id": "agent-zero-flow",
        "session_id": "agent-zero-session"
    });
    let preflight =
        agent_zero_preflight_with_plugin(&root, &project, &empty_path, &descriptor, &input);
    assert_eq!(preflight["state"], "active");

    let active =
        integration_status_with_agent_zero_plugin(&root, &project, &empty_path, &descriptor);
    assert_status_state(&active, "agent-zero", "active");

    // A receipt cannot keep Agent Zero active after the separately installed
    // plugin is disabled: status must return to the passive needs-plugin
    // state until a live compatible descriptor is available again.
    fs::write(
        &descriptor,
        r#"{"schema_version":1,"kind":"tree-ring-agent-zero-plugin-capability","plugin_id":"tree_ring_memory","plugin_version":"3.2.0","activation_protocol_version":1,"tree_ring_version":{"min":"0.15.3","minor":"0.15"},"enabled":false}"#,
    )
    .unwrap();
    let disabled =
        integration_status_with_agent_zero_plugin(&root, &project, &empty_path, &descriptor);
    assert_status_state(&disabled, "agent-zero", "needs-plugin");
}

#[test]
fn alternate_memory_root_is_active_isolated_without_copying_or_mutating_canonical_store() {
    let temp = tempdir().unwrap();
    let project = temp.path().join(PROJECT_NAME);
    let empty_path = temp.path().join("empty-path");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&empty_path).unwrap();
    install_fixture_markers(&project);

    let canonical = project.join(".tree-ring");
    let init = tree_ring(&canonical, &project, &empty_path)
        .arg("init")
        .output()
        .unwrap();
    assert_success("canonical init", &init);
    codex_preflight(
        &canonical,
        &project,
        &empty_path,
        "canonical-worker",
        "canonical-session",
    );
    let canonical_manifest_before = fs::read(canonical.join("activation.json")).unwrap();
    let canonical_sqlite_before = fs::read(canonical.join("memory.sqlite")).unwrap();
    let canonical_receipts_before = receipt_documents(&canonical);
    let canonical_store_id = manifest(&canonical)["store_id"]
        .as_str()
        .unwrap()
        .to_string();

    let isolated_project = temp.path().join("isolated-store");
    fs::create_dir_all(isolated_project.join(".codex")).unwrap();
    let isolated = isolated_project.join(".tree-ring");
    let init = tree_ring(&isolated, &isolated_project, &empty_path)
        .arg("init")
        .output()
        .unwrap();
    assert_success("isolated init", &init);
    let isolated_store_id = manifest(&isolated)["store_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(isolated_store_id, canonical_store_id);

    let canonical_meta = fs::metadata(canonical.join("memory.sqlite")).unwrap();
    let isolated_meta = fs::metadata(isolated.join("memory.sqlite")).unwrap();
    assert_ne!(
        (canonical_meta.dev(), canonical_meta.ino()),
        (isolated_meta.dev(), isolated_meta.ino()),
        "isolated init must create an independent store, not copy or link canonical SQLite"
    );

    let isolated_preflight = codex_isolated_preflight(
        &isolated,
        &isolated_project,
        &project,
        &empty_path,
        "isolated-worker",
        "isolated-session",
    );
    assert_eq!(isolated_preflight["state"], "active-isolated");
    let isolated_output = serde_json::to_string(&isolated_preflight).unwrap();
    for private_path in [&project, &isolated_project] {
        assert!(!isolated_output.contains(private_path.to_string_lossy().as_ref()));
    }

    assert_eq!(
        fs::read(canonical.join("activation.json")).unwrap(),
        canonical_manifest_before
    );
    assert_eq!(
        fs::read(canonical.join("memory.sqlite")).unwrap(),
        canonical_sqlite_before
    );
    assert_eq!(receipt_documents(&canonical), canonical_receipts_before);
    assert_eq!(receipt_documents(&isolated).len(), 1);
}

#[test]
fn live_harness_detection_requires_opt_in_and_explicit_executables() {
    if std::env::var("TREE_RING_LIVE_HARNESS_TESTS").as_deref() != Ok("1") {
        return;
    }

    let temp = tempdir().unwrap();
    let project = temp.path().join(PROJECT_NAME);
    let explicit_bin = temp.path().join("explicit-bin");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&explicit_bin).unwrap();
    install_fixture_markers(&project);

    let mut explicitly_available = BTreeSet::new();
    for fixture in fixtures().into_values() {
        let id = fixture["harness_id"].as_str().unwrap();
        let env_name = fixture["live_executable_env"].as_str().unwrap();
        let Ok(candidate) = std::env::var(env_name) else {
            continue;
        };
        let candidate = PathBuf::from(candidate);
        if !candidate.is_absolute() || !candidate.is_file() {
            continue;
        }
        let command_name = match id {
            "claude-code" => "claude",
            "agent-zero" => "agent-zero",
            other => other,
        };
        symlink(&candidate, explicit_bin.join(command_name)).unwrap();
        explicitly_available.insert(id.to_string());
    }

    let root = project.join(".tree-ring");
    let init = tree_ring(&root, &project, &explicit_bin)
        .arg("init")
        .output()
        .unwrap();
    assert_success("live-mode init", &init);
    let output = tree_ring(&root, &project, &explicit_bin)
        .arg("integrations")
        .arg("certify")
        .arg("--source-root")
        .arg(&project)
        .arg("--out-dir")
        .arg(temp.path().join("live-evidence"))
        .arg("--live")
        .output()
        .unwrap();
    assert_success("live-mode certification", &output);
    let report = output_json("live-mode certification", &output);
    for fixture in fixtures().into_values() {
        let id = fixture["harness_id"].as_str().unwrap();
        if explicitly_available.contains(id) {
            continue;
        }
        let record = record_by_id(&report["report"]["records"], id);
        assert_eq!(record["status"], "skip", "{id}: {record}");
        assert_eq!(
            record["activation"]["diagnostic"], "not installed locally",
            "{id}: {record}"
        );
    }
}

fn fixtures() -> BTreeMap<String, Value> {
    FIXTURE_TEXTS
        .iter()
        .map(|text| {
            let fixture: Value = serde_json::from_str(text).unwrap();
            (fixture["harness_id"].as_str().unwrap().to_string(), fixture)
        })
        .collect()
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry.as_str().unwrap())
        .collect()
}

fn install_fixture_markers(project: &Path) {
    for fixture in fixtures().into_values() {
        for marker in strings(&fixture["marker_paths"]) {
            fs::create_dir_all(project.join(marker)).unwrap();
        }
    }
}

fn assert_initial_states(integrations: &Value) {
    for fixture in fixtures().into_values() {
        let id = fixture["harness_id"].as_str().unwrap();
        let integration = record_by_id(integrations, id);
        assert_eq!(
            integration["state"], fixture["expected_activation_state_before_proof"],
            "{id}: {integration}"
        );
    }
}

fn assert_manifest_contracts(root: &Path) {
    let manifest = manifest(root);
    for fixture in fixtures().into_values() {
        let id = fixture["harness_id"].as_str().unwrap();
        let activation = manifest["harnesses"]
            .get(id)
            .unwrap_or_else(|| panic!("missing manifest contract for {id}"));
        assert_eq!(
            activation["adapter_version"], fixture["adapter_version"],
            "{id}"
        );
        assert_eq!(
            activation["state"], fixture["expected_activation_state_before_proof"],
            "{id}"
        );
        let actual_paths = activation["owned_files"]
            .as_array()
            .into_iter()
            .flatten()
            .chain(
                activation["managed_blocks"]
                    .as_array()
                    .into_iter()
                    .flatten(),
            )
            .map(|owned| owned["path"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        let expected_paths = strings(&fixture["expected_bridge_paths"])
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_paths, expected_paths, "{id}");
    }
}

fn manifest(root: &Path) -> Value {
    serde_json::from_slice(&fs::read(root.join("activation.json")).unwrap()).unwrap()
}

fn seed_for_identity(
    root: &Path,
    project: &Path,
    path: &Path,
    agent_profile: &str,
    workflow_id: &str,
    session_id: &str,
    operation_id: &str,
) {
    let output = tree_ring(root, project, path)
        .arg("remember")
        .arg(SEEDED_MEMORY)
        .arg("--event-type")
        .arg("lesson")
        .arg("--scope")
        .arg("project")
        .arg("--project")
        .arg(PROJECT_NAME)
        .arg("--agent-profile")
        .arg(agent_profile)
        .arg("--workflow-id")
        .arg(workflow_id)
        .arg("--session-id")
        .arg(session_id)
        .arg("--operation-id")
        .arg(operation_id)
        .arg("--source-ref")
        .arg(format!("fixture://harness-activation/{operation_id}"))
        .output()
        .unwrap();
    assert_success(operation_id, &output);
}

fn codex_preflight(root: &Path, project: &Path, path: &Path, worker: &str, session: &str) -> Value {
    let output = tree_ring(root, project, path)
        .env("TREE_RING_COORDINATOR_TOKEN", CAPABILITY_SENTINEL)
        .arg("integrations")
        .arg("preflight")
        .arg("--harness")
        .arg("codex")
        .arg("--agent-profile")
        .arg(worker)
        .arg("--workflow-id")
        .arg(WORKFLOW_ID)
        .arg("--session-id")
        .arg(session)
        .output()
        .unwrap();
    assert_success("Codex preflight", &output);
    output_json("Codex preflight", &output)
}

fn codex_isolated_preflight(
    root: &Path,
    selected_project: &Path,
    canonical_project: &Path,
    path: &Path,
    worker: &str,
    session: &str,
) -> Value {
    let output = tree_ring(root, selected_project, path)
        .arg("integrations")
        .arg("preflight")
        .arg("--harness")
        .arg("codex")
        .arg("--canonical-project-root")
        .arg(canonical_project)
        .arg("--agent-profile")
        .arg(worker)
        .arg("--workflow-id")
        .arg(WORKFLOW_ID)
        .arg("--session-id")
        .arg(session)
        .output()
        .unwrap();
    assert_success("isolated Codex preflight", &output);
    output_json("isolated Codex preflight", &output)
}

fn adapter_preflight(
    root: &Path,
    project: &Path,
    path: &Path,
    harness: &str,
    context_format: &str,
    input: &Value,
) -> Value {
    let output = adapter_preflight_output(root, project, path, harness, context_format, input);
    assert_success(&format!("{harness} preflight"), &output);
    output_json(&format!("{harness} preflight"), &output)
}

fn adapter_preflight_output(
    root: &Path,
    project: &Path,
    path: &Path,
    harness: &str,
    context_format: &str,
    input: &Value,
) -> Output {
    let mut command = tree_ring(root, project, path);
    command
        .arg("integrations")
        .arg("preflight")
        .arg("--harness")
        .arg(harness)
        .arg("--input-json-stdin")
        .arg("--context-format")
        .arg(context_format)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(input).unwrap().as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn integration_status(root: &Path, project: &Path, path: &Path) -> Value {
    let output = tree_ring(root, project, path)
        .arg("integrations")
        .arg("status")
        .arg("--source-root")
        .arg(project)
        .arg("--verbose")
        .output()
        .unwrap();
    assert_success("integration status", &output);
    output_json("integration status", &output)
}

fn integration_status_with_agent_zero_plugin(
    root: &Path,
    project: &Path,
    path: &Path,
    descriptor: &Path,
) -> Value {
    let output = tree_ring(root, project, path)
        .env("TREE_RING_AGENT_ZERO_PLUGIN_MANIFEST", descriptor)
        .arg("integrations")
        .arg("status")
        .arg("--source-root")
        .arg(project)
        .arg("--verbose")
        .output()
        .unwrap();
    assert_success("Agent Zero descriptor status", &output);
    output_json("Agent Zero descriptor status", &output)
}

fn agent_zero_preflight_with_plugin(
    root: &Path,
    project: &Path,
    path: &Path,
    descriptor: &Path,
    input: &Value,
) -> Value {
    let mut command = tree_ring(root, project, path);
    command
        .env("TREE_RING_AGENT_ZERO_PLUGIN_MANIFEST", descriptor)
        .arg("integrations")
        .arg("preflight")
        .arg("--harness")
        .arg("agent-zero")
        .arg("--input-json-stdin")
        .arg("--context-format")
        .arg("json")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(serde_json::to_string(input).unwrap().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_success("Agent Zero descriptor preflight", &output);
    output_json("Agent Zero descriptor preflight", &output)
}

fn install_agent_zero_plugin(base: &Path) -> PathBuf {
    let plugin = base.join("installed-agent-zero-plugin");
    fs::create_dir_all(&plugin).unwrap();
    fs::write(
        plugin.join("plugin.yaml"),
        "name: tree_ring_memory\nversion: 3.3.1\n",
    )
    .unwrap();
    let descriptor = plugin.join("activation-capability.json");
    fs::write(
        &descriptor,
        r#"{"schema_version":1,"kind":"tree-ring-agent-zero-plugin-capability","plugin_id":"tree_ring_memory","plugin_version":"3.3.1","activation_protocol_version":1,"tree_ring_version":{"min":"0.15.3","minor":"0.15"},"enabled":true}"#,
    )
    .unwrap();
    descriptor
}

fn assert_status_state(status: &Value, harness: &str, expected: &str) {
    let entry = record_by_id(&status["integrations"], harness);
    assert_eq!(entry["state"], expected, "{harness}: {entry}");
}

fn record_by_id<'a>(records: &'a Value, id: &str) -> &'a Value {
    records
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["harness_id"] == id || entry["id"] == id)
        .unwrap_or_else(|| panic!("missing record for {id}: {records}"))
}

fn receipt_documents(root: &Path) -> BTreeMap<PathBuf, String> {
    fn collect(directory: &Path, root: &Path, output: &mut BTreeMap<PathBuf, String>) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(&path, root, output);
            } else if path.extension() == Some(OsStr::new("json")) {
                output.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read_to_string(path).unwrap(),
                );
            }
        }
    }

    let mut documents = BTreeMap::new();
    collect(&root.join("activation/receipts"), root, &mut documents);
    documents
}

fn tree_ring(root: &Path, project: &Path, path: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tree-ring"));
    command
        .current_dir(project)
        .env("PATH", OsString::from(path.as_os_str()))
        .env("HOME", project.join("fixture-home"))
        .env_remove("TREE_RING_AGENT_PROFILE")
        .env_remove("TREE_RING_WORKFLOW_ID")
        .env_remove("TREE_RING_SESSION_ID")
        .env_remove("TREE_RING_COORDINATOR_TOKEN")
        .arg("--root")
        .arg(root)
        .arg("--json");
    command
}

fn output_json(context: &str, output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{context} did not emit JSON: {error}; stdout={}; stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_success(context: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{context} failed with {:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
