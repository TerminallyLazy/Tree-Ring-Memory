# Project-Local Harness Activation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `tree-ring init` safely activate detected project-local coding harnesses, inject a scoped recall before work where the harness supports it, and report `active` only after a verifiable receipt.

**Architecture:** Add a library-owned activation domain to the CLI crate. It owns the versioned manifest, privacy-safe receipts, adapter registry, and reversible bridge plans; the binary remains responsible for command parsing and human/JSON output. Claude Code and Pi receive native lifecycle bridges, Codex receives a project instruction and skill bridge whose agent-mediated preflight produces the receipt, and Agent Zero is represented by a protocol binding consumed only by its separate plugin.

**Tech Stack:** Rust 2021, Clap 4, serde/serde_json, chrono, sha2, uuid, existing SQLite `MemoryRetriever`, project-local Markdown/JSON/TypeScript bridge files, existing certification evidence writer.

## Global Constraints

- Release this command surface as Tree Ring `0.14.0` and activation protocol `1`; do not change the SQLite schema for activation metadata.
- `.tree-ring/` remains the canonical store and guidance root. `activation.json` and `activation/receipts/` live beneath it.
- `tree-ring init` writes only inside the project. It never writes `HOME`, global harness settings, trust decisions, or Agent Zero core files.
- Never overwrite user-owned bridge files. Update only files recorded as owned in the manifest or text between exact Tree Ring begin/end markers.
- Do not create a daemon, sidecar, hosted service, transcript collector, background durable writer, or prompt recorder.
- Receipts never contain raw prompts, recalled summaries, secrets, sensitive values, coordinator capabilities, or absolute paths. Retain at most 100 receipts per `(harness, worker)` and no receipt older than 30 days.
- `active` requires a fresh matching receipt. A directory marker, a generated skill, a detected executable, or a static certification scan is never behavioral proof.
- Shared activation claims apply only to concurrent processes on one host and a local filesystem. They are not network-filesystem or cross-host claims.
- Agent Zero is integrated only through the separate `tree_ring_memory` plugin and activation protocol. This repository must not modify Agent Zero core.
- Keep coordinated-policy authorization explicit. Bridges, manifests, receipts, generated context, and JSON output must not contain `TREE_RING_COORDINATOR_TOKEN`.

---

## File Structure

| Path | Responsibility |
| --- | --- |
| `Cargo.toml` | Promote the workspace package version to `0.14.0`. |
| `crates/tree-ring-memory-cli/Cargo.toml` | Add direct `sha2` and `uuid` access for activation identity and receipt digests. |
| `crates/tree-ring-memory-cli/src/lib.rs` | Export the activation library for unit and process-level tests. |
| `crates/tree-ring-memory-cli/src/activation/mod.rs` | Public activation types, constants, errors, and module exports. |
| `crates/tree-ring-memory-cli/src/activation/manifest.rs` | Manifest creation, project fingerprinting, atomic persistence, invalidation, and receipt retention. |
| `crates/tree-ring-memory-cli/src/activation/adapters.rs` | Versioned adapter registry, environment probe abstraction, detection, and bridge plans. |
| `crates/tree-ring-memory-cli/src/activation/bridge.rs` | Owned-file and managed-block application/deactivation without overwriting user material. |
| `crates/tree-ring-memory-cli/src/activation/preflight.rs` | Scoped recall, safe context rendering, receipt construction, and hook-format responses. |
| `crates/tree-ring-memory-cli/src/activation/launcher.rs` | Optional high-assurance Claude Code wrapper that supplies preflight context through a mode-0600 temporary file. |
| `crates/tree-ring-memory-cli/src/actions/integrations.rs` | Thin action facade over activation library commands. |
| `crates/tree-ring-memory-cli/src/main.rs` | `init --dry-run`, integration subcommands, store-open ordering, and stable terminal/JSON output. |
| `crates/tree-ring-memory-cli/src/harness_evidence.rs` | Receipt-backed certification records and Markdown compatibility report. |
| `crates/tree-ring-memory-cli/src/agent_awareness.rs` | Generated guidance that tells agent-mediated bridges to run preflight before substantive work. |
| `crates/tree-ring-memory-cli/src/integrations.rs` | Delete after its marker-only scanner is subsumed by the adapter registry. |
| `crates/tree-ring-memory-cli/tests/harness_activation_acceptance.rs` | Real CLI process tests for init, bridge creation, preflight receipts, deactivation, and multi-worker sharing. |
| `fixtures/harness-activation/*.json` | Hermetic detection/bridge/preflight fixture inputs for Codex, Claude Code, Pi, and Agent Zero. |
| `docs/protocol/harness-activation.md` | Stable activation protocol and receipt schema for the external Agent Zero plugin. |
| `docs/integrations/agent-skill.md`, `README.md`, `skills/tree-ring-memory/SKILL.md` | One-command user journey, status meanings, trust/mount actions, and non-security limitations. |
| `scripts/certify-tree-ring.sh` | Replace marker-only harness pass logic with receipt-backed fixture certification and the plugin protocol check. |

### Task 1: Establish the Activation Manifest and Receipt Contract

**Files:**

- Modify: `Cargo.toml`
- Modify: `crates/tree-ring-memory-cli/Cargo.toml`
- Modify: `crates/tree-ring-memory-cli/src/lib.rs`
- Create: `crates/tree-ring-memory-cli/src/activation/mod.rs`
- Create: `crates/tree-ring-memory-cli/src/activation/manifest.rs`

