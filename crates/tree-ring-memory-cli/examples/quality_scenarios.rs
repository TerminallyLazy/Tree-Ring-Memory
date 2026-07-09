use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tree_ring_memory_core::{
    evaluate_quality_scenario, parse_quality_scenario, summarize_quality_run, QualityRecall,
    QualityRunError, QualityRunReport, QualityScenario, QualityScenarioReport, TreeRingError,
};
use tree_ring_memory_sqlite::{MemoryRetriever, SQLiteMemoryStore};

const USAGE: &str = "usage: quality_scenarios <fixture-dir> <output-dir>";
const RECALL_LIMIT: usize = 8;
static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

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

    let run_report = run_quality_command(&fixture_dir, &output_dir)?;

    println!(
        "quality scenarios passed: {} scenario(s)",
        run_report.scenario_count
    );
    Ok(())
}

fn run_quality_command(fixture_dir: &Path, output_dir: &Path) -> Result<QualityRunReport, String> {
    let run_report = run_quality_scenarios(fixture_dir, output_dir)?;
    verify_persisted_quality_report(output_dir, &run_report)?;
    Ok(run_report)
}

fn verify_persisted_quality_report(
    output_dir: &Path,
    expected: &QualityRunReport,
) -> Result<(), String> {
    let json = fs::read_to_string(output_dir.join("quality-report.json"))
        .map_err(|_| "quality_report_read_error".to_string())?;
    let persisted = serde_json::from_str::<QualityRunReport>(&json)
        .map_err(|_| "quality_report_parse_error".to_string())?;
    if persisted != *expected {
        return Err("quality_report_mismatch_error".to_string());
    }
    if !persisted.ok || !persisted.quality_pass {
        return Err(format!(
            "quality run failed: {} scenarios evaluated",
            persisted.scenario_count
        ));
    }
    Ok(())
}

