//! Storage for connector sources (roadmap F17, V0.2.27): trackers whose issues
//! and, later, pull requests are kept as artifacts.

use anyhow::Result;
use chrono::Utc;
use repomemo_domain::{ArtifactSummary, ArtifactType, Source, SourceType};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use super::{artifact_type_to_db, source_type_to_db, ArtifactSummaryRow, StorageEngine};

/// What a sync gives for one tracker item.
#[derive(Debug, Clone)]
pub struct ConnectorItemWrite {
    /// Stable within the source, such as `issue:123`.
    pub external_id: String,
    /// `issue`, and later `pull_request`.
    pub kind: String,
    pub artifact_type: ArtifactType,
    pub title: String,
    pub path: String,
    /// The text of the item, whose blob is already stored.
    pub content_hash: String,
    pub size_bytes: i64,
    /// `open` or `closed`.
    pub state: String,
    /// The tracker's own update time.
    pub remote_updated_at: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorItemOutcome {
    Added,
    Updated,
    Unchanged,
}

impl StorageEngine {
    /// Creates a connector source. `location` names what it reads (such as
    /// `https://github.com/acme/app`) and is unique within a workspace.
    pub async fn create_connector_source(
        &self,
        workspace_id: &str,
        name: &str,
        location: &str,
        metadata: &Value,
    ) -> Result<Source> {
        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sources WHERE workspace_id = ?1 AND type = ?2 AND root_uri = ?3",
        )
        .bind(workspace_id)
        .bind(source_type_to_db(&SourceType::Connector))
        .bind(location)
        .fetch_one(self.reader())
        .await?;
        if existing > 0 {
            repomemo_domain::bail_conflict!("This tracker is already connected to the workspace.");
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO sources (id, workspace_id, type, name, root_uri, status, metadata_json, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7, ?7)
            "#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(source_type_to_db(&SourceType::Connector))
        .bind(name)
        .bind(location)
        .bind(serde_json::to_string(metadata)?)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.get_source(&id).await
    }

    /// Items kept for a source, by state: `(open, closed)`.
    pub async fn connector_item_counts(&self, source_id: &str) -> Result<(i64, i64)> {
        let row = sqlx::query(
            r#"SELECT COALESCE(SUM(CASE WHEN state = 'open' THEN 1 ELSE 0 END), 0) AS open,
                      COALESCE(SUM(CASE WHEN state = 'closed' THEN 1 ELSE 0 END), 0) AS closed
               FROM connector_items WHERE source_id = ?1"#,
        )
        .bind(source_id)
        .fetch_one(self.reader())
        .await?;
        Ok((row.get("open"), row.get("closed")))
    }

