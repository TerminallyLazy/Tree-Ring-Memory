# Harness Activation Protocol

Tree Ring uses `ACTIVATION_PROTOCOL_VERSION = 1`. Its default project-local
flow is:

```bash
tree-ring init
tree-ring integrations status
```

`init` creates the canonical `.tree-ring/` root, discovers maintained adapters,
and attempts create-only publication of project-local owned bridges or bounded
managed blocks plus the activation manifest. It does not write global settings,
replace or remove an existing final entry, or require users to copy a bridge or
run `integrations link`. An existing bridge or manifest that would need mutation
is preserved and reported as `needs-user-review`. `integrations link` is an
advanced alias for controlled bridge work, not the default journey.

Advanced commands are `tree-ring integrations status --verbose`,
`tree-ring integrations activate --harness <id> --dry-run`,
`tree-ring integrations certify`, and `tree-ring integrations deactivate
--harness <id>`. Certification records JSON and Markdown evidence; it is not
required for initialization.

## States and proof

| State | Meaning |
| --- | --- |
| `active` | A maintained bridge has a fresh matching receipt for a new session's scoped recall and safe context injection. |
| `configured-awaiting-proof` | A safe bridge is installed, but no qualifying fresh receipt exists. |
| `active-isolated` | Preflight succeeded against a store that does not match this project's canonical store. |
| `needs-trust` | The runtime needs its own user approval before project resources load. |
| `needs-project-mount` | The runtime cannot reach the canonical project root. |
| `needs-plugin` | Agent Zero needs its separate `tree_ring_memory` plugin installed or enabled. |
| `needs-user-review` | An existing or changed bridge/manifest cannot be safely replaced or removed, or publication durability is indeterminate. |
| `unsupported` | No maintained adapter can prove the integration. |
| `failed` | Detection, installation, preflight, or receipt verification has a concrete diagnostic. |

A marker, copied skill, or scan never makes a harness `active`. Missing,
expired, malformed, or mismatched receipts remain `configured-awaiting-proof`.
A zero-result recall can be a valid receipt: it proves the check occurred
without inventing context. Hermes and any runtime without a maintained verified
adapter remain non-active.

## Artifacts and privacy

`.tree-ring/activation.json` is the versioned manifest. It contains the schema
and protocol versions, stable `store_id`, project-root fingerprint, CLI version,
and adapter records with state, capability, bridge path, owned files, and managed
blocks. Receipts live under `.tree-ring/activation/receipts/`; they contain
version and harness IDs, fingerprinted worker identity, harness-derived
agent/workflow/session IDs, state, timestamp, and optional query class, result
count, memory-ID digest, duration, and matching-store evidence. They never retain
raw prompts, recalled content, secrets, sensitive values, absolute paths, or
coordinator capabilities. Keep at most 100 receipts per harness/worker and none
older than 30 days.

A receipt proves a privacy-safe preflight check, not durable memory creation or
an adversarial security boundary. Durable writes remain explicit.

Bridge lifecycle writes fail closed. Publication creates an absent final path;
it never overwrites or removes an existing bridge or activation manifest, even
when the existing bytes were previously recorded as Tree Ring-owned.
Semantically unchanged manifests are preserved byte-for-byte. Activation or
deactivation that would mutate a final entry makes no such change and returns
`needs-user-review` with a reconciliation step.

If a directory durability check fails after publication, Tree Ring does not try
to roll the path back because it cannot safely condition removal on the exact
published inode and bytes. It preserves all published disk material, marks each
changed harness `needs-user-review` in the returned in-memory manifest, and
leaves any manifest that already reached disk intact. Disk and memory can
therefore differ until the user reviews and reconciles the indeterminate state.

## Canonical wire shapes

The following JSON shapes are the version-1 interoperability contract. All
fingerprints are lowercase, 64-character SHA-256 hex digests. Paths, when they
are present in a manifest, are project-relative. These examples deliberately
use synthetic IDs and no prompt, recalled context, capability, or absolute path.

### Activation manifest

```json
{
  "schema_version": 1,
  "protocol_version": 1,
  "store_id": "01234567-89ab-4def-8123-456789abcdef",
  "project_root_fingerprint": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "cli_version": "0.14.0",
  "harnesses": {
    "claude-code": {
      "state": "configured-awaiting-proof",
      "adapter_version": "1",
      "harness_version": "1.0.0",
      "adapter_capability": "native-preflight",
      "bridge_fingerprint": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      "bridge_path": ".claude/settings.json",
      "owned_files": [
        {
          "path": ".claude/skills/tree-ring-memory/SKILL.md",
          "sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        }
      ],
      "managed_blocks": [
        {
          "path": ".claude/settings.json",
          "block_id": "tree-ring-session-start-v1",
          "sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
        }
      ]
    }
  }
}
```

