# Agent Zero Activation Protocol Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the separate `tree_ring_memory` Agent Zero plugin so it recognizes a project initialized by `tree-ring init`, performs server-identity preflight recall through the mounted canonical store, writes a compatible receipt, and reports a precise non-active state when the plugin, mount, or protocol is unavailable.

**Architecture:** Keep all Agent Zero work inside `/Users/lazy/Projects/tree-ring-memory-agent-zero`; do not modify Agent Zero core. The plugin reads the core repository's protocol-1 activation manifest through a configured project mount, validates that its configured store is the mounted `<project>/.tree-ring` root, and delegates all recall and receipt work to the `tree-ring` CLI. A dedicated preflight tool and API action give the Agent Zero prompt a privacy-safe first-action path; plugin hooks and Web UI expose state without creating a substitute store.

**Tech Stack:** Python 3.12+, existing Agent Zero plugin hooks/tools/API/Web UI, subprocess JSON bridge to Tree Ring `0.14.x`, pytest, Node syntax checks, protocol-1 JSON files from the core implementation plan.

## Global Constraints

- This is a companion plan for the separate Agent Zero plugin repository at `/Users/lazy/Projects/tree-ring-memory-agent-zero`.
- Release plugin version `3.1.0`, require Tree Ring `0.14.0` through `0.14.x`, and require activation protocol `1`.
- Do not change Agent Zero core, add an `execute.py` compatibility shim, create a second memory engine, or copy memories between plugin and project stores.
- The plugin must derive `agent_profile`, `workflow_id`, and `session_id` from the Agent Zero server context. It must not accept those identity values from API or tool payloads.
- Activation preflight uses the safe fallback query `project startup constraints`; it does not send raw user task text, transcript content, coordinator capability material, or secrets to the CLI.
- A plugin whose configured root differs from the mounted `<project>/.tree-ring` root is `active-isolated`, never shared. A missing mount is `needs-project-mount`; a malformed/incompatible manifest is `failed` or `needs-user-review`.
- The plugin remains a same-host local-SQLite integration. It makes no cross-host, network-filesystem, or adversarial-local-user guarantee.
- Normal plugin bootstrap remains idempotent for existing unbound deployments. An explicit project activation binding that is unavailable must fail closed and must not bootstrap a replacement default store.
- Never expose, log, store, return, render, or forward `TREE_RING_COORDINATOR_TOKEN`.

---

## File Structure

| Path in `tree-ring-memory-agent-zero` | Responsibility |
| --- | --- |
| `plugin.yaml` | Publish version `3.1.0` and retain project/agent config support. |
| `default_config.yaml` | Declare activation protocol and project-binding defaults. |
| `helpers/config.py` | Normalize the trusted project-mount setting and require Tree Ring `0.14.0`. |
| `helpers/paths.py` | Resolve and validate the mounted project and activation-manifest path without creating it. |
| `helpers/activation.py` | Parse core protocol-1 manifest/binding, classify state, and redact diagnostics. |
| `helpers/cli.py` | Add read-only activation status and stdin-based preflight calls to the core CLI. |
| `hooks.py` | Surface binding readiness during plugin bootstrap without initializing an isolated root for an explicit project binding. |
| `api/memory_api.py` | Add `activation_status` and context-gated `preflight` API actions. |
| `tools/preflight.py` | Agent-facing first-action preflight tool using server-derived identity. |
| `tools/_common.py` | Expose the shared activation-aware bridge construction path. |
| `prompts/tree-ring-memory.md` | Require the preflight tool before substantial project work and preserve source-authority rules. |
| `webui/memory-store.js`, `webui/main.html`, `webui/config.html` | Show redacted activation status, the exact next action, and no mount-writing control. |
| `tests/test_activation.py` | Binding parser, root mismatch, CLI stdin, receipt, and real-core protocol coverage. |
| `tests/test_cli.py`, `tests/test_api.py`, `tests/test_hooks.py`, `tests/test_config_paths.py`, `tests/test_webui.py`, `tests/test_manifest.py` | Update existing behavior and release-version expectations. |
| `.github/workflows/build-bundled-binaries.yml` | Build Linux plugin binaries from exact Tree Ring `v0.14.0` release provenance. |
| `bin/SHA256SUMS`, `bin/linux-aarch64/*`, `bin/linux-x86_64/*` | Regenerated, verified `0.14.0` artifacts only after the core release tag exists. |
| `README.md` | Explain automatic project binding, plugin/mount states, the preflight tool, and same-host limit. |

