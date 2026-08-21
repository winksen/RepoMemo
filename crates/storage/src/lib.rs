use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use repomemo_domain::{
    ArtifactDetail, ArtifactSummary, ArtifactType, Chunk, Citation, IndexingJobStatus, MemoryCard,
    MemoryCardDetail, MemoryCardSummary, MemoryEvidence, Organization, ProviderSettings,
    SearchRequest, SearchResult, SharedUser, SharedWorkspace, Source, SourceType, Symbol,
    SymbolKind, SymbolSearchResult, Workspace, WorkspaceActivityEvent, WorkspaceMember,
    WorkspaceMembership, WorkspaceOverview, WorkspaceRole,
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
struct UserRow {
    id: String,
    email: String,
    display_name: String,
    password_hash: String,
}

#[derive(Debug, sqlx::FromRow)]
struct UserProfileRow {
    id: String,
    email: String,
    display_name: String,
    created_at: String,
    updated_at: String,
    last_connected_at: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct OrganizationRow {
    id: String,
    name: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct SharedWorkspaceRow {
    id: String,
    name: String,
    created_at: String,
    updated_at: String,
    settings_json: String,
    organization_id: String,
    role: String,
}

#[derive(Debug, sqlx::FromRow)]
struct WorkspaceMemberRow {
    id: String,
    email: String,
    display_name: String,
    role: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct WorkspaceActivityRow {
    id: String,
    workspace_id: String,
    actor_id: Option<String>,
    actor_email: Option<String>,
    actor_display_name: Option<String>,
    action: String,
    subject_type: String,
    subject_id: Option<String>,
    summary: String,
    created_at: String,
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
struct EmbeddingSearchRow {
    chunk_id: String,
    artifact_id: String,
    title: String,
    path: String,
    artifact_type: String,
    language: Option<String>,
    text: String,
    start_line: Option<i64>,
    end_line: Option<i64>,
    source_name: String,
    vector_blob: Vec<u8>,
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

#[derive(Debug, sqlx::FromRow)]
struct MemoryCardRow {
    id: String,
    workspace_id: String,
    title: String,
    body_markdown: String,
    source: String,
    confidence: Option<f64>,
    created_at: String,
    updated_at: String,
    metadata_json: String,
}

#[derive(Debug, sqlx::FromRow)]
struct MemoryCardSummaryRow {
    id: String,
    workspace_id: String,
    title: String,
    body_excerpt: String,
    source: String,
    evidence_count: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct MemoryEvidenceRow {
    link_id: String,
    target_id: String,
    target_type: String,
    artifact_id: Option<String>,
    chunk_id: Option<String>,
    title: Option<String>,
    path: Option<String>,
    start_line: Option<i64>,
    end_line: Option<i64>,
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

#[derive(Debug, Clone)]
pub struct StoredUser {
    pub user: SharedUser,
    pub password_hash: String,
}

#[derive(Debug, Clone)]
pub struct UserProfile {
    pub user: SharedUser,
    pub created_at: String,
    pub updated_at: String,
    pub last_connected_at: Option<String>,
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

    pub async fn update_workspace_name(&self, workspace_id: &str, name: &str) -> Result<Workspace> {
        let name = require_name(name, "Workspace name")?;
        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query("UPDATE workspaces SET name = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&name)
            .bind(&now)
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            bail!("Workspace was not found.");
        }
        let row = sqlx::query_as::<_, WorkspaceRow>(
            "SELECT id, name, created_at, updated_at, settings_json FROM workspaces WHERE id = ?1",
        )
        .bind(workspace_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(Workspace::from(row))
    }

    pub async fn delete_workspace(&self, workspace_id: &str) -> Result<()> {
        let changed = sqlx::query("DELETE FROM workspaces WHERE id = ?1")
            .bind(workspace_id)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            bail!("Workspace was not found.");
        }
        Ok(())
    }

    pub async fn create_user(
        &self,
        email: &str,
        display_name: &str,
        password_hash: &str,
    ) -> Result<SharedUser> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let email = normalize_email(email)?;

        sqlx::query(
            "INSERT INTO users (id, email, display_name, password_hash, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&id)
        .bind(&email)
        .bind(display_name.trim())
        .bind(password_hash)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(SharedUser {
            id,
            email: Some(email),
            display_name: display_name.trim().to_owned(),
        })
    }

    pub async fn find_user_for_auth(&self, email: &str) -> Result<Option<StoredUser>> {
        let email = normalize_email(email)?;
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, email, display_name, password_hash FROM users WHERE email = ?1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(StoredUser::from))
    }

    pub async fn find_user(&self, user_id: &str) -> Result<Option<SharedUser>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, email, display_name, password_hash FROM users WHERE id = ?1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| StoredUser::from(row).user))
    }

    pub async fn find_user_by_email(&self, email: &str) -> Result<Option<SharedUser>> {
        let email = normalize_email(email)?;
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, email, display_name, password_hash FROM users WHERE email = ?1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| StoredUser::from(row).user))
    }

    pub async fn get_user_profile(&self, user_id: &str) -> Result<Option<UserProfile>> {
        let row = sqlx::query_as::<_, UserProfileRow>(
            "SELECT id, email, display_name, created_at, updated_at, last_connected_at FROM users WHERE id = ?1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(UserProfile::from))
    }

    pub async fn update_user_display_name(&self, user_id: &str, display_name: &str) -> Result<SharedUser> {
        let display_name = require_name(display_name, "Display name")?;
        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query("UPDATE users SET display_name = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&display_name)
            .bind(now)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            bail!("User was not found.");
        }
        self.find_user(user_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("User was not found."))
    }

    pub async fn update_user_password(&self, user_id: &str, password_hash: &str) -> Result<()> {
        let changed = sqlx::query("UPDATE users SET password_hash = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(password_hash)
            .bind(Utc::now().to_rfc3339())
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            bail!("User was not found.");
        }
        Ok(())
    }

    pub async fn touch_user_connection(&self, user_id: &str) -> Result<()> {
        sqlx::query("UPDATE users SET last_connected_at = ?1 WHERE id = ?2")
            .bind(Utc::now().to_rfc3339())
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn create_organization(&self, owner_id: &str, name: &str) -> Result<Organization> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let name = require_name(name, "Organization name")?;
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO organizations (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(&id)
        .bind(&name)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO organization_memberships (organization_id, user_id, role, created_at) VALUES (?1, ?2, 'owner', ?3)",
        )
        .bind(&id)
        .bind(owner_id)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(Organization {
            id,
            name,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub async fn list_organizations_for_user(&self, user_id: &str) -> Result<Vec<Organization>> {
        let rows = sqlx::query_as::<_, OrganizationRow>(
            "SELECT o.id, o.name, o.created_at, o.updated_at FROM organizations o INNER JOIN organization_memberships m ON m.organization_id = o.id WHERE m.user_id = ?1 ORDER BY o.name ASC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Organization::from).collect())
    }

    pub async fn user_belongs_to_organization(
        &self,
        user_id: &str,
        organization_id: &str,
    ) -> Result<bool> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM organization_memberships WHERE organization_id = ?1 AND user_id = ?2",
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(count > 0)
    }

    pub async fn create_shared_workspace(
        &self,
        owner_id: &str,
        organization_id: &str,
        name: &str,
    ) -> Result<SharedWorkspace> {
        if !self
            .user_belongs_to_organization(owner_id, organization_id)
            .await?
        {
            bail!("User is not a member of this organization.");
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let name = require_name(name, "Workspace name")?;
        let mut tx = self.pool.begin().await?;

        sqlx::query("INSERT INTO workspaces (id, name, created_at, updated_at, settings_json) VALUES (?1, ?2, ?3, ?4, '{}')")
            .bind(&id)
            .bind(&name)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO workspace_organizations (workspace_id, organization_id) VALUES (?1, ?2)",
        )
        .bind(&id)
        .bind(organization_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO workspace_memberships (workspace_id, user_id, role, created_at, updated_at) VALUES (?1, ?2, 'owner', ?3, ?4)")
            .bind(&id)
            .bind(owner_id)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO workspace_activity (id, workspace_id, actor_user_id, action, subject_type, subject_id, summary, created_at) VALUES (?1, ?2, ?3, 'workspace_created', 'workspace', ?2, ?4, ?5)")
            .bind(Uuid::new_v4().to_string())
            .bind(&id)
            .bind(owner_id)
            .bind(format!("Created workspace {name}."))
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(SharedWorkspace {
            workspace: Workspace {
                id,
                name,
                created_at: now.clone(),
                updated_at: now,
                settings: Value::Object(Default::default()),
            },
            organization_id: organization_id.to_owned(),
            role: WorkspaceRole::Owner,
        })
    }

    pub async fn list_shared_workspaces_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<SharedWorkspace>> {
        let rows = sqlx::query_as::<_, SharedWorkspaceRow>(
            "SELECT w.id, w.name, w.created_at, w.updated_at, w.settings_json, wo.organization_id, wm.role FROM workspaces w INNER JOIN workspace_organizations wo ON wo.workspace_id = w.id INNER JOIN workspace_memberships wm ON wm.workspace_id = w.id WHERE wm.user_id = ?1 ORDER BY w.updated_at DESC, w.name ASC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(SharedWorkspace::try_from).collect()
    }

    pub async fn workspace_memberships_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<WorkspaceMembership>> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT workspace_id, role FROM workspace_memberships WHERE user_id = ?1",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|(workspace_id, role)| {
                Ok(WorkspaceMembership {
                    workspace_id,
                    role: workspace_role_from_db(&role)?,
                })
            })
            .collect()
    }

    pub async fn workspace_role_for_user(
        &self,
        user_id: &str,
        workspace_id: &str,
    ) -> Result<Option<WorkspaceRole>> {
        let role = sqlx::query_scalar::<_, String>(
            "SELECT role FROM workspace_memberships WHERE workspace_id = ?1 AND user_id = ?2",
        )
        .bind(workspace_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        role.map(|role| workspace_role_from_db(&role)).transpose()
    }

    pub async fn list_workspace_members(&self, workspace_id: &str) -> Result<Vec<WorkspaceMember>> {
        let rows = sqlx::query_as::<_, WorkspaceMemberRow>(
            "SELECT u.id, u.email, u.display_name, wm.role, wm.created_at, wm.updated_at FROM workspace_memberships wm INNER JOIN users u ON u.id = wm.user_id WHERE wm.workspace_id = ?1 ORDER BY CASE wm.role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 WHEN 'member' THEN 2 ELSE 3 END, u.display_name ASC",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(WorkspaceMember::try_from).collect()
    }

    pub async fn record_workspace_activity(
        &self,
        workspace_id: &str,
        actor_user_id: Option<&str>,
        action: &str,
        subject_type: &str,
        subject_id: Option<&str>,
        summary: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO workspace_activity (id, workspace_id, actor_user_id, action, subject_type, subject_id, summary, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(workspace_id)
        .bind(actor_user_id)
        .bind(action)
        .bind(subject_type)
        .bind(subject_id)
        .bind(summary)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_workspace_activity(
        &self,
        workspace_id: &str,
        limit: i64,
    ) -> Result<Vec<WorkspaceActivityEvent>> {
        let rows = sqlx::query_as::<_, WorkspaceActivityRow>(
            r#"
            SELECT
              activity.id,
              activity.workspace_id,
              actor.id AS actor_id,
              actor.email AS actor_email,
              actor.display_name AS actor_display_name,
              activity.action,
              activity.subject_type,
              activity.subject_id,
              activity.summary,
              activity.created_at
            FROM workspace_activity AS activity
            LEFT JOIN users AS actor ON actor.id = activity.actor_user_id
            WHERE activity.workspace_id = ?1
            ORDER BY activity.created_at DESC, activity.id DESC
            LIMIT ?2
            "#,
        )
        .bind(workspace_id)
        .bind(limit.clamp(1, 100))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(WorkspaceActivityEvent::from).collect())
    }

    pub async fn list_user_activity(&self, user_id: &str, limit: i64) -> Result<Vec<WorkspaceActivityEvent>> {
        let rows = sqlx::query_as::<_, WorkspaceActivityRow>(
            r#"
            SELECT
              activity.id,
              activity.workspace_id,
              actor.id AS actor_id,
              actor.email AS actor_email,
              actor.display_name AS actor_display_name,
              activity.action,
              activity.subject_type,
              activity.subject_id,
              activity.summary,
              activity.created_at
            FROM workspace_activity AS activity
            LEFT JOIN users AS actor ON actor.id = activity.actor_user_id
            WHERE activity.actor_user_id = ?1
            ORDER BY activity.created_at DESC, activity.id DESC
            LIMIT ?2
            "#,
        )
        .bind(user_id)
        .bind(limit.clamp(1, 100))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(WorkspaceActivityEvent::from).collect())
    }

    pub async fn upsert_workspace_member(
        &self,
        workspace_id: &str,
        email: &str,
        role: WorkspaceRole,
    ) -> Result<WorkspaceMember> {
        if matches!(role, WorkspaceRole::Owner) {
            bail!("The owner role cannot be assigned through membership management.");
        }
        let user = self
            .find_user_by_email(email)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No RepoMemo account exists for this email address."))?;
        let organization_id = sqlx::query_scalar::<_, String>(
            "SELECT organization_id FROM workspace_organizations WHERE workspace_id = ?1",
        )
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Workspace was not found."))?;
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT OR IGNORE INTO organization_memberships (organization_id, user_id, role, created_at) VALUES (?1, ?2, 'member', ?3)")
            .bind(&organization_id)
            .bind(&user.id)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO workspace_memberships (workspace_id, user_id, role, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(workspace_id, user_id) DO UPDATE SET role = excluded.role, updated_at = excluded.updated_at")
            .bind(workspace_id)
            .bind(&user.id)
            .bind(workspace_role_to_db(&role))
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.list_workspace_members(workspace_id)
            .await?
            .into_iter()
            .find(|member| member.user.id == user.id)
            .ok_or_else(|| anyhow::anyhow!("Workspace member could not be loaded."))
    }

    pub async fn remove_workspace_member(&self, workspace_id: &str, user_id: &str) -> Result<()> {
        let role = self.workspace_role_for_user(user_id, workspace_id).await?;
        if matches!(role, Some(WorkspaceRole::Owner)) {
            bail!("The workspace owner cannot be removed.");
        }
        let result = sqlx::query(
            "DELETE FROM workspace_memberships WHERE workspace_id = ?1 AND user_id = ?2",
        )
        .bind(workspace_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            bail!("Workspace membership was not found.");
        }
        Ok(())
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

    pub async fn update_artifact_title(
        &self,
        artifact_id: &str,
        title: &str,
    ) -> Result<ArtifactSummary> {
        let title = require_name(title, "Artifact title")?;
        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query("UPDATE artifacts SET title = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&title)
            .bind(&now)
            .bind(artifact_id)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            bail!("Artifact was not found.");
        }
        self.get_artifact_summary(artifact_id).await
    }

    pub async fn delete_artifact(&self, artifact_id: &str) -> Result<()> {
        let summary = self.get_artifact_summary(artifact_id).await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM links WHERE workspace_id = ?1 AND (to_id = ?2 OR (to_type = 'chunk' AND to_id IN (SELECT id FROM chunks WHERE artifact_id = ?2)))",
        )
        .bind(&summary.workspace_id)
        .bind(artifact_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM artifacts WHERE id = ?1")
            .bind(artifact_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
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

    pub async fn list_workspace_chunks(&self, workspace_id: &str) -> Result<Vec<Chunk>> {
        let rows = sqlx::query_as::<_, ChunkRow>(
            r#"SELECT id, artifact_id, workspace_id, chunk_index, text, token_count,
               start_line, end_line, heading_path, content_hash, embedding_status, metadata_json
               FROM chunks WHERE workspace_id = ?1 ORDER BY artifact_id, chunk_index"#,
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Chunk::from).collect())
    }

    pub async fn upsert_embeddings(
        &self,
        workspace_id: &str,
        model: &str,
        embeddings: Vec<(String, Vec<f32>)>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        for (chunk_id, vector) in embeddings {
            if vector.is_empty() {
                continue;
            }
            let dimensions = vector.len() as i64;
            let bytes = encode_embedding(&vector);
            sqlx::query(
                r#"INSERT INTO chunk_embeddings (chunk_id, workspace_id, model, dimensions, vector_blob, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
                   ON CONFLICT(chunk_id) DO UPDATE SET model = excluded.model, dimensions = excluded.dimensions,
                     vector_blob = excluded.vector_blob, updated_at = excluded.updated_at"#,
            )
            .bind(&chunk_id).bind(workspace_id).bind(model).bind(dimensions).bind(bytes).bind(&now)
            .execute(&mut *tx).await?;
            sqlx::query("UPDATE chunks SET embedding_status = 'ready' WHERE id = ?1")
                .bind(&chunk_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn search_chunks_by_embedding(
        &self,
        workspace_id: &str,
        query: &[f32],
        limit: i64,
    ) -> Result<Vec<SearchResult>> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let rows = sqlx::query_as::<_, EmbeddingSearchRow>(
            r#"SELECT chunks.id AS chunk_id, chunks.artifact_id, artifacts.title, artifacts.path,
                 artifacts.type AS artifact_type, artifacts.language, chunks.text, chunks.start_line,
                 chunks.end_line, sources.name AS source_name, chunk_embeddings.vector_blob
               FROM chunk_embeddings JOIN chunks ON chunks.id = chunk_embeddings.chunk_id
               JOIN artifacts ON artifacts.id = chunks.artifact_id JOIN sources ON sources.id = artifacts.source_id
               WHERE chunk_embeddings.workspace_id = ?1"#,
        ).bind(workspace_id).fetch_all(&self.pool).await?;
        let mut results = rows
            .into_iter()
            .filter_map(|row| {
                let vector = decode_embedding(&row.vector_blob)?;
                let score = cosine_similarity(query, &vector)?;
                Some(SearchResult {
                    artifact_id: row.artifact_id,
                    chunk_id: row.chunk_id,
                    title: row.title,
                    path: row.path,
                    artifact_type: artifact_type_from_db(&row.artifact_type),
                    language: row.language,
                    snippet: row.text.chars().take(360).collect(),
                    start_line: row.start_line,
                    end_line: row.end_line,
                    score,
                    source_name: row.source_name,
                })
            })
            .collect::<Vec<_>>();
        results.sort_by(|left, right| right.score.total_cmp(&left.score));
        results.truncate(limit.clamp(1, 100) as usize);
        Ok(results)
    }

    pub async fn embedding_count(&self, workspace_id: &str) -> Result<i64> {
        let row =
            sqlx::query("SELECT COUNT(*) AS count FROM chunk_embeddings WHERE workspace_id = ?1")
                .bind(workspace_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(row.get("count"))
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
        let mut metadata = settings.metadata;
        if let Some(api_key) = settings.api_key.filter(|value| !value.trim().is_empty()) {
            metadata["api_key"] = Value::String(api_key);
        } else if let Ok(existing) = self.get_provider_settings(&id).await {
            if let Some(api_key) = existing.api_key {
                metadata["api_key"] = Value::String(api_key);
            }
        }
        let metadata_json = serde_json::to_string(&metadata)?;
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

    pub async fn create_memory_card(
        &self,
        workspace_id: &str,
        title: &str,
        body_markdown: &str,
        source: &str,
        confidence: Option<f64>,
        citations: &[Citation],
    ) -> Result<MemoryCard> {
        for citation in citations {
            self.validate_memory_evidence(workspace_id, citation)
                .await?;
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO memory_cards (
              id, workspace_id, title, body_markdown, source, confidence,
              created_at, updated_at, metadata_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, '{}')
            "#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(title.trim())
        .bind(body_markdown.trim())
        .bind(source.trim())
        .bind(confidence)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        for citation in citations {
            let (target_id, target_type) = citation
                .chunk_id
                .as_ref()
                .map(|chunk_id| (chunk_id.as_str(), "chunk"))
                .unwrap_or((citation.artifact_id.as_str(), "artifact"));
            sqlx::query(
                r#"
                INSERT INTO links (
                  id, workspace_id, from_id, from_type, to_id, to_type,
                  relation_type, confidence, created_by, metadata_json, created_at
                ) VALUES (?1, ?2, ?3, 'memory_card', ?4, ?5, 'cites', ?6, 'user', '{}', ?7)
                "#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(workspace_id)
            .bind(&id)
            .bind(target_id)
            .bind(target_type)
            .bind(citation.confidence)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }

        self.get_memory_card(&id).await.map(|detail| detail.card)
    }

    pub async fn update_memory_card(
        &self,
        card_id: &str,
        title: &str,
        body_markdown: &str,
        source: &str,
        confidence: Option<f64>,
    ) -> Result<MemoryCard> {
        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query(
            r#"
            UPDATE memory_cards
            SET title = ?1, body_markdown = ?2, source = ?3, confidence = ?4, updated_at = ?5
            WHERE id = ?6
            "#,
        )
        .bind(title.trim())
        .bind(body_markdown.trim())
        .bind(source.trim())
        .bind(confidence)
        .bind(&now)
        .bind(card_id)
        .execute(&self.pool)
        .await?;
        if changed.rows_affected() == 0 {
            bail!("Memory card was not found.");
        }
        self.get_memory_card(card_id)
            .await
            .map(|detail| detail.card)
    }

    pub async fn delete_memory_card(&self, card_id: &str) -> Result<()> {
        let detail = self.get_memory_card(card_id).await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM links WHERE workspace_id = ?1 AND from_type = 'memory_card' AND from_id = ?2")
            .bind(&detail.card.workspace_id)
            .bind(card_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM memory_cards WHERE id = ?1")
            .bind(card_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_memory_cards(&self, workspace_id: &str) -> Result<Vec<MemoryCardSummary>> {
        self.query_memory_cards(workspace_id, None).await
    }

    pub async fn search_memory_cards(
        &self,
        workspace_id: &str,
        query: &str,
    ) -> Result<Vec<MemoryCardSummary>> {
        self.query_memory_cards(workspace_id, Some(query.trim()))
            .await
    }

    pub async fn get_memory_card(&self, card_id: &str) -> Result<MemoryCardDetail> {
        let row = sqlx::query_as::<_, MemoryCardRow>(
            r#"
            SELECT id, workspace_id, title, body_markdown, source, confidence,
                   created_at, updated_at, metadata_json
            FROM memory_cards WHERE id = ?1
            "#,
        )
        .bind(card_id)
        .fetch_one(&self.pool)
        .await?;
        let evidence_rows = sqlx::query_as::<_, MemoryEvidenceRow>(
            r#"
            SELECT
              links.id AS link_id,
              links.to_id AS target_id,
              links.to_type AS target_type,
              artifacts.id AS artifact_id,
              chunks.id AS chunk_id,
              artifacts.title AS title,
              artifacts.path AS path,
              chunks.start_line AS start_line,
              chunks.end_line AS end_line
            FROM links
            LEFT JOIN chunks ON links.to_type = 'chunk' AND chunks.id = links.to_id
            LEFT JOIN artifacts ON artifacts.id = CASE
              WHEN links.to_type = 'artifact' THEN links.to_id
              ELSE chunks.artifact_id
            END
            WHERE links.from_type = 'memory_card' AND links.from_id = ?1
            ORDER BY links.created_at ASC
            "#,
        )
        .bind(card_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(MemoryCardDetail {
            card: MemoryCard::from(row),
            evidence: evidence_rows
                .into_iter()
                .map(MemoryEvidence::from)
                .collect(),
        })
    }

    async fn query_memory_cards(
        &self,
        workspace_id: &str,
        search: Option<&str>,
    ) -> Result<Vec<MemoryCardSummary>> {
        let rows = if let Some(search) = search.filter(|value| !value.is_empty()) {
            sqlx::query_as::<_, MemoryCardSummaryRow>(
                r#"
                SELECT memory_cards.id, memory_cards.workspace_id, memory_cards.title,
                       substr(memory_cards.body_markdown, 1, 220) AS body_excerpt,
                       memory_cards.source, COUNT(links.id) AS evidence_count,
                       memory_cards.created_at, memory_cards.updated_at
                FROM memory_cards
                LEFT JOIN links ON links.from_type = 'memory_card' AND links.from_id = memory_cards.id
                WHERE memory_cards.workspace_id = ?1
                  AND lower(memory_cards.title || ' ' || memory_cards.body_markdown) LIKE '%' || lower(?2) || '%'
                GROUP BY memory_cards.id
                ORDER BY memory_cards.updated_at DESC, memory_cards.title ASC
                "#,
            )
            .bind(workspace_id)
            .bind(search)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, MemoryCardSummaryRow>(
                r#"
                SELECT memory_cards.id, memory_cards.workspace_id, memory_cards.title,
                       substr(memory_cards.body_markdown, 1, 220) AS body_excerpt,
                       memory_cards.source, COUNT(links.id) AS evidence_count,
                       memory_cards.created_at, memory_cards.updated_at
                FROM memory_cards
                LEFT JOIN links ON links.from_type = 'memory_card' AND links.from_id = memory_cards.id
                WHERE memory_cards.workspace_id = ?1
                GROUP BY memory_cards.id
                ORDER BY memory_cards.updated_at DESC, memory_cards.title ASC
                "#,
            )
            .bind(workspace_id)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows.into_iter().map(MemoryCardSummary::from).collect())
    }

    async fn validate_memory_evidence(
        &self,
        workspace_id: &str,
        citation: &Citation,
    ) -> Result<()> {
        let exists = if let Some(chunk_id) = &citation.chunk_id {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM chunks WHERE id = ?1 AND workspace_id = ?2",
            )
            .bind(chunk_id)
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM artifacts WHERE id = ?1 AND workspace_id = ?2",
            )
            .bind(&citation.artifact_id)
            .bind(workspace_id)
            .fetch_one(&self.pool)
            .await?
        };
        if exists == 0 {
            bail!("A cited evidence record is missing from this workspace.");
        }
        Ok(())
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

impl From<UserRow> for StoredUser {
    fn from(row: UserRow) -> Self {
        Self {
            user: SharedUser {
                id: row.id,
                display_name: row.display_name,
                email: Some(row.email),
            },
            password_hash: row.password_hash,
        }
    }
}

impl From<UserProfileRow> for UserProfile {
    fn from(row: UserProfileRow) -> Self {
        Self {
            user: SharedUser {
                id: row.id,
                display_name: row.display_name,
                email: Some(row.email),
            },
            created_at: row.created_at,
            updated_at: row.updated_at,
            last_connected_at: row.last_connected_at,
        }
    }
}

impl From<OrganizationRow> for Organization {
    fn from(row: OrganizationRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl TryFrom<SharedWorkspaceRow> for SharedWorkspace {
    type Error = anyhow::Error;

    fn try_from(row: SharedWorkspaceRow) -> Result<Self> {
        Ok(Self {
            workspace: Workspace {
                id: row.id,
                name: row.name,
                created_at: row.created_at,
                updated_at: row.updated_at,
                settings: serde_json::from_str(&row.settings_json).unwrap_or(Value::Null),
            },
            organization_id: row.organization_id,
            role: workspace_role_from_db(&row.role)?,
        })
    }
}

impl TryFrom<WorkspaceMemberRow> for WorkspaceMember {
    type Error = anyhow::Error;

    fn try_from(row: WorkspaceMemberRow) -> Result<Self> {
        Ok(Self {
            user: SharedUser {
                id: row.id,
                display_name: row.display_name,
                email: Some(row.email),
            },
            role: workspace_role_from_db(&row.role)?,
            joined_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

impl From<WorkspaceActivityRow> for WorkspaceActivityEvent {
    fn from(row: WorkspaceActivityRow) -> Self {
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            actor: row.actor_id.map(|id| SharedUser {
                id,
                display_name: row
                    .actor_display_name
                    .unwrap_or_else(|| "Former member".to_owned()),
                email: row.actor_email,
            }),
            action: row.action,
            subject_type: row.subject_type,
            subject_id: row.subject_id,
            summary: row.summary,
            created_at: row.created_at,
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
        let mut metadata = serde_json::from_str::<Value>(&row.metadata_json)
            .unwrap_or_else(|_| Value::Object(Default::default()));
        let api_key = metadata
            .get("api_key")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let Some(object) = metadata.as_object_mut() {
            object.remove("api_key");
        }
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            provider_type: row.provider_type,
            name: row.name,
            base_url: row.base_url,
            model: row.model,
            embedding_model: row.embedding_model,
            enabled: row.enabled,
            metadata,
            api_key,
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

impl From<MemoryCardRow> for MemoryCard {
    fn from(row: MemoryCardRow) -> Self {
        let metadata = serde_json::from_str(&row.metadata_json)
            .unwrap_or_else(|_| Value::Object(Default::default()));
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            title: row.title,
            body_markdown: row.body_markdown,
            source: row.source,
            confidence: row.confidence,
            created_at: row.created_at,
            updated_at: row.updated_at,
            metadata,
        }
    }
}

impl From<MemoryCardSummaryRow> for MemoryCardSummary {
    fn from(row: MemoryCardSummaryRow) -> Self {
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            title: row.title,
            body_excerpt: row.body_excerpt,
            source: row.source,
            evidence_count: row.evidence_count,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<MemoryEvidenceRow> for MemoryEvidence {
    fn from(row: MemoryEvidenceRow) -> Self {
        let exists = row.artifact_id.is_some();
        Self {
            link_id: row.link_id,
            target_id: row.target_id,
            target_type: row.target_type,
            artifact_id: row.artifact_id,
            chunk_id: row.chunk_id,
            title: row.title,
            path: row.path,
            start_line: row.start_line,
            end_line: row.end_line,
            exists,
        }
    }
}

fn normalize_email(value: &str) -> Result<String> {
    let email = value.trim().to_ascii_lowercase();
    if email.len() > 254 || !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
        bail!("A valid email address is required.");
    }
    Ok(email)
}

fn require_name(value: &str, field: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 120 {
        bail!("{field} must be between 1 and 120 characters.");
    }
    Ok(value.to_owned())
}

fn workspace_role_from_db(value: &str) -> Result<WorkspaceRole> {
    match value {
        "owner" => Ok(WorkspaceRole::Owner),
        "admin" => Ok(WorkspaceRole::Admin),
        "member" => Ok(WorkspaceRole::Member),
        "viewer" => Ok(WorkspaceRole::Viewer),
        _ => bail!("Unknown workspace role: {value}"),
    }
}

fn workspace_role_to_db(role: &WorkspaceRole) -> &'static str {
    match role {
        WorkspaceRole::Owner => "owner",
        WorkspaceRole::Admin => "admin",
        WorkspaceRole::Member => "member",
        WorkspaceRole::Viewer => "viewer",
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

pub fn encode_embedding(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

pub fn decode_embedding(bytes: &[u8]) -> Option<Vec<f32>> {
    if bytes.len() % 4 != 0 {
        return None;
    }
    Some(
        bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect(),
    )
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f64> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }
    let (dot, left_norm, right_norm) = left.iter().zip(right).fold(
        (0.0_f64, 0.0_f64, 0.0_f64),
        |(dot, left_norm, right_norm), (a, b)| {
            let a = f64::from(*a);
            let b = f64::from(*b);
            (dot + a * b, left_norm + a * a, right_norm + b * b)
        },
    );
    (left_norm > 0.0 && right_norm > 0.0).then_some(dot / (left_norm.sqrt() * right_norm.sqrt()))
}

#[cfg(test)]
mod tests {
    use super::{decode_embedding, encode_embedding, NewArtifact, StorageConfig, StorageEngine};
    use repomemo_domain::{
        ArtifactType, Chunk, Citation, SearchRequest, SourceType, Symbol, SymbolKind,
    };
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn embedding_serialization_round_trips() {
        let values = vec![0.25_f32, -1.0, 3.5];
        assert_eq!(decode_embedding(&encode_embedding(&values)), Some(values));
        assert_eq!(decode_embedding(&[0, 1]), None);
    }

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
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn memory_cards_link_evidence_and_are_searchable() {
        let data_dir = std::env::temp_dir().join(format!(
            "repomemo-memory-test-{}",
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
        let workspace = storage.create_workspace("Memory test").await.unwrap();
        let source = storage
            .create_or_get_source(&workspace.id, SourceType::Manual, "Notes", None)
            .await
            .unwrap();
        let bytes = b"Local evidence for a durable memory card.";
        let content_hash = StorageEngine::content_hash(bytes);
        storage
            .store_blob(&content_hash, bytes, Some("text/plain"))
            .await
            .unwrap();
        let artifact = storage
            .store_artifact(NewArtifact {
                workspace_id: workspace.id.clone(),
                source_id: source.id,
                artifact_type: ArtifactType::Note,
                title: "Evidence note".to_owned(),
                path: "evidence.txt".to_owned(),
                content_hash,
                mime_type: Some("text/plain".to_owned()),
                language: Some("Text".to_owned()),
                size_bytes: bytes.len() as i64,
                metadata: json!({}),
            })
            .await
            .unwrap()
            .artifact;
        let card = storage
            .create_memory_card(
                &workspace.id,
                "Keep local evidence",
                "Evidence remains inspectable.",
                "manual",
                None,
                &[Citation {
                    artifact_id: artifact.id.clone(),
                    chunk_id: None,
                    title: artifact.title.clone(),
                    path: artifact.path.clone(),
                    start_line: None,
                    end_line: None,
                    confidence: None,
                }],
            )
            .await
            .unwrap();
        let matches = storage
            .search_memory_cards(&workspace.id, "inspectable")
            .await
            .unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].evidence_count, 1);
        let detail = storage.get_memory_card(&card.id).await.unwrap();
        assert_eq!(detail.evidence.len(), 1);
        assert!(detail.evidence[0].exists);
        let updated = storage
            .update_memory_card(
                &card.id,
                "Keep verified evidence",
                "Evidence remains local and inspectable.",
                "manual",
                None,
            )
            .await
            .unwrap();
        assert_eq!(updated.title, "Keep verified evidence");

        storage.pool.close().await;
        drop(storage);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
