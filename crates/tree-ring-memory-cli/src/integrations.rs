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
    pub markers: Vec<String>,
    pub next_step: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationStatus {
    Detected,
    Available,
}

pub fn scan_integrations(root: &Path) -> IntegrationScanReport {
    let home = env::var_os("HOME").map(PathBuf::from);
    let mut integrations = vec![
        detect(
            "dox",
            "DOX / AGENTS.md",
            root,
            home.as_deref(),
            &["AGENTS.md"],
            &[],
            "Run `tree-ring dox sync --source-root . --dry-run`, then sync when the preview looks right.",
        ),
        detect(
            "revolve",
            "Revolve",
            root,
            home.as_deref(),
            &["revolve", ".revolve"],
            &[],
            "Run `tree-ring revolve sync --source-root revolve --dry-run` to import promoted lessons, scars, and seeds.",
        ),
        detect(
            "codex",
            "Codex",
            root,
            home.as_deref(),
            &[".codex", "AGENTS.md"],
            &[".codex"],
            "Reference `.tree-ring/SKILL.md` and `.tree-ring/CLI.md` from project guidance.",
        ),
        detect(
            "claude-code",
            "Claude Code",
            root,
            home.as_deref(),
            &[".claude", "CLAUDE.md"],
            &[".claude"],
            "Reference `.tree-ring/SKILL.md` from `CLAUDE.md` or `.claude` project instructions.",
        ),
        detect(
            "agent-zero",
            "Agent Zero / A0",
            root,
            home.as_deref(),
            &["usr/plugins", "a0", "agent-zero", ".a0"],
            &[".a0"],
            "Use the generated skill/CLI guidance, or bridge via an Agent Zero plugin without modifying core code.",
        ),
        detect(
            "goose",
            "Goose",
            root,
            home.as_deref(),
            &[".goose", "goosehints"],
            &[".goose"],
            "Add Tree Ring Memory recall and remember commands to Goose project instructions.",
        ),
        detect(
            "opencode",
            "OpenCode",
            root,
            home.as_deref(),
            &[".opencode", "opencode.json", "opencode.toml"],
            &[".opencode"],
            "Reference the Tree Ring CLI from OpenCode project configuration or instructions.",
        ),
        detect(
            "hermes",
            "Hermes",
            root,
            home.as_deref(),
            &[".hermes", "hermes.toml"],
            &[".hermes"],
            "Reference Tree Ring Memory as a local CLI memory lifecycle layer.",
        ),
        detect(
            "pi",
            "Pi",
            root,
            home.as_deref(),
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
            markers.push(path.display().to_string());
        }
    }
    if let Some(home) = home {
        for marker in home_markers {
            let path = home.join(marker);
            if path.exists() {
                markers.push(path.display().to_string());
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
}