**Interfaces:**

- Produces `ActivationManifest`, `HarnessActivation`, `ActivationState`, `AdapterCapability`, `SessionIdentity`, and `ActivationReceipt` for all later tasks.
- Produces `load_or_create_manifest(memory_root, project_root, cli_version)` and `load_manifest(memory_root)`; the first is writable and the second never creates files.
- Produces `write_receipt(memory_root, receipt)` and `prune_receipts(memory_root, harness_id, worker_key, now)` with atomic replacement semantics.

- [ ] **Step 1: Write failing manifest and receipt tests**

Add module tests that construct a temporary `<project>/.tree-ring` root and assert all of the following:

```rust
#[test]
fn manifest_assigns_a_stable_store_id_and_fingerprints_the_project() {
    let first = load_or_create_manifest(&root, &project, "0.14.0").unwrap();
    let second = load_or_create_manifest(&root, &project, "0.14.0").unwrap();
    assert_eq!(first.store_id, second.store_id);
    assert_eq!(first.project_root_fingerprint, second.project_root_fingerprint);
    assert!(!first.project_root_fingerprint.contains(project.to_str().unwrap()));
}

#[test]
fn receipt_json_excludes_prompt_context_and_capability_material() {
    let receipt = fixture_receipt();
    let json = serde_json::to_string(&receipt).unwrap();
    assert!(!json.contains("user prompt"));
    assert!(!json.contains("recalled summary"));
    assert!(!json.contains("trcap_v1_"));
}
```

- [ ] **Step 2: Run the focused tests and verify they fail because the activation module is absent**

Run: `cargo test -p tree-ring-memory-cli activation::manifest --lib`

Expected: FAIL with an unresolved `activation` module or missing contract types.

- [ ] **Step 3: Add the versioned, path-safe model and persistence implementation**

Set the workspace version to `0.14.0`, add `sha2.workspace = true` and `uuid.workspace = true` to the CLI crate, and export the new module from `src/lib.rs`. Define these exact public names in `activation/mod.rs`:

```rust
pub const ACTIVATION_SCHEMA_VERSION: u16 = 1;
pub const ACTIVATION_PROTOCOL_VERSION: u16 = 1;
pub const RECEIPT_RETENTION_PER_WORKER: usize = 100;
pub const RECEIPT_RETENTION_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActivationState {
    Active,
    ConfiguredAwaitingProof,
    ActiveIsolated,
    NeedsTrust,
    NeedsProjectMount,
    NeedsPlugin,
    NeedsUserReview,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdapterCapability { NativePreflight, WrapperPreflight, GuidanceOnly }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionIdentity {
    pub agent_profile: String,
    pub workflow_id: String,
    pub session_id: String,
}
```

Persist a manifest at `.tree-ring/activation.json` with `schema_version`, `protocol_version`, generated `store_id`, SHA-256 `project_root_fingerprint`, CLI version, and a `BTreeMap<String, HarnessActivation>`. Store only project-relative bridge paths in `HarnessActivation`. Persist receipts below `.tree-ring/activation/receipts/<harness>/<sha256(worker)>/<receipt-id>.json`; the hash avoids putting an arbitrary worker identity into a filesystem path.

Use one `atomic_write_json(path, &T)` helper that writes a uniquely named sibling temp file, calls `sync_all`, and renames only after serialization succeeds. Validate all externally supplied identifiers as nonblank, at most 256 characters, and free of control characters before they can enter a manifest or receipt.

- [ ] **Step 4: Run focused contract tests**

Run: `cargo test -p tree-ring-memory-cli activation::manifest --lib`

Expected: PASS, including first-create/idempotency, malformed manifest failure, atomic overwrite behavior, 30-day expiry, and 100-record retention.

- [ ] **Step 5: Commit the standalone data contract**

```bash
git add Cargo.toml crates/tree-ring-memory-cli/Cargo.toml \
  crates/tree-ring-memory-cli/src/lib.rs \
  crates/tree-ring-memory-cli/src/activation/mod.rs \
  crates/tree-ring-memory-cli/src/activation/manifest.rs
git commit -m "feat: add harness activation manifest"
```

### Task 2: Define the Versioned Adapter Registry and Non-Active Detection States

**Files:**

- Create: `crates/tree-ring-memory-cli/src/activation/adapters.rs`
- Modify: `crates/tree-ring-memory-cli/src/activation/mod.rs`
- Delete: `crates/tree-ring-memory-cli/src/integrations.rs`
- Modify: `crates/tree-ring-memory-cli/src/actions/integrations.rs`

**Interfaces:**

- Consumes Task 1 types and the project-relative manifest model.
- Produces `maintained_adapters()`, `detect_adapters()`, `plan_activation()`, and `plan_deactivation()`.
- Produces `AdapterPlan` composed only of `BridgeWrite`, `ManagedBlockUpdate`, or a blocking `ActivationState`; no adapter performs filesystem writes itself.

- [ ] **Step 1: Write failing registry tests around false-positive markers and activation plans**

Add tests with a fake `HarnessEnvironment` rather than the real home directory or PATH:

