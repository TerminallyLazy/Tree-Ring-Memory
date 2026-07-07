use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const SKILL_TEMPLATE: &str = include_str!("../../../skills/tree-ring-memory/SKILL.md");
const DOX_TEMPLATE: &str = include_str!("../../../templates/dox/AGENTS.md");
const CLI_REFERENCE: &str = r#"# Tree Ring Memory CLI Quick Reference

Tree Ring Memory is a local-first memory lifecycle layer for AI agents.

Use memory deliberately:

- recall before substantial project work
- remember durable decisions, lessons, warnings, and user preferences
- use scars for failures that should not be repeated
- use seeds for future work and hypotheses
- forget, redact, or supersede stale or sensitive memory

Core commands:

```bash
tree-ring init
tree-ring recall "project startup warnings"
tree-ring remember "Use project-scoped recall before risky changes." --event-type lesson --scope project
tree-ring evidence "Snapshot invalidation fixed stale unread chat state." --outcome promoted --evidence-ref evals/chat-state/run-042 --score 0.91
tree-ring evidence "Aggressive caching caused stale multi-chat state." --outcome rejected --evidence-ref evals/cache-branch/run-013
tree-ring forget mem_example --mode redact --reason "remove sensitive detail"
tree-ring export --output memories.jsonl
tree-ring import memories.jsonl --dry-run
tree-ring audit --audit-type all
tree-ring consolidate --period-type manual --dry-run
tree-ring maintain
tree-ring dox sync --source-root . --dry-run
tree-ring revolve sync --source-root revolve --dry-run
tree-ring integrations scan --source-root .
tree-ring tui
```

Project bridge files:

- keep `.tree-ring/SKILL.md` and `.tree-ring/CLI.md` canonical
- point harness-native files at those generated references
- prefer project-level bridges over global bridges
- treat global bridges as explicit opt-in user configuration

Adapter rules:

- `tree-ring dox sync` summarizes `AGENTS.md` files and keeps source contracts authoritative.
- `tree-ring revolve sync` imports promoted, rejected, deferred, or observed evidence records without replacing Revolve/evaluation docs.
- `tree-ring evidence` records individual evaluated outcomes with an explicit source ref.
- Run adapter commands with `--dry-run` before writing memory.
- `tree-ring integrations scan` is read-only; add harness bridge references manually until a link command is available.

Safety rules:

- Do not store secrets, credentials, private keys, or raw chain-of-thought.
- Prefer concise, source-linked summaries over transcript capture.
- Do not scrape chats or turn TUI event-stream pulses into durable memory without an explicit write command.
- Treat local source files, tests, explicit user instructions, and root `AGENTS.md` files as authoritative.
- When memory and source docs disagree, re-read source docs and update or forget stale memory.
"#;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AgentAwarenessReport {
    pub created: Vec<PathBuf>,
    pub existing: Vec<PathBuf>,
}

pub fn ensure_agent_awareness(root: &Path) -> Result<AgentAwarenessReport, String> {
    fs::create_dir_all(root).map_err(|err| err.to_string())?;
    let mut report = AgentAwarenessReport {
        created: Vec::new(),
        existing: Vec::new(),
    };

    write_if_missing(&root.join("AGENTS.md"), &agent_contract(root), &mut report)?;
    write_if_missing(&root.join("SKILL.md"), SKILL_TEMPLATE, &mut report)?;
    write_if_missing(&root.join("CLI.md"), CLI_REFERENCE, &mut report)?;

    Ok(report)
}