### Task 1: Add a Strict Project Activation Binding Model

**Files:**

- Modify: `default_config.yaml`
- Modify: `helpers/config.py`
- Modify: `helpers/paths.py`
- Create: `helpers/activation.py`
- Create: `tests/test_activation.py`
- Modify: `tests/test_config_paths.py`

**Interfaces:**

- Produces `ACTIVATION_PROTOCOL_VERSION = 1`, `ActivationBinding`, `ActivationBindingStatus`, and `load_activation_binding(config)`.
- Produces `paths.activation_project_root(config)` and `paths.activation_manifest_path(config)`; both are read-only and return no default plugin repository path.
- Consumes core `.tree-ring/activation.json` and `.tree-ring/activation/agent-zero.json` only after validating the configured mount.

- [ ] **Step 1: Write failing binding tests**

Create tests that make the project root and storage root explicit:

```python
def test_binding_requires_an_explicit_project_mount(tmp_path):
    config = load_config({"storage": {"root": str(tmp_path / "memory")}})
    status = load_activation_binding(config)
    assert status.state == "needs-project-mount"
    assert status.binding is None

def test_binding_rejects_a_store_outside_the_mounted_project(tmp_path):
    project = write_protocol_one_project(tmp_path / "project")
    config = load_config({
        "storage": {"root": str(tmp_path / "isolated")},
        "activation": {"project_root": str(project)},
    })
    status = load_activation_binding(config)
    assert status.state == "active-isolated"
    assert status.store_id == "store-fixture"
```

Add cases for malformed JSON, unsupported schema/protocol, relative traversal, correct mounted root, and an Agent Zero binding with a different `store_id`.

- [ ] **Step 2: Run the focused binding tests and verify they fail before the model exists**

Run: `PYTHONPATH="$PWD" PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider tests/test_activation.py tests/test_config_paths.py`

Expected: FAIL because `helpers.activation` and `activation` config are absent.

- [ ] **Step 3: Implement the binding/parser contract**

Add this normalized default to `default_config.yaml` and `DEFAULT_CONFIG`:

```yaml
activation:
  enabled: true
  protocol_version: 1
  project_root: null
```

`load_config` must resolve the project root in this precedence order: `TREE_RING_MEMORY_PROJECT_ROOT`, explicit `activation.project_root`, then `scope.allowed_project_root` only when that value was explicitly supplied by the per-project configuration. It must not silently use `REPO_ROOT` as an activation mount.

Use these exact public shapes:

```python
@dataclass(frozen=True)
class ActivationBinding:
    project_root: Path
    memory_root: Path
    manifest_path: Path
    store_id: str
    project_root_fingerprint: str
    protocol_version: int

@dataclass(frozen=True)
class ActivationBindingStatus:
    state: str
    binding: ActivationBinding | None
    next_step: str
    error: str | None = None
```

Resolve the configured root and `project_root / ".tree-ring"` with `Path.resolve()`. A valid shared binding requires equality. If both roots exist but differ, return `active-isolated`; if project root, manifest, or mount cannot be reached, return `needs-project-mount`; if the manifest is malformed, version-incompatible, or its Agent Zero binding disagrees, return `failed`. The parser may return `store_id`, state, and concise diagnostics, but never raw receipt content, capabilities, or user prompts.

- [ ] **Step 4: Run the binding test suite**

Run: `PYTHONPATH="$PWD" PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider tests/test_activation.py tests/test_config_paths.py`

Expected: PASS for all root/mount/protocol classifications without creating a database or manifest.

