//! Server-authoritative HTTP API for shared RepoMemo workspaces.

use std::{collections::{BTreeMap, BTreeSet}, net::SocketAddr, path::PathBuf, time::Duration};

use anyhow::{bail, Context, Result};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, FromRequestParts, Path, State},
    http::{header, request::Parts, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::{Duration as ChronoDuration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand_core::OsRng;
use repomemo_api::RepoMemoCore;
use repomemo_domain::{
    ArtifactComment, ArtifactDetail, ArtifactLifecycle, ArtifactLifecycleEvent, ArtifactSummary, ArtifactType, AskAnswer, AskRequest, Citation,
    CollaborationTask, CreateMemoryCardRequest, IndexingJobStatus, MemoryCard, MemoryCardDetail,
    MemoryCardSummary, Organization, ProviderSettings, ProviderTestResult, SearchRequest,
    SearchResult, SharedAiProviderSettings, SharedSession, SharedUser, SharedWorkspace,
    SharedNotification,
    UpdateMemoryCardRequest, Workspace, WorkspaceActivityEvent, WorkspaceAiOverview,
    WorkspaceCapabilities, WorkspaceMember, WorkspaceOverview, WorkspaceRole,
};
use repomemo_storage::{NewCollaborationTask, NewSharedNotification, SaveArtifactLifecycle, StorageConfig, StorageEngine};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

const JWT_ISSUER: &str = "repomemo-server";
const ACCESS_TOKEN_TTL_MINUTES: i64 = 60;
const MAX_SHARED_UPLOAD_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub service_name: String,
    pub bind_address: SocketAddr,
    pub data_dir: PathBuf,
    pub jwt_secret: String,
    pub allowed_origin: Option<HeaderValue>,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self> {
        let jwt_secret = std::env::var("REPOMEMO_JWT_SECRET")
            .context("REPOMEMO_JWT_SECRET is required; use at least 32 random characters")?;
        if jwt_secret.len() < 32 {
            bail!("REPOMEMO_JWT_SECRET must contain at least 32 characters");
        }

        let bind_address = std::env::var("REPOMEMO_SERVER_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8787".to_owned())
            .parse()
            .context("REPOMEMO_SERVER_ADDR must be a valid socket address")?;
        let data_dir = std::env::var("REPOMEMO_SERVER_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".repomemo-server"));
        let allowed_origin = std::env::var("REPOMEMO_ALLOWED_ORIGIN")
            .unwrap_or_else(|_| "http://127.0.0.1:5173".to_owned())
            .parse()
            .context("REPOMEMO_ALLOWED_ORIGIN must be a valid HTTP header value")?;

        Ok(Self {
            service_name: std::env::var("REPOMEMO_SERVICE_NAME")
                .unwrap_or_else(|_| "repomemo-server".to_owned()),
            bind_address,
            data_dir,
            jwt_secret,
            allowed_origin: Some(allowed_origin),
        })
    }

    #[cfg(test)]
    fn for_test(data_dir: PathBuf) -> Self {
        Self {
            service_name: "repomemo-server-test".to_owned(),
            bind_address: "127.0.0.1:0".parse().unwrap(),
            data_dir,
            jwt_secret: "test-secret-that-is-long-enough-for-jwt-signing".to_owned(),
            allowed_origin: None,
        }
    }
}

#[derive(Clone)]
struct AppState {
    storage: StorageEngine,
    core: RepoMemoCore,
    jwt_secret: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub service: String,
    pub status: &'static str,
    pub authentication: &'static str,
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
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
        }
    }

    fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: "A valid bearer token is required.".to_owned(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "conflict",
            message: message.into(),
        }
    }

    fn forbidden() -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: "Your membership does not allow access to this workspace.".to_owned(),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(error = %error, "Unhandled API error");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: "The server could not complete this request.".to_owned(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: ErrorDetail {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response()
    }
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
    token_type: &'static str,
    expires_in: u64,
    user: SharedUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JwtClaims {
    sub: String,
    email: String,
    iss: String,
    iat: usize,
    exp: usize,
}

#[derive(Debug, Clone)]
struct AuthenticatedSubject {
    user_id: String,
}

pub async fn router(config: ServerConfig) -> Result<Router> {
    let storage = StorageEngine::open(StorageConfig {
        data_dir: config.data_dir.clone(),
    })
    .await?;
    let core = RepoMemoCore::boot(config.data_dir).await?;
    let state = AppState {
        storage,
        core,
        jwt_secret: config.jwt_secret,
    };
    let cors = match config.allowed_origin {
        Some(origin) => CorsLayer::new()
            .allow_origin(origin)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers([
                header::AUTHORIZATION,
                header::CONTENT_TYPE,
                HeaderName::from_static("x-repomemo-filename"),
            ]),
        None => CorsLayer::new(),
    };

    Ok(Router::new()
        .route("/health", get(health))
        .route("/v1/auth/register", post(register))
        .route("/v1/auth/login", post(login))
        .route("/v1/session", get(session))
        .route(
            "/v1/profile",
            get(get_profile).put(update_profile),
        )
        .route("/v1/profile/password", post(change_profile_password))
        .route("/v1/profile/tasks", get(list_profile_tasks))
        .route("/v1/notifications", get(list_notifications))
        .route("/v1/notifications/read-all", post(mark_all_notifications_read))
        .route(
            "/v1/notifications/{notification_id}/read",
            post(mark_notification_read),
        )
        .route(
            "/v1/organizations",
            get(list_organizations).post(create_organization),
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
            "/v1/workspaces/{workspace_id}/ai-providers",
            get(list_workspace_ai_providers).put(save_workspace_ai_provider),
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
            "/v1/tasks/{task_id}",
            get(get_collaboration_task)
                .put(update_collaboration_task)
                .delete(delete_collaboration_task),
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
        .route("/v1/workspaces/{workspace_id}/index", post(index_workspace))
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
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(MAX_SHARED_UPLOAD_BYTES))
        .layer(cors)
        .with_state(state))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "repomemo-server".to_owned(),
        status: "ok",
        authentication: "jwt",
    })
}