```rust
#[test]
fn an_empty_codex_marker_is_detected_as_a_candidate_but_never_active() {
    let report = detect_adapters(&fixture_project_with(".codex"), &fake_env());
    let codex = report.by_id("codex").unwrap();
    assert_ne!(codex.state, ActivationState::Active);
    assert!(codex.plan.is_some());
}

#[test]
fn unknown_harnesses_are_explicitly_unsupported_without_bridge_writes() {
    let plan = plan_activation("hermes", &context()).unwrap();
    assert_eq!(plan.state, ActivationState::Unsupported);
    assert!(plan.writes.is_empty());
}

#[test]
fn missing_agent_zero_plugin_requires_the_separate_plugin_without_core_mutation() {
    let detection = detect_adapters(&fixture_project(), &fake_env_without_agent_zero_plugin());
    let agent_zero = detection.by_id("agent-zero").unwrap();
    assert_eq!(agent_zero.state, ActivationState::NeedsPlugin);
    assert!(agent_zero.plan.writes.is_empty());
}
```

- [ ] **Step 2: Run the registry tests and verify they fail before the adapter abstraction exists**

Run: `cargo test -p tree-ring-memory-cli activation::adapters --lib`

Expected: FAIL because `HarnessEnvironment`, `AdapterPlan`, and `maintained_adapters` do not exist.

- [ ] **Step 3: Implement the declarative adapter contract and probes**

Use a testable environment abstraction instead of querying global state from constructors:

```rust
pub trait HarnessEnvironment {
    fn executable_version(&self, command: &str) -> Option<String>;
    fn project_path_exists(&self, relative: &Path) -> bool;
    fn read_project_file(&self, relative: &Path) -> Result<Option<String>, String>;
    fn agent_zero_plugin_manifest(&self) -> Option<AgentZeroPluginManifest>;
}

pub trait HarnessAdapter: Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn capability(&self) -> AdapterCapability;
    fn detect(&self, project: &ActivationProject, env: &dyn HarnessEnvironment) -> AdapterDetection;
    fn plan(&self, project: &ActivationProject, detection: &AdapterDetection) -> AdapterPlan;
}
```

Ship four maintained adapter IDs: `codex`, `claude-code`, `pi`, and `agent-zero`. The Agent Zero adapter must return `needs-plugin` with no writes when its compatible separate plugin is absent or disabled; when present, it may plan only the protocol binding consumed by that plugin. Keep `hermes`, `opencode`, and `goose` registry entries as `Unsupported` with an adapter-authoring next step and no filesystem plan. A project marker can identify a candidate but must never generate an active status; a receipt is the only state transition to `active`.

Make `ActivationProject::from_memory_root` derive the project root as the parent of the memory root, reject a root without a parent, and use only normalized project-relative paths in adapter plans. Preserve the old public `integrations scan` behavior as a read-only compatibility alias backed by `detect_adapters`; remove its marker confidence score and emit detection plus activation state instead.

- [ ] **Step 4: Run registry tests and the existing scan tests**

Run: `cargo test -p tree-ring-memory-cli activation::adapters --lib`

Run: `cargo test -p tree-ring-memory-cli integrations_scan_is_read_only_and_detects_project_markers`

Expected: PASS. The compatibility scan still performs no writes; empty `.codex` no longer yields a pass-like state.

- [ ] **Step 5: Commit the adapter planning boundary**

```bash
git add crates/tree-ring-memory-cli/src/activation/mod.rs \
  crates/tree-ring-memory-cli/src/activation/adapters.rs \
  crates/tree-ring-memory-cli/src/actions/integrations.rs \
  crates/tree-ring-memory-cli/src/integrations.rs \
  crates/tree-ring-memory-cli/src/main.rs
git commit -m "feat: add versioned harness adapter registry"
```

### Task 3: Apply and Reverse Only Owned Bridge Material

**Files:**

- Create: `crates/tree-ring-memory-cli/src/activation/bridge.rs`
- Modify: `crates/tree-ring-memory-cli/src/activation/adapters.rs`
- Modify: `crates/tree-ring-memory-cli/src/activation/manifest.rs`

**Interfaces:**

- Consumes `AdapterPlan`, `BridgeWrite`, and manifest-owned relative paths from Tasks 1–2.
- Produces `apply_bridge_plan(project, manifest, plan, accept_managed_block)` and `deactivate_bridge_plan(project, manifest, harness_id)`.
- Produces the canonical marker pair `<!-- tree-ring:begin <harness> v1 -->` and `<!-- tree-ring:end <harness> -->` for Markdown bridge edits.

- [ ] **Step 1: Write failing safe-write tests**

Cover dedicated paths, managed text blocks, JSON merge, and deactivation:

```rust
#[test]
fn unmanaged_agents_file_requires_explicit_review() {
    write(project.join("AGENTS.md"), "# Team contract\n");
    let result = apply_bridge_plan(&ctx, &mut manifest, codex_plan(), false).unwrap();
    assert_eq!(result.state, ActivationState::NeedsUserReview);
    assert_eq!(read(project.join("AGENTS.md")), "# Team contract\n");
}

#[test]
fn deactivation_preserves_non_tree_ring_text_and_receipts() {
    let result = deactivate_bridge_plan(&ctx, &mut manifest, "codex").unwrap();
    assert!(read(project.join("AGENTS.md")).contains("# Team contract"));
    assert!(receipt_path.exists());
    assert!(project.join(".tree-ring/AGENTS.md").exists());
    assert!(!project.join(".agents/skills/tree-ring-memory/SKILL.md").exists());
}
```

- [ ] **Step 2: Run focused bridge tests and verify they fail before any writer exists**

