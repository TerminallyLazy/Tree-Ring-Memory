# Agent Workflow Proof Runner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an explicit, reproducible Codex workflow runner that compares no-memory, raw-memory, and Tree Ring retrieval against deterministic workspace validators, then preserves the trial evidence needed to inspect any claimed lift.

**Architecture:** Keep the fixture schema and validation boundary in `tree-ring-memory-core`; it owns safe workspace fixtures, expected file checks, and the agent request type that deliberately omits validator expectations. Add a small CLI library that creates isolated trial workspaces, builds fair memory contexts, invokes `codex exec` only when the explicit example is run, validates the resulting workspace, and writes JSON/Markdown evidence. The runner is certification-owned through an example binary, not a public `tree-ring eval` command and not a background process.

**Tech Stack:** Rust 2021, serde/serde_json, SQLite `MemoryRetriever`, standard-library `Command`, locally installed Codex CLI, JSON fixtures.

## Global Constraints

- Do not add a daemon, sidecar, hosted service, telemetry pipeline, hidden recorder, or autonomous durable writer.
- Do not scrape transcripts or turn agent event-streams into memory.
- Keep the agent's task prompt and each arm identical except for `memory_context`.
- Never serialize validator expectations, expected files, or fixture locations into `WorkflowAgentRequest`.
- Treat source files and explicit task instructions as authoritative over memory; the Codex prompt must say so.
- Raw-memory and Tree-Ring arms must both exclude secret, non-normal, and superseded memory; only retrieval ranking may differ.
- Run every arm in a separate retained workspace below the caller-provided output directory.
- The runner must preserve structured reports on partial agent failures; control-arm failure is evidence, not a runner error.
- Exit nonzero only when a Tree-Ring arm errors or fails its deterministic validator; do not require controls to pass.
- Do not add a public `tree-ring eval` subcommand. The only executable entry point in this slice is `cargo run -p tree-ring-memory-cli --example workflow_proof -- ...`.
- Do not invoke Codex from normal unit tests or `scripts/certify-tree-ring.sh`; a real run remains a user-visible, explicit command.
- Evidence-producing example runs must require `--model <id>` and record that requested model identity in both JSON and Markdown reports; never silently rely on an unrecorded Codex default.
- Preserve unrelated files; work only on `codex/agent-workflow-proof`.

---

## File Structure

- Create `crates/tree-ring-memory-core/src/workflow.rs`: strict workflow fixture schema, arm enum, agent request/response types, safe relative-path validation, and deterministic file-check evaluator.
- Modify `crates/tree-ring-memory-core/src/lib.rs`: export the workflow types and parser.
- Create `crates/tree-ring-memory-core/tests/workflow_scenario.rs`: parser and request-redaction coverage.
- Create `crates/tree-ring-memory-cli/src/lib.rs`: library target for proof-runner reuse.
- Create `crates/tree-ring-memory-cli/src/workflow_proof.rs`: paired runner, retained workspaces, raw/Tree-Ring context construction, Codex adapter, report writer, and test-only fake agent.
- Create `crates/tree-ring-memory-cli/examples/workflow_proof.rs`: minimal explicit argument entry point.
- Create `crates/tree-ring-memory-cli/tests/workflow_proof.rs`: end-to-end runner tests using a fake in-process agent.
- Create `fixtures/workflow-proof/no-background-writer.json`, `fixtures/workflow-proof/stale-cli-contract.json`, and `fixtures/workflow-proof/scar-recovery.json`: reviewable, source-linked workflow fixtures.
- Create `docs/integrations/agent-workflow-proof.md`: command contract, evidence layout, interpretation limits, and reproducibility checklist.
- Modify `crates/tree-ring-memory-cli/examples/workflow_proof.rs`: factor its positional parser into a testable private parser and make `--help` print the exact usage text without invoking Codex.
- Modify `crates/tree-ring-memory-cli/src/workflow_proof.rs`: surface the agent evidence identity in `WorkflowProofReport`; Codex must include the explicitly requested model ID.
- Modify `README.md`: link the explicit workflow-proof command and make its evidence claim precise.

