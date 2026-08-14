use super::{
    adapters::{ActivationProject, AdapterPlan, ManagedBlockUpdate, PlannedWrite},
    manifest::{
        save_manifest, validate_manifest, validate_project_relative_path, ActivationManifest,
        HarnessActivation, OwnedBridgeFile, OwnedManagedBlock,
    },
    ActivationState, AdapterCapability, ACTIVATION_PROTOCOL_VERSION,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

const CODEX_RETRY: &str = "tree-ring integrations activate --harness codex --accept-managed-block";
const CLAUDE_DESCRIPTION: &str = "Tree Ring Memory managed preflight v1";
const CLAUDE_COMMAND: &str = "tree-ring --root .tree-ring integrations preflight --harness claude-code --input-json-stdin --context-format claude-session-start";

const SKILL_BRIDGE: &str = r#"---
name: tree-ring-memory
description: Use the canonical project-local Tree Ring Memory guidance and preflight contract.
---

# Tree Ring Memory

Read `.tree-ring/SKILL.md`, `.tree-ring/AGENTS.md`, and `.tree-ring/CLI.md` from this project. Before substantive work, use the project-local Tree Ring preflight interface for this harness. Treat configured guidance as non-active until a receipt-producing preflight succeeds.
"#;

const CODEX_BLOCK_BODY: &str = r#"## Tree Ring Memory

Read `.tree-ring/AGENTS.md`, `.tree-ring/SKILL.md`, and `.tree-ring/CLI.md` before substantive work. Use `tree-ring --root .tree-ring integrations preflight --harness codex` with the later preflight identity interface, and do not claim memory is active without a valid project-local recall receipt.
"#;

const PI_EXTENSION: &str = r#"import { spawn } from "node:child_process";
import type { ExtensionAPI } from "@mariozechner/pi-coding-agent";

type PreflightResponse = { context: string };

function invokePreflight(cwd: string, input: string): Promise<PreflightResponse> {
  return new Promise((resolve, reject) => {
    const child = spawn(
      "tree-ring",
      [
        "--root",
        ".tree-ring",
        "integrations",
        "preflight",
        "--harness",
        "pi",
        "--input-json-stdin",
        "--context-format",
        "pi-before-agent-start",
      ],
      { cwd, stdio: ["pipe", "pipe", "pipe"] },
    );
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0) {
        reject(new Error(stderr || `Tree Ring preflight exited with status ${code}`));
        return;
      }
      try {
        const response = JSON.parse(stdout) as PreflightResponse;
        if (typeof response.context !== "string") throw new Error("missing context");
        resolve(response);
      } catch (error) {
        reject(new Error(`Invalid Tree Ring preflight response: ${String(error)}`));
      }
    });
    child.stdin.end(input);
  });
}

function sensitivityRejected(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return message.includes("sensitive-task-hint");
}

async function runPreflightWithStdin(cwd: string, input: string): Promise<PreflightResponse> {
  try {
    return await invokePreflight(cwd, input);
  } catch (error) {
    if (!sensitivityRejected(error)) throw error;
    const safeInput = {
      ...(JSON.parse(input) as Record<string, unknown>),
      task_hint: "project startup constraints",
    };
    return invokePreflight(cwd, JSON.stringify(safeInput));
  }
}

export default function treeRingMemory(pi: ExtensionAPI) {
  pi.on("before_agent_start", async (event, ctx) => {
    const input = JSON.stringify({
      agent_profile: "pi",
      workflow_id: ctx.sessionManager.getSessionFile() ?? "pi-ephemeral",
      session_id: ctx.sessionManager.getSessionFile() ?? "pi-ephemeral",
      task_hint: event.prompt,
    });
    const response = await runPreflightWithStdin(ctx.cwd, input);
    return { message: { customType: "tree-ring-preflight", content: response.context, display: false } };
  });
}
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgePlanResult {
    pub state: ActivationState,
    pub changed_paths: Vec<PathBuf>,
    pub next_step: String,
}

#[derive(Debug, Clone)]
struct PreparedFile {
    relative: PathBuf,
    before: Option<Vec<u8>>,
    after: Option<Vec<u8>>,
}

#[derive(Debug)]
enum Preparation {
    Ready {
        files: Vec<PreparedFile>,
        activation: HarnessActivation,
    },
    Review {
        next_step: String,
    },
}

/// Applies an adapter plan after every target has passed ownership and format
/// checks. A review result is non-mutating and never leaves a partial bridge.
pub fn apply_bridge_plan(
    project: &ActivationProject,
    manifest: &mut ActivationManifest,
    plan: AdapterPlan,
    accept_managed_block: bool,
) -> Result<BridgePlanResult, String> {
    validate_project(project)?;
    validate_manifest(manifest)?;
    validate_plan(&plan)?;

    if matches!(
        plan.state,
        ActivationState::NeedsPlugin | ActivationState::Unsupported
    ) {
        if !plan.writes.is_empty() {
            return Err(format!(
                "{} plan cannot write while in state {:?}",
                plan.harness_id, plan.state
            ));
        }
        let mut next_manifest = manifest.clone();
        next_manifest.harnesses.insert(
            plan.harness_id.clone(),
            HarnessActivation {
                state: plan.state,
                adapter_capability: capability_for(&plan.harness_id)?,
                bridge_path: None,
                owned_files: Vec::new(),
                managed_blocks: Vec::new(),
            },
        );
        save_manifest(&project.memory_root, &next_manifest)?;
        *manifest = next_manifest;
        return Ok(BridgePlanResult {
            state: plan.state,
            changed_paths: Vec::new(),
            next_step: plan.next_step,
        });
    }

    let prepared = prepare_apply(project, manifest, &plan, accept_managed_block)?;
    let Preparation::Ready { files, activation } = prepared else {
        let Preparation::Review { next_step } = prepared else {
            unreachable!()
        };
        return Ok(BridgePlanResult {
            state: ActivationState::NeedsUserReview,
            changed_paths: Vec::new(),
            next_step,
        });
    };

    let mut next_manifest = manifest.clone();
    next_manifest
        .harnesses
        .insert(plan.harness_id.clone(), activation);
    validate_manifest(&next_manifest)?;
    let changed_paths = files
        .iter()
        .filter(|file| file.before != file.after)
        .map(|file| file.relative.clone())
        .collect::<Vec<_>>();
    commit_files_and_manifest(project, manifest, next_manifest, &files)?;

    let state = applied_state(&plan.harness_id, plan.state);
    Ok(BridgePlanResult {
        state,
        changed_paths,
        next_step: plan.next_step,
    })
}

