use clap::{Parser, Subcommand};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use tree_ring_memory_core::sensitivity::SensitivityGuard;
use tree_ring_memory_core::{
    AuditReport, ConsolidationReport, DoxSyncReport, MaintenanceReport, RevolveSyncReport,
};
use tree_ring_memory_core::{MemoryEvent, MemoryLink};
use tree_ring_memory_sqlite::{
    AuthorizationAuditEvent, PolicyGrant, PolicyStatus, SQLiteMemoryStore, WriteContext,
};

use actions::adapters::{sync_dox, sync_revolve, DoxSyncActionRequest, RevolveSyncActionRequest};
use actions::audit::{audit_store, AuditActionRequest};
use actions::export_import::{
    export_jsonl as export_action, import_jsonl as import_action, ExportActionRequest,
    ImportActionRequest,
};
use actions::integrations::{scan as integration_scan_action, IntegrationScanRequest};
use actions::lifecycle::{
    consolidate, consolidate_dry_run_from_path, maintain, ConsolidateActionRequest,
    MaintainActionRequest,
};
use actions::recall::{recall as recall_action, RecallRequest};
use actions::remember::{remember as remember_action, store_event_idempotently, RememberRequest};
use harness_evidence::{
    certify_harnesses, HarnessCertificationReport, HarnessCertificationRequest,
};
use recall_quality::{run_recall_quality, RecallQualityReport, RecallQualityRequest};
use serde_json::json;