- [ ] **Step 5: Commit the project-binding boundary**

```bash
git add default_config.yaml helpers/config.py helpers/paths.py \
  helpers/activation.py tests/test_activation.py tests/test_config_paths.py
git commit -m "feat: validate Agent Zero project activation binding"
```

### Task 2: Add CLI Delegation for Status and Server-Derived Preflight

**Files:**

- Modify: `helpers/cli.py`
- Modify: `tests/test_cli.py`
- Modify: `tests/test_activation.py`

**Interfaces:**

- Consumes `ActivationBinding` from Task 1 and existing `InvocationContext`.
- Produces `TreeRingCli.activation_status(binding)` and `TreeRingCli.preflight_activation(binding)`.
- Produces `_run_json_stdin(args, payload, protected=False)` without placing payload content on argv.

- [ ] **Step 1: Write failing CLI bridge tests**

Use the existing injected runner to assert exact command and stdin behavior:

```python
def test_preflight_sends_only_server_derived_identity_over_stdin(tmp_path):
    calls = []
    bridge = TreeRingCli(config(tmp_path / "project" / ".tree-ring", executable(tmp_path)),
                          context=InvocationContext("reviewer", "proj", "fanout-7", "session-9"),
                          runner=recording_runner(calls))
    result = bridge.preflight_activation(binding(tmp_path))
    assert result["state"] == "active"
    command, kwargs = calls[-1]
    assert command[-5:] == ["preflight", "--harness", "agent-zero", "--input-json-stdin", "--context-format", "json"]
    payload = json.loads(kwargs["input"])
    assert payload["agent_profile"] == "reviewer"
    assert "task_hint" not in payload
    assert "TREE_RING_COORDINATOR_TOKEN" not in kwargs["env"]
```

Also test that a nonshared binding returns its state without invoking `tree-ring preflight`, and that activation status invokes `integrations status --source-root <project>` without creating a store.

- [ ] **Step 2: Run focused CLI tests and verify they fail before the new methods exist**

Run: `PYTHONPATH="$PWD" PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider tests/test_cli.py tests/test_activation.py`

Expected: FAIL with missing activation methods and stdin support.

- [ ] **Step 3: Implement core command delegation without weakening existing safeguards**

Set `SUPPORTED_TREE_RING_VERSION = "0.14.0"`. Keep `_assert_compatible` minor-pinned so the plugin accepts `0.14.0` through `0.14.x` only.

Implement:

```python
def activation_status(self, binding: ActivationBinding) -> dict[str, Any]:
    return _require_dict(self._run_json([
        "integrations", "status", "--source-root", str(binding.project_root)
    ]), "activation status")

def preflight_activation(self, binding: ActivationBinding) -> dict[str, Any]:
    if binding.memory_root.resolve() != self.root.resolve():
        return {"state": "active-isolated", "store_id": binding.store_id}
    return _require_dict(self._run_json_stdin(
        ["integrations", "preflight", "--harness", "agent-zero",
         "--input-json-stdin", "--context-format", "json"],
        self._activation_identity_payload(),
    ), "activation preflight")
```

`_activation_identity_payload` must contain only server-derived identity and optional project name from `InvocationContext`; omit task hints, capabilities, secrets, and raw context. `_run_json_stdin` must pass `input=<json>` to `subprocess.run`, preserve the existing timeout/error/redaction behavior, and remove every identity/capability environment variable before invocation. It must parse/redact the JSON response exactly as `_run_json` does.

- [ ] **Step 4: Run CLI bridge tests**

Run: `PYTHONPATH="$PWD" PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider tests/test_cli.py tests/test_activation.py`

Expected: PASS for current recall/write behavior plus activation command construction, root mismatch short-circuit, capability redaction, and incompatible `0.13.x` rejection.

- [ ] **Step 5: Commit CLI protocol delegation**

```bash
git add helpers/cli.py tests/test_cli.py tests/test_activation.py
git commit -m "feat: delegate Agent Zero preflight to tree-ring"
```

