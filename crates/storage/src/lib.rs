use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use repomemo_domain::{
    ArtifactDetail, ArtifactSummary, ArtifactType, Source, SourceType, Workspace,
    WorkspaceOverview,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StorageEngine {
    pool: SqlitePool,
    data_dir: PathBuf,
    blob_dir: PathBuf,
}

#[derive(Debug, sqlx::FromRow)]
struct WorkspaceRow {
    id: String,
    name: String,
    created_at: String,
    updated_at: String,
    settings_json: String,
}

#[derive(Debug, sqlx::FromRow)]
struct SourceRow {
    id: String,
    workspace_id: String,
    source_type: String,
    name: String,
    root_uri: Option<String>,
    last_indexed_at: Option<String>,
    status: String,
    metadata_json: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ArtifactSummaryRow {
    id: String,
    workspace_id: String,
    source_id: String,
    source_name: String,
    artifact_type: String,
    title: String,
    path: String,
    content_hash: String,
    mime_type: Option<String>,
    language: Option<String>,
    size_bytes: i64,
    created_at: String,
    updated_at: String,
    indexed_at: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ArtifactDetailRow {
    id: String,
    workspace_id: String,
    source_id: String,
    source_name: String,
    artifact_type: String,
    title: String,
    path: String,
    content_hash: String,
    mime_type: Option<String>,
    language: Option<String>,
    size_bytes: i64,
    created_at: String,
    updated_at: String,
    indexed_at: Option<String>,
    metadata_json: String,
}

#[derive(Debug, Clone)]
pub struct NewArtifact {
    pub workspace_id: String,
    pub source_id: String,
    pub artifact_type: ArtifactType,
    pub title: String,
    pub path: String,
    pub content_hash: String,
    pub mime_type: Option<String>,
    pub language: Option<String>,
    pub size_bytes: i64,
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct StoredArtifact {
    pub artifact: ArtifactSummary,
    pub created: bool,
}

impl StorageEngine {
    pub async fn open(config: StorageConfig) -> Result<Self> {
        tokio::fs::create_dir_all(&config.data_dir).await?;

        let blob_dir = config.data_dir.join("blobs");
        tokio::fs::create_dir_all(&blob_dir).await?;

        let db_path = config.data_dir.join("repomemo.sqlite");
        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self {
            pool,
            data_dir: config.data_dir,
            blob_dir,
        })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn blob_dir(&self) -> &Path {
        &self.blob_dir
    }

    pub async fn workspace_exists(&self, workspace_id: &str) -> Result<bool> {
        let exists = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM workspaces
            WHERE id = ?1
            "#,
        )
        .bind(workspace_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(exists > 0)
    }

    pub async fn create_workspace(&self, name: &str) -> Result<Workspace> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let settings_json = "{}";

        sqlx::query(
            r#"
            INSERT INTO workspaces (id, name, created_at, updated_at, settings_json)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(&id)
        .bind(name.trim())
        .bind(&now)
        .bind(&now)
        .bind(settings_json)
        .execute(&self.pool)
        .await?;

        Ok(Workspace {
            id,
            name: name.trim().to_owned(),
            created_at: now.clone(),
            updated_at: now,
            settings: Value::Object(Default::default()),
        })
    }

    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        let rows = sqlx::query_as::<_, WorkspaceRow>(
            r#"
            SELECT id, name, created_at, updated_at, settings_json
            FROM workspaces
            ORDER BY updated_at DESC, name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Workspace::from).collect())
    }

    pub async fn create_or_get_source(
        &self,
        workspace_id: &str,
        source_type: SourceType,
        name: &str,
        root_uri: Option<&str>,
    ) -> Result<Source> {
        if let Some(row) = sqlx::query_as::<_, SourceRow>(
            r#"
            SELECT
              id,
              workspace_id,
              type AS source_type,
              name,
              root_uri,
              last_indexed_at,
              status,
              metadata_json,
              created_at,
              updated_at
            FROM sources
            WHERE workspace_id = ?1
              AND type = ?2
              AND COALESCE(root_uri, '') = COALESCE(?3, '')
            LIMIT 1
            "#,
        )
        .bind(workspace_id)
        .bind(source_type_to_db(&source_type))
        .bind(root_uri)
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok(Source::from(row));
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO sources (
              id,
              workspace_id,
              type,
              name,
              root_uri,
              status,
              metadata_json,
              created_at,
              updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, 'ready', '{}', ?6, ?7)
            "#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(source_type_to_db(&source_type))
        .bind(name)
        .bind(root_uri)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(Source {
            id,
            workspace_id: workspace_id.to_owned(),
            source_type,
            name: name.to_owned(),
            root_uri: root_uri.map(str::to_owned),
            last_indexed_at: None,
            status: "ready".to_owned(),
            metadata: Value::Object(Default::default()),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub async fn store_blob(
        &self,
        content_hash: &str,
        bytes: &[u8],
        mime_type: Option<&str>,
    ) -> Result<String> {
        if content_hash.len() < 4 {
            bail!("content hash is too short");
        }

        let blob_path = self.blob_path(content_hash);
        if let Some(parent) = blob_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        if tokio::fs::metadata(&blob_path).await.is_err() {
            tokio::fs::write(&blob_path, bytes).await.with_context(|| {
                format!("failed to write blob {}", blob_path.display())
            })?;
        }

        let storage_uri = self.blob_storage_uri(content_hash);
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO blobs (hash, storage_uri, size_bytes, mime_type, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
        )
        .bind(content_hash)
        .bind(&storage_uri)
        .bind(bytes.len() as i64)
        .bind(mime_type)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(storage_uri)
    }

    pub async fn store_artifact(&self, artifact: NewArtifact) -> Result<StoredArtifact> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let metadata_json = serde_json::to_string(&artifact.metadata)?;

        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO artifacts (
              id,
              workspace_id,
              source_id,
              type,
              title,
              path,
              content_hash,
              mime_type,
              language,
              size_bytes,
              created_at,
              updated_at,
              metadata_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
        )
        .bind(&id)
        .bind(&artifact.workspace_id)
        .bind(&artifact.source_id)
        .bind(artifact_type_to_db(&artifact.artifact_type))
        .bind(&artifact.title)
        .bind(&artifact.path)
        .bind(&artifact.content_hash)
        .bind(&artifact.mime_type)
        .bind(&artifact.language)
        .bind(artifact.size_bytes)
        .bind(&now)
        .bind(&now)
        .bind(&metadata_json)
        .execute(&self.pool)
        .await?;

        let created = result.rows_affected() > 0;
        let stored = self
            .get_artifact_summary_by_identity(
                &artifact.workspace_id,
                &artifact.source_id,
                &artifact.path,
                &artifact.content_hash,
            )
            .await?;

        Ok(StoredArtifact {
            artifact: stored,
            created,
        })
    }

    pub async fn list_artifacts(&self, workspace_id: &str) -> Result<Vec<ArtifactSummary>> {
        let rows = sqlx::query_as::<_, ArtifactSummaryRow>(
            r#"
            SELECT
              artifacts.id,
              artifacts.workspace_id,
              artifacts.source_id,
              sources.name AS source_name,
              artifacts.type AS artifact_type,
              artifacts.title,
              artifacts.path,
              artifacts.content_hash,
              artifacts.mime_type,
              artifacts.language,
              artifacts.size_bytes,
              artifacts.created_at,
              artifacts.updated_at,
              artifacts.indexed_at
            FROM artifacts
            JOIN sources ON sources.id = artifacts.source_id
            WHERE artifacts.workspace_id = ?1
            ORDER BY artifacts.updated_at DESC, artifacts.path ASC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(ArtifactSummary::from).collect())
    }

    pub async fn get_artifact(&self, artifact_id: &str) -> Result<ArtifactDetail> {
        let row = sqlx::query_as::<_, ArtifactDetailRow>(
            r#"
            SELECT
              artifacts.id,
              artifacts.workspace_id,
              artifacts.source_id,
              sources.name AS source_name,
              artifacts.type AS artifact_type,
              artifacts.title,
              artifacts.path,
              artifacts.content_hash,
              artifacts.mime_type,
              artifacts.language,
              artifacts.size_bytes,
              artifacts.created_at,
              artifacts.updated_at,
              artifacts.indexed_at,
              artifacts.metadata_json
            FROM artifacts
            JOIN sources ON sources.id = artifacts.source_id
            WHERE artifacts.id = ?1
            "#,
        )
        .bind(artifact_id)
        .fetch_one(&self.pool)
        .await?;

        let summary = ArtifactSummary::from(row.summary_row());
        let metadata = serde_json::from_str(&row.metadata_json)
            .unwrap_or_else(|_| Value::Object(Default::default()));
        let bytes = tokio::fs::read(self.blob_path(&row.content_hash))
            .await
            .unwrap_or_default();
        let preview_limit = 120_000;
        let content_truncated = bytes.len() > preview_limit;
        let preview_bytes = if content_truncated {
            &bytes[..preview_limit]
        } else {
            &bytes
        };
        let content_preview = String::from_utf8(preview_bytes.to_vec()).ok();

        Ok(ArtifactDetail {
            summary,
            metadata,
            content_preview,
            content_truncated,
        })
    }

    pub async fn workspace_overview(&self, workspace_id: &str) -> Result<WorkspaceOverview> {
        Ok(WorkspaceOverview {
            workspace_id: workspace_id.to_owned(),
            source_count: self.count_for_workspace("sources", workspace_id).await?,
            artifact_count: self.count_for_workspace("artifacts", workspace_id).await?,
            chunk_count: self.count_for_workspace("chunks", workspace_id).await?,
            symbol_count: self.count_for_workspace("symbols", workspace_id).await?,
            memory_card_count: self
                .count_for_workspace("memory_cards", workspace_id)
                .await?,
        })
    }

    pub fn content_hash(bytes: &[u8]) -> String {
        let digest = Sha256::digest(bytes);
        hex::encode(digest)
    }

    pub fn blob_path_for_hash(&self, content_hash: &str) -> PathBuf {
        self.blob_path(content_hash)
    }

    async fn get_artifact_summary_by_identity(
        &self,
        workspace_id: &str,
        source_id: &str,
        path: &str,
        content_hash: &str,
    ) -> Result<ArtifactSummary> {
        let row = sqlx::query_as::<_, ArtifactSummaryRow>(
            r#"
            SELECT
              artifacts.id,
              artifacts.workspace_id,
              artifacts.source_id,
              sources.name AS source_name,
              artifacts.type AS artifact_type,
              artifacts.title,
              artifacts.path,
              artifacts.content_hash,
              artifacts.mime_type,
              artifacts.language,
              artifacts.size_bytes,
              artifacts.created_at,
              artifacts.updated_at,
              artifacts.indexed_at
            FROM artifacts
            JOIN sources ON sources.id = artifacts.source_id
            WHERE artifacts.workspace_id = ?1
              AND artifacts.source_id = ?2
              AND artifacts.path = ?3
              AND artifacts.content_hash = ?4
            LIMIT 1
            "#,
        )
        .bind(workspace_id)
        .bind(source_id)
        .bind(path)
        .bind(content_hash)
        .fetch_one(&self.pool)
        .await?;

        Ok(ArtifactSummary::from(row))
    }

    async fn count_for_workspace(&self, table: &str, workspace_id: &str) -> Result<i64> {
        let sql = format!("SELECT COUNT(*) FROM {table} WHERE workspace_id = ?1");
        let count = sqlx::query_scalar::<_, i64>(&sql)
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?;

        Ok(count)
    }

    fn blob_path(&self, content_hash: &str) -> PathBuf {
        self.blob_dir
            .join(&content_hash[0..2])
            .join(&content_hash[2..4])
            .join(content_hash)
    }

    fn blob_storage_uri(&self, content_hash: &str) -> String {
        format!(
            "blobs/{}/{}/{}",
            &content_hash[0..2],
            &content_hash[2..4],
            content_hash
        )
    }
}

impl From<WorkspaceRow> for Workspace {
    fn from(row: WorkspaceRow) -> Self {
        let settings = serde_json::from_str(&row.settings_json).unwrap_or_else(|_| {
            Value::Object(Default::default())
        });

        Self {
            id: row.id,
            name: row.name,
            created_at: row.created_at,
            updated_at: row.updated_at,
            settings,
        }
    }
}

impl From<SourceRow> for Source {
    fn from(row: SourceRow) -> Self {
        let metadata = serde_json::from_str(&row.metadata_json)
            .unwrap_or_else(|_| Value::Object(Default::default()));

        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            source_type: source_type_from_db(&row.source_type),
            name: row.name,
            root_uri: row.root_uri,
            last_indexed_at: row.last_indexed_at,
            status: row.status,
            metadata,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<ArtifactSummaryRow> for ArtifactSummary {
    fn from(row: ArtifactSummaryRow) -> Self {
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            source_id: row.source_id,
            source_name: row.source_name,
            artifact_type: artifact_type_from_db(&row.artifact_type),
            title: row.title,
            path: row.path,
            content_hash: row.content_hash,
            mime_type: row.mime_type,
            language: row.language,
            size_bytes: row.size_bytes,
            created_at: row.created_at,
            updated_at: row.updated_at,
            indexed_at: row.indexed_at,
        }
    }
}

impl ArtifactDetailRow {
    fn summary_row(&self) -> ArtifactSummaryRow {
        ArtifactSummaryRow {
            id: self.id.clone(),
            workspace_id: self.workspace_id.clone(),
            source_id: self.source_id.clone(),
            source_name: self.source_name.clone(),
            artifact_type: self.artifact_type.clone(),
            title: self.title.clone(),
            path: self.path.clone(),
            content_hash: self.content_hash.clone(),
            mime_type: self.mime_type.clone(),
            language: self.language.clone(),
            size_bytes: self.size_bytes,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            indexed_at: self.indexed_at.clone(),
        }
    }
}

fn source_type_to_db(source_type: &SourceType) -> &'static str {
    match source_type {
        SourceType::Upload => "upload",
        SourceType::Folder => "folder",
        SourceType::GitRepo => "git_repo",
        SourceType::Manual => "manual",
        SourceType::Connector => "connector",
    }
}

fn source_type_from_db(value: &str) -> SourceType {
    match value {
        "folder" => SourceType::Folder,
        "git_repo" => SourceType::GitRepo,
        "manual" => SourceType::Manual,
        "connector" => SourceType::Connector,
        _ => SourceType::Upload,
    }
}

fn artifact_type_to_db(artifact_type: &ArtifactType) -> &'static str {
    match artifact_type {
        ArtifactType::File => "file",
        ArtifactType::MarkdownDoc => "markdown_doc",
        ArtifactType::CodeFile => "code_file",
        ArtifactType::Issue => "issue",
        ArtifactType::Pr => "pr",
        ArtifactType::Decision => "decision",
        ArtifactType::Incident => "incident",
        ArtifactType::Runbook => "runbook",
        ArtifactType::ApiSpec => "api_spec",
        ArtifactType::Note => "note",
    }
}

fn artifact_type_from_db(value: &str) -> ArtifactType {
    match value {
        "markdown_doc" => ArtifactType::MarkdownDoc,
        "code_file" => ArtifactType::CodeFile,
        "issue" => ArtifactType::Issue,
        "pr" => ArtifactType::Pr,
        "decision" => ArtifactType::Decision,
        "incident" => ArtifactType::Incident,
        "runbook" => ArtifactType::Runbook,
        "api_spec" => ArtifactType::ApiSpec,
        "note" => ArtifactType::Note,
        _ => ArtifactType::File,
    }
}

#[cfg(test)]
mod tests {
    use super::StorageEngine;

    #[test]
    fn content_hash_is_stable_sha256_hex() {
        let hash = StorageEngine::content_hash(b"repomemo");

        assert_eq!(hash.len(), 64);
        assert_eq!(hash, StorageEngine::content_hash(b"repomemo"));
        assert_ne!(hash, StorageEngine::content_hash(b"RepoMemo"));
    }
}
