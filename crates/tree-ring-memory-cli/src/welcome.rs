use serde_json::json;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;
use tree_ring_memory_sqlite::SQLiteMemoryStore;

use crate::agent_awareness::{ensure_agent_awareness, AgentAwarenessReport};

const RESET: &str = "\x1b[0m";
const TEAL: &str = "\x1b[38;2;22;156;166m";
const PINK: &str = "\x1b[38;2;239;65;103m";
const ORANGE: &str = "\x1b[38;2;255;125;34m";
const YELLOW: &str = "\x1b[38;5;220m";
const CORAL: &str = "\x1b[38;2;255;101;83m";
const BOLD: &str = "\x1b[1m";

pub fn run(root: &Path, init: bool, no_animation: bool, json_output: bool) -> Result<(), String> {
    let db_path = root.join("memory.sqlite");
    let (initialized, awareness) = if init {
        let awareness = ensure_agent_awareness(root)?;
        SQLiteMemoryStore::open(&db_path).map_err(|err| err.to_string())?;
        (true, Some(awareness))
    } else {
        (db_path.exists(), None)
    };

    if json_output {
        println!(
            "{}",
            json!({
                "ok": true,
                "root": root,
                "sqlite_path": db_path,
                "initialized": initialized,
                "init_requested": init,
                "agent_awareness": awareness,
                "next": next_commands(root),
            })
        );
        return Ok(());
    }

    let color = color_output_enabled();
    print_static_welcome(
        root,
        initialized,
        init,
        awareness.as_ref(),
        color,
        !no_animation,
    )?;
    Ok(())
}

fn print_static_welcome(
    root: &Path,
    initialized: bool,
    init_requested: bool,
    awareness: Option<&AgentAwarenessReport>,
    color: bool,
    animated: bool,
) -> Result<(), String> {
    if color && animated {
        animate_logo(color)?;
    } else if color {
        print!("{}", welcome_logo_frame(1, color));
    } else {
        print!("{}", welcome_logo_frame(1, color));
    }
    println!();
    println!("{}", paint("Tree Ring Memory is ready.", BOLD, color));
    if initialized && init_requested {
        println!(
            "Project initialized at {}. Fresh memory can start in cambium.",
            root.display()
        );
    } else if initialized {
        println!(
            "Memory root found at {}. You can recall or open the TUI now.",
            root.display()
        );
    } else {
        println!(
            "No memory root yet at {}. Run init when you are ready.",
            root.display()
        );
    }
    println!("Local-first by default. Secret-like memory is blocked before storage.");
    if let Some(awareness) = awareness {
        println!();
        println!("{}", paint("Agent awareness", YELLOW, color));
        if awareness.created.is_empty() {
            println!("  Existing guidance found in the memory root.");
        } else {
            for path in &awareness.created {
                println!("  created {}", path.display());
            }
        }
        println!("  Read SKILL.md for agent behavior and CLI.md for commands.");
        println!(
            "  Merge AGENTS.md guidance into the project root AGENTS.md for DOX-aware agents."
        );
    }
    println!();
    println!("{}", paint("Next useful commands", YELLOW, color));
    for command in next_commands(root) {
        println!("  {command}");
    }
    Ok(())
}

fn color_output_enabled() -> bool {
    io::stdout().is_terminal()
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM")
            .map(|term| term != "dumb")
            .unwrap_or(true)
}

fn animate_logo(color: bool) -> Result<(), String> {
    let mut stdout = io::stdout();
    let rows = welcome_logo_frame(0, color).lines().count();
    let sequence = [0usize, 1, 2, 3, 4, 3, 2, 1];

    for (index, frame_index) in sequence.iter().enumerate() {
        if index > 0 {
            write!(stdout, "\x1b[{rows}A\x1b[J").map_err(|err| err.to_string())?;
        }
        write!(stdout, "{}", welcome_logo_frame(*frame_index, color))
            .map_err(|err| err.to_string())?;
        stdout.flush().map_err(|err| err.to_string())?;
        thread::sleep(Duration::from_millis(95));
    }
    Ok(())
}

