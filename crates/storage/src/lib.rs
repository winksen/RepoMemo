use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use repomemo_domain::{
    AgentConversation, AgentMessage, AgentReply, AgentTurn, ArtifactComment, ArtifactDetail, ArtifactIndexFailure,
    ArtifactLifecycle, ArtifactLifecycleEvent, ArtifactSummary, ArtifactType, ArtifactVersion, Chunk, Citation,
    CollaborationTask, Folder, IndexingJobStatus, MemoryCard, MemoryCardDetail, MemoryCardSummary, MemoryEvidence,
    MemoryReviewReason, Organization, OrganizationMember, OrganizationRole, ProviderSettings, SavedSearch,
    SearchRequest, SearchResult, SharedNotification, SharedUser, SharedWorkspace, Source, SourceType, Symbol,
    SymbolKind, SymbolSearchResult, TaskChecklistItem, Workspace, WorkspaceActivityEvent, WorkspaceMember,
    WorkspaceMembership, WorkspaceOverview, WorkspaceRole, MAX_FOLDER_DEPTH,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow, SqliteSynchronous};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};
use uuid::Uuid;

mod access;
mod accounts;
mod activity;
mod archive;
mod archive_import;
mod artifacts;
mod collaboration;
mod connectors;
mod credentials;
mod environment;
mod identity;
mod jobs;
mod mail;
mod memory;
mod mentions;
mod organizations;
mod providers;
mod repo;
mod search;
mod secrets;
mod setup;
mod staging;
mod stats;
mod system;
mod users;
mod vectors;
pub use access::{PasswordReset, PersonalTokenGrant, StoredInvitation, PERSONAL_TOKEN_PREFIX};
pub use accounts::{Anonymization, Deactivation, OwnedPlace, FORMER_MEMBER};
pub use archive_import::{ImportedWorkspace, PendingPassage};
pub use connectors::{ConnectorItemOutcome, ConnectorItemWrite};
pub use credentials::{RepoCredentialInfo, RepoSecret, RepoWebhookInfo};
pub use environment::{inspect_environment, inspect_environment_split, EnvironmentState};
pub use identity::IdentityProvider;
pub use mail::{DigestCandidate, DigestNotice, MailOutboxCounts, MailSettingsUpdate, QueuedMail, StoredMailSettings};
pub use mentions::{MentionRow, MentionWrite, WorkspaceFileIndex};
pub use organizations::StarterSetup;
pub use repo::RepoLastChange;
pub use repo::{RepoFileRecord, RepoFileWrite};
mod newer;
use secrets::SecretBox;
pub use secrets::{restrict_to_owner, KEY_FILE_NAME as SECRET_KEY_FILE_NAME, MIN_MASTER_KEY_CHARS};
pub use setup::SetupState;
pub use staging::{StagedBlob, StagedBlobWriter};
pub use stats::{ArtifactQuery, ArtifactStats, DashboardCount, DashboardCounts, KindFilter};
pub use system::{StoredSystemSetting, UserAccess};
pub use users::RefreshOutcome;

/// The database file inside an environment folder.
const DATABASE_FILE: &str = "repomemo.sqlite";
/// The folder inside a separate blob folder where received files wait (V0.4.4).
pub(crate) const STAGING_DIR: &str = ".incoming";
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// How long SQLite waits for a competing writer before failing with
/// `database is locked`. Background indexing, embedding and syncs write
/// concurrently with requests, so the default of a few seconds is too short.
const BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// Observer invoked whenever a job row is created or updated. The shared
/// server registers one to fan events into its SSE bus; local desktop use
/// keeps the default no-op observer and pays no per-write cost.
pub trait JobObserver: Send + Sync {
    fn on_job_changed(&self, job: &IndexingJobStatus);
}