### Task 3: Expose a Safe Preflight Tool and Context-Gated API Actions

**Files:**

- Create: `tools/preflight.py`
- Modify: `tools/_common.py`
- Modify: `api/memory_api.py`
- Modify: `prompts/tree-ring-memory.md`
- Modify: `tests/test_api.py`
- Modify: `tests/test_activation.py`

**Interfaces:**

- Consumes Task 2's `TreeRingCli.preflight_activation` and server-derived `InvocationContext`.
- Produces the agent tool class `Preflight` and API actions `activation_status` and `preflight`.
- Keeps identity payload fields unavailable to callers and keeps `preflight` non-mutating except for the core-owned receipt.

- [ ] **Step 1: Write failing tool/API behavior tests**

Add an API fake bridge with `activation_status` and `preflight_activation`, then assert:

```python
def test_preflight_requires_an_existing_agent_zero_context(monkeypatch):
    handler, fake = handler_with_fake(monkeypatch)
    result = asyncio.run(handler.process({"action": "preflight"}, None))
    assert result["ok"] is False
    assert "context_id is required" in result["error"]
    assert fake.calls == []

def test_activation_status_is_read_only_and_returns_one_next_step(monkeypatch):
    handler, fake = handler_with_fake(monkeypatch)
    result = asyncio.run(handler.process({"action": "activation_status"}, None))
    assert result["data"]["state"] == "needs-project-mount"
    assert result["data"]["next_step"]
    assert fake.calls == [("activation_status", {})]
```

Write a tool test that attaches a fake Agent Zero agent, calls `Preflight.execute()`, and verifies the plugin sends no task text or caller-supplied identity to the CLI wrapper.

- [ ] **Step 2: Run the focused plugin API tests and verify they fail before dispatch exists**

Run: `PYTHONPATH="$PWD" PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider tests/test_api.py tests/test_activation.py`

Expected: FAIL because `preflight` and `activation_status` are unknown actions.

- [ ] **Step 3: Implement the agent-facing proof path**

Create `tools/preflight.py` following the existing `Recall` tool shape. It must call `bridge_and_config(getattr(self, "agent", None))`, obtain the binding through `load_activation_binding`, call `bridge.preflight_activation(binding)`, and return `tool_success(response, "Tree Ring preflight complete.")`. It accepts no identity, project root, prompt, or capability parameters.

In `MemoryApi.process`:

- `activation_status` builds the bridge without a user context and returns the binding state plus CLI status when accessible;
- `preflight` requires `context_id`, resolves the Agent Zero context with the existing `_bridge(..., require_context=True)` path, and calls the preflight method;
- recursive capability field rejection applies before either action;
- an unshared/missing binding returns its exact non-active state without calling `ensure_initialized`.

Replace the first paragraph of `prompts/tree-ring-memory.md` with a concise operational rule: call `preflight` once before substantial or risky project work; it supplies safe scoped recall and writes a behavioral receipt; a zero-result preflight is valid; then re-read current project sources and instructions. Keep the existing rules about privacy, durable writes, coordinator policy, and same-host scope.

- [ ] **Step 4: Run tool/API tests**

Run: `PYTHONPATH="$PWD" PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider tests/test_api.py tests/test_activation.py tests/test_context.py`

Expected: PASS. The only preflight identity comes from `InvocationContext.from_agent`; no request field can impersonate a worker or coordinator.

- [ ] **Step 5: Commit the Agent Zero preflight surface**

```bash
git add tools/preflight.py tools/_common.py api/memory_api.py \
  prompts/tree-ring-memory.md tests/test_api.py tests/test_activation.py
git commit -m "feat: add Agent Zero activation preflight tool"
```

### Task 4: Make Bootstrap and Web UI Report Binding State Without Creating a Replacement Store

**Files:**

- Modify: `hooks.py`
- Modify: `api/memory_api.py`
- Modify: `webui/memory-store.js`
- Modify: `webui/main.html`
- Modify: `webui/config.html`
- Modify: `tests/test_hooks.py`
- Modify: `tests/test_webui.py`
- Modify: `tests/test_api.py`