/// Produces the same validated plan as apply without writing files or the
/// activation manifest.
pub fn preview_bridge_plan(
    project: &ActivationProject,
    manifest: &ActivationManifest,
    plan: AdapterPlan,
    accept_managed_block: bool,
) -> Result<BridgePlanResult, String> {
    validate_project(project)?;
    validate_manifest(manifest)?;
    validate_plan(&plan)?;
    if matches!(
        plan.state,
        ActivationState::NeedsPlugin | ActivationState::Unsupported
    ) {
        if !plan.writes.is_empty() {
            return Err(format!(
                "{} plan cannot write while in state {:?}",
                plan.harness_id, plan.state
            ));
        }
        return Ok(BridgePlanResult {
            state: plan.state,
            changed_paths: Vec::new(),
            next_step: plan.next_step,
        });
    }
    match prepare_apply(project, manifest, &plan, accept_managed_block)? {
        Preparation::Review { next_step } => Ok(BridgePlanResult {
            state: ActivationState::NeedsUserReview,
            changed_paths: Vec::new(),
            next_step,
        }),
        Preparation::Ready { files, .. } => Ok(BridgePlanResult {
            state: applied_state(&plan.harness_id, plan.state),
            changed_paths: files
                .into_iter()
                .filter(|file| file.before != file.after)
                .map(|file| file.relative)
                .collect(),
            next_step: plan.next_step,
        }),
    }
}

/// Removes only hash-matching complete files and exact managed blocks recorded
/// for one harness. Canonical store files, receipts, and other harness owners
/// are never targets.
pub fn deactivate_bridge_plan(
    project: &ActivationProject,
    manifest: &mut ActivationManifest,
    harness_id: &str,
) -> Result<BridgePlanResult, String> {
    validate_project(project)?;
    validate_manifest(manifest)?;
    let Some(current) = manifest.harnesses.get(harness_id).cloned() else {
        return Ok(BridgePlanResult {
            state: ActivationState::ConfiguredAwaitingProof,
            changed_paths: Vec::new(),
            next_step: "No manifest-recorded Tree Ring bridge material was present.".to_string(),
        });
    };

    let mut files = Vec::new();
    let mut retained_files = Vec::new();
    for owned in &current.owned_files {
        let relative = PathBuf::from(&owned.path);
        ensure_local_target(project, &relative)?;
        let before = read_optional(&project.project_root.join(&relative))?;
        let other_owner = manifest.harnesses.iter().any(|(other_id, activation)| {
            other_id != harness_id
                && activation
                    .owned_files
                    .iter()
                    .any(|other| other.path == owned.path && other.sha256 == owned.sha256)
        });
        match before {
            Some(bytes) if sha256(&bytes) == owned.sha256 && !other_owner => {
                files.push(PreparedFile {
                    relative,
                    before: Some(bytes),
                    after: None,
                });
            }
            Some(bytes) if sha256(&bytes) == owned.sha256 && other_owner => {}
            None => {}
            _ => retained_files.push(owned.clone()),
        }
    }

    let mut retained_blocks = Vec::new();
    for owned in &current.managed_blocks {
        let relative = PathBuf::from(&owned.path);
        ensure_local_target(project, &relative)?;
        let before = read_optional(&project.project_root.join(&relative))?;
        let Some(before_bytes) = before else {
            continue;
        };
        let removal = if relative == Path::new("AGENTS.md") {
            remove_markdown_block(&before_bytes, &owned.block_id, &owned.sha256)?
        } else if relative == Path::new(".claude/settings.json") {
            remove_claude_handler(&before_bytes, &owned.sha256)?
        } else {
            return Err(format!(
                "unsupported managed bridge target in manifest: {}",
                relative.display()
            ));
        };
        match removal {
            Some(after) => files.push(PreparedFile {
                relative,
                before: Some(before_bytes),
                after: Some(after),
            }),
            None => retained_blocks.push(owned.clone()),
        }
    }

    let mut next_manifest = manifest.clone();
    let next = next_manifest
        .harnesses
        .get_mut(harness_id)
        .expect("cloned manifest retained harness");
    next.state = ActivationState::ConfiguredAwaitingProof;
    next.bridge_path = None;
    next.owned_files = retained_files;
    next.managed_blocks = retained_blocks;
    if let Some(first) = next
        .owned_files
        .first()
        .map(|owned| owned.path.clone())
        .or_else(|| next.managed_blocks.first().map(|owned| owned.path.clone()))
    {
        next.bridge_path = Some(first);
    }

    let changed_paths = files
        .iter()
        .filter(|file| file.before != file.after)
        .map(|file| file.relative.clone())
        .collect::<Vec<_>>();
    commit_files_and_manifest(project, manifest, next_manifest, &files)?;
    let needs_review = !next_manifest_ownership_empty(manifest, harness_id);
    Ok(BridgePlanResult {
        state: if needs_review {
            ActivationState::NeedsUserReview
        } else {
            ActivationState::ConfiguredAwaitingProof
        },
        changed_paths,
        next_step: if needs_review {
            "Tree Ring preserved changed or shared bridge material for explicit review.".to_string()
        } else {
            "Tree Ring-owned bridge material was removed; store guidance and receipts were retained."
                .to_string()
        },
    })
}

fn prepare_apply(
    project: &ActivationProject,
    manifest: &ActivationManifest,
    plan: &AdapterPlan,
    accept_managed_block: bool,
) -> Result<Preparation, String> {
    let existing = manifest.harnesses.get(&plan.harness_id);
    let mut files = Vec::new();
    let mut owned_files = existing
        .map(|activation| activation.owned_files.clone())
        .unwrap_or_default();
    let mut managed_blocks = existing
        .map(|activation| activation.managed_blocks.clone())
        .unwrap_or_default();

    for write in &plan.writes {
        let prepared = match write {
            PlannedWrite::BridgeWrite(write) => {
                let desired = render_complete_file(&plan.harness_id, &write.path, manifest)?;
                prepare_complete_file(
                    project,
                    manifest,
                    &plan.harness_id,
                    &write.path,
                    desired,
                    &mut owned_files,
                )?
            }
            PlannedWrite::ManagedBlockUpdate(write) if write.path == Path::new("AGENTS.md") => {
                prepare_markdown_file(
                    project,
                    &plan.harness_id,
                    write,
                    accept_managed_block,
                    &mut owned_files,
                    &mut managed_blocks,
                )?
            }
            PlannedWrite::ManagedBlockUpdate(write)
                if write.path == Path::new(".claude/settings.json") =>
            {
                prepare_claude_settings(project, write, &mut owned_files, &mut managed_blocks)?
            }
            PlannedWrite::ManagedBlockUpdate(write) => {
                return Err(format!(
                    "unsupported managed bridge target: {}",
                    write.path.display()
                ));
            }
        };
        let Some(prepared) = prepared else {
            return Ok(Preparation::Review {
                next_step: review_step(&plan.harness_id),
            });
        };
        files.push(prepared);
    }

    owned_files.sort_by(|left, right| left.path.cmp(&right.path));
    owned_files.dedup_by(|left, right| left.path == right.path);
    managed_blocks.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.block_id.cmp(&right.block_id))
    });
    managed_blocks
        .dedup_by(|left, right| left.path == right.path && left.block_id == right.block_id);
    let bridge_path = owned_files
        .first()
        .map(|owned| owned.path.clone())
        .or_else(|| managed_blocks.first().map(|owned| owned.path.clone()));
    Ok(Preparation::Ready {
        files,
        activation: HarnessActivation {
            state: applied_state(&plan.harness_id, plan.state),
            adapter_capability: capability_for(&plan.harness_id)?,
            bridge_path,
            owned_files,
            managed_blocks,
        },
    })
}

