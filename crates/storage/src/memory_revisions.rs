//! Attributed revisions of a memory card (V0.6.3): written with every change to the card, never edited, restorable as a
//! new revision.

use super::*;
use serde::{Deserialize, Serialize};

/// One cited passage or file as a revision kept it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionCitation {
    pub target_type: String,
    pub target_id: String,
    pub confidence: Option<f64>,
    pub metadata: String,
    pub title: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct StoredMemoryRevision {
    pub id: String,
    pub card_id: String,
    pub workspace_id: String,
    pub revision: i64,
    pub title: String,
    pub body_markdown: String,
    pub source: String,
    pub confidence: Option<f64>,
    pub citations: Vec<RevisionCitation>,
    pub author: Option<SharedUser>,
    pub created_at: String,
    pub restored_from: Option<i64>,
}

/// The result of a restore: the card, the new revision's number and the citations that could not come back because
/// the cited passage or file is gone.
#[derive(Debug, Clone)]
pub struct MemoryRestore {
    pub card: MemoryCard,
    pub revision: i64,
    pub skipped: Vec<RevisionCitation>,
}

/// Writes the card as it is now as the next revision. Called inside the transaction that changed the card, so a
/// revision and the state it describes cannot disagree.
pub(crate) async fn snapshot_memory_card(
    connection: &mut sqlx::SqliteConnection,
    card_id: &str,
    restored_from: Option<i64>,
) -> Result<i64> {
    let row =
        sqlx::query("SELECT workspace_id, title, body_markdown, source, confidence FROM memory_cards WHERE id = ?1")
            .bind(card_id)
            .fetch_one(&mut *connection)
            .await?;
    let links = sqlx::query(
        r#"
        SELECT link.to_type, link.to_id, link.confidence, link.metadata_json,
               COALESCE(artifact.title, '') AS title, COALESCE(artifact.path, '') AS path
        FROM links AS link
        LEFT JOIN chunks AS chunk ON link.to_type = 'chunk' AND chunk.id = link.to_id
        LEFT JOIN artifacts AS artifact ON artifact.id = CASE WHEN link.to_type = 'artifact' THEN link.to_id ELSE chunk.artifact_id END
        WHERE link.from_type = 'memory_card' AND link.from_id = ?1 AND link.relation_type = 'cites'
        ORDER BY link.created_at ASC, link.id ASC
        "#,
    )
    .bind(card_id)
    .fetch_all(&mut *connection)
    .await?;
    let citations: Vec<RevisionCitation> = links
        .iter()
        .map(|link| RevisionCitation {
            target_type: link.get("to_type"),
            target_id: link.get("to_id"),
            confidence: link.get("confidence"),
            metadata: link.get("metadata_json"),
            title: link.get("title"),
            path: link.get("path"),
        })
        .collect();
    let next = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(revision), 0) + 1 FROM memory_card_revisions WHERE card_id = ?1",
    )
    .bind(card_id)
    .fetch_one(&mut *connection)
    .await?;
    sqlx::query(
        r#"INSERT INTO memory_card_revisions
           (id, card_id, workspace_id, revision, title, body_markdown, source, confidence, citations_json, created_by_user_id, created_at, restored_from)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, ?11)"#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(card_id)
    .bind(row.get::<String, _>("workspace_id"))
    .bind(next)
    .bind(row.get::<String, _>("title"))
    .bind(row.get::<String, _>("body_markdown"))
    .bind(row.get::<String, _>("source"))
    .bind(row.get::<Option<f64>, _>("confidence"))
    .bind(serde_json::to_string(&citations)?)
    .bind(Utc::now().to_rfc3339())
    .bind(restored_from)
    .execute(&mut *connection)
    .await?;
    Ok(next)
}

