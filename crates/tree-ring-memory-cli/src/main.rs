use clap::{Parser, Subcommand};
use serde_json::json;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use tree_ring_memory_core::plan_maintenance;
use tree_ring_memory_core::sensitivity::SensitivityGuard;
use tree_ring_memory_core::{
    audit_memories, collect_dox_memories, collect_revolve_memories, consolidate_memories,
    decode_jsonl, normalize_import_events, AuditReport, ConsolidationPeriod, ConsolidationReport,
    ConsolidationRequest, DoxSyncReport, DoxSyncRequest, MaintenanceReport, MaintenanceRequest,
    RevolveSyncReport, RevolveSyncRequest,
};
use tree_ring_memory_core::{MemoryEvent, MemoryLink};
use tree_ring_memory_sqlite::{MemoryRetriever, SQLiteMemoryStore};

mod agent_awareness;
mod integrations;
mod tui;
mod welcome;

#[derive(Debug, Parser)]
#[command(
    name = "tree-ring",
    version,
    about = "Local tree-ring memory for AI agents."
)]
struct Cli {
    #[arg(
        long,
        default_value = ".tree-ring",
        global = true,
        help = "memory store root"
    )]
    root: PathBuf,
    #[arg(
        long,
        global = true,
        help = "emit machine-readable JSON where supported"
    )]
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
    #[command(about = "record an evidence-backed improvement-loop outcome")]
    Evidence {
        summary: String,
        #[arg(
            long,
            default_value = "observed",
            help = "observed, promoted, rejected, or deferred"
        )]
        outcome: String,
        #[arg(
            long,
            help = "file path, run id, checkpoint id, PR, issue, or eval ref"
        )]
        evidence_ref: String,
        #[arg(long, help = "optional project scope")]
        project: Option<String>,
        #[arg(long, help = "optional extra context")]
        details: Option<String>,
        #[arg(long, help = "optional numeric evaluation score")]
        score: Option<f64>,
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
    #[command(about = "audit memory quality, privacy, and integrity")]
    Audit {
        #[arg(
            long,
            default_value = "all",
            help = "all, stale, sensitive, low_confidence, supersession, or contradictions"
        )]
        audit_type: String,
    },
    #[command(about = "consolidate memories into deterministic ring summaries")]
    Consolidate {
        #[arg(
            long,
            default_value = "daily",
            help = "daily, weekly, monthly, yearly, or manual"
        )]
        period_type: String,
        #[arg(
            long,
            help = "stable period key; derived from current UTC time when omitted"
        )]
        period_key: Option<String>,
        #[arg(long, help = "optional project filter")]
        project: Option<String>,
        #[arg(long, help = "plan consolidation without writing summaries or records")]
        dry_run: bool,
        #[arg(
            long,
            help = "create a new consolidation and supersede prior summaries"
        )]
        force: bool,
    },
    #[command(about = "plan or apply Rust-owned memory maintenance")]
    Maintain {
        #[arg(long, help = "optional project filter")]
        project: Option<String>,
        #[arg(long, help = "include superseded memories in maintenance planning")]
        include_superseded: bool,
        #[arg(long, help = "delete expired temporary memories")]
        apply_expired: bool,
        #[arg(long, help = "redact memories with secret-like content")]
        apply_secret_redactions: bool,
        #[arg(long, help = "rebuild the SQLite FTS index")]
        repair_fts: bool,
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
    #[command(about = "show first-run onboarding and next commands")]
    Welcome {
        #[arg(long, help = "initialize the configured memory root during onboarding")]
        init: bool,
        #[arg(long, help = "print a stable onboarding screen without animation")]
        no_animation: bool,
    },
    #[command(about = "summarize DOX-style AGENTS.md guidance into memory")]
    Dox {
        #[command(subcommand)]
        command: DoxCommand,
    },
    #[command(about = "import Revolve-style evidence records into memory")]
    Revolve {
        #[command(subcommand)]
        command: RevolveCommand,
    },
    #[command(about = "discover local agent-framework integration markers")]
    Integrations {
        #[command(subcommand)]
        command: IntegrationCommand,
    },
}

#[derive(Debug, Subcommand)]
enum DoxCommand {
    #[command(about = "scan AGENTS.md files and store concise source-linked memories")]
    Sync {
        #[arg(
            long,
            default_value = ".",
            help = "project root or AGENTS.md file to scan"
        )]
        source_root: PathBuf,
        #[arg(long, help = "optional project scope for imported memories")]
        project: Option<String>,
        #[arg(long, help = "preview generated memories without writing them")]
        dry_run: bool,
    },
}

