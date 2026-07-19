use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use repomemo_domain::{
    ArtifactDetail, ArtifactSummary, ArtifactType, Chunk, IndexingJobStatus, ProviderSettings,
    SearchRequest, SearchResult, Source, SourceType, Symbol, SymbolKind, SymbolSearchResult,
    Workspace, WorkspaceOverview,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};
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

#[derive(Debug, sqlx::FromRow)]
struct ChunkRow {
    id: String,
    artifact_id: String,
    workspace_id: String,
    chunk_index: i64,
    text: String,
    token_count: Option<i64>,
    start_line: Option<i64>,
    end_line: Option<i64>,
    heading_path: Option<String>,
    content_hash: String,
    embedding_status: String,
    metadata_json: String,
}

#[derive(Debug, sqlx::FromRow)]
struct SymbolRow {
    id: String,
    artifact_id: String,
    workspace_id: String,
    kind: String,
    name: String,
    signature: Option<String>,
    start_line: Option<i64>,
    end_line: Option<i64>,
    metadata_json: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ProviderSettingsRow {
    id: String,
    workspace_id: Option<String>,
    provider_type: String,
    name: String,
    base_url: Option<String>,
    model: Option<String>,
    embedding_model: Option<String>,
    enabled: bool,
    metadata_json: String,
}

#[derive(Debug, sqlx::FromRow)]
struct IndexingJobRow {
    id: String,
    workspace_id: String,
    source_id: Option<String>,
    status: String,
    stage: String,
    progress_current: i64,
    progress_total: Option<i64>,
    error_message: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug)]
struct SearchResultRow {
    artifact_id: String,
    chunk_id: String,
    title: String,
    path: String,
    artifact_type: String,
    language: Option<String>,
    snippet: String,
    start_line: Option<i64>,
    end_line: Option<i64>,
    score: f64,
    source_name: String,
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
            tokio::fs::write(&blob_path, bytes)
                .await
                .with_context(|| format!("failed to write blob {}", blob_path.display()))?;
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
        let chunks = self.list_chunks_for_artifact(artifact_id).await?;

