# Tree Ring Harness Activation And Behavioral Proof Design

## Status

Approved direction: `tree-ring init` becomes a one-command, project-local
activation flow. It must make supported coding harnesses aware of Tree Ring,
verify that a new harness session actually performs a recall before substantive
work, and preserve the existing same-host multi-agent safety model.

## Relationship To Earlier Bridge Design

This design replaces the activation semantics in
`2026-07-06-tree-ring-agent-mediated-bridges-design.md`.

`integrations link` remains a useful low-level and dry-run operation, but a
normal `tree-ring init` must now invoke safe project-level activation itself.
Generating `.tree-ring` guidance or finding a marker directory is not enough to
call a harness active.

## Intent

After a user runs this from a project root:

```bash
tree-ring init
```

Tree Ring should do the ordinary setup automatically:

1. Initialize the project-local memory root and canonical guidance.
2. Discover compatible harnesses through versioned adapters.
3. Install safe, project-local native bridge files or managed instruction
   blocks.
4. Record exactly what was configured and what still requires platform consent.
5. On the first new session for each configured harness, run a privacy-safe,
   scoped recall preflight and record a behavioral receipt.

The user should not need to copy skills, edit configuration, select worker IDs,
or run a separate certification command for ordinary use. Platform-owned
consent, such as trusting a Pi project or installing the separate Agent Zero
plugin, is the only expected user action. Those cases must be one concise,
actionable next step rather than a configuration tutorial.

## Goals

- Make project-local Tree Ring activation automatic for each compatible
  detected harness.
- Reserve the status `active` for a verified preflight receipt, never a marker
  scan or generated file alone.
- Preserve `.tree-ring` as the canonical project guidance and storage root.
- Support Codex, Claude Code, Pi, Agent Zero, and other harnesses through a
  versioned adapter contract rather than one fragile universal configuration.
- Keep an extensible path for Hermes, OpenCode, Goose, and future harnesses.
- Give multi-agent workers distinct identity and prove whether they share the
  same project store.
- Keep durable writes explicit and preserve the existing coordinator policy for
  shared promotion, lifecycle mutations, and fan-in.
- Make activation, receipts, and deactivation reversible without deleting
  memories.
- Keep the default experience to one command and quiet session startup.

## Non-Goals

- Do not change Agent Zero core. Agent Zero integration extends the separate
  `tree_ring_memory` plugin only.
- Do not create a daemon, capture transcripts, or persist prompts simply to
  prove activation.
- Do not write global harness configuration during project initialization.
- Do not overwrite user-owned harness configuration or instruction files.
- Do not claim that local SQLite is safe across hosts or network filesystems.
- Do not treat a usage receipt as a security boundary against a user who can
  alter local files or process state.
- Do not silently create a separate Agent Zero store and call it shared with the
  project root.

## Activation Vocabulary

| State | Meaning |
| --- | --- |
| `active` | A native bridge exists and a current-session receipt proves scoped recall succeeded. |
| `configured-awaiting-proof` | Tree Ring safely installed the bridge; no qualifying new session has yet reported a receipt. |
| `active-isolated` | A harness completed preflight against its own accessible store, but that store does not match the project store. It is not a cross-harness shared workflow. |
| `needs-trust` | The harness requires its own user trust or approval before project resources can load. |
| `needs-project-mount` | The harness can run Tree Ring but cannot reach the canonical project store. |
| `needs-plugin` | The Agent Zero adapter requires the separate plugin to be installed or enabled. |
| `needs-user-review` | Existing unmanaged configuration makes automatic modification unsafe. |
| `unsupported` | No maintained adapter can prove this harness integration. |
| `failed` | Detection, bridge installation, preflight, or receipt verification failed with a concrete diagnostic. |

`configured-awaiting-proof`, `active-isolated`, and every `needs-*` state are
deliberately distinct from `active`.

## Command Experience

### Default Flow

`tree-ring init` performs canonical initialization plus safe activation. Its
normal human output is short: list each detected harness, its state, and at
most one required action. It does not print a bridge tutorial.