fn welcome_logo_frame(frame: usize, color: bool) -> String {
    let cambium = frame_style(0, frame, TEAL, color);
    let outer = frame_style(1, frame, PINK, color);
    let inner = frame_style(2, frame, ORANGE, color);
    let heartwood = frame_style(3, frame, YELLOW, color);
    let scar = frame_style(4, frame, CORAL, color);
    let mut lines = vec![
        join([raw("   "), span(".-=====-.", &cambium), span("/", &scar)]),
        join([
            raw(" "),
            span(".-' ", &cambium),
            span(".---.", &outer),
            span("/", &scar),
            span(" '-.", &cambium),
        ]),
        join([
            span("/ ", &cambium),
            span(".-' ", &outer),
            span(".-.", &inner),
            span(" '-.", &outer),
            span(" \\", &cambium),
        ]),
        join([
            span("| ", &cambium),
            span("\\-. ", &outer),
            span("oo", &heartwood),
            span(" .-/", &outer),
            span(" |", &cambium),
        ]),
        join([
            raw(" "),
            span("\\ ", &cambium),
            span("'-. ", &outer),
            span("__", &inner),
            span(" .-'", &outer),
            span(" /", &cambium),
        ]),
        join([
            raw("     "),
            span("'--", &cambium),
            span("/", &scar),
            span("--'", &cambium),
        ]),
        join([
            span("Tree ", &cambium),
            span("Ring ", &outer),
            span("Memory", &heartwood),
        ]),
    ];
    lines.push(String::new());
    lines.join("\n")
}

fn frame_style(index: usize, frame: usize, code: &str, color: bool) -> String {
    if !color {
        return String::new();
    }
    if frame % 5 == index {
        format!("{BOLD}{code}")
    } else {
        code.to_string()
    }
}

fn span(text: &str, style: &str) -> String {
    if style.is_empty() {
        text.to_string()
    } else {
        format!("{style}{text}{RESET}")
    }
}

fn raw(text: &str) -> String {
    text.to_string()
}

fn join<const N: usize>(parts: [String; N]) -> String {
    parts.join("")
}

fn next_commands(root: &Path) -> Vec<String> {
    vec![
        format!("tree-ring --root {} init", shell_path(root)),
        format!(
            "tree-ring --root {} remember \"Use project-scoped recall before risky changes.\" --event-type lesson --scope project",
            shell_path(root)
        ),
        format!("tree-ring --root {} tui", shell_path(root)),
    ]
}

fn shell_path(path: &Path) -> String {
    let value = path.display().to_string();
    if value.contains(' ') {
        format!("'{value}'")
    } else {
        value
    }
}

fn paint(text: &str, color_code: &str, color: bool) -> String {
    if color {
        format!("{color_code}{text}{RESET}")
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn next_commands_point_at_root_and_tui() {
        let commands = next_commands(Path::new(".tree-ring"));

        assert_eq!(commands.len(), 3);
        assert!(commands[0].contains(" init"));
        assert!(commands[2].contains(" tui"));
    }

    #[test]
    fn ascii_welcome_logo_is_compact_and_frame_stable() {
        let plain = welcome_logo_frame(0, false);
        let color = welcome_logo_frame(1, true);

        assert_eq!(plain.lines().count(), color.lines().count());
        assert!(plain.lines().count() <= 10);
        assert!(plain.contains("Tree Ring Memory"));
        assert!(plain.contains(".-=====-./"));
        assert!(color.contains("\x1b["));
    }

    #[test]
    fn no_animation_welcome_can_initialize_store() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        run(&root, true, true, false).unwrap();

        assert!(root.join("memory.sqlite").exists());
        assert!(root.join("AGENTS.md").exists());
        assert!(root.join("SKILL.md").exists());
        assert!(root.join("CLI.md").exists());
    }

    #[test]
    fn json_welcome_can_initialize_store() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        run(&root, true, true, true).unwrap();

        assert!(root.join("memory.sqlite").exists());
        assert!(root.join("AGENTS.md").exists());
        assert!(root.join("SKILL.md").exists());
        assert!(root.join("CLI.md").exists());
    }
}
