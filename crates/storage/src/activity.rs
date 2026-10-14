use super::*;

impl StorageEngine {
    pub async fn record_workspace_activity(
        &self,
        workspace_id: &str,
        actor_user_id: Option<&str>,
        action: &str,
        subject_type: &str,
        subject_id: Option<&str>,
        summary: &str,
    ) -> Result<()> {
        let event_id = Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO workspace_activity (id, workspace_id, actor_user_id, action, subject_type, subject_id, summary, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&event_id)
        .bind(workspace_id)
        .bind(actor_user_id)
        .bind(action)
        .bind(subject_type)
        .bind(subject_id)
        .bind(summary)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;

        // Emit only if someone is listening. Fetching the joined actor row
        // is cheap and keeps SSE payloads identical to the REST list view.
        if self
            .activity_observer
            .read()
            .ok()
            .and_then(|guard| guard.as_ref().map(|_| ()))
            .is_some()
        {
            if let Ok(Some(event)) = self.get_workspace_activity_event(&event_id).await {
                self.emit_activity(&event);
            }
        }
        Ok(())
    }

    pub(crate) async fn get_workspace_activity_event(&self, event_id: &str) -> Result<Option<WorkspaceActivityEvent>> {
        let row = sqlx::query_as::<_, WorkspaceActivityRow>(
            r#"
            SELECT
              activity.id,
              activity.workspace_id,
              actor.id AS actor_id,
              actor.email AS actor_email,
              actor.display_name AS actor_display_name,
              activity.action,
              activity.subject_type,
              activity.subject_id,
              activity.summary,
              activity.created_at
            FROM workspace_activity AS activity
            LEFT JOIN users AS actor ON actor.id = activity.actor_user_id
            WHERE activity.id = ?1
            "#,
        )
        .bind(event_id)
        .fetch_optional(self.reader())
        .await?;
        Ok(row.map(WorkspaceActivityEvent::from))
    }

    pub async fn list_workspace_activity(&self, workspace_id: &str, limit: i64) -> Result<Vec<WorkspaceActivityEvent>> {
        let rows = sqlx::query_as::<_, WorkspaceActivityRow>(
            r#"
            SELECT
              activity.id,
              activity.workspace_id,
              actor.id AS actor_id,
              actor.email AS actor_email,
              actor.display_name AS actor_display_name,
              activity.action,
              activity.subject_type,
              activity.subject_id,
              activity.summary,
              activity.created_at
            FROM workspace_activity AS activity
            LEFT JOIN users AS actor ON actor.id = activity.actor_user_id
            WHERE activity.workspace_id = ?1
            ORDER BY activity.created_at DESC, activity.id DESC
            LIMIT ?2
            "#,
        )
        .bind(workspace_id)
        .bind(limit.clamp(1, 100))
        .fetch_all(self.reader())
        .await?;

        Ok(rows.into_iter().map(WorkspaceActivityEvent::from).collect())
    }

    pub async fn list_user_activity(&self, user_id: &str, limit: i64) -> Result<Vec<WorkspaceActivityEvent>> {
        let rows = sqlx::query_as::<_, WorkspaceActivityRow>(
            r#"
            SELECT
              activity.id,
              activity.workspace_id,
              actor.id AS actor_id,
              actor.email AS actor_email,
              actor.display_name AS actor_display_name,
              activity.action,
              activity.subject_type,
              activity.subject_id,
              activity.summary,
              activity.created_at
            FROM workspace_activity AS activity
            LEFT JOIN users AS actor ON actor.id = activity.actor_user_id
            WHERE activity.actor_user_id = ?1
            ORDER BY activity.created_at DESC, activity.id DESC
            LIMIT ?2
            "#,
        )
        .bind(user_id)
        .bind(limit.clamp(1, 100))
        .fetch_all(self.reader())
        .await?;
        Ok(rows.into_iter().map(WorkspaceActivityEvent::from).collect())
    }

    pub async fn workspace_activity_by_day(&self, workspace_id: &str, since: &str) -> Result<Vec<(String, i64)>> {
        Ok(sqlx::query_as::<_, (String, i64)>(
            r#"
            SELECT substr(created_at, 1, 10) AS day, COUNT(*) AS count
            FROM workspace_activity
            WHERE workspace_id = ?1 AND created_at >= ?2
            GROUP BY day
            ORDER BY day ASC
            "#,
        )
        .bind(workspace_id)
        .bind(since)
        .fetch_all(self.reader())
        .await?)
    }

    pub async fn user_activity_by_day(&self, user_id: &str, since: &str) -> Result<Vec<(String, i64)>> {
        Ok(sqlx::query_as::<_, (String, i64)>(
            r#"
            SELECT substr(created_at, 1, 10) AS day, COUNT(*) AS count
            FROM workspace_activity
            WHERE actor_user_id = ?1 AND created_at >= ?2
            GROUP BY day
            ORDER BY day ASC
            "#,
        )
        .bind(user_id)
        .bind(since)
        .fetch_all(self.reader())
        .await?)
    }

    pub async fn workspace_collaboration_counts(&self, workspace_id: &str) -> Result<WorkspaceCollaborationCounts> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT status, COUNT(*) FROM workspace_tasks WHERE workspace_id = ?1 GROUP BY status",
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        let mut counts = WorkspaceCollaborationCounts::default();
        for (status, count) in rows {
            match status.as_str() {
                "open" => counts.open_tasks = count,
                "in_progress" => counts.in_progress_tasks = count,
                "blocked" => counts.blocked_tasks = count,
                "done" => counts.completed_tasks = count,
                _ => {}
            }
        }
        counts.overdue_tasks = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM workspace_tasks
            WHERE workspace_id = ?1 AND status != 'done' AND due_at IS NOT NULL AND due_at < ?2
            "#,
        )
        .bind(workspace_id)
        .bind(Utc::now().to_rfc3339())
        .fetch_one(self.reader())
        .await?;
        counts.comments =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM artifact_comments WHERE workspace_id = ?1")
                .bind(workspace_id)
                .fetch_one(self.reader())
                .await?;
        Ok(counts)
    }

    pub async fn workspace_overview(&self, workspace_id: &str) -> Result<WorkspaceOverview> {
        Ok(WorkspaceOverview {
            workspace_id: workspace_id.to_owned(),
            source_count: self.count_for_workspace("sources", workspace_id).await?,
            artifact_count: self.count_for_workspace("artifacts", workspace_id).await?,
            chunk_count: self.count_for_workspace("chunks", workspace_id).await?,
            symbol_count: self.count_for_workspace("symbols", workspace_id).await?,
            memory_card_count: self.count_for_workspace("memory_cards", workspace_id).await?,
        })
    }
}
