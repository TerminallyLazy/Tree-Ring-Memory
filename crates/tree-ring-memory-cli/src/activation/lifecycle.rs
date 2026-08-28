use super::{
    adapters::ActivationProject,
    manifest::fingerprint,
    preflight::{PreflightRequest, PreflightResponse},
    SessionIdentity,
};
use serde_json::{json, Map, Value};
use std::path::Path;
use tree_ring_memory_core::SensitivityGuard;
use uuid::Uuid;

const MAX_HOOK_INPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleHookEvent {
    SessionStart,
    SubagentStart,
    Stop,
    SubagentStop,
}

impl LifecycleHookEvent {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "SessionStart" => Ok(Self::SessionStart),
            "SubagentStart" => Ok(Self::SubagentStart),
            "Stop" => Ok(Self::Stop),
            "SubagentStop" => Ok(Self::SubagentStop),
            _ => Err("unsupported lifecycle hook event".to_string()),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::SubagentStart => "SubagentStart",
            Self::Stop => "Stop",
            Self::SubagentStop => "SubagentStop",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureCheckpoint {
    pub identity: SessionIdentity,
    pub project: String,
    pub checkpoint_id: String,
    pub stop_hook_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleHookRequest {
    pub event: LifecycleHookEvent,
    pub preflight: Option<PreflightRequest>,
    pub capture_checkpoint: Option<CaptureCheckpoint>,
}

/// Normalizes only stable, harness-owned lifecycle fields. Transcript paths,
/// model output, prompts, and arbitrary extension fields are deliberately
/// ignored and therefore cannot enter recall queries or activation receipts.
pub fn parse_lifecycle_hook(
    project: &ActivationProject,
    harness_id: &str,
    input: &str,
) -> Result<LifecycleHookRequest, String> {
    if input.len() > MAX_HOOK_INPUT_BYTES {
        return Err("lifecycle hook stdin exceeds 1 MiB".to_string());
    }
    if !matches!(harness_id, "codex" | "claude-code") {
        return Err("unknown lifecycle hook harness".to_string());
    }
    let value: Value =
        serde_json::from_str(input).map_err(|_| "invalid lifecycle hook stdin".to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "invalid lifecycle hook stdin".to_string())?;
    reject_capability_fields(object)?;

    let event = LifecycleHookEvent::parse(required_string(object, "hook_event_name")?)?;
    let cwd = Path::new(required_string(object, "cwd")?);
    ensure_cwd_inside_project(project, cwd)?;
    let parent_session = normalized("session", required_string(object, "session_id")?);

    let identity = match event {
        LifecycleHookEvent::SessionStart | LifecycleHookEvent::Stop => SessionIdentity {
            agent_profile: harness_id.to_string(),
            workflow_id: parent_session.clone(),
            session_id: parent_session,
        },
        LifecycleHookEvent::SubagentStart | LifecycleHookEvent::SubagentStop => {
            let agent_id = required_string(object, "agent_id")?;
            let agent_type = normalized("agent-type", required_string(object, "agent_type")?);
            SessionIdentity {
                agent_profile: format!(
                    "{harness_id}:{agent_type}:{}",
                    &fingerprint(agent_id)[..16]
                ),
                workflow_id: parent_session,
                session_id: normalized("agent", agent_id),
            }
        }
    };

    let (preflight, capture_checkpoint) = match event {
        LifecycleHookEvent::SessionStart | LifecycleHookEvent::SubagentStart => (
            Some(PreflightRequest::lifecycle_hook(
                harness_id,
                identity.clone(),
            )),
            None,
        ),
        LifecycleHookEvent::Stop | LifecycleHookEvent::SubagentStop => {
            let stop_hook_active = object
                .get("stop_hook_active")
                .and_then(Value::as_bool)
                .ok_or_else(|| "invalid lifecycle hook stdin".to_string())?;
            let project_name = project
                .project_root
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.trim().is_empty())
                .unwrap_or("project");
            (
                None,
                Some(CaptureCheckpoint {
                    identity,
                    project: normalized("project", project_name),
                    checkpoint_id: Uuid::new_v4().hyphenated().to_string(),
                    stop_hook_active,
                }),
            )
        }
    };

    Ok(LifecycleHookRequest {
        event,
        preflight,
        capture_checkpoint,
    })
}

pub fn render_lifecycle_hook(
    event: LifecycleHookEvent,
    response: &PreflightResponse,
) -> Result<String, String> {
    serde_json::to_string(&json!({
        "hookSpecificOutput": {
            "hookEventName": event.as_str(),
            "additionalContext": response.context,
        }
    }))
    .map_err(|_| "failed to serialize lifecycle hook context".to_string())
}

pub fn render_capture_checkpoint(checkpoint: &CaptureCheckpoint) -> Result<String, String> {
    if checkpoint.stop_hook_active {
        return Ok("{}".to_string());
    }
    let identity = &checkpoint.identity;
    let reason = format!(
        "Tree Ring automatic capture checkpoint {checkpoint_id}. Before stopping, review only the durable outcomes already available in your working context. For zero to three genuinely reusable normal-sensitivity preferences, decisions, validated lessons, warnings, corrections, or future seeds, run one strict capture command per candidate:\nproject_root=\"$(git rev-parse --show-toplevel 2>/dev/null || pwd)\"; tree-ring --root \"$project_root/.tree-ring\" capture \"<concise summary>\" --event-type <preference|decision|lesson|warning|correction|seed> --ring <cambium|scar|seed> --project \"{project}\" --agent-profile \"{agent_profile}\" --workflow-id \"{workflow_id}\" --session-id \"{session_id}\" --operation-id auto-{checkpoint_id}-<1|2|3> --source-ref agent-checkpoint:{checkpoint_id}\nUse the same indexed operation ID only when retrying that same candidate. Use only `tree-ring capture` for this automatic checkpoint; `remember` and `evidence` remain separate manual surfaces. If nothing durable occurred, write nothing and finish; never invent memory. Never store raw prompts, transcripts, tool logs, secrets, or sensitive data, and do not start a background recorder. After this checkpoint, finish the response.",
        checkpoint_id = checkpoint.checkpoint_id,
        project = checkpoint.project,
        agent_profile = identity.agent_profile,
        workflow_id = identity.workflow_id,
        session_id = identity.session_id,
    );
    serde_json::to_string(&json!({
        "decision": "block",
        "reason": reason,
    }))
    .map_err(|_| "failed to serialize automatic capture checkpoint".to_string())
}

fn required_string<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a str, String> {
    object
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "invalid lifecycle hook stdin".to_string())
}

