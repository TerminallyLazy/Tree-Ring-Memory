# Tree Ring Agent-Mediated Bridges Design

## Status

Approved direction: agent-mediated bridge linking. Tree Ring Memory should make
itself visible to the agent harness a user is already using, while keeping
`.tree-ring` as the canonical memory root and avoiding hidden background
recording.

## Intent

When a user initializes Tree Ring Memory in a project, the active coding agent
should know that Tree Ring exists, when to recall memory, when to store durable
lessons, and which CLI commands are safe to use.

The current init flow creates `.tree-ring/AGENTS.md`, `.tree-ring/SKILL.md`,
`.tree-ring/CLI.md`, and `memory.sqlite`. That is enough for Tree Ring itself,
but not enough for every agent harness to discover the guidance. Harnesses look
in different places: Codex and Gemini can discover `.agents/skills`, Claude Code
uses `CLAUDE.md` and `.claude/skills`, OpenCode reads `AGENTS.md`, Pi reads
`.pi/settings.json` resource paths, and other agents may only understand a root
project instruction file.

Tree Ring should therefore create small, explicit bridge files that point the
current harness at the canonical `.tree-ring` guidance instead of duplicating
memory data or scraping sessions.

## Goals

- Keep `.tree-ring` the canonical storage and generated guidance root.
- Add project-level bridge linking for common agent harnesses.
- Keep global bridge linking explicit and opt-in.
- Make bridge writes non-destructive and reviewable.
- Tell the active agent when to call Tree Ring deliberately.
- Update README, CLI help, generated `.tree-ring/CLI.md`, generated
  `.tree-ring/AGENTS.md`, generated `.tree-ring/SKILL.md`, and integration docs
  so the command surface stays visible.
- Preserve the privacy boundary: no transcript scraping, no hidden daemon, and
  no autonomous durable writes outside explicit agent or user actions.

## Non-Goals

- Do not create a background recorder.
- Do not read private chat transcripts or shell history.
- Do not auto-modify global agent configuration during `tree-ring init`.
- Do not overwrite existing project instruction files.
- Do not make Tree Ring memory more authoritative than source files, tests,
  root agent contracts, explicit user instructions, DOX contracts, or evaluated
  evidence.
- Do not implement a sidecar or MCP server in this change.

## Current Behavior

`tree-ring init` creates the SQLite store and the three generated awareness
files under `.tree-ring`. `tree-ring integrations scan` detects likely harnesses
and prints next steps, but it does not write harness-native bridge files.

Durable memory writes currently happen only when a user, agent, adapter, import,
TUI action, consolidation command, or explicit maintenance command calls Tree
Ring. The TUI has real-time display updates through SQLite store-watch polling
and optional JSONL event-stream pulses, but event-stream pulses are display
signals only and are not durable memory.

## Design

### Command Surface

Add a link command under the existing integrations namespace:

```bash
tree-ring integrations link --scope project --harness auto
tree-ring integrations link --scope project --harness codex
tree-ring integrations link --scope global --harness codex
tree-ring integrations link --scope project --harness claude-code --dry-run
```

Defaults:

- `--scope project`
- `--harness auto`
- non-destructive writes
- readable summary output
- `--json` support through the existing CLI JSON mode

`tree-ring init` should remain safe and predictable. It may print the recommended
project link command after initialization, but project bridge writes should be
performed by `tree-ring integrations link` unless the final implementation adds
an explicit init flag such as `tree-ring init --link-project`.

### Project Bridges

Project bridges are checked into or kept with the current repository. They make
the current project self-describing to the agent harness that opens it.

Codex and Gemini:

- Preferred shared path: `.agents/skills/tree-ring-memory/SKILL.md`
- The bridge skill should point back to `.tree-ring/SKILL.md` and `.tree-ring/CLI.md`.
- If `.agents/skills/tree-ring-memory/SKILL.md` already exists, do not overwrite
  it. Report it as existing and show the manual merge path.

Claude Code:

- Preferred skill path: `.claude/skills/tree-ring-memory/SKILL.md`
- Preferred instruction bridge: `CLAUDE.md` importing or referencing
  `.tree-ring/AGENTS.md` and `.tree-ring/CLI.md`
- If `CLAUDE.md` already exists, append only inside a Tree Ring managed block or
  report a manual patch. Do not rewrite unrelated Claude instructions.

OpenCode:

- Preferred instruction bridge: root `AGENTS.md`
- If root `AGENTS.md` exists, append only inside a Tree Ring managed block or
  report a manual patch.
- Optional later enhancement: `opencode.json` `instructions` entry pointing at
  `.tree-ring/AGENTS.md` and `.tree-ring/CLI.md`.

Pi:

- Preferred project bridge: `.pi/settings.json` with a local skill/resource path
  that points at a Tree Ring skill bridge.
- Preserve unrelated Pi settings by parsing and merging JSON instead of string
  editing.
- If merging safely is not possible, report a manual patch.

Generic:

- Provide a root `AGENTS.md` bridge only when explicitly selected.
- The root bridge should be small and should direct the agent to read
  `.tree-ring/SKILL.md` and `.tree-ring/CLI.md`.

### Global Bridges

Global bridges affect all projects and must never be written by ordinary
project initialization.

Examples:

- `~/.agents/skills/tree-ring-memory/SKILL.md`
- `~/.codex/AGENTS.md`
- `~/.claude/skills/tree-ring-memory/SKILL.md`
- `~/.gemini/skills/tree-ring-memory/SKILL.md`
- `~/.pi/agent/settings.json`

Global linking requires `--scope global`. The command should print the target
paths before writing, support `--dry-run`, and refuse destructive overwrites.

