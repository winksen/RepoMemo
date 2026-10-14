//! An in-memory copy of a workspace's passage vectors, so a meaning search is
//! one pass over a matrix instead of a database scan that decodes every
//! vector for every query (roadmap P1).
//!
//! The copy is always checked against the database before it is used: the
//! number of vectors must match. Writes of the embedding queue update it in
//! place, so it stays current while a workspace is being embedded. A workspace
//! whose vectors would not fit the memory budget is searched by scanning the
//! database, as before.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use super::*;

/// Rows read from the database per page while an index is built.
const BUILD_PAGE: i64 = 2_000;

/// The vectors of one workspace for one embedding model, each scaled to
/// length one so a dot product is the cosine similarity.
pub(crate) struct WorkspaceVectors {
    model: String,
    dims: usize,
    ids: Vec<String>,
    data: Vec<f32>,
    positions: HashMap<String, usize>,
}

impl WorkspaceVectors {
    fn new(model: &str, dims: usize) -> Self {
        Self {
            model: model.to_owned(),
            dims,
            ids: Vec::new(),
            data: Vec::new(),
            positions: HashMap::new(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.ids.len()
    }

    fn bytes(&self) -> usize {
        self.data.len() * std::mem::size_of::<f32>() + self.ids.len() * 64
    }

    /// Adds or replaces the vector of a passage. Vectors of another size or
    /// with no direction are ignored.
    fn put(&mut self, chunk_id: &str, vector: &[f32]) {
        if vector.len() != self.dims {
            return;
        }
        let Some(unit) = normalised(vector) else {
            return;
        };
        match self.positions.get(chunk_id) {
            Some(&position) => {
                self.data[position * self.dims..(position + 1) * self.dims].copy_from_slice(&unit);
            }
            None => {
                self.positions.insert(chunk_id.to_owned(), self.ids.len());
                self.ids.push(chunk_id.to_owned());
                self.data.extend_from_slice(&unit);
            }
        }
    }

    /// The `count` passages closest to `query` (a vector of length one),
    /// nearest first, with their similarity.
    pub(crate) fn nearest(&self, query: &[f32], count: usize) -> Vec<(String, f32)> {
        if query.len() != self.dims || self.ids.is_empty() || count == 0 {
            return Vec::new();
        }
        let mut scored = self
            .data
            .chunks_exact(self.dims)
            .enumerate()
            .map(|(position, row)| (position, row.iter().zip(query).map(|(a, b)| a * b).sum::<f32>()))
            .collect::<Vec<_>>();
        let count = count.min(scored.len());
        if count < scored.len() {
            scored.select_nth_unstable_by(count - 1, |left, right| right.1.total_cmp(&left.1));
            scored.truncate(count);
        }
        scored.sort_by(|left, right| right.1.total_cmp(&left.1));
        scored
            .into_iter()
            .map(|(position, score)| (self.ids[position].clone(), score))
            .collect()
    }
}

/// The vector with length one, or `None` for one with no direction.
pub(crate) fn normalised(vector: &[f32]) -> Option<Vec<f32>> {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if !norm.is_finite() || norm <= 0.0 {
        return None;
    }
    Some(vector.iter().map(|value| value / norm).collect())
}

/// What the engine keeps: one index per workspace, within one memory budget.
pub(crate) struct VectorCache {
    budget_bytes: usize,
    workspaces: Mutex<HashMap<String, Arc<RwLock<WorkspaceVectors>>>>,
    /// One build at a time, so a burst of searches does not build it many times.
    build: tokio::sync::Mutex<()>,
}

impl VectorCache {
    pub(crate) fn new(budget_mb: u32) -> Self {
        Self {
            budget_bytes: budget_mb as usize * 1024 * 1024,
            workspaces: Mutex::new(HashMap::new()),
            build: tokio::sync::Mutex::new(()),
        }
    }

    fn get(&self, workspace_id: &str) -> Option<Arc<RwLock<WorkspaceVectors>>> {
        self.workspaces.lock().ok()?.get(workspace_id).cloned()
    }

    /// Memory held by the indexes, and how many workspaces have one.
    pub(crate) fn usage(&self) -> (usize, usize) {
        let Ok(workspaces) = self.workspaces.lock() else {
            return (0, 0);
        };
        let bytes = workspaces
            .values()
            .map(|index| index.read().map(|index| index.bytes()).unwrap_or(0))
            .sum();
        (bytes, workspaces.len())
    }

    /// Brings freshly written vectors into an index that exists already.
    pub(crate) fn apply(&self, workspace_id: &str, model: &str, vectors: &[(String, Vec<f32>)]) {
        let Some(index) = self.get(workspace_id) else {
            return;
        };
        let Ok(mut index) = index.write() else {
            return;
        };
        if index.model != model {
            return;
        }
        for (chunk_id, vector) in vectors {
            index.put(chunk_id, vector);
        }
    }

    fn forget(&self, workspace_id: &str) {
        if let Ok(mut workspaces) = self.workspaces.lock() {
            workspaces.remove(workspace_id);
        }
    }

    fn store(&self, workspace_id: &str, index: WorkspaceVectors) -> Arc<RwLock<WorkspaceVectors>> {
        let index = Arc::new(RwLock::new(index));
        if let Ok(mut workspaces) = self.workspaces.lock() {
            // Workspaces take turns: another one's copy makes room.
            let others = workspaces
                .iter()
                .filter(|(id, _)| id.as_str() != workspace_id)
                .map(|(_, other)| other.read().map(|other| other.bytes()).unwrap_or(0))
                .sum::<usize>();
            let needed = index.read().map(|index| index.bytes()).unwrap_or(0);
            if others + needed > self.budget_bytes {
                workspaces.retain(|id, _| id == workspace_id);
            }
            workspaces.insert(workspace_id.to_owned(), index.clone());
        }
        index
    }
}

impl StorageEngine {
    /// The in-memory index of a workspace's vectors from `model`, built when
    /// missing or out of date. `None` when there are no vectors, the index is
    /// turned off, or the vectors would not fit the budget: the caller then
    /// scans the database.
    pub(crate) async fn vector_index(
        &self,
        workspace_id: &str,
        model: &str,
    ) -> Result<Option<Arc<RwLock<WorkspaceVectors>>>> {
        if self.vectors.budget_bytes == 0 {
            return Ok(None);
        }
        let row = sqlx::query(
            "SELECT COUNT(*) AS total, COALESCE(MAX(dimensions), 0) AS dims
             FROM chunk_embeddings WHERE workspace_id = ?1 AND model = ?2",
        )
        .bind(workspace_id)
        .bind(model)
        .fetch_one(self.reader())
        .await?;
        let total = row.get::<i64, _>("total").max(0) as usize;
        let dims = row.get::<i64, _>("dims").max(0) as usize;
        if total == 0 || dims == 0 {
            self.vectors.forget(workspace_id);
            return Ok(None);
        }
        let is_current = |index: &Arc<RwLock<WorkspaceVectors>>| {
            index
                .read()
                .map(|index| index.model == model && index.len() == total)
                .unwrap_or(false)
        };
        if let Some(index) = self.vectors.get(workspace_id).filter(is_current) {
            return Ok(Some(index));
        }
        if total * (dims * std::mem::size_of::<f32>() + 64) > self.vectors.budget_bytes {
            self.vectors.forget(workspace_id);
            return Ok(None);
        }

        let _building = self.vectors.build.lock().await;
        if let Some(index) = self.vectors.get(workspace_id).filter(is_current) {
            return Ok(Some(index));
        }
        let started = std::time::Instant::now();
        let mut built = WorkspaceVectors::new(model, dims);
        let mut after = String::new();
        loop {
            let rows = sqlx::query(
                "SELECT chunk_id, vector_blob FROM chunk_embeddings
                 WHERE workspace_id = ?1 AND model = ?2 AND chunk_id > ?3
                 ORDER BY chunk_id LIMIT ?4",
            )
            .bind(workspace_id)
            .bind(model)
            .bind(&after)
            .bind(BUILD_PAGE)
            .fetch_all(self.reader())
            .await?;
            let Some(last) = rows.last() else {
                break;
            };
            after = last.get::<String, _>("chunk_id");
            let page = rows.len();
            for row in &rows {
                if let Some(vector) = decode_embedding(&row.get::<Vec<u8>, _>("vector_blob")) {
                    built.put(&row.get::<String, _>("chunk_id"), &vector);
                }
            }
            if (page as i64) < BUILD_PAGE {
                break;
            }
        }
        tracing::debug!(
            target: "timing",
            workspace_id,
            vectors = built.len(),
            elapsed_ms = started.elapsed().as_millis() as u64,
            "Built the vector index"
        );
        Ok(Some(self.vectors.store(workspace_id, built)))
    }

    /// Memory held by the vector indexes (bytes) and the workspaces that have one.
    pub fn vector_index_usage(&self) -> (usize, usize) {
        self.vectors.usage()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_returns_the_closest_passages_first() {
        let mut index = WorkspaceVectors::new("m", 2);
        index.put("east", &[2.0, 0.0]);
        index.put("north", &[0.0, 3.0]);
        index.put("north-east", &[1.0, 1.0]);
        index.put("ignored", &[1.0]);
        index.put("empty", &[0.0, 0.0]);
        assert_eq!(index.len(), 3);
        let query = normalised(&[1.0, 0.2]).unwrap();
        let found = index.nearest(&query, 2);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].0, "east");
        assert_eq!(found[1].0, "north-east");
        assert!(found[0].1 > found[1].1);
        assert!(
            (found[0].1 - 0.9806).abs() < 0.01,
            "cosine similarity, not a raw dot product"
        );
        // Writing a passage again replaces its vector.
        index.put("east", &[0.0, 5.0]);
        assert_eq!(index.len(), 3);
        assert_eq!(index.nearest(&normalised(&[0.0, 1.0]).unwrap(), 1)[0].1.round(), 1.0);
        assert!(index.nearest(&[1.0, 0.0, 0.0], 3).is_empty(), "other sizes never match");
    }
}