#[derive(Debug, Subcommand)]
enum RevolveCommand {
    #[command(about = "scan Revolve records and store evidence-linked memories")]
    Sync {
        #[arg(
            long,
            default_value = "revolve",
            help = "Revolve root or evidence file to scan"
        )]
        source_root: PathBuf,
        #[arg(long, help = "optional project scope for imported memories")]
        project: Option<String>,
        #[arg(long, help = "preview generated memories without writing them")]
        dry_run: bool,
    },
}

#[derive(Debug, Subcommand)]
enum IntegrationCommand {
    #[command(about = "scan a project root for known agent-framework markers")]
    Scan {
        #[arg(long, default_value = ".", help = "project root to scan")]
        source_root: PathBuf,
    },
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum ForgetMode {
    Delete,
    Redact,
}

fn main() -> std::process::ExitCode {
    let args = std::env::args_os().collect::<Vec<_>>();
    if let Some((root, json_output)) = global_welcome_request(&args) {
        return exit_from_result(welcome::run(&root, false, false, json_output));
    }
    exit_from_result(run(Cli::parse_from(args)))
}

fn exit_from_result(result: Result<(), String>) -> std::process::ExitCode {
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::from(2)
        }
    }
}

fn global_welcome_request(args: &[OsString]) -> Option<(PathBuf, bool)> {
    let mut index = 1usize;
    let mut root = PathBuf::from(".tree-ring");
    let mut json_output = false;
    while index < args.len() {
        let arg = args[index].to_str()?;
        match arg {
            "--json" => {
                json_output = true;
                index += 1;
            }
            "--root" => {
                let value = args.get(index + 1)?;
                root = PathBuf::from(value);
                index += 2;
            }
            "-h" | "--help" | "-V" | "--version" => return None,
            value if value.starts_with("--root=") => {
                root = PathBuf::from(value.trim_start_matches("--root="));
                index += 1;
            }
            value if value.starts_with('-') => return None,
            _command => return None,
        }
    }
    Some((root, json_output))
}

