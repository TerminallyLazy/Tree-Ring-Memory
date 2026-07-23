# Agent Skill And Project Contract

Tree Ring Memory ships two integration aids for agent workflows:

- `skills/tree-ring-memory/SKILL.md`: a portable agent skill that teaches an agent when to recall, remember, redact, forget, and avoid memory capture.
- `templates/dox/AGENTS.md`: a DOX-style project contract template for repos that want Tree Ring Memory guidance alongside source code.

`tree-ring init` and `tree-ring welcome --init` also create local copies in the
configured memory root:

- `.tree-ring/AGENTS.md`
- `.tree-ring/SKILL.md`
- `.tree-ring/CLI.md`

Existing files are not overwritten.

These generated files are the canonical project-local guidance. Harness-native
bridge files should point back to them rather than copying memory data or
duplicating long instructions.

## Skill Usage

Use the skill in agent runtimes that support local skills or instruction packs.
The skill is framework-agnostic and does not assume any single host runtime, model provider, CLI, or orchestration framework.

Recommended activation moments:

- project start or resume
- user says "remember this"
- user asks what was decided
- user corrects the agent
- a repeated mistake appears
- a durable decision is made
- a future idea should be tracked
- work is closing out

## Project Contract Usage

Use `templates/dox/AGENTS.md` when a repo wants local memory rules.
Copy it to the project root as `AGENTS.md`, or merge its sections into an existing project contract.

The contract intentionally says that Tree Ring Memory is not authoritative over source docs.
Agents should still read local project instructions and source evidence directly.

The CLI does not modify a project root `AGENTS.md` automatically. Merge the
generated `.tree-ring/AGENTS.md` guidance manually when you want DOX-aware
agents to encounter Tree Ring Memory instructions before entering `.tree-ring/`.

## Minimal CLI Flow

```bash
tree-ring init
tree-ring recall "project startup warnings"
tree-ring remember "Use protocol-first design." --event-type decision --scope project --tag architecture
tree-ring evidence "Snapshot invalidation fixed stale unread chat state." --outcome promoted --evidence-ref evals/chat-state/run-042 --score 0.91
tree-ring dox sync --source-root . --dry-run
tree-ring revolve sync --source-root revolve --dry-run
tree-ring integrations scan --source-root .
tree-ring forget mem_example --mode redact --reason "remove sensitive detail"
tree-ring maintain
```

For a project-local install, use the generated quick reference in
`.tree-ring/CLI.md` and pass the project memory root explicitly when needed:

```bash
.tree-ring/bin/tree-ring --root .tree-ring recall "project startup warnings"
.tree-ring/bin/tree-ring --root .tree-ring tui
```

## Multi-Agent Fan-Out And Fan-In

Tree Ring can coordinate multiple CLI workers that share one local memory root
on one host. The coordinator chooses one project, workflow ID, and session ID.
Every worker gets a unique agent profile and a stable operation ID for each
logical write:

```bash
tree-ring --root .tree-ring init

tree-ring --root .tree-ring remember "Storage worker validated WAL behavior." \
  --event-type lesson \
  --scope agent \
  --project example-service \
  --agent-profile worker-storage \
  --workflow-id release-readiness \
  --session-id attempt-1 \
  --operation-id validate-storage-v1 \
  --source-ref runs/release-readiness/worker-storage.json \
  --tag coordination
```

Repeat the worker command with a different `--agent-profile`,
`--operation-id`, and source reference. Keep `--workflow-id` and
`--session-id` shared across that fan-out. An exact retry of one logical write
reuses its original session and operation IDs. Rotate the session and assign new
operation IDs only when starting a genuinely new execution attempt.

Scope determines the required partition identity:

| Scope | Required identity | Intended use |
| --- | --- | --- |
| `agent` | `agent_profile` | One worker's partitioned task memory |
| `workflow` | `workflow_id` | Shared state for one fan-out/fan-in |
| `session` | `session_id` | State partitioned by execution attempt |
| `project` | None; `project` is recommended | Shared repository memory |
| `global` | None | Deliberate cross-project memory |

