#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionKind {
    Delete,
    Redact,
    ChangeRing { ring: String, event_type: String },
    Supersede { old_id: String, new_id: String },
    Placeholder { command: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn placeholder(command: &str) -> Self {
        Self {
            kind: ActionKind::Placeholder {
                command: command.to_string(),
            },
            memory_id: None,
            summary: format!("Run {command} maintenance flow"),
        }
    }

    pub fn confirmation_prompt(&self) -> String {
        format!("{} - press y to confirm, n/Esc to cancel", self.summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dangerous_actions_are_explicit_pending_values() {
        let pending = PendingAction::delete("mem_1".to_string(), "Bad memory".to_string());

        assert!(pending.confirmation_prompt().contains("press y"));
        assert_eq!(pending.memory_id.as_deref(), Some("mem_1"));
    }
}