fn run(cli: Cli) -> Result<(), String> {
    let db_path = cli.root.join("memory.sqlite");

    if let Command::Welcome { init, no_animation } = &cli.command {
        return welcome::run(&cli.root, *init, *no_animation, cli.json);
    }

    if let Command::Integrations {
        command: IntegrationCommand::Scan { source_root },
    } = &cli.command
    {
        let report = integrations::scan_integrations(source_root);
        print_integration_report(&report, cli.json)?;
        return Ok(());
    }

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

    if let Command::Audit { audit_type } = &cli.command {
        let report = if db_path.exists() {
            SQLiteMemoryStore::open_read_only(&db_path)
                .and_then(|store| store.audit(audit_type))
                .map_err(|err| err.to_string())?
        } else {
            audit_memories(&[], audit_type).map_err(|err| err.to_string())?
        };
        print_audit_report(&report, cli.json)?;
        return Ok(());
    }

    if let Command::Consolidate {
        period_type,
        period_key,
        project,
        dry_run: true,
        force,
    } = &cli.command
    {
        let request = consolidation_request(
            period_type,
            period_key.clone(),
            project.clone(),
            true,
            *force,
        )?;
        let report = if db_path.exists() {
            let store =
                SQLiteMemoryStore::open_read_only(&db_path).map_err(|err| err.to_string())?;
            let events = store.list_all(false).map_err(|err| err.to_string())?;
            consolidate_memories(&events, &request).map_err(|err| err.to_string())?
        } else {
            consolidate_memories(&[], &request).map_err(|err| err.to_string())?
        };
        print_consolidation_report(&report, cli.json)?;
        return Ok(());
    }

    if let Command::Maintain {
        project,
        include_superseded,
        apply_expired,
        apply_secret_redactions,
        repair_fts,
    } = &cli.command
    {
        let request = maintenance_request(
            project.clone(),
            *include_superseded,
            *apply_expired,
            *apply_secret_redactions,
            *repair_fts,
        );
        if request.dry_run {
            let report = if db_path.exists() {
                let mut store =
                    SQLiteMemoryStore::open_read_only(&db_path).map_err(|err| err.to_string())?;
                store.maintain(&request).map_err(|err| err.to_string())?
            } else {
                plan_maintenance(&[], &request)
            };
            print_maintenance_report(&report, cli.json)?;
            return Ok(());
        }
    }

    if let Command::Dox {
        command:
            DoxCommand::Sync {
                source_root,
                project,
                dry_run: true,
            },
    } = &cli.command
    {
        let report = collect_dox_memories(&dox_request(source_root.clone(), project.clone()))
            .map_err(|err| err.to_string())?;
        print_dox_report(&report, cli.json, true)?;
        return Ok(());
    }

    if let Command::Revolve {
        command:
            RevolveCommand::Sync {
                source_root,
                project,
                dry_run: true,
            },
    } = &cli.command
    {
        let report =
            collect_revolve_memories(&revolve_request(source_root.clone(), project.clone()))
                .map_err(|err| err.to_string())?;
        print_revolve_report(&report, cli.json, true)?;
        return Ok(());
    }

    let mut store = SQLiteMemoryStore::open(&db_path).map_err(|err| err.to_string())?;

    match cli.command {
        Command::Init => {
            let awareness = agent_awareness::ensure_agent_awareness(&cli.root)
                .map_err(|err| err.to_string())?;
            if cli.json {
                println!(
                    "{}",
                    json!({
                        "ok": true,
                        "root": cli.root,
                        "sqlite_path": db_path,
                        "message": "Tree Ring Memory initialized",
                        "agent_awareness": awareness,
                    })
                );
            } else {
                println!("Tree Ring Memory initialized at {}", cli.root.display());
                println!("No cloud sync; secret-like memory is blocked by default.");
                print_agent_awareness_summary(&awareness);
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
        Command::Evidence {
            summary,
            outcome,
            evidence_ref,
            project,
            details,
            score,
            tags,
        } => {
            let event = evidence_event(
                summary,
                outcome,
                evidence_ref,
                project,
                details,
                score,
                tags,
            )?;
            store.put(&event).map_err(|err| err.to_string())?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string(&event).map_err(|err| err.to_string())?
                );
            } else {
                println!(
                    "{} [{}] {} evidence={}",
                    event.id, event.ring, event.summary, event.source.ref_
                );
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
        Command::Audit { .. } => unreachable!("audit returns before opening the writable store"),
        Command::Consolidate {
            period_type,
            period_key,
            project,
            dry_run,
            force,
        } => {
            let request = consolidation_request(&period_type, period_key, project, dry_run, force)?;
            let report = store.consolidate(&request).map_err(|err| err.to_string())?;
            print_consolidation_report(&report, cli.json)?;
        }
        Command::Maintain {
            project,
            include_superseded,
            apply_expired,
            apply_secret_redactions,
            repair_fts,
        } => {
            let request = maintenance_request(
                project,
                include_superseded,
                apply_expired,
                apply_secret_redactions,
                repair_fts,
            );
            let report = store.maintain(&request).map_err(|err| err.to_string())?;
            print_maintenance_report(&report, cli.json)?;
        }
        Command::Tui { .. } => unreachable!("tui returns before opening the scriptable store"),
        Command::Welcome { .. } => {
            unreachable!("welcome returns before opening the scriptable store")
        }
        Command::Integrations { .. } => {
            unreachable!("integrations scan returns before opening the scriptable store")
        }
        Command::Dox {
            command:
                DoxCommand::Sync {
                    source_root,
                    project,
                    dry_run,
                },
        } => {
            let report = collect_dox_memories(&dox_request(source_root, project))
                .map_err(|err| err.to_string())?;
            if !dry_run {
                store
                    .put_many(&report.events)
                    .map_err(|err| err.to_string())?;
            }
            print_dox_report(&report, cli.json, dry_run)?;
        }
        Command::Revolve {
            command:
                RevolveCommand::Sync {
                    source_root,
                    project,
                    dry_run,
                },
        } => {
            let report = collect_revolve_memories(&revolve_request(source_root, project))
                .map_err(|err| err.to_string())?;
            if !dry_run {
                store
                    .put_many(&report.events)
                    .map_err(|err| err.to_string())?;
            }
            print_revolve_report(&report, cli.json, dry_run)?;
        }
    }
    Ok(())
}

fn dox_request(source_root: PathBuf, project: Option<String>) -> DoxSyncRequest {
    let mut request = DoxSyncRequest::new(source_root);
    request.project = project;
    request
}

fn revolve_request(source_root: PathBuf, project: Option<String>) -> RevolveSyncRequest {
    let mut request = RevolveSyncRequest::new(source_root);
    request.project = project;
    request
}

fn maintenance_request(
    project: Option<String>,
    include_superseded: bool,
    apply_expired: bool,
    apply_secret_redactions: bool,
    repair_fts: bool,
) -> MaintenanceRequest {
    MaintenanceRequest {
        dry_run: !(apply_expired || apply_secret_redactions || repair_fts),
        apply_expired,
        apply_secret_redactions,
        repair_fts,
        include_superseded,
        project,
    }
}

fn consolidation_request(
    period_type: &str,
    period_key: Option<String>,
    project: Option<String>,
    dry_run: bool,
    force: bool,
) -> Result<ConsolidationRequest, String> {
    Ok(ConsolidationRequest {
        period_type: ConsolidationPeriod::parse(period_type).map_err(|err| err.to_string())?,
        period_key,
        project,
        dry_run,
        force,
    })
}

fn evidence_event(
    summary: String,
    outcome: String,
    evidence_ref: String,
    project: Option<String>,
    details: Option<String>,
    score: Option<f64>,
    tags: Vec<String>,
) -> Result<MemoryEvent, String> {
    if summary.trim().is_empty() {
        return Err("evidence summary is required".to_string());
    }
    if evidence_ref.trim().is_empty() {
        return Err("evidence-ref is required".to_string());
    }
    if let Some(score) = score {
        if !(0.0..=1.0).contains(&score) {
            return Err("evidence score must be between 0 and 1".to_string());
        }
    }

    let normalized_outcome = outcome.trim().to_ascii_lowercase();
    let (ring, event_type, salience, confidence, retention) = match normalized_outcome.as_str() {
        "promoted" | "promotion" => (
            "heartwood",
            "evaluation_promotion",
            0.86,
            score.unwrap_or(0.84).max(0.75),
            "durable",
        ),
        "rejected" | "rejection" => (
            "scar",
            "evaluation_rejection",
            0.90,
            score.unwrap_or(0.78),
            "durable",
        ),
        "deferred" | "seed" | "hypothesis" => (
            "seed",
            "evaluation_hypothesis",
            0.68,
            score.unwrap_or(0.60),
            "normal",
        ),
        "observed" | "observation" | "result" => (
            "outer",
            "evaluation_result",
            0.72,
            score.unwrap_or(0.70),
            "normal",
        ),
        _ => {
            return Err(
                "evidence outcome must be observed, promoted, rejected, or deferred".to_string(),
            )
        }
    };

    let guard = SensitivityGuard::default();
    let values = [&summary, &outcome, &evidence_ref]
        .into_iter()
        .chain(project.iter())
        .chain(details.iter())
        .chain(tags.iter())
        .map(String::as_str);
    let detected_sensitivity = guard
        .detect_text_sensitivity(values)
        .map_err(|err| err.to_string())?;

    let mut event = MemoryEvent::new(summary.trim(), event_type).map_err(|err| err.to_string())?;
    event.ring = ring.to_string();
    event.scope = "eval".to_string();
    event.project = project;
    event.details = evidence_details(&normalized_outcome, score, details);
    event.source.source_type = "evidence".to_string();
    event.source.ref_ = evidence_ref.trim().to_string();
    event.tags = evidence_tags(normalized_outcome.as_str(), tags);
    event.salience = salience;
    event.confidence = confidence.clamp(0.0, 1.0);
    event.retention = retention.to_string();
    event.links.push(MemoryLink {
        link_type: "evidence".to_string(),
        target: event.source.ref_.clone(),
    });
    if detected_sensitivity != "normal" {
        event.sensitivity = detected_sensitivity;
    }
    event.validate().map_err(|err| err.to_string())?;
    Ok(event)
}

fn evidence_details(outcome: &str, score: Option<f64>, details: Option<String>) -> String {
    let mut lines = vec![format!("Outcome: {outcome}")];
    if let Some(score) = score {
        lines.push(format!("Score: {score:.3}"));
    }
    if let Some(details) = details {
        let trimmed = details.trim();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }
    lines.join("\n")
}

fn evidence_tags(outcome: &str, mut tags: Vec<String>) -> Vec<String> {
    tags.push("evidence".to_string());
    tags.push("improvement-loop".to_string());
    tags.push(format!("outcome:{outcome}"));
    tags.sort();
    tags.dedup();
    tags
}

fn print_audit_report(report: &AuditReport, json_output: bool) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string(report).map_err(|err| err.to_string())?
        );
    } else {
        println!(
            "Tree Ring Memory audit: type={} memories={} findings={}",
            report.audit_type, report.memory_count, report.finding_count
        );
        for finding in &report.findings {
            let memory_id = finding.memory_id.as_deref().unwrap_or("-");
            let related = finding.related_memory_id.as_deref().unwrap_or("-");
            println!(
                "{} [{}] memory={} related={} {} -> {}",
                finding.audit_type,
                finding.severity,
                memory_id,
                related,
                finding.finding,
                finding.recommended_action
            );
        }
    }
    Ok(())
}

