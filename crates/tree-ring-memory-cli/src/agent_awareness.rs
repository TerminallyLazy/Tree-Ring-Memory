use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const SKILL_TEMPLATE: &str = include_str!("../../../skills/tree-ring-memory/SKILL.md");
const DOX_TEMPLATE: &str = include_str!("../../../templates/dox/AGENTS.md");
const AGENT_HEADER: &str = "# Tree Ring Memory Agent Instructions";
const CLI_HEADER: &str = "# Tree Ring Memory CLI Quick Reference";
const SKILL_FRONT_MATTER_MARKER: &str = "name: tree-ring-memory";
const AGENT_QUALITY_GATES_HEADING: &str = "## Memory Quality Gates";
const AGENT_QUALITY_GATES_ANCHOR: &str = "## DOX Integration";
const CLI_QUALITY_GATES_HEADING: &str = "Memory quality gates:";
const CLI_QUALITY_GATES_ANCHOR: &str = "Safety rules:";
const SKILL_QUALITY_GATES_HEADING: &str = "## Memory Quality Gates";
const SKILL_QUALITY_GATES_ANCHOR: &str = "## Ring Selection";
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

Memory quality gates:

- Before substantial project work, recall project constraints, scars, user preferences, and unresolved seeds.
- Before risky changes, recall warnings and evidence-linked prior failures.
- Before repeating a workflow, recall prior errors and accepted procedures.
- Before closeout, recall recent decisions so memory updates do not contradict already-stored lessons.
- Before trusting memory, prefer source-linked, non-superseded, high-confidence results.
- Re-read source files, tests, explicit user instructions, DOX contracts, or Revolve evidence when memory conflicts with current sources.
- Before writing memory, reject transient planning chatter, duplicate wording, tool noise, and unsupported claims.
- Require evidence refs for promoted or rejected evaluated outcomes.
- Require user confirmation before creating or promoting broad cross-project heartwood.

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

    let agent_contract = agent_contract(root);
    write_if_missing_or_backfill(
        &root.join("AGENTS.md"),
        &agent_contract,
        &mut report,
        is_generated_agents_file,
        AGENT_QUALITY_GATES_HEADING,
        AGENT_QUALITY_GATES_ANCHOR,
        extract_section(
            &agent_contract,
            AGENT_QUALITY_GATES_HEADING,
            AGENT_QUALITY_GATES_ANCHOR,
        ),
    )?;
    write_if_missing_or_backfill(
        &root.join("SKILL.md"),
        SKILL_TEMPLATE,
        &mut report,
        is_generated_skill_file,
        SKILL_QUALITY_GATES_HEADING,
        SKILL_QUALITY_GATES_ANCHOR,
        extract_section(
            SKILL_TEMPLATE,
            SKILL_QUALITY_GATES_HEADING,
            SKILL_QUALITY_GATES_ANCHOR,
        ),
    )?;
    write_if_missing_or_backfill(
        &root.join("CLI.md"),
        CLI_REFERENCE,
        &mut report,
        is_generated_cli_file,
        CLI_QUALITY_GATES_HEADING,
        CLI_QUALITY_GATES_ANCHOR,
        extract_section(
            CLI_REFERENCE,
            CLI_QUALITY_GATES_HEADING,
            CLI_QUALITY_GATES_ANCHOR,
        ),
    )?;

    Ok(report)
}

