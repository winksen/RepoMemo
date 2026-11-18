//! Task context (V0.7.5): the evidence and memory cards a task cites. Nothing cited is copied; the title is read from the
//! target each time, so a deleted target shows as unavailable. The task's old single file link is kept as a reference too.

use super::*;
use repomemo_domain::{FollowUpTask, RemoteItemInfo};

/// The most references one task can hold.
pub const MAX_TASK_REFERENCES: i64 = 50;

const SELECT_REFERENCE: &str = r#"
    SELECT reference.id, reference.task_id, reference.kind, reference.target_id, reference.note, reference.created_at,
           adder.id AS adder_id, adder.display_name AS adder_name, adder.email AS adder_email,
           CASE reference.kind
             WHEN 'memory_card' THEN (SELECT title FROM memory_cards WHERE memory_cards.id = reference.target_id AND memory_cards.workspace_id = reference.workspace_id)
             ELSE (SELECT title FROM artifacts WHERE artifacts.id = reference.target_id AND artifacts.workspace_id = reference.workspace_id
                   AND NOT EXISTS (SELECT 1 FROM connector_quarantine WHERE connector_quarantine.source_id = artifacts.source_id))
           END AS title,
           item.source_id AS remote_source_id, item.state AS remote_state, item.remote_updated_at AS remote_updated_at,
           (SELECT name FROM sources WHERE sources.id = item.source_id) AS remote_source_name,
           (SELECT json_extract(artifacts.metadata_json, '$.url') FROM artifacts WHERE artifacts.id = reference.target_id) AS remote_url,
           (SELECT json_extract(artifacts.metadata_json, '$.provider') FROM artifacts WHERE artifacts.id = reference.target_id) AS remote_provider
    FROM task_references AS reference
    LEFT JOIN connector_items AS item ON item.artifact_id = reference.target_id AND reference.kind = 'evidence'
    LEFT JOIN users AS adder ON adder.id = reference.added_by_user_id"#;

fn task_reference_from(row: &SqliteRow) -> TaskReference {
    let adder_id: Option<String> = row.get("adder_id");
    let title: Option<String> = row.get("title");
    // A quarantined item has no title here, and no remote state either.
    let remote = row
        .get::<Option<String>, _>("remote_source_id")
        .filter(|_| title.is_some())
        .map(|source_id| RemoteItemInfo {
            provider: row.get::<Option<String>, _>("remote_provider").unwrap_or_default(),
            source_name: row.get::<Option<String>, _>("remote_source_name").unwrap_or_default(),
            source_id,
            state: row.get::<Option<String>, _>("remote_state").unwrap_or_default(),
            url: row.get("remote_url"),
            synced_at: row.get::<Option<String>, _>("remote_updated_at").unwrap_or_default(),
            owner: "remote".to_owned(),
        });
    TaskReference {
        id: row.get("id"),
        task_id: row.get("task_id"),
        kind: row.get("kind"),
        target_id: row.get("target_id"),
        available: title.is_some(),
        title,
        note: row.get("note"),
        added_by: adder_id.map(|id| SharedUser {
            id,
            display_name: row.get("adder_name"),
            email: row.get("adder_email"),
        }),
        remote,
        created_at: row.get("created_at"),
    }
}

impl StorageEngine {
    /// What the tracker says about a mirrored item, or none when the file is not one (or is held back by a quarantine).
    pub async fn remote_item_info(&self, artifact_id: &str) -> Result<Option<RemoteItemInfo>> {
        let row = sqlx::query(
            r#"SELECT item.source_id, item.state, item.remote_updated_at, sources.name AS source_name,
                      json_extract(artifacts.metadata_json, '$.url') AS url,
                      json_extract(artifacts.metadata_json, '$.provider') AS provider
               FROM connector_items AS item
               JOIN artifacts ON artifacts.id = item.artifact_id
               JOIN sources ON sources.id = item.source_id
               WHERE item.artifact_id = ?1
                 AND NOT EXISTS (SELECT 1 FROM connector_quarantine WHERE connector_quarantine.source_id = item.source_id)"#,
        )
        .bind(artifact_id)
        .fetch_optional(self.reader())
        .await?;
        Ok(row.map(|row| RemoteItemInfo {
            provider: row.get::<Option<String>, _>("provider").unwrap_or_default(),
            source_id: row.get("source_id"),
            source_name: row.get("source_name"),
            state: row.get("state"),
            url: row.get("url"),
            synced_at: row.get("remote_updated_at"),
            owner: "remote".to_owned(),
        }))
    }