---

### Task 1: Add the Strict Workflow Scenario Contract

**Files:**
- Create: `crates/tree-ring-memory-core/tests/workflow_scenario.rs`
- Create: `crates/tree-ring-memory-core/src/workflow.rs`
- Modify: `crates/tree-ring-memory-core/src/lib.rs`

**Interfaces:**
- Produces `parse_workflow_scenario(input: &str) -> TreeRingResult<WorkflowScenario>`.
- Produces `WorkflowArm::{NoMemory, RawMemory, TreeRing}` with serde names `no_memory`, `raw_memory`, and `tree_ring`.
- Produces `WorkflowAgentRequest` containing only `schema_version`, `scenario_id`, `arm`, `task`, `workspace_root`, and `memory_context`.
- Produces `WorkflowAgentResponse { summary: String, used_memory_ids: Vec<String> }`; validation requires a nonblank summary and unique, nonblank IDs. The runner, not the response type, checks that cited IDs were actually supplied.
- Produces `evaluate_workspace(scenario: &WorkflowScenario, workspace_root: &Path) -> Vec<WorkflowFileCheckReport>`.

- [ ] **Step 1: Write the failing parser and boundary tests**

Create `crates/tree-ring-memory-core/tests/workflow_scenario.rs` with an inline valid JSON fixture containing one `workspace_files` entry, one `expected_files` entry, and no seed memories. Do not use the Task 3 fixture pack yet. Assert that parsing succeeds, a `../escape.txt` path is rejected, unknown top-level fields are rejected, and serializing `WorkflowAgentRequest` does not contain the actual expected-file text.

```rust
use tree_ring_memory_core::{parse_workflow_scenario, WorkflowAgentRequest, WorkflowArm};

const VALID_SCENARIO: &str = r#"{
  "name": "safe workflow",
  "task": "Prepare decision.md from the workspace.",
  "seed_memories": [],
  "workspace_files": [{"path": "proposal.md", "content": "draft"}],
  "expected_files": [{"path": "decision.md", "contains": "safe action"}]
}"#;

#[test]
fn parses_safe_workflow_fixture_and_keeps_validator_out_of_agent_request() {
    let scenario = parse_workflow_scenario(VALID_SCENARIO).unwrap();
    let request = WorkflowAgentRequest::new(
        scenario.name.clone(),
        WorkflowArm::NoMemory,
        scenario.task.clone(),
        "/tmp/trial".into(),
        Vec::new(),
    );
    let serialized = serde_json::to_string(&request).unwrap();
    assert!(!serialized.contains("safe action"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --locked -p tree-ring-memory-core --test workflow_scenario`

Expected: compilation failure because `parse_workflow_scenario`, `WorkflowAgentRequest`, and `WorkflowArm` do not exist.

- [ ] **Step 3: Implement the minimal core model**

Add `workflow.rs` with `#[serde(deny_unknown_fields)]` on every fixture-owned struct. The exact safe-path helper must reject absolute paths, empty paths, and any `Component::ParentDir`; it must allow nested relative paths. Use these types:

```rust
pub struct WorkflowScenario {
    pub name: String,
    pub task: String,
    #[serde(default)] pub seed_memories: Vec<MemoryEvent>,
    #[serde(default)] pub workspace_files: Vec<WorkflowWorkspaceFile>,
    #[serde(default)] pub expected_files: Vec<WorkflowFileExpectation>,
}

pub struct WorkflowAgentRequest {
    pub schema_version: u8,
    pub scenario_id: String,
    pub arm: WorkflowArm,
    pub task: String,
    pub workspace_root: PathBuf,
    pub memory_context: Vec<WorkflowMemoryContext>,
}

pub struct WorkflowAgentResponse {
    pub summary: String,
    #[serde(default)] pub used_memory_ids: Vec<String>,
}
```