mod actions;
mod agent_awareness;
mod commands;
mod evidence;
mod harness_evidence;
mod integrations;
mod recall_quality;
mod ring_mark;
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
        #[arg(
            long,
            env = "TREE_RING_AGENT_PROFILE",
            help = "agent role or worker identity that produced this memory"
        )]
        agent_profile: Option<String>,
        #[arg(
            long,
            env = "TREE_RING_WORKFLOW_ID",
            help = "shared workflow or fan-out/fan-in correlation id"
        )]
        workflow_id: Option<String>,
        #[arg(
            long,
            env = "TREE_RING_SESSION_ID",
            help = "session or execution-attempt correlation id"
        )]
        session_id: Option<String>,
        #[arg(
            long,
            help = "idempotency key for one logical write within its project/workflow/agent namespace"
        )]
        operation_id: Option<String>,
        #[arg(
            long,
            help = "source artifact, task, run, message, or result reference"
        )]
        source_ref: Option<String>,
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
        #[arg(
            long,
            env = "TREE_RING_AGENT_PROFILE",
            help = "agent role or worker identity that produced this evidence"
        )]
        agent_profile: Option<String>,
        #[arg(
            long,
            env = "TREE_RING_WORKFLOW_ID",
            help = "shared workflow or fan-out/fan-in correlation id"
        )]
        workflow_id: Option<String>,
        #[arg(
            long,
            env = "TREE_RING_SESSION_ID",
            help = "session or execution-attempt correlation id"
        )]
        session_id: Option<String>,
        #[arg(long, help = "idempotency key for one logical evidence write")]
        operation_id: Option<String>,
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
        #[arg(
            long,
            env = "TREE_RING_AGENT_PROFILE",
            help = "return only memories attributed to this agent profile"
        )]
        agent_profile: Option<String>,
        #[arg(
            long,
            env = "TREE_RING_WORKFLOW_ID",
            help = "return only memories from this workflow"
        )]
        workflow_id: Option<String>,
        #[arg(
            long,
            env = "TREE_RING_SESSION_ID",
            help = "return only memories from this session"
        )]
        session_id: Option<String>,
        #[arg(
            long,
            help = "return only this scope (global, project, agent, session, workflow, tool, eval, manual, dox, revolve)"
        )]
        scope: Option<String>,
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
        #[arg(
            long,
            env = "TREE_RING_AGENT_PROFILE",
            help = "optional agent-profile filter"
        )]
        agent_profile: Option<String>,
        #[arg(long, env = "TREE_RING_WORKFLOW_ID", help = "optional workflow filter")]
        workflow_id: Option<String>,
        #[arg(long, env = "TREE_RING_SESSION_ID", help = "optional session filter")]
        session_id: Option<String>,
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
    #[command(about = "manage coordinated multi-agent write authorization")]
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
    #[command(about = "open the Rust-native Tree Ring Memory terminal console")]
    Tui {
        #[arg(long, help = "optional JSONL event stream to light rings in real time")]
        event_stream: Option<PathBuf>,
        #[arg(
            long,
            env = "TREE_RING_AGENT_PROFILE",
            help = "agent role or worker identity for TUI writes"
        )]
        agent_profile: Option<String>,
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
    #[command(about = "write non-private recall quality evidence")]
    RecallQuality {
        #[arg(
            long,
            default_value = ".",
            help = "project root used for default evidence output"
        )]
        source_root: PathBuf,
        #[arg(
            long,
            help = "evidence output directory; defaults to <source-root>/target/tree-ring-certification"
        )]
        out_dir: Option<PathBuf>,
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
    #[command(about = "write non-mutating harness certification evidence")]
    Certify {
        #[arg(long, default_value = ".", help = "project root to certify")]
        source_root: PathBuf,
        #[arg(
            long,
            help = "evidence output directory; defaults to <source-root>/target/tree-ring-certification"
        )]
        out_dir: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum PolicyCommand {
    #[command(about = "enable coordinated mode and print a one-time coordinator capability")]
    Enable {
        #[arg(long, help = "human-readable coordinator label")]
        coordinator: Option<String>,
    },
    #[command(about = "show the current store policy")]
    Status,
    #[command(about = "rotate and print a one-time coordinator capability")]
    Rotate {
        #[arg(long, help = "human-readable coordinator label")]
        coordinator: Option<String>,
    },
    #[command(about = "return the store to open mode")]
    Disable,
    #[command(about = "show recent protected-write authorization decisions")]
    Audit {
        #[arg(long, default_value_t = 100)]
        limit: usize,
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
        let report = integration_scan_action(IntegrationScanRequest {
            source_root: source_root.clone(),
        });
        print_integration_report(&report.report, cli.json)?;
        return Ok(());
    }

    if let Command::Integrations {
        command:
            IntegrationCommand::Certify {
                source_root,
                out_dir,
            },
    } = &cli.command
    {
        let evidence_dir = out_dir
            .clone()
            .unwrap_or_else(|| evidence::certification_dir_for_project(source_root));
        let report = certify_harnesses(HarnessCertificationRequest {
            source_root: source_root.clone(),
            evidence_dir,
        })?;
        print_harness_certification_report(&report, cli.json)?;
        return Ok(());
    }

    if let Command::RecallQuality {
        source_root,
        out_dir,
    } = &cli.command
    {
        let evidence_dir = out_dir
            .clone()
            .unwrap_or_else(|| evidence::certification_dir_for_project(source_root));
        let report = run_recall_quality(RecallQualityRequest {
            source_root: source_root.clone(),
            evidence_dir,
        })?;
        print_recall_quality_report(&report, cli.json)?;
        return Ok(());
    }

    if let Command::Tui {
        event_stream,
        agent_profile,
        tick_ms,
    } = cli.command
    {
        if cli.json {
            return Err("--json is not supported with the interactive TUI".to_string());
        }
        let context = write_context(agent_profile.clone(), "tui")?;
        return tui::run(cli.root, event_stream, tick_ms, context, agent_profile);
    }

    if let Command::Policy { command } = &cli.command {
        match command {
            PolicyCommand::Status => {
                let store = open_policy_read_only(&db_path)?;
                let status = store.policy_status().map_err(|err| err.to_string())?;
                print_policy_status(&status, cli.json)?;
                return Ok(());
            }
            PolicyCommand::Audit { limit } => {
                if !(1..=1000).contains(limit) {
                    return Err("policy audit limit must be between 1 and 1000".to_string());
                }
                let store = open_policy_read_only(&db_path)?;
                let events = store.policy_audit(*limit).map_err(|err| err.to_string())?;
                print_policy_audit(&events, cli.json)?;
                return Ok(());
            }
            PolicyCommand::Enable { .. }
            | PolicyCommand::Rotate { .. }
            | PolicyCommand::Disable => {}
        }
    }

    if let Command::Import {
        path,
        dry_run: true,
        replace_existing,
    } = cli.command
    {
        let report = import_action(
            None,
            ImportActionRequest {
                path,
                dry_run: true,
                replace_existing,
            },
        )?;
        commands::scriptable::print_import_report(report, cli.json)?;
        return Ok(());
    }

    if let Command::Audit { audit_type } = &cli.command {
        let report = audit_store(
            &db_path,
            AuditActionRequest {
                audit_type: audit_type.clone(),
            },
        )?;
        print_audit_report(&report.report, cli.json)?;
        return Ok(());
    }

    if let Command::Consolidate {
        period_type,
        period_key,
        project,
        agent_profile,
        workflow_id,
        session_id,
        force,
        dry_run: true,
    } = &cli.command
    {
        let report = consolidate_dry_run_from_path(
            &db_path,
            ConsolidateActionRequest {
                period_type: period_type.clone(),
                period_key: period_key.clone(),
                project: project.clone(),
                agent_profile: agent_profile.clone(),
                workflow_id: workflow_id.clone(),
                session_id: session_id.clone(),
                dry_run: true,
                force: *force,
            },
        )?;
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
        let request = MaintainActionRequest {
            project: project.clone(),
            include_superseded: *include_superseded,
            apply_expired: *apply_expired,
            apply_secret_redactions: *apply_secret_redactions,
            repair_fts: *repair_fts,
        };
        if !(request.apply_expired || request.apply_secret_redactions || request.repair_fts) {
            let report = maintain(&db_path, None, request)?;
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
        let report = sync_dox(
            None,
            DoxSyncActionRequest {
                source_root: source_root.clone(),
                project: project.clone(),
                dry_run: true,
            },
        )?;
        print_dox_report(&report.report, cli.json, report.dry_run)?;
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
        let report = sync_revolve(
            None,
            RevolveSyncActionRequest {
                source_root: source_root.clone(),
                project: project.clone(),
                dry_run: true,
            },
        )?;
        print_revolve_report(&report.report, cli.json, report.dry_run)?;
        return Ok(());
    }

    let context = write_context(
        command_actor_profile(&cli.command),
        command_origin(&cli.command),
    )?;
    let mut store =
        SQLiteMemoryStore::open_with_context(&db_path, context).map_err(|err| err.to_string())?;

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
            agent_profile,
            workflow_id,
            session_id,
            operation_id,
            source_ref,
            tags,
        } => {
            let report = remember_action(
                &mut store,
                RememberRequest {
                    summary,
                    event_type,
                    ring,
                    scope,
                    project,
                    agent_profile,
                    workflow_id,
                    session_id,
                    operation_id,
                    source_ref,
                    tags,
                },
            )?;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string(&report.memory).map_err(|err| err.to_string())?
                );
            } else {
                println!("{}", report.memory.id);
            }
        }
        Command::Evidence {
            summary,
            outcome,
            evidence_ref,
            project,
            agent_profile,
            workflow_id,
            session_id,
            operation_id,
            details,
            score,
            tags,
        } => {
            let event = evidence_event(EvidenceEventRequest {
                summary,
                outcome,
                evidence_ref,
                project,
                agent_profile,
                workflow_id,
                session_id,
                operation_id,
                details,
                score,
                tags,
            })?;
            let (event, _created) = store_event_idempotently(&mut store, &event)?;
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
            agent_profile,
            workflow_id,
            session_id,
            scope,
            limit,
            include_sensitive,
        } => {
            let report = recall_action(
                &store,
                RecallRequest {
                    query,
                    project,
                    agent_profile,
                    workflow_id,
                    session_id,
                    scope,
                    limit,
                    include_sensitive,
                    include_superseded: false,
                    explain: false,
                },
            )?;
            commands::scriptable::print_recall_report(report, cli.json)?;
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
            let report = export_action(
                &store,
                ExportActionRequest {
                    output,
                    include_sensitive,
                    include_superseded,
                },
            )?;
            commands::scriptable::print_export_report(report, cli.json)?;
        }
        Command::Import {
            path,
            dry_run,
            replace_existing,
        } => {
            let report = import_action(
                Some(&mut store),
                ImportActionRequest {
                    path,
                    dry_run,
                    replace_existing,
                },
            )?;
            commands::scriptable::print_import_report(report, cli.json)?;
        }
        Command::Audit { .. } => unreachable!("audit returns before opening the writable store"),
        Command::Consolidate {
            period_type,
            period_key,
            project,
            agent_profile,
            workflow_id,
            session_id,
            dry_run,
            force,
        } => {
            let report = consolidate(
                &mut store,
                ConsolidateActionRequest {
                    period_type,
                    period_key,
                    project,
                    agent_profile,
                    workflow_id,
                    session_id,
                    dry_run,
                    force,
                },
            )?;
            print_consolidation_report(&report, cli.json)?;
        }
        Command::Maintain {
            project,
            include_superseded,
            apply_expired,
            apply_secret_redactions,
            repair_fts,
        } => {
            let report = maintain(
                &db_path,
                Some(&mut store),
                MaintainActionRequest {
                    project,
                    include_superseded,
                    apply_expired,
                    apply_secret_redactions,
                    repair_fts,
                },
            )?;
            print_maintenance_report(&report, cli.json)?;
        }
        Command::Policy { command } => match command {
            PolicyCommand::Enable { coordinator } => {
                let grant = store
                    .enable_coordinated_policy(coordinator.as_deref())
                    .map_err(|err| err.to_string())?;
                print_policy_grant(&grant, cli.json)?;
            }
            PolicyCommand::Status => unreachable!("policy status returns through read-only open"),
            PolicyCommand::Rotate { coordinator } => {
                let grant = store
                    .rotate_coordinator_capability(coordinator.as_deref())
                    .map_err(|err| err.to_string())?;
                print_policy_grant(&grant, cli.json)?;
            }
            PolicyCommand::Disable => {
                let status = store
                    .disable_coordinated_policy()
                    .map_err(|err| err.to_string())?;
                print_policy_status(&status, cli.json)?;
            }
            PolicyCommand::Audit { .. } => {
                unreachable!("policy audit returns through read-only open")
            }
        },
        Command::Tui { .. } => unreachable!("tui returns before opening the scriptable store"),
        Command::Welcome { .. } => {
            unreachable!("welcome returns before opening the scriptable store")
        }
        Command::RecallQuality { .. } => {
            unreachable!("recall-quality returns before opening the scriptable store")
        }
        Command::Integrations { .. } => {
            unreachable!("integrations commands return before opening the scriptable store")
        }
        Command::Dox {
            command:
                DoxCommand::Sync {
                    source_root,
                    project,
                    dry_run,
                },
        } => {
            let report = sync_dox(
                if dry_run { None } else { Some(&mut store) },
                DoxSyncActionRequest {
                    source_root,
                    project,
                    dry_run,
                },
            )?;
            print_dox_report(&report.report, cli.json, report.dry_run)?;
        }
        Command::Revolve {
            command:
                RevolveCommand::Sync {
                    source_root,
                    project,
                    dry_run,
                },
        } => {
            let report = sync_revolve(
                if dry_run { None } else { Some(&mut store) },
                RevolveSyncActionRequest {
                    source_root,
                    project,
                    dry_run,
                },
            )?;
            print_revolve_report(&report.report, cli.json, report.dry_run)?;
        }
    }
    Ok(())
}

