//! Server-authoritative HTTP API for shared RepoMemo workspaces.

use std::{net::SocketAddr, path::PathBuf, time::Duration};

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
    ArtifactDetail, ArtifactSummary, AskAnswer, AskRequest, Citation, CreateMemoryCardRequest,
    IndexingJobStatus, MemoryCard, MemoryCardDetail, MemoryCardSummary, Organization,
    ProviderSettings, ProviderTestResult, SearchRequest, SearchResult, SharedAiProviderSettings,
    SharedSession, SharedUser, SharedWorkspace, UpdateMemoryCardRequest, Workspace,
    WorkspaceActivityEvent, WorkspaceAiOverview, WorkspaceCapabilities, WorkspaceMember,
    WorkspaceOverview, WorkspaceRole,
};
use repomemo_storage::{StorageConfig, StorageEngine};
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
        .route("/v1/artifacts/{artifact_id}/index", post(index_artifact))
        .route("/v1/workspaces/{workspace_id}/index", post(index_workspace))
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
    if request.password.len() < 12 || request.password.len() > 1024 {
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
        let search = app.clone().oneshot(json_request("POST", &format!("/v1/workspaces/{workspace_id}/search"), &authorization, json!({"query":"shared data", "artifact_types": [], "languages": [], "source_ids": [], "limit": 20}))).await.unwrap();
        assert_eq!(search.status(), 200);
        let search: Value =
            serde_json::from_slice(&to_bytes(search.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(search.as_array().unwrap().len(), 1);
        assert_eq!(search[0]["artifact_id"], artifact_id);
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