The intended supporting commands are:

```bash
tree-ring init
tree-ring integrations status
tree-ring integrations status --verbose
tree-ring integrations certify
tree-ring integrations deactivate --harness <id>
```

`status --verbose` exposes bridge paths, adapter version, receipt age, root
topology, and an actionable error. `certify` produces durable JSON and Markdown
evidence; it is not required for ordinary initialization. `deactivate` removes
only Tree Ring-owned bridge files or bounded managed blocks.

### Advanced Flow

Advanced users may request a dry-run, an individual harness, an explicit
recheck, or coordinated policy. Those options must not be required for default
activation:

```bash
tree-ring init --dry-run
tree-ring integrations activate --harness claude-code --dry-run
tree-ring integrations certify --live
tree-ring --root .tree-ring policy enable --coordinator release-coordinator
```

The policy command remains explicit because it emits a one-time capability.
Initialization must not nominate a coordinator or expose that capability.

## Components

### Canonical Project Root

`.tree-ring/` remains the source of truth for:

- `memory.sqlite` and its SQLite sidecars;
- generated `AGENTS.md`, `SKILL.md`, and `CLI.md` guidance;
- `activation.json`, the versioned activation manifest;
- bounded, non-sensitive activation receipts under `activation/receipts/`.

`activation.json` contains a stable `store_id`, project-root fingerprint,
Tree Ring CLI version, activation schema version, and one record per adapter.
The `store_id` is created once for a new project root and lets a container path
and a host path prove they refer to the same store without recording either
path in receipts.

### Adapter Registry

The CLI owns a versioned registry of harness adapters. Every adapter implements
the following conceptual operations:

1. **Detect** exact project signals, installed version, and activation
   capability. A directory by itself is never sufficient.
2. **Plan** all bridge writes and explain any platform requirement.
3. **Install** only owned files or Tree Ring managed blocks.
4. **Preflight** a new harness session using native hooks, configuration, or an
   approved launch wrapper.
5. **Verify** the resulting receipt and shared-store fingerprint.
6. **Deactivate** only the files and blocks it owns.

An adapter declares one of three evidence capabilities:

- `native-preflight`: it can run and report recall from a harness lifecycle
  boundary; this can become `active`.
- `wrapper-preflight`: it can prove recall only when the harness is launched by
  Tree Ring's optional wrapper; this is active for wrapper-launched sessions.
- `guidance-only`: it can install portable instructions but cannot prove use;
  it remains `configured-awaiting-proof` or `unsupported`, never `active`.

Third-party adapters use the same contract and fixtures. This is the practical
meaning of broad harness support: every harness can gain a first-class adapter,
but no unsupported harness receives a false universal-compatibility claim.

### Native Bridge Ownership

The generated bridge is intentionally thin. It points to canonical
`.tree-ring/SKILL.md`, `.tree-ring/AGENTS.md`, and `.tree-ring/CLI.md` rather
than copying long instructions or memory data.

Safe automatic writes include absent Tree Ring-owned paths such as a dedicated
skill directory. Existing files can be changed only inside a clearly bounded
Tree Ring managed block. Parsed settings files, such as Pi project settings,
must preserve unrelated valid content. Invalid, conflicting, or unmanaged
configurations result in `needs-user-review` with a precise diff preview.

The initial maintained adapters have these targets:

- **Codex:** a project skill bridge under `.agents/skills/tree-ring-memory/`
  plus a safe project instruction bridge when the adapter's live capability
  probe requires it.
- **Claude Code:** `.claude/skills/tree-ring-memory/` plus an owned project
  instruction or hook configuration only when it can be merged safely.
- **Pi:** the portable `.agents/skills` bridge and project `.pi` resource
  configuration, subject to Pi's own project-trust decision.
- **Agent Zero:** the separate `tree_ring_memory` plugin and its per-project,
  per-agent configuration; no generic `.a0` marker bridge is a substitute.