const COORDINATOR_TOKEN_ENV: &str = "TREE_RING_COORDINATOR_TOKEN";

fn open_policy_read_only(db_path: &Path) -> Result<SQLiteMemoryStore, String> {
    if !db_path.is_file() {
        return Err(format!(
            "Tree Ring Memory store is not initialized at {}; policy status and audit never create or migrate a store",
            db_path.display()
        ));
    }
    SQLiteMemoryStore::open_read_only(db_path).map_err(|err| err.to_string())
}

fn write_context(
    actor_profile: Option<String>,
    origin: impl Into<String>,
) -> Result<WriteContext, String> {
    let capability = match std::env::var(COORDINATOR_TOKEN_ENV) {
        Ok(value) if value.trim().is_empty() => {
            return Err(format!("{COORDINATOR_TOKEN_ENV} cannot be blank"));
        }
        Ok(value) => Some(value),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(format!("{COORDINATOR_TOKEN_ENV} must be valid UTF-8"));
        }
    };
    WriteContext::new(actor_profile, capability.as_deref(), origin).map_err(|err| err.to_string())
}

fn command_actor_profile(command: &Command) -> Option<String> {
    match command {
        Command::Remember { agent_profile, .. }
        | Command::Evidence { agent_profile, .. }
        | Command::Recall { agent_profile, .. }
        | Command::Consolidate { agent_profile, .. } => agent_profile.clone(),
        _ => None,
    }
}