fn run_quality_scenarios(
    fixture_dir: &Path,
    output_dir: &Path,
) -> Result<QualityRunReport, String> {
    fs::create_dir_all(output_dir).map_err(|_| "output_directory_create_error".to_string())?;

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
            Err(_) => {
                return write_failed_report(
                    output_dir,
                    reports,
                    QualityRunError {
                        scenario: None,
                        path: Some(path.display().to_string()),
                        stage: "file_read".to_string(),
                        message: "fixture_file_read_error".to_string(),
                    },
                );
            }
        };
        let scenario = match parse_quality_scenario(&input) {
            Ok(scenario) => scenario,
            Err(err) => {
                return write_failed_report(
                    output_dir,
                    reports,
                    QualityRunError {
                        scenario: None,
                        path: Some(path.display().to_string()),
                        stage: "parse".to_string(),
                        message: classify_parse_error(&err).to_string(),
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
    let entries = fs::read_dir(fixture_dir).map_err(|_| QualityRunError {
        scenario: None,
        path: Some(fixture_dir.display().to_string()),
        stage: "fixture_directory".to_string(),
        message: "fixture_directory_read_error".to_string(),
    })?;
    let mut paths = entries
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|_| QualityRunError {
                    scenario: None,
                    path: Some(fixture_dir.display().to_string()),
                    stage: "fixture_directory".to_string(),
                    message: "fixture_directory_read_error".to_string(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.extension() == Some(OsStr::new("json")));
    paths.sort();
    Ok(paths)
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
    let mut store = SQLiteMemoryStore::open(&db_path).map_err(|_| ScenarioRunFailure {
        stage: "store_open",
        message: "sqlite_store_open_error".to_string(),
    })?;
    store
        .put_many(&scenario.seed_memories)
        .map_err(|_| ScenarioRunFailure {
            stage: "seed",
            message: "seed_memory_write_error".to_string(),
        })?;

    let prompt = scenario.prompt().ok_or_else(|| ScenarioRunFailure {
        stage: "evaluation",
        message: "scenario_prompt_missing".to_string(),
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
            false,
        )
        .map_err(|_| ScenarioRunFailure {
            stage: "recall",
            message: "recall_execution_error".to_string(),
        })?
        .into_iter()
        .map(|result| QualityRecall {
            memory: result.memory,
            score: result.score,
        })
        .collect::<Vec<_>>();

    evaluate_quality_scenario(scenario, &recalls).map_err(|err| ScenarioRunFailure {
        stage: "evaluation",
        message: classify_evaluation_error(&err).to_string(),
    })
}

fn write_reports(output_dir: &Path, report: &QualityRunReport) -> Result<(), String> {
    let json =
        serde_json::to_string_pretty(report).map_err(|_| "report_json_encode_error".to_string())?;
    fs::write(output_dir.join("quality-report.json"), json)
        .map_err(|_| "report_json_write_error".to_string())?;
    fs::write(
        output_dir.join("quality-summary.md"),
        markdown_summary(report),
    )
    .map_err(|_| "report_markdown_write_error".to_string())?;
    Ok(())
}

fn classify_parse_error(error: &TreeRingError) -> &'static str {
    match error {
        TreeRingError::Json(_) => "scenario_parse_error",
        TreeRingError::Validation(_) => "scenario_validation_error",
        _ => "scenario_parse_error",
    }
}

fn classify_evaluation_error(error: &TreeRingError) -> &'static str {
    match error {
        TreeRingError::Validation(_) => "scenario_validation_error",
        _ => "scenario_evaluation_error",
    }
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
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "temporary_root_clock_error".to_string())?
            .as_nanos();
        let unique = TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let safe_name = name
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
            .collect::<String>();
        let path = std::env::temp_dir().join(format!(
            "tree-ring-quality-{}-{}-{}-{safe_name}",
            std::process::id(),
            nanos,
            unique
        ));
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|_| "temporary_root_cleanup_error".to_string())?;
        }
        fs::create_dir_all(&path).map_err(|_| "temporary_root_create_error".to_string())?;
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

    const SECRET_MARKER: &str = "EDGE-SECRET-MARKER-7f1c2e";

    fn copy_valid_fixture(fixture_dir: &Path, name: &str) {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/quality/no-background-writer-constraint.json");
        fs::copy(source, fixture_dir.join(name)).unwrap();
    }

    fn write_invalid_memory_fixture(fixture_dir: &Path) {
        fs::write(
            fixture_dir.join("invalid-memory.json"),
            format!(
                r#"{{
                  "name": "invalid-memory-{SECRET_MARKER}",
                  "category": "constraint_recall",
                  "query": "quality proof",
                  "seed_memories": [
                    {{
                      "id": "mem_invalid_secret",
                      "created_at": "2026-07-09T00:00:00Z",
                      "updated_at": "2026-07-09T00:00:00Z",
                      "project": "tree-ring",
                      "agent_profile": null,
                      "scope": "project",
                      "ring": "{SECRET_MARKER}",
                      "event_type": "decision",
                      "summary": "Invalid memory",
                      "details": "",
                      "source": {{"type": "evidence", "ref": "docs/spec.md", "quote": ""}},
                      "tags": [],
                      "salience": 0.8,
                      "confidence": 0.8,
                      "sensitivity": "normal",
                      "retention": "durable",
                      "expires_at": null,
                      "supersedes": [],
                      "superseded_by": null,
                      "links": [],
                      "review": {{"needs_review": false, "review_reason": null, "reviewed_at": null, "reviewed_by": null}}
                    }}
                  ],
                  "expected_recall": [
                    {{"memory_id": "mem_invalid_secret"}}
                  ]
                }}"#
            ),
        )
        .unwrap();
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
        assert_eq!(report.errors[0].message, "scenario_parse_error");
        assert!(report.errors[0].scenario.is_none());
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
    fn sanitizes_validation_failures_before_writing_reports() {
        let fixtures = tempdir().unwrap();
        let output = tempdir().unwrap();
        copy_valid_fixture(fixtures.path(), "01-valid.json");
        write_invalid_memory_fixture(fixtures.path());

        let report = run_quality_scenarios(fixtures.path(), output.path()).unwrap();
        let json = fs::read_to_string(output.path().join("quality-report.json")).unwrap();
        let markdown = fs::read_to_string(output.path().join("quality-summary.md")).unwrap();

        assert!(!report.ok);
        assert!(!report.quality_pass);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].stage, "parse");
        assert_eq!(report.errors[0].message, "scenario_validation_error");
        assert!(report.errors[0].scenario.is_none());
        assert!(!report.errors[0].message.contains(SECRET_MARKER));
        assert!(!json.contains(SECRET_MARKER));
        assert!(!markdown.contains(SECRET_MARKER));
    }

    #[test]
    fn sanitizes_validation_failures_in_returned_command_error() {
        let fixtures = tempdir().unwrap();
        let output = tempdir().unwrap();
        write_invalid_memory_fixture(fixtures.path());

        let error = run_quality_command(fixtures.path(), output.path()).unwrap_err();

        assert_eq!(error, "quality run failed: 0 scenarios evaluated");
        assert!(!error.contains(SECRET_MARKER));
    }

    #[test]
    fn persisted_gate_rejects_nested_pass_when_top_level_fails() {
        let output = tempdir().unwrap();
        let scenario = QualityScenarioReport {
            name: "nested pass".to_string(),
            category: "constraint_recall".to_string(),
            constraint_recall_rate: Some(1.0),
            forbidden_recall_rate: None,
            spam_rejection_rate: None,
            evidence_required_rate: None,
            behavior_proof_pass: None,
            behavior_expectation: None,
            quality_pass: true,
            expected_recall: Vec::new(),
            forbidden_recall: Vec::new(),
            write_decisions: Vec::new(),
        };
        let mut report = summarize_quality_run(vec![scenario]);
        report.ok = false;
        report.quality_pass = false;
        write_reports(output.path(), &report).unwrap();

        let error = verify_persisted_quality_report(output.path(), &report).unwrap_err();

        assert_eq!(error, "quality run failed: 1 scenarios evaluated");
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
