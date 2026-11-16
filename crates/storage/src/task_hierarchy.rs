//! Task hierarchy (V0.7.8): reading a task's sub-tasks and the shape of the tree around it, so a save can refuse a cycle or a tree
//! that is too deep. A parent that is completed hides none of its sub-tasks: they stay listed and open.

use super::*;
use crate::collaboration::TASK_SELECT;

/// The deepest a tree of tasks may go, counting the top task as level 1.
pub const MAX_TASK_DEPTH: usize = 5;

impl StorageEngine {
    /// The direct sub-tasks of a task, open ones first.
    pub async fn list_child_tasks(&self, task_id: &str) -> Result<Vec<CollaborationTask>> {
        let rows = sqlx::query_as::<_, CollaborationTaskRow>(&format!(
            "{TASK_SELECT} WHERE task.parent_task_id = ?1 ORDER BY CASE WHEN task.status = 'done' THEN 1 ELSE 0 END, task.number ASC"
        ))
        .bind(task_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.into_iter().map(CollaborationTask::from).collect())
    }

    /// The ids above a task, nearest first (its parent, that parent's parent, and so on). Stops at a loop, so a bad record
    /// cannot hang a request.
    pub async fn task_ancestor_ids(&self, task_id: &str) -> Result<Vec<String>> {
        let mut found: Vec<String> = Vec::new();
        let mut at = task_id.to_owned();
        for _ in 0..=MAX_TASK_DEPTH * 4 {
            let parent: Option<Option<String>> =
                sqlx::query_scalar("SELECT parent_task_id FROM workspace_tasks WHERE id = ?1")
                    .bind(&at)
                    .fetch_optional(self.reader())
                    .await?;
            match parent.flatten() {
                Some(parent) if !found.contains(&parent) => {
                    found.push(parent.clone());
                    at = parent;
                }
                _ => break,
            }
        }
        Ok(found)
    }

    /// How many levels a task's own sub-tasks add below it (0 for a task with none).
    pub async fn task_subtree_height(&self, task_id: &str) -> Result<usize> {
        let mut height = 0;
        let mut level = vec![task_id.to_owned()];
        while !level.is_empty() && height <= MAX_TASK_DEPTH {
            let mut next = Vec::new();
            for id in &level {
                let children: Vec<String> =
                    sqlx::query_scalar("SELECT id FROM workspace_tasks WHERE parent_task_id = ?1")
                        .bind(id)
                        .fetch_all(self.reader())
                        .await?;
                next.extend(children);
            }
            if next.is_empty() {
                break;
            }
            height += 1;
            level = next;
        }
        Ok(height)
    }
}
