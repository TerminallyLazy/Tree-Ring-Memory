#![cfg(unix)]

use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt,
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
