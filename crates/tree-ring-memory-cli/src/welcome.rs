use serde_json::json;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;
use tree_ring_memory_sqlite::SQLiteMemoryStore;

use crate::agent_awareness::{ensure_agent_awareness, AgentAwarenessReport};

const RESET: &str = "\x1b[0m";
const YELLOW: &str = "\x1b[38;5;220m";
const BOLD: &str = "\x1b[1m";
const LOGO_64_COLOR_FRAMES: [&str; 3] = [
    include_str!("generated/logo_64_color_frame_0.ansi"),
    include_str!("generated/logo_64_color_frame_1.ansi"),
    include_str!("generated/logo_64_color_frame_2.ansi"),
];
const LOGO_80_COLOR_FRAMES: [&str; 3] = [
    include_str!("generated/logo_80_color_frame_0.ansi"),
    include_str!("generated/logo_80_color_frame_1.ansi"),
    include_str!("generated/logo_80_color_frame_2.ansi"),
];
const LOGO_96_COLOR_FRAMES: [&str; 3] = [
    include_str!("generated/logo_96_color_frame_0.ansi"),
    include_str!("generated/logo_96_color_frame_1.ansi"),
    include_str!("generated/logo_96_color_frame_2.ansi"),
];
const LOGO_112_COLOR_FRAMES: [&str; 3] = [
    include_str!("generated/logo_112_color_frame_0.ansi"),
    include_str!("generated/logo_112_color_frame_1.ansi"),
    include_str!("generated/logo_112_color_frame_2.ansi"),
];
const LOGO_64_PLAIN: &str = include_str!("generated/logo_64_plain.txt");
const LOGO_80_PLAIN: &str = include_str!("generated/logo_80_plain.txt");
const LOGO_96_PLAIN: &str = include_str!("generated/logo_96_plain.txt");
const LOGO_112_PLAIN: &str = include_str!("generated/logo_112_plain.txt");

struct LogoVariant {
    width: u16,
    frames: [&'static str; 3],
    plain: &'static str,
}

const LOGO_64: LogoVariant = LogoVariant {
    width: 64,
    frames: LOGO_64_COLOR_FRAMES,
    plain: LOGO_64_PLAIN,
};
const LOGO_80: LogoVariant = LogoVariant {
    width: 80,
    frames: LOGO_80_COLOR_FRAMES,
    plain: LOGO_80_PLAIN,
};
const LOGO_96: LogoVariant = LogoVariant {
    width: 96,
    frames: LOGO_96_COLOR_FRAMES,
    plain: LOGO_96_PLAIN,
};
const LOGO_112: LogoVariant = LogoVariant {
    width: 112,
    frames: LOGO_112_COLOR_FRAMES,
    plain: LOGO_112_PLAIN,
};

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
        animate_logo(selected_logo_variant())?;
    } else if color {
        print!("{}", selected_logo_variant().frames[1]);
    } else if let Some(variant) = selected_plain_logo_variant() {
        print!("{}", variant.plain);
    } else {
        println!("Tree Ring Memory");
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

fn terminal_width() -> u16 {
    ratatui::crossterm::terminal::size()
        .map(|(width, _)| width)
        .unwrap_or(80)
}

fn selected_logo_variant() -> &'static LogoVariant {
    let width = terminal_width();
    if width >= LOGO_112.width {
        &LOGO_112
    } else if width >= LOGO_96.width {
        &LOGO_96
    } else if width >= LOGO_80.width {
        &LOGO_80
    } else {
        &LOGO_64
    }
}

fn selected_plain_logo_variant() -> Option<&'static LogoVariant> {
    let width = terminal_width();
    if width >= LOGO_112.width {
        Some(&LOGO_112)
    } else if width >= LOGO_96.width {
        Some(&LOGO_96)
    } else if width >= LOGO_80.width {
        Some(&LOGO_80)
    } else if width >= LOGO_64.width {
        Some(&LOGO_64)
    } else {
        None
    }
}

fn animate_logo(variant: &LogoVariant) -> Result<(), String> {
    let mut stdout = io::stdout();
    let rows = variant.frames[0].lines().count();
    let sequence = [0usize, 1, 2, 1];

    for (index, frame_index) in sequence.iter().enumerate() {
        if index > 0 {
            write!(stdout, "\x1b[{rows}A\x1b[J").map_err(|err| err.to_string())?;
        }
        write!(stdout, "{}", variant.frames[*frame_index]).map_err(|err| err.to_string())?;
        stdout.flush().map_err(|err| err.to_string())?;
        thread::sleep(Duration::from_millis(85));
    }
    Ok(())
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
    fn generated_logo_frames_are_consistent() {
        for variant in [&LOGO_64, &LOGO_80, &LOGO_96, &LOGO_112] {
            let rows = variant.frames[0].lines().count();

            assert!(rows > 20);
            assert!(variant.frames[0]
                .lines()
                .all(|line| line.len() >= variant.width as usize));
            assert_eq!(variant.frames[1].lines().count(), rows);
            assert_eq!(variant.frames[2].lines().count(), rows);
            assert_eq!(variant.plain.lines().count(), rows);
            assert!(variant.plain.contains("@@@"));
        }
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