async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<TokenResponse>), ApiError> {
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
    let password_hash = hash_password(&request.password)?;
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
    let response = issue_token(&state.jwt_secret, user)?;
    Ok((StatusCode::CREATED, Json(response)))
}

async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let account = state
        .storage
        .find_user_for_auth(&request.email)
        .await
        .map_err(|_| invalid_credentials())?;
    let Some(account) = account else {
        return Err(invalid_credentials());
    };
    if !verify_password(&request.password, &account.password_hash) {
        return Err(invalid_credentials());
    }
    state
        .storage
        .touch_user_connection(&account.user.id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(issue_token(&state.jwt_secret, account.user)?))
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

async fn change_profile_password(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<StatusCode, ApiError> {
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
    if !verify_password(&request.current_password, &stored_account.password_hash) {
        return Err(ApiError::bad_request("Current password is incorrect."));
    }
    let password_hash = hash_password(&request.new_password)?;
    state
        .storage
        .update_user_password(&subject.user_id, &password_hash)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
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

async fn list_organizations(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<Organization>>, ApiError> {
    state
        .storage
        .list_organizations_for_user(&subject.user_id)
        .await
        .map(Json)
        .map_err(ApiError::internal)
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

async fn list_workspaces(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<SharedWorkspace>>, ApiError> {
    state
        .storage
        .list_shared_workspaces_for_user(&subject.user_id)
        .await
        .map(Json)
        .map_err(ApiError::internal)
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
        *artifact_types
            .entry(artifact_type.clone())
            .or_default() += 1;
        *artifact_bytes_by_type.entry(artifact_type).or_default() += artifact.size_bytes;
        artifacts_created_last_7_days += i64::from(artifact.created_at >= freshness_threshold);
        artifacts_updated_last_7_days += i64::from(artifact.updated_at >= freshness_threshold);
        if let Some(language) = artifact.language.as_ref().filter(|language| !language.trim().is_empty()) {
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

    Ok(Json(WorkspaceMetricsResponse {
        workspace_id,
        generated_at: Utc::now().to_rfc3339(),
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
    }
}

fn metric_breakdown(values: BTreeMap<String, i64>) -> Vec<WorkspaceMetricBreakdown> {
    let mut values = values
        .into_iter()
        .map(|(label, value)| WorkspaceMetricBreakdown { label, value })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| right.value.cmp(&left.value).then_with(|| left.label.cmp(&right.label)));
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
    Ok(Json(capabilities_for_role(role)))
}

async fn generate_workspace_ai_overview(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceAiOverview>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let provider = state
        .storage
        .list_provider_settings(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .find(|setting| setting.enabled);

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
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let provider = state
        .storage
        .list_provider_settings(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .find(|setting| setting.enabled)
        .ok_or_else(|| {
            ApiError::bad_request(
                "No enabled AI provider is configured for this workspace. An administrator can configure one in Settings.",
            )
        })?;
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
        "Asked a citation-backed question of workspace evidence.".to_owned(),
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
    let api_key = request
        .api_key
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            existing
                .as_ref()
                .and_then(|settings| settings.api_key.clone())
        });
    let cloud_content_acknowledged = request.cloud_content_acknowledged
        || existing
            .as_ref()
            .and_then(|settings| settings.metadata.get("cloud_content_acknowledged"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    let provider = state
        .core
        .save_provider_settings(ProviderSettings {
            id: provider_id,
            workspace_id: Some(workspace_id.clone()),
            provider_type: request.provider_type,
            name: request.name,
            base_url: request.base_url,
            model: request.model,
            embedding_model: None,
            enabled: request.enabled,
            metadata: json!({ "cloud_content_acknowledged": cloud_content_acknowledged }),
            api_key,
        })
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
    Ok(Json(shared_provider_settings(provider)))
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

fn shared_provider_settings(provider: ProviderSettings) -> SharedAiProviderSettings {
    SharedAiProviderSettings {
        id: provider.id,
        provider_type: provider.provider_type,
        name: provider.name,
        base_url: provider.base_url,
        model: provider.model,
        enabled: provider.enabled,
    }
}

async fn list_workspace_activity(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<WorkspaceActivityEvent>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
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
    require_workspace_read(&state, &subject, &workspace_id).await?;
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
        format!("Updated task: {} ({}).", task.title, task.status.replace('_', " ")),
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

async fn normalize_collaboration_task(
    state: &AppState,
    workspace_id: &str,
    request: SaveCollaborationTaskRequest,
) -> Result<NewCollaborationTask, ApiError> {
    let title = request.title.trim().to_owned();
    let description = request.description.trim().to_owned();
    if title.is_empty() || title.len() > 180 {
        return Err(ApiError::bad_request("Task title must be between 1 and 180 characters."));
    }
    if description.len() > 5_000 {
        return Err(ApiError::bad_request("Task description cannot exceed 5,000 characters."));
    }
    if !matches!(request.status.as_str(), "open" | "in_progress" | "blocked" | "done") {
        return Err(ApiError::bad_request("Task status must be open, in_progress, blocked, or done."));
    }
    if !matches!(request.priority.as_str(), "low" | "medium" | "high" | "urgent") {
        return Err(ApiError::bad_request("Task priority must be low, medium, high, or urgent."));
    }
    if let Some(user_id) = request.assignee_user_id.as_deref() {
        if state
            .storage
            .workspace_role_for_user(user_id, workspace_id)
            .await
            .map_err(map_storage_error)?
            .is_none()
        {
            return Err(ApiError::bad_request("Task assignee must be a workspace member."));
        }
    }
    if let Some(artifact_id) = request.artifact_id.as_deref() {
        let artifact = state
            .storage
            .get_artifact(artifact_id)
            .await
            .map_err(map_storage_error)?;
        if artifact.summary.workspace_id != workspace_id {
            return Err(ApiError::bad_request("Task evidence must belong to this workspace."));
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
        return Err(ApiError::bad_request("Comment must be between 1 and 5,000 characters."));
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
                .trim_matches(|character: char| matches!(character, ',' | '.' | ':' | ';' | '!' | '?' | ')' | ']' | '}' | '"' | '\''))
                .strip_prefix('@')
                .map(|email| email.to_ascii_lowercase())
        })
        .collect::<BTreeSet<_>>();
    for member in members {
        let Some(email) = member.user.email.as_deref() else {
            continue;
        };
        if member.user.id == actor_user_id || !mentioned_emails.contains(&email.to_ascii_lowercase()) {
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
    require_workspace_read(&state, &subject, &workspace_id).await?;
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

async fn list_artifacts(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<ArtifactSummary>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .core
        .list_artifacts(workspace_id)
        .await
        .map(Json)
        .map_err(map_core_error)
}

async fn query_artifacts(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<QueryArtifactsRequest>,
) -> Result<Json<Vec<ArtifactSummary>>, ApiError> {
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
        .collect();
    Ok(Json(artifacts))
}

async fn create_text_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<CreateTextArtifactRequest>,
) -> Result<(StatusCode, Json<ArtifactSummary>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
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
    Ok((StatusCode::CREATED, Json(artifact)))
}

async fn upload_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<ArtifactSummary>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let filename = headers
        .get("x-repomemo-filename")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request("X-RepoMemo-Filename is required for uploads."))?
        .to_owned();
    let mime_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let artifact = state
        .core
        .import_upload(workspace_id.clone(), filename, body.to_vec(), mime_type)
        .await
        .map_err(map_core_error)?;
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
    Ok((StatusCode::CREATED, Json(artifact)))
}

async fn get_artifact(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<ArtifactDetail>, ApiError> {
    let artifact = state
        .core
        .get_artifact(artifact_id)
        .await
        .map_err(map_core_error)?;
    require_workspace_read(&state, &subject, &artifact.summary.workspace_id).await?;
    Ok(Json(artifact))
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
    if !matches!(status.as_str(), "active" | "needs_review" | "verified" | "outdated" | "superseded") {
        return Err(ApiError::bad_request("Evidence lifecycle status is invalid."));
    }
    let review_note = request.review_note.trim().to_owned();
    if review_note.len() > 5_000 {
        return Err(ApiError::bad_request("Evidence lifecycle note is too long."));
    }
    let owner_user_id = request.owner_user_id.filter(|value| !value.trim().is_empty());
    if let Some(owner_user_id) = owner_user_id.as_deref() {
        if state
            .storage
            .workspace_role_for_user(owner_user_id, &artifact.summary.workspace_id)
            .await
            .map_err(map_storage_error)?
            .is_none()
        {
            return Err(ApiError::bad_request("Evidence owner must be a workspace member."));
        }
    }
    let superseded_by_artifact_id = request
        .superseded_by_artifact_id
        .filter(|value| !value.trim().is_empty());
    if status == "superseded" && superseded_by_artifact_id.is_none() {
        return Err(ApiError::bad_request("Choose the replacement evidence before marking this item superseded."));
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
            return Err(ApiError::bad_request("Replacement evidence must belong to this workspace."));
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
        format!("Updated evidence lifecycle for {} to {}.", artifact.summary.title, status),
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

    for artifact in artifacts.iter().filter(|artifact| artifact.indexed_at.is_some()) {
        if !artifact_types.contains(&artifact.artifact_type) {
            artifact_types.push(artifact.artifact_type.clone());
        }
        if let Some(language) = artifact.language.as_ref().filter(|language| !language.trim().is_empty()) {
            if !languages.iter().any(|candidate| candidate.eq_ignore_ascii_case(language)) {
                languages.push(language.clone());
            }
        }
        if !sources.iter().any(|source: &RetrievalSourceFacet| source.id == artifact.source_id) {
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
    state
        .storage
        .workspace_role_for_user(&subject.user_id, workspace_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::forbidden)
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

fn capabilities_for_role(role: WorkspaceRole) -> WorkspaceCapabilities {
    let can_write_content = !matches!(&role, WorkspaceRole::Viewer);
    let can_manage_members = matches!(&role, WorkspaceRole::Owner | WorkspaceRole::Admin);
    WorkspaceCapabilities {
        can_read: true,
        can_delete_content: can_write_content,
        can_write_content,
        can_assign_admin: matches!(&role, WorkspaceRole::Owner),
        can_manage_workspace: matches!(&role, WorkspaceRole::Owner),
        can_generate_ai_overview: true,
        can_create_tasks: can_write_content,
        can_comment: can_write_content,
        can_moderate_comments: matches!(&role, WorkspaceRole::Owner | WorkspaceRole::Admin),
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
        Ok(Self {
            user_id: claims.sub,
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

fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(ApiError::internal)
}

fn verify_password(password: &str, stored_hash: &str) -> bool {
    PasswordHash::new(stored_hash)
        .ok()
        .and_then(|hash| {
            Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .ok()
        })
        .is_some()
}

fn issue_token(secret: &str, user: SharedUser) -> Result<TokenResponse, ApiError> {
    let now = Utc::now();
    let expires_at = now + ChronoDuration::minutes(ACCESS_TOKEN_TTL_MINUTES);
    let claims = JwtClaims {
        sub: user.id.clone(),
        email: user.email.clone().unwrap_or_default(),
        iss: JWT_ISSUER.to_owned(),
        iat: now.timestamp() as usize,
        exp: expires_at.timestamp() as usize,
    };
    let access_token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(ApiError::internal)?;
    Ok(TokenResponse {
        access_token,
        token_type: "Bearer",
        expires_in: Duration::from_secs(ACCESS_TOKEN_TTL_MINUTES as u64 * 60).as_secs(),
        user,
    })
}

fn invalid_credentials() -> ApiError {
    ApiError {
        status: StatusCode::UNAUTHORIZED,
        code: "invalid_credentials",
        message: "Email or password is incorrect.".to_owned(),
    }
}

fn map_storage_error(error: anyhow::Error) -> ApiError {
    let message = error.to_string();
    if message.contains("UNIQUE constraint failed") {
        ApiError::conflict("This record already exists.")
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
    if message.contains("was not found")
        || message.contains("is required")
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
    use super::{router, ServerConfig};
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;

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
        assert_eq!(changed_password.status(), 204);

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
        let lifecycle: Value = serde_json::from_slice(
            &to_bytes(lifecycle.into_body(), usize::MAX).await.unwrap(),
        )
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
            serde_json::from_slice(&to_bytes(task.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        let task_id = task["id"].as_str().unwrap();
        assert_eq!(task["assignee"]["display_name"], "Collaborator");
        let task_notifications = app
            .clone()
            .oneshot(auth_request("GET", "/v1/notifications", &collaborator_authorization))
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
        assert_eq!(mention_notifications[0]["notification_type"], "evidence_mention");
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
                    "source_ids":[],
                    "indexed":false
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

        let indexed = app
            .clone()
            .oneshot(auth_request(
                "POST",
                &format!("/v1/artifacts/{artifact_id}/index"),
                &authorization,
            ))
            .await
            .unwrap();
        assert_eq!(indexed.status(), 200);
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
        assert_eq!(metrics["indexed_artifact_count"], 1);
        assert_eq!(metrics["pending_artifact_count"], 1);
        assert_eq!(metrics["member_count"], 2);
        assert_eq!(metrics["completed_task_count"], 1);
        assert_eq!(metrics["comment_count"], 1);
        assert!(metrics["total_artifact_bytes"].as_i64().unwrap() > 0);
        assert!(metrics["indexed_artifact_bytes"].as_i64().unwrap() > 0);
        assert_eq!(metrics["activity_by_day"].as_array().unwrap().len(), 14);
        assert!(!metrics["artifact_bytes_by_type"].as_array().unwrap().is_empty());
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
        assert_eq!(retrieval_facets["artifact_types"], json!(["markdown_doc"]));
        assert_eq!(retrieval_facets["languages"], json!(["Markdown"]));
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

    fn auth_request(method: &str, uri: &str, authorization: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", authorization)
            .body(Body::empty())
            .unwrap()
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