Run: `cargo test -p tree-ring-memory-cli activation::bridge --lib`

Expected: FAIL with missing `apply_bridge_plan` and `deactivate_bridge_plan`.

- [ ] **Step 3: Implement deterministic bridge ownership and exact renderer output**

Implement these owned bridge targets:

- Codex: `.agents/skills/tree-ring-memory/SKILL.md` plus an `AGENTS.md` managed block. If root `AGENTS.md` is absent, create an entirely Tree Ring-owned file. If it exists without the exact markers, return `needs-user-review` and show the one-command retry `tree-ring integrations activate --harness codex --accept-managed-block`.
- Claude Code: `.claude/skills/tree-ring-memory/SKILL.md` and a structured `SessionStart` command-hook entry in `.claude/settings.json`. The JSON entry is identifiable by `description: "Tree Ring Memory managed preflight v1"` and invokes `tree-ring --root .tree-ring integrations preflight --harness claude-code --input-json-stdin --context-format claude-session-start`. Parse valid JSON, append only the owned handler, preserve every unrelated value, and return `needs-user-review` for invalid JSON or a conflicting Tree Ring handler.
- Pi: `.agents/skills/tree-ring-memory/SKILL.md` and the fully owned `.pi/extensions/tree-ring-memory.ts`. The extension invokes the preflight command with JSON on stdin during `before_agent_start`, receives the safe context, and returns it as a non-display message. Its source must use only Node built-ins and Pi's supplied `ExtensionAPI`; it must not add a dependency install or a background process.
- Agent Zero: only after the registry verified a compatible separate plugin, write `.tree-ring/activation/agent-zero.json`, containing protocol version, `store_id`, project fingerprint, relative `.tree-ring` root, and command protocol. It never installs or modifies the plugin. Otherwise retain `needs-plugin` and make no Agent Zero bridge write.

Record a SHA-256 of every complete owned file and an explicit block identifier for each managed block. On retry, update only an unchanged owned file or exact block. Do not delete an unmanaged file whose content happens to resemble Tree Ring guidance.

Deactivation removes only files and exact managed blocks recorded as Tree Ring-owned for the selected harness. It retains the `.tree-ring` database, canonical generated guidance, activation manifest, and historical receipts so another harness can remain active and a later reactivation has an audit trail.

The Pi extension must have this behavior shape:

```ts
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
```

`runPreflightWithStdin` must pass the task hint over stdin, never argv or disk, and must use the fallback query when sensitivity detection rejects the hint. Pi remains `needs-trust` until its project extension executes a receipt-producing preflight.

- [ ] **Step 4: Run bridge tests**

Run: `cargo test -p tree-ring-memory-cli activation::bridge --lib`

Expected: PASS for idempotency, dry-run zero writes, managed-block retry, exact JSON merge preservation, invalid configuration, and receipt-preserving deactivation.

- [ ] **Step 5: Commit safe bridge material**

```bash
git add crates/tree-ring-memory-cli/src/activation/bridge.rs \
  crates/tree-ring-memory-cli/src/activation/adapters.rs \
  crates/tree-ring-memory-cli/src/activation/manifest.rs
git commit -m "feat: install reversible project harness bridges"
```

### Task 4: Run Scoped Recall and Persist a Privacy-Safe Behavioral Receipt

**Files:**

- Create: `crates/tree-ring-memory-cli/src/activation/preflight.rs`
- Modify: `crates/tree-ring-memory-cli/src/activation/mod.rs`
- Modify: `crates/tree-ring-memory-cli/src/activation/manifest.rs`

**Interfaces:**

- Consumes the Task 1 manifest/receipt contract, the active adapter record, and `SQLiteMemoryStore`.
- Produces `run_preflight(store, project, manifest, request) -> Result<PreflightResponse, ActivationError>`.
- Produces hook-format emitters `render_claude_session_start`, `render_pi_context`, and `render_json_context` without persisting their context text.

- [ ] **Step 1: Write failing preflight tests against a seeded local store**

Seed one normal project memory and one sensitive memory, then assert:

```rust
#[test]
fn preflight_injects_only_safe_recall_and_writes_a_matching_receipt() {
    let response = run_preflight(&store, &project, &manifest, fixture_request()).unwrap();
    assert!(response.context.contains("project startup constraint"));
    assert!(!response.context.contains("sensitive fixture"));
    let receipt = load_only_receipt(&root);
    assert_eq!(receipt.query_class, "task_hint");
    assert_eq!(receipt.result_count, 1);
    assert_eq!(receipt.store_id, manifest.store_id);
}

#[test]
fn failed_context_serialization_leaves_no_receipt() {
    let error = render_then_record(&store, invalid_context_request()).unwrap_err();
    assert!(error.to_string().contains("context"));
    assert!(receipt_files(&root).is_empty());
}
```

- [ ] **Step 2: Run preflight tests and verify they fail before the service exists**

Run: `cargo test -p tree-ring-memory-cli activation::preflight --lib`

Expected: FAIL with missing `run_preflight` and response types.

- [ ] **Step 3: Implement preflight, receipt validation, and deterministic context output**

Use this request and response boundary:

```rust
pub struct PreflightRequest {
    pub harness_id: String,
    pub identity: SessionIdentity,
    pub task_hint: Option<String>,
    pub context_format: PreflightContextFormat,
}

pub struct PreflightResponse {
    pub state: ActivationState,
    pub context: String,
    pub receipt: ActivationReceiptSummary,
}
```