/// Observer invoked after each workspace activity insert. The server uses
/// it to push live-updates to SSE subscribers.
pub trait ActivityObserver: Send + Sync {
    fn on_activity_recorded(&self, event: &WorkspaceActivityEvent);
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub data_dir: PathBuf,
    /// Where the database file lives (V0.4.4); `None` keeps it in `data_dir`.
    pub db_dir: Option<PathBuf>,
    /// Where the stored files live; `None` keeps them in `data_dir/blobs`. Received files wait in a folder
    /// of their own beside them, so that storing one is a rename on one volume.
    pub blob_dir: Option<PathBuf>,
    /// Where backups are made and kept; `None` keeps them in `data_dir/backups`.
    pub backup_dir: Option<PathBuf>,
    /// Key material used to encrypt secrets (AI provider API keys) at rest.
    /// When `None`, a random key is kept in `secret.key` in the data
    /// directory, created on first use.
    pub master_key: Option<String>,
    /// Connections that serve reads (search, lists, pages). Writes go through
    /// one connection of their own, so a burst of reads never waits for a
    /// write lock and writers never queue behind each other inside SQLite.
    pub read_connections: u32,
    /// Memory the in-memory vector indexes may hold, in MiB; 0 turns them off
    /// and meaning searches scan the database.
    pub vector_index_mb: u32,
    /// The page cache of each read connection, in MiB (SQLite's own is about 2 MiB); 0 keeps SQLite's.
    pub read_cache_mb: u32,
    /// How much of the database each read connection may map into memory, in MiB; 0 turns it off.
    pub read_mmap_mb: u32,
}

impl StorageConfig {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            db_dir: None,
            blob_dir: None,
            backup_dir: None,
            master_key: None,
            read_connections: DEFAULT_READ_CONNECTIONS,
            vector_index_mb: DEFAULT_VECTOR_INDEX_MB,
            read_cache_mb: DEFAULT_READ_CACHE_MB,
            read_mmap_mb: DEFAULT_READ_MMAP_MB,
        }
    }
}

/// The page cache of each read connection unless the operator says otherwise.
pub const DEFAULT_READ_CACHE_MB: u32 = 32;

/// Memory-mapped reads of each read connection unless the operator says otherwise.
pub const DEFAULT_READ_MMAP_MB: u32 = 256;

/// How many connections serve reads unless the operator says otherwise.
pub const DEFAULT_READ_CONNECTIONS: u32 = 4;

/// Memory for vector indexes unless the operator says otherwise: a workspace
/// of 50,000 passages with 768-dimensional vectors takes about 150 MiB.
pub const DEFAULT_VECTOR_INDEX_MB: u32 = 512;

/// What a blob garbage-collection pass removed.
#[derive(Debug, Clone, Default)]
pub struct BlobCollection {
    /// Content hashes whose blob was deleted.
    pub removed: Vec<String>,
    pub bytes: i64,
}

#[derive(Clone)]
pub struct StorageEngine {
    /// The one connection that writes (and runs every transaction).
    pool: SqlitePool,
    /// Read-only connections for everything that only looks.
    read_pool: SqlitePool,
    data_dir: PathBuf,
    db_dir: PathBuf,
    blob_dir: PathBuf,
    /// Where files being received wait: always on the volume of `blob_dir`.
    staging_dir: PathBuf,
    backup_dir: PathBuf,
    secrets: SecretBox,
    /// Serialises blob writes with blob garbage collection, so a blob that is
    /// being stored again is never deleted underneath the writer.
    blob_lock: Arc<tokio::sync::Mutex<()>>,
    /// Passage vectors held in memory for meaning search.
    vectors: Arc<vectors::VectorCache>,
    /// What was written to passages and vectors, for derived results.
    vector_changes: Arc<vectors::VectorChanges>,
    job_observer: Arc<RwLock<Option<Arc<dyn JobObserver>>>>,
    activity_observer: Arc<RwLock<Option<Arc<dyn ActivityObserver>>>>,
}

