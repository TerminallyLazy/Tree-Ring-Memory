use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use serde::Deserialize;
use std::path::PathBuf;
use tree_ring_memory_core::sensitivity::SensitivityGuard;
use tree_ring_memory_core::{
    ConsolidationPeriod, ConsolidationRequest, MemoryEvent, MemoryLink, MemoryReview, MemorySource,
};
use tree_ring_memory_sqlite::{MemoryRetriever, SQLiteMemoryStore};

#[pyclass(name = "TreeRingMemoryNative", unsendable)]
pub struct PyTreeRingMemoryNative {
    root: PathBuf,
    store: SQLiteMemoryStore,
}

#[pymethods]
impl PyTreeRingMemoryNative {
    #[new]
    pub fn new(root: String) -> PyResult<Self> {
        Self::open(root)
    }

    #[staticmethod]
    pub fn open(root: String) -> PyResult<Self> {
        let root = PathBuf::from(root);
        let db_path = root.join("memory.sqlite");
        let store = SQLiteMemoryStore::open(&db_path).map_err(to_py_runtime_error)?;
        Ok(Self { root, store })
    }

    #[getter]
    pub fn root(&self) -> String {
        self.root.display().to_string()
    }

    #[pyo3(signature = (summary, event_type, ring=None, scope=None, project=None, tags=None))]
    pub fn remember_json(
        &mut self,
        summary: String,
        event_type: String,
        ring: Option<String>,
        scope: Option<String>,
        project: Option<String>,
        tags: Option<Vec<String>>,
    ) -> PyResult<String> {
        let detected_sensitivity = detect_memory_input_sensitivity(
            &summary,
            &event_type,
            ring.as_deref(),
            scope.as_deref(),
            project.as_deref(),
            tags.as_deref(),
        )?;
        let mut event = MemoryEvent::new(summary, event_type).map_err(to_py_value_error)?;
        if let Some(ring) = ring {
            event.ring = ring;
        }
        if let Some(scope) = scope {
            event.scope = scope;
        }
        event.project = project;
        event.tags = tags.unwrap_or_default();
        if detected_sensitivity != "normal" {
            event.sensitivity = detected_sensitivity;
        }
        self.put_event(event)
    }

    pub fn remember_event_json(&mut self, request_json: &str) -> PyResult<String> {
        let request: RememberRequest =
            serde_json::from_str(request_json).map_err(to_py_value_error)?;
        let mut event =
            MemoryEvent::new(request.summary, request.event_type).map_err(to_py_value_error)?;
        event.scope = request.scope.unwrap_or_else(|| "global".to_string());
        event.ring = request.ring.unwrap_or_else(|| "cambium".to_string());
        event.project = request.project;
        event.agent_profile = request.agent_profile;
        event.details = request.details.unwrap_or_default();
        event.source = request.source.unwrap_or_default();
        event.tags = request.tags.unwrap_or_default();
        if let Some(salience) = request.salience {
            event.salience = salience;
        }
        if let Some(confidence) = request.confidence {
            event.confidence = confidence;
        }
        if let Some(sensitivity) = request.sensitivity {
            event.sensitivity = sensitivity;
        }
        if let Some(retention) = request.retention {
            event.retention = retention;
        }
        event.expires_at = request.expires_at;
        event.supersedes = request.supersedes.unwrap_or_default();
        event.links = request.links.unwrap_or_default();
        event.review = request.review.unwrap_or_default();
        self.put_event(event)
    }

    pub fn put_event_json(&mut self, event_json: &str) -> PyResult<String> {
        let event: MemoryEvent = serde_json::from_str(event_json).map_err(to_py_value_error)?;
        self.put_event(event)
    }

