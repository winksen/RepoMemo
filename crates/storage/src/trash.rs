//! Bounded trash for locally authored memory cards and uploaded files (V0.6.15).
//!
//! Deleting a card or a file first keeps what is needed to put it back: a card with its revisions, approvals, imported
//! approval records, review assignment and citations; a file with its record, its stored content (which garbage
//! collection then leaves alone) and the citations that pointed at it. Restoring brings the same ids back, so history
//! and references line up again. Trash is bounded by age (`TRASH_RETENTION_DAYS`) and by count
//! (`TRASH_MAX_ITEMS` per workspace); purging is explicit and the server records it.
//!
//! Only content that was authored or uploaded here is kept. A file that came from a repository, a connector or any
//! other remote source is never put in trash, so removed or revoked remote content cannot be brought back from it.
//! Comments, tasks, lifecycle history and earlier versions of a deleted file are not kept.

use serde_json::{json, Value};

use super::*;

/// How long a deleted item can be restored.
pub const TRASH_RETENTION_DAYS: i64 = 30;
/// The most items one workspace keeps; the oldest are purged first.
pub const TRASH_MAX_ITEMS: i64 = 200;

/// One item in trash, without the data needed to restore it.
#[derive(Debug, Clone)]
pub struct StoredTrashItem {
    pub id: String,
    pub workspace_id: String,
    /// `memory_card` or `artifact`.
    pub kind: String,
    pub item_id: String,
    pub title: String,
    pub deleted_by: Option<SharedUser>,
    pub deleted_at: String,
    pub expires_at: String,
}

/// A card's citation to a file that was restored, to place once the file is indexed again.
#[derive(Debug, Clone)]
pub struct RestoredCitation {
    pub card_id: String,
    pub confidence: Option<f64>,
    /// The cited passage's place and text hash when the citation was to a passage.
    pub chunk_index: Option<i64>,
    pub chunk_hash: Option<String>,
}

/// What a restore brought back.
#[derive(Debug, Clone, Default)]
pub struct TrashRestore {
    pub kind: String,
    pub item_id: String,
    pub workspace_id: String,
    pub title: String,
    /// Citations of a restored card whose target is gone, so they were left out.
    pub skipped_citations: u64,
    /// Citations to place after a restored file is indexed again.
    pub citations: Vec<RestoredCitation>,
}

async fn json_value(tx: &mut sqlx::Transaction<'_, Sqlite>, sql: &str, id: &str) -> Result<Value> {
    let text: Option<String> = sqlx::query_scalar(sql)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .flatten();
    Ok(text
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null))
}

fn text(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or_default().to_owned()
}

fn optional_text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

async fn known_user(tx: &mut sqlx::Transaction<'_, Sqlite>, id: Option<String>) -> Result<Option<String>> {
    let Some(id) = id else { return Ok(None) };
    let found = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE id = ?1")
        .bind(&id)
        .fetch_one(&mut **tx)
        .await?;
    Ok((found > 0).then_some(id))
}

/// Whether a citation's target exists in the workspace being restored into (V0.6.29): a target of another workspace is not one.
async fn target_exists(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    workspace_id: &str,
    target_type: &str,
    target_id: &str,
) -> Result<bool> {
    let table = match target_type {
        "chunk" => "chunks",
        "artifact" => "artifacts",
        "memory_card" => "memory_cards",
        _ => return Ok(false),
    };
    Ok(sqlx::query_scalar::<_, i64>(&format!(
        "SELECT COUNT(*) FROM {table} WHERE id = ?1 AND workspace_id = ?2"
    ))
    .bind(target_id)
    .bind(workspace_id)
    .fetch_one(&mut **tx)
    .await?
        > 0)
}