`WorkflowScenario::validate` must require nonblank `name` and `task`, at least one expected file, unique workspace-file paths, unique expected-file `(path, contains)` pairs, valid seed memories, safe paths, and nonblank expected `contains` strings. `WorkflowMemoryContext` must contain `id`, `summary`, `details`, `ring`, `event_type`, `source_ref`, and `confidence`. `WorkflowAgentRequest::new` must not accept a `WorkflowScenario` or any validator data. `WorkflowAgentResponse::validate` must reject an empty summary, blank memory IDs, and duplicate memory IDs.

- [ ] **Step 4: Export and verify the core contract**

Export all public workflow types and `parse_workflow_scenario` from `lib.rs`. Run:

```bash
cargo test --locked -p tree-ring-memory-core --test workflow_scenario
cargo test --locked -p tree-ring-memory-core
```

Expected: all new workflow-contract tests and the existing core suite pass.

- [ ] **Step 5: Commit the core contract**

```bash
git add crates/tree-ring-memory-core/src/lib.rs crates/tree-ring-memory-core/src/workflow.rs crates/tree-ring-memory-core/tests/workflow_scenario.rs
git commit -m "feat: add workflow proof scenario contract"
```

### Task 2: Build the Paired Runner and Explicit Codex Adapter

**Files:**
- Create: `crates/tree-ring-memory-cli/src/lib.rs`
- Create: `crates/tree-ring-memory-cli/src/workflow_proof.rs`
- Create: `crates/tree-ring-memory-cli/examples/workflow_proof.rs`
- Create: `crates/tree-ring-memory-cli/tests/workflow_proof.rs`

**Interfaces:**
- Produces `run_workflow_proof(fixture_dir: &Path, output_dir: &Path, agent: &impl WorkflowAgent) -> Result<WorkflowProofReport, String>`.
- Produces `CodexWorkflowAgent::new(binary: PathBuf, model: Option<String>)`.
- Produces `workflow-proof-report.json`, `workflow-proof-summary.md`, and `trials/<scenario>/<arm>/` under the selected output directory.
- Consumes the Task 1 types and `MemoryRetriever` without changing SQLite or normal certification behavior.

- [ ] **Step 1: Write failing runner tests with a real fake agent**

Create `crates/tree-ring-memory-cli/tests/workflow_proof.rs`. Write the no-background-writer scenario into a test-owned `tempdir` as JSON; do not depend on the Task 3 fixture pack. Define a `FakeAgent` that receives `WorkflowAgentRequest`, writes `decision.md` only when its context includes `mem_quality_no_background_writer`, and returns that ID in `used_memory_ids`. Assert that:

1. `NoMemory` receives an empty context and fails the expected-file validator.
2. `RawMemory` and `TreeRing` receive only normal, non-superseded memory.
3. Tree Ring passes, the report records one observed lift over no-memory, and all three retained `trials/.../workspace` directories exist.
4. A fake response that cites an ID absent from `memory_context` is recorded as an error rather than counted as a pass.

- [ ] **Step 2: Run the runner test to verify it fails**

Run: `cargo test --locked -p tree-ring-memory-cli --test workflow_proof`

Expected: compilation failure because the CLI library and `workflow_proof` interfaces do not exist.

- [ ] **Step 3: Implement fair arm construction and deterministic validation**

Add a CLI library target exposing `workflow_proof`. In `run_workflow_proof`:

1. Read sorted `*.json` fixtures and parse them with `parse_workflow_scenario`.
2. For every scenario and arm, create `output_dir/trials/<safe-scenario>/<arm>/workspace`, materialize only `workspace_files`, and keep it after the run.
3. Build all arm prompts from the same task. `NoMemory` gets `[]`; `RawMemory` gets all visible seed memories in fixture order; `TreeRing` inserts the seed set into an in-memory SQLite store and uses `MemoryRetriever::recall(task, ..., 8, false)`.
4. Project memory into `WorkflowMemoryContext` with ID, summary, details, ring, event type, source ref, and confidence. Never expose `source.quote`.
5. Require every `used_memory_id` returned by the agent to be present in the request's `memory_context`; otherwise emit a trial error.
6. Apply `evaluate_workspace` after agent execution. A control failure is a normal `Fail`; a Tree Ring failure or error makes the example exit nonzero after reports are written.

