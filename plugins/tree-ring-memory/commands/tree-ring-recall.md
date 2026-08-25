---
description: Recall durable Tree Ring Memory context before starting or resuming work
argument-hint: "[focused query]"
allowed-tools: ["Bash", "Read", "Grep", "Glob"]
---

# Tree Ring Recall

Recall useful project memory before acting on context-dependent work.

1. Check for project-local guidance first:

   ```bash
   test -f .tree-ring/SKILL.md && sed -n '1,520p' .tree-ring/SKILL.md
   test -f .tree-ring/CLI.md && sed -n '1,520p' .tree-ring/CLI.md
   ```

   Follow the skill's Runtime Bootstrap And Updates procedure and confirm the
   selected binary reports 0.15.0 or newer. Bootstrap or upgrade when the
   user's request already authorizes it; otherwise obtain permission before
   downloading or changing software. Never fabricate recall.

2. Use the user's argument as the focused recall query when present:

   ```bash
   tree-ring recall "$ARGUMENTS"
   ```

3. If no argument is provided, choose a narrow query from the current task and
   include project scope when known.

4. In a fan-out worker, filter recall with its agent profile plus the shared
   workflow and current session. At coordinator fan-in, omit only the
   agent-profile filter deliberately so all worker results remain visible:

   ```bash
   env -u TREE_RING_AGENT_PROFILE tree-ring recall "$ARGUMENTS" \
     --workflow-id "$TREE_RING_WORKFLOW_ID" \
     --session-id "$TREE_RING_SESSION_ID" \
     --scope agent
   ```

5. Treat recall as context, not authority. Prefer source-linked,
   high-confidence, non-superseded entries and verify drift-prone facts before
   relying on them.

Identity and scope partition and route local memory; they are not read ACLs.
The supported shared-root boundary is cooperative processes on one host and a
local filesystem, not cross-host or network-filesystem coordination.