        Ok(ArtifactDetail {
            summary,
            metadata,
            content_preview,
            content_truncated,
            chunks,
        })
    }

    pub async fn get_artifact_summary(&self, artifact_id: &str) -> Result<ArtifactSummary> {
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
            WHERE artifacts.id = ?1
            "#,
        )
        .bind(artifact_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(ArtifactSummary::from(row))
    }

    pub async fn read_artifact_blob(&self, artifact_id: &str) -> Result<Vec<u8>> {
        let content_hash = sqlx::query_scalar::<_, String>(
            r#"
            SELECT content_hash
            FROM artifacts
            WHERE id = ?1
            "#,
        )
        .bind(artifact_id)
        .fetch_one(&self.pool)
        .await?;

        tokio::fs::read(self.blob_path(&content_hash))
            .await
            .with_context(|| format!("failed to read blob for artifact {artifact_id}"))
    }

    pub async fn replace_artifact_chunks(
        &self,
        artifact_id: &str,
        chunks: Vec<Chunk>,
    ) -> Result<()> {
        self.replace_artifact_index(artifact_id, chunks, Vec::new())
            .await
    }

    pub async fn replace_artifact_index(
        &self,
        artifact_id: &str,
        chunks: Vec<Chunk>,
        symbols: Vec<Symbol>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM chunks WHERE artifact_id = ?1")
            .bind(artifact_id)
            .execute(&mut *tx)
            .await?;

        for mut chunk in chunks {
            if chunk.id.is_empty() {
                chunk.id = Uuid::new_v4().to_string();
            }
            let metadata_json = serde_json::to_string(&chunk.metadata)?;

            sqlx::query(
                r#"
                INSERT INTO chunks (
                  id,
                  artifact_id,
                  workspace_id,
                  chunk_index,
                  text,
                  token_count,
                  start_line,
                  end_line,
                  heading_path,
                  content_hash,
                  embedding_status,
                  metadata_json
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                "#,
            )
            .bind(&chunk.id)
            .bind(&chunk.artifact_id)
            .bind(&chunk.workspace_id)
            .bind(chunk.chunk_index)
            .bind(&chunk.text)
            .bind(chunk.token_count)
            .bind(chunk.start_line)
            .bind(chunk.end_line)
            .bind(&chunk.heading_path)
            .bind(&chunk.content_hash)
            .bind(&chunk.embedding_status)
            .bind(&metadata_json)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query("DELETE FROM symbols WHERE artifact_id = ?1")
            .bind(artifact_id)
            .execute(&mut *tx)
            .await?;

        for mut symbol in symbols {
            if symbol.id.is_empty() {
                symbol.id = Uuid::new_v4().to_string();
            }
            let metadata_json = serde_json::to_string(&symbol.metadata)?;
            sqlx::query(
                r#"
                INSERT INTO symbols (
                  id, artifact_id, workspace_id, kind, name, signature,
                  start_line, end_line, metadata_json
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
            )
            .bind(&symbol.id)
            .bind(&symbol.artifact_id)
            .bind(&symbol.workspace_id)
            .bind(symbol_kind_to_db(&symbol.kind))
            .bind(&symbol.name)
            .bind(&symbol.signature)
            .bind(symbol.start_line)
            .bind(symbol.end_line)
            .bind(&metadata_json)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            r#"
            UPDATE artifacts
            SET indexed_at = ?1,
                updated_at = ?1
            WHERE id = ?2
            "#,
        )
        .bind(&now)
        .bind(artifact_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn list_symbols(&self, artifact_id: &str) -> Result<Vec<Symbol>> {
        let rows = sqlx::query_as::<_, SymbolRow>(
            r#"
            SELECT id, artifact_id, workspace_id, kind, name, signature,
                   start_line, end_line, metadata_json
            FROM symbols
            WHERE artifact_id = ?1
            ORDER BY COALESCE(start_line, 2147483647), name
            "#,
        )
        .bind(artifact_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Symbol::from).collect())
    }

    pub async fn search_symbols(
        &self,
        workspace_id: &str,
        query: &str,
        limit: i64,
    ) -> Result<Vec<SymbolSearchResult>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            r#"
            SELECT symbols.id, symbols.artifact_id, symbols.workspace_id,
                   symbols.kind, symbols.name, symbols.signature,
                   symbols.start_line, symbols.end_line, symbols.metadata_json,
                   artifacts.title, artifacts.path, artifacts.language,
                   sources.name AS source_name
            FROM symbols
            JOIN artifacts ON artifacts.id = symbols.artifact_id
            JOIN sources ON sources.id = artifacts.source_id
            WHERE symbols.workspace_id = ?1
              AND instr(lower(symbols.name), lower(?2)) > 0
            ORDER BY
              CASE WHEN lower(symbols.name) = lower(?2) THEN 0 ELSE 1 END,
              length(symbols.name), symbols.name, artifacts.path
            LIMIT ?3
            "#,
        )
        .bind(workspace_id)
        .bind(query)
        .bind(limit.clamp(1, 100))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| SymbolSearchResult {
                symbol: Symbol::from(SymbolRow {
                    id: row.get("id"),
                    artifact_id: row.get("artifact_id"),
                    workspace_id: row.get("workspace_id"),
                    kind: row.get("kind"),
                    name: row.get("name"),
                    signature: row.get("signature"),
                    start_line: row.get("start_line"),
                    end_line: row.get("end_line"),
                    metadata_json: row.get("metadata_json"),
                }),
                title: row.get("title"),
                path: row.get("path"),
                language: row.get("language"),
                source_name: row.get("source_name"),
            })
            .collect())
    }

    pub async fn list_provider_settings(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<ProviderSettings>> {
        let rows = sqlx::query_as::<_, ProviderSettingsRow>(
            r#"
            SELECT id, workspace_id, provider_type, name, base_url, model,
                   embedding_model, enabled, metadata_json
            FROM provider_settings
            WHERE workspace_id = ?1
            ORDER BY enabled DESC, updated_at DESC, name
            "#,
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(ProviderSettings::from).collect())
    }

    pub async fn get_provider_settings(&self, provider_id: &str) -> Result<ProviderSettings> {
        let row = sqlx::query_as::<_, ProviderSettingsRow>(
            r#"
            SELECT id, workspace_id, provider_type, name, base_url, model,
                   embedding_model, enabled, metadata_json
            FROM provider_settings
            WHERE id = ?1
            "#,
        )
        .bind(provider_id)
        .fetch_optional(&self.pool)
        .await?
        .context("provider settings were not found")?;
        Ok(row.into())
    }

    pub async fn save_provider_settings(
        &self,
        settings: ProviderSettings,
    ) -> Result<ProviderSettings> {
        let id = if settings.id.trim().is_empty() {
            Uuid::new_v4().to_string()
        } else {
            settings.id
        };
        let now = Utc::now().to_rfc3339();
        let metadata_json = serde_json::to_string(&settings.metadata)?;
        sqlx::query(
            r#"
            INSERT INTO provider_settings (
              id, workspace_id, provider_type, name, base_url, model, embedding_model,
              enabled, metadata_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
            ON CONFLICT(id) DO UPDATE SET
              workspace_id = excluded.workspace_id, provider_type = excluded.provider_type,
              name = excluded.name, base_url = excluded.base_url, model = excluded.model,
              embedding_model = excluded.embedding_model, enabled = excluded.enabled,
              metadata_json = excluded.metadata_json, updated_at = excluded.updated_at
            "#,
        )
        .bind(&id)
        .bind(&settings.workspace_id)
        .bind(&settings.provider_type)
        .bind(&settings.name)
        .bind(&settings.base_url)
        .bind(&settings.model)
        .bind(&settings.embedding_model)
        .bind(settings.enabled)
        .bind(metadata_json)
        .bind(now)
        .execute(&self.pool)
        .await?;
        self.get_provider_settings(&id).await
    }

    pub async fn app_ai_status(&self) -> Result<(bool, Option<String>)> {
        let row = sqlx::query(
            "SELECT name FROM provider_settings WHERE enabled = 1 ORDER BY updated_at DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(match row {
            Some(row) => (true, Some(row.get("name"))),
            None => (false, None),
        })
    }

    pub async fn create_indexing_job(
        &self,
        workspace_id: &str,
        source_id: Option<&str>,
        stage: &str,
        progress_total: Option<i64>,
    ) -> Result<IndexingJobStatus> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO indexing_jobs (
              id,
              workspace_id,
              source_id,
              status,
              stage,
              progress_current,
              progress_total,
              created_at,
              updated_at
            )
            VALUES (?1, ?2, ?3, 'running', ?4, 0, ?5, ?6, ?7)
            "#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(source_id)
        .bind(stage)
        .bind(progress_total)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        self.get_indexing_job(&id).await
    }

    pub async fn update_indexing_job(
        &self,
        job_id: &str,
        status: &str,
        stage: &str,
        progress_current: i64,
        error_message: Option<&str>,
    ) -> Result<IndexingJobStatus> {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            UPDATE indexing_jobs
            SET status = ?1,
                stage = ?2,
                progress_current = ?3,
                error_message = ?4,
                updated_at = ?5
            WHERE id = ?6
            "#,
        )
        .bind(status)
        .bind(stage)
        .bind(progress_current)
        .bind(error_message)
        .bind(&now)
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        self.get_indexing_job(job_id).await
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

    pub async fn search_chunks(
        &self,
        request: &SearchRequest,
        fts_query: &str,
    ) -> Result<Vec<SearchResult>> {
        let mut query = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
              artifacts.id AS artifact_id,
              chunks.id AS chunk_id,
              artifacts.title,
              artifacts.path,
              artifacts.type AS artifact_type,
              artifacts.language,
              snippet(chunks_fts, 0, '<mark>', '</mark>', ' ... ', 24) AS snippet,
              chunks.start_line,
              chunks.end_line,
              -bm25(chunks_fts) AS score,
              sources.name AS source_name
            FROM chunks_fts
            JOIN chunks ON chunks.rowid = chunks_fts.rowid
            JOIN artifacts ON artifacts.id = chunks.artifact_id
            JOIN sources ON sources.id = artifacts.source_id
            WHERE chunks_fts MATCH
            "#,
        );
        query.push_bind(fts_query);
        query.push(" AND chunks.workspace_id = ");
        query.push_bind(&request.workspace_id);

        if !request.artifact_types.is_empty() {
            query.push(" AND artifacts.type IN (");
            let mut separated = query.separated(", ");
            for artifact_type in &request.artifact_types {
                separated.push_bind(artifact_type_to_db(artifact_type));
            }
            separated.push_unseparated(")");
        }

        if !request.languages.is_empty() {
            query.push(" AND artifacts.language IN (");
            let mut separated = query.separated(", ");
            for language in &request.languages {
                separated.push_bind(language);
            }
            separated.push_unseparated(")");
        }

        if !request.source_ids.is_empty() {
            query.push(" AND artifacts.source_id IN (");
            let mut separated = query.separated(", ");
            for source_id in &request.source_ids {
                separated.push_bind(source_id);
            }
            separated.push_unseparated(")");
        }

        query.push(" ORDER BY bm25(chunks_fts) ASC, artifacts.path ASC LIMIT ");
        query.push_bind(request.limit.unwrap_or(40).clamp(1, 100));

        let rows = query
            .build()
            .map(|row: SqliteRow| SearchResultRow {
                artifact_id: row.get("artifact_id"),
                chunk_id: row.get("chunk_id"),
                title: row.get("title"),
                path: row.get("path"),
                artifact_type: row.get("artifact_type"),
                language: row.get("language"),
                snippet: row.get("snippet"),
                start_line: row.get("start_line"),
                end_line: row.get("end_line"),
                score: row.get("score"),
                source_name: row.get("source_name"),
            })
            .fetch_all(&self.pool)
            .await?;

        Ok(rows.into_iter().map(SearchResult::from).collect())
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

    async fn list_chunks_for_artifact(&self, artifact_id: &str) -> Result<Vec<Chunk>> {
        let rows = sqlx::query_as::<_, ChunkRow>(
            r#"
            SELECT
              id,
              artifact_id,
              workspace_id,
              chunk_index,
              text,
              token_count,
              start_line,
              end_line,
              heading_path,
              content_hash,
              embedding_status,
              metadata_json
            FROM chunks
            WHERE artifact_id = ?1
            ORDER BY chunk_index ASC
            "#,
        )
        .bind(artifact_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Chunk::from).collect())
    }

    async fn get_indexing_job(&self, job_id: &str) -> Result<IndexingJobStatus> {
        let row = sqlx::query_as::<_, IndexingJobRow>(
            r#"
            SELECT
              id,
              workspace_id,
              source_id,
              status,
              stage,
              progress_current,
              progress_total,
              error_message,
              created_at,
              updated_at
            FROM indexing_jobs
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(IndexingJobStatus::from(row))
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
        let settings = serde_json::from_str(&row.settings_json)
            .unwrap_or_else(|_| Value::Object(Default::default()));

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

impl From<ChunkRow> for Chunk {
    fn from(row: ChunkRow) -> Self {
        let metadata = serde_json::from_str(&row.metadata_json)
            .unwrap_or_else(|_| Value::Object(Default::default()));

        Self {
            id: row.id,
            artifact_id: row.artifact_id,
            workspace_id: row.workspace_id,
            chunk_index: row.chunk_index,
            text: row.text,
            token_count: row.token_count,
            start_line: row.start_line,
            end_line: row.end_line,
            heading_path: row.heading_path,
            content_hash: row.content_hash,
            embedding_status: row.embedding_status,
            metadata,
        }
    }
}

impl From<SymbolRow> for Symbol {
    fn from(row: SymbolRow) -> Self {
        let metadata = serde_json::from_str(&row.metadata_json)
            .unwrap_or_else(|_| Value::Object(Default::default()));
        Self {
            id: row.id,
            artifact_id: row.artifact_id,
            workspace_id: row.workspace_id,
            kind: symbol_kind_from_db(&row.kind),
            name: row.name,
            signature: row.signature,
            start_line: row.start_line,
            end_line: row.end_line,
            metadata,
        }
    }
}

impl From<ProviderSettingsRow> for ProviderSettings {
    fn from(row: ProviderSettingsRow) -> Self {
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            provider_type: row.provider_type,
            name: row.name,
            base_url: row.base_url,
            model: row.model,
            embedding_model: row.embedding_model,
            enabled: row.enabled,
            metadata: serde_json::from_str(&row.metadata_json)
                .unwrap_or_else(|_| Value::Object(Default::default())),
        }
    }
}

impl From<IndexingJobRow> for IndexingJobStatus {
    fn from(row: IndexingJobRow) -> Self {
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            source_id: row.source_id,
            status: row.status,
            stage: row.stage,
            progress_current: row.progress_current,
            progress_total: row.progress_total,
            error_message: row.error_message,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<SearchResultRow> for SearchResult {
    fn from(row: SearchResultRow) -> Self {
        Self {
            artifact_id: row.artifact_id,
            chunk_id: row.chunk_id,
            title: row.title,
            path: row.path,
            artifact_type: artifact_type_from_db(&row.artifact_type),
            language: row.language,
            snippet: row.snippet,
            start_line: row.start_line,
            end_line: row.end_line,
            score: row.score,
            source_name: row.source_name,
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
        ArtifactType::Image => "image",
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
        "image" => ArtifactType::Image,
        _ => ArtifactType::File,
    }
}

fn symbol_kind_to_db(kind: &SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Class => "class",
        SymbolKind::Method => "method",
        SymbolKind::Interface => "interface",
        SymbolKind::Enum => "enum",
        SymbolKind::Route => "route",
        SymbolKind::Endpoint => "endpoint",
        SymbolKind::Config => "config",
        SymbolKind::Test => "test",
    }
}

fn symbol_kind_from_db(value: &str) -> SymbolKind {
    match value {
        "class" => SymbolKind::Class,
        "method" => SymbolKind::Method,
        "interface" => SymbolKind::Interface,
        "enum" => SymbolKind::Enum,
        "route" => SymbolKind::Route,
        "endpoint" => SymbolKind::Endpoint,
        "config" => SymbolKind::Config,
        "test" => SymbolKind::Test,
        _ => SymbolKind::Function,
    }
}

#[cfg(test)]
mod tests {
    use super::{NewArtifact, StorageConfig, StorageEngine};
    use repomemo_domain::{ArtifactType, Chunk, SearchRequest, SourceType, Symbol, SymbolKind};
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn content_hash_is_stable_sha256_hex() {
        let hash = StorageEngine::content_hash(b"repomemo");

        assert_eq!(hash.len(), 64);
        assert_eq!(hash, StorageEngine::content_hash(b"repomemo"));
        assert_ne!(hash, StorageEngine::content_hash(b"RepoMemo"));
    }

    #[tokio::test]
    async fn fts_search_returns_snippet_and_respects_language_filter() {
        let data_dir = std::env::temp_dir().join(format!(
            "repomemo-search-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let storage = StorageEngine::open(StorageConfig {
            data_dir: data_dir.clone(),
        })
        .await
        .unwrap();
        let workspace = storage.create_workspace("Search test").await.unwrap();
        let source = storage
            .create_or_get_source(
                &workspace.id,
                SourceType::Folder,
                "fixture",
                Some("fixture"),
            )
            .await
            .unwrap();
        let bytes = b"RepoMemo provides local artifact retrieval.";
        let content_hash = StorageEngine::content_hash(bytes);
        storage
            .store_blob(&content_hash, bytes, Some("text/markdown"))
            .await
            .unwrap();
        let stored = storage
            .store_artifact(NewArtifact {
                workspace_id: workspace.id.clone(),
                source_id: source.id.clone(),
                artifact_type: ArtifactType::MarkdownDoc,
                title: "README.md".to_owned(),
                path: "README.md".to_owned(),
                content_hash: content_hash.clone(),
                mime_type: Some("text/markdown".to_owned()),
                language: Some("Markdown".to_owned()),
                size_bytes: bytes.len() as i64,
                metadata: json!({}),
            })
            .await
            .unwrap();
        storage
            .replace_artifact_index(
                &stored.artifact.id,
                vec![Chunk {
                    id: String::new(),
                    artifact_id: stored.artifact.id.clone(),
                    workspace_id: workspace.id.clone(),
                    chunk_index: 0,
                    text: String::from_utf8(bytes.to_vec()).unwrap(),
                    token_count: Some(5),
                    start_line: Some(1),
                    end_line: Some(1),
                    heading_path: Some("Overview".to_owned()),
                    content_hash,
                    embedding_status: "not_configured".to_owned(),
                    metadata: json!({}),
                }],
                vec![Symbol {
                    id: String::new(),
                    artifact_id: stored.artifact.id.clone(),
                    workspace_id: workspace.id.clone(),
                    kind: SymbolKind::Function,
                    name: "retrieve_artifact".to_owned(),
                    signature: Some("fn retrieve_artifact()".to_owned()),
                    start_line: Some(7),
                    end_line: Some(9),
                    metadata: json!({}),
                }],
            )
            .await
            .unwrap();

        let request = SearchRequest {
            workspace_id: workspace.id,
            query: "artifact retrieval".to_owned(),
            artifact_types: vec![ArtifactType::MarkdownDoc],
            languages: vec!["Markdown".to_owned()],
            source_ids: vec![source.id],
            limit: Some(10),
        };
        let results = storage
            .search_chunks(&request, "\"artifact\"* AND \"retrieval\"*")
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].artifact_id, stored.artifact.id);
        assert_eq!(results[0].start_line, Some(1));
        assert!(results[0].snippet.contains("<mark>artifact</mark>"));

        let symbol_results = storage
            .search_symbols(&request.workspace_id, "retrieve", 10)
            .await
            .unwrap();
        assert_eq!(symbol_results.len(), 1);
        assert_eq!(symbol_results[0].symbol.name, "retrieve_artifact");
        assert_eq!(symbol_results[0].symbol.start_line, Some(7));

        storage.pool.close().await;
        drop(storage);
        std::fs::remove_dir_all(data_dir).unwrap();
    }
}