Define `PreflightContextFormat::{ClaudeSessionStart, PiBeforeAgentStart, Json}`. For a Claude `SessionStart` hook, write exactly one JSON value to stdout and no human/log text:

```json
{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"<safe context>"}}
```

For Pi and the Agent Zero plugin, emit one JSON object containing the safe `context`, redacted state, and receipt summary; their bridge readers must reject malformed output. Never include raw recalled content in a receipt or diagnostic even though safe context is passed privately to the running harness.

Derive the query as a redacted task hint only when `SensitivityGuard` accepts it; otherwise use the exact fallback `project startup constraints`. Run `MemoryRetriever` with `include_sensitive: false`, `include_superseded: false`, project/workflow/session filters from `SessionIdentity`, and a maximum of eight results. Render only safe memory IDs, summaries, source references, and a reminder that project source and instructions remain authoritative.

Build the complete hook/JSON payload before writing a receipt. Then atomically write the receipt with harness/adapter versions, `store_id`, project fingerprint, identity, query class, result count, SHA-256 digest of sorted selected memory IDs, duration, and success status. A zero-result recall writes a valid receipt. Errors, timeout, wrong manifest protocol, stale adapter version, root mismatch, failed JSON serialization, or failed recall write no active receipt.

Set `active-isolated` when the caller's configured memory root is not the mounted `<project>/.tree-ring` root; return a receipt only for the local accessible root and never copy data between stores. Invalidate all receipts for an adapter when its version, bridge fingerprint, project fingerprint, or store ID changes.

- [ ] **Step 4: Run preflight and receipt retention tests**

Run: `cargo test -p tree-ring-memory-cli activation::preflight --lib`

Expected: PASS for normal recall, zero-result recall, sensitive exclusion, fallback query, store mismatch, malformed stdin, receipt expiry, and no prompt/content leakage.

- [ ] **Step 5: Commit behavioral proof support**

```bash
git add crates/tree-ring-memory-cli/src/activation/mod.rs \
  crates/tree-ring-memory-cli/src/activation/preflight.rs \
  crates/tree-ring-memory-cli/src/activation/manifest.rs
git commit -m "feat: record harness preflight receipts"
```

### Task 5: Add the Optional High-Assurance Claude Code Wrapper

**Files:**

- Create: `crates/tree-ring-memory-cli/src/activation/launcher.rs`
- Modify: `crates/tree-ring-memory-cli/src/activation/mod.rs`
- Modify: `crates/tree-ring-memory-cli/src/main.rs`
- Modify: `crates/tree-ring-memory-cli/tests/harness_activation_acceptance.rs`

**Interfaces:**

- Consumes Task 4's preflight response before its receipt is persisted.
- Produces `tree-ring integrations launch --harness claude-code -- <claude arguments>` for an explicit high-assurance launch path.
- Produces no generic launch claim for Codex, Pi, or Agent Zero when their adapter lacks a tested wrapper capability.

- [ ] **Step 1: Write failing wrapper command-construction tests**

Add a process-double test that captures the child command and temporary context path:

```rust
#[test]
fn claude_launcher_injects_preflight_context_from_a_private_file() {
    let launch = launch_with_preflight(&store, &project, &manifest, request(), &["--model", "sonnet"]);
    let child = launch.unwrap();
    assert_eq!(child.program, PathBuf::from("claude"));
    let context_argument = child.context_path.to_string_lossy().into_owned();
    assert!(child.args.windows(2).any(|pair| {
        pair[0] == "--append-system-prompt-file"
            && pair[1] == context_argument
    }));
    assert_eq!(mode(&child.context_path), 0o600);
    assert!(read(&child.context_path).contains("Tree Ring preflight context"));
}

#[test]
fn wrapper_rejects_an_unsupported_harness_without_writing_context() {
    let error = launch_with_preflight(&store, &project, &manifest, request_for("codex"), &[]).unwrap_err();
    assert!(error.to_string().contains("does not provide a wrapper preflight"));
    assert!(runtime_context_files(&root).is_empty());
}
```

- [ ] **Step 2: Run wrapper tests and verify they fail before the launcher exists**

Run: `cargo test -p tree-ring-memory-cli activation::launcher --lib`

Expected: FAIL with missing launcher functions and command variant.

- [ ] **Step 3: Implement only the tested Claude Code wrapper**

Add `AdapterCapability::WrapperPreflight` support in the registry, but initially assign it only to the Claude Code wrapper launch descriptor. The launcher must:

1. Call the Task 4 preflight preparation path without immediately committing the receipt.
2. Write the already-rendered safe context to `.tree-ring/activation/runtime/<receipt-id>.md` with mode `0600`.
3. Spawn `claude --append-system-prompt-file <context-path>` followed by the user arguments after `--`.
4. Persist the receipt only after `Command::spawn` succeeds, because the exact context file was handed to the harness process.
5. Wait for the child, delete the temporary context file on normal exit, error, and signal handling, and return the child's exit code.

Expose it as:

```text
tree-ring integrations launch --harness claude-code -- --model sonnet
```

Do not accept an arbitrary program path, do not pass recalled context in argv, and do not use the wrapper during ordinary `tree-ring init`. Pi's native extension and Agent Zero's plugin protocol remain their preferred paths; Codex remains agent-mediated through its bridge and preflight command.

