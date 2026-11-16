//! Due reminders (V0.7.15): the open, assigned tasks that have a due date, and the ledger that makes a reminder go out once. A
//! reminder is claimed before it is sent, so two passes (or a retry after a crash) cannot both send it.

use super::*;

/// An open task with a due date and an assignee who can still read the workspace.
#[derive(Debug, Clone)]
pub struct TaskReminderCandidate {
    pub task_id: String,
    pub workspace_id: String,
    pub key: String,
    pub title: String,
    pub due_at: String,
    pub user_id: String,
    pub email: Option<String>,
}

impl StorageEngine {
    /// The tasks that might need a reminder now: not done, with a due date, assigned to someone who is not deactivated and
    /// whose membership has not ended. Whether a reminder is owed is decided from the due date by the caller.
    pub async fn task_reminder_candidates(&self) -> Result<Vec<TaskReminderCandidate>> {
        let rows = sqlx::query(
            r#"
            SELECT task.id, task.workspace_id, COALESCE(task.number, 0) AS number, task.title, task.due_at,
                   task.assignee_user_id AS user_id, user.email
            FROM workspace_tasks AS task
            JOIN users AS user ON user.id = task.assignee_user_id AND user.deactivated_at IS NULL
            JOIN workspace_memberships AS membership
              ON membership.workspace_id = task.workspace_id AND membership.user_id = task.assignee_user_id
             AND (membership.expires_at IS NULL OR membership.expires_at > ?1)
            WHERE task.status != 'done' AND task.due_at IS NOT NULL AND task.assignee_user_id IS NOT NULL
            ORDER BY task.due_at ASC, task.id ASC
            LIMIT 1000
            "#,
        )
        .bind(Utc::now().to_rfc3339())
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .iter()
            .map(|row| TaskReminderCandidate {
                task_id: row.get("id"),
                workspace_id: row.get("workspace_id"),
                key: format!("T-{}", row.get::<i64, _>("number")),
                title: row.get("title"),
                due_at: row.get("due_at"),
                user_id: row.get("user_id"),
                email: row.get("email"),
            })
            .collect())
    }

    /// Takes the right to send this reminder. True when nothing was sent for this task, person, kind and due value yet; false
    /// when it was, which is how a retry or a second pass sends nothing.
    pub async fn claim_task_reminder(&self, candidate: &TaskReminderCandidate, kind: &str) -> Result<bool> {
        let claimed = sqlx::query(
            r#"INSERT INTO task_reminders (task_id, user_id, workspace_id, kind, due_key, sent_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)
               ON CONFLICT(task_id, user_id, kind) DO UPDATE SET due_key = excluded.due_key, sent_at = excluded.sent_at
               WHERE task_reminders.due_key != excluded.due_key"#,
        )
        .bind(&candidate.task_id)
        .bind(&candidate.user_id)
        .bind(&candidate.workspace_id)
        .bind(kind)
        .bind(&candidate.due_at)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(claimed.rows_affected() > 0)
    }

    /// Gives a claim back because nothing could be delivered, so the next pass tries again.
    pub async fn release_task_reminder(&self, candidate: &TaskReminderCandidate, kind: &str) -> Result<()> {
        sqlx::query("DELETE FROM task_reminders WHERE task_id = ?1 AND user_id = ?2 AND kind = ?3 AND due_key = ?4")
            .bind(&candidate.task_id)
            .bind(&candidate.user_id)
            .bind(kind)
            .bind(&candidate.due_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