fn write_if_missing(
    path: &Path,
    content: &str,
    report: &mut AgentAwarenessReport,
) -> Result<(), String> {
    if path.exists() {
        report.existing.push(path.to_path_buf());
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(path, content).map_err(|err| err.to_string())?;
    report.created.push(path.to_path_buf());
    Ok(())
}

fn agent_contract(root: &Path) -> String {
    format!(
        r#"# Tree Ring Memory Agent Instructions

This directory contains Tree Ring Memory's local project memory store.

Memory is a recall aid. It does not replace project source files, tests, root
`AGENTS.md` files, explicit user instructions, DOX contracts, or other local
project documentation.

## Memory Root

```text
{root}
```

Do not commit local memory databases or exports unless the project explicitly
requires sanitized fixtures.

## Agent Skill

Read `SKILL.md` before using Tree Ring Memory. It explains when to recall,
remember, redact, forget, consolidate, and avoid memory capture.

## CLI Reference

Read `CLI.md` for local commands. Common commands:

```bash
tree-ring --root {root} recall "project startup warnings"
tree-ring --root {root} remember "Use project-scoped recall before risky changes." --event-type lesson --scope project
tree-ring --root {root} evidence "A promoted evaluation fixed stale state." --outcome promoted --evidence-ref evals/run-042 --score 0.91
tree-ring --root {root} forget mem_example --mode redact --reason "remove sensitive detail"
tree-ring --root {root} dox sync --source-root . --dry-run
tree-ring --root {root} revolve sync --source-root revolve --dry-run
tree-ring --root {root} integrations scan --source-root .
tree-ring --root {root} tui
```

## Harness Bridges

Harness-native bridge files should point back to this directory instead of
copying memory data. Recommended project bridges include
`.agents/skills/tree-ring-memory/SKILL.md` for Codex/Gemini-style skill loaders,
`.claude/skills/tree-ring-memory/SKILL.md` plus `CLAUDE.md` references for
Claude Code, root `AGENTS.md` references for OpenCode/DOX-style agents, and
`.pi/settings.json` resource references for Pi.

Project-level bridges are preferred because they stay scoped to the current
repo. Global bridges affect every project and should be treated as explicit
user opt-in configuration.

Tree Ring Memory is agent-mediated. Bridge files tell the active agent when to
call `tree-ring recall`, `tree-ring remember`, `tree-ring evidence`,
`tree-ring forget`, `tree-ring consolidate --dry-run`, or `tree-ring maintain`.
They do not authorize hidden transcript scraping or autonomous durable writes.

## DOX Integration

If this project uses DOX-style `AGENTS.md` traversal, merge the relevant
sections from the template below into the project root `AGENTS.md`. Tree Ring
Memory does not overwrite root project contracts automatically.

Use `tree-ring --root {root} dox sync --source-root . --dry-run` to preview
concise memory summaries for local `AGENTS.md` files. The source contracts
remain authoritative; memory is only a recall aid.

## Revolve And Evidence Integration

Use `tree-ring --root {root} evidence ... --evidence-ref <ref>` for individual
evaluated outcomes. Use the Revolve sync command below to preview source-linked
memories from Revolve/evaluation records:

```bash
tree-ring --root {root} revolve sync --source-root revolve --dry-run
```

Promoted outcomes become heartwood, rejected outcomes become scars, deferred
outcomes become seeds, and observed outcomes become outer-ring evidence.

---

{DOX_TEMPLATE}
"#,
        root = shell_path(root)
    )
}

fn shell_path(path: &Path) -> String {
    let value = path.display().to_string();
    if value.contains(' ') {
        format!("'{value}'")
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn init_writes_agent_awareness_files_without_overwriting() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        let first = ensure_agent_awareness(&root).unwrap();
        assert_eq!(first.created.len(), 3);
        assert!(root.join("AGENTS.md").exists());
        assert!(root.join("SKILL.md").exists());
        assert!(root.join("CLI.md").exists());

        fs::write(root.join("CLI.md"), "custom").unwrap();
        let second = ensure_agent_awareness(&root).unwrap();
        assert!(second.existing.iter().any(|path| path.ends_with("CLI.md")));
        assert_eq!(fs::read_to_string(root.join("CLI.md")).unwrap(), "custom");
    }

    #[test]
    fn generated_agents_file_mentions_skill_cli_and_dox() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        ensure_agent_awareness(&root).unwrap();
        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();

        assert!(agents.contains("SKILL.md"));
        assert!(agents.contains("CLI.md"));
        assert!(agents.contains("DOX Integration"));
        assert!(agents.contains("Revolve And Evidence Integration"));
        assert!(agents.contains("dox sync --source-root"));
        assert!(agents.contains("revolve sync --source-root"));
        assert!(agents.contains("integrations scan --source-root"));
        assert!(agents.contains("Tree Ring Memory Project Contract"));
    }

    #[test]
    fn generated_agents_file_quotes_roots_with_spaces() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("project memory").join(".tree-ring");

        ensure_agent_awareness(&root).unwrap();
        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();

        assert!(agents.contains(&format!("--root '{}'", root.display())));
    }
}