impl StorageEngine {
    /// Names the person who made the card's newest revision, when it has no author yet. Storage writes the revision
    /// with the card; the request that caused it knows who asked.
    pub async fn attribute_memory_revision(&self, card_id: &str, user_id: &str) -> Result<()> {
        sqlx::query(
            r#"UPDATE memory_card_revisions SET created_by_user_id = ?2
               WHERE card_id = ?1 AND created_by_user_id IS NULL
                 AND revision = (SELECT MAX(revision) FROM memory_card_revisions WHERE card_id = ?1)"#,
        )
        .bind(card_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The card's revisions, newest first.
    pub async fn memory_revisions(&self, card_id: &str) -> Result<Vec<StoredMemoryRevision>> {
        let rows = sqlx::query(
            r#"
            SELECT revision.id, revision.card_id, revision.workspace_id, revision.revision, revision.title,
                   revision.body_markdown, revision.source, revision.confidence, revision.citations_json,
                   revision.created_at, revision.restored_from,
                   users.id AS author_id, users.display_name AS author_name, users.email AS author_email
            FROM memory_card_revisions AS revision
            LEFT JOIN users ON users.id = revision.created_by_user_id
            WHERE revision.card_id = ?1
            ORDER BY revision.revision DESC
            "#,
        )
        .bind(card_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(stored_revision).collect())
    }

    pub async fn memory_revision(&self, card_id: &str, revision: i64) -> Result<StoredMemoryRevision> {
        self.memory_revisions(card_id)
            .await?
            .into_iter()
            .find(|candidate| candidate.revision == revision)
            .ok_or_else(|| repomemo_domain::not_found!("That revision was not found."))
    }

    /// Makes an earlier revision the card's current content as a new revision. History is never rewritten. A cited
    /// passage or file that no longer exists is left out and reported, not invented back.
    pub async fn restore_memory_revision(&self, card_id: &str, revision: i64) -> Result<MemoryRestore> {
        let earlier = self.memory_revision(card_id, revision).await?;
        let mut tx = self.pool.begin().await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE memory_cards SET title = ?1, body_markdown = ?2, source = ?3, confidence = ?4, updated_at = ?5 WHERE id = ?6",
        )
        .bind(&earlier.title)
        .bind(&earlier.body_markdown)
        .bind(&earlier.source)
        .bind(earlier.confidence)
        .bind(&now)
        .bind(card_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM links WHERE from_type = 'memory_card' AND from_id = ?1 AND relation_type = 'cites'")
            .bind(card_id)
            .execute(&mut *tx)
            .await?;
        let mut skipped = Vec::new();
        for citation in &earlier.citations {
            let table = if citation.target_type == "chunk" {
                "chunks"
            } else {
                "artifacts"
            };
            let exists = sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table} WHERE id = ?1"))
                .bind(&citation.target_id)
                .fetch_one(&mut *tx)
                .await?
                > 0;
            if !exists {
                skipped.push(citation.clone());
                continue;
            }
            sqlx::query(
                r#"INSERT INTO links (id, workspace_id, from_id, from_type, to_id, to_type, relation_type, confidence, created_by, metadata_json, created_at)
                   VALUES (?1, ?2, ?3, 'memory_card', ?4, ?5, 'cites', ?6, 'user', ?7, ?8)"#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&earlier.workspace_id)
            .bind(card_id)
            .bind(&citation.target_id)
            .bind(&citation.target_type)
            .bind(citation.confidence)
            .bind(&citation.metadata)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }
        let created = snapshot_memory_card(&mut tx, card_id, Some(revision)).await?;
        tx.commit().await?;
        let card = self.get_memory_card(card_id).await?.card;
        Ok(MemoryRestore {
            card,
            revision: created,
            skipped,
        })
    }
}

fn stored_revision(row: &SqliteRow) -> StoredMemoryRevision {
    let author_id: Option<String> = row.get("author_id");
    StoredMemoryRevision {
        id: row.get("id"),
        card_id: row.get("card_id"),
        workspace_id: row.get("workspace_id"),
        revision: row.get("revision"),
        title: row.get("title"),
        body_markdown: row.get("body_markdown"),
        source: row.get("source"),
        confidence: row.get("confidence"),
        citations: serde_json::from_str(&row.get::<String, _>("citations_json")).unwrap_or_default(),
        author: author_id.map(|id| SharedUser {
            id,
            display_name: row.get("author_name"),
            email: row.get("author_email"),
        }),
        created_at: row.get("created_at"),
        restored_from: row.get("restored_from"),
    }
}
