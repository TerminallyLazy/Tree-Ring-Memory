use serde_json::json;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;
use tree_ring_memory_sqlite::SQLiteMemoryStore;

const RESET: &str = "\x1b[0m";
const TEAL: &str = "\x1b[38;5;37m";
const PINK: &str = "\x1b[38;5;204m";
const ORANGE: &str = "\x1b[38;5;208m";
const YELLOW: &str = "\x1b[38;5;220m";
const BLUE: &str = "\x1b[38;5;33m";
const BOLD: &str = "\x1b[1m";

pub fn run(root: &Path, init: bool, no_animation: bool, json_output: bool) -> Result<(), String> {
    let db_path = root.join("memory.sqlite");
    let initialized = if init {
        SQLiteMemoryStore::open(&db_path).map_err(|err| err.to_string())?;
        true
    } else {
        db_path.exists()
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
                "next": next_commands(root),
            })
        );
        return Ok(());
    }

    let color = io::stdout().is_terminal();
    if color && !no_animation {
        animate_intro()?;
    }
    print_static_welcome(root, initialized, init, color);
    Ok(())
}

fn animate_intro() -> Result<(), String> {
    let mut stdout = io::stdout();
    for frame in 0..8 {
        write!(stdout, "\x1b[2J\x1b[H").map_err(|err| err.to_string())?;
        for line in ring_frame(frame, true) {
            writeln!(stdout, "{line}").map_err(|err| err.to_string())?;
        }
        stdout.flush().map_err(|err| err.to_string())?;
        thread::sleep(Duration::from_millis(90));
    }
    Ok(())
}

fn print_static_welcome(root: &Path, initialized: bool, init_requested: bool, color: bool) {
    for line in ring_frame(7, color) {
        println!("{line}");
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
    println!();
    println!("{}", paint("Next useful commands", YELLOW, color));
    for command in next_commands(root) {
        println!("  {command}");
    }
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

fn ring_frame(frame: usize, color: bool) -> Vec<String> {
    let pulse = if frame % 2 == 0 { "*" } else { "+" };
    vec![
        paint("        .-=================-.        ", TEAL, color) + pulse,
        paint("     .-'   cambium  fresh   '-.     ", PINK, color),
        paint("   .'   .--- outer  detailed ---.   '.", ORANGE, color),
        paint("  /   .'   .-- inner  compressed --.  \\", YELLOW, color),
        paint(" |   /   .' heartwood durable '.   | ", BLUE, color),
        paint("  \\   '.   scars visible seeds   .'  /", PINK, color),
        paint("   '.   '---.          .---'   .'   ", ORANGE, color),
        paint("     '-.       '------'       .-'    ", TEAL, color),
        paint("        '==================='        ", BLUE, color),
    ]
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
    fn no_animation_welcome_can_initialize_store() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        run(&root, true, true, false).unwrap();

        assert!(root.join("memory.sqlite").exists());
    }

    #[test]
    fn json_welcome_can_initialize_store() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        run(&root, true, true, true).unwrap();

        assert!(root.join("memory.sqlite").exists());
    }
}