The CLI also reads `TREE_RING_AGENT_PROFILE`, `TREE_RING_WORKFLOW_ID`, and
`TREE_RING_SESSION_ID`. Explicit flags are easier to audit in retained worker
commands. If the coordinator uses environment defaults, clear the agent-profile
default before recalling all workers; otherwise it becomes an unintended recall
filter.

At fan-in, recall worker results through the shared dimensions:

```bash
tree-ring --root .tree-ring --json recall "release readiness" \
  --project example-service \
  --workflow-id release-readiness \
  --session-id attempt-1 \
  --scope agent \
  --limit 64
```

Inspect each memory's source reference before writing a coordinator-owned
summary. Use `scope=workflow` with the same workflow ID for shared workflow
state, or `scope=project` for a reviewed durable project conclusion:

```bash
tree-ring --root .tree-ring remember "Release readiness checks passed." \
  --event-type summary \
  --scope workflow \
  --project example-service \
  --agent-profile coordinator \
  --workflow-id release-readiness \
  --session-id attempt-1 \
  --operation-id fan-in-summary-v1 \
  --source-ref runs/release-readiness/fan-in.json
```

Consolidation accepts `--agent-profile`, `--workflow-id`, and `--session-id`
filters. Agent-, workflow-, and session-scoped inputs remain partitioned;
consolidation does not silently merge memories from different partition
identities. A coordinator that wants a shared conclusion should write the
explicit source-linked summary shown above.

### Idempotency

`operation_id` is an idempotency key inside the
`(project, workflow_id, agent_profile)` namespace:

- An exact retry with the same payload returns the existing memory ID and does
  not add a row.
- Reusing the key in that namespace for a different payload fails nonzero.
- `session_id` is retained context but is not part of the idempotency namespace;
  changing only the session makes the retry conflict.
- Omitting `operation_id` creates an ordinary new write.
- Consolidation-derived summaries do not inherit a source operation ID.
- Replacing an active memory ID preserves its old operation namespace as a
  one-way claim.
- Redaction keeps both the one-way operation claim and a memory-ID tombstone,
  so retries and replacement imports fail closed without restoring redacted
  content. Only hard deletion removes those claims.

Pre-0.12 `agent`, `workflow`, or `session` records that lack the now-required
identity are assigned a deterministic, per-record `legacy-*` identity during
migration/import and marked for review. They remain privately partitioned and
portable instead of being widened into project/global scope.

The identifiers must be nonblank, contain no control characters, and stay at or
below 256 characters. These fields are routing and correlation metadata, not an
authorization boundary. A process with filesystem access to the SQLite store
can read it, so host permissions still control access.

### Runtime Boundary And Evidence

The supported shared-root pattern is concurrent processes on the same host
using a local filesystem. SQLite WAL, bounded lock retries, and a busy timeout
handle local contention. Tree Ring is not a distributed database or lock
service, and this evidence does not establish safe shared-database operation
over NFS, network filesystems, containers on different hosts, or multiple
machines. Use per-host roots and an explicit export/import or other
evidence-preserving coordinator when work spans hosts.

`crates/tree-ring-memory-cli/tests/multi_agent_acceptance.rs` is a bounded
process-level acceptance test. It launches eight real CLI writers against one
root, exercises profile/workflow/session/scope recall filters, verifies exact
retry and conflicting-key behavior, and checks memory-row/FTS parity through
the JSON maintenance report. It is evidence for the same-host contract only;
it is not a sustained-load, crash-recovery, fairness, or distributed-storage
certification.

## Evidence-Driven Improvement

Use `tree-ring evidence` when a lesson comes from an evaluation, checkpoint,
experiment, branch, incident, or reviewed run artifact.

Outcome mapping:

- `promoted` creates durable heartwood from supported evidence.
- `rejected` creates a scar when a failed or rolled-back approach has reusable warning value.
- `deferred` creates a seed for a promising but unresolved option.
- `observed` creates an outer-ring evaluation result.

Plain `remember` is still appropriate for user preferences, explicit decisions,
and project lessons that do not come from a formal evaluated outcome.

## Memory Quality Gates

