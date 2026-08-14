#![cfg(unix)]

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};
use tree_ring_memory_cli::activation::{
    manifest::{bridge_fingerprint, save_manifest},
    ActivationManifest, ActivationState, AdapterCapability, HarnessActivation, OwnedBridgeFile,
    ACTIVATION_PROTOCOL_VERSION, ACTIVATION_SCHEMA_VERSION,
};
use tree_ring_memory_sqlite::SQLiteMemoryStore;

#[test]
fn claude_launch_cli_forwards_only_the_private_context_path_and_child_arguments() {
    let temp = tempfile::tempdir().unwrap();
    let project_root = temp.path().join("project");
    let memory_root = project_root.join(".tree-ring");
    fs::create_dir_all(project_root.join(".claude/skills/tree-ring-memory")).unwrap();
    fs::create_dir_all(&memory_root).unwrap();
    fs::write(
        project_root.join(".claude/skills/tree-ring-memory/SKILL.md"),
        "Tree Ring bridge fixture\n",
    )
    .unwrap();
    drop(SQLiteMemoryStore::open(memory_root.join("memory.sqlite")).unwrap());

    let mut activation = HarnessActivation {
        state: ActivationState::ConfiguredAwaitingProof,
        adapter_capability: AdapterCapability::WrapperPreflight,
        adapter_version: "1".to_string(),
        bridge_fingerprint: String::new(),
        bridge_path: Some(".claude/skills/tree-ring-memory/SKILL.md".to_string()),
        owned_files: vec![OwnedBridgeFile {
            path: ".claude/skills/tree-ring-memory/SKILL.md".to_string(),
            sha256: "b".repeat(64),
        }],
        managed_blocks: Vec::new(),
    };
    activation.bridge_fingerprint = bridge_fingerprint("claude-code", &activation);
    let manifest = ActivationManifest {
        schema_version: ACTIVATION_SCHEMA_VERSION,
        protocol_version: ACTIVATION_PROTOCOL_VERSION,
        store_id: "store-cli-launch-test".to_string(),
        project_root_fingerprint: fingerprint_path(&project_root),
        cli_version: env!("CARGO_PKG_VERSION").to_string(),
        harnesses: BTreeMap::from([("claude-code".to_string(), activation)]),
    };
    save_manifest(&memory_root, &manifest).unwrap();

    let fake_bin = temp.path().join("bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let fake_claude = fake_bin.join("claude");
    fs::write(
        &fake_claude,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$TREE_RING_LAUNCH_ARGS\"\nif [ ! -f \"$2\" ]; then exit 91; fi\nhead -n 1 \"$2\" > \"$TREE_RING_LAUNCH_CONTEXT\"\nexit 7\n",
    )
    .unwrap();
    fs::set_permissions(&fake_claude, fs::Permissions::from_mode(0o755)).unwrap();
    let captured_args = temp.path().join("args.txt");
    let captured_context = temp.path().join("context.txt");
    let path = prepend_path(&fake_bin);

    let output = Command::new(env!("CARGO_BIN_EXE_tree-ring"))
        .env("PATH", path)
        .env("TREE_RING_LAUNCH_ARGS", &captured_args)
        .env("TREE_RING_LAUNCH_CONTEXT", &captured_context)
        .arg("--root")
        .arg(&memory_root)
        .arg("integrations")
        .arg("launch")
        .arg("--harness")
        .arg("claude-code")
        .arg("--")
        .arg("--model")
        .arg("sonnet")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(7));
    let args = fs::read_to_string(captured_args).unwrap();
    let lines = args.lines().collect::<Vec<_>>();
    assert_eq!(lines[0], "--append-system-prompt-file");
    assert!(lines[1].ends_with(".md"));
    assert_eq!(&lines[2..], ["--", "--model", "sonnet"]);
    assert_eq!(
        fs::read_to_string(captured_context).unwrap().trim(),
        "Tree Ring Memory scoped preflight recall:"
    );
    assert!(directory_files(&memory_root.join("activation/runtime")).is_empty());
    assert_eq!(
        json_files_below(&memory_root.join("activation/receipts")),
        1
    );
}

