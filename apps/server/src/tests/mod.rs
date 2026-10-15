// Tests of the HTTP API, by area. Shared helpers live here.

use crate::{router, ServerConfig};
use axum::{
    body::{to_bytes, Body},
    http::Request,
};
use serde_json::{json, Value};
use tower::ServiceExt;

mod ai;
mod auth;
mod connectors;
mod connectors_gitlab;
mod evidence;
mod evidence_tools;
mod memory;
mod operations;
mod performance;
mod system;
mod workspaces;

pub(super) async fn wait_for_indexed_artifacts(
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
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
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

pub(super) fn auth_request(method: &str, uri: &str, authorization: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", authorization)
        .body(Body::empty())
        .unwrap()
}

/// A stand-in for Ollama. Embeddings are 3-dimensional "topic" vectors
/// (shipping, resilience, constant) so meaning can match without shared
/// words; every generate prompt is recorded.
pub(super) async fn start_fake_ollama() -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    use axum::routing::{get, post};
    let prompts = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let topic = |text: &str| {
        let text = text.to_lowercase();
        let has = |words: &[&str]| {
            if words.iter().any(|word| text.contains(word)) {
                1.0
            } else {
                0.0
            }
        };
        json!([
            has(&["deploy", "ship", "release"]),
            has(&["retry", "backoff", "resilien"]),
            0.1
        ])
    };
    let recorded = prompts.clone();
    let app = axum::Router::new()
        .route(
            "/api/tags",
            get(|| async { axum::Json(json!({"models": [{"name": "mock-chat"}, {"name": "mock-embed"}]})) }),
        )
        .route(
            "/api/embed",
            post(move |axum::Json(body): axum::Json<Value>| async move {
                let vectors = body["input"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|text| topic(text.as_str().unwrap()))
                    .collect::<Vec<_>>();
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

pub(super) fn temp_data_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("repomemo-server-{label}-{}", uuid::Uuid::new_v4()))
}

pub(super) async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

pub(super) fn public_post(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Registers an account and returns its bearer header and token response.
pub(super) async fn register_account(app: &axum::Router, email: &str) -> (String, Value) {
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

/// Brings an existing account into a workspace or organization the way
/// people do: `admin` makes an invitation and the account accepts it (the
/// member forms no longer add people who are not in yet, review M4).
pub(super) async fn join(app: &axum::Router, admin: &str, scope: &str, id: &str, email: &str, role: &str) {
    let invitation = response_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/{scope}/{id}/invitations"),
                admin,
                json!({"email": email, "role": role}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let token = invitation["token"].as_str().unwrap().to_owned();
    let session = response_json(
        app.clone()
            .oneshot(public_post(
                "/v1/auth/login",
                json!({"email": email, "password": "not-a-real-password"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let bearer = format!("Bearer {}", session["access_token"].as_str().unwrap());
    let accepted = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/invitations/accept",
            &bearer,
            json!({"token": token}),
        ))
        .await
        .unwrap();
    assert_eq!(accepted.status(), 200, "{email} accepts the invitation");
}

/// Creates an organization and a workspace owned by `owner`.
pub(super) async fn create_workspace(app: &axum::Router, owner: &str) -> String {
    let organization = response_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/organizations",
                owner,
                json!({"name": "Team"}),
            ))
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

pub(super) fn json_request(method: &str, uri: &str, authorization: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", authorization)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