/// Keeps the card in trash. Called inside the transaction that deletes it, before anything is removed.
pub(crate) async fn trash_memory_card(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    card_id: &str,
    deleted_by: Option<&str>,
) -> Result<()> {
    let card = json_value(
        tx,
        "SELECT json_object('workspace_id', workspace_id, 'title', title, 'body_markdown', body_markdown, 'source', source,
                'confidence', confidence, 'created_at', created_at, 'updated_at', updated_at, 'metadata_json', metadata_json)
         FROM memory_cards WHERE id = ?1",
        card_id,
    )
    .await?;
    if card.is_null() {
        return Ok(());
    }
    let revisions = json_value(
        tx,
        "SELECT json_group_array(json_object('id', id, 'revision', revision, 'title', title, 'body_markdown', body_markdown,
                'source', source, 'confidence', confidence, 'citations_json', citations_json,
                'created_by_user_id', created_by_user_id, 'created_at', created_at, 'restored_from', restored_from,
                'author_label', author_label, 'import_baseline', import_baseline))
         FROM memory_card_revisions WHERE card_id = ?1",
        card_id,
    )
    .await?;
    let approvals = json_value(
        tx,
        "SELECT json_group_array(json_object('revision', revision, 'decision', decision, 'user_id', user_id, 'note', note,
                'created_at', created_at))
         FROM (SELECT * FROM memory_card_approvals WHERE card_id = ?1 ORDER BY id)",
        card_id,
    )
    .await?;
    let imported = json_value(
        tx,
        "SELECT json_group_array(json_object('revision', revision, 'decision', decision, 'person_label', person_label,
                'note', note, 'decided_at', decided_at, 'imported_at', imported_at))
         FROM (SELECT * FROM memory_card_imported_approvals WHERE card_id = ?1 ORDER BY id)",
        card_id,
    )
    .await?;
    let assignment = json_value(
        tx,
        "SELECT json_object('reviewer_user_id', reviewer_user_id, 'reviewer_label', reviewer_label,
                'next_review_at', next_review_at, 'last_reviewed_at', last_reviewed_at,
                'assigned_by_user_id', assigned_by_user_id, 'updated_at', updated_at)
         FROM review_assignments WHERE item_kind = 'memory_card' AND item_id = ?1",
        card_id,
    )
    .await?;
    let links = json_value(
        tx,
        "SELECT json_group_array(json_object('to_type', to_type, 'to_id', to_id, 'relation_type', relation_type,
                'confidence', confidence, 'created_by', created_by, 'metadata_json', metadata_json, 'created_at', created_at))
         FROM (SELECT * FROM links WHERE from_type = 'memory_card' AND from_id = ?1 ORDER BY created_at, id)",
        card_id,
    )
    .await?;
    let workspace_id = text(&card, "workspace_id");
    let title = text(&card, "title");
    let payload = json!({
        "card": card, "revisions": revisions, "approvals": approvals, "imported_approvals": imported,
        "assignment": assignment, "links": links,
    });
    put_in_trash(
        tx,
        &workspace_id,
        "memory_card",
        card_id,
        &title,
        &payload,
        None,
        deleted_by,
    )
    .await
}

