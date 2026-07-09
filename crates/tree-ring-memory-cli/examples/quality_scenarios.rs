use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tree_ring_memory_core::{
    evaluate_quality_scenario, parse_quality_scenario, summarize_quality_run, QualityRecall,
    QualityRunReport, QualityScenario, QualityScenarioReport,
};
use tree_ring_memory_sqlite::{MemoryRetriever, SQLiteMemoryStore};

const USAGE: &str = "usage: quality_scenarios <fixture-dir> <output-dir>";
const RECALL_LIMIT: usize = 8;

fn main() {
    if let Err(err) = run() {
        eprintln!("quality scenarios failed: {err}");
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
    if args.next().is_some() {
        return Err(USAGE.to_string());
    }

    fs::create_dir_all(&output_dir)
        .map_err(|err| format!("create output dir {}: {err}", output_dir.display()))?;

    let scenarios = load_scenarios(&fixture_dir)?;
    let mut reports = Vec::with_capacity(scenarios.len());
    for scenario in &scenarios {
        reports.push(run_scenario(scenario)?);
    }

    let run_report = summarize_quality_run(reports);
    write_reports(&output_dir, &run_report)?;
    if !run_report.quality_pass {
        return Err(format!(
            "quality run failed: {} scenarios evaluated",
            run_report.scenario_count
        ));
    }

    println!(
        "quality scenarios passed: {} scenario(s)",
        run_report.scenario_count
    );
    Ok(())
}

fn load_scenarios(fixture_dir: &Path) -> Result<Vec<QualityScenario>, String> {
    let mut paths = fs::read_dir(fixture_dir)
        .map_err(|err| format!("read fixture dir {}: {err}", fixture_dir.display()))?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|err| err.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();

    let mut scenarios = Vec::new();
    for path in paths {
        if path.extension() != Some(OsStr::new("json")) {
            continue;
        }
        let input = fs::read_to_string(&path)
            .map_err(|err| format!("read fixture {}: {err}", path.display()))?;
        let scenario =
            parse_quality_scenario(&input).map_err(|err| format!("{}: {err}", path.display()))?;
        scenarios.push(scenario);
    }

    if scenarios.is_empty() {
        return Err(format!(
            "no quality scenario json files in {}",
            fixture_dir.display()
        ));
    }

    Ok(scenarios)
}

fn run_scenario(scenario: &QualityScenario) -> Result<QualityScenarioReport, String> {
    let root = TemporaryRoot::new(&scenario.name)?;
    let db_path = root.path().join("memory.sqlite");
    let mut store = SQLiteMemoryStore::open(&db_path)
        .map_err(|err| format!("open scenario store {}: {err}", scenario.name))?;
    store
        .put_many(&scenario.seed_memories)
        .map_err(|err| format!("seed scenario store {}: {err}", scenario.name))?;

    let prompt = scenario
        .prompt()
        .ok_or_else(|| format!("quality scenario {} has no prompt", scenario.name))?;
    let recalls = MemoryRetriever::new(&store)
        .recall(
            prompt,
            None,
            None,
            None,
            None,
            None,
            false,
            false,
            RECALL_LIMIT,
            true,
        )
        .map_err(|err| format!("recall scenario {}: {err}", scenario.name))?
        .into_iter()
        .map(|result| QualityRecall {
            memory: result.memory,
            score: result.score,
        })
        .collect::<Vec<_>>();

    evaluate_quality_scenario(scenario, &recalls)
        .map_err(|err| format!("evaluate scenario {}: {err}", scenario.name))
}

fn write_reports(output_dir: &Path, report: &QualityRunReport) -> Result<(), String> {
    let json = serde_json::to_string_pretty(report).map_err(|err| err.to_string())?;
    fs::write(output_dir.join("quality-report.json"), json)
        .map_err(|err| format!("write quality-report.json: {err}"))?;
    fs::write(
        output_dir.join("quality-summary.md"),
        markdown_summary(report),
    )
    .map_err(|err| format!("write quality-summary.md: {err}"))?;
    Ok(())
}

fn markdown_summary(report: &QualityRunReport) -> String {
    let mut lines = vec![
        "# Tree Ring Memory Quality Summary".to_string(),
        String::new(),
        format!("- quality pass: {}", report.quality_pass),
        format!("- scenarios: {}", report.scenario_count),
        format!(
            "- constraint recall rate: {:.3}",
            report.constraint_recall_rate
        ),
        format!(
            "- forbidden recall rate: {:.3}",
            report.forbidden_recall_rate
        ),
        format!("- spam rejection rate: {:.3}", report.spam_rejection_rate),
        format!(
            "- evidence required rate: {:.3}",
            report.evidence_required_rate
        ),
        format!(
            "- behavior proof pass rate: {:.3}",
            report.behavior_proof_pass_rate
        ),
        String::new(),
        "## Scenarios".to_string(),
        String::new(),
    ];

    for scenario in &report.scenarios {
        lines.push(format!(
            "- `{}` [{}]: pass={}",
            scenario.name, scenario.category, scenario.quality_pass
        ));
    }

    lines.push(String::new());
    lines.join("\n")
}

struct TemporaryRoot {
    path: PathBuf,
}

impl TemporaryRoot {
    fn new(name: &str) -> Result<Self, String> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_millis();
        let safe_name = name
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
            .collect::<String>();
        let path = std::env::temp_dir().join(format!(
            "tree-ring-quality-{}-{}-{safe_name}",
            std::process::id(),
            millis
        ));
        if path.exists() {
            fs::remove_dir_all(&path)
                .map_err(|err| format!("remove existing temp root {}: {err}", path.display()))?;
        }
        fs::create_dir_all(&path)
            .map_err(|err| format!("create temp root {}: {err}", path.display()))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryRoot {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