fn prepare_complete_file(
    project: &ActivationProject,
    manifest: &ActivationManifest,
    harness_id: &str,
    relative: &Path,
    desired: Vec<u8>,
    owned_files: &mut Vec<OwnedBridgeFile>,
) -> Result<Option<PreparedFile>, String> {
    ensure_local_target(project, relative)?;
    let before = read_optional(&project.project_root.join(relative))?;
    let path = relative_string(relative)?;
    let desired_hash = sha256(&desired);
    let own = owned_files.iter().find(|owned| owned.path == path).cloned();
    let other_owners = manifest
        .harnesses
        .iter()
        .filter(|(other_id, _)| other_id.as_str() != harness_id)
        .filter_map(|(_, activation)| {
            activation
                .owned_files
                .iter()
                .find(|owned| owned.path == path)
        })
        .collect::<Vec<_>>();

    let permitted = match (&before, own.as_ref()) {
        (None, None) => true,
        (None, Some(_)) => false,
        (Some(bytes), Some(owned)) => {
            sha256(bytes) == owned.sha256
                && (other_owners.is_empty()
                    || other_owners
                        .iter()
                        .all(|other| other.sha256 == owned.sha256 && desired_hash == owned.sha256))
        }
        (Some(bytes), None) => {
            !other_owners.is_empty()
                && other_owners
                    .iter()
                    .all(|other| sha256(bytes) == other.sha256 && desired_hash == other.sha256)
        }
    };
    if !permitted {
        return Ok(None);
    }
    upsert_owned_file(owned_files, path, desired_hash);
    Ok(Some(PreparedFile {
        relative: relative.to_path_buf(),
        before,
        after: Some(desired),
    }))
}

fn prepare_markdown_file(
    project: &ActivationProject,
    harness_id: &str,
    write: &ManagedBlockUpdate,
    accept_managed_block: bool,
    owned_files: &mut Vec<OwnedBridgeFile>,
    managed_blocks: &mut Vec<OwnedManagedBlock>,
) -> Result<Option<PreparedFile>, String> {
    ensure_local_target(project, &write.path)?;
    let before = read_optional(&project.project_root.join(&write.path))?;
    let path = relative_string(&write.path)?;
    let block = markdown_block(harness_id);
    if before.is_none() {
        let bytes = block.into_bytes();
        upsert_owned_file(owned_files, path, sha256(&bytes));
        return Ok(Some(PreparedFile {
            relative: write.path.clone(),
            before: None,
            after: Some(bytes),
        }));
    }
    if let Some(owned) = owned_files.iter().find(|owned| owned.path == path) {
        let bytes = before.as_ref().expect("checked above");
        if sha256(bytes) != owned.sha256 {
            return Ok(None);
        }
        let desired = block.into_bytes();
        upsert_owned_file(owned_files, path, sha256(&desired));
        return Ok(Some(PreparedFile {
            relative: write.path.clone(),
            before,
            after: Some(desired),
        }));
    }

    let before_bytes = before.as_ref().expect("checked above");
    let before_text = std::str::from_utf8(before_bytes).map_err(|_| {
        format!(
            "managed Markdown bridge is not UTF-8: {}",
            write.path.display()
        )
    })?;
    let markers = locate_markdown_block(before_text, harness_id)?;
    let after = if let Some((start, end)) = markers {
        let existing_block = &before_text[start..end];
        let recorded = managed_blocks
            .iter()
            .find(|owned| owned.path == path && owned.block_id == write.block_id);
        if recorded.is_some_and(|owned| sha256(existing_block.as_bytes()) != owned.sha256) {
            return Ok(None);
        }
        replace_range(before_text, start, end, &block).into_bytes()
    } else {
        if !accept_managed_block {
            return Ok(None);
        }
        append_markdown_block(before_text, &block).into_bytes()
    };
    upsert_managed_block(
        managed_blocks,
        path,
        write.block_id.clone(),
        sha256(block.as_bytes()),
    );
    Ok(Some(PreparedFile {
        relative: write.path.clone(),
        before,
        after: Some(after),
    }))
}

fn prepare_claude_settings(
    project: &ActivationProject,
    write: &ManagedBlockUpdate,
    owned_files: &mut Vec<OwnedBridgeFile>,
    managed_blocks: &mut Vec<OwnedManagedBlock>,
) -> Result<Option<PreparedFile>, String> {
    ensure_local_target(project, &write.path)?;
    let before = read_optional(&project.project_root.join(&write.path))?;
    let path = relative_string(&write.path)?;
    let handler = claude_handler();
    let handler_hash = sha256(&serde_json::to_vec(&handler).map_err(json_error)?);
    if before.is_none() {
        let mut root = Map::new();
        insert_claude_handler(&mut root, handler)?;
        let bytes = pretty_json(&Value::Object(root))?;
        upsert_owned_file(owned_files, path, sha256(&bytes));
        return Ok(Some(PreparedFile {
            relative: write.path.clone(),
            before: None,
            after: Some(bytes),
        }));
    }
    if let Some(owned) = owned_files.iter().find(|owned| owned.path == path) {
        let bytes = before.as_ref().expect("checked above");
        if sha256(bytes) != owned.sha256 {
            return Ok(None);
        }
        let mut root = parse_json_object(bytes)?;
        let state = match inspect_claude_handler(&root) {
            Ok(state) => state,
            Err(_) => return Ok(None),
        };
        match state {
            ClaudeHandlerState::Exact => {}
            ClaudeHandlerState::Absent => {
                if insert_claude_handler(&mut root, handler).is_err() {
                    return Ok(None);
                }
            }
            ClaudeHandlerState::Conflict => return Ok(None),
        }
        let desired = pretty_json(&Value::Object(root))?;
        upsert_owned_file(owned_files, path, sha256(&desired));
        return Ok(Some(PreparedFile {
            relative: write.path.clone(),
            before,
            after: Some(desired),
        }));
    }

    let before_bytes = before.as_ref().expect("checked above");
    let mut root = match parse_json_object(before_bytes) {
        Ok(root) => root,
        Err(_) => return Ok(None),
    };
    let state = match inspect_claude_handler(&root) {
        Ok(state) => state,
        Err(_) => return Ok(None),
    };
    match state {
        ClaudeHandlerState::Conflict => return Ok(None),
        ClaudeHandlerState::Exact => {}
        ClaudeHandlerState::Absent => {
            if insert_claude_handler(&mut root, handler).is_err() {
                return Ok(None);
            }
        }
    }
    let after = pretty_json(&Value::Object(root))?;
    upsert_managed_block(managed_blocks, path, write.block_id.clone(), handler_hash);
    Ok(Some(PreparedFile {
        relative: write.path.clone(),
        before,
        after: Some(after),
    }))
}