/// Keeps a locally uploaded or authored file in trash. A file from any other source is not kept.
pub(crate) async fn trash_artifact(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    artifact_id: &str,
    deleted_by: Option<&str>,
) -> Result<()> {
    let artifact = json_value(
        tx,
        "SELECT json_object('workspace_id', artifacts.workspace_id, 'source_id', artifacts.source_id,
                'type', artifacts.type, 'title', artifacts.title, 'path', artifacts.path,
                'content_hash', artifacts.content_hash, 'mime_type', artifacts.mime_type,
                'language', artifacts.language, 'size_bytes', artifacts.size_bytes,
                'created_at', artifacts.created_at, 'updated_at', artifacts.updated_at,
                'metadata_json', artifacts.metadata_json, 'folder_id', artifacts.folder_id,
                'source_type', sources.type)
         FROM artifacts JOIN sources ON sources.id = artifacts.source_id WHERE artifacts.id = ?1",
        artifact_id,
    )
    .await?;
    if !matches!(text(&artifact, "source_type").as_str(), "upload" | "manual") {
        return Ok(());
    }
    let links = json_value(
        tx,
        "SELECT json_group_array(json_object('from_id', link.from_id, 'from_type', link.from_type,
                'relation_type', link.relation_type, 'confidence', link.confidence,
                'chunk_index', chunk.chunk_index, 'chunk_hash', chunk.content_hash))
         FROM links AS link LEFT JOIN chunks AS chunk ON link.to_type = 'chunk' AND chunk.id = link.to_id
         WHERE link.from_type = 'memory_card' AND link.relation_type = 'cites'
           AND (link.to_id = ?1 OR chunk.artifact_id = ?1)",
        artifact_id,
    )
    .await?;
    let workspace_id = text(&artifact, "workspace_id");
    let title = text(&artifact, "title");
    let hash = text(&artifact, "content_hash");
    let payload = json!({ "artifact": artifact, "links": links });
    put_in_trash(
        tx,
        &workspace_id,
        "artifact",
        artifact_id,
        &title,
        &payload,
        Some(hash.as_str()),
        deleted_by,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn put_in_trash(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    workspace_id: &str,
    kind: &str,
    item_id: &str,
    title: &str,
    payload: &Value,
    content_hash: Option<&str>,
    deleted_by: Option<&str>,
) -> Result<()> {
    let now = Utc::now();
    let deleted_by = known_user(tx, deleted_by.map(str::to_owned)).await?;
    // Deleting the same item again replaces its earlier entry.
    sqlx::query("DELETE FROM trash_items WHERE kind = ?1 AND item_id = ?2")
        .bind(kind)
        .bind(item_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        r#"INSERT INTO trash_items
           (id, workspace_id, kind, item_id, title, payload_json, content_hash, deleted_by_user_id, deleted_at, expires_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(workspace_id)
    .bind(kind)
    .bind(item_id)
    .bind(title)
    .bind(serde_json::to_string(payload)?)
    .bind(content_hash)
    .bind(deleted_by)
    .bind(now.to_rfc3339())
    .bind((now + chrono::Duration::days(TRASH_RETENTION_DAYS)).to_rfc3339())
    .execute(&mut **tx)
    .await?;
    // Bounded: what has expired goes, then the oldest beyond the limit.
    sqlx::query("DELETE FROM trash_items WHERE workspace_id = ?1 AND expires_at < ?2")
        .bind(workspace_id)
        .bind(now.to_rfc3339())
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        r#"DELETE FROM trash_items WHERE workspace_id = ?1 AND id IN (
             SELECT id FROM trash_items WHERE workspace_id = ?1 ORDER BY deleted_at DESC LIMIT -1 OFFSET ?2)"#,
    )
    .bind(workspace_id)
    .bind(TRASH_MAX_ITEMS)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

impl StorageEngine {
    /// What is in the workspace's trash, newest first. Expired items are not listed.
    pub async fn list_trash(&self, workspace_id: &str) -> Result<Vec<StoredTrashItem>> {
        let rows = sqlx::query(
            r#"SELECT trash.id, trash.workspace_id, trash.kind, trash.item_id, trash.title, trash.deleted_at,
                      trash.expires_at, users.id AS user_id, users.display_name AS user_name, users.email AS user_email
               FROM trash_items AS trash LEFT JOIN users ON users.id = trash.deleted_by_user_id
               WHERE trash.workspace_id = ?1 AND trash.expires_at >= ?2
               ORDER BY trash.deleted_at DESC LIMIT ?3"#,
        )
        .bind(workspace_id)
        .bind(Utc::now().to_rfc3339())
        .bind(TRASH_MAX_ITEMS)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .iter()
            .map(|row| {
                let user_id: Option<String> = row.get("user_id");
                StoredTrashItem {
                    id: row.get("id"),
                    workspace_id: row.get("workspace_id"),
                    kind: row.get("kind"),
                    item_id: row.get("item_id"),
                    title: row.get("title"),
                    deleted_by: user_id.map(|id| SharedUser {
                        id,
                        display_name: row.get("user_name"),
                        email: row.get("user_email"),
                    }),
                    deleted_at: row.get("deleted_at"),
                    expires_at: row.get("expires_at"),
                }
            })
            .collect())
    }

    /// The trash entry of one item, in the workspace it was deleted from.
    pub async fn trash_item(&self, workspace_id: &str, trash_id: &str) -> Result<StoredTrashItem> {
        self.list_trash(workspace_id)
            .await?
            .into_iter()
            .find(|item| item.id == trash_id)
            .ok_or_else(|| {
                repomemo_domain::not_found!(
                    "That item is not in the trash (it may have been restored or may have expired)."
                )
            })
    }

    /// The size of the file an item in trash would restore, so a restore can be held to the workspace's allowance first
    /// (V0.6.29). None for a card or an item that is not there.
    pub async fn trash_artifact_size(&self, workspace_id: &str, trash_id: &str) -> Result<Option<i64>> {
        Ok(sqlx::query_scalar::<_, Option<i64>>(
            "SELECT json_extract(payload_json, '$.artifact.size_bytes') FROM trash_items WHERE id = ?1 AND workspace_id = ?2 AND kind = 'artifact'",
        )
        .bind(trash_id)
        .bind(workspace_id)
        .fetch_optional(self.reader())
        .await?
        .flatten())
    }

    /// Removes an item from trash for good. A file's stored content is left for garbage collection.
    pub async fn purge_trash_item(&self, workspace_id: &str, trash_id: &str) -> Result<()> {
        let removed = sqlx::query("DELETE FROM trash_items WHERE id = ?1 AND workspace_id = ?2")
            .bind(trash_id)
            .bind(workspace_id)
            .execute(&self.pool)
            .await?
            .rows_affected();
        if removed == 0 {
            repomemo_domain::bail_not_found!("That item is not in the trash.");
        }
        Ok(())
    }

    /// Puts an item back with its original id. A card returns with its revisions, approvals, review assignment and the
    /// citations whose targets still exist. A file returns as a record that is indexed again by the caller, which then
    /// places the citations in `citations`.
    pub async fn restore_from_trash(&self, workspace_id: &str, trash_id: &str) -> Result<TrashRestore> {
        let item = self.trash_item(workspace_id, trash_id).await?;
        let payload_text =
            sqlx::query_scalar::<_, String>("SELECT payload_json FROM trash_items WHERE id = ?1 AND workspace_id = ?2")
                .bind(trash_id)
                .bind(workspace_id)
                .fetch_one(self.reader())
                .await?;
        let payload: Value = serde_json::from_str(&payload_text)?;
        let mut tx = self.pool.begin().await?;
        let mut restore = TrashRestore {
            kind: item.kind.clone(),
            item_id: item.item_id.clone(),
            workspace_id: workspace_id.to_owned(),
            title: item.title.clone(),
            ..TrashRestore::default()
        };
        if item.kind == "memory_card" {
            restore_card(&mut tx, workspace_id, &item.item_id, &payload, &mut restore).await?;
        } else {
            restore_artifact(&mut tx, workspace_id, &item.item_id, &payload, &mut restore).await?;
        }
        sqlx::query("DELETE FROM trash_items WHERE id = ?1")
            .bind(trash_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(restore)
    }

    /// Places one citation of a restored file, as a passage when it can be found again and as the whole file when not.
    /// Returns false when the card is gone or already cites the target.
    pub async fn relink_restored_citation(
        &self,
        workspace_id: &str,
        card_id: &str,
        artifact_id: &str,
        chunk_id: Option<&str>,
        confidence: Option<f64>,
    ) -> Result<bool> {
        let exists =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM memory_cards WHERE id = ?1 AND workspace_id = ?2")
                .bind(card_id)
                .bind(workspace_id)
                .fetch_one(self.reader())
                .await?
                > 0;
        if !exists {
            return Ok(false);
        }
        let (target, kind) = match chunk_id {
            Some(chunk) => (chunk, "chunk"),
            None => (artifact_id, "artifact"),
        };
        let present = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM links WHERE from_type = 'memory_card' AND from_id = ?1 AND relation_type = 'cites' AND to_id = ?2",
        )
        .bind(card_id)
        .bind(target)
        .fetch_one(self.reader())
        .await?
            > 0;
        if present {
            return Ok(false);
        }
        sqlx::query(
            r#"INSERT INTO links (id, workspace_id, from_id, from_type, to_id, to_type, relation_type, confidence, created_by, metadata_json, created_at)
               VALUES (?1, ?2, ?3, 'memory_card', ?4, ?5, 'cites', ?6, 'user', '{}', ?7)"#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(workspace_id)
        .bind(card_id)
        .bind(target)
        .bind(kind)
        .bind(confidence)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(true)
    }
}

async fn restore_card(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    workspace_id: &str,
    card_id: &str,
    payload: &Value,
    restore: &mut TrashRestore,
) -> Result<()> {
    let taken = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM memory_cards WHERE id = ?1")
        .bind(card_id)
        .fetch_one(&mut **tx)
        .await?;
    if taken > 0 {
        repomemo_domain::bail_conflict!("This memory card already exists.");
    }
    let card = &payload["card"];
    sqlx::query(
        r#"INSERT INTO memory_cards (id, workspace_id, title, body_markdown, source, confidence, created_at, updated_at, metadata_json)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
    )
    .bind(card_id)
    .bind(workspace_id)
    .bind(text(card, "title"))
    .bind(text(card, "body_markdown"))
    .bind(text(card, "source"))
    .bind(card.get("confidence").and_then(Value::as_f64))
    .bind(text(card, "created_at"))
    .bind(text(card, "updated_at"))
    .bind(text(card, "metadata_json"))
    .execute(&mut **tx)
    .await?;
    let mut revisions: Vec<&Value> = payload["revisions"]
        .as_array()
        .map(|all| all.iter().collect())
        .unwrap_or_default();
    revisions.sort_by_key(|revision| revision["revision"].as_i64().unwrap_or(0));
    for revision in revisions {
        let author = known_user(tx, optional_text(revision, "created_by_user_id")).await?;
        sqlx::query(
            r#"INSERT INTO memory_card_revisions
               (id, card_id, workspace_id, revision, title, body_markdown, source, confidence, citations_json,
                created_by_user_id, created_at, restored_from, author_label, import_baseline)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"#,
        )
        .bind(text(revision, "id"))
        .bind(card_id)
        .bind(workspace_id)
        .bind(revision["revision"].as_i64().unwrap_or(0))
        .bind(text(revision, "title"))
        .bind(text(revision, "body_markdown"))
        .bind(text(revision, "source"))
        .bind(revision.get("confidence").and_then(Value::as_f64))
        .bind(text(revision, "citations_json"))
        .bind(author)
        .bind(text(revision, "created_at"))
        .bind(revision.get("restored_from").and_then(Value::as_i64))
        .bind(optional_text(revision, "author_label"))
        .bind(revision.get("import_baseline").and_then(Value::as_i64).unwrap_or(0))
        .execute(&mut **tx)
        .await?;
    }
    for approval in payload["approvals"].as_array().into_iter().flatten() {
        let user = known_user(tx, optional_text(approval, "user_id")).await?;
        sqlx::query(
            r#"INSERT INTO memory_card_approvals (card_id, workspace_id, revision, decision, user_id, note, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
        )
        .bind(card_id)
        .bind(workspace_id)
        .bind(approval["revision"].as_i64().unwrap_or(0))
        .bind(text(approval, "decision"))
        .bind(user)
        .bind(text(approval, "note"))
        .bind(text(approval, "created_at"))
        .execute(&mut **tx)
        .await?;
    }
    for record in payload["imported_approvals"].as_array().into_iter().flatten() {
        sqlx::query(
            r#"INSERT INTO memory_card_imported_approvals
               (card_id, workspace_id, revision, decision, person_label, note, decided_at, imported_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        )
        .bind(card_id)
        .bind(workspace_id)
        .bind(record["revision"].as_i64().unwrap_or(0))
        .bind(text(record, "decision"))
        .bind(text(record, "person_label"))
        .bind(text(record, "note"))
        .bind(text(record, "decided_at"))
        .bind(text(record, "imported_at"))
        .execute(&mut **tx)
        .await?;
    }
    let assignment = &payload["assignment"];
    if assignment.is_object() {
        let reviewer = known_user(tx, optional_text(assignment, "reviewer_user_id")).await?;
        let assigned_by = known_user(tx, optional_text(assignment, "assigned_by_user_id")).await?;
        sqlx::query(
            r#"INSERT OR REPLACE INTO review_assignments
               (item_kind, item_id, workspace_id, reviewer_user_id, reviewer_label, next_review_at, last_reviewed_at,
                assigned_by_user_id, updated_at)
               VALUES ('memory_card', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        )
        .bind(card_id)
        .bind(workspace_id)
        .bind(reviewer)
        .bind(optional_text(assignment, "reviewer_label"))
        .bind(optional_text(assignment, "next_review_at"))
        .bind(optional_text(assignment, "last_reviewed_at"))
        .bind(assigned_by)
        .bind(text(assignment, "updated_at"))
        .execute(&mut **tx)
        .await?;
    }
    for link in payload["links"].as_array().into_iter().flatten() {
        let (target_type, target_id) = (text(link, "to_type"), text(link, "to_id"));
        if !target_exists(tx, workspace_id, &target_type, &target_id).await? {
            restore.skipped_citations += 1;
            continue;
        }
        sqlx::query(
            r#"INSERT INTO links (id, workspace_id, from_id, from_type, to_id, to_type, relation_type, confidence, created_by, metadata_json, created_at)
               VALUES (?1, ?2, ?3, 'memory_card', ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(workspace_id)
        .bind(card_id)
        .bind(&target_id)
        .bind(&target_type)
        .bind(text(link, "relation_type"))
        .bind(link.get("confidence").and_then(Value::as_f64))
        .bind(text(link, "created_by"))
        .bind(text(link, "metadata_json"))
        .bind(text(link, "created_at"))
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn restore_artifact(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    workspace_id: &str,
    artifact_id: &str,
    payload: &Value,
    restore: &mut TrashRestore,
) -> Result<()> {
    let artifact = &payload["artifact"];
    let source_id = text(artifact, "source_id");
    let source_kept = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sources WHERE id = ?1 AND workspace_id = ?2")
        .bind(&source_id)
        .bind(workspace_id)
        .fetch_one(&mut **tx)
        .await?
        > 0;
    if !source_kept {
        repomemo_domain::bail_conflict!("The source this file belonged to no longer exists, so it cannot be put back.");
    }
    let hash = text(artifact, "content_hash");
    let content_kept = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM blobs WHERE hash = ?1")
        .bind(&hash)
        .fetch_one(&mut **tx)
        .await?
        > 0;
    if !content_kept {
        repomemo_domain::bail_conflict!("The stored content of this file is no longer available.");
    }
    let folder = match optional_text(artifact, "folder_id") {
        Some(folder_id) => {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM folders WHERE id = ?1 AND workspace_id = ?2")
                .bind(&folder_id)
                .bind(workspace_id)
                .fetch_one(&mut **tx)
                .await
                .map(|found| (found > 0).then_some(folder_id))?
        }
        None => None,
    };
    let duplicate = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM artifacts WHERE id = ?1 OR (workspace_id = ?2 AND source_id = ?3 AND path = ?4 AND content_hash = ?5)",
    )
    .bind(artifact_id)
    .bind(workspace_id)
    .bind(&source_id)
    .bind(text(artifact, "path"))
    .bind(&hash)
    .fetch_one(&mut **tx)
    .await?;
    if duplicate > 0 {
        repomemo_domain::bail_conflict!(
            "A file with the same name and content already exists, so this one was not put back."
        );
    }
    sqlx::query(
        r#"INSERT INTO artifacts
           (id, workspace_id, source_id, type, title, path, content_hash, mime_type, language, size_bytes, created_at,
            updated_at, indexed_at, metadata_json, folder_id, index_version)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL, ?13, ?14, 0)"#,
    )
    .bind(artifact_id)
    .bind(workspace_id)
    .bind(&source_id)
    .bind(text(artifact, "type"))
    .bind(text(artifact, "title"))
    .bind(text(artifact, "path"))
    .bind(&hash)
    .bind(optional_text(artifact, "mime_type"))
    .bind(optional_text(artifact, "language"))
    .bind(artifact.get("size_bytes").and_then(Value::as_i64).unwrap_or(0))
    .bind(text(artifact, "created_at"))
    .bind(Utc::now().to_rfc3339())
    .bind(text(artifact, "metadata_json"))
    .bind(folder)
    .execute(&mut **tx)
    .await?;
    for link in payload["links"].as_array().into_iter().flatten() {
        restore.citations.push(RestoredCitation {
            card_id: text(link, "from_id"),
            confidence: link.get("confidence").and_then(Value::as_f64),
            chunk_index: link.get("chunk_index").and_then(Value::as_i64),
            chunk_hash: optional_text(link, "chunk_hash"),
        });
    }
    Ok(())
}
