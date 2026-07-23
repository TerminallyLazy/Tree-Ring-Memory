# Tree Ring Memory v0.13.0

Tree Ring Memory v0.13.0 adds a bounded same-host multi-agent contract and an
optional coordinator policy for shared publication and lifecycle changes.

## What Ships

- Agent, workflow, session, operation, and source correlation through the Rust
  API and CLI.
- Exact-retry idempotency with conflict detection.
- Real multi-process acceptance coverage with eight concurrent CLI workers.
- Opt-in Coordinated mode: ordinary workers can create only matching
  non-heartwood agent-scoped memory.
- Coordinator-only shared/non-agent writes, heartwood, import, persisted
  adapters and consolidation, lifecycle changes, and applied maintenance.
- One-time coordinator capabilities supplied only through
  `TREE_RING_COORDINATOR_TOKEN`, stored as hashes, and blocked from memory and
  audit/status metadata.
- Transactional protected-write audit records with unsafe targets hashed and
  terminal-safe human output.
- SQLite schema v3 with forward-version rejection and a fence for memory
  inserts, updates, and deletes from old writers.
- Read-only policy status and audit commands that never create or migrate a
  store.
- Agent-aware TUI writes and visible, nonfatal authorization denials.

## Supported Boundary

The concurrency and authorization evidence covers cooperative Tree Ring Rust
and CLI processes sharing a SQLite store on one host and a local filesystem. It
does not establish a read ACL, cross-host coordination, network-filesystem
safety, or protection from an adversary who controls the database files or
process environment.

## Upgrade From v0.12

Schema v3 is a coordinated upgrade:

1. Stop every Tree Ring CLI, TUI, plugin, and bundled worker using the root.
2. Checkpoint SQLite WAL state and make a verified complete store backup.
3. Upgrade every CLI, plugin, and bundled worker to a v0.13-compatible build.
4. Reopen the root with v0.13 to migrate it.

Do not run v0.12 against an upgraded store. Memory inserts, updates, and deletes
from old writers are fenced; all mixed-version operation is unsupported,
including older maintenance. Roll back only by stopping all processes and
restoring the complete pre-upgrade backup.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/TerminallyLazy/Tree-Ring-Memory/main/install.sh | sh
```

Homebrew will be updated to the verified v0.13.0 release artifact after the tag
workflow publishes it.

## Verify

```bash
tree-ring --version
tree-ring policy --help
```

Expected version: `tree-ring 0.13.0`.