- **Hermes, OpenCode, Goose, and future harnesses:** a maintained native adapter
  when available, otherwise an explicit non-active state and adapter authoring
  path.

## Preflight And Receipt Flow

For the first substantive task in a new session, the native bridge executes:

1. Read `activation.json` and verify the adapter version and store state.
2. Derive project, worker, workflow, and session identity from the harness, not
   model-provided values.
3. Build a non-persisted, redacted task hint when available; otherwise use the
   stable query `project startup constraints`.
4. Run scoped recall with sensitive results excluded by default.
5. Inject only the safe recall result into the harness startup context.
6. Atomically write a receipt after successful command completion and context
   injection.

The receipt stores the adapter/harness versions, `store_id`, project identity,
worker/workflow/session identifiers, query class rather than raw prompt text,
result count, a digest of selected memory IDs, duration, and status. It never
stores prompt text, recalled summaries, sensitive values, secrets, or
coordinator capabilities.

A zero-result recall is a valid receipt: it proves the agent checked Tree Ring
without fabricating context. A failed command, failed context injection,
timeout, or mismatched store ID creates no active receipt.

Receipts are operational observability, not durable memories. Retain at most
the most recent 100 receipts per harness/worker and no receipt older than 30
days. Receipt writes use atomic replace semantics.

## Agent Zero Integration

The existing public `tree_ring_memory` Agent Zero plugin is the sole Agent Zero
runtime adapter. It already owns Agent Zero context mapping, per-project and
per-agent configuration, Rust CLI compatibility, version checks, automatic
bootstrap, safe paths, and coordinator-aware tools. This feature adds a
matching activation protocol to that plugin; it does not duplicate its runtime
or touch Agent Zero core.

When `tree-ring init` detects Agent Zero:

1. Verify the plugin manifest and compatible activation-protocol version.
2. If the plugin is absent or disabled, return `needs-plugin` with the one
   installation/enable action.
3. If it is present, create or update the owned project binding.
4. The plugin derives server-side Agent Zero identity, runs the preflight, and
   writes/verifies the receipt through the project mount.

Agent Zero can be `active-isolated` if its configured store is not the
canonical project root. It becomes shared only when its plugin configuration
reaches the mounted project `.tree-ring` root and emits the same `store_id` as
the host harnesses. Tree Ring must never copy data into or out of the plugin's
default store merely to make this status appear green.

The plugin must remain version-pinned to a compatible Tree Ring CLI and retain
its current migration gate. An incompatible or upgrade-required store is
`failed` or `needs-user-review`, not initialized during activation.

## Multi-Agent Coordination

Every bridge derives or receives a unique `agent_profile`, a shared
`workflow_id` for fan-out/fan-in, and a unique `session_id` for each real
attempt. Each worker preflights independently and emits its own receipt.

Open policy remains backward compatible. In coordinated policy:

- ordinary workers write only non-heartwood agent-scoped observations;
- a designated coordinator performs wider fan-in recall;
- shared project/workflow publication, evidence promotion, destructive
  lifecycle actions, imports, consolidation, and applied maintenance require
  the existing coordinator capability;
- the capability stays in the authorized host environment and never appears in
  bridges, receipts, logs, API input, or memory.

The activation report marks a workflow `shared` only when every participating
receipt reports the canonical project's `store_id`. The supported concurrency
claim stays bounded to concurrent processes on one host and a local filesystem.

## Error Handling And Lifecycle

- Missing or incompatible CLI/plugin: report `failed` with the required
  version and exact detected value.
- Platform trust required: report `needs-trust`; do not alter global trust
  settings or bypass a native approval prompt.
- Project store unavailable to a container: report `needs-project-mount`; do
  not create a covert replacement store.
- Existing unmanaged configuration: report `needs-user-review`, show a
  non-destructive plan, and leave it unchanged.
- Missing, expired, malformed, or failed receipt: remain
  `configured-awaiting-proof`; never reuse an old receipt after an adapter,
  harness, or store change.
