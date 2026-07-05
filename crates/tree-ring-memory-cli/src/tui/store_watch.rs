use tree_ring_memory_core::MemoryEvent;
use tree_ring_memory_sqlite::SQLiteMemoryStore;

use super::model::DashboardStats;

#[derive(Debug, Clone)]
pub struct StoreSnapshot {
    pub memories: Vec<MemoryEvent>,
    pub dashboard: DashboardStats,
}

pub struct StoreWatcher {
    previous: Option<DashboardStats>,
}

impl StoreWatcher {
    pub fn new() -> Self {
        Self { previous: None }
    }

    pub fn refresh(&mut self, store: &SQLiteMemoryStore) -> Result<StoreSnapshot, String> {
        let memories = store.list_all(true).map_err(|err| err.to_string())?;
        let dashboard = DashboardStats::from_memories(&memories, self.previous.as_ref());
        self.previous = Some(dashboard.clone());
        Ok(StoreSnapshot {
            memories,
            dashboard,
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;
    use tree_ring_memory_core::MemoryEvent;

    use super::*;

    #[test]
    fn store_refresh_derives_current_counts() {
        let dir = tempdir().unwrap();
        let mut store = SQLiteMemoryStore::open(dir.path().join("memory.sqlite")).unwrap();
        let event = MemoryEvent::new("Use store watch", "lesson").unwrap();
        store.put(&event).unwrap();
        let mut watcher = StoreWatcher::new();

        let snapshot = watcher.refresh(&store).unwrap();

        assert_eq!(snapshot.dashboard.total, 1);
        assert_eq!(snapshot.memories.len(), 1);
    }
}