fn write_if_missing_or_backfill(
    path: &Path,
    content: &str,
    report: &mut AgentAwarenessReport,
    recognizer: fn(&str) -> bool,
    section_heading: &str,
    anchor: &str,
    section: Option<&str>,
) -> Result<(), String> {
    if path.exists() {
        maybe_backfill_generated_file(path, recognizer, section_heading, anchor, section)?;
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

fn maybe_backfill_generated_file(
    path: &Path,
    recognizer: fn(&str) -> bool,
    section_heading: &str,
    anchor: &str,
    section: Option<&str>,
) -> Result<(), String> {
    let Some(section) = section else {
        return Ok(());
    };

    let existing = fs::read_to_string(path).map_err(|err| err.to_string())?;
    if !recognizer(&existing) || existing.contains(section_heading) {
        return Ok(());
    }

    let Some(updated) = insert_section_before_anchor(&existing, section, anchor) else {
        return Ok(());
    };
    if updated != existing {
        fs::write(path, updated).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn extract_section<'a>(content: &'a str, heading: &str, anchor: &str) -> Option<&'a str> {
    let start = content.find(heading)?;
    let end = content[start..].find(anchor)? + start;
    Some(&content[start..end])
}

fn insert_section_before_anchor(content: &str, section: &str, anchor: &str) -> Option<String> {
    let anchor_index = content.find(anchor)?;
    let mut updated = String::with_capacity(content.len() + section.len());
    updated.push_str(&content[..anchor_index]);
    updated.push_str(section);
    updated.push_str(&content[anchor_index..]);
    Some(updated)
}

fn is_generated_agents_file(content: &str) -> bool {
    content.starts_with(AGENT_HEADER)
}

fn is_generated_cli_file(content: &str) -> bool {
    content.starts_with(CLI_HEADER)
}

fn is_generated_skill_file(content: &str) -> bool {
    content.starts_with("---\n") && content.contains(SKILL_FRONT_MATTER_MARKER)
}

fn agent_contract(root: &Path) -> String {
    format!(
        r#"{AGENT_HEADER}

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

## Memory Quality Gates

Recall gates:

- Before substantial project work, recall project constraints, scars, user preferences, and unresolved seeds.
- Before risky changes, recall warnings and evidence-linked prior failures.
- Before repeating a workflow, recall prior errors and accepted procedures.
- Before closeout, recall recent decisions so memory updates do not contradict already-stored lessons.

Trust gates:

- Prefer source-linked, non-superseded, high-confidence memories.
- Re-read source files, tests, explicit user instructions, DOX contracts, or Revolve evidence when memory conflicts with current sources.
- Do not treat sensitive or hidden-by-default memory as ordinary recall context.

Write gates:

- Remember only durable decisions, validated lessons, reusable warnings, corrections, future seeds, and evidence-backed outcomes.
- Reject transient planning chatter, duplicate wording, tool noise, and unsupported claims.
- Require evidence refs for promoted or rejected evaluated outcomes.
- Require user confirmation before creating or promoting broad cross-project heartwood.

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
        AGENT_HEADER = AGENT_HEADER,
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

    const TEST_AGENT_QUALITY_GATES_SECTION: &str = r#"## Memory Quality Gates

Recall gates:

- Before substantial project work, recall project constraints, scars, user preferences, and unresolved seeds.
- Before risky changes, recall warnings and evidence-linked prior failures.
- Before repeating a workflow, recall prior errors and accepted procedures.
- Before closeout, recall recent decisions so memory updates do not contradict already-stored lessons.

Trust gates:

- Prefer source-linked, non-superseded, high-confidence memories.
- Re-read source files, tests, explicit user instructions, DOX contracts, or Revolve evidence when memory conflicts with current sources.
- Do not treat sensitive or hidden-by-default memory as ordinary recall context.

Write gates:

- Remember only durable decisions, validated lessons, reusable warnings, corrections, future seeds, and evidence-backed outcomes.
- Reject transient planning chatter, duplicate wording, tool noise, and unsupported claims.
- Require evidence refs for promoted or rejected evaluated outcomes.
- Require user confirmation before creating or promoting broad cross-project heartwood.

"#;

    const TEST_CLI_QUALITY_GATES_SECTION: &str = r#"Memory quality gates:

- Before substantial project work, recall project constraints, scars, user preferences, and unresolved seeds.
- Before risky changes, recall warnings and evidence-linked prior failures.
- Before repeating a workflow, recall prior errors and accepted procedures.
- Before closeout, recall recent decisions so memory updates do not contradict already-stored lessons.
- Before trusting memory, prefer source-linked, non-superseded, high-confidence results.
- Re-read source files, tests, explicit user instructions, DOX contracts, or Revolve evidence when memory conflicts with current sources.
- Before writing memory, reject transient planning chatter, duplicate wording, tool noise, and unsupported claims.
- Require evidence refs for promoted or rejected evaluated outcomes.
- Require user confirmation before creating or promoting broad cross-project heartwood.

"#;

    const TEST_SKILL_QUALITY_GATES_SECTION: &str = r#"## Memory Quality Gates

Use these gates before relying on or writing memory.

Recall gates:

- Before substantial project work, recall project constraints, scars, user preferences, and unresolved seeds.
- Before risky changes, recall warnings and evidence-linked prior failures.
- Before repeating a workflow, recall prior errors and accepted procedures.
- Before closeout, recall recent decisions so memory updates do not contradict already-stored lessons.

Trust gates:

- Prefer source-linked, non-superseded, high-confidence memories.
- Treat heartwood as durable only when source evidence or user confirmation supports it.
- Re-read source files, tests, explicit user instructions, DOX contracts, or Revolve evidence when memory conflicts with current sources.
- Do not treat sensitive or hidden-by-default memory as ordinary recall context.

Write gates:

- Remember only durable decisions, validated lessons, reusable warnings, corrections, future seeds, and evidence-backed outcomes.
- Reject transient planning chatter, duplicate wording, tool noise, and unsupported claims.
- Require evidence refs for promoted or rejected evaluated outcomes.
- Require user confirmation before creating or promoting broad cross-project heartwood.

"#;

    const TEST_AGENT_QUALITY_GATES_ANCHOR: &str = "## DOX Integration";
    const TEST_CLI_QUALITY_GATES_ANCHOR: &str = "Safety rules:";
    const TEST_SKILL_QUALITY_GATES_ANCHOR: &str = "## Ring Selection";

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
    fn generated_agents_file_mentions_quality_gates() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        ensure_agent_awareness(&root).unwrap();
        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();

        assert!(agents.contains("Memory Quality Gates"));
        assert!(agents.contains("Recall gates"));
        assert!(agents.contains("Trust gates"));
        assert!(agents.contains("Write gates"));
        assert!(agents.contains("Reject transient planning chatter"));
    }

    #[test]
    fn generated_cli_reference_mentions_quality_gates() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        ensure_agent_awareness(&root).unwrap();
        let cli = fs::read_to_string(root.join("CLI.md")).unwrap();

        assert!(cli.contains("Memory quality gates"));
        assert!(cli.contains("Before substantial project work, recall project constraints, scars, user preferences, and unresolved seeds."));
        assert!(cli
            .contains("Before risky changes, recall warnings and evidence-linked prior failures."));
        assert!(cli
            .contains("Before repeating a workflow, recall prior errors and accepted procedures."));
        assert!(cli.contains("Before closeout, recall recent decisions so memory updates do not contradict already-stored lessons."));
        assert!(cli.contains("Before trusting memory, prefer source-linked"));
        assert!(cli.contains("Before writing memory, reject transient planning chatter"));
    }

    #[test]
    fn generated_backfills_quality_gates_into_recognized_stale_generated_files() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        fs::create_dir_all(&root).unwrap();

        fs::write(
            root.join("AGENTS.md"),
            agent_contract(&root).replace(TEST_AGENT_QUALITY_GATES_SECTION, ""),
        )
        .unwrap();
        fs::write(
            root.join("CLI.md"),
            CLI_REFERENCE.replace(TEST_CLI_QUALITY_GATES_SECTION, ""),
        )
        .unwrap();
        fs::write(
            root.join("SKILL.md"),
            SKILL_TEMPLATE.replace(TEST_SKILL_QUALITY_GATES_SECTION, ""),
        )
        .unwrap();

        let report = ensure_agent_awareness(&root).unwrap();
        assert_eq!(report.created.len(), 0);
        assert_eq!(report.existing.len(), 3);

        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        let cli = fs::read_to_string(root.join("CLI.md")).unwrap();
        let skill = fs::read_to_string(root.join("SKILL.md")).unwrap();

        assert!(agents.contains(TEST_AGENT_QUALITY_GATES_SECTION.trim()));
        assert!(cli.contains(TEST_CLI_QUALITY_GATES_SECTION.trim()));
        assert!(skill.contains(TEST_SKILL_QUALITY_GATES_SECTION.trim()));
    }

    #[test]
    fn generated_backfill_inserts_quality_gates_at_canonical_anchors() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        fs::create_dir_all(&root).unwrap();

        fs::write(
            root.join("AGENTS.md"),
            agent_contract(&root).replace(TEST_AGENT_QUALITY_GATES_SECTION, ""),
        )
        .unwrap();
        fs::write(
            root.join("CLI.md"),
            CLI_REFERENCE.replace(TEST_CLI_QUALITY_GATES_SECTION, ""),
        )
        .unwrap();
        fs::write(
            root.join("SKILL.md"),
            SKILL_TEMPLATE.replace(TEST_SKILL_QUALITY_GATES_SECTION, ""),
        )
        .unwrap();

        ensure_agent_awareness(&root).unwrap();

        let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        let cli = fs::read_to_string(root.join("CLI.md")).unwrap();
        let skill = fs::read_to_string(root.join("SKILL.md")).unwrap();

        assert!(agents.contains(&format!(
            "{}{}",
            TEST_AGENT_QUALITY_GATES_SECTION, TEST_AGENT_QUALITY_GATES_ANCHOR
        )));
        assert!(cli.contains(&format!(
            "{}{}",
            TEST_CLI_QUALITY_GATES_SECTION, TEST_CLI_QUALITY_GATES_ANCHOR
        )));
        assert!(skill.contains(&format!(
            "{}{}",
            TEST_SKILL_QUALITY_GATES_SECTION, TEST_SKILL_QUALITY_GATES_ANCHOR
        )));
    }

    #[test]
    fn generated_backfill_preserves_custom_content_in_recognized_generated_files() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        fs::create_dir_all(&root).unwrap();

        let agents_custom = "\nCustom note: keep me.\n";
        let cli_custom = "\nCustom alias note: keep me.\n";
        let skill_custom = "\nCustom workflow note: keep me.\n";

        fs::write(
            root.join("AGENTS.md"),
            agent_contract(&root).replace(TEST_AGENT_QUALITY_GATES_SECTION, "") + agents_custom,
        )
        .unwrap();
        fs::write(
            root.join("CLI.md"),
            CLI_REFERENCE.replace(TEST_CLI_QUALITY_GATES_SECTION, "") + cli_custom,
        )
        .unwrap();
        fs::write(
            root.join("SKILL.md"),
            SKILL_TEMPLATE.replace(TEST_SKILL_QUALITY_GATES_SECTION, "") + skill_custom,
        )
        .unwrap();

        ensure_agent_awareness(&root).unwrap();

        assert!(fs::read_to_string(root.join("AGENTS.md"))
            .unwrap()
            .ends_with(agents_custom));
        assert!(fs::read_to_string(root.join("CLI.md"))
            .unwrap()
            .ends_with(cli_custom));
        assert!(fs::read_to_string(root.join("SKILL.md"))
            .unwrap()
            .ends_with(skill_custom));
    }

    #[test]
    fn generated_backfill_leaves_arbitrary_custom_files_untouched() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        fs::create_dir_all(&root).unwrap();

        fs::write(root.join("AGENTS.md"), "# Custom project contract\n").unwrap();
        fs::write(root.join("CLI.md"), "# Custom CLI guide\n").unwrap();
        fs::write(root.join("SKILL.md"), "# Custom skill\n").unwrap();

        ensure_agent_awareness(&root).unwrap();

        assert_eq!(
            fs::read_to_string(root.join("AGENTS.md")).unwrap(),
            "# Custom project contract\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("CLI.md")).unwrap(),
            "# Custom CLI guide\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("SKILL.md")).unwrap(),
            "# Custom skill\n"
        );
    }

    #[test]
    fn generated_backfill_is_idempotent_for_recognized_stale_files() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        fs::create_dir_all(&root).unwrap();

        fs::write(
            root.join("AGENTS.md"),
            agent_contract(&root).replace(TEST_AGENT_QUALITY_GATES_SECTION, ""),
        )
        .unwrap();
        fs::write(
            root.join("CLI.md"),
            CLI_REFERENCE.replace(TEST_CLI_QUALITY_GATES_SECTION, ""),
        )
        .unwrap();
        fs::write(
            root.join("SKILL.md"),
            SKILL_TEMPLATE.replace(TEST_SKILL_QUALITY_GATES_SECTION, ""),
        )
        .unwrap();

        ensure_agent_awareness(&root).unwrap();
        let first_agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        let first_cli = fs::read_to_string(root.join("CLI.md")).unwrap();
        let first_skill = fs::read_to_string(root.join("SKILL.md")).unwrap();

        ensure_agent_awareness(&root).unwrap();

        let second_agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
        let second_cli = fs::read_to_string(root.join("CLI.md")).unwrap();
        let second_skill = fs::read_to_string(root.join("SKILL.md")).unwrap();

        assert_eq!(first_agents, second_agents);
        assert_eq!(first_cli, second_cli);
        assert_eq!(first_skill, second_skill);
        assert_eq!(second_agents.matches("## Memory Quality Gates").count(), 1);
        assert_eq!(second_cli.matches("Memory quality gates:").count(), 1);
        assert_eq!(second_skill.matches("## Memory Quality Gates").count(), 1);
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
