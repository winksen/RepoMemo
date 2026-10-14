//! Server-authoritative HTTP API for shared RepoMemo workspaces.

mod access;
mod access_control;
mod agent;
mod ai;
mod ai_access;
mod artifacts;
mod auth;
mod avatars;
mod backup;
mod collaboration;
mod config;
mod conversion;
mod derived;
mod embedding;
mod environment;
mod errors;
mod events;
mod health;
mod indexing;
mod logs;
mod mail;
mod maintenance;
mod organizations;
mod profile;
mod quotas;
mod repositories;
mod requests;
mod retrieval;
mod security;
mod serve;
mod setup;
mod streams;
mod system;
mod web_session;
mod workspaces;

use access_control::*;
use ai::*;
use artifacts::*;
use auth::*;
use collaboration::*;
pub use config::*;
use errors::*;
use organizations::*;
use profile::*;
use requests::*;
use retrieval::*;
use streams::*;
use workspaces::*;

pub use logs::{init_logging, log_capture_layer, LogSettings};

pub use maintenance::MaintenanceSettings;
pub use security::Quota;
pub use serve::serve;

use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    net::{IpAddr, SocketAddr},
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
use chrono::{Duration as ChronoDuration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand_core::OsRng;
use repomemo_api::{served_mime_type, RepoLinkPolicy, RepoMemoCore};
use repomemo_domain::{
    ArtifactComment, ArtifactDetail, ArtifactIndexFailure, ArtifactLifecycle, ArtifactLifecycleEvent, ArtifactSummary,
    ArtifactType, AskAnswer, AskRequest, Chunk, Citation, CollaborationTask, CreateMemoryCardRequest, DocumentPreview,
    ErrorKind, Folder, IndexingJobStatus, KnowledgeMap, MemoryCard, MemoryCardDetail, MemoryCardSummary, Organization,
    OrganizationMember, OrganizationRole, ProviderSettings, ProviderTestResult, SavedSearch, SearchRequest,
    SearchResult, SharedAiProviderSettings, SharedNotification, SharedSession, SharedUser, SharedWorkspace,
    TaskChecklistItem, UpdateMemoryCardRequest, Workspace, WorkspaceActivityEvent, WorkspaceAiOverview,
    WorkspaceCapabilities, WorkspaceMember, WorkspaceOverview, WorkspaceRole,
};
use repomemo_storage::{
    ArtifactQuery, NewCollaborationTask, NewSavedSearch, NewSharedNotification, NewTaskChecklistItem,
    SaveArtifactLifecycle, StorageConfig, StorageEngine,
};
use tokio_stream::{
    wrappers::{errors::BroadcastStreamRecvError, BroadcastStream, ReceiverStream},
    Stream, StreamExt,
};

use crate::conversion::{Converter, RenderState};
use crate::embedding::EmbeddingQueue;
use crate::events::{BusActivityObserver, BusJobObserver, WorkspaceEventBus};
use crate::indexing::IndexQueue;
use crate::repositories::RepoSyncRunner;
use crate::security::{consume_heavy_quota, too_many_requests, wait_text, ClientIp, Guards};
use crate::system::{InstanceInfo, MaintenanceStatus, SettingsCell, Telemetry};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::{
    catch_panic::CatchPanicLayer,
    cors::{AllowOrigin, CorsLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

const JWT_ISSUER: &str = "repomemo-server";
/// Longest file name accepted for an upload.
const MAX_UPLOAD_FILENAME_CHARS: usize = 255;

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
    /// Outgoing mail: the outbox and its sender.
    mail: mail::MailService,
    runtime: Arc<SettingsCell>,
    guards: Arc<Guards>,
    /// Knowledge maps and health reports, kept while a workspace is unchanged.
    derived: Arc<derived::DerivedCache>,
    instance: Arc<InstanceInfo>,
    telemetry: Arc<Telemetry>,
    maintenance_status: Arc<std::sync::Mutex<MaintenanceStatus>>,
    /// The backup or restore that is running, or what the last one did.
    backup_status: Arc<std::sync::Mutex<backup::BackupStatus>>,
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
    /// The server still has to be set up through the onboarding.
    pub setup_required: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadinessResponse {
    pub status: &'static str,
    pub database: &'static str,
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
        read_connections: config.db_read_connections,
        vector_index_mb: config.vector_index_mb,
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

    // Only accounts that exist now are promoted; a listed address registered
    // later is not (see `register`).
    let promoted = storage.promote_system_admins(&config.system_admin_emails).await?;
    for email in &config.system_admin_emails {
        if matches!(storage.find_user_by_email(email).await, Ok(None)) {
            tracing::warn!(target: "audit", email = %email, "REPOMEMO_SYSTEM_ADMIN_EMAILS lists an address with no account. Registering it will not make it a system administrator; grant the role in System › Users once the person has an account.");
        }
    }
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

    let built_in_hosts = config.ai_allowed_hosts.as_deref().is_some_and(|hosts| {
        hosts.len() == repomemo_ai::DEFAULT_ALLOWED_HOSTS.len()
            && repomemo_ai::DEFAULT_ALLOWED_HOSTS
                .iter()
                .all(|host| hosts.iter().any(|listed| listed == host))
    });
    if !repomemo_ai::set_endpoint_policy(repomemo_ai::EndpointPolicy {
        allowed_hosts: config.ai_allowed_hosts.clone(),
        // The built-in list stands for the public OpenRouter; an operator's own list may name internal services.
        public_hostnames_only: built_in_hosts,
    }) {
        tracing::debug!("The AI endpoint policy was already set in this process");
    }
    repomemo_api::set_document_time_limit(config.document_time_limit);
    // Git runs on folders other people chose: isolate it from the service
    // account's own configuration (review L8). The configured repository
    // folders, and the ones the operator lists, are the trusted ones.
    if !repomemo_git::harden(repomemo_git::Hardening {
        safe_directories: config
            .repo_roots
            .iter()
            .chain(config.git_safe_directories.iter())
            .map(|folder| folder.to_string_lossy().replace('\\', "/"))
            .collect(),
    }) {
        tracing::debug!("Git was already hardened in this process");
    }
    let repo_policy = if config.repo_roots.is_empty() {
        tracing::info!("Repository links are turned off: set REPOMEMO_REPO_ROOTS to the folders that hold repositories to turn them on");
        RepoLinkPolicy::disabled()
    } else {
        let policy = RepoLinkPolicy::with_allowed_roots(config.repo_roots.clone()).context("REPOMEMO_REPO_ROOTS")?;
        tracing::info!(roots = ?policy.allowed_roots(), "Repository links are restricted to these folders");
        policy
    };

    // Share the same storage handle with the API core so job and activity
    // observers registered here also fire for writes that happen inside
    // `RepoMemoCore`. Without this the two halves would each hold their own
    // pool and SSE would only see writes the server layer performed directly.
    let core = RepoMemoCore::from_storage(storage.clone()).with_repo_link_policy(repo_policy);
    runtime.share_background_limit(core.background_ai_budget().limit_handle());

    let event_bus = WorkspaceEventBus::new();
    storage.set_job_observer(Arc::new(BusJobObserver::new(event_bus.clone())));
    storage.set_activity_observer(Arc::new(BusActivityObserver::new(event_bus.clone())));

    let embedding_queue = EmbeddingQueue::start(core.clone(), config.embedding_concurrency);
    let index_queue = IndexQueue::start(core.clone(), embedding_queue.clone(), config.index_concurrency);
    index_queue.resume_pending().await;
    embedding_queue.resume_pending().await;
    let repo_sync = RepoSyncRunner::new(core.clone(), embedding_queue.clone(), runtime.clone());
    repo_sync.resume_all().await;
    let polling = runtime.clone();
    let polling_task = repo_sync.start_polling(move || polling.current().repo_poll_interval);

    let converter = match config.soffice.clone() {
        Some(path) => Converter::with_options(&data_dir, Some(path), config.soffice_wrapper.clone()),
        None => Converter::detect(&data_dir, config.soffice_wrapper.clone()),
    };
    if converter.enabled() {
        tracing::info!(
            sandboxed = converter.sandboxed(),
            "LibreOffice found: Office files get layout-accurate previews"
        );
        // Not in tests: every test server would start LibreOffice.
        if !cfg!(test) {
            let probing = converter.clone();
            tokio::spawn(async move { probing.probe_version().await });
        }
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
            public_url: config.public_url.clone(),
            data_dir: data_dir.clone(),
        },
    ));
    let (mail, mail_task) = mail::MailService::start(
        storage.clone(),
        repomemo_mail::OutboundPolicy {
            allowed_hosts: config.smtp_allowed_hosts.clone(),
            extra_roots: config.smtp_extra_roots.clone(),
        },
    );
    let state = AppState {
        storage,
        core,
        jwt_secret: config.jwt_secret.clone(),
        event_bus,
        index_queue,
        embedding_queue,
        converter,
        repo_sync,
        mail,
        runtime,
        guards: Arc::new(Guards::default()),
        derived: Arc::new(derived::DerivedCache::default()),
        instance,
        telemetry: Arc::new(Telemetry::default()),
        maintenance_status: Arc::new(std::sync::Mutex::new(MaintenanceStatus::default())),
        backup_status: Arc::new(std::sync::Mutex::new(backup::BackupStatus::default())),
        setup,
        host,
        closing: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };
    let maintenance_task = maintenance::start(state.clone());
    Ok((state, vec![polling_task, maintenance_task, mail_task]))
}

/// Cross-origin rules for the whole application, platform routes included.
fn cors_layer(config: &ServerConfig) -> CorsLayer {
    if config.allowed_origins.is_empty() {
        CorsLayer::new()
    } else {
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(config.allowed_origins.clone()))
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
            .allow_headers([
                header::AUTHORIZATION,
                header::CONTENT_TYPE,
                HeaderName::from_static("x-repomemo-filename"),
                HeaderName::from_static("x-repomemo-folder-id"),
                HeaderName::from_static("x-repomemo-client"),
            ])
            .expose_headers([HeaderName::from_static("x-total-count"), header::RETRY_AFTER])
            // The web app's refresh cookie crosses ports of the same site.
            .allow_credentials(true)
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
        .route("/v1/auth/password-reset/preview", post(access::preview_password_reset))
        .route("/v1/auth/password-reset", post(access::complete_password_reset))
        .route("/v1/invitations/preview", post(access::preview_invitation))
        .route("/v1/invitations/accept", post(access::accept_invitation))
        .route("/v1/invitations/{invitation_id}", delete(access::revoke_invitation))
        .route(
            "/v1/profile/tokens",
            get(access::list_tokens).post(access::create_token),
        )
        .route("/v1/profile/tokens/{token_id}", delete(access::revoke_token))
        .route("/v1/system/overview", get(system::overview))
        .route("/v1/system/usage", get(system::usage))
        .route("/v1/system/users", get(system::list_users))
        .route("/v1/system/users/{user_id}/system-admin", put(system::set_system_admin))
        .route("/v1/system/users/{user_id}/app-admin", put(system::set_app_admin))
        .route(
            "/v1/system/users/{user_id}/sessions/end",
            post(system::end_user_sessions),
        )
        .route("/v1/system/users/{user_id}/unlock", post(system::unlock_user))
        .route(
            "/v1/system/users/{user_id}/password-reset",
            post(access::issue_password_reset),
        )
        .route(
            "/v1/system/settings",
            get(system::get_settings).put(system::update_settings),
        )
        .route("/v1/system/settings/{key}", delete(system::reset_setting))
        .route("/v1/system/environment/detach", post(system::detach_environment))
        .route("/v1/system/logs", get(system::logs))
        .route("/v1/system/logs/files", get(system::log_files))
        .route("/v1/system/logs/files/{day}", get(system::download_log_file))
        .route("/v1/system/audit", get(system::audit_events))
        .route("/v1/system/jobs", get(system::jobs))
        .route("/v1/system/maintenance", get(system::get_maintenance))
        .route("/v1/system/maintenance/run", post(system::run_maintenance))
        .route("/v1/system/backups", get(backup::list).post(backup::create))
        .route("/v1/system/backups/{name}", delete(backup::delete))
        .route("/v1/system/backups/{name}/restore", post(backup::restore))
        .route("/v1/system/metrics", get(system::metrics))
        .route("/v1/system/mail", get(mail::get_mail).put(mail::put_mail))
        .route("/v1/system/mail/outbox", get(mail::list_outbox))
        .route("/v1/system/mail/test", post(mail::test_mail))
        .route(
            "/v1/system/console",
            get(system::console::welcome).post(system::console::run),
        )
        .route("/v1/session", get(session))
        .route("/v1/profile", get(get_profile).put(update_profile))
        .route("/v1/profile/password", post(change_profile_password))
        .route("/v1/profile/tasks", get(list_profile_tasks))
        .merge(avatars::routes())
        .route("/v1/notifications", get(list_notifications))
        .route("/v1/notifications/read-all", post(mark_all_notifications_read))
        .route("/v1/notifications/{notification_id}/read", post(mark_notification_read))
        .route("/v1/organizations", get(list_organizations).post(create_organization))
        .route("/v1/organizations/{organization_id}", put(update_organization))
        .route(
            "/v1/organizations/{organization_id}/members",
            get(list_organization_members).put(upsert_organization_member),
        )
        .route(
            "/v1/organizations/{organization_id}/invitations",
            get(access::list_organization_invitations).post(access::create_organization_invitation),
        )
        .route(
            "/v1/organizations/{organization_id}/members/{user_id}",
            delete(remove_organization_member),
        )
        .route("/v1/dashboard", get(dashboard))
        .route("/v1/workspaces", get(list_workspaces).post(create_workspace))
        .route(
            "/v1/workspaces/{workspace_id}",
            put(update_workspace).delete(delete_workspace),
        )
        .route("/v1/workspaces/{workspace_id}/overview", get(workspace_overview))
        .route("/v1/workspaces/{workspace_id}/metrics", get(workspace_metrics))
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
        .route("/v1/workspaces/{workspace_id}/health", get(health::workspace_health))
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
        .route("/v1/workspaces/{workspace_id}/activity", get(list_workspace_activity))
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
        .route("/v1/saved-searches/{search_id}", delete(delete_saved_search))
        .route(
            "/v1/workspaces/{workspace_id}/members",
            get(list_workspace_members).put(upsert_workspace_member),
        )
        .route(
            "/v1/workspaces/{workspace_id}/invitations",
            get(access::list_workspace_invitations).post(access::create_workspace_invitation),
        )
        .route(
            "/v1/workspaces/{workspace_id}/members/{user_id}",
            delete(remove_workspace_member),
        )
        .route("/v1/workspaces/{workspace_id}/artifacts", get(list_artifacts))
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
        .route("/v1/artifacts/{artifact_id}/open-link", post(create_file_link))
        .route("/v1/shared-files/{token}/{filename}", get(shared_file_by_link))
        .route(
            "/v1/workspaces/{workspace_id}/artifacts/index-failures",
            get(list_artifact_index_failures),
        )
        .route("/v1/workspaces/{workspace_id}/artifacts/query", post(query_artifacts))
        .route(
            "/v1/workspaces/{workspace_id}/artifacts/text",
            post(create_text_artifact),
        )
        .route("/v1/workspaces/{workspace_id}/artifacts/upload", post(upload_artifact))
        .route(
            "/v1/artifacts/{artifact_id}",
            get(get_artifact).put(update_artifact).delete(delete_artifact),
        )
        .route("/v1/artifacts/{artifact_id}/chunks", get(list_artifact_chunks))
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
        .route(
            "/v1/repositories/{source_id}/files",
            get(repositories::list_repository_files),
        )
        .route(
            "/v1/repositories/{source_id}/detail",
            get(repositories::get_repository_detail),
        )
        .route(
            "/v1/repositories/{source_id}/summary",
            post(repositories::summarize_repository),
        )
        .route("/v1/workspaces/{workspace_id}/index", post(index_workspace))
        .route("/v1/workspaces/{workspace_id}/jobs", get(list_workspace_jobs))
        .route("/v1/jobs/{job_id}", get(get_job))
        .route("/v1/jobs/{job_id}/cancel", post(cancel_job))
        .route(
            "/v1/workspaces/{workspace_id}/retrieval-facets",
            get(get_retrieval_facets),
        )
        .route("/v1/workspaces/{workspace_id}/search", post(search_workspace))
        .route("/v1/workspaces/{workspace_id}/artifacts/bulk", post(bulk_artifacts))
        .route(
            "/v1/workspaces/{workspace_id}/folders/{folder_id}/move",
            post(move_folder),
        )
        .route("/v1/artifacts/{artifact_id}/versions", post(upload_artifact_version))
        .route(
            "/v1/workspaces/{workspace_id}/transfer-ownership",
            post(transfer_workspace_ownership),
        )
        .route(
            "/v1/organizations/{organization_id}/transfer-ownership",
            post(transfer_organization_ownership),
        )
        .route("/v1/workspaces/{workspace_id}/symbols", get(search_symbols))
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
            get(get_memory_card).put(update_memory_card).delete(delete_memory_card),
        )
        .route("/v1/memory-cards/{card_id}/export", get(export_memory_card))
        // Every route above answers within the request timeout (review N7).
        // Event streams are added below it: they are meant to stay open.
        .route_layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            if config.request_timeout.is_zero() {
                Duration::from_secs(24 * 60 * 60)
            } else {
                config.request_timeout
            },
        ))
        .route("/v1/workspaces/{workspace_id}/events", get(workspace_events))
        .route("/v1/system/events", get(system_events))
        .layer(middleware::from_fn_with_state(state.clone(), web_session::web_session))
        .layer(CatchPanicLayer::custom(panic_response))
        .layer(middleware::from_fn_with_state(
            security::HeaderPolicy {
                hsts: config
                    .public_url
                    .as_deref()
                    .is_some_and(|url| url.starts_with("https://")),
            },
            security::security_headers,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), system::track_requests))
        .layer(TraceLayer::new_for_http().make_span_with(security::redacted_span))
        .layer(DefaultBodyLimit::max(config.max_upload_bytes))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        service: state.settings().service_name.clone(),
        status: "ok",
        authentication: "jwt",
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

/// Whether an error says that what was asked for does not exist.
/// Records how long a workspace operation took and how much it handled,
/// under the `timing` target: System › Settings › Logging › Performance, at
/// debug. These numbers decide the performance work (roadmap P1–P5).
pub(crate) fn record_timing(operation: &'static str, workspace_id: &str, started: std::time::Instant, items: usize) {
    let elapsed_ms = started.elapsed().as_millis() as u64;
    tracing::debug!(target: "timing", operation, workspace_id = %workspace_id, elapsed_ms, items, "{operation} took {elapsed_ms} ms");
}

#[cfg(test)]
mod tests;
