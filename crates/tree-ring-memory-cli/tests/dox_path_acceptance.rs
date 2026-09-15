use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use tempfile::tempdir;
use tree_ring_memory_core::{collect_dox_memories, DoxSyncRequest, MemoryEvent};
use tree_ring_memory_sqlite::SQLiteMemoryStore;

const PROJECT: &str = "dox-path-acceptance";

fn legacy_event(source: &Path) -> MemoryEvent {
    let mut request = DoxSyncRequest::new(source);
    request.project = Some(PROJECT.to_string());
    let mut event = collect_dox_memories(&request)
        .unwrap()
        .events
        .pop()
        .unwrap();
    event.links.retain(|link| link.link_type != "dox-root");
    event
}

fn write_legacy(database: &Path, event: &MemoryEvent) {
    fs::create_dir_all(database.parent().unwrap()).unwrap();
    SQLiteMemoryStore::open(database)
        .unwrap()
        .put(event)
        .unwrap();
}

fn sync(project_root: &Path, source_root: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tree-ring"))
        .current_dir(project_root)
        .env("PATH", "")
        .env_remove("TREE_RING_AGENT_PROFILE")
        .env_remove("TREE_RING_WORKFLOW_ID")
        .env_remove("TREE_RING_SESSION_ID")
        .env_remove("TREE_RING_OPERATION_ID")
        .env_remove("TREE_RING_COORDINATOR_TOKEN")
        .args([
            "--json",
            "--root",
            ".tree-ring",
            "dox",
            "sync",
            "--source-root",
            source_root,
            "--project",
            PROJECT,
        ])
        .output()
        .unwrap()
}

#[test]
fn bare_relative_agents_file_adopts_legacy_provenance_in_its_local_store() {
    let temp = tempdir().unwrap();
    let project = temp.path().join("Project With Spaces");
    fs::create_dir(&project).unwrap();
    let source = project.join("AGENTS.md");
    fs::write(&source, "# Rules\n\nKeep project guidance.\n").unwrap();
    let legacy = legacy_event(&source);
    let database = project.join(".tree-ring/memory.sqlite");
    write_legacy(&database, &legacy);

    for _ in 0..2 {
        let output = sync(&project, "AGENTS.md");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let saved = SQLiteMemoryStore::open_read_only(&database)
            .unwrap()
            .list_all(true)
            .unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].id, legacy.id);
        assert_eq!(saved[0].summary, legacy.summary);
        assert_eq!(
            saved[0]
                .links
                .iter()
                .filter(|link| link.link_type == "dox-root")
                .count(),
            1
        );
    }
}

#[cfg(unix)]
#[test]
fn symlinked_store_paths_cannot_adopt_legacy_provenance_or_insert_partial_batches() {
    use std::os::unix::fs::symlink;

    for link_directory in [true, false] {
        let temp = tempdir().unwrap();
        let project = temp.path().join("project");
        fs::create_dir(&project).unwrap();
        let source = project.join("AGENTS.md");
        fs::write(
            &source,
            "# Fresh\n\nRun scoped checks.\n\n# Rules\n\nKeep project guidance.\n",
        )
        .unwrap();
        let legacy = legacy_event(&source);
        let database = temp.path().join("other-project/.tree-ring/memory.sqlite");
        write_legacy(&database, &legacy);
        let alias = project.join(".tree-ring");
        if link_directory {
            symlink(database.parent().unwrap(), &alias).unwrap();
        } else {
            fs::create_dir(&alias).unwrap();
            symlink(&database, alias.join("memory.sqlite")).unwrap();
        }

        // The absolute source spelling isolates store aliasing from the bare
        // relative source regression above. Both spellings must remain safe.
        let output = sync(&project, source.to_str().unwrap());
        assert!(
            !output.status.success(),
            "symlinked directory={link_directory} unexpectedly adopted legacy provenance"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("legacy DOX memory has no root provenance"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let saved = SQLiteMemoryStore::open_read_only(&database)
            .unwrap()
            .list_all(true)
            .unwrap();
        assert_eq!(saved, vec![legacy]);
        assert!(fs::symlink_metadata(if link_directory {
            alias
        } else {
            alias.join("memory.sqlite")
        })
        .unwrap()
        .file_type()
        .is_symlink());
    }
}