- [ ] **Step 4: Run wrapper and existing CLI tests**

Run: `cargo test -p tree-ring-memory-cli activation::launcher --lib`

Run: `cargo test -p tree-ring-memory-cli --bin tree-ring-memory-cli`

Expected: PASS. Claude wrapper receipts exist only after a successful spawn and no temporary safe-context file survives process completion.

- [ ] **Step 5: Commit the optional wrapper path**

```bash
git add crates/tree-ring-memory-cli/src/activation/launcher.rs \
  crates/tree-ring-memory-cli/src/activation/mod.rs \
  crates/tree-ring-memory-cli/src/main.rs \
  crates/tree-ring-memory-cli/tests/harness_activation_acceptance.rs
git commit -m "feat: add Claude preflight launch wrapper"
```

### Task 6: Wire `init`, Status, Activation, Preflight, and Deactivation Commands

**Files:**

- Modify: `crates/tree-ring-memory-cli/src/main.rs`
- Modify: `crates/tree-ring-memory-cli/src/actions/integrations.rs`
- Modify: `crates/tree-ring-memory-cli/src/agent_awareness.rs`

**Interfaces:**

- Consumes Tasks 1–4 through a thin action facade.
- Produces `tree-ring init [--dry-run]`, `tree-ring integrations status [--verbose]`, `activate`, `link`, `preflight`, the Claude-only `launch`, `deactivate`, and receipt-backed `certify [--live]`.
- Keeps `tree-ring integrations scan` as a read-only compatibility alias.

- [ ] **Step 1: Write failing CLI parser and output tests**

Add parser and `run(Cli)` tests for these exact invocations:

```rust
let cli = Cli::try_parse_from([
    "tree-ring", "integrations", "preflight", "--harness", "codex",
    "--agent-profile", "worker-a", "--workflow-id", "fanout-1",
    "--session-id", "session-1",
]).unwrap();

let dry_run = Cli::try_parse_from(["tree-ring", "init", "--dry-run"]).unwrap();
assert!(matches!(dry_run.command, Command::Init { dry_run: true }));
```

Assert `init --dry-run` creates neither `memory.sqlite` nor `.tree-ring/activation.json`; ordinary `init` creates canonical guidance, plans all detected adapters, and returns `configured-awaiting-proof` rather than `active` before a receipt.

- [ ] **Step 2: Run CLI tests and verify they fail because the subcommands do not exist**

Run: `cargo test -p tree-ring-memory-cli cli_init_creates_store --bin tree-ring-memory-cli`

Run: `cargo test -p tree-ring-memory-cli integrations_ --bin tree-ring-memory-cli`

Expected: FAIL on the new parser and activation expectations.

- [ ] **Step 3: Implement safe store-open ordering and command routing**

Replace the unit `Command::Init` with `Command::Init { dry_run: bool }`. Handle `init --dry-run`, `integrations status`, scan, activate/link dry-runs, certify, and deactivation before `SQLiteMemoryStore::open_with_context`; therefore read-only status/certification and all dry-runs cannot create or migrate a store. Open the store only for ordinary init and `preflight`.

Add these subcommand shapes:

```rust
enum IntegrationCommand {
    Scan { source_root: PathBuf },
    Status { source_root: PathBuf, verbose: bool },
    Activate { harness: String, source_root: PathBuf, dry_run: bool, accept_managed_block: bool },
    Link { harness: String, source_root: PathBuf, dry_run: bool, accept_managed_block: bool },
    Preflight { harness: String, agent_profile: Option<String>, workflow_id: Option<String>, session_id: Option<String>, input_json_stdin: bool, context_format: PreflightContextFormat },
    Launch { harness: String, arguments: Vec<OsString> },
    Deactivate { harness: String, source_root: PathBuf },
    Certify { source_root: PathBuf, out_dir: Option<PathBuf>, live: bool },
}
```

`init` must call `ensure_agent_awareness`, initialize/load the manifest, detect supported adapters, apply only safe plans, and print one concise line per detected adapter: `Codex: configured-awaiting-proof`, `Claude Code: configured-awaiting-proof`, `Pi: needs-trust`, or an equally exact state. It may print `active` only when a fresh matching receipt already exists. `--json` must include `store_id`, adapter state, capability, managed paths, one actionable next step, and receipt age without raw receipt data.

For `preflight`, require all three identity flags in the direct CLI mode used by Codex guidance. With `--input-json-stdin`, reject those flags and parse a single adapter-specific JSON object without logging it: the Claude Code adapter consumes only `session_id`, `cwd`, and optional `agent_type` from the documented `SessionStart` event, verifies that `cwd` resolves inside the manifest project, ignores `transcript_path` and every other field, and derives a stable session/workflow identity; the Pi extension and Agent Zero plugin provide their validated structured identity payloads. Reject unknown fields that could carry capabilities, preserve only a sensitivity-accepted optional Pi task hint, and use `project startup constraints` otherwise. This resolves identity before constructing `PreflightRequest`; a malformed, incomplete, out-of-project, or ambiguous input writes no receipt.

Update generated `.tree-ring/SKILL.md`, `AGENTS.md`, and `CLI.md` so a Codex-style agent reads the canonical guidance and invokes preflight before substantive work. The generated instructions must name the fallback-safe command and state that a receipt proves usage but is not an adversarial security boundary.

- [ ] **Step 4: Run command and generated-guidance tests**