fn render_complete_file(
    harness_id: &str,
    path: &Path,
    manifest: &ActivationManifest,
) -> Result<Vec<u8>, String> {
    match (harness_id, path.to_str()) {
        ("codex" | "pi", Some(".agents/skills/tree-ring-memory/SKILL.md"))
        | ("claude-code", Some(".claude/skills/tree-ring-memory/SKILL.md")) => {
            Ok(SKILL_BRIDGE.as_bytes().to_vec())
        }
        ("pi", Some(".pi/extensions/tree-ring-memory.ts")) => Ok(PI_EXTENSION.as_bytes().to_vec()),
        ("agent-zero", Some(".tree-ring/activation/agent-zero.json")) => {
            let binding = json!({
                "protocol_version": ACTIVATION_PROTOCOL_VERSION,
                "store_id": manifest.store_id,
                "project_root_fingerprint": manifest.project_root_fingerprint,
                "memory_root": ".tree-ring",
                "command_protocol": {
                    "command": "tree-ring",
                    "arguments": [
                        "--root", ".tree-ring", "integrations", "preflight",
                        "--harness", "agent-zero", "--input-json-stdin",
                        "--context-format", "json"
                    ],
                    "stdin": "json",
                    "stdout": "json"
                }
            });
            pretty_json(&binding)
        }
        _ => Err(format!(
            "unexpected complete bridge target for {harness_id}: {}",
            path.display()
        )),
    }
}

fn validate_plan(plan: &AdapterPlan) -> Result<(), String> {
    let expected = if matches!(
        plan.state,
        ActivationState::NeedsPlugin | ActivationState::Unsupported
    ) {
        Vec::new()
    } else {
        match plan.harness_id.as_str() {
            "codex" => vec![
                ("file", ".agents/skills/tree-ring-memory/SKILL.md", ""),
                ("block", "AGENTS.md", "codex"),
            ],
            "claude-code" => vec![
                ("file", ".claude/skills/tree-ring-memory/SKILL.md", ""),
                ("block", ".claude/settings.json", "claude-code"),
            ],
            "pi" => vec![
                ("file", ".agents/skills/tree-ring-memory/SKILL.md", ""),
                ("file", ".pi/extensions/tree-ring-memory.ts", ""),
            ],
            "agent-zero" => vec![("file", ".tree-ring/activation/agent-zero.json", "")],
            "hermes" | "opencode" | "goose" => Vec::new(),
            other => return Err(format!("unknown harness adapter: {other}")),
        }
    };
    let actual = plan
        .writes
        .iter()
        .map(|write| match write {
            PlannedWrite::BridgeWrite(write) => {
                validate_relative_path_buf(&write.path)?;
                Ok(("file", relative_str(&write.path)?, ""))
            }
            PlannedWrite::ManagedBlockUpdate(write) => {
                validate_relative_path_buf(&write.path)?;
                Ok(("block", relative_str(&write.path)?, write.block_id.as_str()))
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    if actual != expected {
        return Err(format!("unexpected bridge writes for {}", plan.harness_id));
    }
    Ok(())
}

fn validate_project(project: &ActivationProject) -> Result<(), String> {
    if project.memory_root != project.project_root.join(".tree-ring") {
        return Err(
            "activation project memory root must be the project-local .tree-ring".to_string(),
        );
    }
    ensure_local_target(project, Path::new(".tree-ring/activation.json"))
}

fn ensure_local_target(project: &ActivationProject, relative: &Path) -> Result<(), String> {
    validate_relative_path_buf(relative)?;
    let mut current = project.project_root.clone();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            return Err("bridge path must be normalized and project-relative".to_string());
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "bridge path traverses a symlink: {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(&current, error)),
        }
    }
    Ok(())
}

fn validate_relative_path_buf(path: &Path) -> Result<(), String> {
    let value = path
        .to_str()
        .ok_or_else(|| "bridge path must be valid UTF-8".to_string())?;
    validate_project_relative_path(value)
}

fn relative_str(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| "bridge path must be valid UTF-8".to_string())
}

fn relative_string(path: &Path) -> Result<String, String> {
    Ok(relative_str(path)?.to_string())
}

fn markdown_block(harness_id: &str) -> String {
    format!(
        "<!-- tree-ring:begin {harness_id} v1 -->\n{CODEX_BLOCK_BODY}<!-- tree-ring:end {harness_id} -->\n"
    )
}

fn locate_markdown_block(
    content: &str,
    harness_id: &str,
) -> Result<Option<(usize, usize)>, String> {
    let begin = format!("<!-- tree-ring:begin {harness_id} v1 -->");
    let end = format!("<!-- tree-ring:end {harness_id} -->");
    let begins = content.match_indices(&begin).collect::<Vec<_>>();
    let ends = content.match_indices(&end).collect::<Vec<_>>();
    match (begins.as_slice(), ends.as_slice()) {
        ([], []) => Ok(None),
        ([(start, _)], [(end_start, _)]) if start < end_start => {
            let mut end_offset = end_start + end.len();
            if content[end_offset..].starts_with('\n') {
                end_offset += 1;
            }
            Ok(Some((*start, end_offset)))
        }
        _ => Err(format!(
            "conflicting or incomplete Tree Ring managed markers for {harness_id}"
        )),
    }
}

fn append_markdown_block(content: &str, block: &str) -> String {
    if content.is_empty() {
        return block.to_string();
    }
    if content.ends_with("\n\n") {
        format!("{content}{block}")
    } else if content.ends_with('\n') {
        format!("{content}\n{block}")
    } else {
        format!("{content}\n\n{block}")
    }
}

fn replace_range(content: &str, start: usize, end: usize, replacement: &str) -> String {
    format!("{}{}{}", &content[..start], replacement, &content[end..])
}

