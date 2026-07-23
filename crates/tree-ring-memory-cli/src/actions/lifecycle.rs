use std::path::Path;

use tree_ring_memory_core::{
    consolidate_memories, plan_maintenance, ConsolidationPeriod, ConsolidationReport,
    ConsolidationRequest, MaintenanceReport, MaintenanceRequest,
};
use tree_ring_memory_sqlite::SQLiteMemoryStore;

use super::ActionResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolidateActionRequest {
    pub period_type: String,
    pub period_key: Option<String>,
    pub project: Option<String>,
    pub agent_profile: Option<String>,
    pub workflow_id: Option<String>,
    pub session_id: Option<String>,
    pub dry_run: bool,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintainActionRequest {
    pub project: Option<String>,
    pub include_superseded: bool,
    pub apply_expired: bool,
    pub apply_secret_redactions: bool,
    pub repair_fts: bool,
}

pub fn consolidation_request(
    request: ConsolidateActionRequest,
) -> ActionResult<ConsolidationRequest> {
    Ok(ConsolidationRequest {
        period_type: ConsolidationPeriod::parse(&request.period_type)
            .map_err(|err| err.to_string())?,
        period_key: request.period_key,
        project: request.project,
        agent_profile: request.agent_profile,
        workflow_id: request.workflow_id,
        session_id: request.session_id,
        dry_run: request.dry_run,
        force: request.force,
    })
}

pub fn consolidate(
    store: &mut SQLiteMemoryStore,
    request: ConsolidateActionRequest,
) -> ActionResult<ConsolidationReport> {
    let request = consolidation_request(request)?;
    store.consolidate(&request).map_err(|err| err.to_string())
}

pub fn consolidate_dry_run_from_path(
    db_path: &Path,
    request: ConsolidateActionRequest,
) -> ActionResult<ConsolidationReport> {
    let request = consolidation_request(request)?;
    if db_path.exists() {
        let store = SQLiteMemoryStore::open_read_only(db_path).map_err(|err| err.to_string())?;
        let events = store.list_all(false).map_err(|err| err.to_string())?;
        consolidate_memories(&events, &request).map_err(|err| err.to_string())
    } else {
        consolidate_memories(&[], &request).map_err(|err| err.to_string())
    }
}

pub fn maintenance_request(request: MaintainActionRequest) -> MaintenanceRequest {
    MaintenanceRequest {
        dry_run: !(request.apply_expired || request.apply_secret_redactions || request.repair_fts),
        apply_expired: request.apply_expired,
        apply_secret_redactions: request.apply_secret_redactions,
        repair_fts: request.repair_fts,
        include_superseded: request.include_superseded,
        project: request.project,
    }
}

pub fn maintain(
    db_path: &Path,
    store: Option<&mut SQLiteMemoryStore>,
    request: MaintainActionRequest,
) -> ActionResult<MaintenanceReport> {
    let request = maintenance_request(request);
    if request.dry_run && !db_path.exists() {
        return Ok(plan_maintenance(&[], &request));
    }
    if let Some(store) = store {
        return store.maintain(&request).map_err(|err| err.to_string());
    }
    let mut store = SQLiteMemoryStore::open_read_only(db_path).map_err(|err| err.to_string())?;
    store.maintain(&request).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn maintenance_action_missing_store_is_non_mutating_dry_run() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join(".tree-ring/memory.sqlite");

        let report = maintain(
            &db_path,
            None,
            MaintainActionRequest {
                project: None,
                include_superseded: false,
                apply_expired: false,
                apply_secret_redactions: false,
                repair_fts: false,
            },
        )
        .unwrap();

        assert_eq!(report.memory_count, 0);
        assert!(!db_path.exists());
    }
}
