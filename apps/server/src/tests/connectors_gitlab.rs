use super::*;
use std::sync::{Arc, Mutex};

/// A stand-in for a self-hosted GitLab's REST API.
#[derive(Clone, Default)]
struct FakeGitLab {
    tokens: Arc<Mutex<Vec<String>>>,
    queries: Arc<Mutex<Vec<String>>>,
}

fn note_token(state: &FakeGitLab, headers: &axum::http::HeaderMap) {
    if let Some(value) = headers.get("private-token") {
        state.tokens.lock().unwrap().push(value.to_str().unwrap().to_owned());
    }
    assert!(
        headers.get("authorization").is_none(),
        "GitLab takes PRIVATE-TOKEN only"
    );
}

async fn start_fake_gitlab(state: FakeGitLab) -> String {
    use axum::{
        extract::{RawQuery, State},
        http::HeaderMap,
        routing::get,
    };
    let app = axum::Router::new()
        .route(
            "/api/v4/projects/acme%2Fapp",
            get(|State(state): State<FakeGitLab>, headers: HeaderMap| async move {
                note_token(&state, &headers);
                axum::Json(json!({"path_with_namespace": "acme/app", "open_issues_count": 1, "visibility": "private"}))
            }),
        )
        .route(
            "/api/v4/projects/acme%2Fapp/issues",
            get(|State(state): State<FakeGitLab>, headers: HeaderMap, RawQuery(query): RawQuery| async move {
                note_token(&state, &headers);
                state.queries.lock().unwrap().push(query.unwrap_or_default());
                axum::Json(json!([
                    {"iid": 5, "title": "Cache stampede", "state": "opened", "author": {"username": "ana"},
                     "created_at": "2026-03-01T00:00:00Z", "updated_at": "2026-03-02T00:00:00Z",
                     "web_url": "https://gitlab.example.com/acme/app/-/issues/5",
                     "description": "Every worker rebuilds the cache at once.", "labels": ["perf"],
                     "assignees": [], "user_notes_count": 2},
                    {"iid": 6, "title": "Old retry rule", "state": "closed", "author": {"username": "bo"},
                     "created_at": "2026-02-01T00:00:00Z", "updated_at": "2026-03-03T00:00:00Z",
                     "closed_at": "2026-03-03T00:00:00Z",
                     "web_url": "https://gitlab.example.com/acme/app/-/issues/6",
                     "description": "Retry three times.", "labels": [], "assignees": [], "user_notes_count": 0}
                ]))
            }),
        )
        .route(
            "/api/v4/projects/acme%2Fapp/issues/5/notes",
            get(|| async {
                axum::Json(json!([
                    {"system": true, "body": "changed the description", "author": {"username": "ana"}, "created_at": "2026-03-01T01:00:00Z"},
                    {"system": false, "body": "Seen under load tests.", "author": {"username": "bo"}, "created_at": "2026-03-01T02:00:00Z"}
                ]))
            }),
        )
        .route(
            "/api/v4/projects/acme%2Fapp/merge_requests",
            get(|| async {
                axum::Json(json!([
                    {"iid": 9, "title": "Add a cache lock", "state": "merged", "author": {"username": "bo"},
                     "created_at": "2026-03-04T00:00:00Z", "updated_at": "2026-03-05T00:00:00Z",
                     "merged_at": "2026-03-05T00:00:00Z", "source_branch": "cache-lock", "target_branch": "main",
                     "web_url": "https://gitlab.example.com/acme/app/-/merge_requests/9",
                     "description": "Only one worker rebuilds.", "labels": [], "assignees": [], "user_notes_count": 2}
                ]))
            }),
        )
        .route(
            "/api/v4/projects/acme%2Fapp/merge_requests/9",
            get(|| async { axum::Json(json!({"iid": 9, "changes_count": "4"})) }),
        )
        .route(
            "/api/v4/projects/acme%2Fapp/merge_requests/9/commits",
            get(|| async {
                axum::Json(json!([{"id": "9f00000000000000000000000000000000000abc", "title": "Lock the cache rebuild", "author_name": "Bo"}]))
            }),
        )
        .route(
            "/api/v4/projects/acme%2Fapp/merge_requests/9/approvals",
            get(|| async { axum::Json(json!({"approved_by": [{"user": {"username": "ana"}}, {"user": {"username": "cy"}}]})) }),
        )
        .route(
            "/api/v4/projects/acme%2Fapp/merge_requests/9/notes",
            get(|| async {
                axum::Json(json!([
                    {"system": false, "type": "DiffNote", "body": "Is the TTL enough?", "author": {"username": "ana"},
                     "created_at": "2026-03-04T05:00:00Z", "position": {"new_path": "src/cache.rs", "new_line": 31}},
                    {"system": false, "body": "Yes, 30s.", "author": {"username": "bo"}, "created_at": "2026-03-04T06:00:00Z"}
                ]))
            }),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

#[tokio::test]
async fn gitlab_issues_and_merge_requests_follow_the_same_model() {
    let fake = FakeGitLab::default();
    let base = start_fake_gitlab(fake.clone()).await;
    let link = format!("{base}/acme/app");

    let data_dir = temp_data_dir("connectors-gitlab");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.git_allowed_hosts = vec!["127.0.0.1".to_owned()];
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "gitlab-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &owner, body);
        async move { app.oneshot(request).await.unwrap() }
    };

    // A loopback address does not say which tracker it is: the person does.
    let undetected = call(
        "POST",
        format!("/v1/workspaces/{workspace_id}/connectors/check"),
        json!({"link": link, "token": "glpat-secret"}),
    )
    .await;
    assert_eq!(
        undetected.status(),
        400,
        "read as GitHub, which this host does not answer to"
    );
    let checked = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/connectors/check"),
            json!({"link": link, "provider": "gitlab", "token": "glpat-secret"}),
        )
        .await,
    )
    .await;
    assert_eq!(checked["provider"], "gitlab");
    assert_eq!(checked["name"], "acme/app");
    assert_eq!(checked["private"], true);

    let connected = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/connectors"),
            json!({"link": link, "provider": "gitlab", "token": "glpat-secret"}),
        )
        .await,
    )
    .await;
    assert!(!connected.to_string().contains("glpat-secret"));
    let source_id = connected["connector"]["id"].as_str().unwrap().to_owned();
    assert_eq!(connected["connector"]["provider"], "gitlab");

    let mut connector = Value::Null;
    for _ in 0..150 {
        connector = response_json(call("GET", format!("/v1/connectors/{source_id}"), json!({})).await).await;
        if connector["status"] == "ready" && connector["active_job"].is_null() && connector["last_report"]["added"] == 3
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(connector["last_report"]["added"], 3, "{connector}");
    assert_eq!(connector["last_report"]["pull_requests_seen"], 1);
    assert_eq!(
        (connector["open_items"].as_i64(), connector["closed_items"].as_i64()),
        (Some(1), Some(2))
    );
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 3).await;
    let artifacts =
        response_json(call("GET", format!("/v1/workspaces/{workspace_id}/artifacts"), json!({})).await).await;
    let artifact = |part: &str| {
        artifacts
            .as_array()
            .unwrap()
            .iter()
            .find(|artifact| artifact["title"].as_str().unwrap().contains(part))
            .unwrap_or_else(|| panic!("no artifact titled {part}: {artifacts}"))
            .clone()
    };
    let stampede = artifact("Cache stampede");
    let retry = artifact("Old retry rule");
    let lock = artifact("Add a cache lock");
    assert_eq!(stampede["artifact_type"], "issue");
    assert_eq!(lock["artifact_type"], "pr");
    let detail = response_json(
        call(
            "GET",
            format!("/v1/artifacts/{}", lock["id"].as_str().unwrap()),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(detail["metadata"]["origin"], "gitlab_merge_request");
    assert_eq!(detail["metadata"]["state"], "merged");
    // GitLab's approvals are read like reviews.
    assert_eq!(detail["metadata"]["approved_by"], json!(["ana", "cy"]));
    assert_eq!(
        detail["metadata"]["commits"][0],
        "9f00000000000000000000000000000000000abc"
    );
    for (item, expected) in [(&retry, "outdated"), (&lock, "active"), (&stampede, "active")] {
        let lifecycle = response_json(
            call(
                "GET",
                format!("/v1/artifacts/{}/lifecycle", item["id"].as_str().unwrap()),
                json!({}),
            )
            .await,
        )
        .await;
        assert_eq!(lifecycle["status"], expected, "{}", item["title"]);
    }

    // The token went in PRIVATE-TOKEN, and nothing else was sent as one.
    assert!(!fake.tokens.lock().unwrap().is_empty());
    assert!(fake.tokens.lock().unwrap().iter().all(|value| value == "glpat-secret"));

    // The next sync asks only for what moved.
    assert_eq!(
        call("POST", format!("/v1/connectors/{source_id}/sync"), json!({}))
            .await
            .status(),
        200
    );
    for _ in 0..150 {
        if fake
            .queries
            .lock()
            .unwrap()
            .iter()
            .any(|query| query.contains("updated_after=2026-03-03"))
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(
        fake.queries
            .lock()
            .unwrap()
            .iter()
            .any(|query| query.contains("updated_after=2026-03-03")),
        "{:?}",
        fake.queries.lock().unwrap()
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