fn remove_markdown_block(
    bytes: &[u8],
    block_id: &str,
    expected_hash: &str,
) -> Result<Option<Vec<u8>>, String> {
    let content = match std::str::from_utf8(bytes) {
        Ok(content) => content,
        Err(_) => return Ok(None),
    };
    let Some((start, end)) = locate_markdown_block(content, block_id)? else {
        return Ok(None);
    };
    if sha256(&content.as_bytes()[start..end]) != expected_hash {
        return Ok(None);
    }
    let mut result = replace_range(content, start, end, "");
    while result.ends_with("\n\n") {
        result.pop();
    }
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    Ok(Some(result.into_bytes()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClaudeHandlerState {
    Absent,
    Exact,
    Conflict,
}

fn claude_handler() -> Value {
    json!({
        "type": "command",
        "command": CLAUDE_COMMAND,
        "description": CLAUDE_DESCRIPTION
    })
}

fn inspect_claude_handler(root: &Map<String, Value>) -> Result<ClaudeHandlerState, String> {
    let expected = claude_handler();
    let mut exact = 0usize;
    let mut conflict = false;
    inspect_tree_ring_values(
        &Value::Object(root.clone()),
        &expected,
        &mut exact,
        &mut conflict,
    );
    if conflict || exact > 1 {
        Ok(ClaudeHandlerState::Conflict)
    } else if exact == 1 {
        Ok(ClaudeHandlerState::Exact)
    } else {
        validate_claude_hook_shape(root)?;
        Ok(ClaudeHandlerState::Absent)
    }
}

fn inspect_tree_ring_values(
    value: &Value,
    expected: &Value,
    exact: &mut usize,
    conflict: &mut bool,
) {
    match value {
        Value::Object(object) => {
            let description = object.get("description").and_then(Value::as_str);
            let command = object.get("command").and_then(Value::as_str);
            let claims_description = description == Some(CLAUDE_DESCRIPTION);
            let claims_command = command.is_some_and(|command| {
                command.contains("tree-ring")
                    && command.contains("integrations preflight")
                    && command.contains("--harness claude-code")
            });
            if claims_description || claims_command {
                if Value::Object(object.clone()) == *expected {
                    *exact += 1;
                } else {
                    *conflict = true;
                }
            }
            for child in object.values() {
                inspect_tree_ring_values(child, expected, exact, conflict);
            }
        }
        Value::Array(values) => {
            for child in values {
                inspect_tree_ring_values(child, expected, exact, conflict);
            }
        }
        _ => {}
    }
}

fn validate_claude_hook_shape(root: &Map<String, Value>) -> Result<(), String> {
    let Some(hooks) = root.get("hooks") else {
        return Ok(());
    };
    let hooks = hooks
        .as_object()
        .ok_or_else(|| "Claude settings hooks must be an object".to_string())?;
    if let Some(session_start) = hooks.get("SessionStart") {
        session_start
            .as_array()
            .ok_or_else(|| "Claude SessionStart hooks must be an array".to_string())?;
    }
    Ok(())
}

fn insert_claude_handler(root: &mut Map<String, Value>, handler: Value) -> Result<(), String> {
    if !root.contains_key("hooks") {
        root.insert("hooks".to_string(), Value::Object(Map::new()));
    }
    let hooks = root
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "Claude settings hooks must be an object".to_string())?;
    if !hooks.contains_key("SessionStart") {
        hooks.insert("SessionStart".to_string(), Value::Array(Vec::new()));
    }
    let session_start = hooks
        .get_mut("SessionStart")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "Claude SessionStart hooks must be an array".to_string())?;
    session_start.push(json!({"matcher": "", "hooks": [handler]}));
    Ok(())
}

fn remove_claude_handler(bytes: &[u8], expected_hash: &str) -> Result<Option<Vec<u8>>, String> {
    let mut root = match parse_json_object(bytes) {
        Ok(root) => root,
        Err(_) => return Ok(None),
    };
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return Ok(None);
    };
    let Some(entries) = hooks.get_mut("SessionStart").and_then(Value::as_array_mut) else {
        return Ok(None);
    };
    let mut removed = 0usize;
    for entry in entries.iter_mut() {
        let Some(handlers) = entry.get_mut("hooks").and_then(Value::as_array_mut) else {
            continue;
        };
        handlers.retain(|handler| {
            let matches = handler.get("description").and_then(Value::as_str)
                == Some(CLAUDE_DESCRIPTION)
                && serde_json::to_vec(handler)
                    .map(|bytes| sha256(&bytes) == expected_hash)
                    .unwrap_or(false);
            if matches {
                removed += 1;
            }
            !matches
        });
    }
    if removed != 1 {
        return Ok(None);
    }
    entries.retain(|entry| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .is_none_or(|handlers| !handlers.is_empty())
    });
    if entries.is_empty() {
        hooks.remove("SessionStart");
    }
    if hooks.is_empty() {
        root.remove("hooks");
    }
    pretty_json(&Value::Object(root)).map(Some)
}

fn parse_json_object(bytes: &[u8]) -> Result<Map<String, Value>, String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("invalid Claude settings JSON: {error}"))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| "Claude settings root must be an object".to_string())
}

fn pretty_json(value: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(json_error)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn json_error(error: serde_json::Error) -> String {
    format!("failed to render bridge JSON: {error}")
}

fn upsert_owned_file(owned: &mut Vec<OwnedBridgeFile>, path: String, digest: String) {
    if let Some(existing) = owned.iter_mut().find(|owned| owned.path == path) {
        existing.sha256 = digest;
    } else {
        owned.push(OwnedBridgeFile {
            path,
            sha256: digest,
        });
    }
}

fn upsert_managed_block(
    owned: &mut Vec<OwnedManagedBlock>,
    path: String,
    block_id: String,
    digest: String,
) {
    if let Some(existing) = owned
        .iter_mut()
        .find(|owned| owned.path == path && owned.block_id == block_id)
    {
        existing.sha256 = digest;
    } else {
        owned.push(OwnedManagedBlock {
            path,
            block_id,
            sha256: digest,
        });
    }
}

fn commit_files_and_manifest(
    project: &ActivationProject,
    manifest: &mut ActivationManifest,
    next_manifest: ActivationManifest,
    files: &[PreparedFile],
) -> Result<(), String> {
    let original_manifest = manifest.clone();
    let mut applied = Vec::new();
    for file in files.iter().filter(|file| file.before != file.after) {
        if let Err(error) = commit_prepared_file(project, file) {
            rollback_files(project, &applied);
            return Err(error);
        }
        applied.push(file.clone());
    }
    if let Err(error) = save_manifest(&project.memory_root, &next_manifest) {
        rollback_files(project, &applied);
        *manifest = original_manifest;
        return Err(error);
    }
    *manifest = next_manifest;
    Ok(())
}

fn commit_prepared_file(project: &ActivationProject, file: &PreparedFile) -> Result<(), String> {
    ensure_local_target(project, &file.relative)?;
    let path = project.project_root.join(&file.relative);
    let current = read_optional(&path)?;
    if current != file.before {
        return Err(format!(
            "bridge target changed after validation: {}",
            file.relative.display()
        ));
    }
    match &file.after {
        Some(bytes) => atomic_write_bytes(&path, bytes, file.before.is_none()),
        None => match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io_error(&path, error)),
        },
    }
}

