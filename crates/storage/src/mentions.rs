//! Storage for what issues and pull requests mention (roadmap F17d, V0.2.30).

use std::collections::HashMap;

use anyhow::Result;
use repomemo_domain::ArtifactType;
use serde_json::Value;
use sqlx::Row;

use super::{artifact_type_from_db, StorageEngine};

/// One mention to store, from an item to a file artifact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MentionWrite {
    pub to_artifact_id: String,
    /// `file`, `symbol` or `commit`.
    pub kind: String,
    pub label: String,
}

/// A mention as a page shows it.
#[derive(Debug, Clone)]
pub struct MentionRow {
    /// The artifact on the other end: the item that names a file, or the
    /// file an item names.
    pub artifact_id: String,
    pub title: String,
    pub artifact_type: ArtifactType,
    pub path: String,
    pub kind: String,
    pub label: String,
}

/// The files a workspace's repositories hold, for resolving mentions.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceFileIndex {
    /// `(artifact id, path, the commit that last changed it)`
    pub files: Vec<(String, String, Option<String>)>,
    /// Symbol name → the file artifacts that define it.
    pub symbols: HashMap<String, Vec<String>>,
}

impl StorageEngine {
    /// Every tracked file of the workspace's repositories and every symbol
    /// defined in them.
    pub async fn workspace_file_index(&self, workspace_id: &str) -> Result<WorkspaceFileIndex> {
        let rows = sqlx::query(
            r#"SELECT repo_files.artifact_id, repo_files.path, repo_files.last_change_commit
               FROM repo_files JOIN artifacts ON artifacts.id = repo_files.artifact_id
               WHERE artifacts.workspace_id = ?1 AND repo_files.removed_at IS NULL"#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        let files = rows
            .into_iter()
            .map(|row| (row.get("artifact_id"), row.get("path"), row.get("last_change_commit")))
            .collect();
        let rows = sqlx::query(
            r#"SELECT symbols.name, symbols.artifact_id
               FROM symbols JOIN repo_files ON repo_files.artifact_id = symbols.artifact_id
               WHERE symbols.workspace_id = ?1 AND repo_files.removed_at IS NULL
                 AND length(symbols.name) >= 4"#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        let mut symbols: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows {
            let entry = symbols.entry(row.get::<String, _>("name")).or_default();
            let artifact_id: String = row.get("artifact_id");
            if !entry.contains(&artifact_id) {
                entry.push(artifact_id);
            }
        }
        Ok(WorkspaceFileIndex { files, symbols })
    }

    /// The items a workspace's connectors keep, with their metadata.
    pub async fn list_connector_item_metadata(&self, workspace_id: &str, limit: i64) -> Result<Vec<(String, Value)>> {
        let rows = sqlx::query(
            r#"SELECT artifacts.id, artifacts.metadata_json
               FROM connector_items JOIN artifacts ON artifacts.id = connector_items.artifact_id
               WHERE artifacts.workspace_id = ?1
               ORDER BY artifacts.updated_at DESC LIMIT ?2"#,
        )
        .bind(workspace_id)
        .bind(limit)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let metadata = serde_json::from_str::<Value>(&row.get::<String, _>("metadata_json"))
                    .unwrap_or_else(|_| Value::Object(Default::default()));
                (row.get("id"), metadata)
            })
            .collect())
    }

    /// Replaces what an item mentions. Returns how many it now names.
    pub async fn replace_mentions(
        &self,
        workspace_id: &str,
        from_artifact_id: &str,
        mentions: &[MentionWrite],
    ) -> Result<usize> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM artifact_mentions WHERE from_artifact_id = ?1")
            .bind(from_artifact_id)
            .execute(&mut *tx)
            .await?;
        for mention in mentions {
            sqlx::query(
                r#"INSERT OR IGNORE INTO artifact_mentions
                   (workspace_id, from_artifact_id, to_artifact_id, kind, label)
                   VALUES (?1, ?2, ?3, ?4, ?5)"#,
            )
            .bind(workspace_id)
            .bind(from_artifact_id)
            .bind(&mention.to_artifact_id)
            .bind(&mention.kind)
            .bind(&mention.label)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(mentions.len())
    }

    /// The items that name an artifact (`incoming`), or what an item names.
    pub async fn artifact_mentions(&self, artifact_id: &str, incoming: bool) -> Result<Vec<MentionRow>> {
        let (join, filter) = if incoming {
            ("artifact_mentions.from_artifact_id", "artifact_mentions.to_artifact_id")
        } else {
            ("artifact_mentions.to_artifact_id", "artifact_mentions.from_artifact_id")
        };
        let rows = sqlx::query(&format!(
            r#"SELECT artifacts.id, artifacts.title, artifacts.type AS artifact_type, artifacts.path,
                      artifact_mentions.kind, artifact_mentions.label
               FROM artifact_mentions JOIN artifacts ON artifacts.id = {join}
               WHERE {filter} = ?1
               ORDER BY artifacts.updated_at DESC, artifact_mentions.kind, artifact_mentions.label
               LIMIT 200"#
        ))
        .bind(artifact_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| MentionRow {
                artifact_id: row.get("id"),
                title: row.get("title"),
                artifact_type: artifact_type_from_db(&row.get::<String, _>("artifact_type")),
                path: row.get("path"),
                kind: row.get("kind"),
                label: row.get("label"),
            })
            .collect())
    }
}
