//! Task discussion (V0.7.4): comments on a task, and who follows it. A task's creator and assignee follow it unless they
//! mute it, anyone else follows it by asking; muting never touches the notice that a task was assigned.

use super::*;

const SELECT_COMMENT: &str = r#"
    SELECT comment.id, comment.task_id, comment.workspace_id, comment.body, comment.created_at, comment.updated_at,
           author.id AS author_id, author.display_name AS author_name, author.email AS author_email
    FROM task_comments AS comment
    LEFT JOIN users AS author ON author.id = comment.author_user_id"#;

fn comment_from(row: &SqliteRow) -> TaskComment {
    let author_id: Option<String> = row.get("author_id");
    TaskComment {
        id: row.get("id"),
        task_id: row.get("task_id"),
        workspace_id: row.get("workspace_id"),
        author: author_id.map(|id| SharedUser {
            id,
            display_name: row.get("author_name"),
            email: row.get("author_email"),
        }),
        body: row.get("body"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

impl StorageEngine {
    pub async fn list_task_comments(&self, task_id: &str) -> Result<Vec<TaskComment>> {
        let rows = sqlx::query(&format!(
            "{SELECT_COMMENT} WHERE comment.task_id = ?1 ORDER BY comment.created_at ASC, comment.id ASC"
        ))
        .bind(task_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(comment_from).collect())
    }

    pub async fn count_task_comments(&self, task_id: &str) -> Result<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM task_comments WHERE task_id = ?1")
                .bind(task_id)
                .fetch_one(self.reader())
                .await?,
        )
    }

    pub async fn get_task_comment(&self, comment_id: &str) -> Result<TaskComment> {
        sqlx::query(&format!("{SELECT_COMMENT} WHERE comment.id = ?1"))
            .bind(comment_id)
            .fetch_optional(self.reader())
            .await?
            .map(|row| comment_from(&row))
            .ok_or_else(|| repomemo_domain::not_found!("Comment was not found."))
    }

    pub async fn add_task_comment(&self, task: &CollaborationTask, author: &str, body: &str) -> Result<TaskComment> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO task_comments (id, task_id, workspace_id, author_user_id, body, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        )
        .bind(&id)
        .bind(&task.id)
        .bind(&task.workspace_id)
        .bind(author)
        .bind(body)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.get_task_comment(&id).await
    }

    pub async fn update_task_comment(&self, comment_id: &str, body: &str) -> Result<TaskComment> {
        let changed = sqlx::query("UPDATE task_comments SET body = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(comment_id)
            .bind(body)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Comment was not found.");
        }
        self.get_task_comment(comment_id).await
    }

    pub async fn delete_task_comment(&self, comment_id: &str) -> Result<()> {
        let removed = sqlx::query("DELETE FROM task_comments WHERE id = ?1")
            .bind(comment_id)
            .execute(&self.pool)
            .await?;
        if removed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Comment was not found.");
        }
        Ok(())
    }

    /// How a person follows a task: `watching`, `muted` or `default` (the creator and assignee follow it, nobody else).
    pub async fn task_watch_mode(&self, task_id: &str, user_id: &str) -> Result<String> {
        let mode: Option<String> =
            sqlx::query_scalar("SELECT mode FROM task_watchers WHERE task_id = ?1 AND user_id = ?2")
                .bind(task_id)
                .bind(user_id)
                .fetch_optional(self.reader())
                .await?;
        Ok(mode.unwrap_or_else(|| "default".to_owned()))
    }

    /// Sets how a person follows a task. `default` removes the choice.
    pub async fn set_task_watch(&self, task_id: &str, user_id: &str, mode: &str) -> Result<()> {
        if mode == "default" {
            sqlx::query("DELETE FROM task_watchers WHERE task_id = ?1 AND user_id = ?2")
                .bind(task_id)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
        } else {
            sqlx::query(
                "INSERT INTO task_watchers (task_id, user_id, mode, created_at) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(task_id, user_id) DO UPDATE SET mode = excluded.mode",
            )
            .bind(task_id)
            .bind(user_id)
            .bind(mode)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Who follows a task: whoever asked to, and its creator and assignee, minus whoever muted it. The caller still checks
    /// that each person can read the workspace now.
    pub async fn task_followers(&self, task: &CollaborationTask) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT user_id, mode FROM task_watchers WHERE task_id = ?1")
            .bind(&task.id)
            .fetch_all(self.reader())
            .await?;
        let mut followers = std::collections::BTreeSet::new();
        let mut muted = std::collections::BTreeSet::new();
        followers.insert(task.created_by.id.clone());
        if let Some(assignee) = &task.assignee {
            followers.insert(assignee.id.clone());
        }
        for row in &rows {
            let user: String = row.get("user_id");
            let mode: String = row.get("mode");
            if mode == "watching" {
                followers.insert(user);
            } else {
                muted.insert(user);
            }
        }
        Ok(followers.difference(&muted).cloned().collect())
    }
}
