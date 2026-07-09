use std::path::{Component, Path, PathBuf};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tree_ring_memory_core::{now_iso, ConsolidationRequest, MemoryEvent};
use tree_ring_memory_sqlite::{MemoryRetriever, RecallResult, SQLiteMemoryStore};

use crate::integrations::{scan_integrations, IntegrationScanReport};
use crate::actions::export_import::{export_jsonl, ExportActionRequest};
use crate::actions::remember::{remember, RememberRequest};

use super::actions::{ActionKind, PendingAction};
use super::input::{parse_slash_command, SlashCommand};
use super::model::DashboardStats;
use super::store_watch::StoreWatcher;
use super::stream::{EventStreamReader, LiveEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Default,
    Exploded,
    Command,
    Search,
    Stream,
    Watch,
    Integrations,
}

pub struct App {
    root: PathBuf,
    pub store: SQLiteMemoryStore,
    watcher: StoreWatcher,
    pub dashboard: DashboardStats,
    pub memories: Vec<MemoryEvent>,
    pub results: Vec<RecallResult>,
    pub selected_result: usize,
    pub selected_ring: usize,
    pub mode: AppMode,
    pub command_buffer: String,
    pub search_query: String,
    pub include_sensitive: bool,
    pub include_superseded: bool,
    pub pending_action: Option<PendingAction>,
    pub status: String,
    pub live_events: Vec<LiveEvent>,
    pub integration_report: Option<IntegrationScanReport>,
    event_stream: Option<EventStreamReader>,
    pub tick: u64,
    pub should_quit: bool,
}

impl App {
    pub fn new(root: PathBuf, event_stream_path: Option<PathBuf>) -> Result<Self, String> {
        let db_path = root.join("memory.sqlite");
        let store = SQLiteMemoryStore::open(&db_path).map_err(|err| err.to_string())?;
        let mut app = Self {
            root,
            store,
            watcher: StoreWatcher::new(),
            dashboard: DashboardStats::empty(),
            memories: Vec::new(),
            results: Vec::new(),
            selected_result: 0,
            selected_ring: 0,
            mode: AppMode::Default,
            command_buffer: String::new(),
            search_query: String::new(),
            include_sensitive: false,
            include_superseded: false,
            pending_action: None,
            status: format!("Store {}", db_path.display()),
            live_events: Vec::new(),
            integration_report: None,
            event_stream: event_stream_path.map(EventStreamReader::new),
            tick: 0,
            should_quit: false,
        };
        app.refresh_store()?;
        Ok(app)
    }

    pub fn tick(&mut self) -> Result<(), String> {
        self.tick = self.tick.wrapping_add(1);
        self.dashboard.decay_pulses();
        self.refresh_store()?;
        self.read_stream_events()?;
        Ok(())
    }

    pub fn refresh_store(&mut self) -> Result<(), String> {
        let snapshot = self.watcher.refresh(&self.store)?;
        self.dashboard = snapshot.dashboard;
        self.memories = snapshot
            .memories
            .into_iter()
            .filter(|memory| self.include_superseded || memory.superseded_by.is_none())
            .filter(|memory| self.include_sensitive || memory.sensitivity == "normal")
            .collect();
        if !self.search_query.trim().is_empty() {
            self.run_search()?;
        }
        self.clamp_selection();
        Ok(())
    }