    /// The tasks of a workspace that cite an item, newest first.
    pub async fn tasks_following(&self, workspace_id: &str, artifact_id: &str) -> Result<Vec<FollowUpTask>> {
        let rows = sqlx::query(
            r#"SELECT task.id, task.number, task.title, task.status, task.project_id, task.created_at,
                      project.name AS project_name, assignee.display_name AS assignee_name
               FROM task_references AS reference
               JOIN workspace_tasks AS task ON task.id = reference.task_id
               LEFT JOIN workspace_projects AS project ON project.id = task.project_id
               LEFT JOIN users AS assignee ON assignee.id = task.assignee_user_id
               WHERE reference.workspace_id = ?1 AND reference.kind = 'evidence' AND reference.target_id = ?2
               ORDER BY task.created_at DESC"#,
        )
        .bind(workspace_id)
        .bind(artifact_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .iter()
            .map(|row| FollowUpTask {
                task_id: row.get("id"),
                key: format!("T-{}", row.get::<i64, _>("number")),
                title: row.get("title"),
                status: row.get("status"),
                assignee_name: row.get("assignee_name"),
                project_id: row.get("project_id"),
                project_name: row.get("project_name"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    /// Fails with "not found" unless the memory card or evidence exists in this workspace.
    pub async fn check_reference_target(&self, workspace_id: &str, kind: &str, target_id: &str) -> Result<()> {
        let exists: i64 = if kind == "memory_card" {
            sqlx::query_scalar("SELECT COUNT(*) FROM memory_cards WHERE id = ?1 AND workspace_id = ?2")
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM artifacts WHERE id = ?1 AND workspace_id = ?2")
        }
        .bind(target_id)
        .bind(workspace_id)
        .fetch_one(self.reader())
        .await?;
        if exists == 0 {
            repomemo_domain::bail_not_found!("The memory card or evidence was not found in this workspace.");
        }
        Ok(())
    }

    pub async fn list_task_references(&self, task_id: &str) -> Result<Vec<TaskReference>> {
        let rows = sqlx::query(&format!(
            "{SELECT_REFERENCE} WHERE reference.task_id = ?1 ORDER BY reference.created_at ASC, reference.id ASC"
        ))
        .bind(task_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(task_reference_from).collect())
    }

    pub async fn add_task_reference(
        &self,
        task: &CollaborationTask,
        added_by: &str,
        kind: &str,
        target_id: &str,
        note: &str,
    ) -> Result<TaskReference> {
        self.check_reference_target(&task.workspace_id, kind, target_id).await?;
        let held: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_references WHERE task_id = ?1")
            .bind(&task.id)
            .fetch_one(&self.pool)
            .await?;
        if held >= MAX_TASK_REFERENCES {
            repomemo_domain::bail_conflict!("A task can cite at most {MAX_TASK_REFERENCES} items.");
        }
        let already: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM task_references WHERE task_id = ?1 AND kind = ?2 AND target_id = ?3",
        )
        .bind(&task.id)
        .bind(kind)
        .bind(target_id)
        .fetch_one(&self.pool)
        .await?;
        if already > 0 {
            repomemo_domain::bail_conflict!("The task already cites this item.");
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO task_references (id, task_id, workspace_id, kind, target_id, note, added_by_user_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&id)
        .bind(&task.id)
        .bind(&task.workspace_id)
        .bind(kind)
        .bind(target_id)
        .bind(note)
        .bind(added_by)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        let row = sqlx::query(&format!("{SELECT_REFERENCE} WHERE reference.id = ?1"))
            .bind(&id)
            .fetch_one(&self.pool)
            .await?;
        Ok(task_reference_from(&row))
    }

    pub async fn remove_task_reference(&self, task_id: &str, reference_id: &str) -> Result<()> {
        let removed = sqlx::query("DELETE FROM task_references WHERE id = ?1 AND task_id = ?2")
            .bind(reference_id)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        if removed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("The reference was not found.");
        }
        Ok(())
    }

    /// Keeps the task's single file link (`artifact_id`) in its references, so the list is complete whichever way it was set.
    pub(crate) async fn sync_task_artifact_reference(&self, task_id: &str) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO task_references (id, task_id, workspace_id, kind, target_id, note, added_by_user_id, created_at)
             SELECT ?2, id, workspace_id, 'evidence', artifact_id, '', created_by_user_id, ?3
             FROM workspace_tasks WHERE id = ?1 AND artifact_id IS NOT NULL",
        )
        .bind(task_id)
        .bind(Uuid::new_v4().to_string())
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