    #[pyo3(signature = (query, project=None, limit=8, include_sensitive=false))]
    pub fn recall_json(
        &self,
        query: String,
        project: Option<String>,
        limit: usize,
        include_sensitive: bool,
    ) -> PyResult<String> {
        let results = MemoryRetriever::new(&self.store)
            .recall(
                &query,
                project.as_deref(),
                None,
                None,
                None,
                None,
                include_sensitive,
                false,
                limit,
                false,
            )
            .map_err(to_py_runtime_error)?;
        let payload: Vec<_> = results
            .into_iter()
            .map(|result| {
                serde_json::json!({
                    "memory": result.memory,
                    "score": result.score,
                    "ranking": result.ranking,
                })
            })
            .collect();
        serde_json::to_string(&payload).map_err(to_py_runtime_error)
    }

    pub fn recall_query_json(&self, request_json: &str) -> PyResult<String> {
        let request: RecallRequest =
            serde_json::from_str(request_json).map_err(to_py_value_error)?;
        let results = MemoryRetriever::new(&self.store)
            .recall(
                &request.query,
                request.project.as_deref(),
                request.agent_profile.as_deref(),
                request.scope.as_deref(),
                request.rings.as_deref(),
                request.event_types.as_deref(),
                request.include_sensitive.unwrap_or(false),
                request.include_superseded.unwrap_or(false),
                request.limit.unwrap_or(8),
                request.explain_ranking.unwrap_or(false),
            )
            .map_err(to_py_runtime_error)?;
        let payload: Vec<_> = results
            .into_iter()
            .map(|result| {
                serde_json::json!({
                    "memory": result.memory,
                    "score": result.score,
                    "ranking": result.ranking,
                })
            })
            .collect();
        serde_json::to_string(&payload).map_err(to_py_runtime_error)
    }

    pub fn forget(&mut self, memory_id: String, mode: String, reason: String) -> PyResult<()> {
        if reason.trim().is_empty() {
            return Err(PyValueError::new_err("forget reason is required"));
        }
        match mode.as_str() {
            "delete" => self.store.delete(&memory_id).map_err(to_py_runtime_error),
            "redact" => self.store.redact(&memory_id).map_err(to_py_runtime_error),
            other => Err(PyValueError::new_err(format!(
                "unsupported forget mode: {other}"
            ))),
        }
    }

    #[pyo3(signature = (include_sensitive=false, include_superseded=false))]
    pub fn export_jsonl(
        &self,
        include_sensitive: bool,
        include_superseded: bool,
    ) -> PyResult<String> {
        let (jsonl, _report) = self
            .store
            .export_jsonl(include_sensitive, include_superseded)
            .map_err(to_py_runtime_error)?;
        Ok(jsonl)
    }

    #[pyo3(signature = (data, dry_run=false, replace_existing=false))]
    pub fn import_jsonl(
        &mut self,
        data: &str,
        dry_run: bool,
        replace_existing: bool,
    ) -> PyResult<String> {
        let report = self
            .store
            .import_jsonl(data, dry_run, replace_existing)
            .map_err(to_py_runtime_error)?;
        serde_json::to_string(&serde_json::json!({
            "valid_count": report.valid_count,
            "inserted_count": report.inserted_count,
            "replaced_count": report.replaced_count,
            "skipped_duplicate_count": report.skipped_duplicate_count,
            "dry_run": report.dry_run,
        }))
        .map_err(to_py_runtime_error)
    }

    #[pyo3(signature = (audit_type="all"))]
    pub fn audit_json(&self, audit_type: &str) -> PyResult<String> {
        let report = self.store.audit(audit_type).map_err(to_py_runtime_error)?;
        serde_json::to_string(&report).map_err(to_py_runtime_error)
    }

    #[pyo3(signature = (period_type="daily", period_key=None, project=None, dry_run=false, force=false))]
    pub fn consolidate_json(
        &mut self,
        period_type: &str,
        period_key: Option<String>,
        project: Option<String>,
        dry_run: bool,
        force: bool,
    ) -> PyResult<String> {
        let request = ConsolidationRequest {
            period_type: ConsolidationPeriod::parse(period_type).map_err(to_py_value_error)?,
            period_key,
            project,
            dry_run,
            force,
        };
        let report = self
            .store
            .consolidate(&request)
            .map_err(to_py_runtime_error)?;
        serde_json::to_string(&report).map_err(to_py_runtime_error)
    }
}

