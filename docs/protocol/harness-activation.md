# Harness Activation Protocol

Tree Ring uses `ACTIVATION_PROTOCOL_VERSION = 1`. Its default project-local
flow is:

```bash
tree-ring init
tree-ring integrations status
```

`init` creates the canonical `.tree-ring/` root, discovers maintained adapters,
and installs only project-local owned bridges or bounded managed blocks. It does
not write global settings, overwrite unmanaged files, or require users to copy a
bridge or run `integrations link`. That command is an advanced alias for
controlled bridge work, not the default journey.

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
| `needs-user-review` | Existing unmanaged configuration makes automatic change unsafe. |
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

Maintained adapters detect capability, install only owned material, preflight a
new session, verify its receipt, and deactivate only what they own. Conflicting
or unmanaged files remain unchanged and report `needs-user-review`.