#[test]
fn direct_cli_reports_active_isolated_without_touching_the_canonical_store() {
    let temp = tempfile::tempdir().unwrap();
    let canonical_project = temp.path().join("canonical-project");
    let isolated_project = temp.path().join("isolated-store");
    let empty_path = temp.path().join("empty-path");
    let home = temp.path().join("home");
    for project in [&canonical_project, &isolated_project] {
        fs::create_dir_all(project.join(".codex")).unwrap();
    }
    fs::create_dir_all(&empty_path).unwrap();
    fs::create_dir_all(&home).unwrap();
    let canonical_root = canonical_project.join(".tree-ring");
    let isolated_root = isolated_project.join(".tree-ring");

    for (name, project, root) in [
        ("canonical", &canonical_project, &canonical_root),
        ("isolated", &isolated_project, &isolated_root),
    ] {
        let output = tree_ring(root, project, &empty_path, &home)
            .arg("init")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{name} init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let canonical_preflight = tree_ring(&canonical_root, &canonical_project, &empty_path, &home)
        .args([
            "integrations",
            "preflight",
            "--harness",
            "codex",
            "--agent-profile",
            "canonical-worker",
            "--workflow-id",
            "workflow",
            "--session-id",
            "canonical-session",
        ])
        .output()
        .unwrap();
    assert!(canonical_preflight.status.success());

    let canonical_manifest_before = fs::read(canonical_root.join("activation.json")).unwrap();
    let canonical_sqlite_before = fs::read(canonical_root.join("memory.sqlite")).unwrap();
    let canonical_receipts_before = json_files_below(&canonical_root.join("activation/receipts"));
    let canonical_meta = fs::metadata(canonical_root.join("memory.sqlite")).unwrap();
    let isolated_meta = fs::metadata(isolated_root.join("memory.sqlite")).unwrap();
    assert_ne!(
        (canonical_meta.dev(), canonical_meta.ino()),
        (isolated_meta.dev(), isolated_meta.ino())
    );

    let isolated_preflight = tree_ring(&isolated_root, &isolated_project, &empty_path, &home)
        .arg("integrations")
        .arg("preflight")
        .arg("--harness")
        .arg("codex")
        .arg("--canonical-project-root")
        .arg(&canonical_project)
        .arg("--agent-profile")
        .arg("isolated-worker")
        .arg("--workflow-id")
        .arg("workflow")
        .arg("--session-id")
        .arg("isolated-session")
        .output()
        .unwrap();
    assert!(
        isolated_preflight.status.success(),
        "isolated preflight failed: {}",
        String::from_utf8_lossy(&isolated_preflight.stderr)
    );
    let response: Value = serde_json::from_slice(&isolated_preflight.stdout).unwrap();
    assert_eq!(response["state"], "active-isolated");
    assert_eq!(
        fs::read(canonical_root.join("activation.json")).unwrap(),
        canonical_manifest_before
    );
    assert_eq!(
        fs::read(canonical_root.join("memory.sqlite")).unwrap(),
        canonical_sqlite_before
    );
    assert_eq!(
        json_files_below(&canonical_root.join("activation/receipts")),
        canonical_receipts_before
    );
    assert_eq!(
        json_files_below(&isolated_root.join("activation/receipts")),
        1
    );
    let output_text = String::from_utf8(isolated_preflight.stdout).unwrap();
    assert!(!output_text.contains(&canonical_project.to_string_lossy().to_string()));
    assert!(!output_text.contains(&isolated_project.to_string_lossy().to_string()));
}

fn tree_ring(root: &Path, project: &Path, path: &Path, home: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tree-ring"));
    command
        .current_dir(project)
        .env("PATH", path)
        .env("HOME", home)
        .arg("--root")
        .arg(root)
        .arg("--json");
    command
}

fn fingerprint_path(path: &Path) -> String {
    let canonical = fs::canonicalize(path).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    format!("{:x}", hasher.finalize())
}

fn prepend_path(directory: &Path) -> OsString {
    let mut entries = vec![directory.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        entries.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(entries).unwrap()
}

fn directory_files(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    entries.map(|entry| entry.unwrap().path()).collect()
}

fn json_files_below(directory: &Path) -> usize {
    let Ok(entries) = fs::read_dir(directory) else {
        return 0;
    };
    entries
        .map(|entry| entry.unwrap().path())
        .map(|path| {
            if path.is_dir() {
                json_files_below(&path)
            } else {
                usize::from(
                    path.extension()
                        .is_some_and(|extension| extension == "json"),
                )
            }
        })
        .sum()
}
