use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::models::{sqlite_error, MemoryEvent, MemoryLink, TreeRingResult};
use crate::sensitivity::SensitivityGuard;

const DEFAULT_MAX_FILES: usize = 256;
const MAX_SOURCE_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevolveSyncRequest {
    pub root: PathBuf,
    pub project: Option<String>,
    pub max_files: usize,
}

impl RevolveSyncRequest {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            project: None,
            max_files: DEFAULT_MAX_FILES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevolveSyncReport {
    pub root: PathBuf,
    pub source_count: usize,
    pub memory_count: usize,
    pub skipped_large_count: usize,
    pub skipped_secret_count: usize,
    pub warnings: Vec<String>,
    pub events: Vec<MemoryEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RevolveOutcome {
    Promoted,
    Rejected,
    Deferred,
    Observed,
}

pub fn collect_revolve_memories(request: &RevolveSyncRequest) -> TreeRingResult<RevolveSyncReport> {
    let files = discover_revolve_files(&request.root, request.max_files)?;
    let mut events = Vec::new();
    let mut warnings = Vec::new();
    let mut skipped_large_count = 0;
    let mut skipped_secret_count = 0;

    for path in &files {
        match event_from_revolve_file(request, path) {
            Ok(Some(event)) => events.push(event),
            Ok(None) => {}
            Err(AdapterSkip::Large) => skipped_large_count += 1,
            Err(AdapterSkip::Secret) => skipped_secret_count += 1,
            Err(AdapterSkip::Unreadable(message)) => warnings.push(message),
        }
    }

    let source_count = files.len();
    Ok(RevolveSyncReport {
        root: request.root.clone(),
        source_count,
        memory_count: events.len(),
        skipped_large_count,
        skipped_secret_count,
        warnings,
        events,
    })
}

fn discover_revolve_files(root: &Path, max_files: usize) -> TreeRingResult<Vec<PathBuf>> {
    if !root.exists() {
        return Err(sqlite_error(format!(
            "Revolve root does not exist: {}",
            root.display()
        )));
    }
    let metadata = fs::symlink_metadata(root).map_err(|err| sqlite_error(err.to_string()))?;
    if metadata.file_type().is_symlink() {
        return Err(sqlite_error(format!(
            "Revolve root cannot be a symlink: {}",
            root.display()
        )));
    }
    if metadata.is_file() {
        return Ok(if is_supported_source(root) {
            vec![root.to_path_buf()]
        } else {
            Vec::new()
        });
    }
    let mut output = Vec::new();
    visit_directory(root, max_files, &mut output)?;
    output.sort();
    Ok(output)
}

fn visit_directory(root: &Path, max_files: usize, output: &mut Vec<PathBuf>) -> TreeRingResult<()> {
    if output.len() >= max_files {
        return Ok(());
    }
    let mut entries = fs::read_dir(root)
        .map_err(|err| sqlite_error(err.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| sqlite_error(err.to_string()))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        if output.len() >= max_files {
            break;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|err| sqlite_error(err.to_string()))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            visit_directory(&path, max_files, output)?;
        } else if is_supported_source(&path) {
            output.push(path);
        }
    }
    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | "target" | "node_modules" | ".tree-ring")
    )
}

fn is_supported_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("md" | "txt" | "json" | "jsonl")
    )
}

fn event_from_revolve_file(
    request: &RevolveSyncRequest,
    path: &Path,
) -> Result<Option<MemoryEvent>, AdapterSkip> {
    let metadata = fs::metadata(path).map_err(|err| AdapterSkip::Unreadable(err.to_string()))?;
    if metadata.len() > MAX_SOURCE_BYTES {
        return Err(AdapterSkip::Large);
    }
    let content =
        fs::read_to_string(path).map_err(|err| AdapterSkip::Unreadable(err.to_string()))?;
    let relative = relative_display(&request.root, path);
    let haystack = format!("{} {}", relative, content).to_ascii_lowercase();
    let Some(outcome) = classify_outcome(&haystack) else {
        return Ok(None);
    };
    let summary = revolve_summary(outcome, &relative, &content);
    let (ring, event_type, salience, confidence, retention) = outcome_mapping(outcome);
    let source_ref = relative.clone();
    let mut event = MemoryEvent::new(summary, event_type)
        .map_err(|err| AdapterSkip::Unreadable(err.to_string()))?;
    event.id = stable_id("revolve", &format!("{source_ref}:{event_type}"));
    event.scope = "revolve".to_string();
    event.ring = ring.to_string();
    event.project = request.project.clone();
    event.details = format!(
        "Outcome: {}. Source Revolve record remains authoritative. Re-read {relative} before treating this memory as current truth.",
        outcome.as_str()
    );
    event.source.source_type = "revolve".to_string();
    event.source.ref_ = source_ref.clone();
    event.tags = vec![
        "revolve".to_string(),
        "evidence".to_string(),
        format!("outcome:{}", outcome.as_str()),
    ];
    event.salience = salience;
    event.confidence = confidence;
    event.retention = retention.to_string();
    event.links.push(MemoryLink {
        link_type: "revolve".to_string(),
        target: relative,
    });
    match SensitivityGuard::default().detect_memory_event_sensitivity(&event) {
        Ok(sensitivity) => {
            if sensitivity != "normal" {
                event.sensitivity = sensitivity;
            }
        }
        Err(_) => return Err(AdapterSkip::Secret),
    }
    event
        .validate()
        .map_err(|err| AdapterSkip::Unreadable(err.to_string()))?;
    Ok(Some(event))
}

