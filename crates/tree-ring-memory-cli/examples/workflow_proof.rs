use std::path::PathBuf;

use tree_ring_memory_cli::workflow_proof::{
    run_workflow_proof, CodexWorkflowAgent, WorkflowProofReport,
};

const USAGE: &str =
    "usage: workflow_proof <fixture-dir> <output-dir> [--codex-bin <path>] [--model <model>]";

fn main() {
    if let Err(error) = run() {
        eprintln!("workflow proof failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let fixture_dir = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| USAGE.to_string())?;
    let output_dir = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| USAGE.to_string())?;
    let mut codex_binary = PathBuf::from("codex");
    let mut codex_binary_supplied = false;
    let mut model = None;

    while let Some(argument) = args.next() {
        if argument == "--codex-bin" {
            if codex_binary_supplied {
                return Err(USAGE.to_string());
            }
            codex_binary = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| USAGE.to_string())?;
            codex_binary_supplied = true;
        } else if argument == "--model" {
            if model.is_some() {
                return Err(USAGE.to_string());
            }
            let value = args.next().ok_or_else(|| USAGE.to_string())?;
            model = Some(
                value
                    .into_string()
                    .map_err(|_| "model must be valid UTF-8".to_string())?,
            );
        } else {
            return Err(USAGE.to_string());
        }
    }

    let agent = CodexWorkflowAgent::new(codex_binary, model);
    let report = run_workflow_proof(&fixture_dir, &output_dir, &agent)?;
    print_summary(&report);
    if !report.tree_ring_complete {
        return Err(format!(
            "Tree Ring trials were incomplete after reports were written: {} failed or errored trial(s)",
            tree_ring_non_passes(&report)
        ));
    }
    Ok(())
}

fn print_summary(report: &WorkflowProofReport) {
    println!(
        "workflow proof evaluated: {} scenario(s), {} trial(s), observed Tree Ring wins over no-memory: {}, observed Tree Ring wins over raw-memory: {}",
        report.scenario_count,
        report.trial_count,
        report.tree_ring_wins_over_no_memory,
        report.tree_ring_wins_over_raw_memory
    );
}

fn tree_ring_non_passes(report: &WorkflowProofReport) -> usize {
    report
        .arm_summaries
        .iter()
        .find(|summary| summary.arm == tree_ring_memory_core::WorkflowArm::TreeRing)
        .map(|summary| summary.fail_count + summary.error_count)
        .unwrap_or(report.scenario_count)
}
