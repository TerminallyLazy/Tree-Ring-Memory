---
description: Preview and synchronize DOX-style AGENTS.md guidance as source-linked Tree Ring memory
argument-hint: "[project root or AGENTS.md path]"
allowed-tools: ["Bash", "Read", "Grep", "Glob"]
---

# Tree Ring DOX Sync

Use the argument as the DOX source root or single `AGENTS.md` path. Default to
the current project root when no argument is supplied.

1. Read the applicable `AGENTS.md` chain from the project root to the working
   directory. Current source contracts are authoritative; a memory summary
   never overrides them.
2. Read project-local `.tree-ring/SKILL.md` and `.tree-ring/CLI.md` when present,
   then follow the skill's Runtime Bootstrap And Updates procedure and confirm
   the selected binary reports 0.15.0 or newer. Bootstrap or upgrade when the
   user's request already authorizes it; otherwise obtain permission before
   downloading or changing software. Never invent adapter results.
3. Preview without writing:

   ```bash
   tree-ring dox sync --source-root "${ARGUMENTS:-.}" --dry-run
   ```

4. Inspect the proposed summaries and source references. Reject raw contract
   copies, secrets, low-value duplication, or output that weakens a child
   contract.
5. Persist only when the user requested synchronization and the preview is
   concise and source-linked. Run the same command without `--dry-run`. In a
   Coordinated store, persistence requires `TREE_RING_COORDINATOR_TOKEN` in the
   authorized coordinator process; dry-run discovery does not.

The adapter must not rewrite root or child `AGENTS.md` files. Re-read the live
contract chain before editing files and repeat the dry run after contract
changes.