impl PyTreeRingMemoryNative {
    fn put_event(&mut self, mut event: MemoryEvent) -> PyResult<String> {
        let detected_sensitivity = SensitivityGuard::default()
            .detect_memory_event_sensitivity(&event)
            .map_err(to_py_value_error)?;
        if event.sensitivity == "normal" && detected_sensitivity != "normal" {
            event.sensitivity = detected_sensitivity;
        }
        event.validate().map_err(to_py_value_error)?;
        self.store.put(&event).map_err(to_py_runtime_error)?;
        for superseded_id in &event.supersedes {
            self.store
                .supersede(superseded_id, &event.id)
                .map_err(to_py_runtime_error)?;
        }
        serde_json::to_string(&event).map_err(to_py_runtime_error)
    }
}

#[derive(Debug, Deserialize)]
struct RememberRequest {
    summary: String,
    event_type: String,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    ring: Option<String>,
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    agent_profile: Option<String>,
    #[serde(default)]
    details: Option<String>,
    #[serde(default)]
    source: Option<MemorySource>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    salience: Option<f64>,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    sensitivity: Option<String>,
    #[serde(default)]
    retention: Option<String>,
    #[serde(default)]
    expires_at: Option<String>,
    #[serde(default)]
    supersedes: Option<Vec<String>>,
    #[serde(default)]
    links: Option<Vec<MemoryLink>>,
    #[serde(default)]
    review: Option<MemoryReview>,
}

#[derive(Debug, Deserialize)]
struct RecallRequest {
    query: String,
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    agent_profile: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    rings: Option<Vec<String>>,
    #[serde(default)]
    event_types: Option<Vec<String>>,
    #[serde(default)]
    include_sensitive: Option<bool>,
    #[serde(default)]
    include_superseded: Option<bool>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    explain_ranking: Option<bool>,
}

#[pyfunction]
pub fn native_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[pymodule]
fn _tree_ring_memory_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyTreeRingMemoryNative>()?;
    m.add_function(wrap_pyfunction!(native_version, m)?)?;
    Ok(())
}

fn detect_memory_input_sensitivity(
    summary: &str,
    event_type: &str,
    ring: Option<&str>,
    scope: Option<&str>,
    project: Option<&str>,
    tags: Option<&[String]>,
) -> PyResult<String> {
    let guard = SensitivityGuard::default();
    let values = [summary, event_type]
        .into_iter()
        .chain(ring)
        .chain(scope)
        .chain(project)
        .chain(tags.into_iter().flatten().map(String::as_str));
    guard
        .detect_text_sensitivity(values)
        .map_err(to_py_value_error)
}

fn to_py_value_error(error: impl ToString) -> PyErr {
    PyValueError::new_err(error.to_string())
}