Use report fields `schema_version`, `generated_at`, `scenario_count`, `trial_count`, `arm_summaries`, `scenarios`, `tree_ring_wins_over_no_memory`, `tree_ring_wins_over_raw_memory`, and `tree_ring_complete`. Name the comparison signal `observed` lift, never `proven` lift.

- [ ] **Step 4: Implement the explicit Codex adapter**

Define `WorkflowAgent` as:

```rust
pub trait WorkflowAgent {
    fn execute(&self, request: &WorkflowAgentRequest) -> Result<WorkflowAgentResponse, String>;
}
```

`CodexWorkflowAgent` must invoke exactly this shape (with an optional `--model` pair only when supplied):

```text
codex exec --ephemeral --sandbox workspace-write --cd <workspace>
  --output-schema <workspace>/.tree-ring-workflow-schema.json
  --output-last-message <workspace>/.tree-ring-workflow-response.json
  <constructed prompt>
```

The prompt must say: work only in the workspace, use source/task files over memory when they conflict, do not seek validators or fixtures, and return only the response schema fields `summary` and `used_memory_ids`. It must contain the task and a serialized `memory_context`, but never the expected-file checks.

The example parser must accept:

```text
workflow_proof <fixture-dir> <output-dir> [--codex-bin <path>] [--model <model>]
```

The default binary is `codex`; no invocation happens until a user runs this example.

- [ ] **Step 5: Run focused and full verification**

Run:

```bash
cargo test --locked -p tree-ring-memory-cli --test workflow_proof
cargo test --locked
cargo fmt --check
```

Expected: fake-agent proof coverage passes without invoking `codex` and the existing suite stays green.

- [ ] **Step 6: Commit the runner**

```bash
git add crates/tree-ring-memory-cli/src/lib.rs crates/tree-ring-memory-cli/src/workflow_proof.rs crates/tree-ring-memory-cli/examples/workflow_proof.rs crates/tree-ring-memory-cli/tests/workflow_proof.rs
git commit -m "feat: add paired agent workflow proof runner"
```

### Task 3: Add Reviewable Fixtures and Operator Documentation

**Files:**
- Create: `fixtures/workflow-proof/no-background-writer.json`
- Create: `fixtures/workflow-proof/stale-cli-contract.json`
- Create: `fixtures/workflow-proof/scar-recovery.json`
- Create: `docs/integrations/agent-workflow-proof.md`
- Modify: `README.md`

**Interfaces:**
- Fixtures use the Task 1 JSON contract and contain only normal, source-linked synthetic/project-safe memories.
- Documentation gives the exact explicit command, requires `--model <model-id>`, records the requested model identity in `workflow-proof-report.json`, and names that output as observed paired evidence rather than a universal benchmark score.

- [ ] **Step 1: Write failing fixture-validation coverage**

Add a test to `workflow_scenario.rs` that walks `fixtures/workflow-proof`, parses every JSON file, asserts all three required scenario names are present, and asserts each fixture has an expected file. In `examples/workflow_proof.rs`, factor the current argument parsing into a private `parse_cli_args` returning `Help` or `Run`, add a `#[cfg(test)]` unit test that `--help` returns `Help`, and make `run` print the exact usage text then exit zero for that result. This remains an example-only parser; it must not add a normal CLI subcommand or require Cargo example registration.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --locked -p tree-ring-memory-core --test workflow_scenario`

Expected: failure because `fixtures/workflow-proof` does not exist.

- [ ] **Step 3: Add the three fixtures**

Each fixture must seed one or two normal memories, create a small initial workspace file, and require a resulting `decision.md` content fragment. Use these decision outcomes:

| Fixture | Required decision fragment | Tree Ring mechanism under test |
|---|---|---|
| `no-background-writer` | `no hidden durable writer` | constraint recall |
| `stale-cli-contract` | `remember needs --event-type` | current rule beats superseded rule |
| `scar-recovery` | `roll back the cache migration` | failure scar changes recovery workflow |

Keep the task prompt neutral: it asks the agent to inspect the workspace and prepare `decision.md`; it must not state the required fragment. Every memory source must point to an existing repo file or a clearly labeled fixture evidence URI.

- [ ] **Step 4: Document reproducibility and limits**

Create `docs/integrations/agent-workflow-proof.md` containing:

```bash
cargo run --locked -p tree-ring-memory-cli --example workflow_proof -- \
  fixtures/workflow-proof target/tree-ring-certification/workflow-proof
