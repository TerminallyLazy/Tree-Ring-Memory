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
tree-ring consolidate --period-type manual --dry-run
tree-ring maintain
tree-ring tui
```

Safety rules:

- Do not store secrets, credentials, private keys, or raw chain-of-thought.
- Prefer concise, source-linked summaries over transcript capture.
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
tree-ring --root {root} forget mem_example --mode redact --reason "remove sensitive detail"
tree-ring --root {root} tui
```

## DOX Integration

If this project uses DOX-style `AGENTS.md` traversal, merge the relevant
sections from the template below into the project root `AGENTS.md`. Tree Ring
Memory does not overwrite root project contracts automatically.

---

{DOX_TEMPLATE}
"#,
        root = shell_path(root)
    )
}

fn shell_path(path: &Path) -> String {
    path.display().to_string()
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
        assert!(agents.contains("Tree Ring Memory Project Contract"));
    }
}
