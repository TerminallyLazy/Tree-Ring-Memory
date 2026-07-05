use clap::{Parser, Subcommand};
use serde_json::json;
use std::path::PathBuf;
use tree_ring_memory_core::sensitivity::SensitivityGuard;
use tree_ring_memory_core::MemoryEvent;
use tree_ring_memory_sqlite::{MemoryRetriever, SQLiteMemoryStore};

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
            for value in [&summary, &event_type, &ring, &scope]
                .into_iter()
                .chain(project.iter())
                .chain(tags.iter())
            {
                guard.check_or_raise(value).map_err(|err| err.to_string())?;
            }
            let detected_sensitivity = guard.inspect(&summary).sensitivity;
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
                println!("{}", serde_json::to_string(&event).map_err(|err| err.to_string())?);
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
                println!("{}", serde_json::to_string(&payload).map_err(|err| err.to_string())?);
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
}
