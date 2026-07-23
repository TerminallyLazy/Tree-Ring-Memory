use std::{
    path::Path,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;
use tempfile::tempdir;
use tree_ring_memory_core::MemoryEvent;
use tree_ring_memory_sqlite::SQLiteMemoryStore;

const WORKER_COUNT: usize = 8;
const PROJECT: &str = "multi-agent-acceptance";
const WORKFLOW: &str = "fanout-acceptance";
const SESSION: &str = "attempt-1";
const QUERY_TOKEN: &str = "swarmtoken";

#[derive(Debug, Clone)]
struct WriteSpec {
    summary: String,
    scope: String,
    agent_profile: String,
    workflow_id: String,
    session_id: String,
    operation_id: String,
}

impl WriteSpec {
    fn worker(index: usize) -> Self {
        Self {
            summary: format!("{QUERY_TOKEN} worker {index} completed its fan-out task."),
            scope: "agent".to_string(),
            agent_profile: format!("worker-{index}"),
            workflow_id: WORKFLOW.to_string(),
            session_id: SESSION.to_string(),
            operation_id: format!("operation-worker-{index}"),
        }
    }
}

#[test]
fn real_cli_processes_preserve_multi_agent_isolation_and_idempotency() {
    let temp = tempdir().unwrap();
    let root = temp.path().join(".tree-ring");

    let init = base_command(&root).arg("init").output().unwrap();
    assert_success("init", &init);
    let init_json = parse_json("init", &init);
    assert_eq!(init_json["ok"], true);

    let worker_specs = (0..WORKER_COUNT).map(WriteSpec::worker).collect::<Vec<_>>();
    let lock_holder = SQLiteMemoryStore::open(root.join("memory.sqlite")).unwrap();
    lock_holder
        .connection()
        .execute_batch("BEGIN IMMEDIATE")
        .unwrap();
    let mut workers = worker_specs
        .iter()
        .map(|spec| {
            let child = spawn_remember(&root, spec);
            (spec, child)
        })
        .collect::<Vec<_>>();

    thread::sleep(Duration::from_millis(250));
    let waiting_statuses = workers
        .iter_mut()
        .map(|(spec, child)| (spec.agent_profile.clone(), child.try_wait()))
        .collect::<Vec<_>>();
    lock_holder.connection().execute_batch("COMMIT").unwrap();
    drop(lock_holder);
    for (agent_profile, status) in waiting_statuses {
        assert!(
            status.unwrap().is_none(),
            "{} exited instead of waiting for the shared write lock",
            agent_profile
        );
    }

    let mut worker_memories = Vec::with_capacity(WORKER_COUNT);
    for (spec, child) in workers.drain(..) {
        let output = wait_with_output_bounded(child, &spec.agent_profile);
        assert_success(&format!("remember {}", spec.agent_profile), &output);
        let memory = parse_memory(&format!("remember {}", spec.agent_profile), &output);
        assert_eq!(memory.scope, "agent");
        assert_eq!(
            memory.agent_profile.as_deref(),
            Some(spec.agent_profile.as_str())
        );
        assert_eq!(
            memory.workflow_id.as_deref(),
            Some(spec.workflow_id.as_str())
        );
        assert_eq!(memory.session_id.as_deref(), Some(spec.session_id.as_str()));
        assert_eq!(
            memory.operation_id.as_deref(),
            Some(spec.operation_id.as_str())
        );
        worker_memories.push(memory);
    }

    let target_profile = worker_specs[3].agent_profile.clone();
    let decoys = [
        WriteSpec {
            summary: format!("{QUERY_TOKEN} workflow decoy."),
            scope: "agent".to_string(),
            agent_profile: target_profile.clone(),
            workflow_id: "other-workflow".to_string(),
            session_id: SESSION.to_string(),
            operation_id: "operation-workflow-decoy".to_string(),
        },
        WriteSpec {
            summary: format!("{QUERY_TOKEN} session decoy."),
            scope: "agent".to_string(),
            agent_profile: target_profile.clone(),
            workflow_id: WORKFLOW.to_string(),
            session_id: "other-session".to_string(),
            operation_id: "operation-session-decoy".to_string(),
        },
        WriteSpec {
            summary: format!("{QUERY_TOKEN} scope decoy."),
            scope: "project".to_string(),
            agent_profile: target_profile.clone(),
            workflow_id: WORKFLOW.to_string(),
            session_id: SESSION.to_string(),
            operation_id: "operation-scope-decoy".to_string(),
        },
    ];
    for spec in &decoys {
        let output = remember_command(&root, spec).output().unwrap();
        assert_success(&format!("remember {}", spec.operation_id), &output);
        parse_memory(&format!("remember {}", spec.operation_id), &output);
    }

    let by_profile = recall(
        &root,
        &[
            ("--agent-profile", target_profile.as_str()),
            ("--scope", "agent"),
        ],
    );
    assert_eq!(by_profile.len(), 3);
    assert!(by_profile.iter().all(|memory| {
        memory.agent_profile.as_deref() == Some(target_profile.as_str()) && memory.scope == "agent"
    }));

    let by_workflow = recall(&root, &[("--workflow-id", WORKFLOW), ("--scope", "agent")]);
    assert_eq!(by_workflow.len(), WORKER_COUNT + 1);
    assert!(by_workflow
        .iter()
        .all(|memory| memory.workflow_id.as_deref() == Some(WORKFLOW)));

    let by_session = recall(&root, &[("--session-id", SESSION), ("--scope", "agent")]);
    assert_eq!(by_session.len(), WORKER_COUNT + 1);
    assert!(by_session
        .iter()
        .all(|memory| memory.session_id.as_deref() == Some(SESSION)));

    let by_scope = recall(&root, &[("--scope", "project")]);
    assert_eq!(by_scope.len(), 1);
    assert_eq!(by_scope[0].scope, "project");

    let combined = recall(
        &root,
        &[
            ("--agent-profile", target_profile.as_str()),
            ("--workflow-id", WORKFLOW),
            ("--session-id", SESSION),
            ("--scope", "agent"),
        ],
    );
    assert_eq!(combined.len(), 1);
    assert_eq!(
        combined[0].agent_profile.as_deref(),
        Some(target_profile.as_str())
    );

    let retry = remember_command(&root, &worker_specs[0]).output().unwrap();
    assert_success("idempotent operation retry", &retry);
    let retried_memory = parse_memory("idempotent operation retry", &retry);
    assert_eq!(retried_memory.id, worker_memories[0].id);

    let mut conflict = worker_specs[0].clone();
    conflict.summary = format!("{QUERY_TOKEN} conflicting operation reuse.");
    let conflict_output = remember_command(&root, &conflict).output().unwrap();
    assert!(
        !conflict_output.status.success(),
        "conflicting operation unexpectedly succeeded: {}",
        String::from_utf8_lossy(&conflict_output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&conflict_output.stderr)
            .contains("already bound to a different memory write"),
        "unexpected conflict error: {}",
        String::from_utf8_lossy(&conflict_output.stderr)
    );

    let maintain = base_command(&root).arg("maintain").output().unwrap();
    assert_success("maintenance parity report", &maintain);
    let report = parse_json("maintenance parity report", &maintain);
    let expected_rows = WORKER_COUNT + decoys.len();
    assert_eq!(report["memory_count"], expected_rows);
    assert_eq!(report["fts"]["memory_rows"], expected_rows);
    assert_eq!(report["fts"]["fts_rows"], expected_rows);
    assert_eq!(report["fts"]["missing_fts_rows"], 0);
    assert_eq!(report["fts"]["orphan_fts_rows"], 0);
}

fn base_command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tree-ring"));
    command
        .env_remove("TREE_RING_AGENT_PROFILE")
        .env_remove("TREE_RING_WORKFLOW_ID")
        .env_remove("TREE_RING_SESSION_ID")
        .arg("--root")
        .arg(root)
        .arg("--json");
    command
}

