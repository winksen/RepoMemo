//! Quarantine of a connector's copied evidence (V0.8.2). A quarantined connector keeps its rows, and every read path that serves
//! evidence leaves them out (see [`NOT_QUARANTINED`]), until an administrator releases it or removes the connector.

use anyhow::Result;
use chrono::Utc;

use super::StorageEngine;

/// The condition every query over `artifacts` adds so a quarantined connector's items are not served.
pub(crate) const NOT_QUARANTINED: &str =
    "NOT EXISTS (SELECT 1 FROM connector_quarantine WHERE connector_quarantine.source_id = artifacts.source_id)";

/// Why a connector was quarantined.
pub const QUARANTINE_REASONS: [&str; 3] = ["access_revoked", "access_denied", "not_found"];

#[derive(Debug, Clone)]
pub struct StoredQuarantine {
    pub source_id: String,
    pub workspace_id: String,
    pub reason: String,
    pub detail: String,
    pub items: i64,
    pub quarantined_at: String,
}

impl StorageEngine {
    /// Holds a connector's evidence back. A second call keeps the first time and reason: the stale-access window is counted from
    /// when it began.
    pub async fn quarantine_connector(&self, source_id: &str, reason: &str, detail: &str) -> Result<()> {
        if !QUARANTINE_REASONS.contains(&reason) {
            repomemo_domain::bail_invalid!("That is not a reason for a quarantine.");
        }
        sqlx::query(
            r#"INSERT OR IGNORE INTO connector_quarantine (source_id, workspace_id, reason, detail, items, quarantined_at)
               SELECT sources.id, sources.workspace_id, ?2, ?3,
                      (SELECT COUNT(*) FROM connector_items WHERE connector_items.source_id = sources.id), ?4
               FROM sources WHERE sources.id = ?1"#,
        )
        .bind(source_id)
        .bind(reason)
        .bind(detail.chars().take(500).collect::<String>())
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn connector_quarantine(&self, source_id: &str) -> Result<Option<StoredQuarantine>> {
        let row = sqlx::query_as::<_, (String, String, String, String, i64, String)>(
            "SELECT source_id, workspace_id, reason, detail, items, quarantined_at FROM connector_quarantine WHERE source_id = ?1",
        )
        .bind(source_id)
        .fetch_optional(self.reader())
        .await?;
        Ok(row.map(
            |(source_id, workspace_id, reason, detail, items, quarantined_at)| StoredQuarantine {
                source_id,
                workspace_id,
                reason,
                detail,
                items,
                quarantined_at,
            },
        ))
    }

    /// Lets the evidence be served again. `true` when there was a quarantine to end.
    pub async fn release_connector_quarantine(&self, source_id: &str) -> Result<bool> {
        let done = sqlx::query("DELETE FROM connector_quarantine WHERE source_id = ?1")
            .bind(source_id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Whether an artifact belongs to a quarantined connector.
    pub async fn artifact_is_quarantined(&self, artifact_id: &str) -> Result<bool> {
        let held: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM artifacts JOIN connector_quarantine ON connector_quarantine.source_id = artifacts.source_id WHERE artifacts.id = ?1",
        )
        .bind(artifact_id)
        .fetch_one(self.reader())
        .await?;
        Ok(held > 0)
    }
}