The adapter record is keyed by its canonical `harness_id`. `adapter_version`,
`harness_version`, and `bridge_fingerprint` are required for receipt-backed
activation. For a bridge with more than one owned file or managed block,
`bridge_fingerprint` is the SHA-256 of the UTF-8 canonical JSON array of its
components. Each component has exactly `path`, `kind`, and `sha256`; sort
the array by `path`, then `kind`, and serialize object keys in lexical order
with no insignificant whitespace. `kind` is either `file` or
`managed-block:<block_id>`. This lets every adapter calculate the same
fingerprint without recording the project root.

### Redacted receipt

```json
{
  "schema_version": 1,
  "protocol_version": 1,
  "receipt_id": "receipt-01",
  "harness_id": "claude-code",
  "harness_version": "1.0.0",
  "adapter_version": "1",
  "store_id": "01234567-89ab-4def-8123-456789abcdef",
  "project_root_fingerprint": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "bridge_fingerprint": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "worker_key_fingerprint": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
  "session": {
    "agent_profile": "claude-code",
    "workflow_id": "workflow-01",
    "session_id": "session-01"
  },
  "query_class": "project-startup-constraints",
  "result_count": 0,
  "selected_memory_ids_digest": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
  "duration_ms": 18,
  "state": "active",
  "recorded_at": "2026-08-13T12:00:00Z",
  "expires_at": "2026-09-12T12:00:00Z"
}
```

`query_class` is an approved stable category, never a raw task hint. A
zero-result receipt has `result_count: 0` and a digest of the empty selected-ID
set; it is still valid only after the adapter successfully injects safe context.

### Claude Code SessionStart input

The managed Claude command consumes only this privacy-safe projection of the
host's SessionStart event:

```json
{
  "session_id": "session-01",
  "cwd": ".",
  "agent_type": "claude-code"
}
```

`session_id` and `cwd` are required; `agent_type` is optional and defaults
to `claude-code`. `cwd` is resolved by the running hook and is not copied into
a receipt. Claude may include `transcript_path` in its original event, but Tree
Ring ignores it completely: it is not read, forwarded, logged, persisted, or
included in recall. No other event field is part of the preflight request.

### Claude Code SessionStart output

Claude's managed `SessionStart` command reads its event input from stdin and
writes exactly this JSON object to stdout on successful preflight:

```json
{
  "hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": "Tree Ring preflight completed; safe project context was injected."
  }
}
```

`additionalContext` contains only the safe context produced for the live
session. It must not echo hook input, task prompts, receipt JSON, capabilities,
or unredacted recalled content. The command fails without emitting a successful
SessionStart object if receipt verification fails.

### Pi JSON stdin request

Pi's `before_agent_start` extension sends local runtime identity to the CLI on
stdin only:

```json
{
  "agent_profile": "pi",
  "workflow_id": "workflow-01",
  "session_id": "session-01",
  "task_hint": "project startup constraints"
}
```

`agent_profile`, `workflow_id`, and `session_id` come from Pi's local
session manager, not model text. `task_hint` is optional; when supplied it is
sent only on stdin for the current preflight, subject to sensitivity rejection
and fallback to `project startup constraints`. The raw value is never written
to a receipt, log, memory, bridge, or response.

### Agent Zero JSON stdin request

The separate `tree_ring_memory` plugin derives Agent Zero identity on the
server and sends only this request to the project-local command:

```json
{
  "agent_profile": "agent-zero-worker",
  "workflow_id": "workflow-01",
  "session_id": "session-01"
}
```

Agent Zero does not send a task hint, prompt, coordinator capability, token, or
model-supplied identity. The plugin reads the relative project binding and
derives the three identity fields before invoking the command.

### Input validation and handling

Each adapter validates the field whitelist above before preflight. Benign host
metadata outside that whitelist is ignored and never forwarded. Unknown
capability-bearing fields (including `capability`, `token`,
`authorization`, `coordinator_capability`, or
`TREE_RING_COORDINATOR_TOKEN`) are rejected rather than ignored. Inputs that
claim a store, root, bridge fingerprint, harness identity, or receipt state are
also rejected; those values come only from the local manifest and adapter.
Raw stdin and SessionStart input are transient: Tree Ring does not persist or
log them, regardless of whether validation succeeds.

### Pi and Agent Zero JSON preflight responses

Pi's `before_agent_start` extension and the Agent Zero
`tree_ring_memory` plugin both invoke the project-local preflight command with
JSON stdin and consume this response shape. Their harness ID and
`context_format` differ, but the JSON result is identical:

```json
{
  "protocol_version": 1,
  "status": "ok",
  "context": "Tree Ring preflight completed; safe project context was injected.",
  "receipt": {
    "receipt_id": "receipt-01",
    "harness_id": "pi",
    "harness_version": "1.0.0",
    "adapter_version": "1",
    "store_id": "01234567-89ab-4def-8123-456789abcdef",
    "project_root_fingerprint": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "bridge_fingerprint": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "worker_key_fingerprint": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
    "session": {
      "agent_profile": "pi",
      "workflow_id": "workflow-01",
      "session_id": "session-01"
    },
    "query_class": "project-startup-constraints",
    "result_count": 0,
    "selected_memory_ids_digest": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    "duration_ms": 18,
    "state": "active",
    "recorded_at": "2026-08-13T12:00:00Z",
    "expires_at": "2026-09-12T12:00:00Z"
  }
}
```

Agent Zero returns this complete shape through its separate plugin:

```json
{
  "protocol_version": 1,
  "status": "ok",
  "context": "Tree Ring preflight completed; safe project context was injected.",
  "receipt": {
    "receipt_id": "receipt-02",
    "harness_id": "agent-zero",
    "harness_version": "1.0.0",
    "adapter_version": "1",
    "store_id": "01234567-89ab-4def-8123-456789abcdef",
    "project_root_fingerprint": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "bridge_fingerprint": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    "worker_key_fingerprint": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
    "session": {
      "agent_profile": "agent-zero-worker",
      "workflow_id": "workflow-01",
      "session_id": "session-01"
    },
    "query_class": "project-startup-constraints",
    "result_count": 0,
    "selected_memory_ids_digest": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
    "duration_ms": 18,
    "state": "active",
    "recorded_at": "2026-08-13T12:00:00Z",
    "expires_at": "2026-09-12T12:00:00Z"
  }
}
```

The plugin derives the Agent Zero identity server-side; it must not accept a
model-supplied identity. Its binding carries the same protocol version,
`store_id`, project-root fingerprint, relative `memory_root`, and JSON
stdin/stdout command contract. An error response has `status: "error"`, a
bounded `error_code`, and no `context` or `receipt`.

## Receipt verification

An adapter may classify a harness as `active` only after it verifies the
manifest, bridge, and receipt as one tuple. It must reject the receipt and
report the exact non-active state if any of these equality requirements fail:

1. `receipt.protocol_version == manifest.protocol_version == 1`.
2. `receipt.harness_id` is exactly the adapter's canonical harness ID, and
   `receipt.harness_version == manifest.harnesses[harness_id].harness_version`.
3. `receipt.adapter_version == manifest.harnesses[harness_id].adapter_version`.
4. `receipt.store_id == manifest.store_id`.
5. `receipt.project_root_fingerprint == manifest.project_root_fingerprint`.
6. `receipt.bridge_fingerprint == manifest.harnesses[harness_id].bridge_fingerprint`,
   after recomputing that fingerprint from the installed owned material.
7. `receipt.session.session_id` is the currently running harness session and
   its recorded worker/workflow identity matches the adapter-derived identity.
8. `receipt.state == "active"`, `recorded_at` is not in the future, and
   `expires_at - recorded_at == 2,592,000 seconds` (30 days). The receipt is
   fresh only while `recorded_at <= now < expires_at`; it is never reused for
   a later session even if its TTL has not elapsed.

Any adapter, harness, root, or bridge change invalidates prior receipts. A
matching receipt for another accessible store is `active-isolated`, never
`active` for the canonical shared project.

## Runtime and shared-store boundaries

Pi project trust is a user decision: report `needs-trust` and leave global
trust unchanged. Agent Zero uses only the separate `tree_ring_memory` plugin;
Tree Ring does not modify Agent Zero core, and a generic `.a0` marker is not an
adapter. An absent plugin is `needs-plugin`; an inaccessible project root is
`needs-project-mount`; a different accessible store is `active-isolated`.

Shared status requires every receipt to match the canonical project `store_id`.
It is supported only for concurrent processes on the same host and a local
filesystem, not across hosts, NFS/network filesystems, or containers on
different hosts. Use per-host roots and explicit evidence-preserving fan-in.

Maintained adapters detect capability, create only absent owned material,
preflight a new session, and verify its receipt. Deactivation is also
creation-only at the writer boundary: existing final entries are not removed or
replaced automatically, including recorded owned material. Conflicting,
changed, or otherwise contested bridge and manifest entries remain unchanged
and report `needs-user-review`.
