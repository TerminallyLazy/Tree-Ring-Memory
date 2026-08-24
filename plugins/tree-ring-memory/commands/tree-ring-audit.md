---
description: Audit consolidate or forget stale sensitive or superseded Tree Ring Memory entries
argument-hint: "[audit focus]"
allowed-tools: ["Bash"]
---

# Tree Ring Audit

Audit memory when work is closing, when privacy may matter, or when older
entries may be stale.

Read project-local `.tree-ring/SKILL.md` and `.tree-ring/CLI.md` first when
present. Confirm `tree-ring --version` reports 0.14.0 or newer. If the runtime
is missing or older, stop and explain the limitation; do not install, upgrade,
or invent results without explicit user permission.

Before any current command other than `policy status` or `policy audit` opens
an existing pre-v0.13 store, stop every Tree Ring process, checkpoint and back
up the complete store, and upgrade every CLI, plugin, and bundled worker. All
mixed-version operation is unsupported.

These policy preflight commands never create or migrate a store:

```bash
tree-ring policy status
tree-ring policy audit --limit 100
```

After the verified schema-v3 upgrade, start with non-mutating inspection:

```bash
tree-ring audit --audit-type sensitive
tree-ring consolidate --period-type manual --dry-run
tree-ring maintain
```

If the user names a focus, use it to narrow recall before changing memory:

```bash
tree-ring recall "$ARGUMENTS"
```

Use explicit forget operations only when justified:

```bash
tree-ring forget mem_example --mode redact --reason "remove sensitive detail"
tree-ring forget mem_example --mode delete --reason "should not be retained"
```

The CLI forget modes are `redact` and `delete`. For an explicit supersession,
use the TUI's `/supersede <old_id>` lifecycle action. Clarify the exact memory
identifier and mutation before running any lifecycle write.

In Coordinated mode, forget/redact, supersede, persisted consolidation, and
applied maintenance such as `--repair-fts` require
`TREE_RING_COORDINATOR_TOKEN` in the coordinator process environment. Never put
the capability value in a prompt, CLI argument, memory, log, source reference,
or committed file. `policy status`, `policy audit`, ordinary audit,
consolidation dry-run, and plain `maintain` do not change memory content on an
already-upgraded schema-v3 store. Only the two policy commands are guaranteed
not to create or migrate a store.

Do not delete, redact, or supersede memory without a clear reason. The
coordinator policy is not a read ACL, and its shared-root support is bounded to
cooperative processes on one host using a local filesystem.