fn print_consolidation_report(
    report: &ConsolidationReport,
    json_output: bool,
) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string(report).map_err(|err| err.to_string())?
        );
    } else {
        println!(
            "Tree Ring Memory consolidation: type={} key={} candidates={} outputs={} status={}",
            report.period_type,
            report.period_key,
            report.candidate_count,
            report.output_memory_ids.len(),
            report.status
        );
        if !report.notes.is_empty() {
            println!("{}", report.notes);
        }
    }
    Ok(())
}

fn print_maintenance_report(report: &MaintenanceReport, json_output: bool) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string(report).map_err(|err| err.to_string())?
        );
    } else {
        println!(
            "Tree Ring Memory maintenance: memories={} planned={} applied={} dry_run={} status={}",
            report.memory_count,
            report.planned_action_count,
            report.applied_action_count,
            report.dry_run,
            report.status
        );
        println!(
            "FTS: memories={} index={} missing={} orphan={} repaired={}",
            report.fts.memory_rows,
            report.fts.fts_rows,
            report.fts.missing_fts_rows,
            report.fts.orphan_fts_rows,
            report.fts.repaired
        );
        for action in &report.actions {
            println!(
                "{} [{}] memory={} applied={} {}",
                action.action_type,
                action.severity,
                action.memory_id,
                action.applied,
                action.reason
            );
        }
        if report.dry_run {
            println!(
                "Report-only: use --apply-expired, --apply-secret-redactions, or --repair-fts to apply eligible maintenance."
            );
        }
    }
    Ok(())
}