fn classify_outcome(text: &str) -> Option<RevolveOutcome> {
    if contains_any(
        text,
        &[
            "rejected",
            "rejection",
            "rolled back",
            "rollback",
            "regression",
            "failed branch",
        ],
    ) {
        Some(RevolveOutcome::Rejected)
    } else if contains_any(text, &["promoted", "promotion", "accepted", "winner"]) {
        Some(RevolveOutcome::Promoted)
    } else if contains_any(
        text,
        &[
            "deferred",
            "future work",
            "hypothesis",
            "seed",
            "unresolved",
        ],
    ) {
        Some(RevolveOutcome::Deferred)
    } else if contains_any(
        text,
        &["evaluation", "result", "score", "checkpoint", "run"],
    ) {
        Some(RevolveOutcome::Observed)
    } else {
        None
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn outcome_mapping(
    outcome: RevolveOutcome,
) -> (&'static str, &'static str, f64, f64, &'static str) {
    match outcome {
        RevolveOutcome::Promoted => ("heartwood", "evaluation_promotion", 0.86, 0.84, "durable"),
        RevolveOutcome::Rejected => ("scar", "evaluation_rejection", 0.90, 0.78, "durable"),
        RevolveOutcome::Deferred => ("seed", "evaluation_hypothesis", 0.68, 0.60, "normal"),
        RevolveOutcome::Observed => ("outer", "evaluation_result", 0.72, 0.70, "normal"),
    }
}

fn revolve_summary(outcome: RevolveOutcome, relative: &str, content: &str) -> String {
    let excerpt = first_meaningful_line(content)
        .map(|line| truncate_chars(&line, 180))
        .unwrap_or_else(|| format!("Revolve {} evidence in {relative}", outcome.as_str()));
    format!(
        "Revolve {} evidence from {relative}: {excerpt}",
        outcome.as_str()
    )
}

fn first_meaningful_line(content: &str) -> Option<String> {
    content
        .lines()
        .map(|line| {
            line.trim()
                .trim_start_matches('#')
                .trim_start_matches(['-', '*'])
                .trim()
                .to_string()
        })
        .find(|line| !line.is_empty() && line.len() > 6)
}

impl RevolveOutcome {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Promoted => "promoted",
            Self::Rejected => "rejected",
            Self::Deferred => "deferred",
            Self::Observed => "observed",
        }
    }
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn stable_id(prefix: &str, value: &str) -> String {
    format!("mem_{prefix}_{:016x}", stable_hash(value.as_bytes()))
}

fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
}

#[derive(Debug, Clone)]
enum AdapterSkip {
    Large,
    Secret,
    Unreadable(String),
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn imports_promotions_as_heartwood_evidence() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(
            dir.path()
                .join("projects/ui/revisions/rev-001/checkpoints/win"),
        )
        .unwrap();
        fs::write(
            dir.path()
                .join("projects/ui/revisions/rev-001/checkpoints/win/AGENTS.md"),
            "# Promotion\nPromoted snapshot invalidation because it fixed stale unread state.",
        )
        .unwrap();

        let report = collect_revolve_memories(&RevolveSyncRequest::new(dir.path())).unwrap();
        let event = report.events.first().unwrap();

        assert_eq!(event.ring, "heartwood");
        assert_eq!(event.scope, "revolve");
        assert_eq!(event.event_type, "evaluation_promotion");
        assert_eq!(event.retention, "durable");
        assert!(event.source.ref_.contains("projects/ui"));
    }

    #[test]
    fn imports_rejections_as_scars() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("cache-experiment.md"),
            "# Rejected\nRejected aggressive caching after stale multi-chat regression.",
        )
        .unwrap();

        let report = collect_revolve_memories(&RevolveSyncRequest::new(dir.path())).unwrap();
        let event = report.events.first().unwrap();

        assert_eq!(event.ring, "scar");
        assert_eq!(event.event_type, "evaluation_rejection");
        assert!(event.tags.contains(&"outcome:rejected".to_string()));
    }

    #[test]
    fn imports_deferred_hypotheses_as_seeds() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("timeline-ui.md"),
            "# Deferred\nHypothesis: a visual ring timeline may improve explainability.",
        )
        .unwrap();

        let report = collect_revolve_memories(&RevolveSyncRequest::new(dir.path())).unwrap();
        let event = report.events.first().unwrap();

        assert_eq!(event.ring, "seed");
        assert_eq!(event.event_type, "evaluation_hypothesis");
    }

    #[test]
    fn ignores_sources_without_evidence_outcomes() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("notes.md"), "plain notes without outcome").unwrap();

        let report = collect_revolve_memories(&RevolveSyncRequest::new(dir.path())).unwrap();

        assert_eq!(report.memory_count, 0);
    }

    #[cfg(unix)]
    #[test]
    fn skips_symlinked_directories() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::write(
            outside.path().join("promotion.md"),
            "# Promotion\nPromoted external branch.",
        )
        .unwrap();
        symlink(outside.path(), dir.path().join("outside-link")).unwrap();

        let report = collect_revolve_memories(&RevolveSyncRequest::new(dir.path())).unwrap();

        assert_eq!(report.source_count, 0);
        assert!(report.events.is_empty());
    }
}
