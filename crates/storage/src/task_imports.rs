//! CSV migration (V0.7.18): making the tasks of an import. Each row is made in one transaction with its entry in the ledger of
//! imported rows, so a retry, a second attempt or a restart cannot make a row twice and cannot leave one half made.

use super::*;

/// What one valid row of a CSV becomes.
#[derive(Debug, Clone, Default)]
pub struct ImportedTaskRow {
    pub title: String,
    pub description: String,
    /// The category (`open`, `in_progress`, `blocked`, `done`).
    pub status: String,
    pub priority: String,
    pub assignee_user_id: Option<String>,
    pub due_at: Option<String>,
    pub project_id: Option<String>,
    /// A named workflow state whose category is `status`.
    pub state_id: Option<String>,
    pub label_ids: Vec<String>,
}

impl StorageEngine {
    /// Which of these source keys were already imported into the workspace.
    pub async fn known_task_import_keys(
        &self,
        workspace_id: &str,
        keys: &[String],
    ) -> Result<std::collections::HashSet<String>> {
        let mut known = std::collections::HashSet::new();
        for key in keys {
            let found: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM task_imports WHERE workspace_id = ?1 AND source_key = ?2")
                    .bind(workspace_id)
                    .bind(key)
                    .fetch_one(self.reader())
                    .await?;
            if found > 0 {
                known.insert(key.clone());
            }
        }
        Ok(known)
    }

    /// Makes one row, unless its source key was imported before (then `None`). The ledger entry, the task, its number and its
    /// labels are written together.
    pub async fn import_task_row(
        &self,
        workspace_id: &str,
        user_id: &str,
        source_key: &str,
        row: ImportedTaskRow,
    ) -> Result<Option<CollaborationTask>> {
        let now = Utc::now().to_rfc3339();
        let mut transaction = self.pool.begin().await?;
        let claimed = sqlx::query(
            "INSERT OR IGNORE INTO task_imports (workspace_id, source_key, task_id, imported_by_user_id, imported_at)
             VALUES (?1, ?2, NULL, ?3, ?4)",
        )
        .bind(workspace_id)
        .bind(source_key)
        .bind(user_id)
        .bind(&now)
        .execute(&mut *transaction)
        .await?;
        if claimed.rows_affected() == 0 {
            return Ok(None);
        }
        let id = Uuid::new_v4().to_string();
        let number = crate::collaboration::take_task_number(&mut transaction, workspace_id).await?;
        let completed_at = (row.status == "done").then(|| now.clone());
        sqlx::query(
            "INSERT INTO workspace_tasks (id, workspace_id, title, description, status, priority, assignee_user_id,
                created_by_user_id, artifact_id, due_at, completed_at, created_at, updated_at, project_id, number, state_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10, ?11, ?11, ?12, ?13, ?14)",
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(&row.title)
        .bind(&row.description)
        .bind(&row.status)
        .bind(&row.priority)
        .bind(&row.assignee_user_id)
        .bind(user_id)
        .bind(&row.due_at)
        .bind(completed_at)
        .bind(&now)
        .bind(&row.project_id)
        .bind(number)
        .bind(&row.state_id)
        .execute(&mut *transaction)
        .await?;
        for label_id in &row.label_ids {
            sqlx::query("INSERT OR IGNORE INTO task_label_links (task_id, label_id, created_at) VALUES (?1, ?2, ?3)")
                .bind(&id)
                .bind(label_id)
                .bind(&now)
                .execute(&mut *transaction)
                .await?;
        }
        sqlx::query("UPDATE task_imports SET task_id = ?3 WHERE workspace_id = ?1 AND source_key = ?2")
            .bind(workspace_id)
            .bind(source_key)
            .bind(&id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(Some(self.get_collaboration_task(&id).await?))
    }
}

/// A fingerprint of some text parts (SHA-256, hex), so a row without an id in its file still has a stable identity.
pub fn row_fingerprint(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0x1f]);
    }
    hex::encode(hasher.finalize())
}
