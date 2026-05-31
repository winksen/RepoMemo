use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Utc;
use repomemo_domain::Workspace;
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

    pub fn content_hash(bytes: &[u8]) -> String {
        let digest = Sha256::digest(bytes);
        hex::encode(digest)
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