impl std::fmt::Debug for StorageEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StorageEngine")
            .field("data_dir", &self.data_dir)
            .field("blob_dir", &self.blob_dir)
            .finish_non_exhaustive()
    }
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
    role: String,
    created_at: String,
    updated_at: String,
    #[sqlx(default)]
    guest: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct OrganizationMemberRow {
    id: String,
    email: String,
    display_name: String,
    role: String,
    created_at: String,
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
struct CollaborationTaskRow {
    id: String,
    workspace_id: String,
    title: String,
    description: String,
    status: String,
    priority: String,
    assignee_id: Option<String>,
    assignee_email: Option<String>,
    assignee_display_name: Option<String>,
    creator_id: String,
    creator_email: String,
    creator_display_name: String,
    artifact_id: Option<String>,
    due_at: Option<String>,
    completed_at: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct SavedSearchRow {
    id: String,
    workspace_id: String,
    name: String,
    query: String,
    artifact_types_json: String,
    languages_json: String,
    source_ids_json: String,
    result_limit: i64,
    shared: bool,
    creator_id: String,
    creator_email: String,
    creator_display_name: String,
    created_at: String,
    updated_at: String,
}

/// A chunk waiting for an embedding, with the context that makes its vector
/// more specific than the bare text.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EmbeddingCandidate {
    pub chunk_id: String,
    pub title: String,
    pub heading_path: Option<String>,
    pub text: String,
}

const AGENT_CONVERSATION_SELECT: &str = "SELECT conversation.id, conversation.workspace_id, conversation.title, (SELECT COUNT(*) FROM assistant_turns turn WHERE turn.conversation_id = conversation.id) AS turn_count, conversation.created_at, conversation.updated_at FROM assistant_conversations conversation";

#[derive(Debug, sqlx::FromRow)]
struct AgentConversationRow {
    id: String,
    workspace_id: String,
    title: String,
    turn_count: i64,
    created_at: String,
    updated_at: String,
}

impl From<AgentConversationRow> for AgentConversation {
    fn from(row: AgentConversationRow) -> Self {
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            title: row.title,
            turn_count: row.turn_count,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct AgentTurnRow {
    id: String,
    conversation_id: String,
    position: i64,
    label: String,
    request_json: String,
    reply_json: String,
    created_at: String,
}

impl TryFrom<AgentTurnRow> for AgentTurn {
    type Error = anyhow::Error;

    fn try_from(row: AgentTurnRow) -> Result<Self> {
        Ok(Self {
            id: row.id,
            conversation_id: row.conversation_id,
            position: row.position,
            label: row.label,
            request: serde_json::from_str(&row.request_json).context("stored assistant request is not valid JSON")?,
            reply: serde_json::from_str(&row.reply_json).context("stored assistant reply is not valid JSON")?,
            created_at: row.created_at,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct TaskChecklistItemRow {
    id: String,
    task_id: String,
    workspace_id: String,
    body: String,
    position: i64,
    completed_at: Option<String>,
    completed_by_id: Option<String>,
    completed_by_email: Option<String>,
    completed_by_display_name: Option<String>,
    creator_id: String,
    creator_email: String,
    creator_display_name: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ArtifactCommentRow {
    id: String,
    workspace_id: String,
    artifact_id: String,
    author_id: String,
    author_email: String,
    author_display_name: String,
    body: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ArtifactLifecycleRow {
    artifact_id: String,
    workspace_id: String,
    status: String,
    owner_id: Option<String>,
    owner_email: Option<String>,
    owner_display_name: Option<String>,
    review_note: String,
    reviewer_id: Option<String>,
    reviewer_email: Option<String>,
    reviewer_display_name: Option<String>,
    reviewed_at: Option<String>,
    superseded_by_artifact_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ArtifactLifecycleEventRow {
    id: String,
    artifact_id: String,
    actor_id: Option<String>,
    actor_email: Option<String>,
    actor_display_name: Option<String>,
    action: String,
    detail: String,
    created_at: String,
}

#[derive(Debug, sqlx::FromRow)]
struct SharedNotificationRow {
    id: String,
    workspace_id: Option<String>,
    notification_type: String,
    title: String,
    body: String,
    href: String,
    read_at: Option<String>,
    created_at: String,
}

#[derive(Debug, Clone)]
pub struct NewCollaborationTask {
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assignee_user_id: Option<String>,
    pub artifact_id: Option<String>,
    pub due_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChunkMention {
    pub chunk_id: String,
    pub artifact_id: String,
    pub text: String,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NewSavedSearch {
    pub name: String,
    pub query: String,
    pub artifact_types: Vec<ArtifactType>,
    pub languages: Vec<String>,
    pub source_ids: Vec<String>,
    pub result_limit: i64,
}

#[derive(Debug, Clone)]
pub struct NewTaskChecklistItem {
    pub body: String,
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceCollaborationCounts {
    pub open_tasks: i64,
    pub in_progress_tasks: i64,
    pub blocked_tasks: i64,
    pub completed_tasks: i64,
    pub overdue_tasks: i64,
    pub comments: i64,
}

/// Figures for the Overview that no other count carries (V0.3.18).
#[derive(Debug, Clone, Default)]
pub struct WorkspaceReviewFigures {
    /// Linked git repositories, and how many of them finished their last sync.
    pub repositories: i64,
    pub repositories_in_sync: i64,
    /// Files whose lifecycle was reviewed since the day asked for.
    pub reviewed_recently: i64,
    /// Files that repeat the exact content of another file of the workspace (the second and later copies).
    pub duplicate_files: i64,
}

#[derive(Debug, Clone)]
pub struct SaveArtifactLifecycle {
    pub status: String,
    pub owner_user_id: Option<String>,
    pub review_note: String,
    pub superseded_by_artifact_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewSharedNotification {
    pub user_id: String,
    pub workspace_id: Option<String>,
    pub notification_type: String,
    pub title: String,
    pub body: String,
    pub href: String,
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
    index_version: i64,
    source_metadata_json: String,
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
    kind: String,
    status: String,
    stage: String,
    progress_current: i64,
    progress_total: Option<i64>,
    error_message: Option<String>,
    cancel_requested: i64,
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
    source_id: Option<String>,
    heading_path: Option<String>,
    review_status: Option<String>,
    start_line: Option<i64>,
    end_line: Option<i64>,
    commit: Option<String>,
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

        let db_dir = config.db_dir.clone().unwrap_or_else(|| config.data_dir.clone());
        tokio::fs::create_dir_all(&db_dir).await?;
        let blob_dir = config.blob_dir.clone().unwrap_or_else(|| config.data_dir.join("blobs"));
        tokio::fs::create_dir_all(&blob_dir).await?;
        // Received files wait beside the stored ones when those are elsewhere, so storing one stays a rename.
        let staging_dir = match config.blob_dir {
            Some(_) => blob_dir.join(STAGING_DIR),
            None => config.data_dir.join("tmp"),
        };
        let backup_dir = config
            .backup_dir
            .clone()
            .unwrap_or_else(|| config.data_dir.join("backups"));
        // Files that were being received when the last process stopped.
        let _ = tokio::fs::remove_dir_all(&staging_dir).await;

        let secrets = SecretBox::load(&config.data_dir, config.master_key.as_deref())?;

        let db_path = db_dir.join(DATABASE_FILE);
        // WAL with `synchronous = NORMAL` never corrupts the database; at worst
        // the last transactions before a power loss are rolled back.
        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(BUSY_TIMEOUT);

        // SQLite lets one writer in at a time anyway: a single connection
        // makes writers wait in line here instead of failing with "database
        // is locked", and the readers below never wait for it (WAL).
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone())
            .await?;

        // A database from a newer release is refused before anything is changed (V0.4.36).
        newer::refuse_newer_database(&pool, &db_dir).await?;
        MIGRATOR.run(&pool).await?;
        newer::leave_marker(&db_dir);

        // The readers do the searching, so they get a larger page cache, memory-mapped reads and
        // temporary tables in memory; the connection that writes keeps SQLite's defaults.
        let mut read_options = options
            .read_only(true)
            .create_if_missing(false)
            .pragma("temp_store", "MEMORY");
        if config.read_cache_mb > 0 {
            // A negative `cache_size` is a size in KiB.
            read_options = read_options.pragma("cache_size", format!("-{}", u64::from(config.read_cache_mb) * 1024));
        }
        if config.read_mmap_mb > 0 {
            read_options = read_options.pragma("mmap_size", (u64::from(config.read_mmap_mb) * 1024 * 1024).to_string());
        }
        let read_pool = SqlitePoolOptions::new()
            .max_connections(config.read_connections.clamp(1, 64))
            .connect_with(read_options)
            .await?;

        let engine = Self {
            pool,
            read_pool,
            data_dir: config.data_dir,
            db_dir,
            blob_dir,
            staging_dir,
            backup_dir,
            secrets,
            blob_lock: Arc::new(tokio::sync::Mutex::new(())),
            vectors: Arc::new(vectors::VectorCache::new(config.vector_index_mb)),
            vector_changes: Arc::new(vectors::VectorChanges::default()),
            job_observer: Arc::new(RwLock::new(None)),
            activity_observer: Arc::new(RwLock::new(None)),
        };
        let sealed = engine.seal_plaintext_provider_keys().await?;
        if sealed > 0 {
            tracing::info!(count = sealed, "Encrypted AI provider API keys stored in plain text");
        }
        Ok(engine)
    }

    /// Folds the write-ahead log back and closes every connection, so the
    /// environment folder can be left, moved or attached again. Clones of
    /// this engine fail afterwards.
    pub async fn close(&self) {
        if let Err(error) = self.optimize().await {
            tracing::warn!(error = %error, "Could not fold the write-ahead log before closing");
        }
        self.read_pool.close().await;
        self.pool.close().await;
    }

    /// SQLite's full integrity check, used by isolated restore drills.
    pub async fn check_integrity(&self) -> Result<()> {
        let verdict: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(self.reader())
            .await?;
        if verdict != "ok" {
            bail!("Restored database failed integrity_check: {verdict}");
        }
        Ok(())
    }

    /// A connection for statements that only read.
    fn reader(&self) -> &SqlitePool {
        &self.read_pool
    }

    /// Checks that the database answers, for readiness probes.
    pub async fn ping(&self) -> Result<()> {
        sqlx::query_scalar::<_, i64>("SELECT 1")
            .fetch_one(self.reader())
            .await?;
        Ok(())
    }

    /// Lets SQLite refresh its query-planner statistics and folds the
    /// write-ahead log back into the database file, so the log does not grow
    /// without bound on a server that is never idle. Safe to run at any time.
    pub async fn optimize(&self) -> Result<()> {
        sqlx::query("PRAGMA optimize").execute(&self.pool).await?;
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// The folder that holds the database file.
    pub fn db_dir(&self) -> &Path {
        &self.db_dir
    }

    /// The folder backups are made in.
    pub fn backup_dir(&self) -> &Path {
        &self.backup_dir
    }

    pub fn blob_dir(&self) -> &Path {
        &self.blob_dir
    }

    /// Install a job observer that receives every job insert/update. Replaces
    /// any previously registered observer. The default is unset (no-op).
    pub fn set_job_observer(&self, observer: Arc<dyn JobObserver>) {
        if let Ok(mut guard) = self.job_observer.write() {
            *guard = Some(observer);
        }
    }

    /// Install an activity observer that receives every workspace activity
    /// insert. Replaces any previously registered observer.
    pub fn set_activity_observer(&self, observer: Arc<dyn ActivityObserver>) {
        if let Ok(mut guard) = self.activity_observer.write() {
            *guard = Some(observer);
        }
    }

    fn emit_job(&self, job: &IndexingJobStatus) {
        if let Ok(guard) = self.job_observer.read() {
            if let Some(observer) = guard.as_ref() {
                observer.on_job_changed(job);
            }
        }
    }

    fn emit_activity(&self, event: &WorkspaceActivityEvent) {
        if let Ok(guard) = self.activity_observer.read() {
            if let Some(observer) = guard.as_ref() {
                observer.on_activity_recorded(event);
            }
        }
    }

    pub fn content_hash(bytes: &[u8]) -> String {
        let digest = Sha256::digest(bytes);
        hex::encode(digest)
    }

    pub fn blob_path_for_hash(&self, content_hash: &str) -> PathBuf {
        self.blob_path(content_hash)
    }

    async fn count_for_workspace(&self, table: &str, workspace_id: &str) -> Result<i64> {
        let sql = format!("SELECT COUNT(*) FROM {table} WHERE workspace_id = ?1");
        let count = sqlx::query_scalar::<_, i64>(&sql)
            .bind(workspace_id)
            .fetch_one(self.reader())
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
        format!("blobs/{}/{}/{}", &content_hash[0..2], &content_hash[2..4], content_hash)
    }
}

impl From<WorkspaceRow> for Workspace {
    fn from(row: WorkspaceRow) -> Self {
        let settings = serde_json::from_str(&row.settings_json).unwrap_or_else(|_| Value::Object(Default::default()));

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
            role: organization_role_from_db(&row.role).unwrap_or(OrganizationRole::Member),
            created_at: row.created_at,
            updated_at: row.updated_at,
            guest: row.guest,
        }
    }
}

impl TryFrom<OrganizationMemberRow> for OrganizationMember {
    type Error = anyhow::Error;

    fn try_from(row: OrganizationMemberRow) -> Result<Self> {
        Ok(Self {
            user: SharedUser {
                id: row.id,
                display_name: row.display_name,
                email: Some(row.email),
            },
            role: organization_role_from_db(&row.role)?,
            joined_at: row.created_at,
        })
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
                display_name: row.actor_display_name.unwrap_or_else(|| "Former member".to_owned()),
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

impl From<CollaborationTaskRow> for CollaborationTask {
    fn from(row: CollaborationTaskRow) -> Self {
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            title: row.title,
            description: row.description,
            status: row.status,
            priority: row.priority,
            assignee: row.assignee_id.map(|id| SharedUser {
                id,
                display_name: row.assignee_display_name.unwrap_or_else(|| "Member".to_owned()),
                email: row.assignee_email,
            }),
            created_by: SharedUser {
                id: row.creator_id,
                display_name: row.creator_display_name,
                email: Some(row.creator_email),
            },
            artifact_id: row.artifact_id,
            due_at: row.due_at,
            completed_at: row.completed_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<ArtifactCommentRow> for ArtifactComment {
    fn from(row: ArtifactCommentRow) -> Self {
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            artifact_id: row.artifact_id,
            author: SharedUser {
                id: row.author_id,
                display_name: row.author_display_name,
                email: Some(row.author_email),
            },
            body: row.body,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<SavedSearchRow> for SavedSearch {
    fn from(row: SavedSearchRow) -> Self {
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            name: row.name,
            query: row.query,
            artifact_types: serde_json::from_str(&row.artifact_types_json).unwrap_or_default(),
            languages: serde_json::from_str(&row.languages_json).unwrap_or_default(),
            source_ids: serde_json::from_str(&row.source_ids_json).unwrap_or_default(),
            result_limit: row.result_limit,
            shared: row.shared,
            created_by: SharedUser {
                id: row.creator_id,
                display_name: row.creator_display_name,
                email: Some(row.creator_email),
            },
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<TaskChecklistItemRow> for TaskChecklistItem {
    fn from(row: TaskChecklistItemRow) -> Self {
        Self {
            id: row.id,
            task_id: row.task_id,
            workspace_id: row.workspace_id,
            body: row.body,
            position: row.position,
            completed_at: row.completed_at,
            completed_by: row.completed_by_id.map(|id| SharedUser {
                id,
                display_name: row.completed_by_display_name.unwrap_or_else(|| "Member".to_owned()),
                email: row.completed_by_email,
            }),
            created_by: SharedUser {
                id: row.creator_id,
                display_name: row.creator_display_name,
                email: Some(row.creator_email),
            },
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<ArtifactLifecycleRow> for ArtifactLifecycle {
    fn from(row: ArtifactLifecycleRow) -> Self {
        Self {
            artifact_id: row.artifact_id,
            workspace_id: row.workspace_id,
            status: row.status,
            owner: row.owner_id.map(|id| SharedUser {
                id,
                display_name: row.owner_display_name.unwrap_or_else(|| "Member".to_owned()),
                email: row.owner_email,
            }),
            review_note: row.review_note,
            reviewed_by: row.reviewer_id.map(|id| SharedUser {
                id,
                display_name: row.reviewer_display_name.unwrap_or_else(|| "Member".to_owned()),
                email: row.reviewer_email,
            }),
            reviewed_at: row.reviewed_at,
            superseded_by_artifact_id: row.superseded_by_artifact_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

impl From<ArtifactLifecycleEventRow> for ArtifactLifecycleEvent {
    fn from(row: ArtifactLifecycleEventRow) -> Self {
        Self {
            id: row.id,
            artifact_id: row.artifact_id,
            actor: row.actor_id.map(|id| SharedUser {
                id,
                display_name: row.actor_display_name.unwrap_or_else(|| "Member".to_owned()),
                email: row.actor_email,
            }),
            action: row.action,
            detail: row.detail,
            created_at: row.created_at,
        }
    }
}

impl From<SharedNotificationRow> for SharedNotification {
    fn from(row: SharedNotificationRow) -> Self {
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            notification_type: row.notification_type,
            title: row.title,
            body: row.body,
            href: row.href,
            read_at: row.read_at,
            created_at: row.created_at,
        }
    }
}

impl From<SourceRow> for Source {
    fn from(row: SourceRow) -> Self {
        let metadata = serde_json::from_str(&row.metadata_json).unwrap_or_else(|_| Value::Object(Default::default()));

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

fn validate_folder_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        repomemo_domain::bail_invalid!("Folder name is required.");
    }
    if name.chars().count() > 80 {
        repomemo_domain::bail_invalid!("Folder names must be 80 characters or fewer.");
    }
    if name.contains(['/', '\\']) {
        repomemo_domain::bail_invalid!("Folder names cannot contain slashes.");
    }
    Ok(name)
}

fn folder_from_row(row: &SqliteRow) -> Folder {
    Folder {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        parent_id: row.get("parent_id"),
        name: row.get("name"),
        created_at: row.get("created_at"),
        file_count: row.try_get("file_count").unwrap_or(0),
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
        let metadata = serde_json::from_str(&row.metadata_json).unwrap_or_else(|_| Value::Object(Default::default()));

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
        let metadata = serde_json::from_str(&row.metadata_json).unwrap_or_else(|_| Value::Object(Default::default()));
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

/// Where provider API keys were kept in plain text before encryption at rest.
const PROVIDER_API_KEY_FIELD: &str = "api_key";
/// Where the encrypted provider API key is kept in `metadata_json`.
const PROVIDER_SEALED_API_KEY_FIELD: &str = "api_key_sealed";

fn provider_secret_context(provider_id: &str) -> String {
    format!("provider_settings:{provider_id}:api_key")
}

/// Whether two versions of a provider send requests to the same service.
fn same_provider_endpoint(existing: &ProviderSettings, updated: &ProviderSettings) -> bool {
    let endpoint = |settings: &ProviderSettings| {
        settings
            .base_url
            .as_deref()
            .unwrap_or_default()
            .trim()
            .trim_end_matches('/')
            .to_ascii_lowercase()
    };
    existing.provider_type == updated.provider_type && endpoint(existing) == endpoint(updated)
}

impl From<IndexingJobRow> for IndexingJobStatus {
    fn from(row: IndexingJobRow) -> Self {
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            source_id: row.source_id,
            kind: row.kind,
            status: row.status,
            stage: row.stage,
            progress_current: row.progress_current,
            progress_total: row.progress_total,
            error_message: row.error_message,
            cancel_requested: row.cancel_requested != 0,
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
            status: None,
            title: row.title,
            path: row.path,
            artifact_type: artifact_type_from_db(&row.artifact_type),
            language: row.language,
            snippet: row.snippet,
            start_line: row.start_line,
            end_line: row.end_line,
            score: row.score,
            source_name: row.source_name,
            match_kind: "keyword".to_owned(),
            commit: None,
            last_author: None,
            last_changed_at: None,
        }
    }
}

impl From<MemoryCardRow> for MemoryCard {
    fn from(row: MemoryCardRow) -> Self {
        let metadata = serde_json::from_str(&row.metadata_json).unwrap_or_else(|_| Value::Object(Default::default()));
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
            needs_review: false,
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
            source_id: row.source_id,
            heading_path: row.heading_path,
            review_status: row.review_status,
            start_line: row.start_line,
            end_line: row.end_line,
            exists,
            commit: row.commit,
        }
    }
}

fn normalize_email(value: &str) -> Result<String> {
    let email = value.trim().to_ascii_lowercase();
    if email.len() > 254 || !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
        repomemo_domain::bail_invalid!("A valid email address is required.");
    }
    Ok(email)
}

/// A character that changes how text is drawn or read without being seen: any control character, and the
/// bidirectional marks and overrides that make `gpj.exe` look like `exe.jpg` (V0.2.146, review E13).
fn is_hidden_character(character: char) -> bool {
    character.is_control()
        || matches!(
            character,
            '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
        )
}

/// A name a person gave something new: 1 to 120 characters, none of them hidden. Names stored before
/// V0.2.146 are left as they are.
fn require_name(value: &str, field: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 120 {
        repomemo_domain::bail_invalid!("{field} must be between 1 and 120 characters.");
    }
    if value.chars().any(is_hidden_character) {
        repomemo_domain::bail_invalid!("{field} must not contain control or text-direction characters.");
    }
    Ok(value.to_owned())
}

/// A title taken from a file name or a pasted text: only the length is checked, as before, because a
/// repository can hold a file name this release would not accept for a person's name.
fn require_title(value: &str, field: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 120 {
        repomemo_domain::bail_invalid!("{field} must be between 1 and 120 characters.");
    }
    Ok(value.to_owned())
}

/// An address given for a new account, invitation or change: one `@` between two non-empty parts, no
/// white space, control or text-direction characters. Looking an address up stays as lenient as before
/// ([`normalize_email`]), so accounts made earlier still sign in.
fn normalize_new_email(value: &str) -> Result<String> {
    let email = normalize_email(value)?;
    let (local, domain) = email.split_once('@').unwrap_or_default();
    if local.is_empty()
        || domain.is_empty()
        || domain.contains('@')
        || email
            .chars()
            .any(|character| character.is_whitespace() || is_hidden_character(character))
    {
        repomemo_domain::bail_invalid!("A valid email address is required.");
    }
    Ok(email)
}

fn workspace_role_from_db(value: &str) -> Result<WorkspaceRole> {
    match value {
        "owner" => Ok(WorkspaceRole::Owner),
        "admin" => Ok(WorkspaceRole::Admin),
        "member" => Ok(WorkspaceRole::Member),
        "viewer" => Ok(WorkspaceRole::Viewer),
        _ => repomemo_domain::bail_invalid!("Unknown workspace role: {value}"),
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

fn organization_role_from_db(value: &str) -> Result<OrganizationRole> {
    match value {
        "owner" => Ok(OrganizationRole::Owner),
        "admin" => Ok(OrganizationRole::Admin),
        "member" => Ok(OrganizationRole::Member),
        _ => repomemo_domain::bail_invalid!("Unknown organization role: {value}"),
    }
}

fn organization_role_to_db(role: &OrganizationRole) -> &'static str {
    match role {
        OrganizationRole::Owner => "owner",
        OrganizationRole::Admin => "admin",
        OrganizationRole::Member => "member",
    }
}

fn organization_role_to_workspace_role(role: &OrganizationRole) -> WorkspaceRole {
    match role {
        OrganizationRole::Owner => WorkspaceRole::Owner,
        OrganizationRole::Admin => WorkspaceRole::Admin,
        OrganizationRole::Member => WorkspaceRole::Member,
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
        ArtifactType::Repository => "repository",
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
        "repository" => ArtifactType::Repository,
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
        SymbolKind::Constant => "constant",
        SymbolKind::Variable => "variable",
        SymbolKind::Field => "field",
        SymbolKind::Property => "property",
        SymbolKind::TypeAlias => "type_alias",
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
        "constant" => SymbolKind::Constant,
        "variable" => SymbolKind::Variable,
        "field" => SymbolKind::Field,
        "property" => SymbolKind::Property,
        "type_alias" => SymbolKind::TypeAlias,
        _ => SymbolKind::Function,
    }
}

pub fn encode_embedding(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|value| value.to_le_bytes()).collect()
}

/// Appends the optional type, language and source filters of a search, for
/// queries that join `artifacts`.
/// The filters every search shares. `folders` is the folder of the request and
/// the ones below it, already resolved (empty: no folder filter).
fn push_search_filters<'a>(query: &mut QueryBuilder<'a, Sqlite>, request: &'a SearchRequest, folders: &'a [String]) {
    // Retired evidence is hidden unless a state is asked for by name: an
    // outdated or superseded file must not be served as current.
    query.push(
        " AND COALESCE((SELECT status FROM artifact_lifecycle WHERE artifact_lifecycle.artifact_id = artifacts.id), 'active')",
    );
    if request.lifecycle_statuses.is_empty() {
        query.push(" NOT IN ('outdated', 'superseded')");
    } else {
        query.push(" IN (");
        let mut separated = query.separated(", ");
        for status in &request.lifecycle_statuses {
            separated.push_bind(status);
        }
        separated.push_unseparated(")");
    }
    if !folders.is_empty() {
        query.push(" AND artifacts.folder_id IN (");
        let mut separated = query.separated(", ");
        for folder_id in folders {
            separated.push_bind(folder_id);
        }
        separated.push_unseparated(")");
    }
    if let Some(after) = request.modified_after.as_deref().filter(|day| !day.is_empty()) {
        query.push(" AND substr(artifacts.updated_at, 1, 10) >= ");
        query.push_bind(after);
    }
    if let Some(before) = request.modified_before.as_deref().filter(|day| !day.is_empty()) {
        query.push(" AND substr(artifacts.updated_at, 1, 10) <= ");
        query.push_bind(before);
    }
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
}

pub fn decode_embedding(bytes: &[u8]) -> Option<Vec<f32>> {
    if !bytes.len().is_multiple_of(4) {
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
    let (dot, left_norm, right_norm) =
        left.iter()
            .zip(right)
            .fold((0.0_f64, 0.0_f64, 0.0_f64), |(dot, left_norm, right_norm), (a, b)| {
                let a = f64::from(*a);
                let b = f64::from(*b);
                (dot + a * b, left_norm + a * a, right_norm + b * b)
            });
    (left_norm > 0.0 && right_norm > 0.0).then_some(dot / (left_norm.sqrt() * right_norm.sqrt()))
}

#[cfg(test)]
mod tests;

fn hash_refresh_token(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

/// Whether an error is a database uniqueness violation (a duplicate row), as
/// opposed to every other way a statement can fail.
pub fn is_unique_violation(error: &anyhow::Error) -> bool {
    error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<sqlx::Error>())
        .any(|error| {
            error
                .as_database_error()
                .is_some_and(|database| database.is_unique_violation())
        })
}