fn command_origin(command: &Command) -> &'static str {
    match command {
        Command::Init => "cli:init",
        Command::Remember { .. } => "cli:remember",
        Command::Evidence { .. } => "cli:evidence",
        Command::Recall { .. } => "cli:recall",
        Command::Forget { .. } => "cli:forget",
        Command::Export { .. } => "cli:export",
        Command::Import { .. } => "cli:import",
        Command::Audit { .. } => "cli:audit",
        Command::Consolidate { .. } => "cli:consolidate",
        Command::Maintain { .. } => "cli:maintain",
        Command::Policy { command } => match command {
            PolicyCommand::Enable { .. } => "cli:policy-enable",
            PolicyCommand::Status => "cli:policy-status",
            PolicyCommand::Rotate { .. } => "cli:policy-rotate",
            PolicyCommand::Disable => "cli:policy-disable",
            PolicyCommand::Audit { .. } => "cli:policy-audit",
        },
        Command::Tui { .. } => "tui",
        Command::Welcome { .. } => "cli:welcome",
        Command::RecallQuality { .. } => "cli:recall-quality",
        Command::Dox { .. } => "cli:dox",
        Command::Revolve { .. } => "cli:revolve",
        Command::Integrations { .. } => "cli:integrations",
    }
}

struct EvidenceEventRequest {
    summary: String,
    outcome: String,
    evidence_ref: String,
    project: Option<String>,
    agent_profile: Option<String>,
    workflow_id: Option<String>,
    session_id: Option<String>,
    operation_id: Option<String>,
    details: Option<String>,
    score: Option<f64>,
    tags: Vec<String>,
}

fn evidence_event(request: EvidenceEventRequest) -> Result<MemoryEvent, String> {
    let EvidenceEventRequest {
        summary,
        outcome,
        evidence_ref,
        project,
        agent_profile,
        workflow_id,
        session_id,
        operation_id,
        details,
        score,
        tags,
    } = request;

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
        .chain(agent_profile.iter())
        .chain(workflow_id.iter())
        .chain(session_id.iter())
        .chain(operation_id.iter())
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
    event.agent_profile = agent_profile;
    event.workflow_id = workflow_id;
    event.session_id = session_id;
    event.operation_id = operation_id;
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

fn print_policy_grant(grant: &PolicyGrant, json_output: bool) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string(grant).map_err(|err| err.to_string())?
        );
    } else {
        println!(
            "Tree Ring Memory policy: mode={:?} coordinator={}",
            grant.status.mode,
            escape_terminal_field(grant.status.coordinator_label.as_deref().unwrap_or("-"))
        );
        println!("{COORDINATOR_TOKEN_ENV}={}", grant.capability);
        println!("Store this capability securely; Tree Ring will not show it again.");
    }
    Ok(())
}

fn print_policy_status(status: &PolicyStatus, json_output: bool) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string(status).map_err(|err| err.to_string())?
        );
    } else {
        println!(
            "Tree Ring Memory policy: mode={:?} coordinator={} enabled_at={} updated_at={}",
            status.mode,
            escape_terminal_field(status.coordinator_label.as_deref().unwrap_or("-")),
            escape_terminal_field(status.enabled_at.as_deref().unwrap_or("-")),
            escape_terminal_field(&status.updated_at)
        );
    }
    Ok(())
}

fn print_policy_audit(events: &[AuthorizationAuditEvent], json_output: bool) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string(events).map_err(|err| err.to_string())?
        );
    } else if events.is_empty() {
        println!("Tree Ring Memory policy audit: no protected-write decisions recorded");
    } else {
        for event in events {
            println!("{}", format_policy_audit_event(event));
        }
    }
    Ok(())
}

fn format_policy_audit_event(event: &AuthorizationAuditEvent) -> String {
    format!(
        "{} {} action={} actor={} origin={} target={} reason={}",
        escape_terminal_field(&event.created_at),
        escape_terminal_field(&event.decision),
        escape_terminal_field(&event.action),
        escape_terminal_field(event.actor_profile.as_deref().unwrap_or("-")),
        escape_terminal_field(&event.origin),
        escape_terminal_field(event.target_memory_id.as_deref().unwrap_or("-")),
        escape_terminal_field(&event.reason),
    )
}

fn escape_terminal_field(value: &str) -> String {
    value.chars().flat_map(char::escape_default).collect()
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
                println!(
                    "  markers: {}",
                    integrations::format_markers(&integration.markers)
                );
            }
            println!("  next: {}", integration.next_step);
        }
    }
    Ok(())
}

fn print_harness_certification_report(
    report: &HarnessCertificationReport,
    json_output: bool,
) -> Result<(), String> {
    println!(
        "{}",
        format_harness_certification_report(report, json_output)
    );
    Ok(())
}

fn print_recall_quality_report(
    report: &RecallQualityReport,
    json_output: bool,
) -> Result<(), String> {
    println!("{}", format_recall_quality_report(report, json_output));
    Ok(())
}

fn format_harness_certification_report(
    report: &HarnessCertificationReport,
    json_output: bool,
) -> String {
    if json_output {
        json!({
            "ok": true,
            "report": report,
        })
        .to_string()
    } else {
        let mut lines = vec![format!(
            "Tree Ring Memory harness certification: pass={} fail={} skip={} evidence={}",
            report.pass_count,
            report.fail_count,
            report.skip_count,
            report.evidence_dir.display()
        )];
        for record in &report.records {
            lines.push(format!(
                "{} [{}] {}",
                record.name,
                record.status.as_str(),
                record.summary
            ));
            lines.push(format!("  next: {}", record.next_step));
        }
        lines.join("\n")
    }
}

