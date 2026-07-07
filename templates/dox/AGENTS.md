# Tree Ring Memory Project Contract

This file is a DOX-style template for projects that use Tree Ring Memory.
Copy it into a project as `AGENTS.md` or merge the relevant sections into an existing project instruction file.

## Authority

Project source files, tests, local `AGENTS.md` files, and explicit user instructions remain authoritative.
Tree Ring Memory is a recall aid. It must not replace reading the local project contract.

## Memory Store

Default local memory path:

```text
.tree-ring/
```

Do not commit local memory databases or exports unless the project explicitly requires sanitized fixtures.

Harness-native bridge files should point agents back to this memory root's
generated guidance instead of duplicating memory data. Prefer project-level
bridges for the current repo. Treat global Tree Ring bridges as explicit user
configuration that affects every project.

## Recall Rules

Before substantial work, recall project-scoped memory for:

- current project conventions
- prior decisions
- durable user preferences
- warnings and scars related to the task
- unresolved seeds that may affect the plan

Prefer narrow recall queries that include the project name, subsystem, file path, or workflow.

## Remember Rules

Remember only meaningful information:

- durable decisions
- verified lessons
- user corrections
- repeated workflow warnings
- project conventions
- future seeds

Do not store full transcripts, scratchpad notes, raw chain-of-thought, secrets, credentials, or sensitive personal details.

Use `tree-ring evidence` for evaluated outcomes from runs, checkpoints,
experiments, incidents, PRs, issues, or reviewed artifacts. Promotions should
become heartwood only when the evidence supports durable reuse. Rejections with
reusable warning value should become scars. Deferred possibilities should become
seeds.

Use source adapters when local authoritative files already contain the guidance
or evaluated outcome:

```bash
tree-ring dox sync --source-root . --dry-run
tree-ring revolve sync --source-root revolve --dry-run
tree-ring integrations scan --source-root .
```

Run adapter syncs as previews first. Persist only concise, source-linked
summaries that help future recall.

## Agent-Mediated Updates

Tree Ring Memory writes durable memories only when a user, agent, adapter,
import, TUI action, consolidation command, or explicit maintenance command calls
the CLI. It must not be used as a hidden transcript recorder. Bridge files tell
the active agent when to call Tree Ring; they do not authorize autonomous
background capture.

## Ring Mapping

- Use `cambium` for active task context.
- Use `outer` for recent project lessons.
- Use `inner` for compressed older project knowledge.
- Use `heartwood` for confirmed durable rules and preferences.
- Use `scar` for failures, regressions, rejected approaches, and security or privacy warnings.
- Use `seed` for future work and unresolved hypotheses.

## Sensitive Data

Secrets and credentials must not be stored.
If a useful lesson involves sensitive data, store a redacted summary and source pointer only.

## Forgetting

If a memory is incorrect, stale, sensitive, or superseded, delete, redact, or supersede it with an explicit reason.

## Source Discipline

Memory summaries should point back to source evidence such as:

- file paths
- PRs or issues
- tests
- evaluation runs
- local project docs

When source documents and memory disagree, re-read the source documents and update or forget the stale memory.

DOX-style `AGENTS.md` files and Revolve/evaluation records remain
authoritative. Tree Ring Memory can summarize and point to them, but it must not
replace DOX traversal, copy whole contract trees, treat stale scores as current
truth, or promote an outcome without source evidence.
