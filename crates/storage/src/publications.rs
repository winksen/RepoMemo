//! Public snapshots (V0.8.16): the record of what was published.

use super::*;
use repomemo_domain::Publication;

type Row = (
    String,
    String,
    String,
    String,
    String,
    String,
    i64,
    Option<String>,
    String,
    String,
);

const COLUMNS: &str =
    "id, workspace_id, slug, title, snapshot_hash, status, version, source_artifact_id, published_at, updated_at";

fn publication_of(row: Row) -> Publication {
    Publication {
        id: row.0,
        workspace_id: row.1,
        slug: row.2,
        title: row.3,
        snapshot_hash: row.4,
        status: row.5,
        version: row.6,
        source_artifact_id: row.7,
        published_at: row.8,
        updated_at: row.9,
    }
}

impl StorageEngine {
    /// Records a snapshot. The text is stored as given: the caller made it from what the publisher previewed.
    pub async fn create_publication(
        &self,
        workspace_id: &str,
        title: &str,
        body_markdown: &str,
        snapshot_hash: &str,
        source_artifact_id: &str,
        user_id: &str,
    ) -> Result<Publication> {
        let id = Uuid::new_v4().to_string();
        // An address nobody can guess and that says nothing about the workspace or the file.
        let slug = Uuid::new_v4().simple().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"INSERT INTO publications (id, workspace_id, slug, title, body_markdown, snapshot_hash, status, version,
                 source_artifact_id, published_by_user_id, published_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'published', 1, ?7, ?8, ?9, ?9)"#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(&slug)
        .bind(title)
        .bind(body_markdown)
        .bind(snapshot_hash)
        .bind(source_artifact_id)
        .bind(user_id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.publication(&id).await
    }

    pub async fn publication(&self, id: &str) -> Result<Publication> {
        let row = sqlx::query_as::<_, Row>(&format!("SELECT {COLUMNS} FROM publications WHERE id = ?1"))
            .bind(id)
            .fetch_optional(self.reader())
            .await?;
        match row {
            Some(row) => Ok(publication_of(row)),
            None => repomemo_domain::bail_not_found!("Publication was not found."),
        }
    }

    pub async fn list_publications(&self, workspace_id: &str) -> Result<Vec<Publication>> {
        Ok(sqlx::query_as::<_, Row>(&format!(
            "SELECT {COLUMNS} FROM publications WHERE workspace_id = ?1 ORDER BY published_at DESC"
        ))
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?
        .into_iter()
        .map(publication_of)
        .collect())
    }

    /// The published snapshot of a file, if one is live.
    pub async fn live_publication_of(&self, artifact_id: &str) -> Result<Option<Publication>> {
        let row = sqlx::query_as::<_, Row>(&format!(
            "SELECT {COLUMNS} FROM publications WHERE source_artifact_id = ?1 AND status = 'published'"
        ))
        .bind(artifact_id)
        .fetch_optional(self.reader())
        .await?;
        Ok(row.map(publication_of))
    }

    /// The text a snapshot holds, for the publisher's preview of what readers see (and, later, for the public routes).
    pub async fn publication_body(&self, id: &str) -> Result<String> {
        match sqlx::query_scalar::<_, String>("SELECT body_markdown FROM publications WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.reader())
            .await?
        {
            Some(body) => Ok(body),
            None => repomemo_domain::bail_not_found!("Publication was not found."),
        }
    }

    /// Withdraws or marks stale. The version grows, so a cache holding an older one revalidates and finds it gone.
    pub async fn set_publication_status(&self, id: &str, status: &str) -> Result<Publication> {
        if !matches!(status, "published" | "withdrawn" | "stale") {
            repomemo_domain::bail_invalid!("That is not a publication status.");
        }
        self.publication(id).await?;
        sqlx::query("UPDATE publications SET status = ?2, version = version + 1, updated_at = ?3 WHERE id = ?1")
            .bind(id)
            .bind(status)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        self.publication(id).await
    }

    /// Replaces the snapshot with a newly reviewed one and makes it live again.
    pub async fn replace_publication_snapshot(
        &self,
        id: &str,
        body_markdown: &str,
        snapshot_hash: &str,
    ) -> Result<Publication> {
        self.publication(id).await?;
        sqlx::query(
            "UPDATE publications SET body_markdown = ?2, snapshot_hash = ?3, status = 'published', version = version + 1, updated_at = ?4 WHERE id = ?1",
        )
        .bind(id)
        .bind(body_markdown)
        .bind(snapshot_hash)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        self.publication(id).await
    }

    /// Stops serving every live snapshot made from files of a source (its access was lost). They stay recorded as stale.
    pub async fn mark_source_publications_stale(&self, source_id: &str) -> Result<u64> {
        let done = sqlx::query(
            "UPDATE publications SET status = 'stale', version = version + 1, updated_at = ?2 WHERE status = 'published' AND source_artifact_id IN (SELECT id FROM artifacts WHERE source_id = ?1)",
        )
        .bind(source_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected())
    }
}
