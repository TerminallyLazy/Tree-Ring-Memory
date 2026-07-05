use clap::{Parser, Subcommand};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tree_ring_memory_core::sensitivity::SensitivityGuard;
use tree_ring_memory_core::MemoryEvent;
use tree_ring_memory_core::{decode_jsonl, normalize_import_events};
use tree_ring_memory_sqlite::{MemoryRetriever, SQLiteMemoryStore};

mod tui;

#[derive(Debug, Parser)]
#[command(name = "tree-ring", about = "Local tree-ring memory for AI agents.")]
struct Cli {
    #[arg(long, default_value = ".tree-ring", help = "memory store root")]
    root: PathBuf,
    #[arg(long, help = "emit machine-readable JSON where supported")]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "initialize a local memory store")]
    Init,
    #[command(about = "store a memory")]
    Remember {
        summary: String,
        #[arg(long)]
        event_type: String,
        #[arg(long, default_value = "cambium")]
        ring: String,
        #[arg(long, default_value = "global")]
        scope: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    #[command(about = "recall memories")]
    Recall {
        query: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long, default_value_t = 8)]
        limit: usize,
        #[arg(long)]
        include_sensitive: bool,
    },
    #[command(about = "delete or redact a memory")]
    Forget {
        memory_id: String,
        #[arg(long, default_value = "delete")]
        mode: ForgetMode,
        #[arg(long)]
        reason: String,
    },
    #[command(about = "export memories as portable JSONL")]
    Export {
        #[arg(long, help = "write JSONL export to a file instead of stdout")]
        output: Option<PathBuf>,
        #[arg(long, help = "include sensitive memories in the export")]
        include_sensitive: bool,
        #[arg(long, help = "include superseded memories in the export")]
        include_superseded: bool,
    },
    #[command(about = "import memories from portable JSONL")]
    Import {
        path: PathBuf,
        #[arg(long, help = "validate the import without writing memories")]
        dry_run: bool,
        #[arg(long, help = "replace existing memories with matching ids")]
        replace_existing: bool,
    },
    #[command(about = "open the Rust-native Tree Ring Memory terminal console")]
    Tui {
        #[arg(long, help = "optional JSONL event stream to light rings in real time")]
        event_stream: Option<PathBuf>,
        #[arg(
            long,
            default_value_t = 250,
            help = "animation and refresh cadence in milliseconds"
        )]
        tick_ms: u64,
    },
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum ForgetMode {
    Delete,
    Redact,
}

