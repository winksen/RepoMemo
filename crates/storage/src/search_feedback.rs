//! Search feedback (V0.6.25): a question the evidence could not answer, or a result that did not help, kept as a request the
//! workspace's owners can resolve with evidence or a memory card. Retention is stated here and applied on every read and
//! write: an open request lasts at most `OPEN_RETENTION_DAYS`, a resolved or declined one `CLOSED_RETENTION_DAYS`.

use super::*;

/// An open request is removed after this long without an answer.
pub const OPEN_RETENTION_DAYS: i64 = 180;
/// A resolved or declined request, and the query text in it, is removed this long after it was closed.
pub const CLOSED_RETENTION_DAYS: i64 = 30;
/// The most open requests one person can have in a workspace.
pub const MAX_OPEN_PER_USER: i64 = 25;

#[derive(Debug, Clone)]
pub struct StoredSearchFeedback {
    pub id: String,
    pub workspace_id: String,
    pub user: Option<SharedUser>,
    pub kind: String,
    pub query: String,
    pub artifact_id: Option<String>,
    pub artifact_title: Option<String>,
    pub note: String,
    pub status: String,
    pub resolution_kind: Option<String>,
    pub resolution_id: Option<String>,
    pub resolution_title: Option<String>,
    pub resolved_by: Option<SharedUser>,
    pub resolved_note: String,
    pub resolved_at: Option<String>,
    pub created_at: String,
}

const SELECT_FEEDBACK: &str = r#"
    SELECT feedback.id, feedback.workspace_id, feedback.kind, feedback.query, feedback.artifact_id, feedback.note,
           feedback.status, feedback.resolution_kind, feedback.resolution_id, feedback.resolved_note,
           feedback.resolved_at, feedback.created_at,
           asker.id AS asker_id, asker.display_name AS asker_name, asker.email AS asker_email,
           resolver.id AS resolver_id, resolver.display_name AS resolver_name, resolver.email AS resolver_email,
           artifact.title AS artifact_title,
           COALESCE(
             (SELECT title FROM artifacts WHERE artifacts.id = feedback.resolution_id AND feedback.resolution_kind = 'evidence'),
             (SELECT title FROM memory_cards WHERE memory_cards.id = feedback.resolution_id AND feedback.resolution_kind = 'memory_card')
           ) AS resolution_title
    FROM search_feedback AS feedback
    LEFT JOIN users AS asker ON asker.id = feedback.user_id
    LEFT JOIN users AS resolver ON resolver.id = feedback.resolved_by_user_id
    LEFT JOIN artifacts AS artifact ON artifact.id = feedback.artifact_id"#;

fn person(row: &SqliteRow, prefix: &str) -> Option<SharedUser> {
    let id: Option<String> = row.get(format!("{prefix}_id").as_str());
    id.map(|id| SharedUser {
        id,
        display_name: row.get(format!("{prefix}_name").as_str()),
        email: row.get(format!("{prefix}_email").as_str()),
    })
}

fn stored(row: &SqliteRow) -> StoredSearchFeedback {
    StoredSearchFeedback {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        user: person(row, "asker"),
        kind: row.get("kind"),
        query: row.get("query"),
        artifact_id: row.get("artifact_id"),
        artifact_title: row.get("artifact_title"),
        note: row.get("note"),
        status: row.get("status"),
        resolution_kind: row.get("resolution_kind"),
        resolution_id: row.get("resolution_id"),
        resolution_title: row.get("resolution_title"),
        resolved_by: person(row, "resolver"),
        resolved_note: row.get("resolved_note"),
        resolved_at: row.get("resolved_at"),
        created_at: row.get("created_at"),
    }
}

