use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::models::{sqlite_error, MemoryEvent, MemoryLink, TreeRingResult};
use crate::sensitivity::SensitivityGuard;

const DEFAULT_MAX_FILES: usize = 128;
const DEFAULT_MAX_SECTIONS_PER_FILE: usize = 8;
const MAX_SOURCE_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoxSyncRequest {
    pub root: PathBuf,
    pub project: Option<String>,
    pub max_files: usize,
    pub max_sections_per_file: usize,
}

impl DoxSyncRequest {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            project: None,
            max_files: DEFAULT_MAX_FILES,
            max_sections_per_file: DEFAULT_MAX_SECTIONS_PER_FILE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoxSyncReport {
    pub root: PathBuf,
    pub source_count: usize,
    pub memory_count: usize,
    pub skipped_secret_count: usize,
    pub warnings: Vec<String>,
    pub events: Vec<MemoryEvent>,
}

pub fn collect_dox_memories(request: &DoxSyncRequest) -> TreeRingResult<DoxSyncReport> {
    let files = discover_agents_files(&request.root, request.max_files)?;
    let mut events = Vec::new();
    let mut warnings = Vec::new();
    let mut skipped_secret_count = 0;

    for path in &files {
        match events_from_agents_file(request, path) {
            Ok(mut file_events) => events.append(&mut file_events),
            Err(AdapterSkip::Secret) => skipped_secret_count += 1,
            Err(AdapterSkip::Unreadable(message)) => warnings.push(message),
        }
    }

    let source_count = files.len();
    Ok(DoxSyncReport {
        root: request.root.clone(),
        source_count,
        memory_count: events.len(),
        skipped_secret_count,
        warnings,
        events,
    })
}

fn discover_agents_files(root: &Path, max_files: usize) -> TreeRingResult<Vec<PathBuf>> {
    let mut output = Vec::new();
    if !root.exists() {
        return Err(sqlite_error(format!(
            "DOX root does not exist: {}",
            root.display()
        )));
    }
    let metadata = fs::symlink_metadata(root).map_err(|err| sqlite_error(err.to_string()))?;
    if metadata.file_type().is_symlink() {
        return Err(sqlite_error(format!(
            "DOX root cannot be a symlink: {}",
            root.display()
        )));
    }
    if metadata.is_file() {
        if root.file_name().and_then(|name| name.to_str()) == Some("AGENTS.md") {
            output.push(root.to_path_buf());
        }
        return Ok(output);
    }
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
        let file_type = entry
            .file_type()
            .map_err(|err| sqlite_error(err.to_string()))?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            visit_directory(&path, max_files, output)?;
        } else if path.file_name().and_then(|name| name.to_str()) == Some("AGENTS.md") {
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

fn events_from_agents_file(
    request: &DoxSyncRequest,
    path: &Path,
) -> Result<Vec<MemoryEvent>, AdapterSkip> {
    let metadata =
        fs::symlink_metadata(path).map_err(|err| AdapterSkip::Unreadable(err.to_string()))?;
    if metadata.file_type().is_symlink() {
        return Err(AdapterSkip::Unreadable(format!(
            "Refusing symlinked AGENTS.md: {}",
            path.display()
        )));
    }
    if metadata.len() > MAX_SOURCE_BYTES {
        return Err(AdapterSkip::Unreadable(format!(
            "AGENTS.md exceeds {} bytes: {}",
            MAX_SOURCE_BYTES,
            path.display()
        )));
    }
    let content =
        fs::read_to_string(path).map_err(|err| AdapterSkip::Unreadable(err.to_string()))?;
    let relative = relative_display(&request.root, path);
    let sections = extract_sections(&content, request.max_sections_per_file);
    let mut events = Vec::new();
    let guard = SensitivityGuard::default();

    for section in sections {
        let snippet = summarize_lines(&section.lines);
        if snippet.is_empty() {
            continue;
        }
        let category = classify_section(&section.heading, &snippet);
        let durable = is_durable_rule(&section.heading, &snippet);
        let event_type = if durable {
            "dox_contract"
        } else {
            "dox_guidance"
        };
        let ring = if durable { "heartwood" } else { "outer" };
        let summary = format!("DOX {category} guidance in {relative}: {snippet}");
        let source_ref = format!(
            "{relative}#{}-{}",
            slugify(&section.heading),
            section.occurrence
        );
        let mut event = MemoryEvent::new(summary, event_type)
            .map_err(|err| AdapterSkip::Unreadable(err.to_string()))?;
        event.id = stable_id("dox", &source_ref);
        event.scope = "dox".to_string();
        event.ring = ring.to_string();
        event.project = request.project.clone();
        event.details = format!(
            "Source AGENTS.md remains authoritative. Re-read {relative} before acting. Heading: {}.",
            section.heading
        );
        event.source.source_type = "dox".to_string();
        event.source.ref_ = source_ref.clone();
        event.source.quote = snippet.clone();
        event.tags = vec![
            "dox".to_string(),
            "agents-md".to_string(),
            category.to_string(),
        ];
        event.salience = if durable { 0.84 } else { 0.62 };
        event.confidence = if durable { 0.88 } else { 0.72 };
        event.retention = if durable {
            "durable".to_string()
        } else {
            "normal".to_string()
        };
        event.links.push(MemoryLink {
            link_type: "dox".to_string(),
            target: relative.clone(),
        });
        match guard.detect_memory_event_sensitivity(&event) {
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
        events.push(event);
    }

    Ok(events)
}

#[derive(Debug, Clone)]
struct Section {
    heading: String,
    occurrence: usize,
    lines: Vec<String>,
}

fn extract_sections(content: &str, max_sections: usize) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut current = Section {
        heading: "overview".to_string(),
        occurrence: 1,
        lines: Vec::new(),
    };
    let mut in_fence = false;
    let mut occurrence = 1usize;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if trimmed.starts_with('#') {
            push_section(&mut sections, &mut current, max_sections);
            current.heading = trimmed.trim_start_matches('#').trim().to_string();
            occurrence += 1;
            current.occurrence = occurrence;
            current.lines.clear();
            continue;
        }
        if !trimmed.is_empty() {
            current.lines.push(trimmed.to_string());
        }
    }
    push_section(&mut sections, &mut current, max_sections);
    sections
}

fn push_section(sections: &mut Vec<Section>, current: &mut Section, max_sections: usize) {
    if sections.len() >= max_sections || current.lines.is_empty() {
        return;
    }
    sections.push(current.clone());
}

fn summarize_lines(lines: &[String]) -> String {
    let mut summary = lines
        .iter()
        .take(4)
        .map(|line| {
            line.trim()
                .trim_start_matches(['-', '*'])
                .trim()
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    summary = summary.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_chars(&summary, 180)
}

fn classify_section(heading: &str, snippet: &str) -> &'static str {
    let text = format!("{heading} {snippet}").to_ascii_lowercase();
    if text.contains("security") || text.contains("privacy") || text.contains("secret") {
        "safety"
    } else if text.contains("test") || text.contains("verify") || text.contains("check") {
        "verification"
    } else if text.contains("build") || text.contains("run") || text.contains("install") {
        "workflow"
    } else if text.contains("owner") || text.contains("authority") || text.contains("contract") {
        "authority"
    } else {
        "project"
    }
}

fn is_durable_rule(heading: &str, snippet: &str) -> bool {
    let text = format!("{heading} {snippet}").to_ascii_lowercase();
    [
        "must",
        "never",
        "required",
        "do not",
        "contract",
        "authority",
        "rule",
        "security",
        "privacy",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn slugify(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    slug.split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
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
    Secret,
    Unreadable(String),
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn discovers_nested_agents_files_and_preserves_source_refs() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("AGENTS.md"),
            "# Rules\n- You must run cargo test before release.",
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("crates/core")).unwrap();
        fs::write(
            dir.path().join("crates/core/AGENTS.md"),
            "# Verification\nUse focused tests before full cargo test.",
        )
        .unwrap();

        let mut request = DoxSyncRequest::new(dir.path());
        request.project = Some("tree-ring".to_string());
        let report = collect_dox_memories(&request).unwrap();

        assert_eq!(report.source_count, 2);
        assert!(report
            .events
            .iter()
            .any(|event| event.source.ref_ == "AGENTS.md#rules-2"));
        assert!(report
            .events
            .iter()
            .any(|event| event.source.ref_ == "crates/core/AGENTS.md#verification-2"));
        assert!(report.events.iter().all(|event| event.scope == "dox"));
    }

    #[test]
    fn summarizes_agents_without_full_doc_dump() {
        let dir = tempdir().unwrap();
        let long_line = "a ".repeat(400);
        fs::write(
            dir.path().join("AGENTS.md"),
            format!("# Guidance\n{long_line}\n\n## More\n{long_line}"),
        )
        .unwrap();

        let report = collect_dox_memories(&DoxSyncRequest::new(dir.path())).unwrap();

        assert!(!report.events.is_empty());
        assert!(report.events.iter().all(|event| event.summary.len() < 260));
        assert!(report
            .events
            .iter()
            .all(|event| event.details.contains("remains authoritative")));
    }

    #[test]
    fn durable_dox_rules_become_heartwood_contracts() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("AGENTS.md"),
            "# Contract\nYou must not store secrets in memory.",
        )
        .unwrap();

        let report = collect_dox_memories(&DoxSyncRequest::new(dir.path())).unwrap();
        let event = report.events.first().unwrap();

        assert_eq!(event.ring, "heartwood");
        assert_eq!(event.event_type, "dox_contract");
        assert_eq!(event.retention, "durable");
    }