fn rollback_files(project: &ActivationProject, files: &[PreparedFile]) {
    for file in files.iter().rev() {
        let path = project.project_root.join(&file.relative);
        match &file.before {
            Some(bytes) => {
                let _ = atomic_write_bytes(&path, bytes, false);
            }
            None => {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

fn atomic_write_bytes(path: &Path, bytes: &[u8], create_only: bool) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("bridge output has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("bridge output has no UTF-8 file name: {}", path.display()))?;
    let temp = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(|error| io_error(&temp, error))?;
        file.write_all(bytes)
            .map_err(|error| io_error(&temp, error))?;
        file.sync_all().map_err(|error| io_error(&temp, error))?;
        drop(file);
        if create_only {
            fs::hard_link(&temp, path).map_err(|error| io_error(path, error))?;
            fs::remove_file(&temp).map_err(|error| io_error(&temp, error))
        } else {
            fs::rename(&temp, path).map_err(|error| io_error(path, error))
        }
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(path, error)),
    }
}

fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn capability_for(harness_id: &str) -> Result<AdapterCapability, String> {
    match harness_id {
        "codex" => Ok(AdapterCapability::WrapperPreflight),
        "claude-code" | "pi" | "agent-zero" => Ok(AdapterCapability::NativePreflight),
        "hermes" | "opencode" | "goose" => Ok(AdapterCapability::GuidanceOnly),
        other => Err(format!("unknown harness adapter: {other}")),
    }
}

fn applied_state(harness_id: &str, planned: ActivationState) -> ActivationState {
    if harness_id == "pi" {
        ActivationState::NeedsTrust
    } else {
        planned
    }
}

fn review_step(harness_id: &str) -> String {
    if harness_id == "codex" {
        CODEX_RETRY.to_string()
    } else {
        format!("Review the existing {harness_id} bridge configuration before retrying activation.")
    }
}

fn next_manifest_ownership_empty(manifest: &ActivationManifest, harness_id: &str) -> bool {
    manifest.harnesses.get(harness_id).is_none_or(|activation| {
        activation.owned_files.is_empty() && activation.managed_blocks.is_empty()
    })
}

fn io_error(path: &Path, error: std::io::Error) -> String {
    format!("{}: {error}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activation::{
        adapters::{plan_activation, ActivationProject, AdapterPlan, BridgeWrite, PlannedWrite},
        manifest::ActivationManifest,
        ActivationState, ACTIVATION_PROTOCOL_VERSION, ACTIVATION_SCHEMA_VERSION,
    };
    use serde_json::{json, Value};
    use std::{collections::BTreeMap, fs, path::Path};
    use tempfile::TempDir;

    fn fixture() -> (TempDir, ActivationProject, ActivationManifest) {
        let temp = tempfile::tempdir().unwrap();
        let project = ActivationProject::from_project_root(temp.path());
        fs::create_dir_all(&project.memory_root).unwrap();
        fs::write(
            project.memory_root.join("AGENTS.md"),
            "# Canonical guidance\n",
        )
        .unwrap();
        fs::write(project.memory_root.join("SKILL.md"), "# Canonical skill\n").unwrap();
        fs::write(project.memory_root.join("CLI.md"), "# Canonical CLI\n").unwrap();
        let manifest = ActivationManifest {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            protocol_version: ACTIVATION_PROTOCOL_VERSION,
            store_id: "store-test".to_string(),
            project_root_fingerprint: "a".repeat(64),
            cli_version: "0.14.0".to_string(),
            harnesses: BTreeMap::new(),
        };
        (temp, project, manifest)
    }

    fn read(path: impl AsRef<Path>) -> String {
        fs::read_to_string(path).unwrap()
    }

    fn write(path: impl AsRef<Path>, content: &str) {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn plan(harness: &str, project: &ActivationProject) -> AdapterPlan {
        plan_activation(harness, project).unwrap()
    }

    #[test]
    fn unmanaged_agents_file_requires_explicit_review_without_partial_writes() {
        let (_temp, project, mut manifest) = fixture();
        write(project.project_root.join("AGENTS.md"), "# Team contract\n");

        let result =
            apply_bridge_plan(&project, &mut manifest, plan("codex", &project), false).unwrap();

        assert_eq!(result.state, ActivationState::NeedsUserReview);
        assert_eq!(
            read(project.project_root.join("AGENTS.md")),
            "# Team contract\n"
        );
        assert!(!project
            .project_root
            .join(".agents/skills/tree-ring-memory/SKILL.md")
            .exists());
        assert_eq!(
            result.next_step,
            "tree-ring integrations activate --harness codex --accept-managed-block"
        );
        assert!(manifest.harnesses.is_empty());
    }

    #[test]
    fn absent_agents_file_becomes_hash_recorded_owned_file_and_retries_idempotently() {
        let (_temp, project, mut manifest) = fixture();
        let codex = plan("codex", &project);

        let first = apply_bridge_plan(&project, &mut manifest, codex.clone(), false).unwrap();
        let agents = read(project.project_root.join("AGENTS.md"));
        let skill = read(
            project
                .project_root
                .join(".agents/skills/tree-ring-memory/SKILL.md"),
        );
        let serialized = serde_json::to_value(&manifest).unwrap();
        let second = apply_bridge_plan(&project, &mut manifest, codex, false).unwrap();

        assert_eq!(first.state, ActivationState::ConfiguredAwaitingProof);
        assert_eq!(second.state, ActivationState::ConfiguredAwaitingProof);
        assert!(second.changed_paths.is_empty());
        assert_eq!(
            agents.matches("<!-- tree-ring:begin codex v1 -->").count(),
            1
        );
        assert!(skill.contains(".tree-ring/SKILL.md"));
        assert_eq!(
            serialized["harnesses"]["codex"]["owned_files"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        for owned in serialized["harnesses"]["codex"]["owned_files"]
            .as_array()
            .unwrap()
        {
            assert_eq!(owned["sha256"].as_str().unwrap().len(), 64);
        }
    }

    #[test]
    fn accepted_agents_block_preserves_unrelated_text_and_records_exact_block_id() {
        let (_temp, project, mut manifest) = fixture();
        write(project.project_root.join("AGENTS.md"), "# Team contract\n");

        apply_bridge_plan(&project, &mut manifest, plan("codex", &project), true).unwrap();
        let agents = read(project.project_root.join("AGENTS.md"));
        let serialized = serde_json::to_value(&manifest).unwrap();
        apply_bridge_plan(&project, &mut manifest, plan("codex", &project), false).unwrap();

        assert!(agents.starts_with("# Team contract\n"));
        assert_eq!(
            agents.matches("<!-- tree-ring:begin codex v1 -->").count(),
            1
        );
        assert_eq!(agents.matches("<!-- tree-ring:end codex -->").count(), 1);
        assert_eq!(
            serialized["harnesses"]["codex"]["managed_blocks"][0]["block_id"],
            "codex"
        );
    }

    #[test]
    fn changed_owned_file_is_never_overwritten_on_retry() {
        let (_temp, project, mut manifest) = fixture();
        apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap();
        let extension = project
            .project_root
            .join(".pi/extensions/tree-ring-memory.ts");
        write(&extension, "// owner changed this file\n");

        let result =
            apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap();

        assert_eq!(result.state, ActivationState::NeedsUserReview);
        assert_eq!(read(extension), "// owner changed this file\n");
    }

    #[test]
    fn claude_settings_merge_preserves_every_unrelated_json_value_exactly() {
        let (_temp, project, mut manifest) = fixture();
        let settings_path = project.project_root.join(".claude/settings.json");
        let original = json!({
            "permissions": {"allow": ["Read", "Bash(git status)"]},
            "hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "guard"}]}]},
            "custom": [1, {"nested": true}]
        });
        write(
            &settings_path,
            &serde_json::to_string_pretty(&original).unwrap(),
        );

        apply_bridge_plan(
            &project,
            &mut manifest,
            plan("claude-code", &project),
            false,
        )
        .unwrap();
        let merged: Value = serde_json::from_str(&read(&settings_path)).unwrap();
        let first_bytes = read(&settings_path);
        let retry = apply_bridge_plan(
            &project,
            &mut manifest,
            plan("claude-code", &project),
            false,
        )
        .unwrap();

        assert_eq!(merged["permissions"], original["permissions"]);
        assert_eq!(
            merged["hooks"]["PreToolUse"],
            original["hooks"]["PreToolUse"]
        );
        assert_eq!(merged["custom"], original["custom"]);
        let session_hooks = merged["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(session_hooks.len(), 1);
        assert_eq!(
            session_hooks[0]["hooks"][0]["description"],
            "Tree Ring Memory managed preflight v1"
        );
        assert_eq!(
            session_hooks[0]["hooks"][0]["command"],
            "tree-ring --root .tree-ring integrations preflight --harness claude-code --input-json-stdin --context-format claude-session-start"
        );
        assert!(retry.changed_paths.is_empty());
        assert_eq!(read(settings_path), first_bytes);
    }

    #[test]
    fn invalid_or_conflicting_claude_settings_require_review_without_any_write() {
        for contents in [
            "not json".to_string(),
            "[]".to_string(),
            serde_json::to_string(&json!({"hooks": 5})).unwrap(),
            serde_json::to_string(&json!({
                "hooks": {"SessionStart": [{"hooks": [{
                    "type": "command",
                    "command": "some-other-command",
                    "description": "Tree Ring Memory managed preflight v1"
                }]}]}
            }))
            .unwrap(),
        ] {
            let (_temp, project, mut manifest) = fixture();
            let settings_path = project.project_root.join(".claude/settings.json");
            write(&settings_path, &contents);

            let result = apply_bridge_plan(
                &project,
                &mut manifest,
                plan("claude-code", &project),
                false,
            )
            .unwrap();

            assert_eq!(result.state, ActivationState::NeedsUserReview);
            assert_eq!(read(settings_path), contents);
            assert!(!project
                .project_root
                .join(".claude/skills/tree-ring-memory/SKILL.md")
                .exists());
            assert!(manifest.harnesses.is_empty());
        }
    }

    #[test]
    fn preview_is_a_zero_write_dry_run() {
        let (_temp, project, manifest) = fixture();
        let before = serde_json::to_value(&manifest).unwrap();
        let pi = plan("pi", &project);

        assert_eq!(pi.state, ActivationState::NeedsTrust);
        let result = preview_bridge_plan(&project, &manifest, pi, false).unwrap();

        assert_eq!(result.state, ActivationState::NeedsTrust);
        assert_eq!(result.changed_paths.len(), 2);
        assert_eq!(serde_json::to_value(&manifest).unwrap(), before);
        assert!(!project.project_root.join(".agents").exists());
        assert!(!project.project_root.join(".pi").exists());
    }

    #[test]
    fn pi_extension_uses_stdin_fallback_and_returns_non_display_context() {
        let (_temp, project, mut manifest) = fixture();

        let result =
            apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap();
        let source = read(
            project
                .project_root
                .join(".pi/extensions/tree-ring-memory.ts"),
        );

        assert_eq!(result.state, ActivationState::NeedsTrust);
        assert!(source.contains("pi.on(\"before_agent_start\", async (event, ctx) =>"));
        assert!(source.contains("task_hint: event.prompt"));
        assert!(source.contains("child.stdin.end(input)"));
        assert!(source.contains("task_hint: \"project startup constraints\""));
        assert!(source.contains("customType: \"tree-ring-preflight\""));
        assert!(source.contains("content: response.context"));
        assert!(source.contains("display: false"));
        assert!(!source.contains("npm install"));
        assert!(!source.contains("detached: true"));
    }

    #[test]
    fn agent_zero_requires_verified_plugin_plan_and_writes_only_protocol_binding() {
        let (_temp, project, mut manifest) = fixture();
        let blocked = plan("agent-zero", &project);

        let blocked_result = apply_bridge_plan(&project, &mut manifest, blocked, false).unwrap();
        assert_eq!(blocked_result.state, ActivationState::NeedsPlugin);
        assert!(blocked_result.changed_paths.is_empty());
        assert!(!project
            .project_root
            .join(".tree-ring/activation/agent-zero.json")
            .exists());

        let verified = AdapterPlan {
            harness_id: "agent-zero".to_string(),
            state: ActivationState::ConfiguredAwaitingProof,
            writes: vec![PlannedWrite::BridgeWrite(BridgeWrite {
                path: ".tree-ring/activation/agent-zero.json".into(),
            })],
            next_step: "verified separate plugin".to_string(),
        };
        apply_bridge_plan(&project, &mut manifest, verified, false).unwrap();
        let binding: Value = serde_json::from_str(&read(
            project
                .project_root
                .join(".tree-ring/activation/agent-zero.json"),
        ))
        .unwrap();

        assert_eq!(binding["protocol_version"], ACTIVATION_PROTOCOL_VERSION);
        assert_eq!(binding["store_id"], "store-test");
        assert_eq!(binding["project_root_fingerprint"], "a".repeat(64));
        assert_eq!(binding["memory_root"], ".tree-ring");
        assert_eq!(binding["command_protocol"]["stdin"], "json");
        assert_eq!(binding["command_protocol"]["stdout"], "json");
        assert!(!project.project_root.join(".a0").exists());
    }

    #[test]
    fn deactivation_preserves_non_tree_ring_text_canonical_store_and_receipts() {
        let (_temp, project, mut manifest) = fixture();
        write(project.project_root.join("AGENTS.md"), "# Team contract\n");
        apply_bridge_plan(&project, &mut manifest, plan("codex", &project), true).unwrap();
        let receipt_path = project
            .memory_root
            .join("activation/receipts/codex/worker/receipt.json");
        write(&receipt_path, "{}\n");

        let result = deactivate_bridge_plan(&project, &mut manifest, "codex").unwrap();

        assert_eq!(result.state, ActivationState::ConfiguredAwaitingProof);
        assert_eq!(
            read(project.project_root.join("AGENTS.md")),
            "# Team contract\n"
        );
        assert!(receipt_path.exists());
        assert!(project.memory_root.join("AGENTS.md").exists());
        assert!(project.memory_root.join("SKILL.md").exists());
        assert!(project.memory_root.join("CLI.md").exists());
        assert!(project.memory_root.join("activation.json").exists());
        assert!(!project
            .project_root
            .join(".agents/skills/tree-ring-memory/SKILL.md")
            .exists());
        assert!(manifest.harnesses.contains_key("codex"));
    }

    #[test]
    fn deactivation_never_deletes_changed_owned_or_unrecorded_lookalike_files() {
        let (_source_temp, source_project, mut source_manifest) = fixture();
        apply_bridge_plan(
            &source_project,
            &mut source_manifest,
            plan("pi", &source_project),
            false,
        )
        .unwrap();
        let generated = read(
            source_project
                .project_root
                .join(".pi/extensions/tree-ring-memory.ts"),
        );

        let (_temp, project, mut manifest) = fixture();
        let extension = project
            .project_root
            .join(".pi/extensions/tree-ring-memory.ts");
        write(&extension, &generated);
        deactivate_bridge_plan(&project, &mut manifest, "pi").unwrap();
        assert_eq!(read(&extension), generated);

        let review =
            apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap();
        assert_eq!(review.state, ActivationState::NeedsUserReview);
        fs::remove_file(&extension).unwrap();
        apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap();
        write(&extension, "// user changed owned bridge\n");
        deactivate_bridge_plan(&project, &mut manifest, "pi").unwrap();
        assert_eq!(read(extension), "// user changed owned bridge\n");
    }

    #[test]
    fn shared_skill_file_survives_deactivation_of_one_recorded_owner() {
        let (_temp, project, mut manifest) = fixture();
        apply_bridge_plan(&project, &mut manifest, plan("codex", &project), false).unwrap();
        apply_bridge_plan(&project, &mut manifest, plan("pi", &project), false).unwrap();

        let result = deactivate_bridge_plan(&project, &mut manifest, "codex").unwrap();

        assert_eq!(result.state, ActivationState::ConfiguredAwaitingProof);
        assert!(project
            .project_root
            .join(".agents/skills/tree-ring-memory/SKILL.md")
            .exists());
        assert!(!project.project_root.join("AGENTS.md").exists());
        assert!(manifest.harnesses["codex"].owned_files.is_empty());
    }

    #[test]
    fn claude_deactivation_removes_only_the_exact_recorded_handler() {
        let (_temp, project, mut manifest) = fixture();
        let settings_path = project.project_root.join(".claude/settings.json");
        let original = json!({
            "permissions": {"allow": ["Read"]},
            "hooks": {"SessionStart": [{"matcher": "compact", "hooks": [{"type": "command", "command": "keep-me"}]}]}
        });
        write(
            &settings_path,
            &serde_json::to_string_pretty(&original).unwrap(),
        );
        apply_bridge_plan(
            &project,
            &mut manifest,
            plan("claude-code", &project),
            false,
        )
        .unwrap();

        let result = deactivate_bridge_plan(&project, &mut manifest, "claude-code").unwrap();
        let after: Value = serde_json::from_str(&read(settings_path)).unwrap();

        assert_eq!(result.state, ActivationState::ConfiguredAwaitingProof);
        assert_eq!(after, original);
        assert!(!project
            .project_root
            .join(".claude/skills/tree-ring-memory/SKILL.md")
            .exists());
    }

    #[test]
    fn unsafe_or_unexpected_plan_paths_are_rejected_before_writes() {
        let (_temp, project, mut manifest) = fixture();
        for path in ["../outside", "/tmp/outside", "AGENTS.md"] {
            let malicious = AdapterPlan {
                harness_id: "pi".to_string(),
                state: ActivationState::ConfiguredAwaitingProof,
                writes: vec![PlannedWrite::BridgeWrite(BridgeWrite { path: path.into() })],
                next_step: String::new(),
            };
            assert!(apply_bridge_plan(&project, &mut manifest, malicious, false).is_err());
        }
        assert!(!project.project_root.join(".agents").exists());
        assert!(!Path::new("/tmp/outside").exists());
    }

    #[cfg(unix)]
    #[test]
    fn project_local_manifest_never_follows_a_tree_ring_symlink() {
        use std::os::unix::fs::symlink;

        let project_temp = tempfile::tempdir().unwrap();
        let outside_temp = tempfile::tempdir().unwrap();
        symlink(outside_temp.path(), project_temp.path().join(".tree-ring")).unwrap();
        let project = ActivationProject::from_project_root(project_temp.path());
        let mut manifest = ActivationManifest {
            schema_version: ACTIVATION_SCHEMA_VERSION,
            protocol_version: ACTIVATION_PROTOCOL_VERSION,
            store_id: "store-test".to_string(),
            project_root_fingerprint: "a".repeat(64),
            cli_version: "0.14.0".to_string(),
            harnesses: BTreeMap::new(),
        };

        let error =
            apply_bridge_plan(&project, &mut manifest, plan("codex", &project), false).unwrap_err();

        assert!(error.contains("symlink"));
        assert!(!outside_temp.path().join("activation.json").exists());
        assert!(!project_temp.path().join("AGENTS.md").exists());
    }
}