Tree Ring guidance is meant to improve agent behavior, not increase memory volume.
Use these gates when wiring Tree Ring into an agent harness:

- Recall gates: before substantial or risky work, recall constraints, scars, preferences, and unresolved seeds.
- Trust gates: prefer source-linked, non-superseded, high-confidence memories and re-read authoritative sources when memory conflicts with source files or user instructions.
- Write gates: reject transient planning chatter, duplicate wording, tool noise, and unsupported claims; require evidence refs for promoted or rejected evaluated outcomes.

The certification suite includes quality scenarios that exercise missed constraints, memory spam, stale truth suppression, and behavior proof.
Quality artifacts are written to
`target/tree-ring-certification/quality/quality-report.json` and
`target/tree-ring-certification/quality/quality-summary.md`.

## Source Adapter Flow

Use DOX and Revolve adapters when the source artifacts already exist locally:

```bash
tree-ring dox sync --source-root . --dry-run
tree-ring revolve sync --source-root revolve --dry-run
```

The adapters are Rust-native and local-only. They create concise, source-linked
memory events through the same SQLite store as manual memories. They do not
modify root `AGENTS.md` files, rewrite DOX contracts, mutate Revolve records,
or import raw run-log bloat.

DOX adapter rules:

- Scan a project root or a single `AGENTS.md` file.
- Store concise summaries and source refs.
- Treat source `AGENTS.md` files as authoritative.
- Re-read the DOX chain before editing files.

Revolve adapter rules:

- Scan a Revolve root or an evidence file.
- Import promoted outcomes as heartwood.
- Import reusable rejected outcomes as scars.
- Import deferred hypotheses as seeds.
- Import observed results as outer-ring evidence.
- Ignore outcome-free files as durable truth.

Run `--dry-run` first, inspect the generated memories, then rerun without
`--dry-run` only when the summaries are useful and source-linked.

## Agent Harness Notes

Tree Ring Memory is framework-agnostic. For agent harnesses that support local
skills, add `skills/tree-ring-memory/SKILL.md` or the generated
`.tree-ring/SKILL.md` to startup context. For DOX-aware harnesses, merge the
generated `.tree-ring/AGENTS.md` guidance into the project root `AGENTS.md`
when you want agents to see memory rules before entering the memory directory.
For CLI-only harnesses, include `.tree-ring/CLI.md` in startup context and call
`tree-ring --help` when command flags are uncertain.

Recommended project-level bridge targets:

- Codex and Gemini-style skill loaders: `.agents/skills/tree-ring-memory/SKILL.md`
  pointing to `.tree-ring/SKILL.md` and `.tree-ring/CLI.md`.
- Claude Code: `.claude/skills/tree-ring-memory/SKILL.md` plus a `CLAUDE.md`
  reference to `.tree-ring/AGENTS.md` and `.tree-ring/CLI.md`.
- OpenCode and DOX-style agents: a root `AGENTS.md` managed block or manual
  section that tells the agent to read `.tree-ring/AGENTS.md`,
  `.tree-ring/SKILL.md`, and `.tree-ring/CLI.md`.
- Pi: a `.pi/settings.json` resource path that points at the Tree Ring skill or
  CLI guidance.

Project bridge files are preferred because they stay scoped to the current
repo. Global bridge files under `~/.agents`, `~/.codex`, `~/.claude`,
`~/.gemini`, or `~/.pi` affect every project and should be written only through
an explicit global opt-in flow.

The bridge-linking design is agent-mediated: bridge files teach the active
agent when to call Tree Ring, but Tree Ring does not run a background recorder
or autonomously persist chat transcripts. Durable writes happen only when a
user, agent, adapter, import, TUI action, consolidation command, or explicit
maintenance command calls the CLI.

`tree-ring integrations scan --source-root .` is read-only today. The planned
`tree-ring integrations link --scope project --harness auto --dry-run` command
will preview bridge writes first, then write only missing files or safe managed
blocks. Until that command is implemented, add the bridge references manually.

## Safety Rule

When in doubt, do not store the memory.
Prefer a short, redacted, source-linked lesson over detailed sensitive capture.
