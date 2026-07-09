use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use tree_ring_memory_core::{
    evaluate_quality_scenario, parse_quality_scenario, summarize_quality_run, QualityRecall,
    QualityRunError, QualityRunReport, QualityScenario, QualityScenarioReport,
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

    let run_report = run_quality_scenarios(&fixture_dir, &output_dir)?;
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

fn run_quality_scenarios(
    fixture_dir: &Path,
    output_dir: &Path,
) -> Result<QualityRunReport, String> {
    fs::create_dir_all(output_dir)
        .map_err(|err| format!("create output dir {}: {err}", output_dir.display()))?;

    let paths = match fixture_paths(fixture_dir) {
        Ok(paths) if !paths.is_empty() => paths,
        Ok(_) => {
            return write_failed_report(
                output_dir,
                Vec::new(),
                QualityRunError {
                    scenario: None,
                    path: Some(fixture_dir.display().to_string()),
                    stage: "fixture_directory".to_string(),
                    message: "no quality scenario JSON files found".to_string(),
                },
            );
        }
        Err(error) => return write_failed_report(output_dir, Vec::new(), error),
    };

    let mut reports = Vec::with_capacity(paths.len());
    for path in paths {
        let input = match fs::read_to_string(&path) {
            Ok(input) => input,
            Err(err) => {
                return write_failed_report(
                    output_dir,
                    reports,
                    QualityRunError {
                        scenario: None,
                        path: Some(path.display().to_string()),
                        stage: "file_read".to_string(),
                        message: err.to_string(),
                    },
                );
            }
        };
        let scenario_name = scenario_name_hint(&input);
        let scenario = match parse_quality_scenario(&input) {
            Ok(scenario) => scenario,
            Err(err) => {
                return write_failed_report(
                    output_dir,
                    reports,
                    QualityRunError {
                        scenario: scenario_name,
                        path: Some(path.display().to_string()),
                        stage: "parse".to_string(),
                        message: err.to_string(),
                    },
                );
            }
        };
        match run_scenario(&scenario) {
            Ok(report) => reports.push(report),
            Err(failure) => {
                return write_failed_report(
                    output_dir,
                    reports,
                    QualityRunError {
                        scenario: Some(scenario.name),
                        path: Some(path.display().to_string()),
                        stage: failure.stage.to_string(),
                        message: failure.message,
                    },
                );
            }
        }
    }

    let report = summarize_quality_run(reports);
    write_reports(output_dir, &report)?;
    Ok(report)
}

fn fixture_paths(fixture_dir: &Path) -> Result<Vec<PathBuf>, QualityRunError> {
    let entries = fs::read_dir(fixture_dir).map_err(|err| QualityRunError {
        scenario: None,
        path: Some(fixture_dir.display().to_string()),
        stage: "fixture_directory".to_string(),
        message: err.to_string(),
    })?;
    let mut paths = entries
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|err| QualityRunError {
                    scenario: None,
                    path: Some(fixture_dir.display().to_string()),
                    stage: "fixture_directory".to_string(),
                    message: err.to_string(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.extension() == Some(OsStr::new("json")));
    paths.sort();
    Ok(paths)
}

fn scenario_name_hint(input: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(input)
        .ok()?
        .get("name")?
        .as_str()
        .map(str::to_string)
}

fn write_failed_report(
    output_dir: &Path,
    reports: Vec<QualityScenarioReport>,
    error: QualityRunError,
) -> Result<QualityRunReport, String> {
    let mut report = summarize_quality_run(reports);
    report.ok = false;
    report.quality_pass = false;
    report.errors.push(error);
    write_reports(output_dir, &report)?;
    Ok(report)
}

struct ScenarioRunFailure {
    stage: &'static str,
    message: String,
}

fn run_scenario(scenario: &QualityScenario) -> Result<QualityScenarioReport, ScenarioRunFailure> {
    let root = TemporaryRoot::new(&scenario.name).map_err(|message| ScenarioRunFailure {
        stage: "store_open",
        message,
    })?;
    let db_path = root.path().join("memory.sqlite");
    let mut store = SQLiteMemoryStore::open(&db_path).map_err(|err| ScenarioRunFailure {
        stage: "store_open",
        message: err.to_string(),
    })?;
    store
        .put_many(&scenario.seed_memories)
        .map_err(|err| ScenarioRunFailure {
            stage: "seed",
            message: err.to_string(),
        })?;

    let prompt = scenario.prompt().ok_or_else(|| ScenarioRunFailure {
        stage: "evaluation",
        message: "validated scenario has no prompt".to_string(),
    })?;
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
        .map_err(|err| ScenarioRunFailure {
            stage: "recall",
            message: err.to_string(),
        })?
        .into_iter()
        .map(|result| QualityRecall {
            memory: result.memory,
            score: result.score,
        })
        .collect::<Vec<_>>();

    evaluate_quality_scenario(scenario, &recalls).map_err(|err| ScenarioRunFailure {
        stage: "evaluation",
        message: err.to_string(),
    })
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
            "- constraint recall rate: {}",
            format_rate(report.constraint_recall_rate)
        ),
        format!(
            "- forbidden recall rate: {}",
            format_rate(report.forbidden_recall_rate)
        ),
        format!(
            "- spam rejection rate: {}",
            format_rate(report.spam_rejection_rate)
        ),
        format!(
            "- evidence required rate: {}",
            format_rate(report.evidence_required_rate)
        ),
        format!(
            "- behavior proof pass rate: {}",
            format_rate(report.behavior_proof_pass_rate)
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

    if !report.errors.is_empty() {
        lines.extend([String::new(), "## Errors".to_string(), String::new()]);
        for error in &report.errors {
            let scenario = error.scenario.as_deref().unwrap_or("unknown");
            let path = error.path.as_deref().unwrap_or("unknown");
            lines.push(format!(
                "- stage={} scenario=`{scenario}` path=`{path}`: {}",
                error.stage, error.message
            ));
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

fn format_rate(rate: Option<f64>) -> String {
    rate.map_or_else(|| "n/a".to_string(), |rate| format!("{rate:.3}"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn copy_valid_fixture(fixture_dir: &Path, name: &str) {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/quality/no-background-writer-constraint.json");
        fs::copy(source, fixture_dir.join(name)).unwrap();
    }

    #[test]
    fn writes_partial_artifacts_when_a_later_fixture_fails_to_parse() {
        let fixtures = tempdir().unwrap();
        let output = tempdir().unwrap();
        copy_valid_fixture(fixtures.path(), "01-valid.json");
        fs::write(
            fixtures.path().join("02-private-invalid.json"),
            r#"{
              "name": "invalid-private-fixture",
              "category": "constraint_recall",
              "query": "private-memory-payload",
              "expected_recal": []
            }"#,
        )
        .unwrap();

        let report = run_quality_scenarios(fixtures.path(), output.path()).unwrap();

        assert!(!report.ok);
        assert!(!report.quality_pass);
        assert_eq!(report.scenario_count, 1);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].stage, "parse");
        assert_eq!(
            report.errors[0].scenario.as_deref(),
            Some("invalid-private-fixture")
        );
        assert!(report.errors[0]
            .path
            .as_deref()
            .is_some_and(|path| path.ends_with("02-private-invalid.json")));

        let json = fs::read_to_string(output.path().join("quality-report.json")).unwrap();
        let markdown = fs::read_to_string(output.path().join("quality-summary.md")).unwrap();
        assert!(!json.contains("private-memory-payload"));
        assert!(!markdown.contains("private-memory-payload"));
        assert!(markdown.contains("## Errors"));
        assert!(markdown.contains("stage=parse"));
    }

    #[test]
    fn writes_failed_artifacts_for_an_empty_fixture_directory() {
        let fixtures = tempdir().unwrap();
        let output = tempdir().unwrap();

        let report = run_quality_scenarios(fixtures.path(), output.path()).unwrap();

        assert!(!report.ok);
        assert!(!report.quality_pass);
        assert_eq!(report.scenario_count, 0);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].stage, "fixture_directory");
        assert!(output.path().join("quality-report.json").is_file());
        let markdown = fs::read_to_string(output.path().join("quality-summary.md")).unwrap();
        assert!(markdown.contains("constraint recall rate: n/a"));
        assert!(markdown.contains("## Errors"));
    }
}