fn to_py_runtime_error(error: impl ToString) -> PyErr {
    PyRuntimeError::new_err(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn native_binding_store_round_trips_json() {
        pyo3::prepare_freethreaded_python();
        let dir = tempdir().unwrap();
        let mut memory =
            PyTreeRingMemoryNative::open(dir.path().join(".tree-ring").display().to_string())
                .unwrap();

        let event_json = memory
            .remember_json(
                "Native binding remembers through Rust.".to_string(),
                "lesson".to_string(),
                None,
                None,
                Some("bindings".to_string()),
                Some(vec!["native".to_string()]),
            )
            .unwrap();
        let event: MemoryEvent = serde_json::from_str(&event_json).unwrap();
        let recalled_json = memory
            .recall_json(
                "binding remembers".to_string(),
                Some("bindings".to_string()),
                8,
                false,
            )
            .unwrap();

        assert!(event.id.starts_with("mem_"));
        assert!(recalled_json.contains(&event.id));
    }

    #[test]
    fn native_binding_remember_event_json_preserves_full_facade_contract() {
        pyo3::prepare_freethreaded_python();
        let dir = tempdir().unwrap();
        let mut memory =
            PyTreeRingMemoryNative::open(dir.path().join(".tree-ring").display().to_string())
                .unwrap();
        let request = serde_json::json!({
            "summary": "Rust owns the full remember contract.",
            "details": "Details should be stored natively.",
            "event_type": "decision",
            "scope": "project",
            "ring": "heartwood",
            "project": "migration",
            "agent_profile": "default",
            "source": {"type": "file", "ref": "README.md", "quote": ""},
            "tags": ["rust", "native"],
            "salience": 0.8,
            "confidence": 0.9,
            "retention": "durable",
            "links": [{"type": "file", "target": "README.md"}],
            "review": {"needs_review": true, "review_reason": "native parity"}
        });

        let event_json = memory
            .remember_event_json(&serde_json::to_string(&request).unwrap())
            .unwrap();
        let event: MemoryEvent = serde_json::from_str(&event_json).unwrap();

        assert_eq!(event.summary, "Rust owns the full remember contract.");
        assert_eq!(event.details, "Details should be stored natively.");
        assert_eq!(event.scope, "project");
        assert_eq!(event.ring, "heartwood");
        assert_eq!(event.project.as_deref(), Some("migration"));
        assert_eq!(event.agent_profile.as_deref(), Some("default"));
        assert_eq!(event.source.ref_, "README.md");
        assert_eq!(event.tags, vec!["rust".to_string(), "native".to_string()]);
        assert_eq!(event.salience, 0.8);
        assert_eq!(event.confidence, 0.9);
        assert_eq!(event.retention, "durable");
        assert_eq!(event.links[0].target, "README.md");
        assert!(event.review.needs_review);
    }

    #[test]
    fn native_binding_remember_event_json_supersedes_prior_memory() {
        pyo3::prepare_freethreaded_python();
        let dir = tempdir().unwrap();
        let mut memory =
            PyTreeRingMemoryNative::open(dir.path().join(".tree-ring").display().to_string())
                .unwrap();
        let old_json = memory
            .remember_event_json(
                &serde_json::json!({
                    "summary": "Old migration decision.",
                    "event_type": "decision",
                    "project": "migration"
                })
                .to_string(),
            )
            .unwrap();
        let old: MemoryEvent = serde_json::from_str(&old_json).unwrap();
        let new_json = memory
            .remember_event_json(
                &serde_json::json!({
                    "summary": "New migration decision.",
                    "event_type": "decision",
                    "project": "migration",
                    "supersedes": [old.id]
                })
                .to_string(),
            )
            .unwrap();
        let new_event: MemoryEvent = serde_json::from_str(&new_json).unwrap();
        let old_after = memory.store.get(&old.id).unwrap().unwrap();

        assert_eq!(
            old_after.superseded_by.as_deref(),
            Some(new_event.id.as_str())
        );
        assert!(memory
            .recall_query_json(
                &serde_json::json!({
                    "query": "migration decision",
                    "project": "migration",
                    "include_superseded": false
                })
                .to_string()
            )
            .unwrap()
            .contains(&new_event.id));
    }

    #[test]
    fn native_binding_recall_query_json_applies_filters_and_ranking() {
        pyo3::prepare_freethreaded_python();
        let dir = tempdir().unwrap();
        let mut memory =
            PyTreeRingMemoryNative::open(dir.path().join(".tree-ring").display().to_string())
                .unwrap();
        let target_json = memory
            .remember_event_json(
                &serde_json::json!({
                    "summary": "Filtered native recall keeps durable Rust migration decisions.",
                    "event_type": "decision",
                    "scope": "project",
                    "ring": "heartwood",
                    "project": "migration",
                    "agent_profile": "default",
                    "tags": ["rust"]
                })
                .to_string(),
            )
            .unwrap();
        let target: MemoryEvent = serde_json::from_str(&target_json).unwrap();
        memory
            .remember_event_json(
                &serde_json::json!({
                    "summary": "Filtered native recall should skip other projects.",
                    "event_type": "lesson",
                    "scope": "project",
                    "ring": "outer",
                    "project": "other",
                    "agent_profile": "default",
                    "tags": ["rust"]
                })
                .to_string(),
            )
            .unwrap();

        let payload: serde_json::Value = serde_json::from_str(
            &memory
                .recall_query_json(
                    &serde_json::json!({
                        "query": "durable Rust migration decision",
                        "project": "migration",
                        "agent_profile": "default",
                        "scope": "project",
                        "rings": ["heartwood"],
                        "event_types": ["decision"],
                        "limit": 8,
                        "explain_ranking": true
                    })
                    .to_string(),
                )
                .unwrap(),
        )
        .unwrap();

        let results = payload.as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["memory"]["id"], target.id);
        assert!(results[0]["ranking"]["source_authority"].is_number());
    }

    #[test]
    fn native_binding_rejects_blank_forget_reason() {
        pyo3::prepare_freethreaded_python();
        let dir = tempdir().unwrap();
        let mut memory =
            PyTreeRingMemoryNative::open(dir.path().join(".tree-ring").display().to_string())
                .unwrap();

        let error = memory
            .forget(
                "mem_missing".to_string(),
                "delete".to_string(),
                " ".to_string(),
            )
            .unwrap_err();

        assert!(error.to_string().contains("forget reason is required"));
    }

    #[test]
    fn native_binding_blocks_secret_full_event_json() {
        pyo3::prepare_freethreaded_python();
        let dir = tempdir().unwrap();
        let mut memory =
            PyTreeRingMemoryNative::open(dir.path().join(".tree-ring").display().to_string())
                .unwrap();
        let mut event = MemoryEvent::new("Store via full JSON.", "lesson").unwrap();
        event.source.ref_ = "sk-proj-abcdefghijklmnopqrstuvwxyz1234567890".to_string();
        let payload = serde_json::to_string(&event).unwrap();

        let error = memory.put_event_json(&payload).unwrap_err();

        assert!(error.to_string().contains("secret-like memory is blocked"));
    }

    #[test]
    fn native_binding_blocks_secret_in_full_event_review_metadata() {
        pyo3::prepare_freethreaded_python();
        let dir = tempdir().unwrap();
        let mut memory =
            PyTreeRingMemoryNative::open(dir.path().join(".tree-ring").display().to_string())
                .unwrap();
        let mut event = MemoryEvent::new("Store via full JSON.", "lesson").unwrap();
        event.review.review_reason = Some("PASSWORD=do-not-store-this".to_string());
        let payload = serde_json::to_string(&event).unwrap();

        let error = memory.put_event_json(&payload).unwrap_err();

        assert!(error.to_string().contains("secret-like memory is blocked"));
    }

    #[test]
    fn native_binding_classifies_sensitive_full_event_metadata() {
        pyo3::prepare_freethreaded_python();
        let dir = tempdir().unwrap();
        let mut memory =
            PyTreeRingMemoryNative::open(dir.path().join(".tree-ring").display().to_string())
                .unwrap();
        let mut event = MemoryEvent::new("Store sensitive metadata.", "lesson").unwrap();
        event.details = "private diagnosis in details".to_string();
        let payload = serde_json::to_string(&event).unwrap();

        let stored_json = memory.put_event_json(&payload).unwrap();
        let stored: MemoryEvent = serde_json::from_str(&stored_json).unwrap();
        let hidden = memory
            .recall_json("sensitive metadata".to_string(), None, 8, false)
            .unwrap();
        let visible = memory
            .recall_json("sensitive metadata".to_string(), None, 8, true)
            .unwrap();

        assert_eq!(stored.sensitivity, "health");
        assert_eq!(hidden, "[]");
        assert!(visible.contains(&stored.id));
    }

    #[test]
    fn native_binding_exports_and_imports_jsonl() {
        pyo3::prepare_freethreaded_python();
        let source_dir = tempdir().unwrap();
        let target_dir = tempdir().unwrap();
        let mut source = PyTreeRingMemoryNative::open(
            source_dir.path().join(".tree-ring").display().to_string(),
        )
        .unwrap();
        let mut target = PyTreeRingMemoryNative::open(
            target_dir.path().join(".tree-ring").display().to_string(),
        )
        .unwrap();
        let event_json = source
            .remember_event_json(
                &serde_json::json!({
                    "summary": "Native JSONL import export preserves memory.",
                    "event_type": "lesson",
                    "project": "bindings"
                })
                .to_string(),
            )
            .unwrap();
        let event: MemoryEvent = serde_json::from_str(&event_json).unwrap();

        let jsonl = source.export_jsonl(false, false).unwrap();
        let dry_run_report: serde_json::Value =
            serde_json::from_str(&target.import_jsonl(&jsonl, true, false).unwrap()).unwrap();
        let import_report: serde_json::Value =
            serde_json::from_str(&target.import_jsonl(&jsonl, false, false).unwrap()).unwrap();

        assert!(jsonl.contains("tree_ring_memory_export"));
        assert!(jsonl.contains(&event.id));
        assert_eq!(dry_run_report["valid_count"], 1);
        assert_eq!(dry_run_report["inserted_count"], 0);
        assert_eq!(dry_run_report["dry_run"], true);
        assert_eq!(import_report["inserted_count"], 1);
        assert!(target
            .recall_json(
                "JSONL preserves memory".to_string(),
                Some("bindings".to_string()),
                8,
                false
            )
            .unwrap()
            .contains(&event.id));
    }

    #[test]
    fn native_binding_exposes_audit_json() {
        pyo3::prepare_freethreaded_python();
        let dir = tempdir().unwrap();
        let mut memory =
            PyTreeRingMemoryNative::open(dir.path().join(".tree-ring").display().to_string())
                .unwrap();
        let event_json = memory
            .remember_event_json(
                &serde_json::json!({
                    "summary": "Private durable memory should be audited.",
                    "event_type": "lesson",
                    "sensitivity": "health",
                    "retention": "durable"
                })
                .to_string(),
            )
            .unwrap();
        let event: MemoryEvent = serde_json::from_str(&event_json).unwrap();

        let report: serde_json::Value =
            serde_json::from_str(&memory.audit_json("sensitive").unwrap()).unwrap();

        assert_eq!(report["audit_type"], "sensitive");
        assert_eq!(report["memory_count"], 1);
        assert!(report["finding_count"].as_u64().unwrap() >= 1);
        assert_eq!(report["findings"][0]["memory_id"], event.id);
    }

    #[test]
    fn native_binding_exposes_consolidation_json() {
        pyo3::prepare_freethreaded_python();
        let dir = tempdir().unwrap();
        let mut memory =
            PyTreeRingMemoryNative::open(dir.path().join(".tree-ring").display().to_string())
                .unwrap();
        let event_json = memory
            .remember_event_json(
                &serde_json::json!({
                    "summary": "Consolidate native Python surface.",
                    "event_type": "decision",
                    "project": "bindings",
                    "salience": 0.8
                })
                .to_string(),
            )
            .unwrap();
        let event: MemoryEvent = serde_json::from_str(&event_json).unwrap();

        let dry_run: serde_json::Value = serde_json::from_str(
            &memory
                .consolidate_json(
                    "manual",
                    Some("manual-native-test".to_string()),
                    Some("bindings".to_string()),
                    true,
                    false,
                )
                .unwrap(),
        )
        .unwrap();
        let created: serde_json::Value = serde_json::from_str(
            &memory
                .consolidate_json(
                    "manual",
                    Some("manual-native-test".to_string()),
                    Some("bindings".to_string()),
                    false,
                    false,
                )
                .unwrap(),
        )
        .unwrap();
        let unchanged: serde_json::Value = serde_json::from_str(
            &memory
                .consolidate_json(
                    "manual",
                    Some("manual-native-test".to_string()),
                    Some("bindings".to_string()),
                    false,
                    false,
                )
                .unwrap(),
        )
        .unwrap();

        assert_eq!(dry_run["status"], "dry_run");
        assert_eq!(dry_run["candidate_count"], 1);
        assert_eq!(dry_run["source_memory_ids"][0], event.id);
        assert_eq!(created["status"], "created");
        assert_eq!(created["candidate_count"], 1);
        assert_eq!(unchanged["status"], "unchanged");
    }
}
