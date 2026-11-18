//! Stored AI usage and the workspace's own limit (V0.8.13).

use super::*;

/// What a request to spend calls was told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiSpend {
    pub allowed: bool,
    /// The limit that applied (0 is none) and where it came from.
    pub limit: u32,
    pub limit_source: &'static str,
    /// Calls made today across all kinds, including this one when it was allowed.
    pub spent_today: i64,
}

/// One kind of call on one day.
#[derive(Debug, Clone)]
pub struct AiUsageRow {
    pub day: String,
    pub kind: String,
    pub calls: i64,
    pub refused: i64,
}

fn today() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

impl StorageEngine {
    /// The limit a workspace set for itself, if it did (0 is no limit).
    pub async fn workspace_ai_limit(&self, workspace_id: &str) -> Result<Option<u32>> {
        let limit: Option<i64> =
            sqlx::query_scalar("SELECT daily_calls FROM workspace_ai_limits WHERE workspace_id = ?1")
                .bind(workspace_id)
                .fetch_optional(self.reader())
                .await?;
        Ok(limit.map(|value| value.clamp(0, i64::from(u32::MAX)) as u32))
    }

    /// Sets the workspace's own daily limit, or clears it (the server's default applies again).
    pub async fn set_workspace_ai_limit(
        &self,
        workspace_id: &str,
        limit: Option<u32>,
        user_id: Option<&str>,
    ) -> Result<()> {
        match limit {
            Some(limit) => {
                sqlx::query(
                    r#"INSERT INTO workspace_ai_limits (workspace_id, daily_calls, updated_by_user_id, updated_at)
                       VALUES (?1, ?2, ?3, ?4)
                       ON CONFLICT(workspace_id) DO UPDATE SET daily_calls = excluded.daily_calls,
                         updated_by_user_id = excluded.updated_by_user_id, updated_at = excluded.updated_at"#,
                )
                .bind(workspace_id)
                .bind(i64::from(limit))
                .bind(user_id)
                .bind(Utc::now().to_rfc3339())
                .execute(&self.pool)
                .await?;
            }
            None => {
                sqlx::query("DELETE FROM workspace_ai_limits WHERE workspace_id = ?1")
                    .bind(workspace_id)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }

    /// Spends `calls` of today's allowance for a kind of call, under the workspace's limit or the server's default
    /// (`default_limit`, 0 is none). Nothing is spent, and the refusal is counted, when it would go over. The count is stored,
    /// so it is the same after a restart.
    pub async fn spend_ai_calls(
        &self,
        workspace_id: &str,
        kind: &str,
        calls: u32,
        default_limit: u32,
    ) -> Result<AiSpend> {
        let (limit, limit_source) = match self.workspace_ai_limit(workspace_id).await? {
            Some(limit) => (limit, "workspace"),
            None if default_limit > 0 => (default_limit, "server"),
            None => (0, "none"),
        };
        let day = today();
        let mut tx = self.pool.begin().await?;
        let spent: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(calls), 0) FROM ai_usage_daily WHERE workspace_id = ?1 AND day = ?2",
        )
        .bind(workspace_id)
        .bind(&day)
        .fetch_one(&mut *tx)
        .await?;
        let allowed = limit == 0 || spent + i64::from(calls) <= i64::from(limit);
        sqlx::query(
            r#"INSERT INTO ai_usage_daily (workspace_id, day, kind, calls, refused) VALUES (?1, ?2, ?3, ?4, ?5)
               ON CONFLICT(workspace_id, day, kind) DO UPDATE SET calls = calls + excluded.calls, refused = refused + excluded.refused"#,
        )
        .bind(workspace_id)
        .bind(&day)
        .bind(kind)
        .bind(if allowed { i64::from(calls) } else { 0 })
        .bind(if allowed { 0 } else { i64::from(calls) })
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(AiSpend {
            allowed,
            limit,
            limit_source,
            spent_today: if allowed { spent + i64::from(calls) } else { spent },
        })
    }