### Bridge Content

Every bridge should be short. Its job is discovery, not duplicating the full
Tree Ring manual.

Required bridge instructions:

- Read `.tree-ring/SKILL.md` before using Tree Ring Memory.
- Use `.tree-ring/CLI.md` as the local command reference.
- Recall before substantial project work, risky changes, user corrections,
  repeated workflows, and closeout.
- Remember only durable, useful, privacy-safe lessons.
- Use `tree-ring evidence` for evaluated outcomes.
- Use `tree-ring forget` or redaction when memory is wrong, stale, or sensitive.
- Run `tree-ring consolidate --dry-run` and `tree-ring maintain` as explicit
  review surfaces, not hidden automatic jobs.
- Never store secrets, credentials, private keys, raw chain-of-thought, or
  transcript dumps.

Commands that must stay visible:

```bash
tree-ring recall "project startup warnings"
tree-ring remember "Use project-scoped recall before risky changes." --event-type lesson --scope project
tree-ring evidence "A promoted evaluation fixed stale state." --outcome promoted --evidence-ref evals/run-042
tree-ring forget mem_example --mode redact --reason "remove sensitive detail"
tree-ring consolidate --period-type manual --dry-run
tree-ring maintain
tree-ring integrations scan --source-root .
tree-ring integrations link --scope project --harness auto --dry-run
tree-ring tui
```

### Agent-Mediated Memory Updates

Tree Ring should not autonomously update durable memory in the background for
this version.

The agent-mediated contract is:

1. Bridge files teach the active agent when memory is useful.
2. The agent calls Tree Ring CLI commands deliberately.
3. Tree Ring applies validation, sensitivity checks, storage, recall ranking,
   consolidation, or maintenance according to the command invoked.
4. The TUI may display live store-watch and event-stream signals, but display
   signals do not become durable memory unless an explicit write command stores
   them.

This preserves portability across Codex, Claude Code, Gemini CLI, Pi, OpenCode,
and generic `AGENTS.md` agents without a daemon or privileged background
collector.

## Data Flow

Project linking flow:

1. User runs `tree-ring init`.
2. Tree Ring creates `.tree-ring` awareness files if missing.
3. User runs `tree-ring integrations link --scope project --harness auto`.
4. Tree Ring scans project and home markers to identify likely harnesses.
5. Tree Ring plans bridge file writes.
6. Dry-run prints proposed paths and managed blocks.
7. Apply writes missing bridge files or safe managed blocks.
8. Future agent sessions discover Tree Ring through harness-native paths.

Memory update flow:

1. Agent reads its bridge file.
2. Agent recalls relevant memory with `tree-ring recall`.
3. Agent works from source evidence and user instructions.
4. Agent stores durable lessons with `tree-ring remember` or evaluated outcomes
   with `tree-ring evidence`.
5. Agent uses `tree-ring forget`, redaction, consolidation, and maintenance only
   when explicitly appropriate.

## Error Handling

- Existing unmanaged files are never overwritten.
- Existing managed Tree Ring blocks can be replaced only if bounded by clear
  begin/end markers.
- JSON settings files are parsed before modification. Invalid JSON produces a
  controlled error and manual patch text.
- Global writes require `--scope global`.
- Unsupported harness ids produce a list of supported ids.
- `--dry-run` reports all planned writes without touching the filesystem.
- Commands return structured JSON under `--json`.

## Documentation Requirements

Update:

- README installation and Agent Workflow Integration sections.
- `docs/integrations/agent-skill.md`.
- Generated `.tree-ring/CLI.md` reference in `agent_awareness.rs`.
- Generated `.tree-ring/AGENTS.md` guidance in `agent_awareness.rs`.
- `skills/tree-ring-memory/SKILL.md`.
- CLI help for `tree-ring integrations link`.
- Rust architecture status after implementation.

Docs must explain:

- Project scope vs global scope.
- Why project scope is recommended.
- Why global scope is opt-in.
- Agent-mediated updates vs background recording.
- Which bridge paths are used for each harness.
- How to preview changes with `--dry-run`.

## Testing

Add focused Rust tests for:

- Project link dry-run creates no files.
- Project link writes missing bridge files.
- Existing bridge files are reported and not overwritten.
- Managed blocks are updated without touching surrounding content.
- Global link refuses to run unless `--scope global` is explicit.
- Claude `CLAUDE.md` bridge imports or references Tree Ring guidance safely.
- Codex/Gemini `.agents/skills` bridge is discoverable by path.
- Pi `.pi/settings.json` merge preserves unrelated settings.
- JSON output includes planned, created, existing, skipped, and warning fields.
- Generated `.tree-ring` guidance mentions `integrations link`.

Run:

```bash
cargo test --locked
git diff --check
```

## Acceptance Criteria

1. `tree-ring integrations link --scope project --harness auto --dry-run`
   reports planned harness bridges without writing files.
2. `tree-ring integrations link --scope project --harness codex` creates a
   Codex/Gemini-compatible `.agents/skills/tree-ring-memory/SKILL.md` bridge
   when missing.
3. `tree-ring integrations link --scope project --harness claude-code` creates
   or safely updates Claude project guidance without overwriting unrelated
   content.
4. `tree-ring integrations link --scope global --harness codex --dry-run`
   reports global targets and writes nothing.
5. Ordinary `tree-ring init` does not modify global agent configuration.
6. README and generated CLI guidance explain project vs global linking.
7. Generated agent guidance explains that durable memory updates are
   agent-mediated, not hidden background recording.
8. Focused Rust tests and `cargo test --locked` pass.
9. `git diff --check` passes.