**Interfaces:**

- Consumes binding classification from Task 1 and read-only activation status from Task 2.
- Produces a `status["activation"]` payload with `state`, `store_id` only when safe, `receipt_age_seconds`, and `next_step`.
- Preserves the current `bootstrap_runtime` return shape while adding `activation` and never auto-initializing a replacement root for an explicit broken binding.

- [ ] **Step 1: Write failing bootstrap and UI source tests**

Add a hook regression test with an explicit project binding that has no mount:

```python
def test_explicit_missing_project_binding_does_not_bootstrap_default_store(tmp_path, monkeypatch):
    bridge = FakeBridgeThatFailsIfInitRuns()
    monkeypatch.setattr(hooks, "TreeRingCli", lambda config: bridge)
    report = hooks.bootstrap_runtime(project_binding_config(tmp_path / "missing"))
    assert report["ready"] is False
    assert report["activation"]["state"] == "needs-project-mount"
    assert bridge.calls == ["status"]
```

Extend `test_webui.py` to require the safe activation rendering terms and forbid UI mount writers:

```python
assert "activationLabel" in store
assert "activation.next_step" in main + config
assert "TREE_RING_MEMORY_PROJECT_ROOT" not in store
assert "coordinator capability" not in main.lower()
```

- [ ] **Step 2: Run bootstrap/UI tests and verify they fail before state is exposed**

Run: `PYTHONPATH="$PWD" PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider tests/test_hooks.py tests/test_webui.py tests/test_api.py`

Expected: FAIL for missing activation data and UI rendering methods.

- [ ] **Step 3: Implement readiness composition and concise status rendering**

In `bootstrap_runtime`, call `load_activation_binding` before `paths.ensure_memory_dirs`. If configuration explicitly selects an activation project and the binding is not shared/usable, return `{"ok": True, "ready": False, "activation": ...}` after the existing non-mutating CLI status probe. Do not call `bridge.init`, migration, audit, or directory creation for that condition. Preserve current behavior for existing deployments with no activation project selection.

On regular status responses, add a redacted `activation` object. The Web UI must render one compact line such as `Project activation: active`, `Project activation: needs project mount`, or `Project activation: active isolated`, plus the server-provided next step. It may provide a refresh button. It must not expose a filesystem picker, mutate project mounts, install plugins, or display receipt contents/capabilities.

Use a pure `activationLabel()` formatter in `memory-store.js`; keep the existing runtime/schema and policy labels intact. Update main/config templates to use `x-text` for the label and next step, escaping through the existing framework binding rather than interpolating HTML.

- [ ] **Step 4: Run Python tests and JavaScript syntax check**

Run: `PYTHONPATH="$PWD" PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider tests/test_hooks.py tests/test_webui.py tests/test_api.py`

Run: `node --check webui/memory-store.js`

Expected: PASS. A broken explicit binding never creates a default store, and the UI exposes only redacted state.

- [ ] **Step 5: Commit binding visibility**

```bash
git add hooks.py api/memory_api.py webui/memory-store.js webui/main.html \
  webui/config.html tests/test_hooks.py tests/test_webui.py tests/test_api.py
git commit -m "feat: report Agent Zero project activation state"
```

### Task 5: Verify the Cross-Repository Protocol and Ship Matching Binaries

**Files:**

- Modify: `plugin.yaml`
- Modify: `default_config.yaml`
- Modify: `helpers/config.py`
- Modify: `tests/test_manifest.py`
- Modify: `.github/workflows/build-bundled-binaries.yml`
- Modify: `bin/SHA256SUMS`
- Modify: `bin/linux-aarch64/PROVENANCE.txt`
- Modify: `bin/linux-aarch64/tree-ring`
- Modify: `bin/linux-x86_64/PROVENANCE.txt`
- Modify: `bin/linux-x86_64/tree-ring`
- Modify: `README.md`
- Modify: `tests/test_activation.py`