Run: `cargo test -p tree-ring-memory-cli --bin tree-ring-memory-cli`

Expected: PASS, including no-store dry-run/status behavior, init idempotency, JSON output, managed-block review, and generated preflight guidance.

- [ ] **Step 5: Commit the one-command user flow**

```bash
git add crates/tree-ring-memory-cli/src/main.rs \
  crates/tree-ring-memory-cli/src/actions/integrations.rs \
  crates/tree-ring-memory-cli/src/agent_awareness.rs
git commit -m "feat: activate harnesses during tree-ring init"
```

### Task 7: Replace Marker Certification with Receipt-Backed Evidence

**Files:**

- Modify: `crates/tree-ring-memory-cli/src/harness_evidence.rs`
- Modify: `crates/tree-ring-memory-cli/src/main.rs`
- Modify: `crates/tree-ring-memory-cli/src/evidence.rs`

**Interfaces:**

- Consumes activation manifest and receipts from Tasks 1–6.
- Produces `HarnessActivationEvidence` within each `HarnessProbeRecord` and a Markdown report that differentiates active, configured, isolated, blocked, unsupported, skipped, and failed.
- Preserves `EvidenceIndex.harness` compatibility while mapping `active` to pass, blocking states to skip, and genuine failures to fail.

- [ ] **Step 1: Write failing certification tests that reject the former false positive**

Replace the current marker-pass test with this behavior:

```rust
#[test]
fn certification_does_not_pass_an_empty_codex_directory() {
    fs::create_dir_all(project.join(".codex")).unwrap();
    let report = certify_harnesses(request(project)).unwrap();
    assert_ne!(record(&report, "codex").status, EvidenceStatus::Pass);
    assert_eq!(record(&report, "codex").activation.state, ActivationState::ConfiguredAwaitingProof);
}

#[test]
fn certification_passes_only_a_fresh_matching_receipt() {
    write_matching_receipt(&project, "claude-code");
    let report = certify_harnesses(request(project)).unwrap();
    assert_eq!(record(&report, "claude-code").status, EvidenceStatus::Pass);
}
```

- [ ] **Step 2: Run evidence tests and verify the old marker behavior fails**

Run: `cargo test -p tree-ring-memory-cli harness_evidence --bin tree-ring-memory-cli`

Expected: FAIL until certification reads manifests and validates receipts.

- [ ] **Step 3: Implement activation evidence records and `--live` semantics**

Add an activation section with adapter version, capability, state, receipt timestamp/age, store-id match boolean, project-root match boolean, and redacted diagnostic. Preserve raw bridge paths only under `integrations status --verbose`, not in default certification summaries.

Validate each receipt against manifest protocol, adapter version, bridge fingerprint, store ID, project fingerprint, 30-day TTL, and the selected harness ID. A marker can appear only as explanatory detection evidence. It cannot determine pass/fail. `--live` may execute an installed maintainer harness fixture; missing optional executables produce `skip` with `not installed locally`, never `pass` and never `fail`.

Publish JSON and `harness-activation-summary.md` with the existing atomic evidence-index protocol. Do not change unrelated recall-quality or performance evidence behavior.

- [ ] **Step 4: Run harness evidence suite**

Run: `cargo test -p tree-ring-memory-cli harness_certification --bin tree-ring-memory-cli`

Expected: PASS for fresh/expired/malformed receipts, empty marker rejection, isolated Agent Zero status, and unchanged evidence index preservation.

- [ ] **Step 5: Commit behavioral certification**

```bash
git add crates/tree-ring-memory-cli/src/harness_evidence.rs \
  crates/tree-ring-memory-cli/src/main.rs \
  crates/tree-ring-memory-cli/src/evidence.rs
git commit -m "feat: certify harness behavior with receipts"
```

### Task 8: Prove Same-Host Multi-Agent Behavior with Hermetic Harness Fixtures

**Files:**

- Create: `crates/tree-ring-memory-cli/tests/harness_activation_acceptance.rs`
- Create: `fixtures/harness-activation/codex.json`
- Create: `fixtures/harness-activation/claude-code.json`
- Create: `fixtures/harness-activation/pi.json`
- Create: `fixtures/harness-activation/agent-zero.json`
- Modify: `crates/tree-ring-memory-cli/tests/multi_agent_acceptance.rs`

**Interfaces:**

- Consumes actual CLI commands from Task 6 and receipt validation from Task 7.
- Produces reproducible process-level evidence that different workers emit distinct receipts and are shared only through the canonical same-host store ID.

- [ ] **Step 1: Write failing end-to-end acceptance tests**

Use `CARGO_BIN_EXE_tree-ring` and a temporary project. Seed a normal project memory, run init, execute Codex preflight for workers `worker-a` and `worker-b`, then execute Claude Code preflight for `worker-b`; assert:

```rust
assert_eq!(status("codex")["state"], "active");
assert_eq!(status("claude-code")["state"], "active");
assert_ne!(receipt("codex", "worker-a")["session_id"], receipt("codex", "worker-b")["session_id"]);
assert_eq!(receipt("codex", "worker-a")["store_id"], receipt("claude-code", "worker-b")["store_id"]);
assert!(!receipt_json.contains("seeded memory summary"));
```

Add an isolated-root fixture that calls preflight from a different `.tree-ring` directory and asserts `active-isolated`, no canonical-store receipt mutation, and no copied SQLite file.

- [ ] **Step 2: Run the acceptance target and verify it fails before end-to-end support exists**

