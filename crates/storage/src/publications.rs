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
}