**Interfaces:**

- Consumes the core repository's `v0.14.0` tag and protocol document from `docs/protocol/harness-activation.md`.
- Produces plugin release `3.1.0` with two verified native binaries and a real-core preflight test.

- [ ] **Step 1: Write failing version, provenance, and real-core test coverage**

Update manifest tests to expect:

```python
assert manifest["version"] == "3.1.0"
assert defaults["cli"]["required_version"] == "0.14.0"
assert provenance["source_tag"] == "v0.14.0"
assert provenance["binary_version"] == "tree-ring 0.14.0"
```

Add a real-core test guarded only by a missing explicit executable, not by a success claim:

```python
@pytest.mark.skipif(not os.environ.get("TREE_RING_MEMORY_CLI"), reason="explicit core binary not supplied")
def test_real_core_agent_zero_preflight_uses_the_project_store(tmp_path):
    project = initialize_protocol_one_project_with_real_cli(tmp_path)
    bridge = TreeRingCli(shared_project_config(project), context=fixture_context())
    response = bridge.preflight_activation(load_activation_binding(bridge.config).binding)
    assert response["state"] == "active"
    assert response["receipt"]["store_id"] == response["store_id"]
```

- [ ] **Step 2: Run version/protocol tests and verify they fail before the release update**

Run: `PYTHONPATH="$PWD" PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider tests/test_manifest.py tests/test_activation.py`

Expected: FAIL because the checked-in release metadata and bundled binaries still identify `0.13.0`.

- [ ] **Step 3: Complete the matching core release before regenerating artifacts**

From the core repository after the core implementation plan passes all verification, create and push the immutable release tag:

```bash
git tag -a v0.14.0 -m "Tree Ring 0.14.0 harness activation"
git push origin v0.14.0
```

Update the plugin binary workflow to check out `ref: v0.14.0`, assert the checked-out `HEAD` equals `git rev-list -n 1 v0.14.0`, build/test the CLI, and assert `target/release/tree-ring --version` is exactly `tree-ring 0.14.0`. Write the resolved 40-character tag commit into each `PROVENANCE.txt` during the workflow rather than hardcoding an unknown future commit.

Run the workflow for `linux-x86_64` and `linux-aarch64`, verify SHA-256 checksums, replace only `bin/linux-*/tree-ring`, `PROVENANCE.txt`, and `SHA256SUMS`, then rerun the provenance test. This produces artifact evidence tied to the immutable core tag and keeps the plugin's minor-version gate honest.

- [ ] **Step 4: Run the complete plugin and real-core verification sequence**

Run: `TREE_RING_MEMORY_CLI=/absolute/path/to/tree-ring-0.14.0 PYTHONPATH="$PWD" PYTHONDONTWRITEBYTECODE=1 python3 -m pytest -q -p no:cacheprovider tests`

Run: `node --check webui/memory-store.js`

Run: `git diff --check`

Expected: PASS. The real-core test proves an Agent Zero context can issue preflight against the mounted project store and records an activation receipt without a copied store.

- [ ] **Step 5: Commit and document the compatible plugin release**

```bash
git add plugin.yaml default_config.yaml helpers/config.py tests/test_manifest.py \
  .github/workflows/build-bundled-binaries.yml bin README.md tests/test_activation.py
git commit -m "release: support tree-ring 0.14 activation protocol"
```

## Completion Evidence

From a clean plugin worktree with a real `tree-ring 0.14.0` binary supplied:

```bash
TREE_RING_MEMORY_CLI=/absolute/path/to/tree-ring-0.14.0 \
PYTHONPATH="$PWD" PYTHONDONTWRITEBYTECODE=1 \
python3 -m pytest -q -p no:cacheprovider tests
node --check webui/memory-store.js
git diff --check
```

Review a successful `activation_status` payload, one preflight response, and one generated core receipt. Confirm their identities are server-derived, their store IDs match the mounted project root, their JSON includes no prompt/recall/capability content, and a mismatched root reports `active-isolated` rather than shared.
