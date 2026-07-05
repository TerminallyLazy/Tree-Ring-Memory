use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use std::path::PathBuf;
use tree_ring_memory_core::sensitivity::SensitivityGuard;
use tree_ring_memory_core::MemoryEvent;
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
        event.validate().map_err(to_py_value_error)?;
        self.store.put(&event).map_err(to_py_runtime_error)?;
        serde_json::to_string(&event).map_err(to_py_runtime_error)
    }

    pub fn put_event_json(&mut self, event_json: &str) -> PyResult<String> {
        let mut event: MemoryEvent = serde_json::from_str(event_json).map_err(to_py_value_error)?;
        let detected_sensitivity = SensitivityGuard::default()
            .detect_memory_event_sensitivity(&event)
            .map_err(to_py_value_error)?;
        if event.sensitivity == "normal" && detected_sensitivity != "normal" {
            event.sensitivity = detected_sensitivity;
        }
        event.validate().map_err(to_py_value_error)?;
        self.store.put(&event).map_err(to_py_runtime_error)?;
        serde_json::to_string(&event).map_err(to_py_runtime_error)
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
}
