use super::*;
use std::sync::{Arc, Mutex};

/// A stand-in for GitHub's REST API with two versions of the same two issues
/// (and one pull request, which a connector leaves alone).
#[derive(Clone, Default)]
struct FakeGitHub {
    version: Arc<Mutex<u32>>,
    authorizations: Arc<Mutex<Vec<String>>>,
    queries: Arc<Mutex<Vec<String>>>,
}

async fn start_fake_github(state: FakeGitHub) -> String {
    use axum::{
        extract::{RawQuery, State},
        http::HeaderMap,
        routing::get,
    };
    let app = axum::Router::new()
        .route(
            "/api/v3/repos/acme/app",
            get(|State(state): State<FakeGitHub>, headers: HeaderMap| async move {
                if let Some(value) = headers.get("authorization") {
                    state.authorizations.lock().unwrap().push(value.to_str().unwrap().to_owned());
                }
                axum::Json(json!({"full_name": "acme/app", "open_issues_count": 1, "private": true}))
            }),
        )
        .route(
            "/api/v3/repos/acme/app/issues",
            get(|State(state): State<FakeGitHub>, headers: HeaderMap, RawQuery(query): RawQuery| async move {
                if let Some(value) = headers.get("authorization") {
                    state.authorizations.lock().unwrap().push(value.to_str().unwrap().to_owned());
                }
                state.queries.lock().unwrap().push(query.unwrap_or_default());
                let version = *state.version.lock().unwrap();
                if version == 1 {
                    axum::Json(json!([
                        {"number": 1, "title": "Refunds double post", "state": "open", "comments": 1,
                         "user": {"login": "ana"}, "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-02T00:00:00Z",
                         "html_url": "https://github.com/acme/app/issues/1", "body": "Seen on staging with the ledger.",
                         "labels": [{"name": "bug"}], "assignees": []},
                        {"number": 2, "title": "Old rounding rule", "state": "closed", "comments": 0,
                         "user": {"login": "bo"}, "created_at": "2025-12-01T00:00:00Z", "updated_at": "2026-01-03T00:00:00Z",
                         "closed_at": "2026-01-03T00:00:00Z", "html_url": "https://github.com/acme/app/issues/2",
                         "body": "Round half up.", "labels": [], "assignees": []},
                        {"number": 3, "title": "A pull request", "state": "open", "comments": 0, "pull_request": {"url": "x"},
                         "user": {"login": "cy"}, "created_at": "2026-01-04T00:00:00Z", "updated_at": "2026-01-04T00:00:00Z",
                         "body": "not an issue", "labels": [], "assignees": []}
                    ]))
                } else {
                    axum::Json(json!([
                        {"number": 2, "title": "Old rounding rule", "state": "open", "comments": 0,
                         "user": {"login": "bo"}, "created_at": "2025-12-01T00:00:00Z", "updated_at": "2026-02-01T00:00:00Z",
                         "html_url": "https://github.com/acme/app/issues/2", "body": "Round half up, again.",
                         "labels": [], "assignees": []}
                    ]))
                }
            }),
        )
        .route(
            "/api/v3/repos/acme/app/issues/1/comments",
            get(|| async {
                axum::Json(json!([{"user": {"login": "bo"}, "created_at": "2026-01-02T00:00:00Z", "body": "Reproduced with refund 42."}]))
            }),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

#[tokio::test]
async fn github_issues_become_evidence_that_follows_the_tracker() {
    let fake = FakeGitHub::default();
    *fake.version.lock().unwrap() = 1;
    let base = start_fake_github(fake.clone()).await;
    let link = format!("{base}/acme/app");

    let data_dir = temp_data_dir("connectors");
    // Off until the operator names the hosts a tracker may be on.
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "tracker-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |app: &axum::Router, method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let off = call(
        &app,
        "POST",
        format!("/v1/workspaces/{workspace_id}/connectors/check"),
        owner.clone(),
        json!({"link": link}),
    )
    .await;
    assert_eq!(off.status(), 400);
    assert!(response_json(off).await.to_string().contains("REPOMEMO_GIT_ALLOWED_HOSTS"));
    drop(app);

    let mut config = ServerConfig::for_test(data_dir.clone());
    config.git_allowed_hosts = vec!["127.0.0.1".to_owned()];
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "tracker-owner2@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;

    // A host that is not named, and addresses that are not repositories.
    for (bad, expected) in [
        ("https://gitlab.example.com/acme/app", "only connects trackers"),
        ("https://github.com/acme", "owner and the repository"),
        ("ftp://127.0.0.1/acme/app", "https"),
    ] {
        let refused = call(
            &app,
            "POST",
            format!("/v1/workspaces/{workspace_id}/connectors/check"),
            owner.clone(),
            json!({"link": bad}),
        )
        .await;
        assert_eq!(refused.status(), 400, "{bad}");
        assert!(response_json(refused).await.to_string().contains(expected), "{bad}");
    }

    let checked = response_json(
        call(
            &app,
            "POST",
            format!("/v1/workspaces/{workspace_id}/connectors/check"),
            owner.clone(),
            json!({"link": link, "token": "ghp_trackertoken"}),
        )
        .await,
    )
    .await;
    assert_eq!(checked["name"], "acme/app");
    assert_eq!(checked["private"], true);
    assert_eq!(checked["already_connected"], false);

    let connected = call(
        &app,
        "POST",
        format!("/v1/workspaces/{workspace_id}/connectors"),
        owner.clone(),
        json!({"link": link, "token": "ghp_trackertoken"}),
    )
    .await;
    assert_eq!(connected.status(), 201);
    let connected = response_json(connected).await;
    assert!(!connected.to_string().contains("ghp_trackertoken"));
    let source_id = connected["connector"]["id"].as_str().unwrap().to_owned();
    assert_eq!(connected["connector"]["has_credential"], true);

    // Waits for a finished sync whose report satisfies `done`.
    let wait = |done: fn(&Value) -> bool| {
        let app = app.clone();
        let owner = owner.clone();
        let source_id = source_id.clone();
        async move {
            for _ in 0..150 {
                let connector = response_json(
                    app.clone()
                        .oneshot(auth_request("GET", &format!("/v1/connectors/{source_id}"), &owner))
                        .await
                        .unwrap(),
                )
                .await;
                if connector["status"] == "ready" && connector["active_job"].is_null() && done(&connector) {
                    return connector;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            panic!("the connector did not finish its sync");
        }
    };

    // First sync: two issues, the pull request left alone; the closed one is outdated.
    let first = wait(|connector| connector["last_report"]["added"] == 2).await;
    assert_eq!((first["open_items"].as_i64(), first["closed_items"].as_i64()), (Some(1), Some(1)));
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 2).await;
    let artifacts = response_json(
        call(&app, "GET", format!("/v1/workspaces/{workspace_id}/artifacts"), owner.clone(), json!({})).await,
    )
    .await;
    let artifact = |title_part: &str| {
        artifacts
            .as_array()
            .unwrap()
            .iter()
            .find(|artifact| artifact["title"].as_str().unwrap().contains(title_part))
            .unwrap_or_else(|| panic!("no artifact titled {title_part}: {artifacts}"))
            .clone()
    };
    let double = artifact("Refunds double post");
    let rounding = artifact("Old rounding rule");
    assert_eq!(double["artifact_type"], "issue");
    let detail = response_json(
        call(&app, "GET", format!("/v1/artifacts/{}", double["id"].as_str().unwrap()), owner.clone(), json!({})).await,
    )
    .await;
    assert_eq!(detail["metadata"]["origin"], "github_issue");
    assert_eq!(detail["metadata"]["number"], 1);
    let closed = response_json(
        call(
            &app,
            "GET",
            format!("/v1/artifacts/{}/lifecycle", rounding["id"].as_str().unwrap()),
            owner.clone(),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(closed["status"], "outdated");

    // The token went to the tracker as a bearer token, and nowhere else.
    assert!(!fake.authorizations.lock().unwrap().is_empty());
    assert!(fake
        .authorizations
        .lock()
        .unwrap()
        .iter()
        .all(|value| value == "Bearer ghp_trackertoken"));

    // The tracker changes: the sync asks only for what moved since, and a
    // reopened issue is active again.
    *fake.version.lock().unwrap() = 2;
    let synced = call(&app, "POST", format!("/v1/connectors/{source_id}/sync"), owner.clone(), json!({})).await;
    assert_eq!(synced.status(), 200);
    let second = wait(|connector| connector["last_report"]["updated"] == 1).await;
    assert_eq!((second["open_items"].as_i64(), second["closed_items"].as_i64()), (Some(2), Some(0)));
    assert!(
        fake.queries.lock().unwrap().iter().any(|query| query.contains("since=2026-01-03")),
        "{:?}",
        fake.queries.lock().unwrap()
    );
    let reopened = response_json(
        call(
            &app,
            "GET",
            format!("/v1/artifacts/{}/lifecycle", rounding["id"].as_str().unwrap()),
            owner.clone(),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(reopened["status"], "active");

    // Removing the connector removes what it kept.
    assert_eq!(
        call(&app, "DELETE", format!("/v1/connectors/{source_id}"), owner.clone(), json!({}))
            .await
            .status(),
        204
    );
    let left = response_json(
        call(&app, "GET", format!("/v1/workspaces/{workspace_id}/artifacts"), owner.clone(), json!({})).await,
    )
    .await;
    assert_eq!(left.as_array().unwrap().len(), 0);
    let _ = std::fs::remove_dir_all(data_dir);
}