    #[test]
    fn duplicate_headings_get_distinct_source_refs_and_ids() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("AGENTS.md"),
            "# Rules\nYou must run focused tests.\n# Rules\nNever store secrets.",
        )
        .unwrap();

        let report = collect_dox_memories(&DoxSyncRequest::new(dir.path())).unwrap();

        assert_eq!(report.events.len(), 2);
        assert_ne!(report.events[0].id, report.events[1].id);
        assert_ne!(report.events[0].source.ref_, report.events[1].source.ref_);
    }

    #[cfg(unix)]
    #[test]
    fn skips_symlinked_directories() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::write(
            outside.path().join("AGENTS.md"),
            "# Outside\nYou must not import this.",
        )
        .unwrap();
        symlink(outside.path(), dir.path().join("outside-link")).unwrap();

        let report = collect_dox_memories(&DoxSyncRequest::new(dir.path())).unwrap();

        assert_eq!(report.source_count, 0);
        assert!(report.events.is_empty());
    }

    #[test]
    fn skips_oversized_agents_file_before_reading() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("AGENTS.md"),
            vec![b'a'; MAX_SOURCE_BYTES as usize + 1],
        )
        .unwrap();

        let report = collect_dox_memories(&DoxSyncRequest::new(dir.path())).unwrap();

        assert_eq!(report.source_count, 1);
        assert_eq!(report.memory_count, 0);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("exceeds"));
    }
}