Run: `cargo test -p tree-ring-memory-cli --test harness_activation_acceptance`

Expected: FAIL because the integration commands and receipts are incomplete.

- [ ] **Step 3: Implement fixtures and update the existing multi-agent acceptance test**

Give every fixture explicit adapter version, expected bridge paths, expected activation state before proof, source-safe seeded memory, and no user-home dependencies. Extend the existing eight-process acceptance test only with receipt assertions; retain its existing write idempotency and coordinator authorization coverage unchanged.

The test must launch only the Tree Ring binary and fixture bridge commands. It must not invoke installed Codex, Claude, Pi, or Agent Zero runtimes by default. Gate a true installed-harness run behind `TREE_RING_LIVE_HARNESS_TESTS=1`, require the harness executable explicitly, and record `skip` rather than creating a support claim when unavailable.

- [ ] **Step 4: Run both multi-agent suites**

Run: `cargo test -p tree-ring-memory-cli --test harness_activation_acceptance`

Run: `cargo test -p tree-ring-memory-cli --test multi_agent_acceptance`

Expected: PASS with same-host worker isolation, receipt retention, isolated-store classification, and coordinator-token redaction intact.

- [ ] **Step 5: Commit harness proof fixtures**

```bash
git add crates/tree-ring-memory-cli/tests/harness_activation_acceptance.rs \
  crates/tree-ring-memory-cli/tests/multi_agent_acceptance.rs \
  fixtures/harness-activation
git commit -m "test: prove multi-agent harness activation"
```

### Task 9: Publish the Protocol, Update Documentation, and Certify Without Static Claims

**Files:**

- Create: `docs/protocol/harness-activation.md`
- Modify: `README.md`
- Modify: `docs/integrations/agent-skill.md`
- Modify: `skills/tree-ring-memory/SKILL.md`
- Modify: `scripts/certify-tree-ring.sh`

**Interfaces:**

- Consumes the final command/output shapes from Tasks 1–8.
- Produces the stable protocol consumed by `tree-ring-memory-agent-zero` and a user-facing one-command workflow that does not promise unsupported harness behavior.

- [ ] **Step 1: Write documentation assertions and certification fixture checks first**

Add focused source-level tests that require documentation to state all non-negotiable claims:

```rust
assert!(readme.contains("tree-ring init"));
assert!(readme.contains("configured-awaiting-proof"));
assert!(readme.contains("active-isolated"));
assert!(readme.contains("same-host local filesystem"));
assert!(!readme.contains("marker-only pass"));
```

Add shell checks in `scripts/certify-tree-ring.sh` that initialize a fixture project, seed safe memory, run a preflight, and fail if `integrations certify` reports a marker-only pass or retains a raw prompt string.

- [ ] **Step 2: Run the documentation assertion and certification fixture commands to verify they fail before updates**

Run: `cargo test -p tree-ring-memory-cli --test harness_activation_acceptance documentation`

Run: `sh scripts/certify-tree-ring.sh`

Expected: the documentation assertion fails until the old read-only discovery language is replaced; certification may fail at the stale Agent Zero `execute.py` probe until that probe is removed.

- [ ] **Step 3: Write the protocol and revise all user guidance**

Document JSON fields, `ACTIVATION_PROTOCOL_VERSION = 1`, receipt redaction, status transition rules, root/store matching, the Agent Zero mount/plugin boundary, and the exact hook payload formats. In README and generated guidance, lead with:

```bash
tree-ring init
tree-ring integrations status
```

Explain that default init does the safe work automatically; only Pi trust, an unmanaged instruction file review, an Agent Zero plugin installation, or an inaccessible project mount requires one concise user action. Replace instructions that say users must manually copy a bridge or run a mandatory link command. Keep `integrations link` documented as an advanced alias, not the default journey.

Replace the stale Agent Zero certification call to a removed `execute.py` with the plugin's documented protocol test command. The core certification script must run that external test only when `TREE_RING_AGENT_ZERO_ROOT` names a checkout containing the plugin; otherwise record `skip` without claiming an Agent Zero pass.

- [ ] **Step 4: Run the full core verification sequence**

Run: `cargo fmt --check`

Run: `cargo test --locked`

Run: `cargo clippy --locked --all-targets -- -D warnings`

Run: `sh scripts/certify-tree-ring.sh`

Run: `git diff --check`

Expected: PASS. Certification contains receipt-backed fixture evidence, no static marker pass, and an explicit Agent Zero plugin skip when the external checkout is absent.

- [ ] **Step 5: Commit protocol and user journey documentation**

```bash
git add docs/protocol/harness-activation.md README.md \
  docs/integrations/agent-skill.md skills/tree-ring-memory/SKILL.md \
  scripts/certify-tree-ring.sh
git commit -m "docs: explain verified harness activation"
```

## Completion Evidence

Before release, retain all of the following from a clean worktree:

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test -p tree-ring-memory-cli --test harness_activation_acceptance
cargo test -p tree-ring-memory-cli --test multi_agent_acceptance
sh scripts/certify-tree-ring.sh
git diff --check
```

The release review must inspect the generated `activation.json`, one redacted receipt, the JSON and Markdown certification reports, and the clean working tree. Treat a missing runtime, denied Pi trust, absent Agent Zero plugin, inaccessible mount, unmanaged project configuration, or expired receipt as its exact non-active state; do not collapse any of them into `active`.
