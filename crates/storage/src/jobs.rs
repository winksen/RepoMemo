use super::*;

impl StorageEngine {
    pub async fn record_health_action(
        &self,
        workspace_id: &str,
        fingerprint: &str,
        detector: &str,
        action: &str,
        actor_user_id: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO workspace_health_actions
              (id, workspace_id, fingerprint, detector, action, actor_user_id, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(workspace_id)
        .bind(fingerprint)
        .bind(detector)
        .bind(action)
        .bind(actor_user_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// `(fingerprint, detector, action)` for every recorded health action.
    pub async fn list_health_actions(&self, workspace_id: &str) -> Result<Vec<(String, String, String)>> {
        let rows =
            sqlx::query("SELECT fingerprint, detector, action FROM workspace_health_actions WHERE workspace_id = ?1")
                .bind(workspace_id)
                .fetch_all(self.reader())
                .await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.get("fingerprint"), row.get("detector"), row.get("action")))
            .collect())
    }

    pub async fn app_ai_status(&self) -> Result<(bool, Option<String>)> {
        let row = sqlx::query("SELECT name FROM provider_settings WHERE enabled = 1 ORDER BY updated_at DESC LIMIT 1")
            .fetch_optional(self.reader())
            .await?;
        Ok(match row {
            Some(row) => (true, Some(row.get("name"))),
            None => (false, None),
        })
    }

    pub async fn create_indexing_job(
        &self,
        workspace_id: &str,
        source_id: Option<&str>,
        stage: &str,
        progress_total: Option<i64>,
    ) -> Result<IndexingJobStatus> {
        self.create_job(workspace_id, source_id, "indexing", stage, progress_total)
            .await
    }

    pub async fn create_job(
        &self,
        workspace_id: &str,
        source_id: Option<&str>,
        kind: &str,
        stage: &str,
        progress_total: Option<i64>,
    ) -> Result<IndexingJobStatus> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO indexing_jobs (
              id,
              workspace_id,
              source_id,
              kind,
              status,
              stage,
              progress_current,
              progress_total,
              cancel_requested,
              created_at,
              updated_at
            )
            VALUES (?1, ?2, ?3, ?4, 'running', ?5, 0, ?6, 0, ?7, ?8)
            "#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(source_id)
        .bind(kind)
        .bind(stage)
        .bind(progress_total)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        let job = self.get_indexing_job(&id).await?;
        self.emit_job(&job);
        Ok(job)
    }

    pub async fn update_indexing_job(
        &self,
        job_id: &str,
        status: &str,
        stage: &str,
        progress_current: i64,
        error_message: Option<&str>,
    ) -> Result<IndexingJobStatus> {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            UPDATE indexing_jobs
            SET status = ?1,
                stage = ?2,
                progress_current = ?3,
                error_message = ?4,
                updated_at = ?5
            WHERE id = ?6
            "#,
        )
        .bind(status)
        .bind(stage)
        .bind(progress_current)
        .bind(error_message)
        .bind(&now)
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        let job = self.get_indexing_job(job_id).await?;
        self.emit_job(&job);
        Ok(job)
    }

    /// Mark a job as cancellation-requested. Long-running loops poll
    /// `is_job_cancel_requested` between units of work and stop cooperatively.
    /// Returns the refreshed job. Emits a job event so SSE subscribers see
    /// the flag flip immediately.
    pub async fn request_job_cancel(&self, job_id: &str) -> Result<IndexingJobStatus> {
        let now = Utc::now().to_rfc3339();
        let affected = sqlx::query(
            r#"
            UPDATE indexing_jobs
            SET cancel_requested = 1,
                updated_at = ?1
            WHERE id = ?2
              AND status IN ('running', 'pending')
            "#,
        )
        .bind(&now)
        .bind(job_id)
        .execute(&self.pool)
        .await?
        .rows_affected();

        let job = self.get_indexing_job(job_id).await?;
        if affected > 0 {
            self.emit_job(&job);
        }
        Ok(job)
    }

    /// Marks every job a stopped process left `running` or `pending` as
    /// failed, whatever its kind, so no job shows progress forever after a
    /// crash or restart. Call once at startup, before new work begins.
    pub async fn fail_all_interrupted_jobs(&self, message: &str) -> Result<u64> {
        Ok(sqlx::query(
            r#"
            UPDATE indexing_jobs
            SET status = 'failed', stage = 'failed', error_message = ?1, updated_at = ?2
            WHERE status IN ('running', 'pending')
            "#,
        )
        .bind(message)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?
        .rows_affected())
    }

    /// Deletes finished jobs (completed, failed or cancelled) last updated
    /// before `cutoff`.
    pub async fn prune_finished_jobs(&self, cutoff: &str) -> Result<u64> {
        Ok(sqlx::query(
            "DELETE FROM indexing_jobs WHERE status IN ('completed', 'failed', 'cancelled') AND updated_at < ?1",
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await?
        .rows_affected())
    }

    pub async fn is_job_cancel_requested(&self, job_id: &str) -> Result<bool> {
        let value = sqlx::query_scalar::<_, i64>("SELECT cancel_requested FROM indexing_jobs WHERE id = ?1")
            .bind(job_id)
            .fetch_optional(self.reader())
            .await?;
        Ok(value.unwrap_or(0) != 0)
    }

    pub async fn get_job(&self, job_id: &str) -> Result<Option<IndexingJobStatus>> {
        let row = sqlx::query_as::<_, IndexingJobRow>(
            r#"
            SELECT
              id,
              workspace_id,
              source_id,
              kind,
              status,
              stage,
              progress_current,
              progress_total,
              error_message,
              cancel_requested,
              created_at,
              updated_at
            FROM indexing_jobs
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .fetch_optional(self.reader())
        .await?;
        Ok(row.map(IndexingJobStatus::from))
    }

    /// Lists jobs for a workspace, newest first. Optional filters narrow by
    /// job kind or status; pass `None`/empty to skip a filter. `limit` is
    /// clamped to a sensible upper bound.
    pub async fn list_workspace_jobs(
        &self,
        workspace_id: &str,
        kind: Option<&str>,
        status: Option<&str>,
        limit: i64,
    ) -> Result<Vec<IndexingJobStatus>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
              id,
              workspace_id,
              source_id,
              kind,
              status,
              stage,
              progress_current,
              progress_total,
              error_message,
              cancel_requested,
              created_at,
              updated_at
            FROM indexing_jobs
            WHERE workspace_id =
            "#,
        );
        builder.push_bind(workspace_id.to_owned());
        if let Some(kind) = kind {
            builder.push(" AND kind = ").push_bind(kind.to_owned());
        }
        if let Some(status) = status {
            builder.push(" AND status = ").push_bind(status.to_owned());
        }
        builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        builder.push_bind(limit.clamp(1, 200));

        let rows = builder
            .build_query_as::<IndexingJobRow>()
            .fetch_all(self.reader())
            .await?;
        Ok(rows.into_iter().map(IndexingJobStatus::from).collect())
    }

    pub(crate) async fn get_indexing_job(&self, job_id: &str) -> Result<IndexingJobStatus> {
        let row = sqlx::query_as::<_, IndexingJobRow>(
            r#"
            SELECT
              id,
              workspace_id,
              source_id,
              kind,
              status,
              stage,
              progress_current,
              progress_total,
              error_message,
              cancel_requested,
              created_at,
              updated_at
            FROM indexing_jobs
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .fetch_one(self.reader())
        .await?;

        Ok(IndexingJobStatus::from(row))
    }
}
