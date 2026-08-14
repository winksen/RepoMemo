//! HTTP boundary for server-authoritative RepoMemo workspaces.
//!
//! This first slice deliberately exposes only service discovery and a dummy
//! authenticated session. It establishes a stable client/server boundary while
//! the PostgreSQL, object-storage, and job-store adapters are introduced.

use axum::{
    extract::State,
    http::{header::HeaderName, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use repomemo_domain::{SharedSession, SharedUser, WorkspaceMembership, WorkspaceRole};
use serde::Serialize;

const USER_ID_HEADER: HeaderName = HeaderName::from_static("x-repomemo-user-id");
const USER_NAME_HEADER: HeaderName = HeaderName::from_static("x-repomemo-user-name");

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub service_name: String,
    pub allow_dummy_sessions: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            service_name: "repomemo-server".to_owned(),
            allow_dummy_sessions: true,
        }
    }
}

impl ServerConfig {
    /// Reads the executable's configuration. Dummy sessions require explicit
    /// opt-in so a public deployment cannot accidentally use demo identity.
    pub fn from_env() -> Self {
        Self {
            service_name: std::env::var("REPOMEMO_SERVICE_NAME")
                .unwrap_or_else(|_| "repomemo-server".to_owned()),
            allow_dummy_sessions: std::env::var("REPOMEMO_ALLOW_DUMMY_SESSIONS")
                .is_ok_and(|value| value.eq_ignore_ascii_case("true")),
        }
    }
}

#[derive(Debug, Clone)]
struct AppState {
    config: ServerConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub service: String,
    pub status: &'static str,
    pub mode: &'static str,
}

pub fn router(config: ServerConfig) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/session", get(session))
        .with_state(AppState { config })
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    Json(HealthResponse {
        service: state.config.service_name,
        status: "ok",
        mode: "shared-backend-foundation",
    })
}

async fn session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SharedSession>, StatusCode> {
    if !state.config.allow_dummy_sessions {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let user_id = header_value(&headers, &USER_ID_HEADER).unwrap_or("demo-user");
    let display_name = header_value(&headers, &USER_NAME_HEADER).unwrap_or("Demo User");

    if user_id.trim().is_empty() || display_name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(Json(SharedSession {
        user: SharedUser {
            id: user_id.to_owned(),
            display_name: display_name.to_owned(),
            email: None,
        },
        authentication: "dummy-session".to_owned(),
        memberships: vec![WorkspaceMembership {
            workspace_id: "demo-workspace".to_owned(),
            role: WorkspaceRole::Owner,
        }],
    }))
}

fn header_value<'a>(headers: &'a HeaderMap, name: &HeaderName) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    use super::{router, ServerConfig};

    #[tokio::test]
    async fn exposes_a_dummy_session_for_local_development() {
        let response = router(ServerConfig::default())
            .oneshot(
                Request::builder()
                    .uri("/v1/session")
                    .header("x-repomemo-user-id", "amina")
                    .header("x-repomemo-user-name", "Amina")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), 200);
    }

    #[tokio::test]
    async fn rejects_dummy_identity_without_explicit_development_configuration() {
        let response = router(ServerConfig {
            service_name: "test".to_owned(),
            allow_dummy_sessions: false,
        })
        .oneshot(
            Request::builder()
                .uri("/v1/session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), 401);
    }
}
