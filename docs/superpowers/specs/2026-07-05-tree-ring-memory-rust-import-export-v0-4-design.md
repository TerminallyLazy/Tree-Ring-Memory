# Tree Ring Memory Rust Import/Export v0.4 Design

## Summary

v0.4 closes the next Rust migration gap: portable local memory import and
export. Tree Ring Memory needs a framework-agnostic way to back up, review, and
migrate memory without exposing sensitive rows by default and without relying on
Python as the behavioral owner.

The first v0.4 target is JSONL. SQLite backups, markdown reports, and richer
bundle signing can follow later.

## Goals

- Add Rust-owned JSONL export.
- Add Rust-owned JSONL import.
- Keep the format portable and line-oriented for CLI agents and Unix tooling.
- Exclude sensitive memory from export by default.
- Exclude superseded memory from export by default.
- Support dry-run import previews.
- Support merge-style import that skips duplicate ids by default.
- Expose the behavior through the Rust CLI.
- Expose the behavior through the optional native Python backend.

## Non-Goals

- No cloud sync.
- No external storage services.
- No automatic disk ingestion.
- No markdown or SQLite backup format in this phase.
- No consolidation/audit implementation in this phase.
- No forced removal of the Python reference backend.

## JSONL Format

The export is newline-delimited JSON.

The first line is a metadata header:

```json
{"type":"tree_ring_memory_export","schema_version":1,"plugin_version":"0.4.0","created_at":"...","memory_count":2,"sensitive_included":false}
```

Each memory line is an envelope:

```json
{"type":"memory_event","memory":{... MemoryEvent JSON ...}}
```

Import accepts both the envelope form and raw `MemoryEvent` JSON lines so older
or hand-edited exports remain usable.

## Safety

- Export excludes non-normal sensitivity unless `--include-sensitive` is set.
- Export excludes superseded memory unless `--include-superseded` is set.
- Import validates every memory event before writing.
- Import dry-run validates and reports counts without mutating storage.
- Import skips duplicate ids by default.
- Import only replaces existing rows when `--replace-existing` is set.

## CLI Shape

```bash
tree-ring export --output memories.jsonl
tree-ring export --include-sensitive --include-superseded
tree-ring import memories.jsonl --dry-run
tree-ring import memories.jsonl --replace-existing
```

`--json` returns operation reports for output-file export and import. When
exporting to stdout, the command emits JSONL because that is the requested data
stream.

## Acceptance

1. Rust can export JSONL from SQLite.
2. Rust can import JSONL into a new store.
3. Sensitive memory is excluded by default.
4. Superseded memory is excluded by default.
5. Import dry-run does not mutate storage.
6. Duplicate import is skipped by default.
7. Replace import can overwrite existing ids.
8. Python can read Rust-exported/Rust-imported stores.
9. Native Python wrapper exposes import/export when the extension is installed.
10. Rust and Python tests pass.