fn normalized(label: &str, value: &str) -> String {
    if value.len() <= 128
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_.:@".contains(character))
        && !matches!(value, "." | "..")
        && SensitivityGuard::default().inspect(value).sensitivity == "normal"
    {
        value.to_string()
    } else {
        format!("{label}-{}", &fingerprint(value)[..24])
    }
}

fn ensure_cwd_inside_project(project: &ActivationProject, cwd: &Path) -> Result<(), String> {
    let project_root = std::fs::canonicalize(&project.project_root)
        .map_err(|_| "lifecycle hook project root unavailable".to_string())?;
    let cwd =
        std::fs::canonicalize(cwd).map_err(|_| "lifecycle hook cwd unavailable".to_string())?;
    if cwd == project_root || cwd.starts_with(&project_root) {
        Ok(())
    } else {
        Err("lifecycle hook cwd is outside the project".to_string())
    }
}

fn reject_capability_fields(object: &Map<String, Value>) -> Result<(), String> {
    const BLOCKED: &[&str] = &[
        "authorization",
        "capability",
        "capabilities",
        "coordinator_capability",
        "memory_root",
        "project_root",
        "store_id",
        "task_hint",
    ];
    if object.keys().any(|key| BLOCKED.contains(&key.as_str())) {
        Err("lifecycle hook stdin contains forbidden fields".to_string())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture() -> (tempfile::TempDir, ActivationProject) {
        let temp = tempfile::tempdir().unwrap();
        let project = ActivationProject::from_project_root(temp.path().join("project"));
        fs::create_dir_all(project.project_root.join("src")).unwrap();
        (temp, project)
    }

    #[test]
    fn session_start_derives_identity_without_reading_transcript_or_prompt() {
        let (_temp, project) = fixture();
        let input = json!({
            "session_id": "thread-1",
            "cwd": project.project_root.join("src"),
            "hook_event_name": "SessionStart",
            "source": "startup",
            "transcript_path": "/private/transcript.jsonl",
            "model": "gpt-example"
        });

        let request = parse_lifecycle_hook(&project, "codex", &input.to_string()).unwrap();

        assert_eq!(request.event, LifecycleHookEvent::SessionStart);
        let preflight = request.preflight.unwrap();
        assert_eq!(preflight.identity.agent_profile, "codex");
        assert_eq!(preflight.identity.workflow_id, "thread-1");
        assert_eq!(preflight.identity.session_id, "thread-1");
        assert!(request.capture_checkpoint.is_none());
    }

    #[test]
    fn subagent_start_derives_distinct_worker_identity() {
        let (_temp, project) = fixture();
        let input = json!({
            "session_id": "thread-1",
            "cwd": project.project_root,
            "hook_event_name": "SubagentStart",
            "agent_id": "agent-2",
            "agent_type": "reviewer"
        });

        let request = parse_lifecycle_hook(&project, "claude-code", &input.to_string()).unwrap();

        assert_eq!(request.event, LifecycleHookEvent::SubagentStart);
        let preflight = request.preflight.unwrap();
        assert!(preflight
            .identity
            .agent_profile
            .starts_with("claude-code:reviewer:"));
        assert_eq!(preflight.identity.workflow_id, "thread-1");
        assert_eq!(preflight.identity.session_id, "agent-2");
        assert!(request.capture_checkpoint.is_none());
    }

    #[test]
    fn stop_enforces_one_structured_checkpoint_without_reading_model_output() {
        let (_temp, project) = fixture();
        let private_output = "private assistant output that must not be captured";
        let input = json!({
            "session_id": "thread-1",
            "cwd": project.project_root,
            "hook_event_name": "Stop",
            "stop_hook_active": false,
            "last_assistant_message": private_output,
            "transcript_path": "/private/transcript.jsonl"
        });

        let request = parse_lifecycle_hook(&project, "codex", &input.to_string()).unwrap();
        let checkpoint = request.capture_checkpoint.unwrap();
        let rendered = render_capture_checkpoint(&checkpoint).unwrap();

        assert_eq!(request.event, LifecycleHookEvent::Stop);
        assert!(request.preflight.is_none());
        assert!(rendered.contains("\"decision\":\"block\""));
        assert!(rendered.contains(" tree-ring --root "));
        assert!(rendered.contains(" capture "));
        assert!(!rendered.contains("--scope"));
        assert!(!rendered.contains(private_output));
        assert!(!rendered.contains("transcript.jsonl"));
    }

    #[test]
    fn active_stop_checkpoint_allows_completion_without_a_loop() {
        let (_temp, project) = fixture();
        let input = json!({
            "session_id": "thread-1",
            "cwd": project.project_root,
            "hook_event_name": "SubagentStop",
            "stop_hook_active": true,
            "agent_id": "agent-2",
            "agent_type": "reviewer"
        });

        let request = parse_lifecycle_hook(&project, "claude-code", &input.to_string()).unwrap();
        let checkpoint = request.capture_checkpoint.unwrap();

        assert_eq!(request.event, LifecycleHookEvent::SubagentStop);
        assert!(checkpoint
            .identity
            .agent_profile
            .starts_with("claude-code:reviewer:"));
        assert_eq!(render_capture_checkpoint(&checkpoint).unwrap(), "{}");
    }

    #[test]
    fn hook_rejects_model_supplied_roots_and_capabilities() {
        let (_temp, project) = fixture();
        let input = json!({
            "session_id": "thread-1",
            "cwd": project.project_root,
            "hook_event_name": "SessionStart",
            "memory_root": "/tmp/other"
        });

        let error = parse_lifecycle_hook(&project, "codex", &input.to_string()).unwrap_err();
        assert_eq!(error, "lifecycle hook stdin contains forbidden fields");
    }
}