fn main() -> std::process::ExitCode {
    match run(Cli::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let db_path = cli.root.join("memory.sqlite");

    if let Command::Tui {
        event_stream,
        tick_ms,
    } = cli.command
    {
        if cli.json {
            return Err("--json is not supported with the interactive TUI".to_string());
        }
        return tui::run(cli.root, event_stream, tick_ms);
    }

    if let Command::Import {
        path,
        dry_run: true,
        replace_existing: _,
    } = cli.command
    {
        let input = fs::read_to_string(&path).map_err(|err| err.to_string())?;
        let decoded = decode_jsonl(&input).map_err(|err| err.to_string())?;
        let events = normalize_import_events(decoded.events).map_err(|err| err.to_string())?;
        if cli.json {
            println!(
                "{}",
                json!({
                    "ok": true,
                    "path": path,
                    "valid_count": events.len(),
                    "inserted_count": 0,
                    "replaced_count": 0,
                    "skipped_duplicate_count": 0,
                    "dry_run": true,
                })
            );
        } else {
            println!(
                "Tree Ring Memory import complete: valid={} inserted=0 replaced=0 skipped_duplicates=0 dry_run=true",
                events.len()
            );
        }
        return Ok(());
    }

    let mut store = SQLiteMemoryStore::open(&db_path).map_err(|err| err.to_string())?;

    match cli.command {
        Command::Init => {
            if cli.json {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "root": cli.root,
                        "sqlite_path": db_path,
                        "message": "Tree Ring Memory initialized",
                    })
                );
            } else {
                println!("Tree Ring Memory initialized at {}", cli.root.display());
                println!("No cloud sync; secret-like memory is blocked by default.");
            }
        }
        Command::Remember {
            summary,
            event_type,
            ring,
            scope,
            project,
            tags,
        } => {
            let guard = SensitivityGuard::default();
            let values = [&summary, &event_type, &ring, &scope]
                .into_iter()
                .chain(project.iter())
                .chain(tags.iter())
                .map(String::as_str);
            let detected_sensitivity = guard
                .detect_text_sensitivity(values)
                .map_err(|err| err.to_string())?;
            let mut event = MemoryEvent::new(summary, event_type).map_err(|err| err.to_string())?;
            event.ring = ring;
            event.scope = scope;
            event.project = project;
            event.tags = tags;
            if detected_sensitivity != "normal" {
                event.sensitivity = detected_sensitivity;
            }
            event.validate().map_err(|err| err.to_string())?;
            store.put(&event).map_err(|err| err.to_string())?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string(&event).map_err(|err| err.to_string())?
                );
            } else {
                println!("{}", event.id);
            }
        }
        Command::Recall {
            query,
            project,
            limit,
            include_sensitive,
        } => {
            let results = MemoryRetriever::new(&store)
                .recall(
                    &query,
                    project.as_deref(),
                    None,
                    None,
                    None,
                    None,
                    include_sensitive,
                    false,
                    limit,
                    false,
                )
                .map_err(|err| err.to_string())?;
            if cli.json {
                let payload: Vec<_> = results
                    .into_iter()
                    .map(|result| {
                        json!({
                            "memory": result.memory,
                            "score": result.score,
                            "ranking": result.ranking,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string(&payload).map_err(|err| err.to_string())?
                );
            } else {
                for result in results {
                    println!(
                        "{} [{}] {} score={:.3}",
                        result.memory.id, result.memory.ring, result.memory.summary, result.score
                    );
                }
            }
        }
        Command::Forget {
            memory_id,
            mode,
            reason,
        } => {
            if reason.trim().is_empty() {
                return Err("forget reason is required".to_string());
            }
            match mode {
                ForgetMode::Delete => store.delete(&memory_id).map_err(|err| err.to_string())?,
                ForgetMode::Redact => store.redact(&memory_id).map_err(|err| err.to_string())?,
            }
            if cli.json {
                println!("{}", json!({"ok": true, "memory_id": memory_id}));
            } else {
                println!("Tree Ring Memory forget complete: {memory_id}");
            }
        }
        Command::Export {
            output,
            include_sensitive,
            include_superseded,
        } => {
            let (jsonl, report) = store
                .export_jsonl(include_sensitive, include_superseded)
                .map_err(|err| err.to_string())?;
            if let Some(output) = output {
                if let Some(parent) = output.parent() {
                    if !parent.as_os_str().is_empty() {
                        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
                    }
                }
                fs::write(&output, jsonl).map_err(|err| err.to_string())?;
                if cli.json {
                    println!(
                        "{}",
                        json!({
                            "ok": true,
                            "path": output,
                            "memory_count": report.memory_count,
                            "sensitive_included": report.sensitive_included,
                            "superseded_included": report.superseded_included,
                        })
                    );
                } else {
                    println!(
                        "Tree Ring Memory export complete: {} memories -> {}",
                        report.memory_count,
                        output.display()
                    );
                }
            } else {
                print!("{jsonl}");
            }
        }
        Command::Import {
            path,
            dry_run,
            replace_existing,
        } => {
            let input = fs::read_to_string(&path).map_err(|err| err.to_string())?;
            let report = store
                .import_jsonl(&input, dry_run, replace_existing)
                .map_err(|err| err.to_string())?;
            if cli.json {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "path": path,
                        "valid_count": report.valid_count,
                        "inserted_count": report.inserted_count,
                        "replaced_count": report.replaced_count,
                        "skipped_duplicate_count": report.skipped_duplicate_count,
                        "dry_run": report.dry_run,
                    })
                );
            } else {
                println!(
                    "Tree Ring Memory import complete: valid={} inserted={} replaced={} skipped_duplicates={} dry_run={}",
                    report.valid_count,
                    report.inserted_count,
                    report.replaced_count,
                    report.skipped_duplicate_count,
                    report.dry_run
                );
            }
        }
        Command::Tui { .. } => unreachable!("tui returns before opening the scriptable store"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn cli_init_creates_store() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        run(Cli {
            root: root.clone(),
            json: false,
            command: Command::Init,
        })
        .unwrap();

        assert!(root.join("memory.sqlite").exists());
    }

    #[test]
    fn blank_forget_reason_is_controlled_error() {
        let dir = tempdir().unwrap();
        let err = run(Cli {
            root: dir.path().join(".tree-ring"),
            json: false,
            command: Command::Forget {
                memory_id: "mem_missing".to_string(),
                mode: ForgetMode::Delete,
                reason: "  ".to_string(),
            },
        })
        .unwrap_err();

        assert_eq!(err, "forget reason is required");
    }

    #[test]
    fn remember_secret_project_is_blocked() {
        let dir = tempdir().unwrap();
        let err = run(Cli {
            root: dir.path().join(".tree-ring"),
            json: false,
            command: Command::Remember {
                summary: "Facade should guard project metadata.".to_string(),
                event_type: "lesson".to_string(),
                ring: "cambium".to_string(),
                scope: "global".to_string(),
                project: Some("sk-proj-abcdefghijklmnopqrstuvwxyz1234567890".to_string()),
                tags: Vec::new(),
            },
        })
        .unwrap_err();

        assert!(err.contains("blocked"));
    }

    #[test]
    fn remember_json_emits_memory_payload() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        run(Cli {
            root: root.clone(),
            json: false,
            command: Command::Init,
        })
        .unwrap();

        run(Cli {
            root,
            json: true,
            command: Command::Remember {
                summary: "Use Rust JSON bridge.".to_string(),
                event_type: "lesson".to_string(),
                ring: "cambium".to_string(),
                scope: "global".to_string(),
                project: None,
                tags: Vec::new(),
            },
        })
        .unwrap();
    }

    #[test]
    fn remember_classifies_sensitive_metadata_before_default_recall() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        run(Cli {
            root: root.clone(),
            json: false,
            command: Command::Remember {
                summary: "Rust CLI classifies metadata.".to_string(),
                event_type: "lesson".to_string(),
                ring: "cambium".to_string(),
                scope: "global".to_string(),
                project: None,
                tags: vec!["private diagnosis".to_string()],
            },
        })
        .unwrap();

        let store = SQLiteMemoryStore::open(root.join("memory.sqlite")).unwrap();
        let hidden = MemoryRetriever::new(&store)
            .recall(
                "classifies metadata",
                None,
                None,
                None,
                None,
                None,
                false,
                false,
                8,
                false,
            )
            .unwrap();
        let visible = MemoryRetriever::new(&store)
            .recall(
                "classifies metadata",
                None,
                None,
                None,
                None,
                None,
                true,
                false,
                8,
                false,
            )
            .unwrap();

        assert!(hidden.is_empty());
        assert_eq!(visible[0].memory.sensitivity, "health");
    }

    #[test]
    fn parses_tui_command() {
        let cli = Cli::try_parse_from([
            "tree-ring",
            "--root",
            ".memory",
            "tui",
            "--event-stream",
            "events.jsonl",
            "--tick-ms",
            "125",
        ])
        .unwrap();

        match cli.command {
            Command::Tui {
                event_stream,
                tick_ms,
            } => {
                assert_eq!(event_stream.unwrap(), PathBuf::from("events.jsonl"));
                assert_eq!(tick_ms, 125);
            }
            _ => panic!("expected tui command"),
        }
    }

    #[test]
    fn tui_rejects_json_mode_before_terminal_start() {
        let dir = tempdir().unwrap();

        let err = run(Cli {
            root: dir.path().join(".tree-ring"),
            json: true,
            command: Command::Tui {
                event_stream: None,
                tick_ms: 250,
            },
        })
        .unwrap_err();

        assert!(err.contains("--json is not supported"));
    }
}