fn remember_command(root: &Path, spec: &WriteSpec) -> Command {
    let mut command = base_command(root);
    command
        .arg("remember")
        .arg(&spec.summary)
        .arg("--event-type")
        .arg("lesson")
        .arg("--scope")
        .arg(&spec.scope)
        .arg("--project")
        .arg(PROJECT)
        .arg("--agent-profile")
        .arg(&spec.agent_profile)
        .arg("--workflow-id")
        .arg(&spec.workflow_id)
        .arg("--session-id")
        .arg(&spec.session_id)
        .arg("--operation-id")
        .arg(&spec.operation_id)
        .arg("--source-ref")
        .arg(format!(
            "runs/{}/{}/{}.json",
            spec.workflow_id, spec.session_id, spec.agent_profile
        ))
        .arg("--tag")
        .arg("multi-agent");
    command
}

fn spawn_remember(root: &Path, spec: &WriteSpec) -> Child {
    remember_command(root, spec)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

fn wait_with_output_bounded(mut child: Child, context: &str) -> Output {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "{context} exceeded the SQLite busy-timeout bound; stdout={}; stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn recall(root: &Path, filters: &[(&str, &str)]) -> Vec<MemoryEvent> {
    let mut command = base_command(root);
    command
        .arg("recall")
        .arg(QUERY_TOKEN)
        .arg("--limit")
        .arg("64");
    for (flag, value) in filters {
        command.arg(flag).arg(value);
    }
    let output = command.output().unwrap();
    assert_success("recall", &output);
    let payload = parse_json("recall", &output);
    payload
        .as_array()
        .expect("recall JSON must be an array")
        .iter()
        .map(|entry| {
            serde_json::from_value(entry["memory"].clone())
                .expect("recall entry must contain a valid memory")
        })
        .collect()
}

fn parse_memory(context: &str, output: &Output) -> MemoryEvent {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{context} did not emit a memory JSON object: {error}; stdout={}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn parse_json(context: &str, output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{context} did not emit valid JSON: {error}; stdout={}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn assert_success(context: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{context} failed with status {:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