- Existing incompatible SQLite schema: preserve the current explicit offline
  upgrade and backup workflow; `init` must not migrate it as part of activation.
- Adapter update or root fingerprint change: atomically update its manifest
  record and invalidate prior receipts.
- `deactivate`: remove only paths and bounded managed blocks recorded in the
  manifest; retain the database, canonical guidance, and historical receipts.

No activation failure writes durable memory or exposes sensitive recalled data.

## Verification Strategy

### Unit And Contract Tests

Add focused Rust tests for:

- adapter capability/version detection, including rejection of marker-only
  false positives;
- idempotent owned-file creation, managed-block updates, parsed settings merge,
  and no-overwrite behavior;
- activation manifest and receipt validation, expiry, bounded retention, and
  atomic updates;
- `store_id` agreement and intentional isolated-store classification;
- redaction of prompts, recall content, secrets, and capabilities from receipts;
- deactivation that removes only owned bridge material;
- worker identity propagation, concurrent receipt writes, and coordinator-only
  fan-in/promotion in coordinated mode.

### Harness Fixtures

Every maintained adapter supplies a hermetic launch fixture that proves:

1. a fresh project runs `tree-ring init`;
2. the adapter installs its native bridge safely;
3. a new harness session executes preflight;
4. seeded project memory is injected as safe context;
5. the receipt verifies against the same canonical `store_id`.

Also test zero-result recall, expired receipt, denied trust, unavailable mount,
unmanaged configuration, and unknown harness. CI fixtures must not call a
marker scan a behavioral pass.

### Live Evidence

`tree-ring integrations certify` emits machine-readable records and a concise
compatibility report. It distinguishes configured from active and shared from
isolated. A `--live` mode may run installed harnesses where the maintainer has
them available; absence of an optional local harness is a skip, not evidence of
support.

The Agent Zero proof remains split across repositories:

- this repository verifies the activation protocol and adapter contract;
- `tree-ring-memory-agent-zero` verifies plugin configuration, server-derived
  identity, preflight/receipt behavior, and a live Agent Zero runtime path.

## Documentation Requirements

After implementation proves the exact command surface, update:

- README quick start and compatibility matrix;
- `docs/integrations/agent-skill.md`;
- generated CLI, skill, and agent guidance;
- CLI help and JSON schemas;
- Agent Zero plugin README and marketplace-facing compatibility statement.

Documentation must distinguish `active`, `configured-awaiting-proof`,
`active-isolated`, and all blocking states. It must say that a receipt proves
preflight usage, not an adversarial security guarantee or automatic durable
memory writes.

## Acceptance Criteria

1. `tree-ring init` performs safe project-local adapter activation without a
   separate required link command.
2. A marker directory alone can never make a harness `active` or pass
   certification.
3. Every supported adapter has a versioned detection rule, owned bridge plan,
   preflight mechanism, receipt verifier, and fixture.
4. A harness becomes `active` only after a new session completes scoped recall,
   safe context injection, and receipt validation.
5. A zero-result recall produces a valid usage receipt without inventing
   context.
6. Receipts contain no raw prompt, recalled summary, secret, sensitive value,
   or coordinator capability.
7. Existing unmanaged harness files are not changed; ambiguous setup is
   `needs-user-review`.
8. All automatic writes remain within the project; global configuration is
   unchanged.
9. Missing platform trust, plugin installation, or project mount yields a
   single concise action and never a false active status.
10. Agent Zero uses the existing plugin only and does not require an Agent Zero
    core change.
11. Multi-agent receipts carry distinct worker identity and prove shared status
    only for the same project `store_id` on a same-host local filesystem.
12. Coordinated-policy authorization remains explicit and coordinator-only.
13. `deactivate` removes only manifest-owned bridge material and preserves
    memories.
14. `integrations certify` produces JSON and Markdown evidence that separates
    configured, active, isolated, blocked, skipped, and failed states.
15. Focused unit, fixture, multi-agent, Agent Zero plugin, formatting, and
    diff-whitespace checks pass before documentation claims behavioral support.
