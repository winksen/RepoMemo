//! Results that are expensive to compute and only change when a workspace's
//! content does: the knowledge map and Workspace Health (roadmap P5).
//!
//! An entry is keyed by the workspace's content version (see
//! `StorageEngine::workspace_content_version`). A request computes the version
//! with a handful of counts, and recomputes only when it differs. Entries also
//! expire, as a safety net for changes the version does not cover.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use repomemo_domain::{KnowledgeMap, WorkspaceHealth};

/// Longest an entry is served, whatever the version says.
const MAX_AGE: Duration = Duration::from_secs(10 * 60);
/// Workspaces kept per kind of result.
const MAX_ENTRIES: usize = 64;

struct Entry<T> {
    version: String,
    stored: Instant,
    value: T,
}

#[derive(Default)]
pub(crate) struct DerivedCache {
    maps: Mutex<HashMap<String, Entry<KnowledgeMap>>>,
    health: Mutex<HashMap<String, Entry<WorkspaceHealth>>>,
}

fn fetch<T: Clone>(entries: &Mutex<HashMap<String, Entry<T>>>, workspace_id: &str, version: &str) -> Option<T> {
    let entries = entries.lock().ok()?;
    let entry = entries.get(workspace_id)?;
    (entry.version == version && entry.stored.elapsed() < MAX_AGE).then(|| entry.value.clone())
}

fn keep<T>(entries: &Mutex<HashMap<String, Entry<T>>>, workspace_id: &str, version: String, value: T) {
    let Ok(mut entries) = entries.lock() else {
        return;
    };
    if entries.len() >= MAX_ENTRIES && !entries.contains_key(workspace_id) {
        entries.retain(|_, entry| entry.stored.elapsed() < MAX_AGE / 2);
        if entries.len() >= MAX_ENTRIES {
            entries.clear();
        }
    }
    entries.insert(
        workspace_id.to_owned(),
        Entry {
            version,
            stored: Instant::now(),
            value,
        },
    );
}

impl DerivedCache {
    pub(crate) fn knowledge_map(&self, workspace_id: &str, version: &str) -> Option<KnowledgeMap> {
        fetch(&self.maps, workspace_id, version)
    }

    pub(crate) fn keep_knowledge_map(&self, workspace_id: &str, version: String, map: KnowledgeMap) {
        keep(&self.maps, workspace_id, version, map);
    }

    pub(crate) fn health(&self, workspace_id: &str, version: &str) -> Option<WorkspaceHealth> {
        fetch(&self.health, workspace_id, version)
    }

    pub(crate) fn keep_health(&self, workspace_id: &str, version: String, health: WorkspaceHealth) {
        keep(&self.health, workspace_id, version, health);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_is_served_only_for_the_version_it_was_made_for() {
        let entries = Mutex::new(HashMap::new());
        keep(&entries, "w", "v1".to_owned(), 7_u32);
        assert_eq!(fetch(&entries, "w", "v1"), Some(7));
        assert_eq!(fetch(&entries, "w", "v2"), None);
        assert_eq!(fetch(&entries, "other", "v1"), None);
        keep(&entries, "w", "v2".to_owned(), 8_u32);
        assert_eq!(fetch(&entries, "w", "v2"), Some(8));
    }
}
