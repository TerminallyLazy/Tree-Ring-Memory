use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntegrationScanReport {
    pub root: PathBuf,
    pub detected_count: usize,
    pub integrations: Vec<AgentIntegration>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentIntegration {
    pub id: &'static str,
    pub name: &'static str,
    pub status: IntegrationStatus,
    pub confidence: f64,
    pub markers: Vec<IntegrationMarker>,
    pub next_step: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct IntegrationMarker {
    pub path: String,
    pub origin: MarkerOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerOrigin {
    Home,
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationStatus {
    Detected,
    Available,
}

pub fn scan_integrations(root: &Path) -> IntegrationScanReport {
    let home = env::var_os("HOME").map(PathBuf::from);
    scan_integrations_with_home(root, home.as_deref())
}

pub fn format_markers(markers: &[IntegrationMarker]) -> String {
    markers
        .iter()
        .map(|marker| format!("{}:{}", marker.origin.as_str(), marker.path))
        .collect::<Vec<_>>()
        .join(", ")
}

fn scan_integrations_with_home(root: &Path, home: Option<&Path>) -> IntegrationScanReport {
    let mut integrations = vec![
        detect(
            "dox",
            "DOX / AGENTS.md",
            root,
            home,
            &["AGENTS.md"],
            &[],
            "Run `tree-ring dox sync --source-root . --dry-run`, then sync when the preview looks right.",
        ),
        detect(
            "revolve",
            "Revolve",
            root,
            home,
            &["revolve", ".revolve"],
            &[],
            "Run `tree-ring revolve sync --source-root revolve --dry-run` to import promoted lessons, scars, and seeds.",
        ),
        detect(
            "codex",
            "Codex",
            root,
            home,
            &[".codex", "AGENTS.md"],
            &[".codex"],
            "Reference `.tree-ring/SKILL.md` and `.tree-ring/CLI.md` from project guidance.",
        ),
        detect(
            "claude-code",
            "Claude Code",
            root,
            home,
            &[".claude", "CLAUDE.md"],
            &[".claude"],
            "Reference `.tree-ring/SKILL.md` from `CLAUDE.md` or `.claude` project instructions.",
        ),
        detect(
            "agent-zero",
            "Agent Zero / A0",
            root,
            home,
            &["usr/plugins", "a0", "agent-zero", ".a0"],
            &[".a0"],
            "Use the generated skill/CLI guidance, or bridge via an Agent Zero plugin without modifying core code.",
        ),
        detect(
            "goose",
            "Goose",
            root,
            home,
            &[".goose", "goosehints"],
            &[".goose"],
            "Add Tree Ring Memory recall and remember commands to Goose project instructions.",
        ),
        detect(
            "opencode",
            "OpenCode",
            root,
            home,
            &[".opencode", "opencode.json", "opencode.toml"],
            &[".opencode"],
            "Reference the Tree Ring CLI from OpenCode project configuration or instructions.",
        ),
        detect(
            "hermes",
            "Hermes",
            root,
            home,
            &[".hermes", "hermes.toml"],
            &[".hermes"],
            "Reference Tree Ring Memory as a local CLI memory lifecycle layer.",
        ),
        detect(
            "pi",
            "Pi",
            root,
            home,
            &[".pi", "pi.toml"],
            &[".pi"],
            "Use the generated portable skill and CLI reference as project instructions.",
        ),
    ];
    integrations.sort_by(|left, right| {
        right
            .confidence
            .partial_cmp(&left.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.name.cmp(right.name))
    });
    let detected_count = integrations
        .iter()
        .filter(|integration| integration.status == IntegrationStatus::Detected)
        .count();
    IntegrationScanReport {
        root: root.to_path_buf(),
        detected_count,
        integrations,
    }
}

impl MarkerOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Home => "home",
        }
    }
}

fn detect(
    id: &'static str,
    name: &'static str,
    root: &Path,
    home: Option<&Path>,
    project_markers: &[&str],
    home_markers: &[&str],
    next_step: &'static str,
) -> AgentIntegration {
    let mut markers = Vec::new();
    for marker in project_markers {
        let path = root.join(marker);
        if path.exists() {
            markers.push(IntegrationMarker {
                path: path.display().to_string(),
                origin: MarkerOrigin::Project,
            });
        }
    }
    if let Some(home) = home {
        for marker in home_markers {
            let path = home.join(marker);
            if path.exists() {
                markers.push(IntegrationMarker {
                    path: path.display().to_string(),
                    origin: MarkerOrigin::Home,
                });
            }
        }
    }
    markers.sort();
    markers.dedup();
    let confidence = if markers.is_empty() {
        0.0
    } else {
        (0.55 + markers.len() as f64 * 0.15).min(0.95)
    };
    AgentIntegration {
        id,
        name,
        status: if markers.is_empty() {
            IntegrationStatus::Available
        } else {
            IntegrationStatus::Detected
        },
        confidence,
        markers,
        next_step,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn detects_dox_revolve_and_claude_project_markers() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# Project rules").unwrap();
        fs::write(dir.path().join("CLAUDE.md"), "# Claude instructions").unwrap();
        fs::create_dir_all(dir.path().join("revolve")).unwrap();

        let report = scan_integrations(dir.path());

        assert!(report.detected_count >= 3);
        assert!(detected(&report, "dox"));
        assert!(detected(&report, "revolve"));
        assert!(detected(&report, "claude-code"));
    }

    #[test]
    fn marks_project_and_home_marker_origins() {
        let project = tempdir().unwrap();
        let home = tempdir().unwrap();
        fs::write(project.path().join("CLAUDE.md"), "# Claude instructions").unwrap();
        fs::create_dir_all(project.path().join(".codex")).unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();

        let report = scan_integrations_with_home(project.path(), Some(home.path()));

        let claude = integration(&report, "claude-code");
        assert!(claude
            .markers
            .iter()
            .any(|marker| marker.origin == MarkerOrigin::Project
                && marker.path.ends_with("CLAUDE.md")));
        assert!(claude
            .markers
            .iter()
            .any(|marker| marker.origin == MarkerOrigin::Home && marker.path.ends_with(".claude")));

        let codex = integration(&report, "codex");
        assert!(codex.markers.iter().any(
            |marker| marker.origin == MarkerOrigin::Project && marker.path.ends_with(".codex")
        ));
        assert!(!codex
            .markers
            .iter()
            .any(|marker| marker.origin == MarkerOrigin::Home));
    }

    #[test]
    fn unavailable_integrations_still_include_onboarding_next_steps() {
        let dir = tempdir().unwrap();

        let report = scan_integrations(dir.path());

        let opencode = report
            .integrations
            .iter()
            .find(|integration| integration.id == "opencode")
            .unwrap();
        assert_eq!(opencode.status, IntegrationStatus::Available);
        assert!(opencode.next_step.contains("Tree Ring"));
    }

    fn detected(report: &IntegrationScanReport, id: &str) -> bool {
        report.integrations.iter().any(|integration| {
            integration.id == id && integration.status == IntegrationStatus::Detected
        })
    }

    fn integration<'a>(report: &'a IntegrationScanReport, id: &str) -> &'a AgentIntegration {
        report
            .integrations
            .iter()
            .find(|integration| integration.id == id)
            .unwrap()
    }
}
