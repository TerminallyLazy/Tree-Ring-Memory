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

## Safety Rule

When in doubt, do not store the memory.
Prefer a short, redacted, source-linked lesson over detailed sensitive capture.
