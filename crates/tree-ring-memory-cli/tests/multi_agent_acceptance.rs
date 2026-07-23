use std::{
    path::Path,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;
use tempfile::tempdir;
use tree_ring_memory_core::MemoryEvent;

const WORKER_COUNT: usize = 8;
const PROJECT: &str = "multi-agent-acceptance";
const WORKFLOW: &str = "fanout-acceptance";
const SESSION: &str = "attempt-1";
const QUERY_TOKEN: &str = "swarmtoken";
const COORDINATOR_TOKEN_ENV: &str = "TREE_RING_COORDINATOR_TOKEN";

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
    let lock_holder = rusqlite::Connection::open(root.join("memory.sqlite")).unwrap();
    lock_holder.execute_batch("BEGIN IMMEDIATE").unwrap();
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
    lock_holder.execute_batch("COMMIT").unwrap();
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

#[test]
fn coordinated_policy_guards_shared_mutations_across_real_cli_processes() {
    let temp = tempdir().unwrap();
    let root = temp.path().join(".tree-ring");

    let init = base_command(&root).arg("init").output().unwrap();
    assert_success("init", &init);

    let enable = base_command(&root)
        .arg("policy")
        .arg("enable")
        .arg("--coordinator")
        .arg("acceptance-owner")
        .output()
        .unwrap();
    assert_success("policy enable", &enable);
    let grant = parse_json("policy enable", &enable);
    let coordinator_token = grant["capability"]
        .as_str()
        .expect("policy enable must return its one-time capability")
        .to_string();
    assert!(!coordinator_token.trim().is_empty());
    assert!(!String::from_utf8_lossy(&enable.stderr).contains(&coordinator_token));

    let status = base_command(&root)
        .arg("policy")
        .arg("status")
        .output()
        .unwrap();
    assert_success("policy status", &status);
    assert_eq!(
        parse_json("policy status", &status)["mode"]
            .as_str()
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("coordinated")
    );

    let worker = WriteSpec::worker(100);
    let worker_write = remember_command(&root, &worker).output().unwrap();
    assert_success("authorized worker-private write", &worker_write);
    let worker_memory = parse_memory("authorized worker-private write", &worker_write);
    assert_eq!(worker_memory.scope, "agent");
    assert_eq!(
        worker_memory.agent_profile.as_deref(),
        Some(worker.agent_profile.as_str())
    );

    let denied_specs = (0..WORKER_COUNT)
        .map(|index| WriteSpec {
            summary: format!("{QUERY_TOKEN} unauthorized shared write {index}."),
            scope: "project".to_string(),
            agent_profile: format!("worker-{index}"),
            workflow_id: WORKFLOW.to_string(),
            session_id: SESSION.to_string(),
            operation_id: format!("operation-denied-{index}"),
        })
        .collect::<Vec<_>>();
    let denied_children = denied_specs
        .iter()
        .map(|spec| (spec, spawn_remember(&root, spec)))
        .collect::<Vec<_>>();
    for (spec, child) in denied_children {
        let output = wait_with_output_bounded(child, &spec.operation_id);
        assert_authorization_denied(&spec.operation_id, &output);
    }

    let shared_before_coordinator = recall(&root, &[("--scope", "project")]);
    assert!(
        shared_before_coordinator.is_empty(),
        "unauthorized concurrent writes reached shared project memory"
    );

    let denied_promotion = base_command(&root)
        .arg("evidence")
        .arg(format!("{QUERY_TOKEN} worker attempted promotion."))
        .arg("--outcome")
        .arg("promoted")
        .arg("--evidence-ref")
        .arg("runs/fanout-acceptance/worker-promotion.json")
        .arg("--project")
        .arg(PROJECT)
        .arg("--agent-profile")
        .arg("worker-3")
        .arg("--workflow-id")
        .arg(WORKFLOW)
        .arg("--session-id")
        .arg(SESSION)
        .arg("--operation-id")
        .arg("operation-denied-promotion")
        .output()
        .unwrap();
    assert_authorization_denied("worker heartwood promotion", &denied_promotion);

    let mut approved_promotion = base_command(&root);
    approved_promotion
        .env(COORDINATOR_TOKEN_ENV, &coordinator_token)
        .arg("evidence")
        .arg(format!("{QUERY_TOKEN} coordinator approved promotion."))
        .arg("--outcome")
        .arg("promoted")
        .arg("--evidence-ref")
        .arg("runs/fanout-acceptance/coordinator-promotion.json")
        .arg("--project")
        .arg(PROJECT)
        .arg("--agent-profile")
        .arg("coordinator")
        .arg("--workflow-id")
        .arg(WORKFLOW)
        .arg("--session-id")
        .arg(SESSION)
        .arg("--operation-id")
        .arg("operation-approved-promotion");
    let approved_promotion_output = approved_promotion.output().unwrap();
    assert_success(
        "coordinator heartwood promotion",
        &approved_promotion_output,
    );
    assert_eq!(
        parse_memory(
            "coordinator heartwood promotion",
            &approved_promotion_output
        )
        .ring,
        "heartwood"
    );

    let coordinator_spec = WriteSpec {
        summary: format!("{QUERY_TOKEN} coordinator-approved shared result."),
        scope: "project".to_string(),
        agent_profile: "coordinator".to_string(),
        workflow_id: WORKFLOW.to_string(),
        session_id: SESSION.to_string(),
        operation_id: "operation-coordinator-shared".to_string(),
    };
    let mut coordinator_write = remember_command(&root, &coordinator_spec);
    coordinator_write.env(COORDINATOR_TOKEN_ENV, &coordinator_token);
    let coordinator_output = coordinator_write.output().unwrap();
    assert_success("coordinator shared write", &coordinator_output);
    let coordinator_memory = parse_memory("coordinator shared write", &coordinator_output);
    assert_eq!(coordinator_memory.scope, "project");

    let wrong_token_spec = WriteSpec {
        summary: format!("{QUERY_TOKEN} wrong-token shared result."),
        scope: "project".to_string(),
        agent_profile: "coordinator".to_string(),
        workflow_id: WORKFLOW.to_string(),
        session_id: SESSION.to_string(),
        operation_id: "operation-wrong-token".to_string(),
    };
    let mut wrong_token_write = remember_command(&root, &wrong_token_spec);
    wrong_token_write.env(COORDINATOR_TOKEN_ENV, "not-the-capability");
    let wrong_token_output = wrong_token_write.output().unwrap();
    assert_authorization_denied("wrong coordinator token", &wrong_token_output);

    let mut rotate = base_command(&root);
    rotate
        .env(COORDINATOR_TOKEN_ENV, &coordinator_token)
        .arg("policy")
        .arg("rotate")
        .arg("--coordinator")
        .arg("rotated-owner");
    let rotate_output = rotate.output().unwrap();
    assert_success("policy rotate", &rotate_output);
    let rotated_grant = parse_json("policy rotate", &rotate_output);
    let rotated_token = rotated_grant["capability"]
        .as_str()
        .expect("policy rotate must return its one-time capability")
        .to_string();
    assert_ne!(rotated_token, coordinator_token);

    let stale_token_spec = WriteSpec {
        summary: format!("{QUERY_TOKEN} stale-token shared result."),
        scope: "project".to_string(),
        agent_profile: "coordinator".to_string(),
        workflow_id: WORKFLOW.to_string(),
        session_id: SESSION.to_string(),
        operation_id: "operation-stale-token".to_string(),
    };
    let mut stale_token_write = remember_command(&root, &stale_token_spec);
    stale_token_write.env(COORDINATOR_TOKEN_ENV, &coordinator_token);
    let stale_token_output = stale_token_write.output().unwrap();
    assert_authorization_denied("rotated coordinator token", &stale_token_output);

    let audit = base_command(&root)
        .arg("policy")
        .arg("audit")
        .arg("--limit")
        .arg("100")
        .output()
        .unwrap();
    assert_success("policy audit", &audit);
    let audit_json = parse_json("policy audit", &audit);
    let audit_events = audit_json
        .as_array()
        .expect("policy audit JSON must be an array");
    let denied_events = audit_events
        .iter()
        .filter(|event| event["decision"].as_str() == Some("denied"))
        .collect::<Vec<_>>();
    let allowed_events = audit_events
        .iter()
        .filter(|event| event["decision"].as_str() == Some("allowed"))
        .collect::<Vec<_>>();
    assert_eq!(
        denied_events.len(),
        WORKER_COUNT + 3,
        "every concurrent/shared/promotion denial must be audited exactly once"
    );
    assert_eq!(
        allowed_events.len(),
        4,
        "enable, two coordinator writes, and rotation must be audited exactly once"
    );
    for index in 0..WORKER_COUNT {
        let actor = format!("worker-{index}");
        assert!(
            denied_events
                .iter()
                .any(|event| event["actor_profile"].as_str() == Some(actor.as_str())),
            "missing denial audit for {actor}"
        );
    }
    let serialized_audit = serde_json::to_string(audit_events).unwrap();
    assert!(!serialized_audit.contains(&coordinator_token));
    assert!(!serialized_audit.contains(&rotated_token));

    let mut disable = base_command(&root);
    disable
        .env(COORDINATOR_TOKEN_ENV, &rotated_token)
        .arg("policy")
        .arg("disable");
    let disable_output = disable.output().unwrap();
    assert_success("policy disable", &disable_output);

    let open_mode_spec = WriteSpec {
        summary: format!("{QUERY_TOKEN} open-mode shared result."),
        scope: "project".to_string(),
        agent_profile: "worker-after-disable".to_string(),
        workflow_id: WORKFLOW.to_string(),
        session_id: SESSION.to_string(),
        operation_id: "operation-open-after-disable".to_string(),
    };
    let open_mode_output = remember_command(&root, &open_mode_spec).output().unwrap();
    assert_success("open mode restored", &open_mode_output);
}

fn base_command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tree-ring"));
    command
        .env_remove("TREE_RING_AGENT_PROFILE")
        .env_remove("TREE_RING_WORKFLOW_ID")
        .env_remove("TREE_RING_SESSION_ID")
        .env_remove(COORDINATOR_TOKEN_ENV)
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

fn assert_authorization_denied(context: &str, output: &Output) {
    assert!(
        !output.status.success(),
        "{context} unexpectedly succeeded; stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_ascii_lowercase().contains("authoriz"),
        "{context} returned a non-authorization error: {stderr}"
    );
}
