---
description: Check receipt-backed Tree Ring harness readiness without claiming configuration is activation
allowed-tools: ["Bash", "Read"]
---

# Tree Ring Status

Read project-local `.tree-ring/SKILL.md` and `.tree-ring/CLI.md` when present,
then confirm the local runtime is Tree Ring Memory 0.14.0 or newer:

```bash
tree-ring --version
```

If the runtime is missing or older, explain the limitation. Do not install or
upgrade software, edit shell configuration, or invent a status without the
user's explicit permission.

Inspect receipt-backed readiness without changing it:

```bash
tree-ring integrations status --json --verbose
```

Report the exact harness `state`, `capability`, and `next_step`. A bridge,
marker, generated skill, passive binding, or successful `init` is not activation
proof. Only a fresh matching receipt from a new session's scoped recall and safe
context injection can produce `active`.

Treat `configured-awaiting-proof`, `active-isolated`, `needs-trust`,
`needs-project-mount`, `needs-plugin`, `needs-user-review`, `unsupported`, and
`failed` as non-active outcomes. Do not modify global trust, manufacture a
receipt, hand-author an Agent Zero capability descriptor, or replace a contested
bridge while checking status.