    /// The usage of the last `days` days, newest first.
    pub async fn ai_usage_days(&self, workspace_id: &str, days: i64) -> Result<Vec<AiUsageRow>> {
        let since = (Utc::now() - chrono::Duration::days(days.clamp(1, 90) - 1))
            .format("%Y-%m-%d")
            .to_string();
        Ok(sqlx::query_as::<_, (String, String, i64, i64)>(
            "SELECT day, kind, calls, refused FROM ai_usage_daily WHERE workspace_id = ?1 AND day >= ?2 ORDER BY day DESC, kind ASC",
        )
        .bind(workspace_id)
        .bind(since)
        .fetch_all(self.reader())
        .await?
        .into_iter()
        .map(|(day, kind, calls, refused)| AiUsageRow {
            day,
            kind,
            calls,
            refused,
        })
        .collect())
    }

    /// The counts a workspace holds that AI work is made from: files, passages and stored vectors.
    pub async fn ai_stored_counts(&self, workspace_id: &str) -> Result<(i64, i64, i64)> {
        let artifacts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artifacts WHERE workspace_id = ?1")
            .bind(workspace_id)
            .fetch_one(self.reader())
            .await?;
        let chunks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chunks WHERE workspace_id = ?1")
            .bind(workspace_id)
            .fetch_one(self.reader())
            .await?;
        Ok((artifacts, chunks, self.embedding_count(workspace_id).await?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    async fn engine() -> (StorageEngine, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "repomemo-ai-usage-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        (StorageEngine::open(StorageConfig::new(dir.clone())).await.unwrap(), dir)
    }

    #[tokio::test]
    async fn a_limit_stops_calls_the_same_way_after_a_restart() {
        let (storage, dir) = engine().await;
        let workspace = storage.create_workspace("AI usage").await.unwrap();
        // The server's default applies until the workspace sets its own.
        let first = storage.spend_ai_calls(&workspace.id, "embedding", 2, 3).await.unwrap();
        assert!(first.allowed && first.limit == 3 && first.limit_source == "server" && first.spent_today == 2);
        assert!(
            !storage
                .spend_ai_calls(&workspace.id, "vision", 2, 3)
                .await
                .unwrap()
                .allowed
        );
        assert!(
            storage
                .spend_ai_calls(&workspace.id, "vision", 1, 3)
                .await
                .unwrap()
                .allowed
        );

        // The counts are stored: a new engine on the same data sees them and refuses the same way.
        drop(storage);
        let storage = StorageEngine::open(StorageConfig::new(dir.clone())).await.unwrap();
        let after = storage.spend_ai_calls(&workspace.id, "embedding", 1, 3).await.unwrap();
        assert!(!after.allowed, "the day's calls were not forgotten");
        assert_eq!(after.spent_today, 3);

        // The workspace's own limit replaces the default; 0 is no limit; clearing returns to the default.
        storage
            .set_workspace_ai_limit(&workspace.id, Some(10), None)
            .await
            .unwrap();
        let own = storage.spend_ai_calls(&workspace.id, "embedding", 1, 3).await.unwrap();
        assert!(own.allowed && own.limit_source == "workspace" && own.limit == 10);
        storage
            .set_workspace_ai_limit(&workspace.id, Some(0), None)
            .await
            .unwrap();
        assert!(
            storage
                .spend_ai_calls(&workspace.id, "embedding", 50, 3)
                .await
                .unwrap()
                .allowed
        );
        storage.set_workspace_ai_limit(&workspace.id, None, None).await.unwrap();
        assert_eq!(storage.workspace_ai_limit(&workspace.id).await.unwrap(), None);

        // The refusals are counted, by kind, next to the calls.
        let rows = storage.ai_usage_days(&workspace.id, 7).await.unwrap();
        let vision = rows.iter().find(|row| row.kind == "vision").unwrap();
        assert_eq!((vision.calls, vision.refused), (1, 2));
        let _ = std::fs::remove_dir_all(dir);
    }
}