fn print_dox_report(
    report: &DoxSyncReport,
    json_output: bool,
    dry_run: bool,
) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            json!({
                "ok": true,
                "dry_run": dry_run,
                "report": report,
            })
        );
    } else {
        println!(
            "Tree Ring Memory DOX sync: sources={} memories={} skipped_secret={} dry_run={}",
            report.source_count, report.memory_count, report.skipped_secret_count, dry_run
        );
        for warning in &report.warnings {
            println!("warning: {warning}");
        }
        for event in &report.events {
            println!(
                "{} [{}] {} <- {}",
                event.id, event.ring, event.summary, event.source.ref_
            );
        }
        println!("Source AGENTS.md files remain authoritative; re-read them before acting.");
    }
    Ok(())
}

fn print_revolve_report(
    report: &RevolveSyncReport,
    json_output: bool,
    dry_run: bool,
) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            json!({
                "ok": true,
                "dry_run": dry_run,
                "report": report,
            })
        );
    } else {
        println!(
            "Tree Ring Memory Revolve sync: sources={} memories={} skipped_large={} skipped_secret={} dry_run={}",
            report.source_count,
            report.memory_count,
            report.skipped_large_count,
            report.skipped_secret_count,
            dry_run
        );
        for warning in &report.warnings {
            println!("warning: {warning}");
        }
        for event in &report.events {
            println!(
                "{} [{}] {} <- {}",
                event.id, event.ring, event.summary, event.source.ref_
            );
        }
        println!("Revolve records remain authoritative; re-read evaluations before treating memory as current truth.");
    }
    Ok(())
}

fn print_integration_report(
    report: &integrations::IntegrationScanReport,
    json_output: bool,
) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            json!({
                "ok": true,
                "report": report,
            })
        );
    } else {
        println!(
            "Tree Ring Memory integrations: root={} detected={}",
            report.root.display(),
            report.detected_count
        );
        for integration in &report.integrations {
            println!(
                "{} [{:?}] confidence={:.2}",
                integration.name, integration.status, integration.confidence
            );
            if !integration.markers.is_empty() {
                println!("  markers: {}", integration.markers.join(", "));
            }
            println!("  next: {}", integration.next_step);
        }
    }
    Ok(())
}

