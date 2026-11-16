//! Labels (V0.7.10): workspace-managed names for kinds of work. A label organizes and filters; nothing reads it to decide who
//! may see or change a task. Renaming keeps every assignment, and merging moves one label's tasks to another.

use super::*;
use repomemo_domain::{label_colors, TaskLabel, TaskLabelTag};

/// The most labels one task can carry.
pub const MAX_LABELS_PER_TASK: usize = 10;
/// The most labels a workspace can have.
pub const MAX_LABELS_PER_WORKSPACE: i64 = 200;

/// A label as a tag on a task, built from the values stored.
pub(crate) fn tag(id: String, name: String, color: String) -> TaskLabelTag {
    let (background_color, text_color) = label_colors(&color);
    TaskLabelTag {
        id,
        name,
        color,
        background_color: background_color.to_owned(),
        text_color: text_color.to_owned(),
    }
}

fn label_from(row: &SqliteRow) -> TaskLabel {
    let tag = tag(row.get("id"), row.get("name"), row.get("color"));
    TaskLabel {
        id: tag.id,
        name: tag.name,
        color: tag.color,
        background_color: tag.background_color,
        text_color: tag.text_color,
        workspace_id: row.get("workspace_id"),
        task_count: row.get("task_count"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

const SELECT_LABEL: &str = r#"
    SELECT label.id, label.workspace_id, label.name, label.color, label.created_at, label.updated_at,
           (SELECT COUNT(*) FROM task_label_links AS link WHERE link.label_id = label.id) AS task_count
    FROM task_labels AS label"#;

impl StorageEngine {
    pub async fn list_task_labels(&self, workspace_id: &str) -> Result<Vec<TaskLabel>> {
        let rows = sqlx::query(&format!(
            "{SELECT_LABEL} WHERE label.workspace_id = ?1 ORDER BY label.name_key ASC"
        ))
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(label_from).collect())
    }

    pub async fn get_task_label(&self, label_id: &str) -> Result<TaskLabel> {
        sqlx::query(&format!("{SELECT_LABEL} WHERE label.id = ?1"))
            .bind(label_id)
            .fetch_optional(self.reader())
            .await?
            .map(|row| label_from(&row))
            .ok_or_else(|| repomemo_domain::not_found!("Label was not found."))
    }

    pub async fn create_task_label(&self, workspace_id: &str, name: &str, color: &str) -> Result<TaskLabel> {
        let held: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_labels WHERE workspace_id = ?1")
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;
        if held >= MAX_LABELS_PER_WORKSPACE {
            repomemo_domain::bail_conflict!("A workspace can have at most {MAX_LABELS_PER_WORKSPACE} labels.");
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let inserted = sqlx::query(
            "INSERT INTO task_labels (id, workspace_id, name, name_key, color, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(name)
        .bind(name.to_lowercase())
        .bind(color)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(anyhow::Error::from);
        match inserted {
            Ok(_) => self.get_task_label(&id).await,
            Err(error) if is_unique_violation(&error) => {
                repomemo_domain::bail_conflict!("A label named \"{name}\" already exists.")
            }
            Err(error) => Err(error),
        }
    }

    /// Renames or recolours a label. Every task keeps it: the link holds the label's id, not its name.
    pub async fn update_task_label(&self, label_id: &str, name: &str, color: &str) -> Result<TaskLabel> {
        let updated =
            sqlx::query("UPDATE task_labels SET name = ?2, name_key = ?3, color = ?4, updated_at = ?5 WHERE id = ?1")
                .bind(label_id)
                .bind(name)
                .bind(name.to_lowercase())
                .bind(color)
                .bind(Utc::now().to_rfc3339())
                .execute(&self.pool)
                .await
                .map_err(anyhow::Error::from);
        match updated {
            Ok(done) if done.rows_affected() == 0 => repomemo_domain::bail_not_found!("Label was not found."),
            Ok(_) => self.get_task_label(label_id).await,
            Err(error) if is_unique_violation(&error) => {
                repomemo_domain::bail_conflict!("A label named \"{name}\" already exists.")
            }
            Err(error) => Err(error),
        }
    }

    pub async fn delete_task_label(&self, label_id: &str) -> Result<()> {
        let removed = sqlx::query("DELETE FROM task_labels WHERE id = ?1")
            .bind(label_id)
            .execute(&self.pool)
            .await?;
        if removed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Label was not found.");
        }
        Ok(())
    }

    /// Moves every task of `from` to `into` (a task that has both keeps one) and removes `from`.
    pub async fn merge_task_labels(&self, from: &str, into: &str) -> Result<TaskLabel> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT OR IGNORE INTO task_label_links (task_id, label_id, created_at)
             SELECT task_id, ?2, created_at FROM task_label_links WHERE label_id = ?1",
        )
        .bind(from)
        .bind(into)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM task_labels WHERE id = ?1")
            .bind(from)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        self.get_task_label(into).await
    }

    /// Replaces the labels on a task. Every label must belong to the task's workspace.
    pub async fn set_task_labels(&self, task: &CollaborationTask, label_ids: &[String]) -> Result<Vec<TaskLabelTag>> {
        let mut wanted: Vec<String> = Vec::new();
        for id in label_ids {
            if !wanted.contains(id) {
                wanted.push(id.clone());
            }
        }
        if wanted.len() > MAX_LABELS_PER_TASK {
            repomemo_domain::bail_invalid!("A task can have at most {MAX_LABELS_PER_TASK} labels.");
        }
        for id in &wanted {
            let found: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_labels WHERE id = ?1 AND workspace_id = ?2")
                .bind(id)
                .bind(&task.workspace_id)
                .fetch_one(&self.pool)
                .await?;
            if found == 0 {
                repomemo_domain::bail_invalid!("Every label must belong to the task's workspace.");
            }
        }
        let mut transaction = self.pool.begin().await?;
        sqlx::query("DELETE FROM task_label_links WHERE task_id = ?1")
            .bind(&task.id)
            .execute(&mut *transaction)
            .await?;
        let now = Utc::now().to_rfc3339();
        for id in &wanted {
            sqlx::query("INSERT INTO task_label_links (task_id, label_id, created_at) VALUES (?1, ?2, ?3)")
                .bind(&task.id)
                .bind(id)
                .bind(&now)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(self.get_collaboration_task(&task.id).await?.labels)
    }

    /// A workspace's tasks that carry a label, in the usual order.
    pub async fn list_collaboration_tasks_by_label(
        &self,
        workspace_id: &str,
        label_id: &str,
    ) -> Result<Vec<CollaborationTask>> {
        let tasks = self.list_collaboration_tasks(workspace_id).await?;
        Ok(tasks
            .into_iter()
            .filter(|task| task.labels.iter().any(|label| label.id == label_id))
            .collect())
    }
}