impl StorageEngine {
    /// Removes what has outlived its retention, in one workspace.
    async fn purge_search_feedback(&self, workspace_id: &str) -> Result<()> {
        let now = Utc::now();
        sqlx::query(
            r#"DELETE FROM search_feedback WHERE workspace_id = ?1 AND (
                 (status = 'open' AND created_at < ?2) OR (status <> 'open' AND COALESCE(resolved_at, created_at) < ?3))"#,
        )
        .bind(workspace_id)
        .bind((now - chrono::Duration::days(OPEN_RETENTION_DAYS)).to_rfc3339())
        .bind((now - chrono::Duration::days(CLOSED_RETENTION_DAYS)).to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Records a request. A result that did not help must be a file of this workspace.
    pub async fn submit_search_feedback(
        &self,
        workspace_id: &str,
        user_id: &str,
        kind: &str,
        query: &str,
        artifact_id: Option<&str>,
        note: &str,
    ) -> Result<StoredSearchFeedback> {
        self.purge_search_feedback(workspace_id).await?;
        if let Some(artifact_id) = artifact_id {
            let in_workspace =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM artifacts WHERE id = ?1 AND workspace_id = ?2")
                    .bind(artifact_id)
                    .bind(workspace_id)
                    .fetch_one(self.reader())
                    .await?
                    > 0;
            if !in_workspace {
                repomemo_domain::bail_not_found!("That result was not found in this workspace.");
            }
        }
        let open = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM search_feedback WHERE workspace_id = ?1 AND user_id = ?2 AND status = 'open'",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_one(self.reader())
        .await?;
        if open >= MAX_OPEN_PER_USER {
            repomemo_domain::bail_conflict!(
                "You already have {MAX_OPEN_PER_USER} open requests in this workspace. Wait for some to be answered."
            );
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"INSERT INTO search_feedback (id, workspace_id, user_id, kind, query, artifact_id, note, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(user_id)
        .bind(kind)
        .bind(query)
        .bind(artifact_id)
        .bind(note)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        self.search_feedback(workspace_id, &id).await
    }

    pub async fn search_feedback(&self, workspace_id: &str, id: &str) -> Result<StoredSearchFeedback> {
        sqlx::query(&format!(
            "{SELECT_FEEDBACK} WHERE feedback.id = ?1 AND feedback.workspace_id = ?2"
        ))
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(self.reader())
        .await?
        .as_ref()
        .map(stored)
        .ok_or_else(|| repomemo_domain::not_found!("That request was not found."))
    }

    /// The requests of a workspace, newest first: all of them (`only_user` none, for administrators) or one person's own.
    pub async fn list_search_feedback(
        &self,
        workspace_id: &str,
        only_user: Option<&str>,
        status: Option<&str>,
    ) -> Result<Vec<StoredSearchFeedback>> {
        self.purge_search_feedback(workspace_id).await?;
        let rows = sqlx::query(&format!(
            r#"{SELECT_FEEDBACK}
               WHERE feedback.workspace_id = ?1 AND (?2 IS NULL OR feedback.user_id = ?2) AND (?3 IS NULL OR feedback.status = ?3)
               ORDER BY feedback.created_at DESC LIMIT 200"#
        ))
        .bind(workspace_id)
        .bind(only_user)
        .bind(status)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(stored).collect())
    }

    /// Closes a request: resolved with a file or a memory card of this workspace, or declined. An already closed request
    /// is a conflict, so two administrators cannot both answer it.
    pub async fn resolve_search_feedback(
        &self,
        workspace_id: &str,
        id: &str,
        resolver_id: &str,
        resolution_kind: Option<&str>,
        target_id: Option<&str>,
        note: &str,
    ) -> Result<StoredSearchFeedback> {
        let mut tx = self.pool.begin().await?;
        if let (Some(kind), Some(target)) = (resolution_kind, target_id) {
            let table = if kind == "memory_card" {
                "memory_cards"
            } else {
                "artifacts"
            };
            let found = sqlx::query_scalar::<_, i64>(&format!(
                "SELECT COUNT(*) FROM {table} WHERE id = ?1 AND workspace_id = ?2"
            ))
            .bind(target)
            .bind(workspace_id)
            .fetch_one(&mut *tx)
            .await?
                > 0;
            if !found {
                repomemo_domain::bail_not_found!("That evidence or memory card was not found in this workspace.");
            }
        }
        let changed = sqlx::query(
            r#"UPDATE search_feedback
               SET status = ?3, resolution_kind = ?4, resolution_id = ?5, resolved_by_user_id = ?6, resolved_note = ?7, resolved_at = ?8
               WHERE id = ?1 AND workspace_id = ?2 AND status = 'open'"#,
        )
        .bind(id)
        .bind(workspace_id)
        .bind(if resolution_kind.is_some() { "resolved" } else { "declined" })
        .bind(resolution_kind)
        .bind(target_id)
        .bind(resolver_id)
        .bind(note)
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if changed == 0 {
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM search_feedback WHERE id = ?1 AND workspace_id = ?2",
            )
            .bind(id)
            .bind(workspace_id)
            .fetch_one(&mut *tx)
            .await?
                > 0;
            if !exists {
                repomemo_domain::bail_not_found!("That request was not found.");
            }
            repomemo_domain::bail_conflict!("That request was already answered.");
        }
        tx.commit().await?;
        self.search_feedback(workspace_id, id).await
    }
}