    /// Stores one tracker item as an artifact: new, changed (queued for
    /// re-indexing, flagged for review when it was verified) or unchanged. A
    /// closed item becomes *outdated* and a reopened one active again.
    pub async fn write_connector_item(
        &self,
        workspace_id: &str,
        source_id: &str,
        item: &ConnectorItemWrite,
    ) -> Result<(String, ConnectorItemOutcome)> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        let existing = sqlx::query_as::<_, (String, String, String)>(
            "SELECT artifact_id, content_hash, state FROM connector_items WHERE source_id = ?1 AND external_id = ?2",
        )
        .bind(source_id)
        .bind(&item.external_id)
        .fetch_optional(&mut *tx)
        .await?;
        let closed_note = "Closed in the tracker.";
        let (artifact_id, outcome) = match existing {
            None => {
                let id = Uuid::new_v4().to_string();
                sqlx::query(
                    r#"
                    INSERT INTO artifacts (
                      id, workspace_id, source_id, type, title, path, content_hash, mime_type,
                      language, size_bytes, created_at, updated_at, metadata_json
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'text/markdown', 'markdown', ?8, ?9, ?9, ?10)
                    "#,
                )
                .bind(&id)
                .bind(workspace_id)
                .bind(source_id)
                .bind(artifact_type_to_db(&item.artifact_type))
                .bind(&item.title)
                .bind(&item.path)
                .bind(&item.content_hash)
                .bind(item.size_bytes)
                .bind(&now)
                .bind(serde_json::to_string(&item.metadata)?)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    r#"INSERT INTO connector_items
                       (source_id, external_id, artifact_id, kind, state, content_hash, remote_updated_at, updated_at)
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
                )
                .bind(source_id)
                .bind(&item.external_id)
                .bind(&id)
                .bind(&item.kind)
                .bind(&item.state)
                .bind(&item.content_hash)
                .bind(&item.remote_updated_at)
                .bind(&now)
                .execute(&mut *tx)
                .await?;
                if item.state == "closed" {
                    super::repo::set_system_lifecycle(
                        &mut tx,
                        &id,
                        workspace_id,
                        &["active"],
                        "outdated",
                        closed_note,
                        "connector_item_closed",
                        &now,
                    )
                    .await?;
                }
                (id, ConnectorItemOutcome::Added)
            }
            Some((artifact_id, previous_hash, previous_state)) => {
                let content_changed = previous_hash != item.content_hash;
                let state_changed = previous_state != item.state;
                if !content_changed && !state_changed {
                    sqlx::query(
                        "UPDATE connector_items SET remote_updated_at = ?3, updated_at = ?4 WHERE source_id = ?1 AND external_id = ?2",
                    )
                    .bind(source_id)
                    .bind(&item.external_id)
                    .bind(&item.remote_updated_at)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                    tx.commit().await?;
                    return Ok((artifact_id, ConnectorItemOutcome::Unchanged));
                }
                sqlx::query(
                    r#"
                    UPDATE artifacts
                    SET title = ?2, path = ?3, content_hash = ?4, size_bytes = ?5, metadata_json = ?6,
                        updated_at = ?7, indexed_at = CASE WHEN ?8 THEN NULL ELSE indexed_at END
                    WHERE id = ?1
                    "#,
                )
                .bind(&artifact_id)
                .bind(&item.title)
                .bind(&item.path)
                .bind(&item.content_hash)
                .bind(item.size_bytes)
                .bind(serde_json::to_string(&item.metadata)?)
                .bind(&now)
                .bind(content_changed)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    r#"UPDATE connector_items SET state = ?3, content_hash = ?4, remote_updated_at = ?5, updated_at = ?6
                       WHERE source_id = ?1 AND external_id = ?2"#,
                )
                .bind(source_id)
                .bind(&item.external_id)
                .bind(&item.state)
                .bind(&item.content_hash)
                .bind(&item.remote_updated_at)
                .bind(&now)
                .execute(&mut *tx)
                .await?;
                if item.state == "closed" && state_changed {
                    super::repo::set_system_lifecycle(
                        &mut tx,
                        &artifact_id,
                        workspace_id,
                        &["active", "verified", "needs_review"],
                        "outdated",
                        closed_note,
                        "connector_item_closed",
                        &now,
                    )
                    .await?;
                } else if item.state == "open" && state_changed {
                    super::repo::set_system_lifecycle(
                        &mut tx,
                        &artifact_id,
                        workspace_id,
                        &["outdated"],
                        "active",
                        "Reopened in the tracker.",
                        "connector_item_reopened",
                        &now,
                    )
                    .await?;
                } else if content_changed {
                    super::repo::set_system_lifecycle(
                        &mut tx,
                        &artifact_id,
                        workspace_id,
                        &["verified"],
                        "needs_review",
                        "Changed in the tracker; it was verified before the change.",
                        "connector_item_changed",
                        &now,
                    )
                    .await?;
                }
                (artifact_id, ConnectorItemOutcome::Updated)
            }
        };
        tx.commit().await?;
        Ok((artifact_id, outcome))
    }

    /// Artifacts of a connector source that still need indexing.
    pub async fn list_connector_artifacts_needing_index(
        &self,
        source_id: &str,
        current_index_version: i64,
    ) -> Result<Vec<ArtifactSummary>> {
        let rows = sqlx::query_as::<_, ArtifactSummaryRow>(
            r#"
            SELECT
              artifacts.id, artifacts.workspace_id, artifacts.source_id, sources.name AS source_name,
              artifacts.type AS artifact_type, artifacts.title, artifacts.path, artifacts.content_hash,
              artifacts.mime_type, artifacts.language, artifacts.size_bytes, artifacts.created_at,
              artifacts.updated_at, artifacts.indexed_at
            FROM connector_items
            JOIN artifacts ON artifacts.id = connector_items.artifact_id
            JOIN sources ON sources.id = artifacts.source_id
            WHERE connector_items.source_id = ?1
              AND (artifacts.indexed_at IS NULL OR artifacts.index_version < ?2)
            ORDER BY artifacts.path ASC
            "#,
        )
        .bind(source_id)
        .bind(current_index_version)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.into_iter().map(ArtifactSummary::from).collect())
    }
}
