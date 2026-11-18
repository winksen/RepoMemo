use super::*;
use std::sync::{Arc, Mutex};

/// A tracker whose answers for the repository and for its issues can each be
/// turned into a refusal (`0` answers normally).
#[derive(Clone, Default)]
struct FakeTracker {
    deny_repository: Arc<Mutex<u16>>,
    deny_issues: Arc<Mutex<u16>>,
}

async fn start_fake_tracker(state: FakeTracker) -> String {
    use axum::{
        extract::State,
        http::StatusCode,
        response::{IntoResponse, Response},
        routing::get,
    };
    fn denial(code: u16) -> Response {
        StatusCode::from_u16(code).unwrap().into_response()
    }
    let app = axum::Router::new()
        .route(
            "/api/v3/repos/acme/app",
            get(|State(state): State<FakeTracker>| async move {
                match *state.deny_repository.lock().unwrap() {
                    0 => axum::Json(json!({"full_name": "acme/app", "open_issues_count": 1, "private": true})).into_response(),
                    code => denial(code),
                }
            }),
        )
        .route(
            "/api/v3/repos/acme/app/issues",
            get(|State(state): State<FakeTracker>| async move {
                match *state.deny_issues.lock().unwrap() {
                    0 => axum::Json(json!([
                        {"number": 1, "title": "Zebrafish migration", "state": "open", "comments": 0,
                         "user": {"login": "ana"}, "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-02T00:00:00Z",
                         "html_url": "https://github.com/acme/app/issues/1", "body": "The zebrafish tank needs a new pump.",
                         "labels": [], "assignees": []}
                    ]))
                    .into_response(),
                    code => denial(code),
                }
            }),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

#[tokio::test]
async fn lost_access_holds_back_the_evidence_until_it_is_released() {
    let fake = FakeTracker::default();
    let base = start_fake_tracker(fake.clone()).await;
    let data_dir = temp_data_dir("connector-quarantine");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.git_allowed_hosts = vec!["127.0.0.1".to_owned()];
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "quarantine-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &owner, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let wait_for = |source_id: String, status: &'static str| {
        let app = app.clone();
        let owner = owner.clone();
        async move {
            for _ in 0..150 {
                let connector = response_json(
                    app.clone()
                        .oneshot(auth_request("GET", &format!("/v1/connectors/{source_id}"), &owner))
                        .await
                        .unwrap(),
                )
                .await;
                if connector["status"] == status && connector["active_job"].is_null() {
                    return connector;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            panic!("the connector never became {status}");
        }
    };
    let found = |query: &'static str| {
        let call = &call;
        let workspace_id = workspace_id.clone();
        async move {
            response_json(
                call(
                    "POST",
                    format!("/v1/workspaces/{workspace_id}/search"),
                    json!({"query": query, "limit": 10}),
                )
                .await,
            )
            .await
            .as_array()
            .map(Vec::len)
            .unwrap_or(0)
        }
    };

    let connected = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/connectors"),
            json!({"link": format!("{base}/acme/app"), "token": "ghp_quarantine", "approve_sharing": true}),
        )
        .await,
    )
    .await;
    let source_id = connected["connector"]["id"].as_str().unwrap().to_owned();
    let first = wait_for(source_id.clone(), "ready").await;
    assert!(first["quarantine"].is_null());
    assert!(found("zebrafish").await >= 1, "the copy is searchable");
    let listed = response_json(call("GET", format!("/v1/workspaces/{workspace_id}/artifacts"), json!({})).await).await;
    let artifact_id = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["title"].as_str().unwrap().contains("Zebrafish"))
        .expect("the issue is a file")["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // One refused answer is a failed sync, not a loss: the repository still answers.
    *fake.deny_issues.lock().unwrap() = 403;
    call("POST", format!("/v1/connectors/{source_id}/sync"), json!({})).await;
    let failed = wait_for(source_id.clone(), "error").await;
    assert!(failed["quarantine"].is_null(), "a transient refusal holds nothing back");
    assert!(found("zebrafish").await >= 1);

    // The tracker stops knowing the repository: the second look confirms it.
    *fake.deny_issues.lock().unwrap() = 404;
    *fake.deny_repository.lock().unwrap() = 404;
    call("POST", format!("/v1/connectors/{source_id}/sync"), json!({})).await;
    let held = wait_for(source_id.clone(), "quarantined").await;
    assert_eq!(held["quarantine"]["reason"], "not_found");
    assert_eq!(held["quarantine"]["items"], 1);
    assert!(!held.to_string().contains("ghp_quarantine"));
    assert_eq!(found("zebrafish").await, 0, "held-back evidence is not searchable");
    let listed = response_json(call("GET", format!("/v1/workspaces/{workspace_id}/artifacts"), json!({})).await).await;
    assert!(
        !listed.to_string().contains("Zebrafish"),
        "held-back evidence is not listed: {listed}"
    );
    assert_eq!(
        call("GET", format!("/v1/artifacts/{artifact_id}"), json!({}))
            .await
            .status(),
        404
    );
    let export = call("GET", format!("/v1/workspaces/{workspace_id}/export"), json!({})).await;
    assert_eq!(export.status(), 200);
    assert_eq!(
        call("POST", format!("/v1/connectors/{source_id}/sync"), json!({}))
            .await
            .status(),
        409,
        "nothing is refreshed while it is held back"
    );

    // Releasing needs the access back first, and then serves the evidence again.
    assert_eq!(
        call("POST", format!("/v1/connectors/{source_id}/release"), json!({}))
            .await
            .status(),
        409
    );
    assert_eq!(found("zebrafish").await, 0);
    *fake.deny_issues.lock().unwrap() = 0;
    *fake.deny_repository.lock().unwrap() = 0;
    let released = call("POST", format!("/v1/connectors/{source_id}/release"), json!({})).await;
    assert_eq!(released.status(), 200);
    let released = response_json(released).await;
    assert!(released["quarantine"].is_null());
    assert_eq!(released["status"], "ready");
    assert!(found("zebrafish").await >= 1, "the evidence is served again");
    assert_eq!(
        call("GET", format!("/v1/artifacts/{artifact_id}"), json!({}))
            .await
            .status(),
        200
    );

    // A refused token is a confirmed loss too, and is told apart from a missing repository.
    *fake.deny_issues.lock().unwrap() = 401;
    *fake.deny_repository.lock().unwrap() = 401;
    call("POST", format!("/v1/connectors/{source_id}/sync"), json!({})).await;
    let revoked = wait_for(source_id.clone(), "quarantined").await;
    assert_eq!(revoked["quarantine"]["reason"], "access_revoked");
    assert_eq!(found("zebrafish").await, 0);
    let _ = std::fs::remove_dir_all(data_dir);
}
