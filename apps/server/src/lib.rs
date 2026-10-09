//! Server-authoritative HTTP API for shared RepoMemo workspaces.

mod agent;
mod ai_access;
mod avatars;
mod conversion;
mod embedding;
mod environment;
mod events;
mod health;
mod indexing;
mod logs;
mod maintenance;
mod repositories;
mod security;
mod setup;
mod system;

pub use logs::{init_logging, log_capture_layer, LogSettings};

pub use maintenance::MaintenanceSettings;
pub use security::Quota;

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use anyhow::{bail, Context, Result};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, FromRequestParts, Path, Query, State},
    http::{header, request::Parts, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    middleware,
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{delete, get, post, put},
    Json, Router,
};
use tokio_stream::{
    wrappers::{errors::BroadcastStreamRecvError, BroadcastStream, ReceiverStream},
    Stream, StreamExt,
};
use chrono::{Duration as ChronoDuration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand_core::OsRng;
use repomemo_api::{RepoLinkPolicy, RepoMemoCore};
use repomemo_domain::{
    ArtifactComment, ArtifactDetail, ArtifactIndexFailure, DocumentPreview, Folder, ArtifactLifecycle, ArtifactLifecycleEvent, ArtifactSummary,
    ArtifactType, AskAnswer, AskRequest, Chunk, Citation, CollaborationTask,
    CreateMemoryCardRequest,
    IndexingJobStatus, KnowledgeMap, MemoryCard, MemoryCardDetail, MemoryCardSummary, Organization,
    OrganizationMember, OrganizationRole, ProviderSettings, ProviderTestResult, SavedSearch,
    SearchRequest, SearchResult, SharedAiProviderSettings, SharedNotification, SharedSession,
    SharedUser, SharedWorkspace, TaskChecklistItem, UpdateMemoryCardRequest, Workspace,
    WorkspaceActivityEvent, WorkspaceAiOverview, WorkspaceCapabilities, WorkspaceMember,
    WorkspaceOverview, WorkspaceRole,
};
use repomemo_storage::{
    NewCollaborationTask, NewSavedSearch, NewSharedNotification, NewTaskChecklistItem,
    SaveArtifactLifecycle, StorageConfig, StorageEngine,
};

use crate::events::{BusActivityObserver, BusJobObserver, WorkspaceEventBus};
use crate::conversion::{Converter, RenderState};
use crate::embedding::EmbeddingQueue;
use crate::repositories::RepoSyncRunner;
use crate::indexing::IndexQueue;
use crate::security::{too_many_requests, wait_text, ClientIp, Guards};
use crate::system::{InstanceInfo, MaintenanceStatus, SettingsCell, Telemetry};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::{
    catch_panic::CatchPanicLayer,
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};

const JWT_ISSUER: &str = "repomemo-server";
/// Longest file name accepted for an upload.
const MAX_UPLOAD_FILENAME_CHARS: usize = 255;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub service_name: String,
    pub bind_address: SocketAddr,
    /// The environment folder to attach at startup (verified, and always
    /// inside `workspace-data/` when it comes from `REPOMEMO_SERVER_DATA_DIR`).
    /// `None` starts the server detached: an administrator picks the
    /// environment in the web app.
    pub data_dir: Option<PathBuf>,
    pub jwt_secret: String,
    /// Browser origins allowed to call the API (CORS). Empty: no CORS headers.
    pub allowed_origins: Vec<HeaderValue>,
    /// LibreOffice executable used for Office previews. When unset the server
    /// looks for it; without it the extracted previews are used.
    pub soffice: Option<PathBuf>,
    /// Key material for encrypting stored secrets (AI provider API keys).
    /// When unset a random key is kept in `secret.key` in the data directory.
    pub secret_key: Option<String>,
    /// Anyone may create an account. When false, only the very first account
    /// can be registered; everyone else is added by an administrator after
    /// registering while it was open, or by the operator.
    pub allow_registration: bool,
    /// Take the client address from `X-Forwarded-For` / `X-Real-IP`. Only for
    /// a server reachable solely through a reverse proxy that sets them.
    pub trust_proxy: bool,
    pub max_upload_bytes: usize,
    pub access_token_ttl_minutes: i64,
    pub refresh_token_ttl_days: i64,
    /// Sign-ins and registrations per client address; token refreshes get
    /// four times as many.
    pub auth_quota: Quota,
    /// Consecutive failed sign-ins before an account is locked out; 0 never.
    pub login_max_failures: u32,
    pub login_lockout: Duration,
    /// AI requests per user per hour; a limit of 0 is unlimited.
    pub ai_quota: Quota,
    /// Hosts AI providers may be configured at; `None` allows any host except
    /// link-local and cloud-metadata addresses.
    pub ai_allowed_hosts: Option<Vec<String>>,
    /// Folders repository links must sit inside; empty allows any local folder.
    pub repo_roots: Vec<PathBuf>,
    /// How often linked repositories are checked for new commits; `None`
    /// turns automatic syncing off.
    pub repo_poll_interval: Option<Duration>,
    pub maintenance: MaintenanceSettings,
    /// Accounts made system administrators at startup and when they
    /// register. The first account on a new server always becomes one.
    pub system_admin_emails: Vec<String>,
    /// System audit events are kept this many days; 0 keeps them forever.
    pub system_audit_retention_days: i64,
    /// What is logged, how it is printed and whether it is kept in files.
    pub logging: LogSettings,
    /// A new server shows an onboarding that creates its first system
    /// administrator. When off, the first account to register becomes one.
    pub setup_wizard: bool,
    /// The one-time code the onboarding asks for; generated and printed on
    /// the console at startup when unset.
    pub setup_code: Option<String>,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self> {
        let jwt_secret = std::env::var("REPOMEMO_JWT_SECRET")
            .context("REPOMEMO_JWT_SECRET is required; use at least 32 random characters")?;
        if jwt_secret.len() < 32 {
            bail!("REPOMEMO_JWT_SECRET must contain at least 32 characters");
        }
        if jwt_secret.chars().collect::<BTreeSet<_>>().len() < 8 {
            bail!("REPOMEMO_JWT_SECRET is too repetitive; use at least 32 random characters");
        }

        let bind_address = std::env::var("REPOMEMO_SERVER_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:3020".to_owned())
            .parse()
            .context("REPOMEMO_SERVER_ADDR must be a valid socket address")?;
        let data_dir = match std::env::var("REPOMEMO_SERVER_DATA_DIR") {
            Ok(value) if !value.trim().is_empty() => Some(environment::resolve_setting(
                &environment::environments_root()?,
                &value,
            )?),
            _ => None,
        };
        let allowed_origins = std::env::var("REPOMEMO_ALLOWED_ORIGIN")
            .unwrap_or_else(|_| "http://127.0.0.1:3021".to_owned())
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(|origin| {
                if origin == "*" {
                    bail!("REPOMEMO_ALLOWED_ORIGIN must list origins; \"*\" is not allowed");
                }
                origin
                    .trim_end_matches('/')
                    .parse::<HeaderValue>()
                    .with_context(|| format!("REPOMEMO_ALLOWED_ORIGIN has an invalid origin: {origin}"))
            })
            .collect::<Result<Vec<_>>>()?;
        let secret_key = env_text("REPOMEMO_SECRET_KEY");
        if secret_key
            .as_ref()
            .is_some_and(|key| key.chars().count() < repomemo_storage::MIN_MASTER_KEY_CHARS)
        {
            bail!(
                "REPOMEMO_SECRET_KEY must contain at least {} characters",
                repomemo_storage::MIN_MASTER_KEY_CHARS
            );
        }
        let ai_allowed_hosts = env_text("REPOMEMO_AI_ALLOWED_HOSTS").map(|hosts| {
            hosts
                .split(',')
                .map(|host| host.trim().to_ascii_lowercase())
                .filter(|host| !host.is_empty())
                .collect::<Vec<_>>()
        });
        let repo_roots = std::env::var_os("REPOMEMO_REPO_ROOTS")
            .map(|roots| {
                std::env::split_paths(&roots)
                    .filter(|root| !root.as_os_str().is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let repo_poll_seconds: u64 = env_parse("REPOMEMO_REPO_POLL_SECONDS", 300)?;
        let maintenance_minutes: u64 = env_parse("REPOMEMO_MAINTENANCE_INTERVAL_MINUTES", 60)?;
        let index_retry_hours: u64 = env_parse("REPOMEMO_INDEX_RETRY_HOURS", 6)?;

        Ok(Self {
            service_name: std::env::var("REPOMEMO_SERVICE_NAME")
                .unwrap_or_else(|_| "repomemo-server".to_owned()),
            bind_address,
            data_dir,
            jwt_secret,
            allowed_origins,
            soffice: std::env::var_os("REPOMEMO_SOFFICE").map(PathBuf::from),
            secret_key,
            allow_registration: env_flag("REPOMEMO_ALLOW_REGISTRATION", true)?,
            trust_proxy: env_flag("REPOMEMO_TRUST_PROXY", false)?,
            max_upload_bytes: env_parse::<usize>("REPOMEMO_MAX_UPLOAD_MB", 10)?.clamp(1, 1024) * 1024 * 1024,
            access_token_ttl_minutes: env_parse::<i64>("REPOMEMO_ACCESS_TOKEN_TTL_MINUTES", 60)?.clamp(5, 24 * 60),
            refresh_token_ttl_days: env_parse::<i64>("REPOMEMO_REFRESH_TOKEN_TTL_DAYS", 30)?.clamp(1, 365),
            auth_quota: Quota::per_minute(env_parse("REPOMEMO_AUTH_REQUESTS_PER_MINUTE", 30)?),
            login_max_failures: env_parse("REPOMEMO_LOGIN_MAX_FAILURES", 10)?,
            login_lockout: Duration::from_secs(60 * env_parse::<u64>("REPOMEMO_LOGIN_LOCKOUT_MINUTES", 15)?.clamp(1, 24 * 60)),
            ai_quota: Quota::per_hour(env_parse("REPOMEMO_AI_REQUESTS_PER_HOUR", 120)?),
            ai_allowed_hosts,
            repo_roots,
            repo_poll_interval: (repo_poll_seconds > 0)
                .then(|| Duration::from_secs(repo_poll_seconds.max(30))),
            maintenance: MaintenanceSettings {
                interval: Duration::from_secs(60 * maintenance_minutes),
                blob_gc: env_flag("REPOMEMO_BLOB_GC", true)?,
                job_retention_days: env_parse("REPOMEMO_JOB_RETENTION_DAYS", 90)?,
                index_retry_interval: Duration::from_secs(3600 * index_retry_hours.max(1)),
            },
            system_admin_emails: env_text("REPOMEMO_SYSTEM_ADMIN_EMAILS")
                .map(|emails| {
                    emails
                        .split(',')
                        .map(|email| email.trim().to_lowercase())
                        .filter(|email| !email.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            system_audit_retention_days: env_parse("REPOMEMO_SYSTEM_AUDIT_RETENTION_DAYS", 365)?,
            logging: LogSettings::from_env()?,
            setup_wizard: env_flag("REPOMEMO_SETUP_WIZARD", true)?,
            setup_code: match env_text("REPOMEMO_SETUP_CODE") {
                Some(code) if code.chars().filter(char::is_ascii_alphanumeric).count() < 12 => {
                    bail!("REPOMEMO_SETUP_CODE must contain at least 12 letters or digits")
                }
                code => code,
            },
        })
    }

    #[cfg(test)]
    fn for_test(data_dir: PathBuf) -> Self {
        Self {
            service_name: "repomemo-server-test".to_owned(),
            bind_address: "127.0.0.1:0".parse().unwrap(),
            data_dir: Some(data_dir),
            jwt_secret: "test-secret-that-is-long-enough-for-jwt-signing".to_owned(),
            allowed_origins: Vec::new(),
            soffice: None,
            secret_key: None,
            allow_registration: true,
            trust_proxy: false,
            max_upload_bytes: 10 * 1024 * 1024,
            access_token_ttl_minutes: 60,
            refresh_token_ttl_days: 30,
            // Tests register many accounts from one address.
            auth_quota: Quota::per_minute(0),
            login_max_failures: 10,
            login_lockout: Duration::from_secs(15 * 60),
            ai_quota: Quota::per_hour(0),
            ai_allowed_hosts: None,
            repo_roots: Vec::new(),
            repo_poll_interval: None,
            maintenance: MaintenanceSettings {
                interval: Duration::ZERO,
                ..MaintenanceSettings::default()
            },
            system_admin_emails: Vec::new(),
            system_audit_retention_days: 365,
            logging: LogSettings {
                to_file: false,
                ..LogSettings::default()
            },
            // Tests register accounts directly; the onboarding has its own tests.
            setup_wizard: false,
            setup_code: None,
        }
    }
}

fn env_text(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Parses an optional environment variable, with a clear error for a value
/// that is set but invalid.
fn env_parse<T: std::str::FromStr>(name: &str, default: T) -> Result<T> {
    let Some(raw) = env_text(name) else {
        return Ok(default);
    };
    raw.parse::<T>()
        .map_err(|_| anyhow::anyhow!("{name} has an invalid value: {raw}"))
}

/// An optional on/off environment variable: true/false, 1/0, yes/no, on/off.
fn env_flag(name: &str, default: bool) -> Result<bool> {
    let Some(raw) = env_text(name) else {
        return Ok(default);
    };
    match raw.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => bail!("{name} must be true or false, not {raw}"),
    }
}

/// The configuration requests need at run time.
#[derive(Debug, Clone)]
struct RuntimeSettings {
    service_name: String,
    allow_registration: bool,
    trust_proxy: bool,
    access_token_ttl_minutes: i64,
    refresh_token_ttl_days: i64,
    auth_quota: Quota,
    login_max_failures: u32,
    login_lockout: Duration,
    ai_quota: Quota,
    repo_poll_interval: Option<Duration>,
    maintenance: MaintenanceSettings,
    system_audit_retention_days: i64,
    system_admin_emails: Vec<String>,
    logging: LogSettings,
}

impl RuntimeSettings {
    /// The settings the environment gives, before system administrators'
    /// run-time changes.
    fn from_config(config: &ServerConfig) -> Self {
        Self {
            service_name: config.service_name.clone(),
            allow_registration: config.allow_registration,
            trust_proxy: config.trust_proxy,
            access_token_ttl_minutes: config.access_token_ttl_minutes,
            refresh_token_ttl_days: config.refresh_token_ttl_days,
            auth_quota: config.auth_quota,
            login_max_failures: config.login_max_failures,
            login_lockout: config.login_lockout,
            ai_quota: config.ai_quota,
            repo_poll_interval: config.repo_poll_interval,
            maintenance: config.maintenance.clone(),
            system_audit_retention_days: config.system_audit_retention_days,
            system_admin_emails: config.system_admin_emails.clone(),
            logging: config.logging.clone(),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    pub(crate) fn runtime_settings() -> super::RuntimeSettings {
        super::RuntimeSettings::from_config(&super::ServerConfig::for_test(std::env::temp_dir()))
    }
}

#[derive(Clone)]
struct AppState {
    storage: StorageEngine,
    core: RepoMemoCore,
    jwt_secret: String,
    event_bus: WorkspaceEventBus,
    index_queue: IndexQueue,
    embedding_queue: EmbeddingQueue,
    converter: Converter,
    repo_sync: RepoSyncRunner,
    runtime: Arc<SettingsCell>,
    guards: Arc<Guards>,
    instance: Arc<InstanceInfo>,
    telemetry: Arc<Telemetry>,
    maintenance_status: Arc<std::sync::Mutex<MaintenanceStatus>>,
    setup: Arc<setup::SetupGate>,
    /// The host this environment is attached to; used to detach it.
    host: environment::HostHandle,
    /// Set when the environment is being detached: open streams end.
    closing: Arc<std::sync::atomic::AtomicBool>,
}

impl AppState {
    /// The run-time settings in force now.
    fn settings(&self) -> Arc<RuntimeSettings> {
        self.runtime.current()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub service: String,
    pub status: &'static str,
    pub authentication: &'static str,
    /// Whether new accounts can be registered.
    pub registration_open: bool,
    /// The server still has to be set up through the onboarding.
    pub setup_required: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadinessResponse {
    pub status: &'static str,
    pub database: &'static str,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Debug, Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    /// Seconds to wait before retrying, sent as `Retry-After`.
    retry_after: Option<u64>,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            retry_after: None,
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }

    fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "A valid bearer token is required.",
        )
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "conflict", message)
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    fn forbidden() -> Self {
        Self::forbidden_because("Your membership does not allow access to this workspace.")
    }

    fn forbidden_because(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(error = %error, "Unhandled API error");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "The server could not complete this request.",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ErrorBody {
                error: ErrorDetail {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response();
        if let Some(seconds) = self.retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(seconds));
        }
        response
    }
}

/// A panic inside a handler becomes the usual JSON error instead of a dropped
/// connection, and is logged.
fn panic_response(panic: Box<dyn std::any::Any + Send + 'static>) -> Response {
    let detail = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or("unknown panic");
    ApiError::internal(format!("handler panicked: {detail}")).into_response()
}

#[derive(Debug, Deserialize)]
struct RegisterRequest {
    email: String,
    display_name: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct UpdateProfileRequest {
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct ChangePasswordRequest {
    current_password: String,
    new_password: String,
}

#[derive(Debug, Deserialize)]
struct CreateOrganizationRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct UpdateOrganizationRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct UpsertOrganizationMemberRequest {
    email: String,
    role: OrganizationRole,
}

#[derive(Debug, Deserialize)]
struct CreateWorkspaceRequest {
    organization_id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct UpdateWorkspaceRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CreateTextArtifactRequest {
    title: String,
    content: String,
    language: Option<String>,
    folder_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RenameFolderRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
struct MoveArtifactRequest {
    folder_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateFolderRequest {
    name: String,
    parent_id: Option<String>,
}

/// An artifact summary plus the folder it sits in, for the shared client.
#[derive(Debug, Serialize)]
struct SharedArtifactSummary {
    #[serde(flatten)]
    summary: ArtifactSummary,
    folder_id: Option<String>,
    /// The repository source the file is synced from, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    repository_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateArtifactRequest {
    title: String,
}

#[derive(Debug, Deserialize)]
struct SaveArtifactLifecycleRequest {
    status: String,
    owner_user_id: Option<String>,
    #[serde(default)]
    review_note: String,
    superseded_by_artifact_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SaveSearchRequest {
    name: String,
    query: String,
    #[serde(default)]
    artifact_types: Vec<ArtifactType>,
    #[serde(default)]
    languages: Vec<String>,
    #[serde(default)]
    source_ids: Vec<String>,
    result_limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CreateChecklistItemRequest {
    body: String,
}

#[derive(Debug, Deserialize)]
struct ToggleChecklistItemRequest {
    completed: bool,
}

#[derive(Debug, Deserialize)]
struct QueryArtifactsRequest {
    #[serde(default)]
    query: String,
    #[serde(default)]
    artifact_types: Vec<repomemo_domain::ArtifactType>,
    #[serde(default)]
    languages: Vec<String>,
    #[serde(default)]
    source_ids: Vec<String>,
    indexed: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct SearchWorkspaceRequest {
    query: String,
    #[serde(default)]
    artifact_types: Vec<repomemo_domain::ArtifactType>,
    #[serde(default)]
    languages: Vec<String>,
    #[serde(default)]
    source_ids: Vec<String>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
struct RetrievalSourceFacet {
    id: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct RetrievalFacetsResponse {
    artifact_types: Vec<ArtifactType>,
    languages: Vec<String>,
    sources: Vec<RetrievalSourceFacet>,
}

#[derive(Debug, Serialize)]
struct WorkspaceMetricBreakdown {
    label: String,
    value: i64,
}

#[derive(Debug, Serialize)]
struct UserProfileResponse {
    user: SharedUser,
    created_at: String,
    updated_at: String,
    last_connected_at: Option<String>,
    workspace_count: i64,
    recent_activity_count: i64,
    activity_by_day: Vec<WorkspaceMetricBreakdown>,
}

#[derive(Debug, Serialize)]
struct WorkspaceActivityCalendarResponse {
    total_activity_count: i64,
    activity_by_day: Vec<WorkspaceMetricBreakdown>,
}

#[derive(Debug, Serialize)]
struct WorkspaceMetricsResponse {
    workspace_id: String,
    generated_at: String,
    source_count: i64,
    member_count: i64,
    artifact_count: i64,
    indexed_artifact_count: i64,
    pending_artifact_count: i64,
    total_artifact_bytes: i64,
    indexed_artifact_bytes: i64,
    pending_artifact_bytes: i64,
    chunk_count: i64,
    /// Chunks with a vector from the current embedding model; `None` when the
    /// workspace has no embedding provider.
    embedded_chunk_count: Option<i64>,
    symbol_count: i64,
    memory_card_count: i64,
    open_task_count: i64,
    in_progress_task_count: i64,
    blocked_task_count: i64,
    completed_task_count: i64,
    overdue_task_count: i64,
    comment_count: i64,
    recent_activity_count: i64,
    artifacts_created_last_7_days: i64,
    artifacts_updated_last_7_days: i64,
    activity_actions: Vec<WorkspaceMetricBreakdown>,
    activity_by_day: Vec<WorkspaceMetricBreakdown>,
    member_roles: Vec<WorkspaceMetricBreakdown>,
    artifact_types: Vec<WorkspaceMetricBreakdown>,
    artifact_bytes_by_type: Vec<WorkspaceMetricBreakdown>,
    languages: Vec<WorkspaceMetricBreakdown>,
}

#[derive(Debug, Deserialize)]
struct AskWorkspaceRequest {
    question: String,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct SearchMemoryCardsRequest {
    query: String,
}

#[derive(Debug, Deserialize)]
struct CreateMemoryCardBody {
    title: String,
    body_markdown: String,
    source: String,
    confidence: Option<f64>,
    #[serde(default)]
    citations: Vec<Citation>,
}

#[derive(Debug, Deserialize)]
struct UpdateMemoryCardBody {
    title: String,
    body_markdown: String,
    source: String,
    confidence: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SaveAiProviderRequest {
    id: Option<String>,
    provider_type: String,
    name: String,
    base_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    enabled: bool,
    #[serde(default)]
    cloud_content_acknowledged: bool,
    /// `text` or `vision`; keeps the saved purpose when omitted.
    purpose: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpsertWorkspaceMemberRequest {
    email: String,
    role: WorkspaceRole,
}

#[derive(Debug, Deserialize)]
struct SaveCollaborationTaskRequest {
    title: String,
    #[serde(default)]
    description: String,
    status: String,
    priority: String,
    assignee_user_id: Option<String>,
    artifact_id: Option<String>,
    due_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SaveArtifactCommentRequest {
    body: String,
}

#[derive(Debug, Serialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    token_type: &'static str,
    expires_in: u64,
    user: SharedUser,
}

#[derive(Debug, Deserialize)]
struct RefreshRequest {
    refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JwtClaims {
    sub: String,
    email: String,
    iss: String,
    iat: usize,
    exp: usize,
    /// The user's session version when the token was issued; tokens from
    /// before a password change or "sign out everywhere" no longer match.
    #[serde(default)]
    ver: i64,
}

#[derive(Debug, Clone)]
struct AuthenticatedSubject {
    user_id: String,
    /// When the access token expires (seconds since the epoch). Long-lived
    /// streams end then, like any other use of the token.
    expires_at: u64,
    session_version: i64,
    /// Administers the whole server: acts as an administrator in every
    /// organization and workspace, and may use the system API.
    is_system_admin: bool,
    /// May use the system API and pages, without the wildcard access.
    is_app_admin: bool,
}

/// The whole application. With `config.data_dir` set the environment is
/// verified and attached right away; otherwise the server waits for an
/// administrator to pick one (see [`environment`]).
pub async fn router(config: ServerConfig) -> Result<Router> {
    let host = environment::Host::start(config).await?;
    Ok(host.router())
}

/// Opens storage in `config.data_dir`, recovers from the previous shutdown
/// and starts the background queues and pollers. The returned tasks are the
/// ones that must be stopped when the environment is detached.
async fn build_state(
    config: &ServerConfig,
    host: environment::HostHandle,
) -> Result<(AppState, Vec<tokio::task::JoinHandle<()>>)> {
    let data_dir = config
        .data_dir
        .clone()
        .context("an environment folder is required to open storage")?;
    let storage = StorageEngine::open(StorageConfig {
        data_dir: data_dir.clone(),
        master_key: config.secret_key.clone(),
    })
    .await?;

    // Jobs a previous process left running will never finish; close them
    // before new work starts so no progress bar spins forever.
    let interrupted = storage
        .fail_all_interrupted_jobs("Interrupted by a server restart.")
        .await?;
    if interrupted > 0 {
        tracing::info!(count = interrupted, "Closed jobs interrupted by the previous shutdown");
    }

    // Settings changed by system administrators override the environment.
    let runtime = Arc::new(SettingsCell::new(
        RuntimeSettings::from_config(config),
        data_dir.clone(),
    ));
    let overrides = storage
        .list_system_settings()
        .await?
        .into_iter()
        .map(|setting| (setting.key, setting.value))
        .collect::<Vec<_>>();
    runtime.load_overrides(&overrides);

    let promoted = storage.promote_system_admins(&config.system_admin_emails).await?;
    if promoted > 0 {
        tracing::info!(target: "audit", count = promoted, "Promoted accounts listed in REPOMEMO_SYSTEM_ADMIN_EMAILS to system administrators");
        storage
            .record_system_event(
                None,
                "system_admin_granted",
                &format!("Made {promoted} account(s) listed in REPOMEMO_SYSTEM_ADMIN_EMAILS system administrators."),
            )
            .await?;
    }
    if storage.system_admin_count().await? == 0 && storage.count_users().await? > 0 {
        tracing::warn!("This server has no system administrator. List one or more account emails in REPOMEMO_SYSTEM_ADMIN_EMAILS and restart.");
    }

    // Computed once, off the async workers, before the first sign-in needs it.
    tokio::task::spawn_blocking(|| {
        dummy_password_hash();
    })
    .await
    .context("preparing password verification")?;

    if !repomemo_ai::set_endpoint_policy(repomemo_ai::EndpointPolicy {
        allowed_hosts: config.ai_allowed_hosts.clone(),
    }) {
        tracing::debug!("The AI endpoint policy was already set in this process");
    }
    let repo_policy = if config.repo_roots.is_empty() {
        RepoLinkPolicy::unrestricted()
    } else {
        let policy = RepoLinkPolicy::with_allowed_roots(config.repo_roots.clone())
            .context("REPOMEMO_REPO_ROOTS")?;
        tracing::info!(roots = ?policy.allowed_roots(), "Repository links are restricted to these folders");
        policy
    };

    // Share the same storage handle with the API core so job and activity
    // observers registered here also fire for writes that happen inside
    // `RepoMemoCore`. Without this the two halves would each hold their own
    // pool and SSE would only see writes the server layer performed directly.
    let core = RepoMemoCore::from_storage(storage.clone()).with_repo_link_policy(repo_policy);

    let event_bus = WorkspaceEventBus::new();
    storage.set_job_observer(Arc::new(BusJobObserver::new(event_bus.clone())));
    storage.set_activity_observer(Arc::new(BusActivityObserver::new(event_bus.clone())));

    let embedding_queue = EmbeddingQueue::start(core.clone());
    let index_queue = IndexQueue::start(core.clone(), embedding_queue.clone());
    index_queue.resume_pending().await;
    embedding_queue.resume_pending().await;
    let repo_sync = RepoSyncRunner::new(core.clone(), embedding_queue.clone());
    repo_sync.resume_all().await;
    let polling = runtime.clone();
    let polling_task = repo_sync.start_polling(move || polling.current().repo_poll_interval);

    let converter = match config.soffice.clone() {
        Some(path) => Converter::with_binary(&data_dir, Some(path)),
        None => Converter::detect(&data_dir),
    };
    if converter.enabled() {
        tracing::info!("LibreOffice found: Office files get layout-accurate previews");
    } else {
        tracing::info!("LibreOffice not found: Office previews show extracted text and tables");
    }
    let instance = Arc::new(InstanceInfo::new(config, converter.enabled()));
    let setup = Arc::new(setup::SetupGate::prepare(
        config.setup_wizard,
        config.setup_code.as_deref(),
        &storage.setup_state().await?,
        setup::SetupFacts {
            secret_key_from_env: config.secret_key.is_some(),
            allowed_origins: config
                .allowed_origins
                .iter()
                .filter_map(|origin| origin.to_str().ok().map(str::to_owned))
                .collect(),
            repo_roots: config
                .repo_roots
                .iter()
                .map(|root| root.to_string_lossy().into_owned())
                .collect(),
            ai_hosts_restricted: config.ai_allowed_hosts.is_some(),
            trust_proxy: config.trust_proxy,
            data_dir: data_dir.clone(),
        },
    ));
    let state = AppState {
        storage,
        core,
        jwt_secret: config.jwt_secret.clone(),
        event_bus,
        index_queue,
        embedding_queue,
        converter,
        repo_sync,
        runtime,
        guards: Arc::new(Guards::default()),
        instance,
        telemetry: Arc::new(Telemetry::default()),
        maintenance_status: Arc::new(std::sync::Mutex::new(MaintenanceStatus::default())),
        setup,
        host,
        closing: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };
    let maintenance_task = maintenance::start(state.clone());
    Ok((state, vec![polling_task, maintenance_task]))
}

/// Cross-origin rules for the whole application, platform routes included.
fn cors_layer(config: &ServerConfig) -> CorsLayer {
    if config.allowed_origins.is_empty() {
        CorsLayer::new()
    } else {
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(config.allowed_origins.clone()))
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
            ])
            .allow_headers([
                header::AUTHORIZATION,
                header::CONTENT_TYPE,
                HeaderName::from_static("x-repomemo-filename"),
                HeaderName::from_static("x-repomemo-folder-id"),
                HeaderName::from_static("x-repomemo-client"),
            ])
            .expose_headers([
                HeaderName::from_static("x-total-count"),
                header::RETRY_AFTER,
            ])
    }
}

/// The routes of one attached environment. Cross-origin handling is done by
/// the host in front of it.
fn routes(state: AppState, config: &ServerConfig) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(readiness))
        .route("/v1/setup", get(setup::get_status))
        .route("/v1/setup/verify-code", post(setup::verify_code))
        .route("/v1/setup/admin", post(setup::create_admin))
        .route("/v1/setup/checks", get(setup::checks))
        .route("/v1/setup/complete", post(setup::complete))
        .route("/v1/auth/register", post(register))
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/refresh", post(refresh_session))
        .route("/v1/auth/logout", post(logout))
        .route("/v1/auth/logout-all", post(logout_everywhere))
        .route("/v1/system/overview", get(system::overview))
        .route("/v1/system/usage", get(system::usage))
        .route("/v1/system/users", get(system::list_users))
        .route("/v1/system/users/{user_id}/system-admin", put(system::set_system_admin))
        .route("/v1/system/users/{user_id}/app-admin", put(system::set_app_admin))
        .route("/v1/system/users/{user_id}/sessions/end", post(system::end_user_sessions))
        .route("/v1/system/users/{user_id}/unlock", post(system::unlock_user))
        .route("/v1/system/settings", get(system::get_settings).put(system::update_settings))
        .route("/v1/system/settings/{key}", delete(system::reset_setting))
        .route("/v1/system/environment/detach", post(system::detach_environment))
        .route("/v1/system/logs", get(system::logs))
        .route("/v1/system/logs/files", get(system::log_files))
        .route("/v1/system/logs/files/{day}", get(system::download_log_file))
        .route("/v1/system/audit", get(system::audit_events))
        .route("/v1/system/jobs", get(system::jobs))
        .route("/v1/system/maintenance", get(system::get_maintenance))
        .route("/v1/system/maintenance/run", post(system::run_maintenance))
        .route("/v1/system/console", get(system::console::welcome).post(system::console::run))
        .route("/v1/session", get(session))
        .route("/v1/profile", get(get_profile).put(update_profile))
        .route("/v1/profile/password", post(change_profile_password))
        .route("/v1/profile/tasks", get(list_profile_tasks))
        .merge(avatars::routes())
        .route("/v1/notifications", get(list_notifications))
        .route(
            "/v1/notifications/read-all",
            post(mark_all_notifications_read),
        )
        .route(
            "/v1/notifications/{notification_id}/read",
            post(mark_notification_read),
        )
        .route(
            "/v1/organizations",
            get(list_organizations).post(create_organization),
        )
        .route(
            "/v1/organizations/{organization_id}",
            put(update_organization),
        )
        .route(
            "/v1/organizations/{organization_id}/members",
            get(list_organization_members).put(upsert_organization_member),
        )
        .route(
            "/v1/organizations/{organization_id}/members/{user_id}",
            delete(remove_organization_member),
        )
        .route(
            "/v1/workspaces",
            get(list_workspaces).post(create_workspace),
        )
        .route(
            "/v1/workspaces/{workspace_id}",
            put(update_workspace).delete(delete_workspace),
        )
        .route(
            "/v1/workspaces/{workspace_id}/overview",
            get(workspace_overview),
        )
        .route(
            "/v1/workspaces/{workspace_id}/metrics",
            get(workspace_metrics),
        )
        .route(
            "/v1/workspaces/{workspace_id}/capabilities",
            get(workspace_capabilities),
        )
        .route(
            "/v1/workspaces/{workspace_id}/ai-overview",
            post(generate_workspace_ai_overview),
        )
        .route("/v1/workspaces/{workspace_id}/ask", post(ask_workspace))
        .route(
            "/v1/workspaces/{workspace_id}/knowledge-map",
            get(workspace_knowledge_map),
        )
        .route(
            "/v1/workspaces/{workspace_id}/health",
            get(health::workspace_health),
        )
        .route(
            "/v1/workspaces/{workspace_id}/health/actions",
            post(health::apply_health_action),
        )
        .route(
            "/v1/workspaces/{workspace_id}/agent/capabilities",
            get(agent::list_agent_capabilities),
        )
        .route(
            "/v1/workspaces/{workspace_id}/agent/messages",
            post(agent::send_agent_message),
        )
        .route(
            "/v1/workspaces/{workspace_id}/agent/conversations",
            get(agent::list_agent_conversations),
        )
        .route(
            "/v1/agent/conversations/{conversation_id}",
            get(agent::get_agent_conversation)
                .put(agent::rename_agent_conversation)
                .delete(agent::delete_agent_conversation),
        )
        .route(
            "/v1/workspaces/{workspace_id}/ai-providers",
            get(list_workspace_ai_providers).put(save_workspace_ai_provider),
        )
        .route(
            "/v1/workspaces/{workspace_id}/ai-policy",
            get(ai_access::get_ai_policy).put(ai_access::update_ai_policy),
        )
        .route(
            "/v1/workspaces/{workspace_id}/ai-providers/{provider_id}/test",
            post(test_workspace_ai_provider),
        )
        .route(
            "/v1/workspaces/{workspace_id}/activity",
            get(list_workspace_activity),
        )
        .route(
            "/v1/workspaces/{workspace_id}/activity/calendar",
            get(workspace_activity_calendar),
        )
        .route(
            "/v1/workspaces/{workspace_id}/tasks",
            get(list_collaboration_tasks).post(create_collaboration_task),
        )
        .route(
            "/v1/workspaces/{workspace_id}/saved-searches",
            get(list_saved_searches).post(create_saved_search),
        )
        .route(
            "/v1/tasks/{task_id}",
            get(get_collaboration_task)
                .put(update_collaboration_task)
                .delete(delete_collaboration_task),
        )
        .route(
            "/v1/tasks/{task_id}/checklist",
            get(list_task_checklist).post(create_task_checklist_item),
        )
        .route(
            "/v1/task-checklist/{item_id}",
            put(toggle_task_checklist_item).delete(delete_task_checklist_item),
        )
        .route(
            "/v1/saved-searches/{search_id}",
            delete(delete_saved_search),
        )
        .route(
            "/v1/workspaces/{workspace_id}/members",
            get(list_workspace_members).put(upsert_workspace_member),
        )
        .route(
            "/v1/workspaces/{workspace_id}/members/{user_id}",
            delete(remove_workspace_member),
        )
        .route(
            "/v1/workspaces/{workspace_id}/artifacts",
            get(list_artifacts),
        )
        .route(
            "/v1/workspaces/{workspace_id}/folders",
            get(list_folders).post(create_folder),
        )
        .route(
            "/v1/workspaces/{workspace_id}/folders/{folder_id}",
            axum::routing::patch(rename_folder).delete(delete_folder),
        )
        .route("/v1/artifacts/{artifact_id}/folder", put(move_artifact))
        .route("/v1/artifacts/{artifact_id}/file", get(download_artifact_file))
        .route(
            "/v1/artifacts/{artifact_id}/document-preview",
            get(artifact_document_preview),
        )
        .route(
            "/v1/artifacts/{artifact_id}/rendered-preview/status",
            get(rendered_preview_status),
        )
        .route(
            "/v1/artifacts/{artifact_id}/rendered-preview",
            get(rendered_preview_pdf),
        )
        .route(
            "/v1/artifacts/{artifact_id}/open-link",
            post(create_file_link),
        )
        .route(
            "/v1/shared-files/{token}/{filename}",
            get(shared_file_by_link),
        )
        .route(
            "/v1/workspaces/{workspace_id}/artifacts/index-failures",
            get(list_artifact_index_failures),
        )
        .route(
            "/v1/workspaces/{workspace_id}/artifacts/query",
            post(query_artifacts),
        )
        .route(
            "/v1/workspaces/{workspace_id}/artifacts/text",
            post(create_text_artifact),
        )
        .route(
            "/v1/workspaces/{workspace_id}/artifacts/upload",
            post(upload_artifact),
        )
        .route(
            "/v1/artifacts/{artifact_id}",
            get(get_artifact)
                .put(update_artifact)
                .delete(delete_artifact),
        )
        .route(
            "/v1/artifacts/{artifact_id}/chunks",
            get(list_artifact_chunks),
        )
        .route(
            "/v1/artifacts/{artifact_id}/comments",
            get(list_artifact_comments).post(create_artifact_comment),
        )
        .route(
            "/v1/artifacts/{artifact_id}/lifecycle",
            get(get_artifact_lifecycle).put(update_artifact_lifecycle),
        )
        .route(
            "/v1/artifacts/{artifact_id}/lifecycle/history",
            get(list_artifact_lifecycle_events),
        )
        .route(
            "/v1/comments/{comment_id}",
            put(update_artifact_comment).delete(delete_artifact_comment),
        )
        .route("/v1/artifacts/{artifact_id}/index", post(index_artifact))
        .route(
            "/v1/workspaces/{workspace_id}/repositories",
            get(repositories::list_repositories).post(repositories::connect_repository),
        )
        .route(
            "/v1/workspaces/{workspace_id}/repositories/check",
            post(repositories::check_repository),
        )
        .route(
            "/v1/repositories/{source_id}",
            get(repositories::get_repository)
                .put(repositories::update_repository)
                .delete(repositories::delete_repository),
        )
        .route("/v1/repositories/{source_id}/sync", post(repositories::sync_repository))
        .route("/v1/repositories/{source_id}/files", get(repositories::list_repository_files))
        .route("/v1/repositories/{source_id}/detail", get(repositories::get_repository_detail))
        .route("/v1/repositories/{source_id}/summary", post(repositories::summarize_repository))
        .route("/v1/workspaces/{workspace_id}/index", post(index_workspace))
        .route("/v1/workspaces/{workspace_id}/jobs", get(list_workspace_jobs))
        .route("/v1/workspaces/{workspace_id}/events", get(workspace_events))
        .route("/v1/jobs/{job_id}", get(get_job))
        .route("/v1/jobs/{job_id}/cancel", post(cancel_job))
        .route(
            "/v1/workspaces/{workspace_id}/retrieval-facets",
            get(get_retrieval_facets),
        )
        .route(
            "/v1/workspaces/{workspace_id}/search",
            post(search_workspace),
        )
        .route(
            "/v1/workspaces/{workspace_id}/memory-cards",
            get(list_memory_cards).post(create_memory_card),
        )
        .route(
            "/v1/workspaces/{workspace_id}/memory-cards/search",
            post(search_memory_cards),
        )
        .route(
            "/v1/memory-cards/{card_id}",
            get(get_memory_card)
                .put(update_memory_card)
                .delete(delete_memory_card),
        )
        .route("/v1/memory-cards/{card_id}/export", get(export_memory_card))
        .layer(CatchPanicLayer::custom(panic_response))
        .layer(middleware::from_fn(security::security_headers))
        .layer(middleware::from_fn_with_state(state.clone(), system::track_requests))
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(config.max_upload_bytes))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        service: state.settings().service_name.clone(),
        status: "ok",
        authentication: "jwt",
        registration_open: registration_open(&state).await.unwrap_or(false),
        setup_required: setup::setup_pending(&state).await.unwrap_or(false),
    })
}

/// Readiness for load balancers and orchestrators: the database answers.
async fn readiness(State(state): State<AppState>) -> (StatusCode, Json<ReadinessResponse>) {
    match state.storage.ping().await {
        Ok(()) => (
            StatusCode::OK,
            Json(ReadinessResponse {
                status: "ready",
                database: "ok",
            }),
        ),
        Err(error) => {
            tracing::error!(error = %error, "Readiness check failed: the database does not answer");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ReadinessResponse {
                    status: "unavailable",
                    database: "unavailable",
                }),
            )
        }
    }
}

/// Registration is open by configuration, or because nobody has an account
/// yet (so a server with registration closed can still get its first owner).
async fn registration_open(state: &AppState) -> Result<bool, ApiError> {
    // Nobody registers before the onboarding is done.
    if setup::setup_pending(state).await? {
        return Ok(false);
    }
    if state.settings().allow_registration {
        return Ok(true);
    }
    Ok(state.storage.count_users().await.map_err(ApiError::internal)? == 0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthAction {
    /// Signing in or registering: guessable, so tightly limited.
    SignIn,
    /// Refreshing a session: every open tab does it and the token cannot be
    /// guessed, so it only needs protecting from floods.
    Refresh,
}

/// Counts one authentication request against the client address's quota.
fn check_auth_quota(state: &AppState, client: &ClientIp, action: AuthAction) -> Result<(), ApiError> {
    let quota = match action {
        AuthAction::SignIn => state.settings().auth_quota,
        AuthAction::Refresh => Quota {
            limit: state.settings().auth_quota.limit.saturating_mul(4),
            ..state.settings().auth_quota
        },
    };
    state
        .guards
        .auth
        .check(&format!("{action:?}:{}", client.0), quota)
        .map_err(|wait| {
            tracing::warn!(target: "audit", client = %client.0, ?action, "Authentication rate limit reached");
            too_many_requests(
                format!("Too many sign-in attempts from this address. Try again in {}.", wait_text(wait)),
                wait,
            )
        })
}

async fn register(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<TokenResponse>), ApiError> {
    check_auth_quota(&state, &client, AuthAction::SignIn)?;
    if setup::setup_pending(&state).await? {
        return Err(ApiError::forbidden_because(
            "This server is not set up yet. Its administrator must finish the setup first.",
        ));
    }
    if !registration_open(&state).await? {
        return Err(ApiError::forbidden_because(
            "Registration is closed on this server. Ask an administrator for an account.",
        ));
    }
    validate_registration(&request)?;
    if state
        .storage
        .find_user_for_auth(&request.email)
        .await
        .map_err(map_storage_error)?
        .is_some()
    {
        return Err(ApiError::conflict(
            "An account already exists for this email address.",
        ));
    }
    let password_hash = hash_password(&request.password).await?;
    let user = state
        .storage
        .create_user(&request.email, &request.display_name, &password_hash)
        .await
        .map_err(ApiError::internal)?;
    state
        .storage
        .touch_user_connection(&user.id)
        .await
        .map_err(ApiError::internal)?;
    tracing::info!(target: "audit", user_id = %user.id, client = %client.0, "Account registered");
    // The first account of a new server administers it, as do accounts the
    // operator listed.
    let first_account = state.storage.count_users().await.map_err(ApiError::internal)? == 1
        && state.storage.system_admin_count().await.map_err(ApiError::internal)? == 0;
    let listed = user
        .email
        .as_deref()
        .is_some_and(|email| state.settings().system_admin_emails.iter().any(|listed| listed == email));
    if first_account || listed {
        state
            .storage
            .set_system_admin(&user.id, true)
            .await
            .map_err(ApiError::internal)?;
        if first_account {
            // With the onboarding off, the first account sets the server up.
            state
                .storage
                .complete_setup(Some(&user.id))
                .await
                .map_err(ApiError::internal)?;
        }
        let reason = if first_account { "the first account on this server" } else { "listed in REPOMEMO_SYSTEM_ADMIN_EMAILS" };
        let detail = format!("{} became a system administrator as {reason}.", user.email.as_deref().unwrap_or("A new account"));
        tracing::info!(target: "audit", user_id = %user.id, "{detail}");
        if let Err(error) = state.storage.record_system_event(Some(&user.id), "system_admin_granted", &detail).await {
            tracing::error!(error = %error, "Failed to record a system audit event");
        }
    }
    let response = issue_session(&state, user).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

async fn login(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    check_auth_quota(&state, &client, AuthAction::SignIn)?;
    let email = request.email.trim().to_lowercase();
    // Failures are counted per account and address, and per account across
    // all addresses with a higher bar, so one attacker cannot lock a victim
    // out from everywhere while a distributed one is still slowed down.
    let pair_key = format!("login:{email}|{}", client.0);
    let account_key = format!("login:{email}");
    let settings = state.settings();
    for key in [&pair_key, &account_key] {
        if let Some(wait) = state.guards.logins.locked_for(key) {
            tracing::warn!(target: "audit", email = %email, client = %client.0, "Sign-in refused: account temporarily locked");
            return Err(too_many_requests(
                format!(
                    "Too many failed sign-in attempts for this account. Try again in {}.",
                    wait_text(wait)
                ),
                wait,
            ));
        }
    }
    let account = state
        .storage
        .find_user_for_auth(&request.email)
        .await
        .map_err(|_| invalid_credentials())?;
    // Verify against a fixed hash when the account does not exist, so the
    // response time does not reveal which email addresses have accounts.
    let (stored_hash, user) = match account {
        Some(account) => (account.password_hash, Some(account.user)),
        None => (dummy_password_hash().to_owned(), None),
    };
    let verified = verify_password(&request.password, &stored_hash).await;
    let Some(user) = user.filter(|_| verified) else {
        // `|`, not `||`: both counters must see every failure.
        let locked = state.guards.logins.record_failure(
            &pair_key,
            settings.login_max_failures,
            settings.login_lockout,
        ) | state.guards.logins.record_failure(
            &account_key,
            settings.login_max_failures.saturating_mul(5),
            settings.login_lockout,
        );
        tracing::warn!(target: "audit", email = %email, client = %client.0, locked, "Sign-in failed");
        return Err(invalid_credentials());
    };
    state.guards.logins.clear(&pair_key);
    state
        .storage
        .touch_user_connection(&user.id)
        .await
        .map_err(ApiError::internal)?;
    tracing::info!(target: "audit", user_id = %user.id, client = %client.0, "Signed in");
    Ok(Json(issue_session(&state, user).await?))
}

/// Exchanges a valid refresh token for a new access token and a rotated refresh token.
async fn refresh_session(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    check_auth_quota(&state, &client, AuthAction::Refresh)?;
    let user_id = state
        .storage
        .consume_refresh_token(&request.refresh_token)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let user = state
        .storage
        .find_user(&user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    Ok(Json(issue_session(&state, user).await?))
}

async fn logout(
    State(state): State<AppState>,
    Json(request): Json<RefreshRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .revoke_refresh_token(&request.refresh_token)
        .await
        .map_err(ApiError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ends every session of the caller on every device: access tokens stop
/// working at once and refresh tokens are revoked.
async fn logout_everywhere(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .end_user_sessions(&subject.user_id)
        .await
        .map_err(ApiError::internal)?;
    tracing::info!(target: "audit", user_id = %subject.user_id, "Signed out of every session");
    Ok(StatusCode::NO_CONTENT)
}

async fn session(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<SharedSession>, ApiError> {
    let user = state
        .storage
        .find_user(&subject.user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let memberships = state
        .storage
        .workspace_memberships_for_user(&subject.user_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(SharedSession {
        user,
        authentication: "jwt".to_owned(),
        memberships,
        is_system_admin: subject.is_system_admin,
        is_app_admin: subject.is_app_admin,
    }))
}

async fn get_profile(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<UserProfileResponse>, ApiError> {
    let profile = state
        .storage
        .get_user_profile(&subject.user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let memberships = state
        .storage
        .workspace_memberships_for_user(&subject.user_id)
        .await
        .map_err(ApiError::internal)?;
    let activity = state
        .storage
        .user_activity_by_day(
            &subject.user_id,
            &(Utc::now() - ChronoDuration::days(364)).to_rfc3339(),
        )
        .await
        .map_err(map_storage_error)?;
    let today = Utc::now().date_naive();
    let mut activity_by_day = (0..365)
        .rev()
        .map(|offset| ((today - ChronoDuration::days(offset)).to_string(), 0))
        .collect::<BTreeMap<_, _>>();
    for (day, count) in activity {
        if let Some(value) = activity_by_day.get_mut(&day) {
            *value = count;
        }
    }
    let recent_activity_count = activity_by_day.values().sum();
    Ok(Json(UserProfileResponse {
        user: profile.user,
        created_at: profile.created_at,
        updated_at: profile.updated_at,
        last_connected_at: profile.last_connected_at,
        workspace_count: memberships.len() as i64,
        recent_activity_count,
        activity_by_day: metric_timeline(activity_by_day),
    }))
}

async fn list_profile_tasks(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<CollaborationTask>>, ApiError> {
    state
        .storage
        .list_assigned_collaboration_tasks(&subject.user_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn list_notifications(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<SharedNotification>>, ApiError> {
    state
        .storage
        .list_shared_notifications(&subject.user_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn mark_notification_read(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(notification_id): Path<String>,
) -> Result<Json<SharedNotification>, ApiError> {
    state
        .storage
        .mark_shared_notification_read(&notification_id, &subject.user_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn mark_all_notifications_read(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .mark_all_shared_notifications_read(&subject.user_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn update_profile(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<UpdateProfileRequest>,
) -> Result<Json<SharedUser>, ApiError> {
    if request.display_name.trim().is_empty() || request.display_name.trim().len() > 120 {
        return Err(ApiError::bad_request(
            "Display name must be between 1 and 120 characters.",
        ));
    }
    state
        .storage
        .update_user_display_name(&subject.user_id, &request.display_name)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

/// Changes the caller's password. Every existing session ends (other devices
/// are signed out at once) and a fresh session is returned for this one.
async fn change_profile_password(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    validate_password(&request.new_password)?;
    let account = state
        .storage
        .find_user(&subject.user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let stored_account = state
        .storage
        .find_user_for_auth(account.email.as_deref().unwrap_or_default())
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    if !verify_password(&request.current_password, &stored_account.password_hash).await {
        tracing::warn!(target: "audit", user_id = %subject.user_id, "Password change refused: wrong current password");
        return Err(ApiError::bad_request("Current password is incorrect."));
    }
    let password_hash = hash_password(&request.new_password).await?;
    state
        .storage
        .update_user_password(&subject.user_id, &password_hash)
        .await
        .map_err(map_storage_error)?;
    state
        .storage
        .end_user_sessions(&subject.user_id)
        .await
        .map_err(ApiError::internal)?;
    tracing::info!(target: "audit", user_id = %subject.user_id, "Password changed; other sessions ended");
    Ok(Json(issue_session(&state, account).await?))
}

async fn create_organization(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<CreateOrganizationRequest>,
) -> Result<(StatusCode, Json<Organization>), ApiError> {
    let organization = state
        .storage
        .create_organization(&subject.user_id, &request.name)
        .await
        .map_err(map_storage_error)?;
    Ok((StatusCode::CREATED, Json(organization)))
}

/// The caller's organizations; every organization for a system administrator.
async fn list_organizations(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<Organization>>, ApiError> {
    let organizations = if subject.is_system_admin {
        state.storage.list_organizations_for_system_admin(&subject.user_id).await
    } else {
        state.storage.list_organizations_for_user(&subject.user_id).await
    };
    organizations.map(Json).map_err(ApiError::internal)
}

async fn update_organization(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
    Json(request): Json<UpdateOrganizationRequest>,
) -> Result<Json<Organization>, ApiError> {
    require_organization_owner(&state, &subject, &organization_id).await?;
    state
        .storage
        .update_organization(&organization_id, &subject.user_id, &request.name)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn list_organization_members(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
) -> Result<Json<Vec<OrganizationMember>>, ApiError> {
    require_organization_read(&state, &subject, &organization_id).await?;
    state
        .storage
        .list_organization_members(&organization_id)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

async fn upsert_organization_member(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
    Json(request): Json<UpsertOrganizationMemberRequest>,
) -> Result<Json<OrganizationMember>, ApiError> {
    let caller_role = require_organization_admin(&state, &subject, &organization_id).await?;
    if matches!(request.role, OrganizationRole::Owner)
        || (matches!(caller_role, OrganizationRole::Admin)
            && matches!(request.role, OrganizationRole::Admin))
    {
        return Err(ApiError::forbidden());
    }
    if matches!(caller_role, OrganizationRole::Admin) {
        if let Some(target) = state
            .storage
            .find_user_by_email(&request.email)
            .await
            .map_err(ApiError::internal)?
        {
            let target_role = state
                .storage
                .organization_role_for_user(&target.id, &organization_id)
                .await
                .map_err(ApiError::internal)?;
            if matches!(
                target_role,
                Some(OrganizationRole::Owner | OrganizationRole::Admin)
            ) {
                return Err(ApiError::forbidden());
            }
        }
    }
    state
        .storage
        .upsert_organization_member(&organization_id, &request.email, request.role)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn remove_organization_member(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((organization_id, user_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let caller_role = require_organization_admin(&state, &subject, &organization_id).await?;
    let target_role = state
        .storage
        .organization_role_for_user(&user_id, &organization_id)
        .await
        .map_err(ApiError::internal)?;
    if matches!(target_role, Some(OrganizationRole::Owner))
        || (matches!(caller_role, OrganizationRole::Admin)
            && matches!(target_role, Some(OrganizationRole::Admin)))
    {
        return Err(ApiError::forbidden());
    }
    state
        .storage
        .remove_organization_member(&organization_id, &user_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_workspace(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<CreateWorkspaceRequest>,
) -> Result<(StatusCode, Json<SharedWorkspace>), ApiError> {
    let workspace = state
        .storage
        .create_shared_workspace(&subject.user_id, &request.organization_id, &request.name)
        .await
        .map_err(map_storage_error)?;
    Ok((StatusCode::CREATED, Json(workspace)))
}

/// The caller's workspaces; every workspace for a system administrator.
async fn list_workspaces(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<SharedWorkspace>>, ApiError> {
    let workspaces = if subject.is_system_admin {
        state.storage.list_shared_workspaces_for_system_admin(&subject.user_id).await
    } else {
        state.storage.list_shared_workspaces_for_user(&subject.user_id).await
    };
    workspaces.map(Json).map_err(ApiError::internal)
}

async fn update_workspace(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<UpdateWorkspaceRequest>,
) -> Result<Json<Workspace>, ApiError> {
    require_workspace_owner(&state, &subject, &workspace_id).await?;
    let workspace = state
        .core
        .update_workspace_name(workspace_id.clone(), request.name)
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "workspace_updated",
        "workspace",
        Some(&workspace_id),
        format!("Renamed the workspace to {}.", workspace.name),
    )
    .await;
    Ok(Json(workspace))
}

async fn delete_workspace(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_workspace_owner(&state, &subject, &workspace_id).await?;
    state
        .core
        .delete_workspace(workspace_id)
        .await
        .map_err(map_core_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn workspace_overview(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceOverview>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .core
        .workspace_overview(workspace_id)
        .await
        .map(Json)
        .map_err(map_core_error)
}

async fn workspace_metrics(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceMetricsResponse>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let overview = state
        .core
        .workspace_overview(workspace_id.clone())
        .await
        .map_err(map_core_error)?;
    let artifacts = state
        .core
        .list_artifacts(workspace_id.clone())
        .await
        .map_err(map_core_error)?;
    let activity = state
        .storage
        .list_workspace_activity(&workspace_id, 100)
        .await
        .map_err(map_storage_error)?;
    let members = state
        .storage
        .list_workspace_members(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let collaboration = state
        .storage
        .workspace_collaboration_counts(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let mut artifact_types = BTreeMap::<String, i64>::new();
    let mut artifact_bytes_by_type = BTreeMap::<String, i64>::new();
    let mut languages = BTreeMap::<String, i64>::new();
    let mut activity_actions = BTreeMap::<String, i64>::new();
    let mut member_roles = BTreeMap::<String, i64>::new();
    let today = Utc::now().date_naive();
    let mut activity_by_day = (0..14)
        .rev()
        .map(|offset| ((today - ChronoDuration::days(offset)).to_string(), 0))
        .collect::<BTreeMap<_, _>>();
    let freshness_threshold = (Utc::now() - ChronoDuration::days(7)).to_rfc3339();
    let mut indexed_artifact_count = 0;
    let mut total_artifact_bytes = 0;
    let mut indexed_artifact_bytes = 0;
    let mut artifacts_created_last_7_days = 0;
    let mut artifacts_updated_last_7_days = 0;

    for artifact in &artifacts {
        if artifact.indexed_at.is_some() {
            indexed_artifact_count += 1;
            indexed_artifact_bytes += artifact.size_bytes;
        }
        total_artifact_bytes += artifact.size_bytes;
        let artifact_type = artifact_type_label(&artifact.artifact_type).to_owned();
        *artifact_types.entry(artifact_type.clone()).or_default() += 1;
        *artifact_bytes_by_type.entry(artifact_type).or_default() += artifact.size_bytes;
        artifacts_created_last_7_days += i64::from(artifact.created_at >= freshness_threshold);
        artifacts_updated_last_7_days += i64::from(artifact.updated_at >= freshness_threshold);
        if let Some(language) = artifact
            .language
            .as_ref()
            .filter(|language| !language.trim().is_empty())
        {
            *languages.entry(language.clone()).or_default() += 1;
        }
    }
    for event in &activity {
        *activity_actions
            .entry(event.action.replace('_', " "))
            .or_default() += 1;
        if let Some(day) = event.created_at.get(..10) {
            if let Some(count) = activity_by_day.get_mut(day) {
                *count += 1;
            }
        }
    }
    for member in &members {
        *member_roles
            .entry(workspace_role_label(&member.role).to_owned())
            .or_default() += 1;
    }

    let embedded_chunk_count = state
        .core
        .embedding_coverage(&workspace_id)
        .await
        .map_err(map_core_error)?
        .map(|(embedded, _)| embedded);

    Ok(Json(WorkspaceMetricsResponse {
        workspace_id,
        generated_at: Utc::now().to_rfc3339(),
        embedded_chunk_count,
        source_count: overview.source_count,
        member_count: members.len() as i64,
        artifact_count: overview.artifact_count,
        indexed_artifact_count,
        pending_artifact_count: overview.artifact_count - indexed_artifact_count,
        total_artifact_bytes,
        indexed_artifact_bytes,
        pending_artifact_bytes: total_artifact_bytes - indexed_artifact_bytes,
        chunk_count: overview.chunk_count,
        symbol_count: overview.symbol_count,
        memory_card_count: overview.memory_card_count,
        open_task_count: collaboration.open_tasks,
        in_progress_task_count: collaboration.in_progress_tasks,
        blocked_task_count: collaboration.blocked_tasks,
        completed_task_count: collaboration.completed_tasks,
        overdue_task_count: collaboration.overdue_tasks,
        comment_count: collaboration.comments,
        recent_activity_count: activity.len() as i64,
        artifacts_created_last_7_days,
        artifacts_updated_last_7_days,
        activity_actions: metric_breakdown(activity_actions),
        activity_by_day: metric_timeline(activity_by_day),
        member_roles: metric_breakdown(member_roles),
        artifact_types: metric_breakdown(artifact_types),
        artifact_bytes_by_type: metric_breakdown(artifact_bytes_by_type),
        languages: metric_breakdown(languages),
    }))
}

fn artifact_type_label(artifact_type: &ArtifactType) -> &'static str {
    match artifact_type {
        ArtifactType::File => "File",
        ArtifactType::MarkdownDoc => "Markdown",
        ArtifactType::CodeFile => "Code",
        ArtifactType::Image => "Image",
        ArtifactType::Issue => "Issue",
        ArtifactType::Pr => "Pull request",
        ArtifactType::Decision => "Decision",
        ArtifactType::Incident => "Incident",
        ArtifactType::Runbook => "Runbook",
        ArtifactType::ApiSpec => "API specification",
        ArtifactType::Note => "Note",
        ArtifactType::Repository => "Repository",
    }
}

fn metric_breakdown(values: BTreeMap<String, i64>) -> Vec<WorkspaceMetricBreakdown> {
    let mut values = values
        .into_iter()
        .map(|(label, value)| WorkspaceMetricBreakdown { label, value })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        right
            .value
            .cmp(&left.value)
            .then_with(|| left.label.cmp(&right.label))
    });
    values
}

fn metric_timeline(values: BTreeMap<String, i64>) -> Vec<WorkspaceMetricBreakdown> {
    values
        .into_iter()
        .map(|(label, value)| WorkspaceMetricBreakdown { label, value })
        .collect()
}

async fn workspace_capabilities(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceCapabilities>, ApiError> {
    let role = require_workspace_read(&state, &subject, &workspace_id).await?;
    let may_use_ai = ai_access::role_may_use_ai(&state, &workspace_id, &role).await?;
    Ok(Json(capabilities_for_role(role, may_use_ai)))
}

async fn workspace_knowledge_map(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<KnowledgeMap>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .core
        .knowledge_map(&workspace_id)
        .await
        .map(Json)
        .map_err(map_core_error)
}

async fn generate_workspace_ai_overview(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceAiOverview>, ApiError> {
    let role = require_workspace_read(&state, &subject, &workspace_id).await?;
    let provider = state
        .storage
        .list_provider_settings(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .find(|setting| setting.enabled && setting.purpose() == "text");

    let Some(provider) = provider else {
        return Ok(Json(WorkspaceAiOverview {
            provider_configured: false,
            provider_name: None,
            summary_markdown: None,
            citations: Vec::new(),
            warnings: vec![
                "No enabled AI provider is configured for this workspace. Evidence remains available locally; configure a workspace provider before generating an AI overview.".to_owned(),
            ],
        }));
    };

    ai_access::authorize_ai_use(&state, &subject, &workspace_id, &role).await?;
    let result = state
        .core
        .summarize_workspace(workspace_id.clone(), provider.id.clone())
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "ai_overview_generated",
        "workspace",
        Some(&workspace_id),
        "Generated a citation-backed AI workspace overview.".to_owned(),
    )
    .await;
    Ok(Json(WorkspaceAiOverview {
        provider_configured: true,
        provider_name: Some(provider.name),
        summary_markdown: Some(result.summary_markdown),
        citations: result.citations,
        warnings: result.warnings,
    }))
}

async fn ask_workspace(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<AskWorkspaceRequest>,
) -> Result<Json<AskAnswer>, ApiError> {
    let role = require_workspace_read(&state, &subject, &workspace_id).await?;
    let provider = state
        .storage
        .list_provider_settings(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .find(|setting| setting.enabled && setting.purpose() == "text")
        .ok_or_else(|| {
            ApiError::bad_request(
                "No enabled AI provider is configured for this workspace. An administrator can configure one in Settings.",
            )
        })?;
    ai_access::authorize_ai_use(&state, &subject, &workspace_id, &role).await?;
    let question = request.question.clone();
    let answer = state
        .core
        .ask_workspace(AskRequest {
            workspace_id: workspace_id.clone(),
            question: request.question,
            provider_id: Some(provider.id),
            limit: request.limit,
        })
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "ai_question_answered",
        "workspace",
        Some(&workspace_id),
        format!(
            "Asked: {}",
            question.trim().chars().take(240).collect::<String>()
        ),
    )
    .await;
    Ok(Json(answer))
}

async fn list_workspace_ai_providers(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<SharedAiProviderSettings>>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_provider_settings(&workspace_id)
        .await
        .map(|providers| {
            providers
                .into_iter()
                .map(shared_provider_settings)
                .collect()
        })
        .map(Json)
        .map_err(map_storage_error)
}

async fn save_workspace_ai_provider(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SaveAiProviderRequest>,
) -> Result<Json<SharedAiProviderSettings>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let provider_id = request.id.unwrap_or_default();
    let existing = if provider_id.trim().is_empty() {
        None
    } else {
        let settings = state
            .storage
            .get_provider_settings(&provider_id)
            .await
            .map_err(map_storage_error)?;
        if settings.workspace_id.as_deref() != Some(workspace_id.as_str()) {
            return Err(ApiError::forbidden());
        }
        Some(settings)
    };
    let supplied_key = request.api_key.filter(|value| !value.trim().is_empty());
    // A stored key is only ever sent to the service it was entered for. When
    // the provider type or address changes, the key must be entered again, so
    // an administrator cannot point a colleague's key at another server.
    let endpoint_changed = existing.as_ref().is_some_and(|settings| {
        provider_endpoint(&settings.provider_type, settings.base_url.as_deref())
            != provider_endpoint(&request.provider_type, request.base_url.as_deref())
    });
    let stored_key = existing.as_ref().and_then(|settings| settings.api_key.clone());
    let api_key = match supplied_key.clone() {
        Some(key) => Some(key),
        None if endpoint_changed => {
            if stored_key.is_some() && request.provider_type == "openrouter" {
                return Err(ApiError::bad_request(
                    "Enter the API key again: the provider's type or address changed, and a saved key is only sent to the address it was saved for.",
                ));
            }
            None
        }
        None => stored_key,
    };
    if supplied_key.is_some() {
        tracing::info!(target: "audit", user_id = %subject.user_id, workspace_id = %workspace_id, "AI provider API key set");
    }
    let cloud_content_acknowledged = request.cloud_content_acknowledged
        || existing
            .as_ref()
            .and_then(|settings| settings.metadata.get("cloud_content_acknowledged"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    let purpose = match request.purpose.as_deref() {
        Some("vision") => "vision",
        Some("text") => "text",
        Some("embedding") => "embedding",
        Some(_) => return Err(ApiError::bad_request("Provider purpose must be text, vision or embedding.")),
        None => existing
            .as_ref()
            .map(|settings| settings.purpose())
            .unwrap_or("text"),
    };
    let settings = ProviderSettings {
        id: provider_id,
        workspace_id: Some(workspace_id.clone()),
        provider_type: request.provider_type,
        name: request.name,
        base_url: request.base_url,
        model: request.model,
        embedding_model: None,
        enabled: request.enabled,
        metadata: json!({
            "cloud_content_acknowledged": cloud_content_acknowledged,
            "purpose": purpose,
        }),
        api_key,
    };
    // Every validation failure is about the submitted settings.
    repomemo_ai::validate_settings(&settings)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let provider = state
        .core
        .save_provider_settings(settings)
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "ai_provider_updated",
        "ai_provider",
        Some(&provider.id),
        format!("Updated AI provider: {}.", provider.name),
    )
    .await;
    if provider.enabled && provider.purpose() == "vision" {
        // Images stuck behind a missing or broken provider are retried now.
        state.index_queue.resume_pending().await;
    }
    if provider.enabled && provider.purpose() == "embedding" {
        // Existing chunks get vectors from the (possibly new) model.
        state.embedding_queue.request(&workspace_id);
    }
    Ok(Json(shared_provider_settings(provider)))
}

async fn list_artifact_index_failures(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<ArtifactIndexFailure>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_index_failures(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn test_workspace_ai_provider(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, provider_id)): Path<(String, String)>,
) -> Result<Json<ProviderTestResult>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let provider = state
        .storage
        .get_provider_settings(&provider_id)
        .await
        .map_err(map_storage_error)?;
    if provider.workspace_id.as_deref() != Some(workspace_id.as_str()) {
        return Err(ApiError::forbidden());
    }
    let result = state
        .core
        .test_provider(provider_id.clone())
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "ai_provider_tested",
        "ai_provider",
        Some(&provider_id),
        format!("Tested AI provider: {}.", provider.name),
    )
    .await;
    Ok(Json(result))
}

/// The service a provider sends requests (and its key) to.
fn provider_endpoint(provider_type: &str, base_url: Option<&str>) -> (String, String) {
    (
        provider_type.trim().to_owned(),
        base_url
            .unwrap_or_default()
            .trim()
            .trim_end_matches('/')
            .to_ascii_lowercase(),
    )
}

fn shared_provider_settings(provider: ProviderSettings) -> SharedAiProviderSettings {
    let purpose = provider.purpose().to_owned();
    SharedAiProviderSettings {
        id: provider.id,
        provider_type: provider.provider_type,
        name: provider.name,
        base_url: provider.base_url,
        purpose,
        model: provider.model,
        enabled: provider.enabled,
    }
}

async fn list_workspace_activity(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<WorkspaceActivityEvent>>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_workspace_activity(&workspace_id, 100)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn workspace_activity_calendar(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceActivityCalendarResponse>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let today = Utc::now().date_naive();
    let mut activity_by_day = (0..365)
        .rev()
        .map(|offset| ((today - ChronoDuration::days(offset)).to_string(), 0))
        .collect::<BTreeMap<_, _>>();
    let rows = state
        .storage
        .workspace_activity_by_day(
            &workspace_id,
            &(Utc::now() - ChronoDuration::days(364)).to_rfc3339(),
        )
        .await
        .map_err(map_storage_error)?;
    for (day, count) in rows {
        if let Some(value) = activity_by_day.get_mut(&day) {
            *value = count;
        }
    }
    let total_activity_count = activity_by_day.values().sum();
    Ok(Json(WorkspaceActivityCalendarResponse {
        total_activity_count,
        activity_by_day: metric_timeline(activity_by_day),
    }))
}

async fn list_collaboration_tasks(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<CollaborationTask>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_collaboration_tasks(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn get_collaboration_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<CollaborationTask>, ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &task.workspace_id).await?;
    Ok(Json(task))
}

async fn create_collaboration_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SaveCollaborationTaskRequest>,
) -> Result<(StatusCode, Json<CollaborationTask>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let payload = normalize_collaboration_task(&state, &workspace_id, request).await?;
    let task = state
        .storage
        .create_collaboration_task(&workspace_id, &subject.user_id, payload)
        .await
        .map_err(map_storage_error)?;
    notify_task_assignee(&state, &subject.user_id, &task).await;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "task_created",
        "task",
        Some(&task.id),
        format!("Created task: {}.", task.title),
    )
    .await;
    Ok((StatusCode::CREATED, Json(task)))
}

async fn update_collaboration_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Json(request): Json<SaveCollaborationTaskRequest>,
) -> Result<Json<CollaborationTask>, ApiError> {
    let current = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    let payload = normalize_collaboration_task(&state, &current.workspace_id, request).await?;
    let task = state
        .storage
        .update_collaboration_task(&task_id, payload)
        .await
        .map_err(map_storage_error)?;
    if current.assignee.as_ref().map(|member| member.id.as_str())
        != task.assignee.as_ref().map(|member| member.id.as_str())
    {
        notify_task_assignee(&state, &subject.user_id, &task).await;
    }
    record_workspace_activity(
        &state,
        &task.workspace_id,
        &subject.user_id,
        "task_updated",
        "task",
        Some(&task.id),
        format!(
            "Updated task: {} ({}).",
            task.title,
            task.status.replace('_', " ")
        ),
    )
    .await;
    Ok(Json(task))
}

async fn delete_collaboration_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    let role = require_workspace_read(&state, &subject, &task.workspace_id).await?;
    if task.created_by.id != subject.user_id
        && !matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin)
    {
        return Err(ApiError::forbidden());
    }
    state
        .storage
        .delete_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &task.workspace_id,
        &subject.user_id,
        "task_deleted",
        "task",
        Some(&task.id),
        format!("Deleted task: {}.", task.title),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_saved_searches(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<SavedSearch>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_saved_searches(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}
async fn create_saved_search(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SaveSearchRequest>,
) -> Result<(StatusCode, Json<SavedSearch>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let name = request.name.trim().to_owned();
    let query = request.query.trim().to_owned();
    let result_limit = request.result_limit.unwrap_or(20);
    if name.is_empty()
        || name.len() > 120
        || query.is_empty()
        || query.len() > 500
        || !(1..=100).contains(&result_limit)
    {
        return Err(ApiError::bad_request(
            "Saved search needs a name, query, and a result limit between 1 and 100.",
        ));
    }
    let saved = state
        .storage
        .create_saved_search(
            &workspace_id,
            &subject.user_id,
            NewSavedSearch {
                name,
                query,
                artifact_types: request.artifact_types,
                languages: request.languages,
                source_ids: request.source_ids,
                result_limit,
            },
        )
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "saved_search_created",
        "saved_search",
        Some(&saved.id),
        format!("Saved retrieval workflow: {}.", saved.name),
    )
    .await;
    Ok((StatusCode::CREATED, Json(saved)))
}
async fn delete_saved_search(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(search_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let saved = state
        .storage
        .get_saved_search(&search_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &saved.workspace_id).await?;
    state
        .storage
        .delete_saved_search(&search_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn list_task_checklist(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Vec<TaskChecklistItem>>, ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &task.workspace_id).await?;
    state
        .storage
        .list_task_checklist_items(&task_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}
async fn create_task_checklist_item(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Json(request): Json<CreateChecklistItemRequest>,
) -> Result<(StatusCode, Json<TaskChecklistItem>), ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &task.workspace_id).await?;
    let body = request.body.trim().to_owned();
    if body.is_empty() || body.len() > 500 {
        return Err(ApiError::bad_request(
            "Checklist item must be between 1 and 500 characters.",
        ));
    }
    let item = state
        .storage
        .create_task_checklist_item(
            &task_id,
            &task.workspace_id,
            &subject.user_id,
            NewTaskChecklistItem { body },
        )
        .await
        .map_err(map_storage_error)?;
    Ok((StatusCode::CREATED, Json(item)))
}
async fn toggle_task_checklist_item(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(item_id): Path<String>,
    Json(request): Json<ToggleChecklistItemRequest>,
) -> Result<Json<TaskChecklistItem>, ApiError> {
    let current = state
        .storage
        .get_task_checklist_item(&item_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    state
        .storage
        .toggle_task_checklist_item(&item_id, &subject.user_id, request.completed)
        .await
        .map(Json)
        .map_err(map_storage_error)
}
async fn delete_task_checklist_item(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(item_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let current = state
        .storage
        .get_task_checklist_item(&item_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    state
        .storage
        .delete_task_checklist_item(&item_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn normalize_collaboration_task(
    state: &AppState,
    workspace_id: &str,
    request: SaveCollaborationTaskRequest,
) -> Result<NewCollaborationTask, ApiError> {
    let title = request.title.trim().to_owned();
    let description = request.description.trim().to_owned();
    if title.is_empty() || title.len() > 180 {
        return Err(ApiError::bad_request(
            "Task title must be between 1 and 180 characters.",
        ));
    }
    if description.len() > 5_000 {
        return Err(ApiError::bad_request(
            "Task description cannot exceed 5,000 characters.",
        ));
    }
    if !matches!(
        request.status.as_str(),
        "open" | "in_progress" | "blocked" | "done"
    ) {
        return Err(ApiError::bad_request(
            "Task status must be open, in_progress, blocked, or done.",
        ));
    }
    if !matches!(
        request.priority.as_str(),
        "low" | "medium" | "high" | "urgent"
    ) {
        return Err(ApiError::bad_request(
            "Task priority must be low, medium, high, or urgent.",
        ));
    }
    if let Some(user_id) = request.assignee_user_id.as_deref() {
        if state
            .storage
            .workspace_role_for_user(user_id, workspace_id)
            .await
            .map_err(map_storage_error)?
            .is_none()
        {
            return Err(ApiError::bad_request(
                "Task assignee must be a workspace member.",
            ));
        }
    }
    if let Some(artifact_id) = request.artifact_id.as_deref() {
        let artifact = state
            .storage
            .get_artifact(artifact_id)
            .await
            .map_err(map_storage_error)?;
        if artifact.summary.workspace_id != workspace_id {
            return Err(ApiError::bad_request(
                "Task evidence must belong to this workspace.",
            ));
        }
    }
    let due_at = request.due_at.filter(|value| !value.trim().is_empty());
    if due_at.as_ref().is_some_and(|value| value.len() > 64) {
        return Err(ApiError::bad_request("Task due date is invalid."));
    }
    Ok(NewCollaborationTask {
        title,
        description,
        status: request.status,
        priority: request.priority,
        assignee_user_id: request.assignee_user_id,
        artifact_id: request.artifact_id,
        due_at,
    })
}

async fn list_artifact_comments(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<Vec<ArtifactComment>>, ApiError> {
    let artifact = state
        .storage
        .get_artifact(&artifact_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &artifact.summary.workspace_id).await?;
    state
        .storage
        .list_artifact_comments(&artifact_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn create_artifact_comment(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
    Json(request): Json<SaveArtifactCommentRequest>,
) -> Result<(StatusCode, Json<ArtifactComment>), ApiError> {
    let artifact = state
        .storage
        .get_artifact(&artifact_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &artifact.summary.workspace_id).await?;
    let body = validate_comment_body(&request.body)?;
    let comment = state
        .storage
        .create_artifact_comment(
            &artifact.summary.workspace_id,
            &artifact_id,
            &subject.user_id,
            &body,
        )
        .await
        .map_err(map_storage_error)?;
    notify_artifact_mentions(
        &state,
        &subject.user_id,
        &artifact.summary.workspace_id,
        &artifact_id,
        &artifact.summary.title,
        &body,
    )
    .await;
    record_workspace_activity(
        &state,
        &comment.workspace_id,
        &subject.user_id,
        "comment_added",
        "artifact",
        Some(&artifact_id),
        format!("Commented on evidence: {}.", artifact.summary.title),
    )
    .await;
    Ok((StatusCode::CREATED, Json(comment)))
}

async fn update_artifact_comment(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(comment_id): Path<String>,
    Json(request): Json<SaveArtifactCommentRequest>,
) -> Result<Json<ArtifactComment>, ApiError> {
    let current = state
        .storage
        .get_artifact_comment(&comment_id)
        .await
        .map_err(map_storage_error)?;
    let role = require_workspace_read(&state, &subject, &current.workspace_id).await?;
    if current.author.id != subject.user_id
        && !matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin)
    {
        return Err(ApiError::forbidden());
    }
    let body = validate_comment_body(&request.body)?;
    let updated = state
        .storage
        .update_artifact_comment(&comment_id, &body)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &updated.workspace_id,
        &subject.user_id,
        "comment_updated",
        "artifact",
        Some(&updated.artifact_id),
        "Updated an evidence comment.".to_owned(),
    )
    .await;
    Ok(Json(updated))
}

async fn delete_artifact_comment(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(comment_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let comment = state
        .storage
        .get_artifact_comment(&comment_id)
        .await
        .map_err(map_storage_error)?;
    let role = require_workspace_read(&state, &subject, &comment.workspace_id).await?;
    if comment.author.id != subject.user_id
        && !matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin)
    {
        return Err(ApiError::forbidden());
    }
    state
        .storage
        .delete_artifact_comment(&comment_id)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &comment.workspace_id,
        &subject.user_id,
        "comment_deleted",
        "artifact",
        Some(&comment.artifact_id),
        "Removed an evidence comment.".to_owned(),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

fn validate_comment_body(body: &str) -> Result<String, ApiError> {
    let body = body.trim().to_owned();
    if body.is_empty() || body.len() > 5_000 {
        return Err(ApiError::bad_request(
            "Comment must be between 1 and 5,000 characters.",
        ));
    }
    Ok(body)
}

async fn record_workspace_activity(
    state: &AppState,
    workspace_id: &str,
    actor_user_id: &str,
    action: &str,
    subject_type: &str,
    subject_id: Option<&str>,
    summary: String,
) {
    if let Err(error) = state
        .storage
        .record_workspace_activity(
            workspace_id,
            Some(actor_user_id),
            action,
            subject_type,
            subject_id,
            &summary,
        )
        .await
    {
        tracing::error!(error = %error, workspace_id, action, "Failed to record workspace activity");
    }
}

async fn notify_task_assignee(state: &AppState, actor_user_id: &str, task: &CollaborationTask) {
    let Some(assignee) = task.assignee.as_ref() else {
        return;
    };
    if assignee.id == actor_user_id {
        return;
    }
    create_notification(
        state,
        NewSharedNotification {
            user_id: assignee.id.clone(),
            workspace_id: Some(task.workspace_id.clone()),
            notification_type: "task_assigned".to_owned(),
            title: "Task assigned to you".to_owned(),
            body: format!("{} was assigned to you.", task.title),
            href: format!("/workspaces/{}/tasks", task.workspace_id),
        },
    )
    .await;
}

async fn notify_artifact_mentions(
    state: &AppState,
    actor_user_id: &str,
    workspace_id: &str,
    artifact_id: &str,
    artifact_title: &str,
    body: &str,
) {
    let members = match state.storage.list_workspace_members(workspace_id).await {
        Ok(members) => members,
        Err(error) => {
            tracing::error!(error = %error, workspace_id, "Failed to resolve evidence comment mentions");
            return;
        }
    };
    let mentioned_emails = body
        .split_whitespace()
        .filter_map(|token| {
            token
                .trim_matches(|character: char| {
                    matches!(
                        character,
                        ',' | '.' | ':' | ';' | '!' | '?' | ')' | ']' | '}' | '"' | '\''
                    )
                })
                .strip_prefix('@')
                .map(|email| email.to_ascii_lowercase())
        })
        .collect::<BTreeSet<_>>();
    for member in members {
        let Some(email) = member.user.email.as_deref() else {
            continue;
        };
        if member.user.id == actor_user_id
            || !mentioned_emails.contains(&email.to_ascii_lowercase())
        {
            continue;
        }
        create_notification(
            state,
            NewSharedNotification {
                user_id: member.user.id,
                workspace_id: Some(workspace_id.to_owned()),
                notification_type: "evidence_mention".to_owned(),
                title: "You were mentioned in evidence discussion".to_owned(),
                body: format!("You were mentioned on {}.", artifact_title),
                href: format!("/workspaces/{workspace_id}/artifacts/{artifact_id}"),
            },
        )
        .await;
    }
}

async fn create_notification(state: &AppState, notification: NewSharedNotification) {
    if let Err(error) = state.storage.create_shared_notification(notification).await {
        tracing::error!(error = %error, "Failed to create shared notification");
    }
}

async fn list_workspace_members(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<WorkspaceMember>>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_workspace_members(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn upsert_workspace_member(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<UpsertWorkspaceMemberRequest>,
) -> Result<Json<WorkspaceMember>, ApiError> {
    let caller_role = require_workspace_admin(&state, &subject, &workspace_id).await?;
    if matches!(&caller_role, WorkspaceRole::Admin) {
        if matches!(request.role, WorkspaceRole::Admin) {
            return Err(ApiError::forbidden());
        }
        if let Some(user) = state
            .storage
            .find_user_by_email(&request.email)
            .await
            .map_err(map_storage_error)?
        {
            let target_role = state
                .storage
                .workspace_role_for_user(&user.id, &workspace_id)
                .await
                .map_err(map_storage_error)?;
            if matches!(
                target_role,
                Some(WorkspaceRole::Owner | WorkspaceRole::Admin)
            ) {
                return Err(ApiError::forbidden());
            }
        }
    }
    let member = state
        .storage
        .upsert_workspace_member(&workspace_id, &request.email, request.role)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "member_updated",
        "user",
        Some(&member.user.id),
        format!(
            "Set {} to {}.",
            member.user.display_name,
            workspace_role_label(&member.role)
        ),
    )
    .await;
    Ok(Json(member))
}

async fn remove_workspace_member(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, user_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let caller_role = require_workspace_admin(&state, &subject, &workspace_id).await?;
    let member = state
        .storage
        .list_workspace_members(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .find(|member| member.user.id == user_id);
    if matches!(&caller_role, WorkspaceRole::Admin)
        && matches!(
            member.as_ref().map(|entry| &entry.role),
            Some(WorkspaceRole::Owner | WorkspaceRole::Admin)
        )
    {
        return Err(ApiError::forbidden());
    }
    let member_name = member
        .map(|entry| entry.user.display_name)
        .unwrap_or_else(|| "a workspace member".to_owned());
    state
        .storage
        .remove_workspace_member(&workspace_id, &user_id)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "member_removed",
        "user",
        Some(&user_id),
        format!("Removed {member_name} from the workspace."),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// Optional `limit`/`offset` paging for list endpoints. Without `limit` the
/// full list is returned, so existing clients keep working.
#[derive(Debug, Default, Deserialize)]
struct PageQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

const MAX_PAGE_SIZE: usize = 500;

impl PageQuery {
    /// Returns the requested page plus an `X-Total-Count` header when paged.
    fn apply<T>(&self, items: Vec<T>) -> (HeaderMap, Vec<T>) {
        let mut headers = HeaderMap::new();
        let Some(limit) = self.limit else {
            return (headers, items);
        };
        let total = items.len();
        headers.insert(
            HeaderName::from_static("x-total-count"),
            HeaderValue::from(total),
        );
        let page = items
            .into_iter()
            .skip(self.offset.unwrap_or(0))
            .take(limit.clamp(1, MAX_PAGE_SIZE))
            .collect();
        (headers, page)
    }
}

async fn list_artifacts(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(page): Query<PageQuery>,
) -> Result<(HeaderMap, Json<Vec<SharedArtifactSummary>>), ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let folder_ids = state
        .storage
        .artifact_folder_ids(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let repository_ids = state
        .storage
        .repo_artifact_sources(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let artifacts = state
        .core
        .list_artifacts(workspace_id)
        .await
        .map_err(map_core_error)?;
    let (headers, artifacts) = page.apply(artifacts);
    Ok((
        headers,
        Json(
            artifacts
                .into_iter()
                .map(|summary| SharedArtifactSummary {
                    folder_id: folder_ids.get(&summary.id).cloned(),
                    repository_id: repository_ids.get(&summary.id).cloned(),
                    summary,
                })
                .collect(),
        ),
    ))
}

async fn list_folders(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<Folder>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_folders(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn create_folder(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<CreateFolderRequest>,
) -> Result<(StatusCode, Json<Folder>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let folder = state
        .storage
        .create_folder(
            &workspace_id,
            request.parent_id.as_deref().filter(|id| !id.is_empty()),
            &request.name,
            Some(&subject.user_id),
        )
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "folder_created",
        "folder",
        Some(&folder.id),
        format!("Created folder: {}.", folder.name),
    )
    .await;
    Ok((StatusCode::CREATED, Json(folder)))
}

const FILE_LINK_ISSUER: &str = "repomemo-file-link";
const FILE_LINK_TTL_SECONDS: u64 = 300;

#[derive(Debug, Serialize, Deserialize)]
struct FileLinkClaims {
    sub: String,
    iss: String,
    exp: u64,
}

#[derive(Debug, Serialize)]
struct FileLinkResponse {
    token: String,
    filename: String,
    expires_in_seconds: u64,
}

/// The stored original as a download. Always an attachment, sandboxed and
/// never sniffed, so an uploaded HTML or SVG file cannot run in the browser.
fn file_response(summary: &ArtifactSummary, bytes: Vec<u8>) -> Response {
    let filename = summary
        .path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("download")
        .to_owned();
    let ascii: String = filename
        .chars()
        .map(|character| {
            if character.is_ascii_graphic() && !matches!(character, '"' | '\\' | '%') {
                character
            } else if character == ' ' {
                ' '
            } else {
                '_'
            }
        })
        .collect();
    let encoded: String = filename
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-_.".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect();
    let mut headers = HeaderMap::new();
    let content_type = summary
        .mime_type
        .as_deref()
        .and_then(|value| HeaderValue::from_str(value).ok())
        .unwrap_or_else(|| HeaderValue::from_static("application/octet-stream"));
    headers.insert(header::CONTENT_TYPE, content_type);
    if let Ok(value) = HeaderValue::from_str(&format!(
        "attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}"
    )) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("sandbox"),
    );
    (StatusCode::OK, headers, bytes).into_response()
}

async fn download_artifact_file(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Response, ApiError> {
    let summary = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(|_| ApiError::bad_request("Artifact was not found."))?;
    require_workspace_read(&state, &subject, &summary.workspace_id).await?;
    let bytes = state
        .storage
        .read_artifact_blob(&artifact_id)
        .await
        .map_err(map_storage_error)?;
    Ok(file_response(&summary, bytes))
}

async fn artifact_document_preview(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<DocumentPreview>, ApiError> {
    let summary = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(|_| ApiError::bad_request("Artifact was not found."))?;
    require_workspace_read(&state, &subject, &summary.workspace_id).await?;
    state
        .core
        .document_preview(&artifact_id)
        .await
        .map(Json)
        .map_err(map_core_error)
}

#[derive(Debug, Serialize)]
struct RenderStatusResponse {
    /// `ready`, `converting`, `failed`, `disabled` or `unsupported`.
    state: &'static str,
    message: Option<String>,
}

async fn rendered_preview_status(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<RenderStatusResponse>, ApiError> {
    let summary = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(|_| ApiError::bad_request("Artifact was not found."))?;
    require_workspace_read(&state, &subject, &summary.workspace_id).await?;
    let (name, message) = match state.converter.state(&state.storage, &summary) {
        RenderState::Ready => ("ready", None),
        RenderState::Converting => ("converting", None),
        RenderState::Disabled => ("disabled", None),
        RenderState::Unsupported => ("unsupported", None),
        RenderState::Failed(message) => ("failed", Some(message)),
    };
    Ok(Json(RenderStatusResponse {
        state: name,
        message,
    }))
}

/// The layout-accurate PDF of an Office file, once its conversion is done.
async fn rendered_preview_pdf(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Response, ApiError> {
    let summary = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(|_| ApiError::bad_request("Artifact was not found."))?;
    require_workspace_read(&state, &subject, &summary.workspace_id).await?;
    let path = state
        .converter
        .cached_pdf(&summary.content_hash)
        .ok_or_else(|| ApiError::bad_request("The rendered preview is not ready yet."))?;
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|error| ApiError::internal(error))?;
    let mut rendered = summary;
    rendered.mime_type = Some("application/pdf".to_owned());
    rendered.path = format!("{}.pdf", rendered.title);
    Ok(file_response(&rendered, bytes))
}

/// A short-lived, single-file link that a desktop app (Word, Excel, ...) can
/// fetch without the user's session. It grants read access to one artifact for
/// a few minutes and nothing else.
async fn create_file_link(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<FileLinkResponse>, ApiError> {
    let summary = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(|_| ApiError::bad_request("Artifact was not found."))?;
    require_workspace_read(&state, &subject, &summary.workspace_id).await?;
    let claims = FileLinkClaims {
        sub: artifact_id,
        iss: FILE_LINK_ISSUER.to_owned(),
        exp: jsonwebtoken::get_current_timestamp() + FILE_LINK_TTL_SECONDS,
    };
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .map_err(|error| ApiError::internal(error))?;
    let filename = summary
        .path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("download")
        .to_owned();
    Ok(Json(FileLinkResponse {
        token,
        filename,
        expires_in_seconds: FILE_LINK_TTL_SECONDS,
    }))
}

async fn shared_file_by_link(
    State(state): State<AppState>,
    Path((token, _filename)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[FILE_LINK_ISSUER]);
    let claims = decode::<FileLinkClaims>(
        &token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|_| ApiError::unauthorized())?
    .claims;
    let summary = state
        .storage
        .get_artifact_summary(&claims.sub)
        .await
        .map_err(|_| ApiError::unauthorized())?;
    let bytes = state
        .storage
        .read_artifact_blob(&claims.sub)
        .await
        .map_err(map_storage_error)?;
    Ok(file_response(&summary, bytes))
}

async fn rename_folder(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, folder_id)): Path<(String, String)>,
    Json(request): Json<RenameFolderRequest>,
) -> Result<Json<Folder>, ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    validate_folder(&state, &workspace_id, Some(&folder_id)).await?;
    let folder = state
        .storage
        .rename_folder(&folder_id, &request.name)
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "folder_renamed",
        "folder",
        Some(&folder.id),
        format!("Renamed folder to {}.", folder.name),
    )
    .await;
    Ok(Json(folder))
}

/// Deletes a folder together with every folder and file inside it.
async fn delete_folder(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, folder_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let folder_id = validate_folder(&state, &workspace_id, Some(&folder_id))
        .await?
        .ok_or_else(|| ApiError::bad_request("Folder was not found."))?;
    let folder = state
        .storage
        .get_folder(&folder_id)
        .await
        .map_err(map_storage_error)?;
    let (folder_ids, artifact_ids) = state
        .storage
        .folder_subtree(&folder_id)
        .await
        .map_err(map_storage_error)?;
    for artifact_id in &artifact_ids {
        state
            .core
            .delete_artifact(artifact_id.clone())
            .await
            .map_err(map_core_error)?;
    }
    state
        .storage
        .delete_folders(&folder_ids)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "folder_deleted",
        "folder",
        Some(&folder_id),
        format!(
            "Deleted folder {} with {} file(s) and {} subfolder(s).",
            folder.name,
            artifact_ids.len(),
            folder_ids.len() - 1
        ),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn move_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
    Json(request): Json<MoveArtifactRequest>,
) -> Result<Json<SharedArtifactSummary>, ApiError> {
    let current = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    let workspace_id = current.summary.workspace_id.clone();
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let folder_id = validate_folder(&state, &workspace_id, request.folder_id.as_deref()).await?;
    state
        .storage
        .move_artifact_to_folder(&artifact_id, folder_id.as_deref())
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "evidence_moved",
        "artifact",
        Some(&artifact_id),
        format!("Moved evidence: {}.", current.summary.title),
    )
    .await;
    Ok(Json(SharedArtifactSummary {
        summary: current.summary,
        folder_id,
        repository_id: None,
    }))
}

/// Checks that a folder belongs to the workspace before anything is stored in it.
async fn validate_folder(
    state: &AppState,
    workspace_id: &str,
    folder_id: Option<&str>,
) -> Result<Option<String>, ApiError> {
    let Some(folder_id) = folder_id.filter(|id| !id.is_empty()) else {
        return Ok(None);
    };
    let folder = state
        .storage
        .get_folder(folder_id)
        .await
        .map_err(|_| ApiError::bad_request("Folder was not found."))?;
    if folder.workspace_id != workspace_id {
        return Err(ApiError::bad_request("Folder was not found."));
    }
    Ok(Some(folder.id))
}

async fn query_artifacts(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(page): Query<PageQuery>,
    Json(request): Json<QueryArtifactsRequest>,
) -> Result<(HeaderMap, Json<Vec<ArtifactSummary>>), ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let query = request.query.trim().to_lowercase();
    let languages = request
        .languages
        .into_iter()
        .map(|language| language.to_lowercase())
        .collect::<Vec<_>>();
    let artifacts = state
        .core
        .list_artifacts(workspace_id)
        .await
        .map_err(map_core_error)?
        .into_iter()
        .filter(|artifact| {
            let query_matches = query.is_empty()
                || artifact.title.to_lowercase().contains(&query)
                || artifact.path.to_lowercase().contains(&query);
            let type_matches = request.artifact_types.is_empty()
                || request
                    .artifact_types
                    .iter()
                    .any(|artifact_type| artifact_type == &artifact.artifact_type);
            let language_matches = languages.is_empty()
                || artifact
                    .language
                    .as_deref()
                    .map(|language| {
                        languages
                            .iter()
                            .any(|candidate| candidate == &language.to_lowercase())
                    })
                    .unwrap_or(false);
            let source_matches = request.source_ids.is_empty()
                || request
                    .source_ids
                    .iter()
                    .any(|source_id| source_id == &artifact.source_id);
            let indexing_matches = request
                .indexed
                .map(|indexed| artifact.indexed_at.is_some() == indexed)
                .unwrap_or(true);
            query_matches && type_matches && language_matches && source_matches && indexing_matches
        })
        .collect::<Vec<_>>();
    let (headers, artifacts) = page.apply(artifacts);
    Ok((headers, Json(artifacts)))
}

async fn create_text_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<CreateTextArtifactRequest>,
) -> Result<(StatusCode, Json<SharedArtifactSummary>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let folder_id = validate_folder(&state, &workspace_id, request.folder_id.as_deref()).await?;
    let artifact = state
        .core
        .import_text(
            workspace_id.clone(),
            request.title,
            request.content,
            request.language,
        )
        .await
        .map_err(map_core_error)?;
    if folder_id.is_some() {
        state
            .storage
            .set_artifact_folder(&artifact.id, folder_id.as_deref())
            .await
            .map_err(map_storage_error)?;
    }
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "evidence_stored",
        "artifact",
        Some(&artifact.id),
        format!("Stored evidence: {}.", artifact.title),
    )
    .await;
    state.index_queue.enqueue(&artifact, Some(&subject.user_id));
    Ok((
        StatusCode::CREATED,
        Json(SharedArtifactSummary {
            summary: artifact,
            folder_id,
            repository_id: None,
        }),
    ))
}

async fn upload_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<SharedArtifactSummary>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let requested_folder = headers
        .get("x-repomemo-folder-id")
        .and_then(|value| value.to_str().ok());
    let folder_id = validate_folder(&state, &workspace_id, requested_folder).await?;
    let filename = headers
        .get("x-repomemo-filename")
        .and_then(decode_filename_header)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request("X-RepoMemo-Filename is required for uploads."))?;
    if filename.chars().count() > MAX_UPLOAD_FILENAME_CHARS || filename.chars().any(char::is_control) {
        return Err(ApiError::bad_request(format!(
            "File names must be at most {MAX_UPLOAD_FILENAME_CHARS} characters long, without control characters."
        )));
    }
    let mime_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    state
        .core
        .ensure_upload_allowed(&workspace_id, &filename)
        .await
        .map_err(map_core_error)?;
    let artifact = state
        .core
        .import_upload(workspace_id.clone(), filename, body.to_vec(), mime_type)
        .await
        .map_err(map_core_error)?;
    // Start rendering the faithful preview now so it is ready when opened.
    state
        .converter
        .ensure(state.storage.clone(), artifact.clone());
    if folder_id.is_some() {
        state
            .storage
            .set_artifact_folder(&artifact.id, folder_id.as_deref())
            .await
            .map_err(map_storage_error)?;
    }
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "evidence_uploaded",
        "artifact",
        Some(&artifact.id),
        format!("Uploaded evidence: {}.", artifact.title),
    )
    .await;
    state.index_queue.enqueue(&artifact, Some(&subject.user_id));
    Ok((
        StatusCode::CREATED,
        Json(SharedArtifactSummary {
            summary: artifact,
            folder_id,
            repository_id: None,
        }),
    ))
}

/// The upload's file name. Clients percent-encode it so names outside ASCII
/// survive the header; a plain ASCII name is used as it is.
fn decode_filename_header(value: &HeaderValue) -> Option<String> {
    let raw = std::str::from_utf8(value.as_bytes()).ok()?;
    if !raw.contains('%') {
        return Some(raw.to_owned());
    }
    percent_decode(raw).or_else(|| Some(raw.to_owned()))
}

/// Decodes `%XX` escapes; `None` when an escape is malformed or the result
/// is not UTF-8.
fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = value.get(index + 1..index + 3)?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

async fn get_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<ArtifactDetail>, ApiError> {
    let mut artifact = state
        .core
        .get_artifact(artifact_id)
        .await
        .map_err(map_core_error)?;
    let role = require_workspace_read(&state, &subject, &artifact.summary.workspace_id).await?;
    // Stored chunks are an administrative view of the index. Everyone else
    // works with the artifact content and search results.
    if !capabilities_for_role(role, false).can_inspect_index {
        artifact.chunks.clear();
    }
    Ok(Json(artifact))
}

async fn list_artifact_chunks(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<Vec<Chunk>>, ApiError> {
    let summary = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_admin(&state, &subject, &summary.workspace_id).await?;
    state
        .storage
        .list_chunks_for_artifact(&artifact_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn update_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
    Json(request): Json<UpdateArtifactRequest>,
) -> Result<Json<ArtifactSummary>, ApiError> {
    let current = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_write(&state, &subject, &current.summary.workspace_id).await?;
    repositories::ensure_not_repository_file(&state, &artifact_id).await?;
    let artifact = state
        .core
        .update_artifact_title(artifact_id, request.title)
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &artifact.workspace_id,
        &subject.user_id,
        "artifact_updated",
        "artifact",
        Some(&artifact.id),
        format!("Renamed evidence to {}.", artifact.title),
    )
    .await;
    Ok(Json(artifact))
}

async fn delete_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let current = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_write(&state, &subject, &current.summary.workspace_id).await?;
    repositories::ensure_not_repository_file(&state, &artifact_id).await?;
    let title = current.summary.title;
    let workspace_id = current.summary.workspace_id;
    state
        .core
        .delete_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "artifact_deleted",
        "artifact",
        Some(&artifact_id),
        format!("Deleted evidence: {title}."),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_artifact_lifecycle(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<ArtifactLifecycle>, ApiError> {
    let artifact = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_read(&state, &subject, &artifact.summary.workspace_id).await?;
    state
        .storage
        .get_artifact_lifecycle(&artifact_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn update_artifact_lifecycle(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
    Json(request): Json<SaveArtifactLifecycleRequest>,
) -> Result<Json<ArtifactLifecycle>, ApiError> {
    let artifact = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_write(&state, &subject, &artifact.summary.workspace_id).await?;
    let status = request.status.trim().to_ascii_lowercase();
    if !matches!(
        status.as_str(),
        "active" | "needs_review" | "verified" | "outdated" | "superseded"
    ) {
        return Err(ApiError::bad_request(
            "Evidence lifecycle status is invalid.",
        ));
    }
    let review_note = request.review_note.trim().to_owned();
    if review_note.len() > 5_000 {
        return Err(ApiError::bad_request(
            "Evidence lifecycle note is too long.",
        ));
    }
    let owner_user_id = request
        .owner_user_id
        .filter(|value| !value.trim().is_empty());
    if let Some(owner_user_id) = owner_user_id.as_deref() {
        if state
            .storage
            .workspace_role_for_user(owner_user_id, &artifact.summary.workspace_id)
            .await
            .map_err(map_storage_error)?
            .is_none()
        {
            return Err(ApiError::bad_request(
                "Evidence owner must be a workspace member.",
            ));
        }
    }
    let superseded_by_artifact_id = request
        .superseded_by_artifact_id
        .filter(|value| !value.trim().is_empty());
    if status == "superseded" && superseded_by_artifact_id.is_none() {
        return Err(ApiError::bad_request(
            "Choose the replacement evidence before marking this item superseded.",
        ));
    }
    if let Some(replacement_id) = superseded_by_artifact_id.as_deref() {
        if replacement_id == artifact_id {
            return Err(ApiError::bad_request("Evidence cannot supersede itself."));
        }
        let replacement = state
            .core
            .get_artifact(replacement_id.to_owned())
            .await
            .map_err(map_core_error)?;
        if replacement.summary.workspace_id != artifact.summary.workspace_id {
            return Err(ApiError::bad_request(
                "Replacement evidence must belong to this workspace.",
            ));
        }
    }
    let lifecycle = state
        .storage
        .save_artifact_lifecycle(
            &artifact_id,
            &subject.user_id,
            SaveArtifactLifecycle {
                status: status.clone(),
                owner_user_id,
                review_note,
                superseded_by_artifact_id,
            },
        )
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &artifact.summary.workspace_id,
        &subject.user_id,
        "evidence_lifecycle_updated",
        "artifact",
        Some(&artifact_id),
        format!(
            "Updated evidence lifecycle for {} to {}.",
            artifact.summary.title, status
        ),
    )
    .await;
    Ok(Json(lifecycle))
}

async fn list_artifact_lifecycle_events(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<Vec<ArtifactLifecycleEvent>>, ApiError> {
    let artifact = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_read(&state, &subject, &artifact.summary.workspace_id).await?;
    state
        .storage
        .list_artifact_lifecycle_events(&artifact_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn index_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<IndexingJobStatus>, ApiError> {
    let artifact = state
        .core
        .get_artifact(artifact_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_write(&state, &subject, &artifact.summary.workspace_id).await?;
    let job = state
        .core
        .index_artifact(artifact_id)
        .await
        .map_err(map_core_error)?;
    state.embedding_queue.request(&artifact.summary.workspace_id);
    record_workspace_activity(
        &state,
        &artifact.summary.workspace_id,
        &subject.user_id,
        "artifact_indexed",
        "artifact",
        Some(&artifact.summary.id),
        format!("Indexed evidence: {}.", artifact.summary.title),
    )
    .await;
    Ok(Json(job))
}

async fn index_workspace(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<IndexingJobStatus>, ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let job = state
        .core
        .index_workspace(workspace_id.clone())
        .await
        .map_err(map_core_error)?;
    state.embedding_queue.request(&workspace_id);
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "workspace_indexed",
        "workspace",
        Some(&workspace_id),
        "Indexed the workspace evidence.".to_owned(),
    )
    .await;
    Ok(Json(job))
}

#[derive(Debug, Deserialize)]
struct JobsQuery {
    kind: Option<String>,
    status: Option<String>,
    limit: Option<i64>,
}

async fn list_workspace_jobs(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<JobsQuery>,
) -> Result<Json<Vec<IndexingJobStatus>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let limit = query.limit.unwrap_or(50);
    state
        .storage
        .list_workspace_jobs(
            &workspace_id,
            query.kind.as_deref(),
            query.status.as_deref(),
            limit,
        )
        .await
        .map(Json)
        .map_err(map_storage_error)
}

async fn get_job(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<Json<IndexingJobStatus>, ApiError> {
    let job = state
        .storage
        .get_job(&job_id)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(|| ApiError::not_found("The job was not found."))?;
    require_workspace_read(&state, &subject, &job.workspace_id).await?;
    Ok(Json(job))
}

async fn cancel_job(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<Json<IndexingJobStatus>, ApiError> {
    let job = state
        .storage
        .get_job(&job_id)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(|| ApiError::not_found("The job was not found."))?;
    require_workspace_write(&state, &subject, &job.workspace_id).await?;
    let updated = state
        .storage
        .request_job_cancel(&job_id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(updated))
}

/// How often an open event stream re-checks that its user may still read the
/// workspace.
#[cfg(not(test))]
const EVENT_STREAM_RECHECK: Duration = Duration::from_secs(30);
#[cfg(test)]
const EVENT_STREAM_RECHECK: Duration = Duration::from_millis(50);

enum EventStreamItem {
    Event(SseEvent),
    Close,
}

/// Server-sent-events stream of job progress and activity for a workspace.
/// Emits `job` and `activity` events, and `resync` when the client fell
/// behind and missed some (it should reload what it shows). `KeepAlive`
/// comments keep proxies from closing the connection during quiet stretches.
///
/// The stream ends when the access token expires, when the user's sessions
/// are ended, or when the user leaves the workspace; the client reconnects
/// with a fresh token.
async fn workspace_events(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let receiver = state.event_bus.subscribe(&workspace_id);
    let events = BroadcastStream::new(receiver).filter_map(|item| match item {
        Ok(event) => {
            let payload = serde_json::to_string(&event).ok()?;
            Some(EventStreamItem::Event(
                SseEvent::default().event(event.event_name()).data(payload),
            ))
        }
        Err(BroadcastStreamRecvError::Lagged(missed)) => Some(EventStreamItem::Event(
            SseEvent::default()
                .event("resync")
                .data(json!({ "type": "resync", "missed": missed }).to_string()),
        )),
    });
    let (closer, closed) = tokio::sync::mpsc::channel::<()>(1);
    tokio::spawn(watch_event_stream_access(
        state.clone(),
        subject,
        workspace_id,
        closer,
    ));
    let open = OpenEventStream::new(state.telemetry.clone());
    let stream = events
        .merge(ReceiverStream::new(closed).map(|_| EventStreamItem::Close))
        .take_while(|item| !matches!(item, EventStreamItem::Close))
        .filter_map(move |item| {
            let _counted = &open;
            match item {
                EventStreamItem::Event(event) => Some(Ok(event)),
                EventStreamItem::Close => None,
            }
        });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

/// Counts an open event stream for the System page while it lives.
struct OpenEventStream(Arc<Telemetry>);

impl OpenEventStream {
    fn new(telemetry: Arc<Telemetry>) -> Self {
        telemetry
            .open_event_streams
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self(telemetry)
    }
}

impl Drop for OpenEventStream {
    fn drop(&mut self) {
        self.0
            .open_event_streams
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Closes an event stream once its token expires or its user may no longer
/// read the workspace. Stops by itself when the client disconnects.
async fn watch_event_stream_access(
    state: AppState,
    subject: AuthenticatedSubject,
    workspace_id: String,
    closer: tokio::sync::mpsc::Sender<()>,
) {
    loop {
        let remaining = subject
            .expires_at
            .saturating_sub(jsonwebtoken::get_current_timestamp());
        tokio::select! {
            _ = closer.closed() => return,
            _ = tokio::time::sleep(EVENT_STREAM_RECHECK.min(Duration::from_secs(remaining))) => {}
        }
        let expired = jsonwebtoken::get_current_timestamp() >= subject.expires_at;
        let access = state.storage.user_access(&subject.user_id).await.ok();
        let session_valid = access.map_or(true, |access| {
            access.is_some_and(|access| access.session_version == subject.session_version)
        });
        let is_system_admin = access.flatten().is_some_and(|access| access.is_system_admin);
        let member = effective_workspace_role(&state, &subject.user_id, is_system_admin, &workspace_id)
            .await
            .map(|role| role.is_some())
            .unwrap_or(true);
        let closing = state.closing.load(std::sync::atomic::Ordering::SeqCst);
        if expired || !session_valid || !member || closing {
            let _ = closer.send(()).await;
            return;
        }
    }
}

async fn search_workspace(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SearchWorkspaceRequest>,
) -> Result<Json<Vec<SearchResult>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let result = state
        .core
        .search_workspace(SearchRequest {
            workspace_id,
            query: request.query,
            artifact_types: request.artifact_types,
            languages: request.languages,
            source_ids: request.source_ids,
            limit: request.limit,
        })
        .await
        .map_err(map_core_error)?;
    Ok(Json(result))
}

async fn get_retrieval_facets(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<RetrievalFacetsResponse>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let artifacts = state
        .core
        .list_artifacts(workspace_id)
        .await
        .map_err(map_core_error)?;
    let mut artifact_types: Vec<ArtifactType> = Vec::new();
    let mut languages: Vec<String> = Vec::new();
    let mut sources: Vec<RetrievalSourceFacet> = Vec::new();

    for artifact in artifacts
        .iter()
        .filter(|artifact| artifact.indexed_at.is_some())
    {
        if !artifact_types.contains(&artifact.artifact_type) {
            artifact_types.push(artifact.artifact_type.clone());
        }
        if let Some(language) = artifact
            .language
            .as_ref()
            .filter(|language| !language.trim().is_empty())
        {
            if !languages
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(language))
            {
                languages.push(language.clone());
            }
        }
        if !sources
            .iter()
            .any(|source: &RetrievalSourceFacet| source.id == artifact.source_id)
        {
            sources.push(RetrievalSourceFacet {
                id: artifact.source_id.clone(),
                name: artifact.source_name.clone(),
            });
        }
    }

    languages.sort_by_key(|language| language.to_lowercase());
    sources.sort_by_key(|source| source.name.to_lowercase());
    Ok(Json(RetrievalFacetsResponse {
        artifact_types,
        languages,
        sources,
    }))
}

async fn list_memory_cards(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<MemoryCardSummary>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .core
        .list_memory_cards(workspace_id)
        .await
        .map(Json)
        .map_err(map_core_error)
}

async fn search_memory_cards(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SearchMemoryCardsRequest>,
) -> Result<Json<Vec<MemoryCardSummary>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .core
        .search_memory_cards(workspace_id, request.query)
        .await
        .map(Json)
        .map_err(map_core_error)
}

async fn create_memory_card(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<CreateMemoryCardBody>,
) -> Result<(StatusCode, Json<MemoryCard>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let card = state
        .core
        .create_memory_card(CreateMemoryCardRequest {
            workspace_id: workspace_id.clone(),
            title: request.title,
            body_markdown: request.body_markdown,
            source: request.source,
            confidence: request.confidence,
            citations: request.citations,
        })
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "memory_created",
        "memory_card",
        Some(&card.id),
        format!("Saved team memory: {}.", card.title),
    )
    .await;
    Ok((StatusCode::CREATED, Json(card)))
}

async fn get_memory_card(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(card_id): Path<String>,
) -> Result<Json<MemoryCardDetail>, ApiError> {
    let card = state
        .core
        .get_memory_card(card_id)
        .await
        .map_err(map_core_error)?;
    require_workspace_read(&state, &subject, &card.card.workspace_id).await?;
    Ok(Json(card))
}

async fn update_memory_card(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(card_id): Path<String>,
    Json(request): Json<UpdateMemoryCardBody>,
) -> Result<Json<MemoryCard>, ApiError> {
    let current = state
        .core
        .get_memory_card(card_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_write(&state, &subject, &current.card.workspace_id).await?;
    let workspace_id = current.card.workspace_id;
    let card = state
        .core
        .update_memory_card(UpdateMemoryCardRequest {
            card_id,
            title: request.title,
            body_markdown: request.body_markdown,
            source: request.source,
            confidence: request.confidence,
        })
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "memory_updated",
        "memory_card",
        Some(&card.id),
        format!("Updated team memory: {}.", card.title),
    )
    .await;
    Ok(Json(card))
}

async fn delete_memory_card(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(card_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let current = state
        .core
        .get_memory_card(card_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_write(&state, &subject, &current.card.workspace_id).await?;
    let workspace_id = current.card.workspace_id;
    let title = current.card.title;
    state
        .core
        .delete_memory_card(card_id.clone())
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "memory_deleted",
        "memory_card",
        Some(&card_id),
        format!("Deleted team memory: {title}."),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

async fn export_memory_card(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(card_id): Path<String>,
) -> Result<Response, ApiError> {
    let card = state
        .core
        .get_memory_card(card_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_read(&state, &subject, &card.card.workspace_id).await?;
    let markdown = state
        .core
        .export_memory_card(card_id)
        .await
        .map_err(map_core_error)?;
    Ok((
        [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
        markdown,
    )
        .into_response())
}

async fn require_workspace_read(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: &str,
) -> Result<WorkspaceRole, ApiError> {
    effective_workspace_role(state, &subject.user_id, subject.is_system_admin, workspace_id)
        .await?
        .ok_or_else(ApiError::forbidden)
}

/// The role a user acts with in a workspace: their membership, raised to
/// administrator for a system administrator (an owner stays owner), who also
/// acts as administrator where they are not a member.
async fn effective_workspace_role(
    state: &AppState,
    user_id: &str,
    is_system_admin: bool,
    workspace_id: &str,
) -> Result<Option<WorkspaceRole>, ApiError> {
    let membership = state
        .storage
        .workspace_role_for_user(user_id, workspace_id)
        .await
        .map_err(ApiError::internal)?;
    if !is_system_admin {
        return Ok(membership);
    }
    Ok(match membership {
        Some(WorkspaceRole::Owner) => Some(WorkspaceRole::Owner),
        Some(_) => Some(WorkspaceRole::Admin),
        None => state
            .storage
            .workspace_exists(workspace_id)
            .await
            .map_err(ApiError::internal)?
            .then_some(WorkspaceRole::Admin),
    })
}

async fn require_organization_read(
    state: &AppState,
    subject: &AuthenticatedSubject,
    organization_id: &str,
) -> Result<OrganizationRole, ApiError> {
    let membership = state
        .storage
        .organization_role_for_user(&subject.user_id, organization_id)
        .await
        .map_err(ApiError::internal)?;
    if !subject.is_system_admin {
        return membership.ok_or_else(ApiError::forbidden);
    }
    match membership {
        Some(OrganizationRole::Owner) => Ok(OrganizationRole::Owner),
        Some(_) => Ok(OrganizationRole::Admin),
        None if state
            .storage
            .organization_exists(organization_id)
            .await
            .map_err(ApiError::internal)? =>
        {
            Ok(OrganizationRole::Admin)
        }
        None => Err(ApiError::forbidden()),
    }
}

async fn require_organization_admin(
    state: &AppState,
    subject: &AuthenticatedSubject,
    organization_id: &str,
) -> Result<OrganizationRole, ApiError> {
    let role = require_organization_read(state, subject, organization_id).await?;
    if !matches!(role, OrganizationRole::Owner | OrganizationRole::Admin) {
        return Err(ApiError::forbidden());
    }
    Ok(role)
}

async fn require_organization_owner(
    state: &AppState,
    subject: &AuthenticatedSubject,
    organization_id: &str,
) -> Result<OrganizationRole, ApiError> {
    let role = require_organization_read(state, subject, organization_id).await?;
    if !matches!(role, OrganizationRole::Owner) {
        return Err(ApiError::forbidden());
    }
    Ok(role)
}

async fn require_workspace_write(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: &str,
) -> Result<WorkspaceRole, ApiError> {
    let role = require_workspace_read(state, subject, workspace_id).await?;
    if matches!(role, WorkspaceRole::Viewer) {
        return Err(ApiError::forbidden());
    }
    Ok(role)
}

async fn require_workspace_admin(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: &str,
) -> Result<WorkspaceRole, ApiError> {
    let role = require_workspace_read(state, subject, workspace_id).await?;
    if !matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin) {
        return Err(ApiError::forbidden());
    }
    Ok(role)
}

async fn require_workspace_owner(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: &str,
) -> Result<WorkspaceRole, ApiError> {
    let role = require_workspace_read(state, subject, workspace_id).await?;
    if !matches!(role, WorkspaceRole::Owner) {
        return Err(ApiError::forbidden());
    }
    Ok(role)
}

/// What a role may do; `may_use_ai` comes from the workspace's AI policy.
fn capabilities_for_role(role: WorkspaceRole, may_use_ai: bool) -> WorkspaceCapabilities {
    let can_write_content = !matches!(&role, WorkspaceRole::Viewer);
    let can_manage_members = matches!(&role, WorkspaceRole::Owner | WorkspaceRole::Admin);
    WorkspaceCapabilities {
        can_read: true,
        can_delete_content: can_write_content,
        can_write_content,
        can_assign_admin: matches!(&role, WorkspaceRole::Owner),
        can_manage_workspace: matches!(&role, WorkspaceRole::Owner),
        can_generate_ai_overview: may_use_ai,
        can_use_ai: may_use_ai,
        can_create_tasks: can_write_content,
        can_comment: can_write_content,
        can_moderate_comments: matches!(&role, WorkspaceRole::Owner | WorkspaceRole::Admin),
        can_inspect_index: can_manage_members,
        role,
        can_manage_members,
    }
}

fn workspace_role_label(role: &WorkspaceRole) -> &'static str {
    match role {
        WorkspaceRole::Owner => "owner",
        WorkspaceRole::Admin => "admin",
        WorkspaceRole::Member => "member",
        WorkspaceRole::Viewer => "viewer",
    }
}

impl FromRequestParts<AppState> for AuthenticatedSubject {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header_value = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(ApiError::unauthorized)?;
        let token = header_value
            .strip_prefix("Bearer ")
            .ok_or_else(ApiError::unauthorized)?;
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[JWT_ISSUER]);
        let claims = decode::<JwtClaims>(
            token,
            &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
            &validation,
        )
        .map_err(|_| ApiError::unauthorized())?
        .claims;
        // A token outlives neither its user nor the end of its sessions.
        let access = state
            .storage
            .user_access(&claims.sub)
            .await
            .map_err(ApiError::internal)?
            .filter(|access| access.session_version == claims.ver)
            .ok_or_else(ApiError::unauthorized)?;
        Ok(Self {
            user_id: claims.sub,
            expires_at: claims.exp as u64,
            session_version: claims.ver,
            is_system_admin: access.is_system_admin,
            is_app_admin: access.is_app_admin,
        })
    }
}

fn validate_registration(request: &RegisterRequest) -> Result<(), ApiError> {
    if request.display_name.trim().is_empty() || request.display_name.trim().len() > 120 {
        return Err(ApiError::bad_request(
            "Display name must be between 1 and 120 characters.",
        ));
    }
    validate_password(&request.password)
}

fn validate_password(password: &str) -> Result<(), ApiError> {
    if password.len() < 12 || password.len() > 1024 {
        return Err(ApiError::bad_request(
            "Password must be between 12 and 1024 characters.",
        ));
    }
    Ok(())
}

/// Argon2 is deliberately slow (tens of milliseconds), so it runs on the
/// blocking pool instead of holding up the async workers.
async fn hash_password(password: &str) -> Result<String, ApiError> {
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
    })
    .await
    .map_err(ApiError::internal)?
    .map_err(ApiError::internal)
}

async fn verify_password(password: &str, stored_hash: &str) -> bool {
    let password = password.to_owned();
    let stored_hash = stored_hash.to_owned();
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&stored_hash)
            .ok()
            .and_then(|hash| {
                Argon2::default()
                    .verify_password(password.as_bytes(), &hash)
                    .ok()
            })
            .is_some()
    })
    .await
    .unwrap_or(false)
}

/// A valid Argon2 hash of a random password nobody knows, checked when an
/// email has no account so that a miss costs as much time as a wrong password.
fn dummy_password_hash() -> &'static str {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| {
        let salt = SaltString::generate(&mut OsRng);
        let secret = random_secret();
        Argon2::default()
            .hash_password(secret.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .unwrap_or_default()
    })
}

fn random_secret() -> String {
    let mut bytes = [0_u8; 32];
    rand_core::RngCore::fill_bytes(&mut OsRng, &mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

async fn issue_session(state: &AppState, user: SharedUser) -> Result<TokenResponse, ApiError> {
    let version = state
        .storage
        .user_session_version(&user.id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let refresh_token = state
        .storage
        .create_refresh_token(&user.id, state.settings().refresh_token_ttl_days)
        .await
        .map_err(ApiError::internal)?;
    issue_token(
        &state.jwt_secret,
        user,
        refresh_token,
        version,
        state.settings().access_token_ttl_minutes,
    )
}

fn issue_token(
    secret: &str,
    user: SharedUser,
    refresh_token: String,
    session_version: i64,
    ttl_minutes: i64,
) -> Result<TokenResponse, ApiError> {
    let now = Utc::now();
    let expires_at = now + ChronoDuration::minutes(ttl_minutes);
    let claims = JwtClaims {
        sub: user.id.clone(),
        email: user.email.clone().unwrap_or_default(),
        iss: JWT_ISSUER.to_owned(),
        iat: now.timestamp() as usize,
        exp: expires_at.timestamp() as usize,
        ver: session_version,
    };
    let access_token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(ApiError::internal)?;
    Ok(TokenResponse {
        access_token,
        refresh_token,
        token_type: "Bearer",
        expires_in: Duration::from_secs(ttl_minutes.max(0) as u64 * 60).as_secs(),
        user,
    })
}

fn invalid_credentials() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "invalid_credentials",
        "Email or password is incorrect.",
    )
}

/// Whether an error says that what was asked for does not exist.
fn is_not_found(message: &str) -> bool {
    message.contains("was not found")
        || message.contains("were not found")
        || message.contains("no rows returned")
}

fn map_storage_error(error: anyhow::Error) -> ApiError {
    let message = error.to_string();
    if message.contains("UNIQUE constraint failed") {
        ApiError::conflict("This record already exists.")
    } else if message.contains("membership was not found") {
        ApiError::bad_request(message)
    } else if is_not_found(&message) {
        ApiError::not_found(message)
    } else if message.contains("not a member")
        || message.contains("must be between")
        || message.contains("valid email")
        || message.contains("No RepoMemo account")
        || message.contains("owner role")
        || message.contains("owner cannot be removed")
        || message.contains("membership was not found")
    {
        ApiError::bad_request(message)
    } else {
        ApiError::internal(error)
    }
}

fn map_core_error(error: anyhow::Error) -> ApiError {
    let message = error.to_string();
    if is_not_found(&message) {
        ApiError::not_found(message)
    } else if message.contains("is required")
        || message.contains("cannot be empty")
        || message.contains("must be between")
        || message.contains("Unsupported AI provider")
        || message.contains("before enabling cloud AI")
        || message.contains("cloud content acknowledgement")
    {
        ApiError::bad_request(message)
    } else {
        ApiError::internal(error)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn page_query_slices_and_reports_total() {
        let page = super::PageQuery { limit: Some(2), offset: Some(3) };
        let (headers, items) = page.apply((0..10).collect::<Vec<_>>());
        assert_eq!(items, vec![3, 4]);
        assert_eq!(headers.get("x-total-count").unwrap(), "10");
        let (headers, items) = super::PageQuery::default().apply((0..10).collect::<Vec<_>>());
        assert_eq!(items.len(), 10);
        assert!(headers.get("x-total-count").is_none());
    }

    use super::{router, ServerConfig};
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;

    #[tokio::test]
    async fn refresh_tokens_rotate_and_detect_reuse() {
        let data_dir =
            std::env::temp_dir().join(format!("repomemo-server-test-{}", uuid::Uuid::new_v4()));
        let app = router(ServerConfig::for_test(data_dir.clone()))
            .await
            .unwrap();
        let post = |uri: &str, body: String| {
            Request::builder().method("POST").uri(uri.to_owned()).header("content-type", "application/json").body(Body::from(body)).unwrap()
        };
        let response = app.clone().oneshot(post("/v1/auth/register", r#"{"email":"refresh@example.com","display_name":"Refresh","password":"not-a-real-password"}"#.to_owned())).await.unwrap();
        let first: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
        let first_refresh = first["refresh_token"].as_str().unwrap().to_owned();

        let response = app.clone().oneshot(post("/v1/auth/refresh", format!(r#"{{"refresh_token":"{first_refresh}"}}"#))).await.unwrap();
        assert_eq!(response.status(), 200);
        let second: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
        let second_refresh = second["refresh_token"].as_str().unwrap().to_owned();
        assert_ne!(first_refresh, second_refresh);

        // Replaying the rotated token fails and revokes the whole family.
        let response = app.clone().oneshot(post("/v1/auth/refresh", format!(r#"{{"refresh_token":"{first_refresh}"}}"#))).await.unwrap();
        assert_eq!(response.status(), 401);
        let response = app.clone().oneshot(post("/v1/auth/refresh", format!(r#"{{"refresh_token":"{second_refresh}"}}"#))).await.unwrap();
        assert_eq!(response.status(), 401);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn registers_and_protects_shared_workspace_routes() {
        let data_dir =
            std::env::temp_dir().join(format!("repomemo-server-test-{}", uuid::Uuid::new_v4()));
        let app = router(ServerConfig::for_test(data_dir.clone()))
            .await
            .unwrap();
        let register = Request::builder().method("POST").uri("/v1/auth/register").header("content-type", "application/json")
            .body(Body::from(r#"{"email":"owner@example.com","display_name":"Owner","password":"not-a-real-password"}"#)).unwrap();
        let response = app.clone().oneshot(register).await.unwrap();
        assert_eq!(response.status(), 201);
        let registration: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let _token = registration["access_token"].as_str().unwrap();
        let protected = Request::builder()
            .uri("/v1/workspaces")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(protected).await.unwrap();
        assert_eq!(response.status(), 401);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn protects_and_serves_shared_evidence_flow() {
        let data_dir =
            std::env::temp_dir().join(format!("repomemo-server-flow-{}", uuid::Uuid::new_v4()));
        let app = router(ServerConfig::for_test(data_dir.clone()))
            .await
            .unwrap();
        let response = app.clone().oneshot(Request::builder().method("POST").uri("/v1/auth/register").header("content-type", "application/json")
            .body(Body::from(r#"{"email":"flow@example.com","display_name":"Flow Owner","password":"not-a-real-password"}"#)).unwrap()).await.unwrap();
        let registration: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let authorization = format!("Bearer {}", registration["access_token"].as_str().unwrap());

        let profile = app
            .clone()
            .oneshot(auth_request("GET", "/v1/profile", &authorization))
            .await
            .unwrap();
        assert_eq!(profile.status(), 200);
        let profile: Value =
            serde_json::from_slice(&to_bytes(profile.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(profile["user"]["display_name"], "Flow Owner");
        assert!(profile["last_connected_at"].is_string());
        assert_eq!(profile["activity_by_day"].as_array().unwrap().len(), 365);

        let renamed_profile = app
            .clone()
            .oneshot(json_request(
                "PUT",
                "/v1/profile",
                &authorization,
                json!({"display_name":"Flow Owner Updated"}),
            ))
            .await
            .unwrap();
        assert_eq!(renamed_profile.status(), 200);

        let changed_password = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/profile/password",
                &authorization,
                json!({"current_password":"not-a-real-password","new_password":"changed-password-123"}),
            ))
            .await
            .unwrap();
        assert_eq!(changed_password.status(), 200);
        let changed_password: Value =
            serde_json::from_slice(&to_bytes(changed_password.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        // The old token is ended with every other session; this one continues
        // with the fresh session returned by the change.
        let stale_profile = app
            .clone()
            .oneshot(auth_request("GET", "/v1/profile", &authorization))
            .await
            .unwrap();
        assert_eq!(stale_profile.status(), 401);
        let authorization = format!("Bearer {}", changed_password["access_token"].as_str().unwrap());

        let organization = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/organizations",
                &authorization,
                json!({"name":"Flow Team"}),
            ))
            .await
            .unwrap();
        assert_eq!(organization.status(), 201);
        let organization: Value = serde_json::from_slice(
            &to_bytes(organization.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let workspace = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/workspaces",
                &authorization,
                json!({"organization_id": organization["id"], "name":"Flow Workspace"}),
            ))
            .await
            .unwrap();
        assert_eq!(workspace.status(), 201);
        let workspace: Value =
            serde_json::from_slice(&to_bytes(workspace.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let workspace_id = workspace["workspace"]["id"].as_str().unwrap();

        let collaborator = app.clone().oneshot(Request::builder().method("POST").uri("/v1/auth/register").header("content-type", "application/json")
            .body(Body::from(r#"{"email":"collaborator@example.com","display_name":"Collaborator","password":"not-a-real-password"}"#)).unwrap()).await.unwrap();
        assert_eq!(collaborator.status(), 201);
        let collaborator: Value = serde_json::from_slice(
            &to_bytes(collaborator.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let collaborator_authorization =
            format!("Bearer {}", collaborator["access_token"].as_str().unwrap());
        let member = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/workspaces/{workspace_id}/members"),
                &authorization,
                json!({"email":"collaborator@example.com","role":"member"}),
            ))
            .await
            .unwrap();
        assert_eq!(member.status(), 200);
        let member: Value =
            serde_json::from_slice(&to_bytes(member.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(member["role"], "member");
        let members = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/members"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(members.status(), 200);
        let members: Value =
            serde_json::from_slice(&to_bytes(members.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(members.as_array().unwrap().len(), 2);
        for endpoint in [
            format!("/v1/workspaces/{workspace_id}/members"),
            format!("/v1/workspaces/{workspace_id}/activity"),
            format!("/v1/workspaces/{workspace_id}/activity/calendar"),
        ] {
            let response = app
                .clone()
                .oneshot(auth_request("GET", &endpoint, &collaborator_authorization))
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                403,
                "{endpoint} should require workspace admin access"
            );
        }
        let collaborator_overview = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/overview"),
                &collaborator_authorization,
            ))
            .await
            .unwrap();
        assert_eq!(collaborator_overview.status(), 200);

        let artifact = app.clone().oneshot(json_request("POST", &format!("/v1/workspaces/{workspace_id}/artifacts/text"), &authorization, json!({"title":"Shared fact", "content":"The API owns shared data.", "language":"Markdown"}))).await.unwrap();
        assert_eq!(artifact.status(), 201);
        let artifact: Value =
            serde_json::from_slice(&to_bytes(artifact.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let artifact_id = artifact["id"].as_str().unwrap();

        let lifecycle = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/artifacts/{artifact_id}/lifecycle"),
                &authorization,
                json!({
                    "status":"needs_review",
                    "owner_user_id":member["user"]["id"],
                    "review_note":"Needs a second reviewer before this decision is relied on.",
                    "superseded_by_artifact_id":null
                }),
            ))
            .await
            .unwrap();
        assert_eq!(lifecycle.status(), 200);
        let lifecycle: Value =
            serde_json::from_slice(&to_bytes(lifecycle.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(lifecycle["status"], "needs_review");
        assert_eq!(lifecycle["owner"]["id"], member["user"]["id"]);
        let lifecycle_history = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/artifacts/{artifact_id}/lifecycle/history"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(lifecycle_history.status(), 200);
        let lifecycle_history: Value = serde_json::from_slice(
            &to_bytes(lifecycle_history.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(lifecycle_history.as_array().unwrap().len(), 1);

        let saved_search = app.clone().oneshot(json_request("POST", &format!("/v1/workspaces/{workspace_id}/saved-searches"), &authorization, json!({"name":"Architecture review","query":"shared","artifact_types":["markdown_doc"],"languages":[],"source_ids":[],"result_limit":20}))).await.unwrap();
        assert_eq!(saved_search.status(), 201);
        let saved_search: Value = serde_json::from_slice(
            &to_bytes(saved_search.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let saved_search_id = saved_search["id"].as_str().unwrap();
        let saved_searches = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/saved-searches"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(saved_searches.status(), 200);

        let task = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/tasks"),
                &authorization,
                json!({
                    "title":"Review shared API decision",
                    "description":"Confirm the evidence and capture follow-up work.",
                    "status":"open",
                    "priority":"high",
                    "assignee_user_id":member["user"]["id"],
                    "artifact_id":artifact_id,
                    "due_at":null
                }),
            ))
            .await
            .unwrap();
        assert_eq!(task.status(), 201);
        let task: Value =
            serde_json::from_slice(&to_bytes(task.into_body(), usize::MAX).await.unwrap()).unwrap();
        let task_id = task["id"].as_str().unwrap();
        let checklist_item = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/tasks/{task_id}/checklist"),
                &authorization,
                json!({"body":"Confirm the shared evidence"}),
            ))
            .await
            .unwrap();
        assert_eq!(checklist_item.status(), 201);
        let checklist_item: Value = serde_json::from_slice(
            &to_bytes(checklist_item.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let checklist_item_id = checklist_item["id"].as_str().unwrap();
        let completed_item = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/task-checklist/{checklist_item_id}"),
                &authorization,
                json!({"completed":true}),
            ))
            .await
            .unwrap();
        assert_eq!(completed_item.status(), 200);
        let delete_search = app
            .clone()
            .oneshot(auth_request(
                "DELETE",
                &format!("/v1/saved-searches/{saved_search_id}"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(delete_search.status(), 204);
        assert_eq!(task["assignee"]["display_name"], "Collaborator");
        let task_notifications = app
            .clone()
            .oneshot(auth_request(
                "GET",
                "/v1/notifications",
                &collaborator_authorization,
            ))
            .await
            .unwrap();
        assert_eq!(task_notifications.status(), 200);
        let task_notifications: Value = serde_json::from_slice(
            &to_bytes(task_notifications.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(task_notifications[0]["notification_type"], "task_assigned");
        let assigned_tasks = app
            .clone()
            .oneshot(auth_request(
                "GET",
                "/v1/profile/tasks",
                &collaborator_authorization,
            ))
            .await
            .unwrap();
        assert_eq!(assigned_tasks.status(), 200);
        let assigned_tasks: Value = serde_json::from_slice(
            &to_bytes(assigned_tasks.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(assigned_tasks.as_array().unwrap().len(), 1);
        let updated_task = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/tasks/{task_id}"),
                &collaborator_authorization,
                json!({
                    "title":"Review shared API decision",
                    "description":"Confirmed and documented.",
                    "status":"done",
                    "priority":"high",
                    "assignee_user_id":member["user"]["id"],
                    "artifact_id":artifact_id,
                    "due_at":null
                }),
            ))
            .await
            .unwrap();
        assert_eq!(updated_task.status(), 200);
        let updated_task: Value = serde_json::from_slice(
            &to_bytes(updated_task.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(updated_task["status"], "done");
        assert!(updated_task["completed_at"].is_string());
        let tasks = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/tasks"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(tasks.status(), 200);
        let tasks: Value =
            serde_json::from_slice(&to_bytes(tasks.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(tasks.as_array().unwrap().len(), 1);

        let comment = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/artifacts/{artifact_id}/comments"),
                &collaborator_authorization,
                json!({"body":"@flow@example.com This confirms the server-authoritative decision."}),
            ))
            .await
            .unwrap();
        assert_eq!(comment.status(), 201);
        let comment: Value =
            serde_json::from_slice(&to_bytes(comment.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let comment_id = comment["id"].as_str().unwrap();
        let mention_notifications = app
            .clone()
            .oneshot(auth_request("GET", "/v1/notifications", &authorization))
            .await
            .unwrap();
        assert_eq!(mention_notifications.status(), 200);
        let mention_notifications: Value = serde_json::from_slice(
            &to_bytes(mention_notifications.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            mention_notifications[0]["notification_type"],
            "evidence_mention"
        );
        let mention_notification_id = mention_notifications[0]["id"].as_str().unwrap();
        let marked_notification = app
            .clone()
            .oneshot(auth_request(
                "POST",
                &format!("/v1/notifications/{mention_notification_id}/read"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(marked_notification.status(), 200);
        let updated_comment = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/comments/{comment_id}"),
                &collaborator_authorization,
                json!({"body":"Confirmed: the API remains authoritative for shared data."}),
            ))
            .await
            .unwrap();
        assert_eq!(updated_comment.status(), 200);
        let comments = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/artifacts/{artifact_id}/comments"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(comments.status(), 200);
        let comments: Value =
            serde_json::from_slice(&to_bytes(comments.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(comments.as_array().unwrap().len(), 1);

        let uploaded = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                    .header("authorization", &authorization)
                    .header("content-type", "text/x-rust")
                    .header("x-repomemo-filename", "upload.rs")
                    .body(Body::from("fn shared_upload() {}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(uploaded.status(), 201);

        let filtered_artifacts = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/artifacts/query"),
                &authorization,
                json!({
                    "query":"shared fact",
                    "artifact_types":["markdown_doc"],
                    "languages":["markdown"],
                    "source_ids":[]
                }),
            ))
            .await
            .unwrap();
        assert_eq!(filtered_artifacts.status(), 200);
        let filtered_artifacts: Value = serde_json::from_slice(
            &to_bytes(filtered_artifacts.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(filtered_artifacts.as_array().unwrap().len(), 1);
        assert_eq!(filtered_artifacts[0]["id"], artifact_id);

        // Notes and uploads are indexed in the background right after storing.
        wait_for_indexed_artifacts(&app, &authorization, workspace_id, 2).await;
        let indexed_only = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/artifacts/query"),
                &authorization,
                json!({"query":"", "artifact_types":[], "languages":[], "source_ids":[], "indexed":true}),
            ))
            .await
            .unwrap();
        let indexed_only: Value = serde_json::from_slice(
            &to_bytes(indexed_only.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        assert_eq!(indexed_only.as_array().unwrap().len(), 2);
        let metrics = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/metrics"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(metrics.status(), 200);
        let metrics: Value =
            serde_json::from_slice(&to_bytes(metrics.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(metrics["artifact_count"], 2);
        assert_eq!(metrics["indexed_artifact_count"], 2);
        assert_eq!(metrics["pending_artifact_count"], 0);
        assert_eq!(metrics["member_count"], 2);
        assert_eq!(metrics["completed_task_count"], 1);
        assert_eq!(metrics["comment_count"], 1);
        assert!(metrics["total_artifact_bytes"].as_i64().unwrap() > 0);
        assert!(metrics["indexed_artifact_bytes"].as_i64().unwrap() > 0);
        assert_eq!(metrics["activity_by_day"].as_array().unwrap().len(), 14);
        assert!(!metrics["artifact_bytes_by_type"]
            .as_array()
            .unwrap()
            .is_empty());
        let calendar = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/activity/calendar"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(calendar.status(), 200);
        let calendar: Value =
            serde_json::from_slice(&to_bytes(calendar.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(calendar["activity_by_day"].as_array().unwrap().len(), 365);
        assert!(calendar["total_activity_count"].as_i64().unwrap() > 0);
        let search = app.clone().oneshot(json_request("POST", &format!("/v1/workspaces/{workspace_id}/search"), &authorization, json!({"query":"shared data", "artifact_types": [], "languages": [], "source_ids": [], "limit": 20}))).await.unwrap();
        assert_eq!(search.status(), 200);
        let search: Value =
            serde_json::from_slice(&to_bytes(search.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(search.as_array().unwrap().len(), 1);
        assert_eq!(search[0]["artifact_id"], artifact_id);
        let retrieval_facets = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/retrieval-facets"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(retrieval_facets.status(), 200);
        let retrieval_facets: Value = serde_json::from_slice(
            &to_bytes(retrieval_facets.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        // Both the pasted note and the uploaded Rust file were indexed automatically.
        assert_eq!(
            retrieval_facets["artifact_types"],
            json!(["code_file", "markdown_doc"])
        );
        assert_eq!(retrieval_facets["languages"], json!(["Markdown", "Rust"]));
        let memory = app.clone().oneshot(json_request("POST", &format!("/v1/workspaces/{workspace_id}/memory-cards"), &authorization, json!({"title":"Rule", "body_markdown":"Keep shared data on the API.", "source":"Flow", "confidence": null, "citations": []}))).await.unwrap();
        assert_eq!(memory.status(), 201);
        let memory: Value =
            serde_json::from_slice(&to_bytes(memory.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let memory_id = memory["id"].as_str().unwrap();
        let memories = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/memory-cards"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(memories.status(), 200);
        let memories: Value =
            serde_json::from_slice(&to_bytes(memories.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(memories.as_array().unwrap().len(), 1);
        let matching_memories = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/memory-cards/search"),
                &authorization,
                json!({"query":"shared data"}),
            ))
            .await
            .unwrap();
        assert_eq!(matching_memories.status(), 200);
        let matching_memories: Value = serde_json::from_slice(
            &to_bytes(matching_memories.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(matching_memories.as_array().unwrap().len(), 1);
        let memory_detail = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/memory-cards/{memory_id}"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(memory_detail.status(), 200);
        let exported = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/memory-cards/{memory_id}/export"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(exported.status(), 200);
        let exported = String::from_utf8(
            to_bytes(exported.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(exported.contains("Keep shared data on the API."));
        let capabilities = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/capabilities"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(capabilities.status(), 200);
        let capabilities: Value = serde_json::from_slice(
            &to_bytes(capabilities.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(capabilities["role"], "owner");
        assert_eq!(capabilities["can_manage_workspace"], true);
        let ai_overview = app
            .clone()
            .oneshot(auth_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/ai-overview"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(ai_overview.status(), 200);
        let ai_overview: Value =
            serde_json::from_slice(&to_bytes(ai_overview.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(ai_overview["provider_configured"], false);
        let ask_without_provider = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/ask"),
                &authorization,
                json!({"question":"What is shared?"}),
            ))
            .await
            .unwrap();
        assert_eq!(ask_without_provider.status(), 400);
        let renamed_artifact = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/artifacts/{artifact_id}"),
                &authorization,
                json!({"title":"Updated shared fact"}),
            ))
            .await
            .unwrap();
        assert_eq!(renamed_artifact.status(), 200);
        let updated_memory = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/memory-cards/{memory_id}"),
                &authorization,
                json!({"title":"Updated rule", "body_markdown":"Keep shared data on the API.", "source":"Flow", "confidence":null}),
            ))
            .await
            .unwrap();
        assert_eq!(updated_memory.status(), 200);
        let overview = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/overview"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(overview.status(), 200);
        let activity = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/activity"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(activity.status(), 200);
        let activity: Value =
            serde_json::from_slice(&to_bytes(activity.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let activity = activity.as_array().unwrap();
        assert!(activity
            .iter()
            .any(|event| event["action"] == "workspace_created"));
        assert!(activity
            .iter()
            .any(|event| event["action"] == "evidence_stored"));
        assert!(activity
            .iter()
            .any(|event| event["action"] == "memory_created"));
        let collaborator_id = collaborator["user"]["id"].as_str().unwrap();
        let removed = app
            .clone()
            .oneshot(auth_request(
                "DELETE",
                &format!("/v1/workspaces/{workspace_id}/members/{collaborator_id}"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(removed.status(), 204);
        let collaborator_overview = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/overview"),
                &collaborator_authorization,
            ))
            .await
            .unwrap();
        assert_eq!(collaborator_overview.status(), 403);
        let renamed_workspace = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/workspaces/{workspace_id}"),
                &authorization,
                json!({"name":"Renamed Flow Workspace"}),
            ))
            .await
            .unwrap();
        assert_eq!(renamed_workspace.status(), 200);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn organization_members_inherit_workspace_access_and_roles() {
        let data_dir = std::env::temp_dir().join(format!(
            "repomemo-organization-test-{}",
            uuid::Uuid::new_v4()
        ));
        let app = router(ServerConfig::for_test(data_dir.clone()))
            .await
            .unwrap();
        let owner = app.clone().oneshot(Request::builder().method("POST").uri("/v1/auth/register").header("content-type", "application/json").body(Body::from(r#"{"email":"org-owner@example.com","display_name":"Org Owner","password":"not-a-real-password"}"#)).unwrap()).await.unwrap();
        let owner: Value =
            serde_json::from_slice(&to_bytes(owner.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let owner_authorization = format!("Bearer {}", owner["access_token"].as_str().unwrap());
        let member = app.clone().oneshot(Request::builder().method("POST").uri("/v1/auth/register").header("content-type", "application/json").body(Body::from(r#"{"email":"org-member@example.com","display_name":"Org Member","password":"not-a-real-password"}"#)).unwrap()).await.unwrap();
        let member: Value =
            serde_json::from_slice(&to_bytes(member.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let member_authorization = format!("Bearer {}", member["access_token"].as_str().unwrap());

        let organization = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/organizations",
                &owner_authorization,
                json!({"name":"Platform"}),
            ))
            .await
            .unwrap();
        let organization: Value = serde_json::from_slice(
            &to_bytes(organization.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let organization_id = organization["id"].as_str().unwrap();
        assert_eq!(organization["role"], "owner");
        for workspace_name in ["Core", "Docs"] {
            let workspace = app
                .clone()
                .oneshot(json_request(
                    "POST",
                    "/v1/workspaces",
                    &owner_authorization,
                    json!({"organization_id": organization_id, "name": workspace_name}),
                ))
                .await
                .unwrap();
            assert_eq!(workspace.status(), 201);
        }

        let added = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/organizations/{organization_id}/members"),
                &owner_authorization,
                json!({"email":"org-member@example.com","role":"admin"}),
            ))
            .await
            .unwrap();
        assert_eq!(added.status(), 200);
        let member_workspaces = app
            .clone()
            .oneshot(auth_request("GET", "/v1/workspaces", &member_authorization))
            .await
            .unwrap();
        assert_eq!(member_workspaces.status(), 200);
        let member_workspaces: Value = serde_json::from_slice(
            &to_bytes(member_workspaces.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(member_workspaces.as_array().unwrap().len(), 2);
        assert!(member_workspaces
            .as_array()
            .unwrap()
            .iter()
            .all(|workspace| workspace["role"] == "admin"));

        let renamed = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/organizations/{organization_id}"),
                &owner_authorization,
                json!({"name":"Platform Engineering"}),
            ))
            .await
            .unwrap();
        assert_eq!(renamed.status(), 200);
        let member_id = member["user"]["id"].as_str().unwrap();
        let removed = app
            .clone()
            .oneshot(auth_request(
                "DELETE",
                &format!("/v1/organizations/{organization_id}/members/{member_id}"),
                &owner_authorization,
            ))
            .await
            .unwrap();
        assert_eq!(removed.status(), 204);
        let member_workspaces = app
            .clone()
            .oneshot(auth_request("GET", "/v1/workspaces", &member_authorization))
            .await
            .unwrap();
        let member_workspaces: Value = serde_json::from_slice(
            &to_bytes(member_workspaces.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(member_workspaces.as_array().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn indexing_publishes_jobs_that_can_be_listed_and_cancelled() {
        let data_dir =
            std::env::temp_dir().join(format!("repomemo-server-jobs-{}", uuid::Uuid::new_v4()));
        let app = router(ServerConfig::for_test(data_dir.clone()))
            .await
            .unwrap();

        let register = Request::builder()
            .method("POST")
            .uri("/v1/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"email":"jobs@example.com","display_name":"Jobs Owner","password":"not-a-real-password"}"#,
            ))
            .unwrap();
        let registration: Value = serde_json::from_slice(
            &to_bytes(
                app.clone().oneshot(register).await.unwrap().into_body(),
                usize::MAX,
            )
            .await
            .unwrap(),
        )
        .unwrap();
        let authorization = format!("Bearer {}", registration["access_token"].as_str().unwrap());

        let organization = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/organizations",
                &authorization,
                json!({"name":"Jobs Org"}),
            ))
            .await
            .unwrap();
        let organization: Value = serde_json::from_slice(
            &to_bytes(organization.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        let workspace = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/workspaces",
                &authorization,
                json!({"organization_id": organization["id"], "name":"Jobs Workspace"}),
            ))
            .await
            .unwrap();
        let workspace: Value = serde_json::from_slice(
            &to_bytes(workspace.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();

        let empty_jobs = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/jobs"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(empty_jobs.status(), 200);
        let empty_jobs: Value = serde_json::from_slice(
            &to_bytes(empty_jobs.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        assert!(empty_jobs.as_array().unwrap().is_empty());

        let artifact = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
                &authorization,
                json!({"title":"Job source","content":"Line one\nLine two\nLine three","language":"Markdown"}),
            ))
            .await
            .unwrap();
        assert_eq!(artifact.status(), 201);
        let artifact: Value =
            serde_json::from_slice(&to_bytes(artifact.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let artifact_id = artifact["id"].as_str().unwrap().to_owned();

        let indexed = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/artifacts/{artifact_id}/index"),
                &authorization,
                json!({}),
            ))
            .await
            .unwrap();
        assert_eq!(indexed.status(), 200);
        let indexed_job: Value =
            serde_json::from_slice(&to_bytes(indexed.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let job_id = indexed_job["id"].as_str().unwrap().to_owned();
        assert_eq!(indexed_job["kind"], "indexing");
        assert_eq!(indexed_job["status"], "completed");
        assert_eq!(indexed_job["cancel_requested"], false);

        let listed = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/jobs?kind=indexing"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(listed.status(), 200);
        let listed: Value =
            serde_json::from_slice(&to_bytes(listed.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let jobs = listed.as_array().unwrap();
        assert!(!jobs.is_empty(), "indexing should have produced a job");
        assert!(jobs.iter().any(|job| job["id"] == job_id));
        for job in jobs {
            assert_eq!(job["kind"], "indexing");
        }

        let single = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/jobs/{job_id}"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(single.status(), 200);
        let single: Value =
            serde_json::from_slice(&to_bytes(single.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(single["id"], job_id);

        // Cancelling a job that already finished is a no-op that returns the
        // current state; only running/pending jobs flip the cancel flag.
        let cancelled = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/jobs/{job_id}/cancel"),
                &authorization,
                json!({}),
            ))
            .await
            .unwrap();
        assert_eq!(cancelled.status(), 200);
        let cancelled: Value = serde_json::from_slice(
            &to_bytes(cancelled.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        assert_eq!(cancelled["status"], "completed");
        assert_eq!(cancelled["cancel_requested"], false);

        let missing = app
            .clone()
            .oneshot(auth_request(
                "GET",
                "/v1/jobs/does-not-exist",
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(missing.status(), 404);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    /// Polls the artifact list until `expected` artifacts report an index
    /// timestamp, since indexing runs on a background queue.
    #[tokio::test]
    async fn workspace_repository_links_are_checked_then_synced_in_the_background() {
        let base = std::env::temp_dir().join(format!("repomemo-server-repo-{}", uuid::Uuid::new_v4()));
        let repo_dir = base.join("repos").join("payments");
        let data_dir = base.join("data");
        std::fs::create_dir_all(repo_dir.join("src")).unwrap();
        let git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(&repo_dir)
                .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
                .args(args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap();
            assert!(status.success());
        };
        git(&["init", "--quiet", "--initial-branch=main"]);
        std::fs::write(repo_dir.join("README.md"), "# Payments\n\nRefunds are issued by the ledger.\n").unwrap();
        std::fs::write(repo_dir.join("src/ledger.rs"), "pub fn post_refund() {}\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "--quiet", "-m", "Initial"]);

        let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
        let read = |response: axum::response::Response| async move {
            serde_json::from_slice::<Value>(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap_or(Value::Null)
        };
        let mut tokens = Vec::new();
        for email in ["repo-owner@example.com", "repo-member@example.com"] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/auth/register")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            json!({"email": email, "display_name": email, "password": "not-a-real-password"})
                                .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            tokens.push(format!("Bearer {}", read(response).await["access_token"].as_str().unwrap()));
        }
        let (owner, member) = (&tokens[0], &tokens[1]);
        let organization = read(
            app.clone()
                .oneshot(json_request("POST", "/v1/organizations", owner, json!({"name":"Repo Team"})))
                .await
                .unwrap(),
        )
        .await;
        let workspace = read(
            app.clone()
                .oneshot(json_request(
                    "POST",
                    "/v1/workspaces",
                    owner,
                    json!({"organization_id": organization["id"], "name":"Repo Workspace"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();
        let added = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/workspaces/{workspace_id}/members"),
                owner,
                json!({"email":"repo-member@example.com","role":"member"}),
            ))
            .await
            .unwrap();
        assert!(added.status().is_success());
        let repositories_uri = format!("/v1/workspaces/{workspace_id}/repositories");

        let listing = read(app.clone().oneshot(auth_request("GET", &repositories_uri, member)).await.unwrap()).await;
        assert!(listing["repositories"].as_array().unwrap().is_empty());

        // The link is checked before anything is stored.
        let check_uri = format!("{repositories_uri}/check");
        let check = |token: &str, link: &str| json_request("POST", &check_uri, token, json!({"link": link}));
        assert_eq!(app.clone().oneshot(check(member, &repo_dir.display().to_string())).await.unwrap().status(), 403);
        for (link, expected) in [
            ("https://github.com/example/payments.git".to_owned(), "not supported yet"),
            (base.join("missing").display().to_string(), "does not exist"),
            (data_dir.display().to_string(), "not inside a git repository"),
        ] {
            let response = app.clone().oneshot(check(owner, &link)).await.unwrap();
            assert_eq!(response.status(), 400, "{link}");
            let message = read(response).await["error"]["message"].as_str().unwrap().to_owned();
            assert!(message.contains(expected), "{message}");
        }
        let checked = read(app.clone().oneshot(check(owner, &format!("\"{}\"", repo_dir.join("src").display()))).await.unwrap()).await;
        assert_eq!(checked["name"], "payments");
        assert_eq!(checked["commit"]["summary"], "Initial");
        assert_eq!(checked["commit"]["branch"], "main");
        assert_eq!(checked["tracked_files"], 2);
        assert_eq!(checked["indexable_files"], 2);
        assert_eq!(checked["already_connected"], false);

        let connect = |token: &str, path: &std::path::Path| {
            json_request("POST", &repositories_uri, token, json!({"link": path.display().to_string()}))
        };
        assert_eq!(app.clone().oneshot(connect(member, &repo_dir)).await.unwrap().status(), 403);

        let connected = app.clone().oneshot(connect(owner, &repo_dir.join("src"))).await.unwrap();
        assert_eq!(connected.status(), 201);
        let connected = read(connected).await;
        assert_eq!(connected["repository"]["name"], "payments");
        assert_eq!(connected["job"]["kind"], "repo_sync");
        let repository_id = connected["repository"]["id"].as_str().unwrap().to_owned();
        assert_eq!(app.clone().oneshot(connect(owner, &repo_dir)).await.unwrap().status(), 409);
        let checked = read(app.clone().oneshot(check(owner, &repo_dir.display().to_string())).await.unwrap()).await;
        assert_eq!(checked["already_connected"], true);

        let repository_uri = format!("/v1/repositories/{repository_id}");
        let mut repository = Value::Null;
        for _ in 0..100 {
            repository = read(app.clone().oneshot(auth_request("GET", &repository_uri, member)).await.unwrap()).await;
            if repository["status"] == "ready" && repository["active_job"].is_null() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert_eq!(repository["status"], "ready", "{repository}");
        assert_eq!(repository["file_count"], 2);
        assert_eq!(repository["indexed_file_count"], 2);
        assert_eq!(repository["last_synced_commit"]["summary"], "Initial");
        assert_eq!(repository["last_report"]["added"], 2);

        let files = read(app.clone().oneshot(auth_request("GET", &format!("{repository_uri}/files"), member)).await.unwrap()).await;
        assert_eq!(files.as_array().unwrap().len(), 2);
        let artifacts = read(
            app.clone()
                .oneshot(auth_request("GET", &format!("/v1/workspaces/{workspace_id}/artifacts"), owner))
                .await
                .unwrap(),
        )
        .await;
        let ledger = artifacts
            .as_array()
            .unwrap()
            .iter()
            .find(|artifact| artifact["path"] == "src/ledger.rs")
            .unwrap();
        assert_eq!(ledger["repository_id"], repository_id.as_str());
        // The repository itself is one evidence item, protected like its files.
        let items = artifacts
            .as_array()
            .unwrap()
            .iter()
            .filter(|artifact| artifact["artifact_type"] == "repository")
            .collect::<Vec<_>>();
        assert_eq!(items.len(), 1);
        assert!(items[0]["repository_id"].is_null());
        assert_eq!(items[0]["source_id"], repository_id.as_str());
        let item_id = items[0]["id"].as_str().unwrap();
        assert_eq!(
            app.clone().oneshot(auth_request("DELETE", &format!("/v1/artifacts/{item_id}"), owner)).await.unwrap().status(),
            400
        );
        let detail = read(app.clone().oneshot(auth_request("GET", &format!("/v1/repositories/{repository_id}/detail"), member)).await.unwrap()).await;
        assert_eq!(detail["repository"]["overview_artifact_id"], item_id);
        assert_eq!(detail["overview"]["file_count"], 2);
        assert_eq!(detail["overview"]["key_files"][0]["path"], "README.md");
        assert_eq!(detail["ai_available"], false);
        assert!(detail["summary"].is_null());
        // The knowledge map draws the repository as one node holding all of
        // its files' passages, while progress still counts every file.
        let map = read(app.clone().oneshot(auth_request("GET", &format!("/v1/workspaces/{workspace_id}/knowledge-map"), member)).await.unwrap()).await;
        let file_nodes = map["nodes"].as_array().unwrap().iter().filter(|node| node["kind"] == "file").collect::<Vec<_>>();
        assert_eq!(file_nodes.len(), 1, "{map}");
        assert_eq!(file_nodes[0]["id"], item_id);
        assert!(file_nodes[0]["passage_count"].as_i64().unwrap() >= 3);
        assert_eq!(map["pipeline"]["file_count"], 3);
        assert!(map["coverage"].as_array().unwrap().iter().any(|row| row["label"] == "Repository files" && row["file_count"] == 2));
        let summary = app.clone().oneshot(auth_request("POST", &format!("/v1/repositories/{repository_id}/summary"), member)).await.unwrap();
        assert_eq!(summary.status(), 400);
        assert!(read(summary).await["error"]["message"].as_str().unwrap().contains("No content was sent"));

        // With a text provider, the summary is generated from the overview and
        // key files, cited, and kept for the repository page.
        let (ollama_url, prompts) = start_fake_ollama().await;
        let saved = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/workspaces/{workspace_id}/ai-providers"),
                owner,
                json!({"provider_type": "ollama", "name": "Mock text", "base_url": ollama_url, "model": "mock-chat", "enabled": true, "purpose": "text"}),
            ))
            .await
            .unwrap();
        assert!(saved.status().is_success());
        let summary = app.clone().oneshot(auth_request("POST", &format!("/v1/repositories/{repository_id}/summary"), member)).await.unwrap();
        assert_eq!(summary.status(), 200);
        let summary = read(summary).await;
        assert_eq!(summary["commit_sha"], detail["overview"]["commit"]["sha"]);
        assert!(summary["citations"].as_array().unwrap().iter().any(|citation| citation["path"] == "README.md"));
        assert!(prompts.lock().unwrap().iter().any(|prompt| prompt.contains("payments")));
        let detail = read(app.clone().oneshot(auth_request("GET", &format!("/v1/repositories/{repository_id}/detail"), member)).await.unwrap()).await;
        assert_eq!(detail["ai_available"], true);
        assert_eq!(detail["summary"]["summary_markdown"], summary["summary_markdown"]);
        let artifact_id = ledger["id"].as_str().unwrap();
        let refused = app
            .clone()
            .oneshot(auth_request("DELETE", &format!("/v1/artifacts/{artifact_id}"), owner))
            .await
            .unwrap();
        assert_eq!(refused.status(), 400);

        // Members may sync; only administrators change or remove a repository.
        let synced = app.clone().oneshot(auth_request("POST", &format!("{repository_uri}/sync"), member)).await.unwrap();
        assert_eq!(synced.status(), 200);
        assert_eq!(
            app.clone()
                .oneshot(json_request("PUT", &repository_uri, member, json!({"exclude": ["*.md"]})))
                .await
                .unwrap()
                .status(),
            403
        );
        let updated = read(
            app.clone()
                .oneshot(json_request("PUT", &repository_uri, owner, json!({"name": "Payments", "exclude": ["*.md"]})))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(updated["name"], "Payments");
        assert_eq!(updated["settings"]["exclude"], json!(["*.md"]));

        for _ in 0..100 {
            repository = read(app.clone().oneshot(auth_request("GET", &repository_uri, owner)).await.unwrap()).await;
            if repository["active_job"].is_null() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert_eq!(app.clone().oneshot(auth_request("DELETE", &repository_uri, member)).await.unwrap().status(), 403);
        assert_eq!(app.clone().oneshot(auth_request("DELETE", &repository_uri, owner)).await.unwrap().status(), 204);
        let artifacts = read(
            app.clone()
                .oneshot(auth_request("GET", &format!("/v1/workspaces/{workspace_id}/artifacts"), owner))
                .await
                .unwrap(),
        )
        .await;
        assert!(artifacts.as_array().unwrap().is_empty());
        assert_eq!(app.clone().oneshot(auth_request("GET", &repository_uri, owner)).await.unwrap().status(), 404);

        drop(app);
        let _ = std::fs::remove_dir_all(base);
    }

    async fn wait_for_indexed_artifacts(
        app: &axum::Router,
        authorization: &str,
        workspace_id: &str,
        expected: usize,
    ) {
        for _ in 0..100 {
            let response = app
                .clone()
                .oneshot(auth_request(
                    "GET",
                    &format!("/v1/workspaces/{workspace_id}/artifacts"),
                    authorization,
                ))
                .await
                .unwrap();
            let artifacts: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                    .unwrap();
            let indexed = artifacts
                .as_array()
                .unwrap()
                .iter()
                .filter(|artifact| artifact["indexed_at"].is_string())
                .count();
            if indexed >= expected {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        panic!("artifacts were not indexed in the background in time");
    }

    #[tokio::test]
    async fn indexes_new_evidence_automatically_and_limits_chunks_to_admins() {
        let data_dir =
            std::env::temp_dir().join(format!("repomemo-server-auto-index-{}", uuid::Uuid::new_v4()));
        let app = router(ServerConfig::for_test(data_dir.clone()))
            .await
            .unwrap();

        let mut tokens = Vec::new();
        for email in ["auto-owner@example.com", "auto-member@example.com"] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/auth/register")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            json!({"email": email, "display_name": email, "password": "not-a-real-password"})
                                .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), 201);
            let body: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                    .unwrap();
            tokens.push(format!("Bearer {}", body["access_token"].as_str().unwrap()));
        }
        let (owner, member) = (&tokens[0], &tokens[1]);

        let organization = app
            .clone()
            .oneshot(json_request("POST", "/v1/organizations", owner, json!({"name":"Auto Team"})))
            .await
            .unwrap();
        let organization: Value = serde_json::from_slice(
            &to_bytes(organization.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        let workspace = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/workspaces",
                owner,
                json!({"organization_id": organization["id"], "name":"Auto Workspace"}),
            ))
            .await
            .unwrap();
        let workspace: Value = serde_json::from_slice(
            &to_bytes(workspace.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();
        let added = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/workspaces/{workspace_id}/members"),
                owner,
                json!({"email":"auto-member@example.com","role":"member"}),
            ))
            .await
            .unwrap();
        assert_eq!(added.status(), 200);

        let artifact = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
                member,
                json!({"title":"Auto note","content":"# Auto\nIndexed without a button.","language":"Markdown"}),
            ))
            .await
            .unwrap();
        assert_eq!(artifact.status(), 201);
        let artifact: Value =
            serde_json::from_slice(&to_bytes(artifact.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let artifact_id = artifact["id"].as_str().unwrap().to_owned();

        wait_for_indexed_artifacts(&app, owner, &workspace_id, 1).await;

        let capabilities = |authorization: String| {
            let app = app.clone();
            let workspace_id = workspace_id.clone();
            async move {
                let response = app
                    .oneshot(auth_request(
                        "GET",
                        &format!("/v1/workspaces/{workspace_id}/capabilities"),
                        &authorization,
                    ))
                    .await
                    .unwrap();
                serde_json::from_slice::<Value>(
                    &to_bytes(response.into_body(), usize::MAX).await.unwrap(),
                )
                .unwrap()
            }
        };
        assert_eq!(capabilities(owner.clone()).await["can_inspect_index"], true);
        assert_eq!(capabilities(member.clone()).await["can_inspect_index"], false);

        // A member can read the artifact but never receives its chunks.
        let detail = app
            .clone()
            .oneshot(auth_request("GET", &format!("/v1/artifacts/{artifact_id}"), member))
            .await
            .unwrap();
        assert_eq!(detail.status(), 200);
        let detail: Value =
            serde_json::from_slice(&to_bytes(detail.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert!(detail["chunks"].as_array().unwrap().is_empty());
        let member_chunks = app
            .clone()
            .oneshot(auth_request("GET", &format!("/v1/artifacts/{artifact_id}/chunks"), member))
            .await
            .unwrap();
        assert_eq!(member_chunks.status(), 403);

        // Owners and admins see the stored chunks through both routes.
        let owner_detail = app
            .clone()
            .oneshot(auth_request("GET", &format!("/v1/artifacts/{artifact_id}"), owner))
            .await
            .unwrap();
        let owner_detail: Value = serde_json::from_slice(
            &to_bytes(owner_detail.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        assert_eq!(owner_detail["chunks"].as_array().unwrap().len(), 1);
        let owner_chunks = app
            .clone()
            .oneshot(auth_request("GET", &format!("/v1/artifacts/{artifact_id}/chunks"), owner))
            .await
            .unwrap();
        assert_eq!(owner_chunks.status(), 200);
        let owner_chunks: Value = serde_json::from_slice(
            &to_bytes(owner_chunks.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        assert_eq!(owner_chunks[0]["heading_path"], "Auto");

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn workspace_health_finds_drift_and_only_admins_act_on_it() {
        let data_dir =
            std::env::temp_dir().join(format!("repomemo-server-health-{}", uuid::Uuid::new_v4()));
        let app = router(ServerConfig::for_test(data_dir.clone()))
            .await
            .unwrap();
        let read = |response: axum::response::Response| async move {
            serde_json::from_slice::<Value>(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap()
        };

        let mut tokens = Vec::new();
        for email in ["health-owner@example.com", "health-member@example.com"] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/auth/register")
                        .header("content-type", "application/json")
                        .body(Body::from(
                            json!({"email": email, "display_name": email, "password": "not-a-real-password"})
                                .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), 201);
            tokens.push(format!("Bearer {}", read(response).await["access_token"].as_str().unwrap()));
        }
        let (owner, member) = (&tokens[0], &tokens[1]);
        let organization = read(
            app.clone()
                .oneshot(json_request("POST", "/v1/organizations", owner, json!({"name":"Health Team"})))
                .await
                .unwrap(),
        )
        .await;
        let workspace = read(
            app.clone()
                .oneshot(json_request(
                    "POST",
                    "/v1/workspaces",
                    owner,
                    json!({"organization_id": organization["id"], "name":"Health Workspace"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();
        let added = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/workspaces/{workspace_id}/members"),
                owner,
                json!({"email":"health-member@example.com","role":"member"}),
            ))
            .await
            .unwrap();
        assert_eq!(added.status(), 200);

        // Two versions of one code file: the second drops `parse_config`.
        for (expected, body) in [
            (1, "fn parse_config() {}\nfn load_settings() {}\n"),
            (2, "fn load_settings() {}\n"),
        ] {
            let uploaded = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                        .header("authorization", owner)
                        .header("content-type", "text/x-rust")
                        .header("x-repomemo-filename", "parser.rs")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(uploaded.status(), 201);
            wait_for_indexed_artifacts(&app, owner, &workspace_id, expected).await;
        }
        for (title, content) in [
            ("Startup runbook", "# Startup\nCall `parse_config` before the server starts."),
            ("Copy A", "Identical body."),
            ("Copy B", "Identical body."),
        ] {
            let created = app
                .clone()
                .oneshot(json_request(
                    "POST",
                    &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
                    owner,
                    json!({"title": title, "content": content, "language": "Markdown"}),
                ))
                .await
                .unwrap();
            assert_eq!(created.status(), 201);
        }
        wait_for_indexed_artifacts(&app, owner, &workspace_id, 5).await;

        let health_uri = format!("/v1/workspaces/{workspace_id}/health");
        let actions_uri = format!("/v1/workspaces/{workspace_id}/health/actions");
        let health = app.clone().oneshot(auth_request("GET", &health_uri, member)).await.unwrap();
        assert_eq!(health.status(), 200);
        let health = read(health).await;
        let finding = |health: &Value, detector: &str| {
            health["findings"]
                .as_array()
                .unwrap()
                .iter()
                .find(|finding| finding["detector"] == detector)
                .cloned()
        };
        let older = finding(&health, "older_version_active").expect("older version finding");
        assert_eq!(older["files"].as_array().unwrap().len(), 2);
        let removed = finding(&health, "removed_symbol_mentioned").expect("removed symbol finding");
        assert!(removed["detail"].as_str().unwrap().contains("`parse_config`"));
        assert!(removed["evidence"][0]["excerpt"].as_str().unwrap().contains("parse_config"));
        let duplicate = finding(&health, "duplicate_content").expect("duplicate finding");
        assert!(finding(&health, "unconnected").is_none());
        assert_eq!(health["similarity_available"], false);

        // Members can read findings but not act on them.
        let forbidden = app
            .clone()
            .oneshot(json_request(
                "POST",
                &actions_uri,
                member,
                json!({"fingerprint": older["fingerprint"], "action": "supersede"}),
            ))
            .await
            .unwrap();
        assert_eq!(forbidden.status(), 403);

        let superseded = app
            .clone()
            .oneshot(json_request(
                "POST",
                &actions_uri,
                owner,
                json!({"fingerprint": older["fingerprint"], "action": "supersede"}),
            ))
            .await
            .unwrap();
        assert_eq!(superseded.status(), 200);
        let superseded = read(superseded).await;
        let old_version_id = superseded["updated_artifact_ids"][0].as_str().unwrap().to_owned();
        assert_ne!(old_version_id, older["keep_artifact_id"].as_str().unwrap());
        let lifecycle = read(
            app.clone()
                .oneshot(auth_request("GET", &format!("/v1/artifacts/{old_version_id}/lifecycle"), owner))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(lifecycle["status"], "superseded");
        assert_eq!(lifecycle["superseded_by_artifact_id"], older["keep_artifact_id"]);

        let not_allowed = app
            .clone()
            .oneshot(json_request(
                "POST",
                &actions_uri,
                owner,
                json!({"fingerprint": duplicate["fingerprint"], "action": "create_task"}),
            ))
            .await
            .unwrap();
        assert_eq!(not_allowed.status(), 400);
        let dismissed = app
            .clone()
            .oneshot(json_request(
                "POST",
                &actions_uri,
                owner,
                json!({"fingerprint": duplicate["fingerprint"], "action": "dismiss"}),
            ))
            .await
            .unwrap();
        assert_eq!(dismissed.status(), 200);
        let tasked = app
            .clone()
            .oneshot(json_request(
                "POST",
                &actions_uri,
                owner,
                json!({"fingerprint": removed["fingerprint"], "action": "create_task"}),
            ))
            .await
            .unwrap();
        assert_eq!(tasked.status(), 200);
        assert!(read(tasked).await["task_id"].is_string());

        // Handled findings stay hidden, and a stale fingerprint is refused.
        let health = read(app.clone().oneshot(auth_request("GET", &health_uri, owner)).await.unwrap()).await;
        assert!(health["findings"].as_array().unwrap().is_empty());
        let stats = |detector: &str| {
            health["detectors"]
                .as_array()
                .unwrap()
                .iter()
                .find(|stats| stats["detector"] == detector)
                .cloned()
                .unwrap()
        };
        assert_eq!(stats("older_version_active")["acted_count"], 1);
        assert_eq!(stats("duplicate_content")["dismissed_count"], 1);
        assert_eq!(stats("removed_symbol_mentioned")["acted_count"], 1);
        let stale = app
            .clone()
            .oneshot(json_request(
                "POST",
                &actions_uri,
                owner,
                json!({"fingerprint": older["fingerprint"], "action": "dismiss"}),
            ))
            .await
            .unwrap();
        assert_eq!(stale.status(), 409);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    fn auth_request(method: &str, uri: &str, authorization: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", authorization)
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn folders_nest_to_a_limit_hold_notes_and_images_need_a_vision_provider() {
        let data_dir =
            std::env::temp_dir().join(format!("repomemo-server-folders-{}", uuid::Uuid::new_v4()));
        let app = router(ServerConfig::for_test(data_dir)).await.unwrap();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": "folders@example.com", "display_name": "Folders", "password": "not-a-real-password"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let owner = format!("Bearer {}", body["access_token"].as_str().unwrap());
        let organization = app
            .clone()
            .oneshot(json_request("POST", "/v1/organizations", &owner, json!({"name":"Folder Team"})))
            .await
            .unwrap();
        let organization: Value = serde_json::from_slice(
            &to_bytes(organization.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        let workspace = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/workspaces",
                &owner,
                json!({"organization_id": organization["id"], "name":"Folder Workspace"}),
            ))
            .await
            .unwrap();
        let workspace: Value = serde_json::from_slice(
            &to_bytes(workspace.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();
        let folders_uri = format!("/v1/workspaces/{workspace_id}/folders");

        // Five levels are allowed, the sixth is refused.
        let mut parent: Option<String> = None;
        let mut deepest = String::new();
        for level in 1..=5 {
            let response = app
                .clone()
                .oneshot(json_request(
                    "POST",
                    &folders_uri,
                    &owner,
                    json!({"name": format!("level {level}"), "parent_id": parent}),
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), 201, "level {level}");
            let folder: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                    .unwrap();
            deepest = folder["id"].as_str().unwrap().to_owned();
            parent = Some(deepest.clone());
        }
        let too_deep = app
            .clone()
            .oneshot(json_request(
                "POST",
                &folders_uri,
                &owner,
                json!({"name": "level 6", "parent_id": deepest}),
            ))
            .await
            .unwrap();
        assert_eq!(too_deep.status(), 400);

        // A note can be filed in a folder and is reported with it.
        let note = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
                &owner,
                json!({"title": "Filed note", "content": "# Hello", "language": "Note", "folder_id": deepest}),
            ))
            .await
            .unwrap();
        assert_eq!(note.status(), 201);
        let note: Value =
            serde_json::from_slice(&to_bytes(note.into_body(), usize::MAX).await.unwrap()).unwrap();
        assert_eq!(note["folder_id"], json!(deepest));
        assert_eq!(note["artifact_type"], "note");
        assert!(note["path"].as_str().unwrap().ends_with(".note"));

        // Renaming, moving a file out, and deleting a folder with everything in it.
        let note_id = note["id"].as_str().unwrap().to_owned();
        let renamed = app
            .clone()
            .oneshot(json_request(
                "PATCH",
                &format!("{folders_uri}/{deepest}"),
                &owner,
                json!({"name": "Renamed"}),
            ))
            .await
            .unwrap();
        assert_eq!(renamed.status(), 200);
        let moved = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/artifacts/{note_id}/folder"),
                &owner,
                json!({"folder_id": null}),
            ))
            .await
            .unwrap();
        assert_eq!(moved.status(), 200);
        let moved: Value =
            serde_json::from_slice(&to_bytes(moved.into_body(), usize::MAX).await.unwrap()).unwrap();
        assert!(moved["folder_id"].is_null());
        let back_in = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/artifacts/{note_id}/folder"),
                &owner,
                json!({"folder_id": deepest}),
            ))
            .await
            .unwrap();
        assert_eq!(back_in.status(), 200);
        let folders = app
            .clone()
            .oneshot(json_request("GET", &folders_uri, &owner, json!(null)))
            .await
            .unwrap();
        let folders: Value =
            serde_json::from_slice(&to_bytes(folders.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let top = folders
            .as_array()
            .unwrap()
            .iter()
            .find(|folder| folder["parent_id"].is_null())
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let deleted = app
            .clone()
            .oneshot(json_request("DELETE", &format!("{folders_uri}/{top}"), &owner, json!(null)))
            .await
            .unwrap();
        assert_eq!(deleted.status(), 204);
        let remaining = app
            .clone()
            .oneshot(json_request("GET", &folders_uri, &owner, json!(null)))
            .await
            .unwrap();
        let remaining: Value =
            serde_json::from_slice(&to_bytes(remaining.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert!(remaining.as_array().unwrap().is_empty());
        let note_gone = app
            .clone()
            .oneshot(json_request("GET", &format!("/v1/artifacts/{note_id}"), &owner, json!(null)))
            .await
            .unwrap();
        assert_ne!(note_gone.status(), 200);

        // Images are refused until an image AI provider exists.
        let image = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                    .header("authorization", &owner)
                    .header("x-repomemo-filename", "diagram.png")
                    .header("content-type", "image/png")
                    .body(Body::from(vec![1_u8, 2, 3]))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(image.status(), 400);
    }

    #[tokio::test]
    async fn business_documents_preview_download_and_open_link() {
        let data_dir =
            std::env::temp_dir().join(format!("repomemo-server-docs-{}", uuid::Uuid::new_v4()));
        let app = router(ServerConfig::for_test(data_dir)).await.unwrap();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": "docs@example.com", "display_name": "Docs", "password": "not-a-real-password"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let owner = format!("Bearer {}", body["access_token"].as_str().unwrap());
        let organization = app
            .clone()
            .oneshot(json_request("POST", "/v1/organizations", &owner, json!({"name":"Docs Team"})))
            .await
            .unwrap();
        let organization: Value = serde_json::from_slice(
            &to_bytes(organization.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        let workspace = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/workspaces",
                &owner,
                json!({"organization_id": organization["id"], "name":"Docs Workspace"}),
            ))
            .await
            .unwrap();
        let workspace: Value = serde_json::from_slice(
            &to_bytes(workspace.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();

        let eml = "From: Ada <ada@example.com>\r\nTo: bob@example.com\r\nSubject: Plan\r\nContent-Type: text/plain\r\n\r\nPlease review.\r\n";
        let uploaded = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                    .header("authorization", &owner)
                    .header("x-repomemo-filename", "plan.eml")
                    .header("content-type", "message/rfc822")
                    .body(Body::from(eml))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(uploaded.status(), 201);
        let uploaded: Value =
            serde_json::from_slice(&to_bytes(uploaded.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(uploaded["language"], "Email");
        let artifact_id = uploaded["id"].as_str().unwrap().to_owned();

        let preview = app
            .clone()
            .oneshot(json_request(
                "GET",
                &format!("/v1/artifacts/{artifact_id}/document-preview"),
                &owner,
                json!(null),
            ))
            .await
            .unwrap();
        assert_eq!(preview.status(), 200);
        let preview: Value =
            serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(preview["kind"], "email");
        assert_eq!(preview["subject"], "Plan");
        assert_eq!(preview["body"], "Please review.");

        let file = app
            .clone()
            .oneshot(json_request(
                "GET",
                &format!("/v1/artifacts/{artifact_id}/file"),
                &owner,
                json!(null),
            ))
            .await
            .unwrap();
        assert_eq!(file.status(), 200);
        assert!(file.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .starts_with("attachment"));
        assert_eq!(to_bytes(file.into_body(), usize::MAX).await.unwrap(), eml.as_bytes());

        let link = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/artifacts/{artifact_id}/open-link"),
                &owner,
                json!({}),
            ))
            .await
            .unwrap();
        assert_eq!(link.status(), 200);
        let link: Value =
            serde_json::from_slice(&to_bytes(link.into_body(), usize::MAX).await.unwrap()).unwrap();
        let token = link["token"].as_str().unwrap();
        let public = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/shared-files/{token}/plan.eml"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(public.status(), 200);
        let forged = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/shared-files/{token}x/plan.eml"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(forged.status(), 401);
        // A file link must not work as a session token.
        let misuse = app
            .clone()
            .oneshot(json_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/artifacts"),
                &format!("Bearer {token}"),
                json!(null),
            ))
            .await
            .unwrap();
        assert_eq!(misuse.status(), 401);
    }

    /// A stand-in for LibreOffice that "converts" by copying the input to
    /// `<outdir>/input.pdf`, so the conversion pipeline can be tested anywhere.
    #[cfg(windows)]
    fn fake_soffice(directory: &std::path::Path) -> std::path::PathBuf {
        std::fs::create_dir_all(directory).unwrap();
        let path = directory.join("fake-soffice.bat");
        let script = [
            "@echo off",
            ":loop",
            "if \"%~1\"==\"\" goto done",
            "if \"%~1\"==\"--outdir\" set OUT=%~2",
            "set LAST=%~1",
            "shift",
            "goto loop",
            ":done",
            "copy /Y \"%LAST%\" \"%OUT%\\input.pdf\" >nul",
        ]
        .join("\r\n");
        std::fs::write(&path, script).unwrap();
        path
    }

    #[cfg(unix)]
    fn fake_soffice(directory: &std::path::Path) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(directory).unwrap();
        let path = directory.join("fake-soffice.sh");
        let script = [
            "#!/bin/sh",
            "while [ $# -gt 0 ]; do",
            "  if [ \"$1\" = \"--outdir\" ]; then out=\"$2\"; fi",
            "  last=\"$1\"; shift",
            "done",
            "cp \"$last\" \"$out/input.pdf\"",
        ]
        .join("\n");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[tokio::test]
    async fn office_files_are_rendered_to_a_cached_pdf() {
        let data_dir =
            std::env::temp_dir().join(format!("repomemo-server-render-{}", uuid::Uuid::new_v4()));
        let mut config = ServerConfig::for_test(data_dir.clone());
        config.soffice = Some(fake_soffice(&data_dir.join("bin")));
        let app = router(config).await.unwrap();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": "render@example.com", "display_name": "Render", "password": "not-a-real-password"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let owner = format!("Bearer {}", body["access_token"].as_str().unwrap());
        let organization = app
            .clone()
            .oneshot(json_request("POST", "/v1/organizations", &owner, json!({"name":"Render Team"})))
            .await
            .unwrap();
        let organization: Value = serde_json::from_slice(
            &to_bytes(organization.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        let workspace = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/workspaces",
                &owner,
                json!({"organization_id": organization["id"], "name":"Render Workspace"}),
            ))
            .await
            .unwrap();
        let workspace: Value = serde_json::from_slice(
            &to_bytes(workspace.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();

        let upload = |name: &'static str, bytes: &'static str| {
            let app = app.clone();
            let owner = owner.clone();
            let workspace_id = workspace_id.clone();
            async move {
                let response = app
                    .oneshot(
                        Request::builder()
                            .method("POST")
                            .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                            .header("authorization", &owner)
                            .header("x-repomemo-filename", name)
                            .body(Body::from(bytes))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), 201);
                let value: Value = serde_json::from_slice(
                    &to_bytes(response.into_body(), usize::MAX).await.unwrap(),
                )
                .unwrap();
                value["id"].as_str().unwrap().to_owned()
            }
        };
        let docx = upload("report.docx", "pretend this is a Word file").await;
        let eml = upload("mail.eml", "Subject: Hi\r\n\r\nBody").await;

        let status = |id: String| {
            let app = app.clone();
            let owner = owner.clone();
            async move {
                let response = app
                    .oneshot(json_request(
                        "GET",
                        &format!("/v1/artifacts/{id}/rendered-preview/status"),
                        &owner,
                        json!(null),
                    ))
                    .await
                    .unwrap();
                assert_eq!(response.status(), 200);
                let value: Value = serde_json::from_slice(
                    &to_bytes(response.into_body(), usize::MAX).await.unwrap(),
                )
                .unwrap();
                value["state"].as_str().unwrap().to_owned()
            }
        };

        assert_eq!(status(eml).await, "unsupported");
        let mut state = status(docx.clone()).await;
        for _ in 0..100 {
            if state == "ready" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            state = status(docx.clone()).await;
        }
        assert_eq!(state, "ready");

        let pdf = app
            .clone()
            .oneshot(json_request(
                "GET",
                &format!("/v1/artifacts/{docx}/rendered-preview"),
                &owner,
                json!(null),
            ))
            .await
            .unwrap();
        assert_eq!(pdf.status(), 200);
        assert_eq!(
            to_bytes(pdf.into_body(), usize::MAX).await.unwrap(),
            "pretend this is a Word file".as_bytes()
        );
    }

    #[tokio::test]
    async fn assistant_routes_messages_and_works_without_ai() {
        let data_dir =
            std::env::temp_dir().join(format!("repomemo-server-agent-{}", uuid::Uuid::new_v4()));
        let app = router(ServerConfig::for_test(data_dir.clone()))
            .await
            .unwrap();
        let read = |response: axum::response::Response| async move {
            serde_json::from_slice::<Value>(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap()
        };

        let registered = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": "agent@example.com", "display_name": "Agent", "password": "not-a-real-password"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let owner = format!("Bearer {}", read(registered).await["access_token"].as_str().unwrap());
        let organization = read(
            app.clone()
                .oneshot(json_request("POST", "/v1/organizations", &owner, json!({"name":"Agent Team"})))
                .await
                .unwrap(),
        )
        .await;
        let workspace = read(
            app.clone()
                .oneshot(json_request(
                    "POST",
                    "/v1/workspaces",
                    &owner,
                    json!({"organization_id": organization["id"], "name":"Agent Workspace"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();
        for (title, content) in [
            ("Retry policy", "# Retries\nUploads use an exponential backoff budget."),
            ("Deploy runbook", "# Deploy\nRun the migration before switching traffic."),
        ] {
            let created = app
                .clone()
                .oneshot(json_request(
                    "POST",
                    &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
                    &owner,
                    json!({"title": title, "content": content, "language": "Markdown"}),
                ))
                .await
                .unwrap();
            assert_eq!(created.status(), 201);
        }
        wait_for_indexed_artifacts(&app, &owner, &workspace_id, 2).await;

        let capabilities = read(
            app.clone()
                .oneshot(auth_request(
                    "GET",
                    &format!("/v1/workspaces/{workspace_id}/agent/capabilities"),
                    &owner,
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(capabilities["provider_name"], Value::Null);
        let available = capabilities["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|capability| capability["available"] == true)
            .map(|capability| capability["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(available, ["find_files", "search_content"]);

        let send = |body: Value| {
            let app = app.clone();
            let uri = format!("/v1/workspaces/{workspace_id}/agent/messages");
            let owner = owner.clone();
            async move {
                let response = app.oneshot(json_request("POST", &uri, &owner, body)).await.unwrap();
                assert_eq!(response.status(), 200);
                read(response).await
            }
        };

        // The first message starts a conversation named after it.
        let first = send(json!({"message": "find the deploy runbook"})).await;
        let conversation_id = first["conversation"]["id"].as_str().unwrap().to_owned();
        assert_eq!(first["conversation"]["title"], "find the deploy runbook");
        let found = &first["turn"]["reply"];
        assert_eq!(found["capability"], "find_files");
        assert_eq!(found["routing"], "rules");
        assert_eq!(found["files"][0]["title"], "Deploy runbook");

        let in_chat = |body: Value| {
            let mut body = body;
            body["conversation_id"] = json!(conversation_id);
            send(body)
        };
        let searched = in_chat(json!({"message": "backoff", "capability": "search_content"})).await;
        assert_eq!(searched["turn"]["reply"]["routing"], "explicit");
        assert_eq!(searched["turn"]["reply"]["matches"][0]["title"], "Retry policy");

        // A file lookup with no matching name falls back to the indexed content.
        let fallback = in_chat(json!({"message": "find migration"})).await;
        assert_eq!(fallback["turn"]["reply"]["files"].as_array().unwrap().len(), 0);
        assert_eq!(fallback["turn"]["reply"]["matches"][0]["title"], "Deploy runbook");

        let summary = in_chat(json!({"message": "summarize the retry policy"})).await;
        assert_eq!(summary["turn"]["reply"]["capability"], "summarize_file");
        assert_eq!(summary["turn"]["reply"]["generated"], false);
        assert!(summary["turn"]["reply"]["reply_markdown"].as_str().unwrap().contains("needs an AI provider"));

        let unmatched = in_chat(json!({"message": "write me a poem"})).await;
        assert_eq!(unmatched["turn"]["reply"]["capability"], Value::Null);
        assert_eq!(unmatched["turn"]["reply"]["routing"], "unmatched");
        assert_eq!(unmatched["turn"]["position"], 5);
        assert_eq!(unmatched["conversation"]["turn_count"], 5);

        // A second chat, then the history of both.
        let overview = send(json!({"capability": "workspace_overview"})).await;
        assert_eq!(overview["conversation"]["title"], "Workspace overview");
        assert_ne!(overview["conversation"]["id"], json!(conversation_id));
        let get = |uri: String, authorization: String| {
            let app = app.clone();
            async move { app.oneshot(auth_request("GET", &uri, &authorization)).await.unwrap() }
        };
        let chats = read(get(format!("/v1/workspaces/{workspace_id}/agent/conversations"), owner.clone()).await).await;
        assert_eq!(chats.as_array().unwrap().len(), 2);
        assert_eq!(chats[0]["title"], "Workspace overview");
        let history = read(get(format!("/v1/agent/conversations/{conversation_id}"), owner.clone()).await).await;
        let labels = history["turns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|turn| turn["label"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(labels, ["find the deploy runbook", "backoff", "find migration", "summarize the retry policy", "write me a poem"]);
        assert_eq!(history["turns"][1]["reply"]["matches"][0]["title"], "Retry policy");

        let renamed = app
            .clone()
            .oneshot(json_request("PUT", &format!("/v1/agent/conversations/{conversation_id}"), &owner, json!({"title": "  Runbook   hunt "})))
            .await
            .unwrap();
        assert_eq!(read(renamed).await["title"], "Runbook hunt");

        // Another member of the workspace cannot see or continue these chats.
        let registered = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": "agent-other@example.com", "display_name": "Other", "password": "not-a-real-password"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let other = format!("Bearer {}", read(registered).await["access_token"].as_str().unwrap());
        let added = app
            .clone()
            .oneshot(json_request("PUT", &format!("/v1/workspaces/{workspace_id}/members"), &owner, json!({"email": "agent-other@example.com", "role": "member"})))
            .await
            .unwrap();
        assert_eq!(added.status(), 200);
        let others_chats = read(get(format!("/v1/workspaces/{workspace_id}/agent/conversations"), other.clone()).await).await;
        assert_eq!(others_chats.as_array().unwrap().len(), 0);
        assert_eq!(get(format!("/v1/agent/conversations/{conversation_id}"), other.clone()).await.status(), 404);
        let hijack = app
            .clone()
            .oneshot(json_request("POST", &format!("/v1/workspaces/{workspace_id}/agent/messages"), &other, json!({"message": "find runbook", "conversation_id": conversation_id})))
            .await
            .unwrap();
        assert_eq!(hijack.status(), 404);

        let deleted = app
            .clone()
            .oneshot(auth_request("DELETE", &format!("/v1/agent/conversations/{conversation_id}"), &owner))
            .await
            .unwrap();
        assert_eq!(deleted.status(), 204);
        assert_eq!(get(format!("/v1/agent/conversations/{conversation_id}"), owner.clone()).await.status(), 404);

        let outsider = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/agent/messages"),
                "Bearer not-a-token",
                json!({"message": "find runbook"}),
            ))
            .await
            .unwrap();
        assert_eq!(outsider.status(), 401);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    /// A stand-in for Ollama. Embeddings are 3-dimensional "topic" vectors
    /// (shipping, resilience, constant) so meaning can match without shared
    /// words; every generate prompt is recorded.
    async fn start_fake_ollama() -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        use axum::routing::{get, post};
        let prompts = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let topic = |text: &str| {
            let text = text.to_lowercase();
            let has = |words: &[&str]| if words.iter().any(|word| text.contains(word)) { 1.0 } else { 0.0 };
            json!([has(&["deploy", "ship", "release"]), has(&["retry", "backoff", "resilien"]), 0.1])
        };
        let recorded = prompts.clone();
        let app = axum::Router::new()
            .route("/api/tags", get(|| async { axum::Json(json!({"models": [{"name": "mock-chat"}, {"name": "mock-embed"}]})) }))
            .route(
                "/api/embed",
                post(move |axum::Json(body): axum::Json<Value>| async move {
                    let vectors = body["input"].as_array().unwrap().iter().map(|text| topic(text.as_str().unwrap())).collect::<Vec<_>>();
                    axum::Json(json!({ "embeddings": vectors }))
                }),
            )
            .route(
                "/api/generate",
                post(move |axum::Json(body): axum::Json<Value>| {
                    let recorded = recorded.clone();
                    async move {
                        let prompt = body["prompt"].as_str().unwrap_or_default().to_owned();
                        recorded.lock().unwrap().push(prompt);
                        axum::Json(json!({ "response": "Run the migration first, then switch traffic [1]." }))
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{address}"), prompts)
    }

    #[tokio::test]
    async fn embeddings_power_semantic_search_and_ask_gets_full_passages() {
        let data_dir =
            std::env::temp_dir().join(format!("repomemo-server-embeddings-{}", uuid::Uuid::new_v4()));
        let app = router(ServerConfig::for_test(data_dir.clone()))
            .await
            .unwrap();
        let (ollama_url, prompts) = start_fake_ollama().await;
        let read = |response: axum::response::Response| async move {
            let status = response.status();
            let body = serde_json::from_slice::<Value>(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
            assert!(status.is_success(), "{status}: {body}");
            body
        };
        let registered = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(json!({"email": "vectors@example.com", "display_name": "Vectors", "password": "not-a-real-password"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let owner = format!("Bearer {}", read(registered).await["access_token"].as_str().unwrap());
        let call = |method: &'static str, uri: String, body: Value| {
            let app = app.clone();
            let owner = owner.clone();
            async move { app.oneshot(json_request(method, &uri, &owner, body)).await.unwrap() }
        };
        let organization = read(call("POST", "/v1/organizations".to_owned(), json!({"name": "Vector Team"})).await).await;
        let workspace = read(call("POST", "/v1/workspaces".to_owned(), json!({"organization_id": organization["id"], "name": "Vector Workspace"})).await).await;
        let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();
        let mut artifact_ids = Vec::new();
        for (title, content) in [
            ("Deploy runbook", "# Deploy\nRun the migration before switching traffic."),
            ("Retry policy", "# Retries\nUploads use an exponential backoff budget."),
            ("Release checklist", "# Release\nTag the build and announce the window."),
        ] {
            let created = read(call("POST", format!("/v1/workspaces/{workspace_id}/artifacts/text"), json!({"title": title, "content": content, "language": "Markdown"})).await).await;
            artifact_ids.push(created["id"].as_str().unwrap().to_owned());
        }
        wait_for_indexed_artifacts(&app, &owner, &workspace_id, 3).await;
        read(call("POST", format!("/v1/workspaces/{workspace_id}/memory-cards"), json!({
            "title": "Uploads retry with backoff",
            "body_markdown": "Decided in the retry review.",
            "source": "manual",
            "citations": [{"artifact_id": artifact_ids[1], "chunk_id": null, "title": "Retry policy", "path": "Retry policy", "start_line": null, "end_line": null, "confidence": null}]
        })).await).await;

        // Without an embedding provider the map still shows indexing and memory links.
        let map = read(call("GET", format!("/v1/workspaces/{workspace_id}/knowledge-map"), json!({})).await).await;
        assert_eq!(map["pipeline"]["file_count"], 3);
        assert_eq!(map["pipeline"]["indexed_count"], 3);
        assert_eq!(map["pipeline"]["embedded_count"], Value::Null);
        assert_eq!(map["similarity_available"], false);
        let kinds = |map: &Value, kind: &str| map["edges"].as_array().unwrap().iter().filter(|edge| edge["kind"] == kind).cloned().collect::<Vec<_>>();
        let cites = kinds(&map, "cites");
        assert_eq!(cites.len(), 1);
        assert_eq!(cites[0]["target"], json!(artifact_ids[1]));

        let provider = |purpose: &str, model: &str| json!({"provider_type": "ollama", "name": format!("Mock {purpose}"), "base_url": ollama_url, "model": model, "enabled": true, "purpose": purpose});
        let embedder = read(call("PUT", format!("/v1/workspaces/{workspace_id}/ai-providers"), provider("embedding", "mock-embed")).await).await;
        assert_eq!(embedder["purpose"], "embedding");

        // Existing chunks are embedded in the background once the provider is saved.
        let mut embedded = false;
        for _ in 0..100 {
            let metrics = read(app.clone().oneshot(auth_request("GET", &format!("/v1/workspaces/{workspace_id}/metrics"), &owner)).await.unwrap()).await;
            if metrics["embedded_chunk_count"].as_i64().is_some_and(|count| count > 0 && count == metrics["chunk_count"].as_i64().unwrap()) {
                embedded = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(embedded, "chunks were not embedded in time");

        let tested = read(call("POST", format!("/v1/workspaces/{workspace_id}/ai-providers/{}/test", embedder["id"].as_str().unwrap()), json!({})).await).await;
        assert_eq!(tested["success"], true);
        assert!(tested["message"].as_str().unwrap().contains("3 dimensions"));

        // Embedded, the map links the two shipping documents and only those.
        let map = read(call("GET", format!("/v1/workspaces/{workspace_id}/knowledge-map"), json!({})).await).await;
        assert_eq!(map["similarity_available"], true);
        assert_eq!(map["pipeline"]["embedded_count"], map["pipeline"]["passage_count"]);
        let similar = kinds(&map, "similar");
        assert_eq!(similar.len(), 1, "{similar:?}");
        let pair = [similar[0]["source"].as_str().unwrap(), similar[0]["target"].as_str().unwrap()];
        assert!(pair.contains(&artifact_ids[0].as_str()) && pair.contains(&artifact_ids[2].as_str()));

        // "shipping" appears in no file, but means the same as "deploy".
        let results = read(call("POST", format!("/v1/workspaces/{workspace_id}/search"), json!({"query": "shipping", "limit": 5})).await).await;
        let top_two = [results[0]["title"].as_str().unwrap(), results[1]["title"].as_str().unwrap()];
        assert!(top_two.contains(&"Deploy runbook") && top_two.contains(&"Release checklist"), "{results}");

        read(call("PUT", format!("/v1/workspaces/{workspace_id}/ai-providers"), provider("text", "mock-chat")).await).await;
        let answer = read(call("POST", format!("/v1/workspaces/{workspace_id}/ask"), json!({"question": "How do we ship safely?"})).await).await;
        // Both shipping documents are equally relevant, so either may lead.
        assert!(["Deploy runbook", "Release checklist"].contains(&answer["citations"][0]["title"].as_str().unwrap()), "{answer}");
        assert!(answer["warnings"].as_array().unwrap().iter().all(|warning| !warning.as_str().unwrap().contains("keyword search only")));
        let last_prompt = prompts.lock().unwrap().last().cloned().unwrap();
        assert!(last_prompt.contains("Run the migration before switching traffic."), "the model should see the full passage: {last_prompt}");
        let _ = std::fs::remove_dir_all(data_dir);
    }

    fn temp_data_dir(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("repomemo-server-{label}-{}", uuid::Uuid::new_v4()))
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    }

    fn public_post(uri: &str, body: Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    /// Registers an account and returns its bearer header and token response.
    async fn register_account(app: &axum::Router, email: &str) -> (String, Value) {
        let response = app
            .clone()
            .oneshot(public_post(
                "/v1/auth/register",
                json!({"email": email, "display_name": "Person", "password": "not-a-real-password"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
        let body = response_json(response).await;
        (format!("Bearer {}", body["access_token"].as_str().unwrap()), body)
    }

    /// Creates an organization and a workspace owned by `owner`.
    async fn create_workspace(app: &axum::Router, owner: &str) -> String {
        let organization = response_json(
            app.clone()
                .oneshot(json_request("POST", "/v1/organizations", owner, json!({"name": "Team"})))
                .await
                .unwrap(),
        )
        .await;
        let workspace = response_json(
            app.clone()
                .oneshot(json_request(
                    "POST",
                    "/v1/workspaces",
                    owner,
                    json!({"organization_id": organization["id"], "name": "Workspace"}),
                ))
                .await
                .unwrap(),
        )
        .await;
        workspace["workspace"]["id"].as_str().unwrap().to_owned()
    }

    #[tokio::test]
    async fn signing_out_everywhere_ends_access_and_refresh_tokens() {
        let data_dir = temp_data_dir("logout-all");
        let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
        let (owner, tokens) = register_account(&app, "everywhere@example.com").await;
        let session = app.clone().oneshot(auth_request("GET", "/v1/session", &owner)).await.unwrap();
        assert_eq!(session.status(), 200);

        let ended = app.clone().oneshot(auth_request("POST", "/v1/auth/logout-all", &owner)).await.unwrap();
        assert_eq!(ended.status(), 204);
        let session = app.clone().oneshot(auth_request("GET", "/v1/session", &owner)).await.unwrap();
        assert_eq!(session.status(), 401, "the access token stops working at once");
        let refreshed = app
            .clone()
            .oneshot(public_post("/v1/auth/refresh", json!({"refresh_token": tokens["refresh_token"]})))
            .await
            .unwrap();
        assert_eq!(refreshed.status(), 401);

        // Signing in again starts a new, working session.
        let login = app
            .clone()
            .oneshot(public_post(
                "/v1/auth/login",
                json!({"email": "everywhere@example.com", "password": "not-a-real-password"}),
            ))
            .await
            .unwrap();
        assert_eq!(login.status(), 200);
        let fresh = format!("Bearer {}", response_json(login).await["access_token"].as_str().unwrap());
        let session = app.clone().oneshot(auth_request("GET", "/v1/session", &fresh)).await.unwrap();
        assert_eq!(session.status(), 200);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn repeated_failed_sign_ins_lock_the_account_for_a_while() {
        let data_dir = temp_data_dir("lockout");
        let mut config = ServerConfig::for_test(data_dir.clone());
        config.login_max_failures = 3;
        let app = router(config).await.unwrap();
        register_account(&app, "locked@example.com").await;
        register_account(&app, "unaffected@example.com").await;
        let login = |email: &str, password: &str| {
            public_post("/v1/auth/login", json!({"email": email, "password": password}))
        };

        for _ in 0..3 {
            let failed = app.clone().oneshot(login("locked@example.com", "wrong-password-123")).await.unwrap();
            assert_eq!(failed.status(), 401);
        }
        let locked = app.clone().oneshot(login("locked@example.com", "not-a-real-password")).await.unwrap();
        assert_eq!(locked.status(), 429, "even the right password waits out the lockout");
        assert!(locked.headers().contains_key("retry-after"));
        assert_eq!(response_json(locked).await["error"]["code"], "rate_limited");

        let other = app.clone().oneshot(login("unaffected@example.com", "not-a-real-password")).await.unwrap();
        assert_eq!(other.status(), 200, "other accounts are not affected");
        let unknown = app.clone().oneshot(login("nobody@example.com", "not-a-real-password")).await.unwrap();
        assert_eq!(unknown.status(), 401);
        assert_eq!(response_json(unknown).await["error"]["code"], "invalid_credentials");
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn the_authentication_rate_limit_answers_429() {
        let data_dir = temp_data_dir("auth-quota");
        let mut config = ServerConfig::for_test(data_dir.clone());
        config.auth_quota = super::Quota::per_minute(2);
        let app = router(config).await.unwrap();
        let sign_in = || public_post("/v1/auth/login", json!({"email": "nobody@example.com", "password": "not-a-real-password"}));
        for _ in 0..2 {
            assert_eq!(app.clone().oneshot(sign_in()).await.unwrap().status(), 401);
        }
        assert_eq!(app.clone().oneshot(sign_in()).await.unwrap().status(), 429);

        // Refreshes have their own, wider bucket.
        let refresh = || public_post("/v1/auth/refresh", json!({"refresh_token": "unknown"}));
        for _ in 0..8 {
            assert_eq!(app.clone().oneshot(refresh()).await.unwrap().status(), 401);
        }
        assert_eq!(app.clone().oneshot(refresh()).await.unwrap().status(), 429);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn closed_registration_only_admits_the_first_account() {
        let data_dir = temp_data_dir("closed-registration");
        let mut config = ServerConfig::for_test(data_dir.clone());
        config.allow_registration = false;
        let app = router(config).await.unwrap();
        let health = response_json(app.clone().oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap()).await;
        assert_eq!(health["registration_open"], true, "the first owner can still register");
        register_account(&app, "first@example.com").await;

        let second = app
            .clone()
            .oneshot(public_post(
                "/v1/auth/register",
                json!({"email": "second@example.com", "display_name": "Second", "password": "not-a-real-password"}),
            ))
            .await
            .unwrap();
        assert_eq!(second.status(), 403);
        let health = response_json(app.clone().oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap()).await;
        assert_eq!(health["registration_open"], false);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn responses_carry_security_headers_and_readiness_checks_the_database() {
        let data_dir = temp_data_dir("headers");
        let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
        let health = app.clone().oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap();
        let headers = health.headers();
        assert_eq!(headers["x-content-type-options"], "nosniff");
        assert_eq!(headers["x-frame-options"], "DENY");
        assert_eq!(headers["referrer-policy"], "no-referrer");
        assert_eq!(headers["cache-control"], "no-store");
        assert!(headers.contains_key("content-security-policy"));

        let ready = app.clone().oneshot(Request::builder().uri("/health/ready").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(ready.status(), 200);
        assert_eq!(response_json(ready).await["database"], "ok");
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn the_ai_policy_and_quota_decide_who_may_call_the_provider() {
        let data_dir = temp_data_dir("ai-policy");
        let mut config = ServerConfig::for_test(data_dir.clone());
        config.ai_quota = super::Quota::per_hour(2);
        let app = router(config).await.unwrap();
        let (ollama_url, prompts) = start_fake_ollama().await;
        let (owner, _) = register_account(&app, "ai-owner@example.com").await;
        let (viewer, _) = register_account(&app, "ai-viewer@example.com").await;
        let workspace_id = create_workspace(&app, &owner).await;
        let call = |method: &'static str, uri: String, who: &str, body: Value| {
            let app = app.clone();
            let request = json_request(method, &uri, who, body);
            async move { app.oneshot(request).await.unwrap() }
        };
        let added = call("PUT", format!("/v1/workspaces/{workspace_id}/members"), &owner, json!({"email": "ai-viewer@example.com", "role": "viewer"})).await;
        assert_eq!(added.status(), 200);
        let provider = call("PUT", format!("/v1/workspaces/{workspace_id}/ai-providers"), &owner, json!({"provider_type": "ollama", "name": "Mock", "base_url": ollama_url, "model": "mock-chat", "enabled": true, "purpose": "text"})).await;
        assert_eq!(provider.status(), 200);

        let policy = response_json(call("GET", format!("/v1/workspaces/{workspace_id}/ai-policy"), &viewer, json!({})).await).await;
        assert_eq!(policy["min_role"], "viewer", "everyone may use AI by default");
        let capabilities = response_json(call("GET", format!("/v1/workspaces/{workspace_id}/capabilities"), &viewer, json!({})).await).await;
        assert_eq!(capabilities["can_use_ai"], true);

        let refused = call("PUT", format!("/v1/workspaces/{workspace_id}/ai-policy"), &viewer, json!({"min_role": "admin"})).await;
        assert_eq!(refused.status(), 403, "viewers cannot change the policy");
        let changed = call("PUT", format!("/v1/workspaces/{workspace_id}/ai-policy"), &owner, json!({"min_role": "member"})).await;
        assert_eq!(changed.status(), 200);

        let capabilities = response_json(call("GET", format!("/v1/workspaces/{workspace_id}/capabilities"), &viewer, json!({})).await).await;
        assert_eq!(capabilities["can_use_ai"], false);
        assert_eq!(capabilities["can_generate_ai_overview"], false);
        let asked = call("POST", format!("/v1/workspaces/{workspace_id}/ask"), &viewer, json!({"question": "How do we deploy?"})).await;
        assert_eq!(asked.status(), 403);
        assert!(response_json(asked).await["error"]["message"].as_str().unwrap().contains("available to members"));
        let agent = response_json(call("GET", format!("/v1/workspaces/{workspace_id}/agent/capabilities"), &viewer, json!({})).await).await;
        assert!(agent["capabilities"].as_array().unwrap().iter().filter(|entry| entry["requires_ai"] == true).all(|entry| entry["available"] == false));
        let reply = call("POST", format!("/v1/workspaces/{workspace_id}/agent/messages"), &viewer, json!({"message": "What is this workspace about?", "capability": "workspace_overview"})).await;
        assert_eq!(reply.status(), 200);
        assert!(response_json(reply).await["turn"]["reply"]["reply_markdown"].as_str().unwrap().contains("available to members"));
        assert!(prompts.lock().unwrap().is_empty(), "nothing reached the provider for the viewer");

        // The owner may, up to the hourly quota.
        for _ in 0..2 {
            let asked = call("POST", format!("/v1/workspaces/{workspace_id}/ask"), &owner, json!({"question": "How do we deploy?"})).await;
            assert_eq!(asked.status(), 200);
        }
        let limited = call("POST", format!("/v1/workspaces/{workspace_id}/ask"), &owner, json!({"question": "How do we deploy?"})).await;
        assert_eq!(limited.status(), 429);
        assert!(limited.headers().contains_key("retry-after"));
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn saved_provider_keys_only_go_to_the_address_they_were_saved_for() {
        let data_dir = temp_data_dir("provider-keys");
        let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
        let (owner, _) = register_account(&app, "keys@example.com").await;
        let workspace_id = create_workspace(&app, &owner).await;
        let save = |body: Value| {
            let app = app.clone();
            let request = json_request("PUT", &format!("/v1/workspaces/{workspace_id}/ai-providers"), &owner, body);
            async move { app.oneshot(request).await.unwrap() }
        };
        let saved = save(json!({"provider_type": "openrouter", "name": "Cloud", "base_url": "https://openrouter.ai/api/v1", "model": "openai/gpt-4o-mini", "api_key": "sk-or-v1-secret", "enabled": true, "cloud_content_acknowledged": true, "purpose": "text"})).await;
        assert_eq!(saved.status(), 200);
        let saved = response_json(saved).await;
        assert!(saved.get("api_key").is_none(), "keys are never sent back");
        let id = saved["id"].as_str().unwrap().to_owned();

        let moved = save(json!({"id": id, "provider_type": "openrouter", "name": "Cloud", "base_url": "https://collector.example/api/v1", "model": "openai/gpt-4o-mini", "enabled": true, "purpose": "text"})).await;
        assert_eq!(moved.status(), 400);
        assert!(response_json(moved).await["error"]["message"].as_str().unwrap().contains("Enter the API key again"));
        let renamed = save(json!({"id": id, "provider_type": "openrouter", "name": "Cloud renamed", "base_url": "https://openrouter.ai/api/v1/", "model": "openai/gpt-4o-mini", "enabled": true, "purpose": "text"})).await;
        assert_eq!(renamed.status(), 200, "the key is kept while the address stays the same");

        let metadata = save(json!({"provider_type": "ollama", "name": "Metadata", "base_url": "http://169.254.169.254/latest", "model": "x", "enabled": true, "purpose": "vision"})).await;
        assert_eq!(metadata.status(), 400);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn uploads_accept_percent_encoded_file_names() {
        let data_dir = temp_data_dir("upload-names");
        let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
        let (owner, _) = register_account(&app, "names@example.com").await;
        let workspace_id = create_workspace(&app, &owner).await;
        let upload = |name: &str| {
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                .header("authorization", &owner)
                .header("content-type", "text/markdown")
                .header("x-repomemo-filename", name)
                .body(Body::from("# Notes\nContent."))
                .unwrap()
        };
        let encoded = app.clone().oneshot(upload("R%C3%A9sum%C3%A9%20%25%20notes.md")).await.unwrap();
        assert_eq!(encoded.status(), 201);
        assert_eq!(response_json(encoded).await["title"], "Résumé % notes.md");
        let plain = app.clone().oneshot(upload("plain.md")).await.unwrap();
        assert_eq!(response_json(plain).await["title"], "plain.md");
        let too_long = app.clone().oneshot(upload(&format!("{}.md", "a".repeat(300)))).await.unwrap();
        assert_eq!(too_long.status(), 400);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn event_streams_end_when_the_member_is_removed() {
        let data_dir = temp_data_dir("event-access");
        let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
        let (owner, _) = register_account(&app, "stream-owner@example.com").await;
        let (member, registration) = register_account(&app, "stream-member@example.com").await;
        let member_id = registration["user"]["id"].as_str().unwrap().to_owned();
        let workspace_id = create_workspace(&app, &owner).await;
        let added = app
            .clone()
            .oneshot(json_request("PUT", &format!("/v1/workspaces/{workspace_id}/members"), &owner, json!({"email": "stream-member@example.com", "role": "member"})))
            .await
            .unwrap();
        assert_eq!(added.status(), 200);

        let stream = app
            .clone()
            .oneshot(auth_request("GET", &format!("/v1/workspaces/{workspace_id}/events"), &member))
            .await
            .unwrap();
        assert_eq!(stream.status(), 200);
        let removed = app
            .clone()
            .oneshot(auth_request("DELETE", &format!("/v1/workspaces/{workspace_id}/members/{member_id}"), &owner))
            .await
            .unwrap();
        assert_eq!(removed.status(), 204);
        let ended = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            to_bytes(stream.into_body(), usize::MAX),
        )
        .await;
        assert!(ended.is_ok(), "the stream should close once the member is removed");
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn a_maintenance_pass_runs_cleanly() {
        let data_dir = temp_data_dir("maintenance");
        let config = ServerConfig::for_test(data_dir.clone());
        let (state, _tasks) = super::build_state(&config, std::sync::Weak::new()).await.unwrap();
        let report = super::maintenance::run_pass(&state, true, true).await;
        assert_eq!(report.failures, 0, "{report:?}");
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn system_administrators_see_every_workspace_and_run_the_server() {
        let data_dir = temp_data_dir("system-admin");
        let mut config = ServerConfig::for_test(data_dir.clone());
        config.system_admin_emails = vec!["listed@example.com".to_owned()];
        let app = router(config).await.unwrap();
        let get = |uri: String, who: &str| {
            let app = app.clone();
            let request = auth_request("GET", &uri, who);
            async move { app.oneshot(request).await.unwrap() }
        };
        let send = |method: &'static str, uri: String, who: &str, body: Value| {
            let app = app.clone();
            let request = json_request(method, &uri, who, body);
            async move { app.oneshot(request).await.unwrap() }
        };

        // The first account of a new server administers it; later ones do not.
        let (admin, _) = register_account(&app, "first@example.com").await;
        let (owner, owner_tokens) = register_account(&app, "owner@example.com").await;
        let owner_id = owner_tokens["user"]["id"].as_str().unwrap().to_owned();
        assert_eq!(response_json(get("/v1/session".to_owned(), &admin).await).await["is_system_admin"], true);
        assert_eq!(response_json(get("/v1/session".to_owned(), &owner).await).await["is_system_admin"], false);
        let (listed, _) = register_account(&app, "listed@example.com").await;
        assert_eq!(response_json(get("/v1/session".to_owned(), &listed).await).await["is_system_admin"], true, "listed accounts are promoted");

        // Someone else's organization and workspace are visible, as an administrator.
        let workspace_id = create_workspace(&app, &owner).await;
        let organizations = response_json(get("/v1/organizations".to_owned(), &admin).await).await;
        assert_eq!(organizations.as_array().unwrap().len(), 1);
        assert_eq!(organizations[0]["role"], "admin");
        let organization_id = organizations[0]["id"].as_str().unwrap().to_owned();
        let workspaces = response_json(get("/v1/workspaces".to_owned(), &admin).await).await;
        assert_eq!(workspaces[0]["role"], "admin");
        assert_eq!(get(format!("/v1/workspaces/{workspace_id}/overview"), &admin).await.status(), 200);
        assert_eq!(get(format!("/v1/workspaces/{workspace_id}/members"), &admin).await.status(), 200, "administrator sections");
        assert_eq!(get(format!("/v1/organizations/{organization_id}/members"), &admin).await.status(), 200);
        let capabilities = response_json(get(format!("/v1/workspaces/{workspace_id}/capabilities"), &admin).await).await;
        assert_eq!(capabilities["can_manage_members"], true);
        assert_eq!(capabilities["can_manage_workspace"], false, "owner-only actions stay with the owner");
        assert_eq!(send("DELETE", format!("/v1/workspaces/{workspace_id}"), &admin, json!({})).await.status(), 403);
        let created = send("POST", "/v1/workspaces".to_owned(), &admin, json!({"organization_id": organization_id, "name": "Admin made"})).await;
        assert_eq!(created.status(), 201, "administrators create workspaces in any organization");
        assert_eq!(get(format!("/v1/workspaces/{workspace_id}/overview"), &listed).await.status(), 200);

        // The system API is for system administrators only.
        assert_eq!(get("/v1/system/overview".to_owned(), &owner).await.status(), 403);
        let overview = response_json(get("/v1/system/overview".to_owned(), &admin).await).await;
        assert_eq!(overview["statistics"]["user_count"], 3);
        assert_eq!(overview["statistics"]["workspace_count"], 2);
        assert_eq!(overview["instance"]["system_admin_count"], 2);
        assert!(overview["traffic"]["requests"].as_u64().unwrap() > 0);
        let usage = response_json(get("/v1/system/usage?days=7".to_owned(), &admin).await).await;
        assert_eq!(usage["activity_by_day"].as_array().unwrap().len(), 7);
        assert_eq!(usage["workspaces"].as_array().unwrap().len(), 2);
        assert_eq!(get("/v1/system/jobs".to_owned(), &admin).await.status(), 200);
        assert_eq!(get("/v1/system/logs?level=warn".to_owned(), &admin).await.status(), 200);

        // Run-time settings apply at once and can be reset.
        let refused = send("PUT", "/v1/system/settings".to_owned(), &admin, json!({"values": {"allow_registration": false, "login_max_failures": -3}})).await;
        assert_eq!(refused.status(), 400, "a request applies completely or not at all");
        let health = response_json(app.clone().oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap()).await.unwrap()).await;
        assert_eq!(health["registration_open"], true);
        let changed = send("PUT", "/v1/system/settings".to_owned(), &admin, json!({"values": {"allow_registration": false}})).await;
        assert_eq!(changed.status(), 200);
        let changed = response_json(changed).await;
        let registration = changed["settings"].as_array().unwrap().iter().find(|setting| setting["key"] == "allow_registration").unwrap().clone();
        assert_eq!((registration["value"].clone(), registration["overridden"].clone()), (json!(false), json!(true)));
        let closed = app.clone().oneshot(public_post("/v1/auth/register", json!({"email": "late@example.com", "display_name": "Late", "password": "not-a-real-password"}))).await.unwrap();
        assert_eq!(closed.status(), 403);
        assert_eq!(send("DELETE", "/v1/system/settings/allow_registration".to_owned(), &admin, json!({})).await.status(), 200);
        let reopened = app.clone().oneshot(public_post("/v1/auth/register", json!({"email": "late@example.com", "display_name": "Late", "password": "not-a-real-password"}))).await.unwrap();
        assert_eq!(reopened.status(), 201);

        // Roles, sessions and the last administrator.
        let users = response_json(get("/v1/system/users".to_owned(), &admin).await).await;
        assert_eq!(users.as_array().unwrap().len(), 4);
        assert_eq!(send("PUT", format!("/v1/system/users/{owner_id}/system-admin"), &owner, json!({"enabled": true})).await.status(), 403);
        assert_eq!(send("PUT", format!("/v1/system/users/{owner_id}/system-admin"), &admin, json!({"enabled": true})).await.status(), 204);
        assert_eq!(get("/v1/system/overview".to_owned(), &owner).await.status(), 200, "the new role applies at once");
        assert_eq!(send("POST", format!("/v1/system/users/{owner_id}/sessions/end"), &admin, json!({})).await.status(), 204);
        assert_eq!(get("/v1/session".to_owned(), &owner).await.status(), 401);
        assert_eq!(send("POST", format!("/v1/system/users/{owner_id}/unlock"), &admin, json!({})).await.status(), 200);
        for user in users.as_array().unwrap() {
            if user["is_system_admin"] == true && user["email"] != "first@example.com" {
                let id = user["id"].as_str().unwrap();
                assert_eq!(send("PUT", format!("/v1/system/users/{id}/system-admin"), &admin, json!({"enabled": false})).await.status(), 204);
            }
        }
        let first_id = users.as_array().unwrap().iter().find(|user| user["email"] == "first@example.com").unwrap()["id"].as_str().unwrap().to_owned();
        // The owner was promoted after `users` was listed.
        assert_eq!(send("PUT", format!("/v1/system/users/{owner_id}/system-admin"), &admin, json!({"enabled": false})).await.status(), 204);
        let last = send("PUT", format!("/v1/system/users/{first_id}/system-admin"), &admin, json!({"enabled": false})).await;
        assert_eq!(last.status(), 409, "the last system administrator stays");

        // Maintenance by hand, and the audit trail of it all.
        let maintenance = response_json(send("POST", "/v1/system/maintenance/run".to_owned(), &admin, json!({})).await).await;
        assert_eq!(maintenance["last_trigger"], "manual");
        assert_eq!(maintenance["last_report"]["failures"], 0);
        let audit = response_json(get("/v1/system/audit".to_owned(), &admin).await).await;
        let actions = audit.as_array().unwrap().iter().map(|event| event["action"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
        for expected in ["system_admin_granted", "system_settings_changed", "system_setting_reset", "user_sessions_ended", "system_admin_revoked", "maintenance_run"] {
            assert!(actions.iter().any(|action| action == expected), "{expected} missing from {actions:?}");
        }
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn the_admin_console_runs_commands_with_the_pages_rights() {
        let data_dir = temp_data_dir("console");
        let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
        let (admin, _) = register_account(&app, "console-admin@example.com").await;
        let (member, _) = register_account(&app, "console-member@example.com").await;
        let run = |who: &str, command: &str| {
            let app = app.clone();
            let request = json_request("POST", "/v1/system/console", who, json!({ "command": command }));
            async move { app.oneshot(request).await.unwrap() }
        };

        assert_eq!(run(&member, "status").await.status(), 403, "administrators only");
        let welcome = response_json(app.clone().oneshot(auth_request("GET", "/v1/system/console", &admin)).await.unwrap()).await;
        assert!(welcome["banner"].as_str().unwrap().contains("██"));
        assert_eq!(welcome["role"], "system administrator");
        assert!(welcome["commands"].as_array().unwrap().iter().any(|command| command["name"] == "jobs list"));

        let status = response_json(run(&admin, "status").await).await;
        assert_eq!((status["command"].clone(), status["view"]["kind"].clone()), (json!("status"), json!("facts")));
        let jobs = response_json(run(&admin, "jobs").await).await;
        assert_eq!(jobs["command"], "jobs list");
        assert!(jobs["data"].is_array());

        // A change is a dry run until confirmed, then goes through the settings handler.
        let preview = response_json(run(&admin, "settings set log_http_level debug").await).await;
        assert_eq!(preview["dry_run"], true);
        let settings = response_json(run(&admin, "settings list").await).await;
        let http = |settings: &Value| settings["data"].as_array().unwrap().iter().find(|row| row["key"] == "log_http_level").unwrap()["value"].clone();
        assert_eq!(http(&settings), "warn", "a dry run changes nothing");
        let applied = response_json(run(&admin, "settings set log_http_level debug --yes").await).await;
        assert_eq!(applied["dry_run"], false);
        assert_eq!(http(&response_json(run(&admin, "settings").await).await), "debug");
        assert_eq!(run(&admin, "settings set log_http_level loud --yes").await.status(), 400, "the handler's validation applies");
        assert_eq!(run(&admin, "settings set no_such_key 1 --yes").await.status(), 404);

        let unlock = response_json(run(&admin, "users unlock CONSOLE-MEMBER@example.com --yes").await).await;
        assert_eq!(unlock["data"]["cleared"], 0);
        assert_eq!(run(&admin, "users unlock nobody@example.com --yes").await.status(), 404);

        let audit = response_json(app.clone().oneshot(auth_request("GET", "/v1/system/audit", &admin)).await.unwrap()).await;
        let actions = audit.as_array().unwrap().iter().map(|event| event["action"].as_str().unwrap()).collect::<Vec<_>>();
        assert_eq!(actions.iter().filter(|action| **action == "console_command").count(), 2, "{actions:?}");
        assert!(actions.contains(&"system_settings_changed") && actions.contains(&"user_unlocked"), "{actions:?}");

        for (bad, why) in [("", "empty"), ("rm -rf /", "unknown"), ("jobs list --stauts failed", "unknown option"), ("logs --level loud", "level"), ("logs --grep \"open", "quote")] {
            assert_eq!(run(&admin, bad).await.status(), 400, "{why}");
        }
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn system_administrators_control_logging() {
        let data_dir = temp_data_dir("logging");
        let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
        let (admin, _) = register_account(&app, "logs-admin@example.com").await;
        let (member, _) = register_account(&app, "logs-member@example.com").await;
        let send = |method: &'static str, uri: &str, who: &str, body: Value| {
            let app = app.clone();
            let request = json_request(method, uri, who, body);
            async move { app.oneshot(request).await.unwrap() }
        };

        let settings = response_json(send("GET", "/v1/system/settings", &admin, json!({})).await).await;
        let logging = settings["settings"].as_array().unwrap().iter().filter(|setting| setting["group"] == "Logging").collect::<Vec<_>>();
        assert_eq!(logging.len(), 10);
        let http = logging.iter().find(|setting| setting["key"] == "log_http_level").unwrap();
        assert_eq!((http["kind"].clone(), http["value"].clone()), (json!("choice"), json!("warn")));
        assert_eq!(http["options"].as_array().unwrap().len(), 6);

        let invalid = send("PUT", "/v1/system/settings", &admin, json!({"values": {"log_console_format": "xml"}})).await;
        assert_eq!(invalid.status(), 400);
        let changed = send("PUT", "/v1/system/settings", &admin, json!({"values": {"log_http_level": "debug", "log_console_format": "json", "log_retention_days": 7}})).await;
        assert_eq!(changed.status(), 200);
        let files = response_json(send("GET", "/v1/system/logs/files", &admin, json!({})).await).await;
        assert_eq!(files["retention_days"], 7);
        assert_eq!(files["console_format"], "json");
        assert_eq!(files["environment_filter"], false);
        let http = files["categories"].as_array().unwrap().iter().find(|category| category["key"] == "http").unwrap().clone();
        assert_eq!(http["level"], "debug");
        assert_eq!(send("DELETE", "/v1/system/settings/log_http_level", &admin, json!({})).await.status(), 200);
        let files = response_json(send("GET", "/v1/system/logs/files", &admin, json!({})).await).await;
        let http = files["categories"].as_array().unwrap().iter().find(|category| category["key"] == "http").unwrap().clone();
        assert_eq!(http["level"], "warn", "reset restores the default");

        assert_eq!(send("GET", "/v1/system/logs?category=activity&workspace=w&level=info", &admin, json!({})).await.status(), 200);
        assert_eq!(send("GET", "/v1/system/logs?day=2001-01-01", &admin, json!({})).await.status(), 404);
        assert_eq!(send("GET", "/v1/system/logs/files/not-a-day", &admin, json!({})).await.status(), 400);
        assert_eq!(send("GET", "/v1/system/logs/files", &member, json!({})).await.status(), 403);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn a_new_server_is_set_up_once_through_the_onboarding() {
        let data_dir = temp_data_dir("onboarding");
        let mut config = ServerConfig::for_test(data_dir.clone());
        config.setup_wizard = true;
        config.setup_code = Some("ABCD-EFGH-JKLM".to_owned());
        let app = router(config).await.unwrap();
        let public_get = |uri: &str| Request::builder().uri(uri.to_owned()).body(Body::empty()).unwrap();
        let admin_body = |code: &str| json!({"code": code, "email": "root@example.com", "display_name": "Root", "password": "not-a-real-password"});

        let status = response_json(app.clone().oneshot(public_get("/v1/setup")).await.unwrap()).await;
        assert_eq!((status["state"].clone(), status["onboarding"].clone()), (json!("new"), json!(true)));
        let health = response_json(app.clone().oneshot(public_get("/health")).await.unwrap()).await;
        assert_eq!((health["setup_required"].clone(), health["registration_open"].clone()), (json!(true), json!(false)));
        let early = app.clone().oneshot(public_post("/v1/auth/register", json!({"email": "early@example.com", "display_name": "Early", "password": "not-a-real-password"}))).await.unwrap();
        assert_eq!(early.status(), 403, "nobody registers before setup");

        assert_eq!(app.clone().oneshot(public_post("/v1/setup/verify-code", json!({"code": "WRONG-CODE-0000"}))).await.unwrap().status(), 403);
        assert_eq!(app.clone().oneshot(public_post("/v1/setup/verify-code", json!({"code": "abcd efgh jklm"}))).await.unwrap().status(), 204);
        assert_eq!(app.clone().oneshot(public_post("/v1/setup/admin", admin_body("WRONG-CODE-0000"))).await.unwrap().status(), 403);
        let created = app.clone().oneshot(public_post("/v1/setup/admin", admin_body("ABCD-EFGH-JKLM"))).await.unwrap();
        assert_eq!(created.status(), 201);
        let admin = format!("Bearer {}", response_json(created).await["access_token"].as_str().unwrap());
        assert_eq!(app.clone().oneshot(public_post("/v1/setup/admin", admin_body("ABCD-EFGH-JKLM"))).await.unwrap().status(), 409, "one first administrator only");
        let session = response_json(app.clone().oneshot(auth_request("GET", "/v1/session", &admin)).await.unwrap()).await;
        assert_eq!(session["is_system_admin"], true);

        let status = response_json(app.clone().oneshot(public_get("/v1/setup")).await.unwrap()).await;
        assert_eq!(status["state"], "finishing");
        let still_closed = app.clone().oneshot(public_post("/v1/auth/register", json!({"email": "early@example.com", "display_name": "Early", "password": "not-a-real-password"}))).await.unwrap();
        assert_eq!(still_closed.status(), 403, "registration waits for the end of setup");
        assert_eq!(app.clone().oneshot(auth_request("GET", "/v1/setup/checks", "Bearer not-a-token")).await.unwrap().status(), 401);
        let checks = response_json(app.clone().oneshot(auth_request("GET", "/v1/setup/checks", &admin)).await.unwrap()).await;
        assert!(checks.as_array().unwrap().iter().any(|check| check["key"] == "database" && check["status"] == "ok"));

        let done = app.clone().oneshot(auth_request("POST", "/v1/setup/complete", &admin)).await.unwrap();
        assert_eq!(done.status(), 200);
        assert_eq!(response_json(done).await["onboarding"], false);

        // From now on the onboarding is gone for good.
        assert_eq!(app.clone().oneshot(auth_request("POST", "/v1/setup/complete", &admin)).await.unwrap().status(), 409);
        assert_eq!(app.clone().oneshot(auth_request("GET", "/v1/setup/checks", &admin)).await.unwrap().status(), 409);
        assert_eq!(app.clone().oneshot(public_post("/v1/setup/verify-code", json!({"code": "ABCD-EFGH-JKLM"}))).await.unwrap().status(), 409);
        assert_eq!(app.clone().oneshot(public_post("/v1/setup/admin", admin_body("ABCD-EFGH-JKLM"))).await.unwrap().status(), 409);
        let status = response_json(app.clone().oneshot(public_get("/v1/setup")).await.unwrap()).await;
        assert_eq!((status["state"].clone(), status["onboarding"].clone()), (json!("complete"), json!(false)));
        let registered = app.clone().oneshot(public_post("/v1/auth/register", json!({"email": "late@example.com", "display_name": "Late", "password": "not-a-real-password"}))).await.unwrap();
        assert_eq!(registered.status(), 201);
        let audit = response_json(app.clone().oneshot(auth_request("GET", "/v1/system/audit", &admin)).await.unwrap()).await;
        let actions = audit.as_array().unwrap().iter().map(|event| event["action"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
        assert!(actions.contains(&"server_setup_admin_created".to_owned()) && actions.contains(&"server_setup_completed".to_owned()));

        // A restart does not bring the onboarding back.
        drop(app);
        let mut config = ServerConfig::for_test(data_dir.clone());
        config.setup_wizard = true;
        let app = router(config).await.unwrap();
        let status = response_json(app.clone().oneshot(public_get("/v1/setup")).await.unwrap()).await;
        assert_eq!(status["onboarding"], false);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    fn json_request(method: &str, uri: &str, authorization: &str, body: Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", authorization)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }
}
