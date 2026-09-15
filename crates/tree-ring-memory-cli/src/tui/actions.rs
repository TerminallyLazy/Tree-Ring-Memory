use std::{cell::Cell, path::PathBuf};

use tree_ring_memory_core::{ConsolidationRequest, DoxSyncReport};

#[derive(Debug, Clone, PartialEq)]
pub enum ActionKind {
    Delete,
    Redact,
    ChangeRing {
        ring: String,
        event_type: String,
    },
    Supersede {
        old_id: String,
        new_id: String,
    },
    Consolidate {
        request: ConsolidationRequest,
    },
    Export {
        output: PathBuf,
        include_sensitive: bool,
        include_superseded: bool,
    },
    SyncDox {
        preview: Box<DoxSyncReport>,
        selected_candidate: usize,
        preview_scroll: Cell<u16>,
    },
    RefreshCertification {
        command: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingAction {
    pub kind: ActionKind,
    pub memory_id: Option<String>,
    pub summary: String,
}

impl PendingAction {
    pub fn delete(memory_id: String, summary: String) -> Self {
        Self {
            kind: ActionKind::Delete,
            memory_id: Some(memory_id),
            summary: format!("Forget memory: {summary}"),
        }
    }

    pub fn redact(memory_id: String, summary: String) -> Self {
        Self {
            kind: ActionKind::Redact,
            memory_id: Some(memory_id),
            summary: format!("Redact memory: {summary}"),
        }
    }

    pub fn change_ring(memory_id: String, summary: String, ring: &str, event_type: &str) -> Self {
        Self {
            kind: ActionKind::ChangeRing {
                ring: ring.to_string(),
                event_type: event_type.to_string(),
            },
            memory_id: Some(memory_id),
            summary: format!("Mark as {ring}: {summary}"),
        }
    }

    pub fn supersede(old_id: String, new_id: String) -> Self {
        let summary = format!("Supersede {old_id} with selected memory {new_id}");
        Self {
            kind: ActionKind::Supersede {
                old_id: old_id.clone(),
                new_id: new_id.clone(),
            },
            memory_id: Some(new_id),
            summary,
        }
    }

    pub fn consolidate(request: ConsolidationRequest) -> Self {
        Self {
            summary: format!(
                "Run {} consolidation{}",
                request.period_type,
                if request.force { " with force" } else { "" }
            ),
            kind: ActionKind::Consolidate { request },
            memory_id: None,
        }
    }

    pub fn export(output: PathBuf, include_sensitive: bool, include_superseded: bool) -> Self {
        let mut warnings = Vec::new();
        if include_sensitive {
            warnings.push("including sensitive memory");
        }
        if include_superseded {
            warnings.push("including superseded memory");
        }
        let suffix = if warnings.is_empty() {
            String::new()
        } else {
            format!(" ({})", warnings.join(", "))
        };
        Self {
            summary: format!("Export memory to {}{suffix}", output.display()),
            kind: ActionKind::Export {
                output,
                include_sensitive,
                include_superseded,
            },
            memory_id: None,
        }
    }

    pub fn sync_dox(preview: DoxSyncReport, project: &str, store_path: &std::path::Path) -> Self {
        let sensitive_count = preview
            .events
            .iter()
            .filter(|event| event.sensitivity != "normal")
            .count();
        let summary = format!(
            "Import or update {} DOX summaries from {} AGENTS.md files.\nSource: {}\nStore: {}\nProject: {}\nIncludes {} sensitive; secret source files skipped: {}; {} warnings.\nStable IDs update existing summaries; source files stay authoritative.",
            preview.memory_count,
            preview.source_count,
            preview.root.display(),
            store_path.display(),
            project,
            sensitive_count,
            preview.skipped_secret_count,
            preview.warnings.len(),
        );
        Self {
            kind: ActionKind::SyncDox {
                preview: Box::new(preview),
                selected_candidate: 0,
                preview_scroll: Cell::new(0),
            },
            memory_id: None,
            summary,
        }
    }

    pub fn refresh_certification(command: &str) -> Self {
        Self {
            kind: ActionKind::RefreshCertification {
                command: command.to_string(),
            },
            memory_id: None,
            summary: "Refresh certification evidence".to_string(),
        }
    }

    #[cfg(test)]
    pub fn confirmation_prompt(&self) -> String {
        format!("{}\npress y to confirm, n/Esc to cancel", self.summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dox_confirmation_counts_secret_source_files_not_sections() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("AGENTS.md"),
            "# Rules\nReview source contracts.\n",
        )
        .unwrap();
        std::fs::create_dir(dir.path().join("nested")).unwrap();
        std::fs::write(
            dir.path().join("nested/AGENTS.md"),
            "# Earlier rules\nUse reviewed configuration.\n# Credentials\nUse key sk-proj-abcdefghijklmnopqrstuvwxyz1234567890\n# Later rules\nRead the source.\n",
        )
        .unwrap();
        let preview = tree_ring_memory_core::collect_dox_memories(
            &tree_ring_memory_core::DoxSyncRequest::new(dir.path()),
        )
        .unwrap();
        assert_eq!(preview.source_count, 2);
        assert_eq!(preview.skipped_secret_count, 1);
        assert!(preview
            .events
            .iter()
            .all(|event| !event.source.ref_.starts_with("nested/")));
        let pending = PendingAction::sync_dox(preview, "fixture", &dir.path().join(".tree-ring"));
        assert!(pending.summary.contains("secret source files skipped: 1"));
        assert!(!pending.summary.contains("secret sections"));
    }

    #[test]
    fn dangerous_actions_are_explicit_pending_values() {
        let pending = PendingAction::delete("mem_1".to_string(), "Bad memory".to_string());

        assert!(pending.confirmation_prompt().contains("press y"));
        assert_eq!(pending.memory_id.as_deref(), Some("mem_1"));
    }

    #[test]
    fn evidence_refresh_is_explicit_pending_value() {
        let pending = PendingAction::refresh_certification("sh scripts/certify-tree-ring.sh");

        assert!(pending.confirmation_prompt().contains("press y"));
        assert!(pending.summary.contains("Refresh certification evidence"));
        assert_eq!(
            pending.kind,
            ActionKind::RefreshCertification {
                command: "sh scripts/certify-tree-ring.sh".to_string()
            }
        );
    }
}