```

Document the three arms, retained workspace evidence, no automatic Codex invocation in CI, required `--model <model-id>` plus model/version/commit capture, no claim beyond the specific controlled fixtures, and the next step of running external benchmark adapters. The model identity must be visible in both `workflow-proof-report.json` and `workflow-proof-summary.md`; do not make it an operator-only side note.

Add a concise README link under certification/evidence documentation.

- [ ] **Step 5: Run fixture and documentation verification**

Run:

```bash
cargo test --locked -p tree-ring-memory-core --test workflow_scenario
cargo test --locked -p tree-ring-memory-cli --test workflow_proof
cargo fmt --check
git diff --check
```

Expected: the fixtures parse, the fake agent proves the runner mechanics, and no documentation or whitespace error remains.

- [ ] **Step 6: Commit the fixture pack and docs**

```bash
git add fixtures/workflow-proof docs/integrations/agent-workflow-proof.md README.md crates/tree-ring-memory-core/tests/workflow_scenario.rs crates/tree-ring-memory-cli/tests/workflow_proof.rs
git commit -m "docs: add agent workflow proof fixtures"
```

### Task 4: Execute the Explicit Real-Agent Proof and Capture Evidence

**Files:**
- Generated only: `target/tree-ring-certification/workflow-proof/`

**Interfaces:**
- Consumes the explicit example and the default local `codex` executable.
- Produces inspectable trial directories, JSON report, and Markdown summary without changing tracked source files.

- [ ] **Step 1: Build and run the runner against the local Codex CLI**

Run:

```bash
rm -rf target/tree-ring-certification/workflow-proof
cargo run --locked -p tree-ring-memory-cli --example workflow_proof -- \
  fixtures/workflow-proof target/tree-ring-certification/workflow-proof
```

Expected: all nine paired trials complete, the report exists, and the process exits zero only when every Tree Ring arm satisfies its deterministic validator.

- [ ] **Step 2: Inspect evidence before interpreting it**

Run:

```bash
sed -n '1,260p' target/tree-ring-certification/workflow-proof/workflow-proof-report.json
find target/tree-ring-certification/workflow-proof/trials -maxdepth 4 -type f | sort
```

Expected: report context IDs differ only by arm; no-memory trials have empty contexts; each retained workspace exposes the validator-observable file state.

- [ ] **Step 3: Run the full release-quality suite**

Run:

```bash
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets
git diff --check
```

Expected: all commands exit zero. Do not run `scripts/certify-tree-ring.sh` as a substitute for the explicit agent proof; it intentionally does not invoke Codex.

---

## Plan Self-Review

- **Spec coverage:** Task 1 prevents validator leakage and unsafe fixtures; Task 2 creates fair paired contexts, real explicit Codex execution, deterministic outcomes, retained traces, and partial-failure reports; Task 3 adds concrete Tree Ring workflow scenarios and truthful docs; Task 4 produces real observed evidence only after code is validated.
- **Scope check:** No public eval subcommand, hidden agent execution, new storage backend, telemetry, or UI changes are introduced.
- **Type consistency:** Core owns `WorkflowScenario`, `WorkflowArm`, `WorkflowAgentRequest`, `WorkflowAgentResponse`, and file-check reports. CLI owns `WorkflowAgent`, `CodexWorkflowAgent`, and `WorkflowProofReport`.
- **Placeholder scan:** All file paths, interfaces, command shapes, scenario names, required decision fragments, and verification commands are explicit.