fn format_recall_quality_report(report: &RecallQualityReport, json_output: bool) -> String {
    if json_output {
        json!({
            "ok": true,
            "report": report,
        })
        .to_string()
    } else {
        let mut lines = vec![format!(
            "Tree Ring Memory recall quality: status={} queries={} pass={} fail={} needs_review={} avg={:.3}ms max={:.3}ms evidence={}",
            report.status.as_str(),
            report.summary.query_count,
            report.summary.pass_count,
            report.summary.fail_count,
            report.summary.needs_review_count,
            report.summary.avg_latency_ms,
            report.summary.max_latency_ms,
            report.evidence_dir.display()
        )];
        for query in &report.queries {
            lines.push(format!(
                "{} [{:?}] latency={:.3}ms returned={}",
                query.query_id,
                query.status,
                query.latency_ms,
                query
                    .returned
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        lines.join("\n")
    }
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
    use std::fs;
    use tempfile::tempdir;
    use tree_ring_memory_core::{ConsolidationPeriod, ConsolidationRequest};
    use tree_ring_memory_sqlite::MemoryRetriever;

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
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                operation_id: None,
                source_ref: None,
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
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                operation_id: None,
                source_ref: None,
                tags: Vec::new(),
            },
        })
        .unwrap();
    }

    #[test]
    fn remember_and_recall_output_stays_stable_after_action_extraction() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        run(Cli::parse_from([
            "tree-ring",
            "--root",
            root.to_str().unwrap(),
            "remember",
            "Use action-backed CLI behavior.",
            "--event-type",
            "lesson",
            "--scope",
            "project",
            "--project",
            "tree-ring",
        ]))
        .unwrap();

        let store = SQLiteMemoryStore::open(root.join("memory.sqlite")).unwrap();
        let memories = store.list_all(false).unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].summary, "Use action-backed CLI behavior.");
    }

    #[test]
    fn import_dry_run_still_does_not_create_store_rows_after_action_extraction() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        let source_path = dir.path().join("source.sqlite");
        let mut source = SQLiteMemoryStore::open(&source_path).unwrap();
        source
            .put(&MemoryEvent::new("Dry-run import action parity.", "lesson").unwrap())
            .unwrap();
        let (jsonl, _) = source.export_jsonl(false, false).unwrap();
        let jsonl_path = dir.path().join("memories.jsonl");
        fs::write(&jsonl_path, jsonl).unwrap();

        run(Cli::parse_from([
            "tree-ring",
            "--root",
            root.to_str().unwrap(),
            "import",
            jsonl_path.to_str().unwrap(),
            "--dry-run",
        ]))
        .unwrap();

        assert!(!root.join("memory.sqlite").exists());
    }

    #[test]
    fn integrations_certify_writes_harness_evidence_without_memory_store() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".codex")).unwrap();
        fs::create_dir_all(dir.path().join(".tree-ring")).unwrap();
        fs::write(
            dir.path().join(".tree-ring/SKILL.md"),
            "Use `tree-ring recall` and `tree-ring remember`.",
        )
        .unwrap();
        fs::write(
            dir.path().join(".tree-ring/CLI.md"),
            "`tree-ring recall` and `tree-ring remember` are available.",
        )
        .unwrap();
        let root = dir.path().join(".tree-ring-memory");
        let out_dir = dir.path().join("proof");

        run(Cli::parse_from([
            "tree-ring",
            "--root",
            root.to_str().unwrap(),
            "--json",
            "integrations",
            "certify",
            "--source-root",
            dir.path().to_str().unwrap(),
            "--out-dir",
            out_dir.to_str().unwrap(),
        ]))
        .unwrap();

        assert!(!root.join("memory.sqlite").exists());
        assert!(out_dir.join("harness/codex.json").exists());
        let index = fs::read_to_string(out_dir.join("evidence-index.json")).unwrap();
        assert!(index.contains("\"codex\""));
        let parsed: serde_json::Value = serde_json::from_str(&index).unwrap();
        assert_eq!(parsed["harness"]["codex"]["status"], "pass");
    }

    #[test]
    fn integrations_certify_defaults_to_project_certification_dir() {
        let dir = tempdir().unwrap();

        run(Cli::parse_from([
            "tree-ring",
            "integrations",
            "certify",
            "--source-root",
            dir.path().to_str().unwrap(),
        ]))
        .unwrap();

        assert!(dir
            .path()
            .join("target/tree-ring-certification/harness/codex.json")
            .exists());
    }

    #[test]
    fn recall_quality_json_output_contract() {
        let report = RecallQualityReport {
            schema_version: 1,
            generated_at: "2026-07-09T00:00:00Z".to_string(),
            query_set_id: "default-fixture-v1".to_string(),
            status: crate::evidence::EvidenceStatus::Pass,
            source_root: PathBuf::from("/tmp/project"),
            evidence_dir: PathBuf::from("/tmp/project/target/tree-ring-certification"),
            record_path: PathBuf::from(
                "/tmp/project/target/tree-ring-certification/recall-quality/default-fixture-v1.json",
            ),
            summary: crate::recall_quality::RecallQualitySummary {
                query_count: 4,
                pass_count: 4,
                fail_count: 0,
                needs_review_count: 0,
                avg_latency_ms: 1.25,
                max_latency_ms: 2.5,
                fixture_memory_count: 5,
                sensitive_fixture_count: 1,
                private_payloads_used: false,
            },
            queries: vec![crate::recall_quality::RecallQualityQueryRecord {
                query_id: "scar-stale-cache".to_string(),
                query: "failure stale cache".to_string(),
                status: crate::recall_quality::RecallQualityQueryStatus::Pass,
                expected_top_id: Some("rq_scar_stale_cache".to_string()),
                expected_rank: Some(1),
                latency_ms: 1.25,
                returned: vec![crate::recall_quality::RecallQualityReturnedMemory {
                    id: "rq_scar_stale_cache".to_string(),
                    rank: 1,
                    ring: "scar".to_string(),
                    source_ref: "fixture://recall-quality/scar-stale-cache".to_string(),
                    score: 0.98,
                    ranking: std::collections::BTreeMap::from([
                        ("fts".to_string(), 0.75),
                        ("ring".to_string(), 0.23),
                    ]),
                }],
                notes: Vec::new(),
            }],
        };

        let output = format_recall_quality_report(&report, true);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["ok"], true);
        assert_eq!(parsed["report"]["query_set_id"], "default-fixture-v1");
        assert_eq!(parsed["report"]["status"], "pass");
        assert_eq!(parsed["report"]["summary"]["query_count"], 4);
        assert_eq!(parsed["report"]["summary"]["sensitive_fixture_count"], 1);
        assert_eq!(parsed["report"]["summary"]["private_payloads_used"], false);
        assert_eq!(
            parsed["report"]["queries"][0]["query_id"],
            "scar-stale-cache"
        );
    }

    #[test]
    fn recall_quality_human_output_contract() {
        let report = RecallQualityReport {
            schema_version: 1,
            generated_at: "2026-07-09T00:00:00Z".to_string(),
            query_set_id: "default-fixture-v1".to_string(),
            status: crate::evidence::EvidenceStatus::Pass,
            source_root: PathBuf::from("/tmp/project"),
            evidence_dir: PathBuf::from("/tmp/project/target/tree-ring-certification"),
            record_path: PathBuf::from(
                "/tmp/project/target/tree-ring-certification/recall-quality/default-fixture-v1.json",
            ),
            summary: crate::recall_quality::RecallQualitySummary {
                query_count: 4,
                pass_count: 4,
                fail_count: 0,
                needs_review_count: 0,
                avg_latency_ms: 1.25,
                max_latency_ms: 2.5,
                fixture_memory_count: 5,
                sensitive_fixture_count: 1,
                private_payloads_used: false,
            },
            queries: vec![
                crate::recall_quality::RecallQualityQueryRecord {
                    query_id: "scar-stale-cache".to_string(),
                    query: "failure stale cache".to_string(),
                    status: crate::recall_quality::RecallQualityQueryStatus::Pass,
                    expected_top_id: Some("rq_scar_stale_cache".to_string()),
                    expected_rank: Some(1),
                    latency_ms: 1.25,
                    returned: vec![crate::recall_quality::RecallQualityReturnedMemory {
                        id: "rq_scar_stale_cache".to_string(),
                        rank: 1,
                        ring: "scar".to_string(),
                        source_ref: "fixture://recall-quality/scar-stale-cache".to_string(),
                        score: 0.98,
                        ranking: std::collections::BTreeMap::new(),
                    }],
                    notes: Vec::new(),
                },
                crate::recall_quality::RecallQualityQueryRecord {
                    query_id: "sensitive-filter".to_string(),
                    query: "health private payload".to_string(),
                    status: crate::recall_quality::RecallQualityQueryStatus::Pass,
                    expected_top_id: None,
                    expected_rank: None,
                    latency_ms: 2.5,
                    returned: Vec::new(),
                    notes: Vec::new(),
                },
            ],
        };

        let output = format_recall_quality_report(&report, false);

        assert!(output.contains(
            "Tree Ring Memory recall quality: status=pass queries=4 pass=4 fail=0 needs_review=0 avg=1.250ms max=2.500ms evidence=/tmp/project/target/tree-ring-certification"
        ));
        assert!(
            output.contains("scar-stale-cache [Pass] latency=1.250ms returned=rq_scar_stale_cache")
        );
        assert!(output.contains("sensitive-filter [Pass] latency=2.500ms returned="));
    }

    #[test]
    fn harness_certification_json_output_contract() {
        let report = HarnessCertificationReport {
            generated_at: "2026-07-09T00:00:00Z".to_string(),
            source_root: PathBuf::from("/tmp/project"),
            evidence_dir: PathBuf::from("/tmp/project/target/tree-ring-certification"),
            index_path: PathBuf::from(
                "/tmp/project/target/tree-ring-certification/evidence-index.json",
            ),
            pass_count: 1,
            fail_count: 1,
            skip_count: 1,
            records: vec![
                crate::harness_evidence::HarnessProbeRecord {
                    schema_version: 1,
                    harness_id: "codex".to_string(),
                    name: "Codex".to_string(),
                    status: crate::evidence::EvidenceStatus::Pass,
                    generated_at: "2026-07-09T00:00:00Z".to_string(),
                    source_root: PathBuf::from("/tmp/project"),
                    command: "tree-ring integrations certify --source-root <source_root>"
                        .to_string(),
                    markers: vec![crate::harness_evidence::HarnessProbeMarker {
                        path: ".codex".to_string(),
                        origin: "project".to_string(),
                    }],
                    guidance: crate::harness_evidence::HarnessGuidanceEvidence {
                        agents_md: None,
                        skill_md: Some(PathBuf::from("/tmp/project/.tree-ring/SKILL.md")),
                        cli_md: Some(PathBuf::from("/tmp/project/.tree-ring/CLI.md")),
                        recall_guidance: true,
                        remember_guidance: true,
                    },
                    summary: "Codex has a project marker and generated Tree Ring recall/remember guidance.".to_string(),
                    next_step: "Merge the generated Tree Ring guidance into the active Codex instructions.".to_string(),
                },
                crate::harness_evidence::HarnessProbeRecord {
                    schema_version: 1,
                    harness_id: "goose".to_string(),
                    name: "Goose".to_string(),
                    status: crate::evidence::EvidenceStatus::Fail,
                    generated_at: "2026-07-09T00:00:00Z".to_string(),
                    source_root: PathBuf::from("/tmp/project"),
                    command: "tree-ring integrations certify --source-root <source_root>"
                        .to_string(),
                    markers: vec![],
                    guidance: crate::harness_evidence::HarnessGuidanceEvidence {
                        agents_md: None,
                        skill_md: None,
                        cli_md: None,
                        recall_guidance: false,
                        remember_guidance: false,
                    },
                    summary: "Goose has a project marker but is missing generated Tree Ring guidance.".to_string(),
                    next_step: "Run `tree-ring init`, then reference `.tree-ring/SKILL.md` and `.tree-ring/CLI.md` from the harness project instructions.".to_string(),
                },
                crate::harness_evidence::HarnessProbeRecord {
                    schema_version: 1,
                    harness_id: "pi".to_string(),
                    name: "PI".to_string(),
                    status: crate::evidence::EvidenceStatus::Skip,
                    generated_at: "2026-07-09T00:00:00Z".to_string(),
                    source_root: PathBuf::from("/tmp/project"),
                    command: "tree-ring integrations certify --source-root <source_root>"
                        .to_string(),
                    markers: vec![],
                    guidance: crate::harness_evidence::HarnessGuidanceEvidence {
                        agents_md: None,
                        skill_md: None,
                        cli_md: None,
                        recall_guidance: false,
                        remember_guidance: false,
                    },
                    summary: "PI was not detected for this project, so no compatibility claim is made.".to_string(),
                    next_step: "Add a project-level harness marker or project instruction file, then rerun `tree-ring integrations certify`.".to_string(),
                },
            ],
        };

        let output = format_harness_certification_report(&report, true);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["ok"], true);
        assert_eq!(parsed["report"]["pass_count"], 1);
        assert_eq!(parsed["report"]["fail_count"], 1);
        assert_eq!(parsed["report"]["skip_count"], 1);
        assert_eq!(parsed["report"]["records"][0]["harness_id"], "codex");
        assert_eq!(parsed["report"]["records"][0]["status"], "pass");
    }

    #[test]
    fn harness_certification_human_output_contract() {
        let evidence_dir = PathBuf::from("/tmp/project/target/tree-ring-certification");
        let report = HarnessCertificationReport {
            generated_at: "2026-07-09T00:00:00Z".to_string(),
            source_root: PathBuf::from("/tmp/project"),
            evidence_dir: evidence_dir.clone(),
            index_path: evidence_dir.join("evidence-index.json"),
            pass_count: 1,
            fail_count: 1,
            skip_count: 1,
            records: vec![
                crate::harness_evidence::HarnessProbeRecord {
                    schema_version: 1,
                    harness_id: "codex".to_string(),
                    name: "Codex".to_string(),
                    status: crate::evidence::EvidenceStatus::Pass,
                    generated_at: "2026-07-09T00:00:00Z".to_string(),
                    source_root: PathBuf::from("/tmp/project"),
                    command: "tree-ring integrations certify --source-root <source_root>"
                        .to_string(),
                    markers: vec![],
                    guidance: crate::harness_evidence::HarnessGuidanceEvidence {
                        agents_md: None,
                        skill_md: Some(PathBuf::from("/tmp/project/.tree-ring/SKILL.md")),
                        cli_md: Some(PathBuf::from("/tmp/project/.tree-ring/CLI.md")),
                        recall_guidance: true,
                        remember_guidance: true,
                    },
                    summary: "Codex has a project marker and generated Tree Ring recall/remember guidance.".to_string(),
                    next_step: "Merge the generated Tree Ring guidance into the active Codex instructions.".to_string(),
                },
                crate::harness_evidence::HarnessProbeRecord {
                    schema_version: 1,
                    harness_id: "goose".to_string(),
                    name: "Goose".to_string(),
                    status: crate::evidence::EvidenceStatus::Fail,
                    generated_at: "2026-07-09T00:00:00Z".to_string(),
                    source_root: PathBuf::from("/tmp/project"),
                    command: "tree-ring integrations certify --source-root <source_root>"
                        .to_string(),
                    markers: vec![],
                    guidance: crate::harness_evidence::HarnessGuidanceEvidence {
                        agents_md: None,
                        skill_md: None,
                        cli_md: None,
                        recall_guidance: false,
                        remember_guidance: false,
                    },
                    summary: "Goose has a project marker but is missing generated Tree Ring guidance.".to_string(),
                    next_step: "Run `tree-ring init`, then reference `.tree-ring/SKILL.md` and `.tree-ring/CLI.md` from the harness project instructions.".to_string(),
                },
                crate::harness_evidence::HarnessProbeRecord {
                    schema_version: 1,
                    harness_id: "pi".to_string(),
                    name: "PI".to_string(),
                    status: crate::evidence::EvidenceStatus::Skip,
                    generated_at: "2026-07-09T00:00:00Z".to_string(),
                    source_root: PathBuf::from("/tmp/project"),
                    command: "tree-ring integrations certify --source-root <source_root>"
                        .to_string(),
                    markers: vec![],
                    guidance: crate::harness_evidence::HarnessGuidanceEvidence {
                        agents_md: None,
                        skill_md: None,
                        cli_md: None,
                        recall_guidance: false,
                        remember_guidance: false,
                    },
                    summary: "PI was not detected for this project, so no compatibility claim is made.".to_string(),
                    next_step: "Add a project-level harness marker or project instruction file, then rerun `tree-ring integrations certify`.".to_string(),
                },
            ],
        };

        let output = format_harness_certification_report(&report, false);

        assert!(output.contains(
            "Tree Ring Memory harness certification: pass=1 fail=1 skip=1 evidence=/tmp/project/target/tree-ring-certification"
        ));
        assert!(output.contains(
            "Codex [pass] Codex has a project marker and generated Tree Ring recall/remember guidance."
        ));
        assert!(output.contains(
            "  next: Merge the generated Tree Ring guidance into the active Codex instructions."
        ));
        assert!(output.contains(
            "Goose [fail] Goose has a project marker but is missing generated Tree Ring guidance."
        ));
        assert!(output.contains(
            "  next: Run `tree-ring init`, then reference `.tree-ring/SKILL.md` and `.tree-ring/CLI.md` from the harness project instructions."
        ));
        assert!(output.contains(
            "PI [skip] PI was not detected for this project, so no compatibility claim is made."
        ));
        assert!(output.contains(
            "  next: Add a project-level harness marker or project instruction file, then rerun `tree-ring integrations certify`."
        ));
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
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                operation_id: None,
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
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                operation_id: None,
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
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                operation_id: None,
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
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                operation_id: None,
                source_ref: None,
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
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                operation_id: None,
                source_ref: None,
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
    fn audit_existing_store_does_not_mutate_database_contents() {
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
        let connection = rusqlite::Connection::open(&db_path).unwrap();
        connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        drop(connection);
        let _ = fs::remove_file(&wal_path);
        let _ = fs::remove_file(&shm_path);
        assert!(db_path.exists());
        assert!(!wal_path.exists());
        assert!(!shm_path.exists());
        let before = fs::read(&db_path).unwrap();

        run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Audit {
                audit_type: "all".to_string(),
            },
        })
        .unwrap();

        assert_eq!(fs::read(&db_path).unwrap(), before);
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
                agent_profile: None,
                workflow_id: None,
                session_id: None,
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
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                dry_run: false,
                force: false,
            },
        })
        .unwrap();
        let connection = rusqlite::Connection::open(root.join("memory.sqlite")).unwrap();
        let memories: i64 = connection
            .query_row("SELECT count(*) FROM memories", [], |row| row.get(0))
            .unwrap();
        let records: i64 = connection
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
                agent_profile: None,
                workflow_id: None,
                session_id: None,
                operation_id: None,
                source_ref: None,
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
                agent_profile: None,
                workflow_id: None,
                session_id: None,
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
                agent_profile: None,
                workflow_id: None,
                session_id: None,
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
    fn policy_status_and_audit_missing_root_do_not_create_a_store() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        let status_error = run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Policy {
                command: PolicyCommand::Status,
            },
        })
        .unwrap_err();
        assert!(status_error.contains("not initialized"));
        assert!(!root.exists());

        let audit_error = run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Policy {
                command: PolicyCommand::Audit { limit: 10 },
            },
        })
        .unwrap_err();
        assert!(audit_error.contains("not initialized"));
        assert!(!root.exists());
    }

    #[test]
    fn policy_status_and_audit_do_not_migrate_or_modify_a_v2_store() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        fs::create_dir_all(&root).unwrap();
        let db_path = root.join("memory.sqlite");
        let connection = rusqlite::Connection::open(&db_path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE sentinel (value TEXT NOT NULL);
                 INSERT INTO sentinel (value) VALUES ('unchanged');
                 PRAGMA user_version=2;",
            )
            .unwrap();
        drop(connection);
        let original_bytes = fs::read(&db_path).unwrap();

        let status_error = run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Policy {
                command: PolicyCommand::Status,
            },
        })
        .unwrap_err();
        assert!(status_error.contains("requires SQLite schema version 3"));
        assert!(status_error.contains("found version 2"));
        assert_eq!(fs::read(&db_path).unwrap(), original_bytes);

        let audit_error = run(Cli {
            root: root.clone(),
            json: true,
            command: Command::Policy {
                command: PolicyCommand::Audit { limit: 10 },
            },
        })
        .unwrap_err();
        assert!(audit_error.contains("requires SQLite schema version 3"));
        assert!(audit_error.contains("found version 2"));
        assert_eq!(fs::read(&db_path).unwrap(), original_bytes);

        let verification = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let version: i64 = verification
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        let sentinel: String = verification
            .query_row("SELECT value FROM sentinel", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 2);
        assert_eq!(sentinel, "unchanged");
        assert!(!db_path.with_extension("sqlite-wal").exists());
        assert!(!db_path.with_extension("sqlite-shm").exists());
    }

    #[test]
    fn policy_audit_human_output_escapes_terminal_controls() {
        let event = AuthorizationAuditEvent {
            id: 1,
            created_at: "2026-07-23\nspoofed".to_string(),
            action: "delete\u{1b}[31m".to_string(),
            decision: "denied".to_string(),
            reason: "missing\ncapability".to_string(),
            actor_profile: Some("worker\u{202e}spoof".to_string()),
            origin: "cli:forget".to_string(),
            target_memory_id: Some("bad\nid\u{1b}[31m".to_string()),
        };

        let rendered = format_policy_audit_event(&event);

        assert!(!rendered.contains('\n'));
        assert!(!rendered.contains('\u{1b}'));
        assert!(!rendered.contains('\u{202e}'));
        assert!(rendered.contains("\\n"));
        assert!(rendered.contains("\\u{1b}"));
        assert!(rendered.contains("\\u{202e}"));
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
            "--agent-profile",
            "reviewer",
            "--tick-ms",
            "125",
        ])
        .unwrap();

        match cli.command {
            Command::Tui {
                event_stream,
                agent_profile,
                tick_ms,
            } => {
                assert_eq!(event_stream.unwrap(), PathBuf::from("events.jsonl"));
                assert_eq!(agent_profile.as_deref(), Some("reviewer"));
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
                agent_profile: None,
                tick_ms: 250,
            },
        })
        .unwrap_err();

        assert!(err.contains("--json is not supported"));
    }
}