fn print_agent_awareness_summary(report: &agent_awareness::AgentAwarenessReport) {
    if !report.created.is_empty() {
        println!("Agent awareness files created:");
        for path in &report.created {
            println!("  {}", path.display());
        }
    }
    if !report.existing.is_empty() {
        println!("Agent awareness files already present:");
        for path in &report.existing {
            println!("  {}", path.display());
        }
    }
    println!("If this repo uses DOX, merge .tree-ring/AGENTS.md guidance into the project root AGENTS.md when you want agents to see it before entering .tree-ring.");
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
        assert!(root.join("AGENTS.md").exists());
        assert!(root.join("SKILL.md").exists());
        assert!(root.join("CLI.md").exists());
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
    fn evidence_promotion_becomes_heartwood_with_evidence_source() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        run(Cli {
            root: root.clone(),
            json: false,
            command: Command::Evidence {
                summary: "Snapshot invalidation fixed stale unread chat state.".to_string(),
                outcome: "promoted".to_string(),
                evidence_ref: "evals/chat-state/run-042".to_string(),
                project: Some("agent-ui".to_string()),
                details: Some("Passed regression suite and manual replay.".to_string()),
                score: Some(0.91),
                tags: vec!["chat".to_string()],
            },
        })
        .unwrap();

        let store = SQLiteMemoryStore::open(root.join("memory.sqlite")).unwrap();
        let memories = store.list_all(false).unwrap();
        assert_eq!(memories.len(), 1);
        let memory = &memories[0];
        assert_eq!(memory.ring, "heartwood");
        assert_eq!(memory.scope, "eval");
        assert_eq!(memory.event_type, "evaluation_promotion");
        assert_eq!(memory.retention, "durable");
        assert_eq!(memory.source.source_type, "evidence");
        assert_eq!(memory.source.ref_, "evals/chat-state/run-042");
        assert!(memory.tags.contains(&"improvement-loop".to_string()));
        assert!(memory.tags.contains(&"outcome:promoted".to_string()));
    }

    #[test]
    fn evidence_rejection_becomes_scar() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        run(Cli {
            root: root.clone(),
            json: false,
            command: Command::Evidence {
                summary: "Aggressive caching caused stale multi-chat state.".to_string(),
                outcome: "rejected".to_string(),
                evidence_ref: "evals/cache-branch/run-013".to_string(),
                project: Some("agent-ui".to_string()),
                details: None,
                score: Some(0.82),
                tags: Vec::new(),
            },
        })
        .unwrap();

        let store = SQLiteMemoryStore::open(root.join("memory.sqlite")).unwrap();
        let memories = store.list_all(false).unwrap();
        assert_eq!(memories[0].ring, "scar");
        assert_eq!(memories[0].event_type, "evaluation_rejection");
        assert_eq!(memories[0].retention, "durable");
    }

    #[test]
    fn evidence_rejects_invalid_scores() {
        let dir = tempdir().unwrap();
        let err = run(Cli {
            root: dir.path().join(".tree-ring"),
            json: false,
            command: Command::Evidence {
                summary: "Invalid evidence score".to_string(),
                outcome: "observed".to_string(),
                evidence_ref: "evals/run".to_string(),
                project: None,
                details: None,
                score: Some(2.0),
                tags: Vec::new(),
            },
        })
        .unwrap_err();

        assert_eq!(err, "evidence score must be between 0 and 1");
    }

    #[test]
    fn dox_sync_dry_run_does_not_create_store() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("AGENTS.md"),
            "# Rules\nYou must run focused tests before full cargo test.",
        )
        .unwrap();
        let root = dir.path().join(".tree-ring");

        run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Dox {
                command: DoxCommand::Sync {
                    source_root: dir.path().to_path_buf(),
                    project: Some("tree-ring".to_string()),
                    dry_run: true,
                },
            },
        })
        .unwrap();

        assert!(!root.exists());
    }

    #[test]
    fn dox_sync_persists_source_linked_contract_memory() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("AGENTS.md"),
            "# Contract\nYou must keep memory source documents authoritative.",
        )
        .unwrap();
        let root = dir.path().join(".tree-ring");

        run(Cli {
            root: root.clone(),
            json: false,
            command: Command::Dox {
                command: DoxCommand::Sync {
                    source_root: dir.path().to_path_buf(),
                    project: Some("tree-ring".to_string()),
                    dry_run: false,
                },
            },
        })
        .unwrap();

        let store = SQLiteMemoryStore::open(root.join("memory.sqlite")).unwrap();
        let memories = store.list_all(false).unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].scope, "dox");
        assert_eq!(memories[0].ring, "heartwood");
        assert_eq!(memories[0].source.source_type, "dox");
        assert_eq!(memories[0].source.ref_, "AGENTS.md#contract-2");
    }

    #[test]
    fn integrations_scan_is_read_only_and_detects_project_markers() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# Rules").unwrap();
        fs::create_dir_all(dir.path().join("revolve")).unwrap();
        let root = dir.path().join(".tree-ring");

        run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Integrations {
                command: IntegrationCommand::Scan {
                    source_root: dir.path().to_path_buf(),
                },
            },
        })
        .unwrap();

        assert!(!root.exists());
        let report = integrations::scan_integrations(dir.path());
        assert!(report.detected_count >= 2);
    }

    #[test]
    fn revolve_sync_persists_rejection_as_scar() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("revolve/projects/ui/branches/cache")).unwrap();
        fs::write(
            dir.path()
                .join("revolve/projects/ui/branches/cache/AGENTS.md"),
            "# Rejected\nRejected aggressive caching after stale state regression.",
        )
        .unwrap();
        let root = dir.path().join(".tree-ring");

        run(Cli {
            root: root.clone(),
            json: false,
            command: Command::Revolve {
                command: RevolveCommand::Sync {
                    source_root: dir.path().join("revolve"),
                    project: Some("ui".to_string()),
                    dry_run: false,
                },
            },
        })
        .unwrap();

        let store = SQLiteMemoryStore::open(root.join("memory.sqlite")).unwrap();
        let memories = store.list_all(false).unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].scope, "revolve");
        assert_eq!(memories[0].ring, "scar");
        assert_eq!(memories[0].event_type, "evaluation_rejection");
        assert_eq!(memories[0].source.source_type, "revolve");
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
    fn audit_json_reports_findings_without_mutating_store() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        run(Cli {
            root: root.clone(),
            json: false,
            command: Command::Remember {
                summary: "Private diagnosis should be reviewed.".to_string(),
                event_type: "lesson".to_string(),
                ring: "cambium".to_string(),
                scope: "global".to_string(),
                project: None,
                tags: Vec::new(),
            },
        })
        .unwrap();
        let before = SQLiteMemoryStore::open(root.join("memory.sqlite"))
            .unwrap()
            .list_all(true)
            .unwrap();

        run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Audit {
                audit_type: "sensitive".to_string(),
            },
        })
        .unwrap();

        let after = SQLiteMemoryStore::open(root.join("memory.sqlite"))
            .unwrap()
            .list_all(true)
            .unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn audit_missing_root_does_not_create_store() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Audit {
                audit_type: "all".to_string(),
            },
        })
        .unwrap();

        assert!(!root.exists());
    }

    #[test]
    fn audit_existing_store_does_not_create_sqlite_sidecars() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        let db_path = root.join("memory.sqlite");
        let wal_path = root.join("memory.sqlite-wal");
        let shm_path = root.join("memory.sqlite-shm");
        run(Cli {
            root: root.clone(),
            json: false,
            command: Command::Init,
        })
        .unwrap();
        let store = SQLiteMemoryStore::open(&db_path).unwrap();
        store
            .connection()
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        drop(store);
        let _ = fs::remove_file(&wal_path);
        let _ = fs::remove_file(&shm_path);
        assert!(db_path.exists());
        assert!(!wal_path.exists());
        assert!(!shm_path.exists());

        run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Audit {
                audit_type: "all".to_string(),
            },
        })
        .unwrap();

        assert!(!wal_path.exists());
        assert!(!shm_path.exists());
    }

    #[test]
    fn consolidate_dry_run_missing_root_does_not_create_store() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Consolidate {
                period_type: "manual".to_string(),
                period_key: Some("manual-test".to_string()),
                project: None,
                dry_run: true,
                force: false,
            },
        })
        .unwrap();

        assert!(!root.exists());
    }

    #[test]
    fn consolidate_empty_creates_store_without_summary_or_record() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Consolidate {
                period_type: "manual".to_string(),
                period_key: Some("manual-empty".to_string()),
                project: Some("core".to_string()),
                dry_run: false,
                force: false,
            },
        })
        .unwrap();
        let store = SQLiteMemoryStore::open(root.join("memory.sqlite")).unwrap();
        let memories: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM memories", [], |row| row.get(0))
            .unwrap();
        let records: i64 = store
            .connection()
            .query_row("SELECT count(*) FROM consolidations", [], |row| row.get(0))
            .unwrap();

        assert_eq!(memories, 0);
        assert_eq!(records, 0);
    }

    #[test]
    fn consolidate_json_creates_summary_and_is_idempotent() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        run(Cli {
            root: root.clone(),
            json: false,
            command: Command::Remember {
                summary: "Use deterministic consolidation.".to_string(),
                event_type: "decision".to_string(),
                ring: "cambium".to_string(),
                scope: "global".to_string(),
                project: Some("core".to_string()),
                tags: vec!["memory".to_string()],
            },
        })
        .unwrap();

        run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Consolidate {
                period_type: "manual".to_string(),
                period_key: Some("manual-test".to_string()),
                project: Some("core".to_string()),
                dry_run: false,
                force: false,
            },
        })
        .unwrap();
        let second = SQLiteMemoryStore::open(root.join("memory.sqlite"))
            .unwrap()
            .consolidate(&ConsolidationRequest {
                period_type: ConsolidationPeriod::Manual,
                period_key: Some("manual-test".to_string()),
                project: Some("core".to_string()),
                dry_run: false,
                force: false,
            })
            .unwrap();

        assert_eq!(second.status, "unchanged");
        assert_eq!(second.output_memory_ids.len(), 1);
    }

    #[test]
    fn maintain_default_missing_root_does_not_create_store() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Maintain {
                project: None,
                include_superseded: false,
                apply_expired: false,
                apply_secret_redactions: false,
                repair_fts: false,
            },
        })
        .unwrap();

        assert!(!root.exists());
    }

    #[test]
    fn maintain_apply_expired_deletes_temporary_memory() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        let mut store = SQLiteMemoryStore::open(root.join("memory.sqlite")).unwrap();
        let mut event = MemoryEvent::new("Delete expired CLI memory.", "lesson").unwrap();
        event.retention = "ephemeral".to_string();
        event.expires_at = Some("2000-01-01T00:00:00Z".to_string());
        let memory_id = event.id.clone();
        store.put(&event).unwrap();
        drop(store);

        run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Maintain {
                project: None,
                include_superseded: false,
                apply_expired: true,
                apply_secret_redactions: false,
                repair_fts: false,
            },
        })
        .unwrap();

        let store = SQLiteMemoryStore::open(root.join("memory.sqlite")).unwrap();
        assert!(store.get(&memory_id).unwrap().is_none());
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
    fn parses_global_flags_after_welcome_command() {
        let cli = Cli::try_parse_from([
            "tree-ring",
            "welcome",
            "--json",
            "--root",
            ".memory",
            "--init",
        ])
        .unwrap();

        assert!(cli.json);
        assert_eq!(cli.root, PathBuf::from(".memory"));
        match cli.command {
            Command::Welcome { init, no_animation } => {
                assert!(init);
                assert!(!no_animation);
            }
            _ => panic!("expected welcome command"),
        }
    }

    #[test]
    fn bare_command_routes_to_welcome() {
        let (root, json_output) =
            global_welcome_request(&[OsString::from("tree-ring")]).expect("welcome request");

        assert_eq!(root, PathBuf::from(".tree-ring"));
        assert!(!json_output);
    }

    #[test]
    fn global_flags_without_subcommand_route_to_welcome() {
        let (root, json_output) = global_welcome_request(&[
            OsString::from("tree-ring"),
            OsString::from("--json"),
            OsString::from("--root"),
            OsString::from(".memory"),
        ])
        .expect("welcome request");

        assert_eq!(root, PathBuf::from(".memory"));
        assert!(json_output);
    }

    #[test]
    fn subcommands_do_not_route_to_global_welcome() {
        assert!(
            global_welcome_request(&[OsString::from("tree-ring"), OsString::from("tui"),])
                .is_none()
        );
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