    pub fn read_stream_events(&mut self) -> Result<(), String> {
        let Some(stream) = &mut self.event_stream else {
            return Ok(());
        };
        for event in stream.read_new_events()? {
            if let Some(ring) = event.ring.as_deref() {
                self.dashboard.pulse_ring(ring, 1.0);
            }
            self.status = format!("event: {}", event.safe_label());
            self.live_events.push(event);
        }
        if self.live_events.len() > 8 {
            let trim = self.live_events.len() - 8;
            self.live_events.drain(0..trim);
        }
        Ok(())
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<(), String> {
        if self.pending_action.is_some() {
            return self.handle_pending_key(key);
        }
        match self.mode {
            AppMode::Command => self.handle_command_key(key),
            AppMode::Search => self.handle_search_key(key),
            _ => self.handle_navigation_key(key),
        }
    }

    fn handle_pending_key(&mut self, key: KeyEvent) -> Result<(), String> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => self.confirm_pending_action(),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.pending_action = None;
                self.status = "action cancelled".to_string();
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn handle_command_key(&mut self, key: KeyEvent) -> Result<(), String> {
        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Default;
                self.command_buffer.clear();
            }
            KeyCode::Enter => {
                let command = self.command_buffer.clone();
                self.command_buffer.clear();
                self.mode = AppMode::Default;
                self.execute_slash_command(&command)?;
            }
            KeyCode::Backspace => {
                self.command_buffer.pop();
            }
            KeyCode::Char(character) => self.command_buffer.push(character),
            _ => {}
        }
        Ok(())
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> Result<(), String> {
        match key.code {
            KeyCode::Esc => self.mode = AppMode::Default,
            KeyCode::Enter => self.run_search()?,
            KeyCode::Down | KeyCode::Char('j') => self.move_result(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_result(-1),
            KeyCode::Backspace => {
                self.search_query.pop();
                self.run_search()?;
            }
            KeyCode::Char(character) => {
                self.search_query.push(character);
                self.run_search()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_navigation_key(&mut self, key: KeyEvent) -> Result<(), String> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return Ok(());
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('/') => {
                self.mode = AppMode::Command;
                self.command_buffer.clear();
            }
            KeyCode::Char('s') => self.mode = AppMode::Search,
            KeyCode::Char('r') => self.mode = AppMode::Exploded,
            KeyCode::Char('i') => {
                self.include_sensitive = !self.include_sensitive;
                self.status = format!("include sensitive: {}", self.include_sensitive);
                self.run_search()?;
            }
            KeyCode::Char('u') => {
                self.include_superseded = !self.include_superseded;
                self.status = format!("include superseded: {}", self.include_superseded);
                self.refresh_store()?;
            }
            KeyCode::Esc => self.mode = AppMode::Default,
            KeyCode::Tab | KeyCode::Right => self.move_ring(1),
            KeyCode::BackTab | KeyCode::Left => self.move_ring(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_result(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_result(-1),
            _ => {}
        }
        Ok(())
    }

    pub fn execute_slash_command(&mut self, input: &str) -> Result<(), String> {
        match parse_slash_command(input) {
            SlashCommand::Rings => self.mode = AppMode::Exploded,
            SlashCommand::Search(query) => {
                self.mode = AppMode::Search;
                if !query.is_empty() {
                    self.search_query = query;
                    self.run_search()?;
                }
            }
            SlashCommand::Remember(summary) => self.remember_summary(summary)?,
            SlashCommand::Forget => self.pending_for_selected(PendingKind::Delete),
            SlashCommand::Redact => self.pending_for_selected(PendingKind::Redact),
            SlashCommand::Promote => self.pending_for_selected(PendingKind::Promote),
            SlashCommand::Scar => self.pending_for_selected(PendingKind::Scar),
            SlashCommand::Seed => self.pending_for_selected(PendingKind::Seed),
            SlashCommand::Supersede(old_id) => {
                if old_id.is_empty() {
                    self.status = "supersede requires an old memory id".to_string();
                } else if let Some(memory) = self.selected_memory() {
                    self.pending_action = Some(PendingAction::supersede(old_id, memory.id.clone()));
                } else {
                    self.status = "select a replacement memory first".to_string();
                }
            }
            SlashCommand::Consolidate => {
                let request = ConsolidationRequest::new("daily").map_err(|err| err.to_string())?;
                self.pending_action = Some(PendingAction::consolidate(request));
            }
            SlashCommand::Export(target) => self.pending_export(target),
            SlashCommand::Sync => self.pending_action = Some(PendingAction::sync_placeholder()),
            SlashCommand::Integrations => self.show_integrations(),
            SlashCommand::Stream => {
                self.mode = AppMode::Stream;
                self.status = "showing recent event-stream signals".to_string();
            }
            SlashCommand::Watch => {
                self.mode = AppMode::Watch;
                self.status = "store-watch polling is active".to_string();
            }
            SlashCommand::Unknown(command) => {
                self.status = if command.is_empty() {
                    "type a slash command".to_string()
                } else {
                    format!("unknown command: {command}")
                };
            }
        }
        Ok(())
    }

    fn remember_summary(&mut self, summary: String) -> Result<(), String> {
        if summary.trim().is_empty() {
            self.status = "remember requires a summary".to_string();
            return Ok(());
        }
        let report = remember(
            &mut self.store,
            RememberRequest {
                summary: summary.trim().to_string(),
                event_type: "lesson".to_string(),
                ring: "cambium".to_string(),
                scope: "project".to_string(),
                project: None,
                tags: Vec::new(),
            },
        )?;
        self.status = format!("remembered {}", report.memory.id);
        self.refresh_store()
    }

    fn pending_for_selected(&mut self, kind: PendingKind) {
        let Some(memory) = self.selected_memory() else {
            self.status = "select a memory first".to_string();
            return;
        };
        let summary = memory.summary.clone();
        let id = memory.id.clone();
        let event_type = memory.event_type.clone();
        self.pending_action = Some(match kind {
            PendingKind::Delete => PendingAction::delete(id, summary),
            PendingKind::Redact => PendingAction::redact(id, summary),
            PendingKind::Promote => {
                PendingAction::change_ring(id, summary, "heartwood", &event_type)
            }
            PendingKind::Scar => PendingAction::change_ring(id, summary, "scar", "warning"),
            PendingKind::Seed => PendingAction::change_ring(id, summary, "seed", "hypothesis"),
        });
    }

    fn confirm_pending_action(&mut self) -> Result<(), String> {
        let Some(pending) = self.pending_action.take() else {
            return Ok(());
        };
        match pending.kind {
            ActionKind::Delete => {
                if let Some(memory_id) = pending.memory_id {
                    self.store
                        .delete(&memory_id)
                        .map_err(|err| err.to_string())?;
                    self.status = format!("forgot {memory_id}");
                }
            }
            ActionKind::Redact => {
                if let Some(memory_id) = pending.memory_id {
                    self.store
                        .redact(&memory_id)
                        .map_err(|err| err.to_string())?;
                    self.status = format!("redacted {memory_id}");
                }
            }
            ActionKind::ChangeRing { ring, event_type } => {
                if let Some(memory_id) = pending.memory_id {
                    if let Some(mut memory) =
                        self.store.get(&memory_id).map_err(|err| err.to_string())?
                    {
                        memory.ring = ring.clone();
                        memory.event_type = event_type;
                        memory.updated_at = now_iso();
                        if ring == "heartwood" {
                            memory.retention = "durable".to_string();
                        }
                        self.store.put(&memory).map_err(|err| err.to_string())?;
                        self.status = format!("marked {memory_id} as {ring}");
                    }
                }
            }
            ActionKind::Supersede { old_id, new_id } => {
                self.store
                    .supersede(&old_id, &new_id)
                    .map_err(|err| err.to_string())?;
                self.status = format!("superseded {old_id} with {new_id}");
            }
            ActionKind::Consolidate { request } => {
                let report = self
                    .store
                    .consolidate(&request)
                    .map_err(|err| err.to_string())?;
                self.status = format!(
                    "consolidation {}: candidates={} outputs={}",
                    report.status,
                    report.candidate_count,
                    report.output_memory_ids.len()
                );
            }
            ActionKind::Export {
                output,
                include_sensitive,
                include_superseded,
            } => {
                if output.exists() {
                    self.status = format!("export refused existing file {}", output.display());
                } else {
                    let report = export_jsonl(
                        &self.store,
                        ExportActionRequest {
                            output: Some(output.clone()),
                            include_sensitive,
                            include_superseded,
                        },
                    )?;
                    self.status = format!(
                        "exported {} memories to {}",
                        report.report.memory_count,
                        output.display()
                    );
                }
            }
            ActionKind::Sync => {
                self.status = "sync adapters are available through CLI commands".to_string();
            }
        }
        self.refresh_store()
    }

    fn pending_export(&mut self, target: String) {
        if target.trim().is_empty() {
            self.status = "export requires an output file".to_string();
            return;
        }
        match resolve_export_path(&self.root, target.trim()) {
            Ok(output) => {
                if output.exists() {
                    self.status = format!("export refused existing file {}", output.display());
                    return;
                }
                self.pending_action = Some(PendingAction::export(
                    output,
                    self.include_sensitive,
                    self.include_superseded,
                ));
            }
            Err(error) => self.status = error,
        }
    }

    fn show_integrations(&mut self) {
        let root = project_root_for_memory_root(&self.root);
        let report = scan_integrations(&root);
        self.status = format!(
            "integration scan: {} detected under {}",
            report.detected_count,
            report.root.display()
        );
        self.integration_report = Some(report);
        self.mode = AppMode::Integrations;
    }

    pub fn run_search(&mut self) -> Result<(), String> {
        if self.search_query.trim().is_empty() {
            self.results.clear();
            self.selected_result = 0;
            return Ok(());
        }
        self.results = MemoryRetriever::new(&self.store)
            .recall(
                &self.search_query,
                None,
                None,
                None,
                None,
                None,
                self.include_sensitive,
                self.include_superseded,
                12,
                true,
            )
            .map_err(|err| err.to_string())?;
        for result in &self.results {
            self.dashboard.pulse_ring(&result.memory.ring, 0.72);
        }
        self.clamp_selection();
        Ok(())
    }

    pub fn selected_memory(&self) -> Option<&MemoryEvent> {
        if self.search_query.trim().is_empty() {
            self.memories.get(self.selected_result)
        } else {
            self.results
                .get(self.selected_result)
                .map(|result| &result.memory)
        }
    }

    fn move_result(&mut self, delta: isize) {
        let len = self.visible_memory_count();
        if len == 0 {
            self.selected_result = 0;
            return;
        }
        self.selected_result = wrap_index(self.selected_result, len, delta);
    }

    fn move_ring(&mut self, delta: isize) {
        let len = self.dashboard.rings.len();
        if len > 0 {
            self.selected_ring = wrap_index(self.selected_ring, len, delta);
        }
    }

    fn clamp_selection(&mut self) {
        let result_len = self.visible_memory_count();
        if result_len == 0 {
            self.selected_result = 0;
        } else if self.selected_result >= result_len {
            self.selected_result = result_len - 1;
        }
        if !self.dashboard.rings.is_empty() && self.selected_ring >= self.dashboard.rings.len() {
            self.selected_ring = self.dashboard.rings.len() - 1;
        }
    }

    fn visible_memory_count(&self) -> usize {
        if self.search_query.trim().is_empty() {
            self.memories.len()
        } else {
            self.results.len()
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum PendingKind {
    Delete,
    Redact,
    Promote,
    Scar,
    Seed,
}

fn wrap_index(current: usize, len: usize, delta: isize) -> usize {
    if delta.is_negative() {
        current.checked_sub(delta.unsigned_abs()).unwrap_or(len - 1)
    } else {
        (current + delta as usize) % len
    }
}

fn resolve_export_path(root: &Path, target: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(target);
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err("export paths cannot contain '..'".to_string());
    }
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(root.join("exports").join(path))
}

fn project_root_for_memory_root(root: &Path) -> PathBuf {
    if root.file_name().and_then(|name| name.to_str()) == Some(".tree-ring") {
        root.parent().unwrap_or(root).to_path_buf()
    } else {
        root.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    use tempfile::{tempdir, TempDir};

    use super::*;

    fn app(dir: &TempDir) -> App {
        App::new(dir.path().join(".tree-ring"), None).unwrap()
    }

    fn confirm(app: &mut App) {
        app.handle_key(KeyEvent::new(
            KeyCode::Char('y'),
            ratatui::crossterm::event::KeyModifiers::NONE,
        ))
        .unwrap();
    }

    #[test]
    fn slash_remember_stores_and_pulses_cambium() {
        let dir = tempdir().unwrap();
        let mut app = app(&dir);

        app.execute_slash_command("/remember User likes durable summaries")
            .unwrap();

        assert_eq!(app.dashboard.total, 1);
        assert_eq!(app.dashboard.ring("cambium").unwrap().total, 1);
    }

    #[test]
    fn slash_remember_uses_shared_action_and_keeps_status_shape() {
        let dir = tempdir().unwrap();
        let mut app = app(&dir);

        app.execute_slash_command("/remember Use shared TUI remember action")
            .unwrap();

        assert!(app.status.starts_with("remembered mem_"));
        let memories = app.store.list_all(false).unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].summary, "Use shared TUI remember action");
        assert_eq!(memories[0].ring, "cambium");
    }

    #[test]
    fn secret_like_remember_is_blocked() {
        let dir = tempdir().unwrap();
        let mut app = app(&dir);

        let err = app
            .execute_slash_command("/remember token sk-proj-abcdefghijklmnopqrstuvwxyz1234567890")
            .unwrap_err();

        assert!(err.contains("blocked"));
        assert_eq!(app.dashboard.total, 0);
    }

    #[test]
    fn dangerous_command_creates_pending_confirmation() {
        let dir = tempdir().unwrap();
        let mut app = app(&dir);
        app.execute_slash_command("/remember Avoid stale UI state")
            .unwrap();

        app.execute_slash_command("/forget").unwrap();

        assert!(app.pending_action.is_some());
        assert_eq!(app.dashboard.total, 1);
    }

    #[test]
    fn confirmation_executes_delete() {
        let dir = tempdir().unwrap();
        let mut app = app(&dir);
        app.execute_slash_command("/remember Avoid stale UI state")
            .unwrap();
        app.execute_slash_command("/forget").unwrap();

        confirm(&mut app);

        assert_eq!(app.dashboard.total, 0);
    }

    #[test]
    fn slash_rings_switches_exploded_mode() {
        let dir = tempdir().unwrap();
        let mut app = app(&dir);

        app.execute_slash_command("/rings").unwrap();

        assert_eq!(app.mode, AppMode::Exploded);
    }

    #[test]
    fn active_search_without_hits_does_not_fall_back_to_browse_selection() {
        let dir = tempdir().unwrap();
        let mut app = app(&dir);
        app.execute_slash_command("/remember Stored memory")
            .unwrap();

        app.execute_slash_command("/search absent needle").unwrap();

        assert!(app.results.is_empty());
        assert!(app.selected_memory().is_none());
    }

    #[test]
    fn slash_export_requires_target() {
        let dir = tempdir().unwrap();
        let mut app = app(&dir);

        app.execute_slash_command("/export").unwrap();

        assert!(app.pending_action.is_none());
        assert_eq!(app.status, "export requires an output file");
    }

    #[test]
    fn slash_export_can_be_cancelled_without_writing() {
        let dir = tempdir().unwrap();
        let mut app = app(&dir);
        app.execute_slash_command("/remember Exportable lesson")
            .unwrap();

        app.execute_slash_command("/export backup.jsonl").unwrap();
        app.handle_key(KeyEvent::new(
            KeyCode::Char('n'),
            ratatui::crossterm::event::KeyModifiers::NONE,
        ))
        .unwrap();

        assert!(!dir
            .path()
            .join(".tree-ring")
            .join("exports")
            .join("backup.jsonl")
            .exists());
        assert_eq!(app.status, "action cancelled");
    }

    #[test]
    fn confirmed_export_uses_shared_action_and_keeps_default_filters() {
        let dir = tempdir().unwrap();
        let mut app = app(&dir);
        app.execute_slash_command("/remember Export through shared TUI action")
            .unwrap();
        app.execute_slash_command("/export shared.jsonl").unwrap();
        confirm(&mut app);

        let output = dir.path().join(".tree-ring/exports/shared.jsonl");
        let jsonl = fs::read_to_string(output).unwrap();
        assert!(jsonl.contains("tree_ring_memory_export"));
        assert!(jsonl.contains("Export through shared TUI action"));
    }

    #[test]
    fn export_path_rejects_parent_dir_in_relative_and_absolute_targets() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");

        let relative_error = resolve_export_path(&root, "../outside.jsonl").unwrap_err();
        assert!(relative_error.contains(".."));

        let absolute_with_parent = std::env::current_dir()
            .unwrap()
            .join("exports")
            .join("..")
            .join("outside.jsonl");
        assert!(absolute_with_parent.is_absolute());
        let absolute_error =
            resolve_export_path(&root, &absolute_with_parent.to_string_lossy()).unwrap_err();
        assert!(absolute_error.contains(".."));
    }

    #[test]
    fn export_path_allows_absolute_target_without_parent_dir() {
        let dir = tempdir().unwrap();
        let root = dir.path().join(".tree-ring");
        let absolute = std::env::current_dir()
            .unwrap()
            .join("tree-ring-export.jsonl");

        let resolved = resolve_export_path(&root, &absolute.to_string_lossy()).unwrap();

        assert_eq!(resolved, absolute);
    }

    #[test]
    fn confirmed_export_writes_jsonl_with_default_privacy_filters() {
        let dir = tempdir().unwrap();
        let mut app = app(&dir);
        app.execute_slash_command("/remember Public lesson")
            .unwrap();
        let mut sensitive = MemoryEvent::new("Private detail", "lesson").unwrap();
        sensitive.sensitivity = "private".to_string();
        app.store.put(&sensitive).unwrap();

        app.execute_slash_command("/export backup.jsonl").unwrap();
        confirm(&mut app);

        let output = dir
            .path()
            .join(".tree-ring")
            .join("exports")
            .join("backup.jsonl");
        let jsonl = fs::read_to_string(output).unwrap();
        assert!(jsonl.contains("Public lesson"));
        assert!(!jsonl.contains("Private detail"));
        assert!(app.status.contains("exported 1 memories"));
    }

    #[test]
    fn confirmed_consolidation_creates_summary_then_idempotent_status() {
        let dir = tempdir().unwrap();
        let mut app = app(&dir);
        app.execute_slash_command("/remember First durable lesson")
            .unwrap();
        app.execute_slash_command("/remember Second durable lesson")
            .unwrap();

        app.execute_slash_command("/consolidate").unwrap();
        confirm(&mut app);
        let first_status = app.status.clone();
        assert!(first_status.contains("consolidation created"));
        assert!(app.dashboard.total > 2);

        app.execute_slash_command("/consolidate").unwrap();
        confirm(&mut app);

        assert!(app.status.contains("consolidation unchanged"));
    }

    #[test]
    fn slash_integrations_scans_project_root_markers() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("AGENTS.md"), "# Rules").unwrap();
        fs::create_dir_all(dir.path().join("revolve")).unwrap();
        let mut app = app(&dir);

        app.execute_slash_command("/integrations").unwrap();

        assert_eq!(app.mode, AppMode::Integrations);
        let report = app.integration_report.as_ref().unwrap();
        assert!(report.detected_count >= 2);
        assert!(app.status.contains("integration scan"));
    }
}
