<!-- tree-ring:begin codex v1 -->
## Tree Ring Memory

Read `.tree-ring/AGENTS.md`, `.tree-ring/SKILL.md`, and `.tree-ring/CLI.md` before substantive work. The project lifecycle hook runs receipt-backed recall at session boundaries and one strict automatic capture checkpoint before a session or subagent turn stops. Do not claim memory is active without a valid project-local recall receipt. Capture zero to three concise durable candidates; never store a transcript or invent memory.
<!-- tree-ring:end codex -->

# Tree Ring Memory repository

Tree Ring is a Rust workspace for local, lifecycle-aware agent memory. Read
`README.md` for commands and `docs/protocol/harness-activation.md` for activation.
Root ownership covers Cargo manifests, `install.sh`, schemas, shared assets,
marketplace descriptors, `.github/workflows`, and the canonical `skills/` guide.

## DOX framework

This repository uses [agent0ai/DOX](https://github.com/agent0ai/dox), adapted from
revision `765ae4ac02cc884eefcd41a3d0f71941721adb89` (MIT), as its project instruction hierarchy.

### Read before editing

1. Read this root file and identify the paths involved in the task.
2. Walk from the root to each target, reading every applicable child `AGENTS.md`
   and following its Child DOX Index before editing.
3. Parent contracts remain in force. The nearest child adds local detail; it
   cannot weaken the DOX workflow or repository-wide authority and safety rules.
4. Re-read the applicable chain in the current session. Recalled memory is a
   navigation aid, never a substitute for the current source and instructions.

### Keep the hierarchy current

After meaningful changes, review the owning document and its parents. Update
purpose, ownership, contracts, workflows, inputs, outputs, side effects, and
verification when they change. Refresh affected child indexes and remove stale
or contradictory text. Document durable rules, not session transcripts.

Create a child at a durable responsibility boundary. Use these sections in
order: Purpose, Ownership, Local Contracts, Work Guidance, Verification, Child
DOX Index. Guidance and verification describe existing requirements and checks;
leave them empty when none exist rather than inventing gates.

Before finishing, re-check changed paths against the hierarchy, run relevant
existing verification, and report intentionally unchanged docs with the reason.

### Instruction and memory separation

The root and child DOX files describe this repository. `.tree-ring/AGENTS.md`,
`.tree-ring/SKILL.md`, and `.tree-ring/CLI.md` describe the local memory runtime.
Keep both sets of instructions: preserve existing root content and Tree Ring's
small marked reference block. Never replace one `AGENTS.md` with the other.
Local memory databases, receipts, private credentials, and generated runtime
files are not DOX documents to publish. Installed third-party skills retain
their own instructions; do not recursively rewrite their contents.

## Work and verification

Keep behavior in the Rust core, SQLite implementation, and shared CLI actions.
Preserve project-local storage, identity scopes, privacy filtering, create-only
bridge publication, and receipt-backed activation. A binary, plugin, configured
hook, or marker alone does not prove live memory use.

For runtime changes run `cargo test --workspace --locked` and
`cargo fmt --all -- --check`. Package changes use
`python3 scripts/validate-plugin-packages.py`; installer edits use `sh -n install.sh`.
Run host probes for lifecycle changes and verify published archive checksums for
releases. Report source, packaged release, installed runtime, actual hook proof,
and external marketplace submission as distinct outcomes.

## Child DOX Index

- [crates/AGENTS.md](crates/AGENTS.md) — Rust workspace and crate boundaries.
- [plugins/AGENTS.md](plugins/AGENTS.md) — Codex and Claude packaging and lifecycle hooks.
- [scripts/AGENTS.md](scripts/AGENTS.md) — Validation, certification, installer and release tooling.
- [docs/AGENTS.md](docs/AGENTS.md) — Published documentation, protocol contracts and design records.
- [fixtures/AGENTS.md](fixtures/AGENTS.md) — Synthetic acceptance, parity and quality data.
- [marketing/AGENTS.md](marketing/AGENTS.md) — Campaign source material and visual assets.
- [templates/dox/AGENTS.md](templates/dox/AGENTS.md) — Existing downstream Tree Ring project-contract template; not this repository’s root contract.
