use super::*;
use std::sync::{Arc, Mutex};

/// A stand-in for GitHub's REST API with two versions of the same two issues
/// and one pull request (open, then merged).
#[derive(Clone, Default)]
struct FakeGitHub {
    version: Arc<Mutex<u32>>,
    authorizations: Arc<Mutex<Vec<String>>>,
    queries: Arc<Mutex<Vec<String>>>,
    /// What the issues list answers instead of its data: 0 nothing special,
    /// 1 a short rate limit for the next `limited_calls` calls, 2 a long rate
    /// limit, 3 a refused token.
    mode: Arc<Mutex<u32>>,
    limited_calls: Arc<Mutex<u32>>,
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
                         "html_url": "https://github.com/acme/app/issues/1", "body": "Seen on staging: the bug is in src/ledger.rs, where `post_refund` posts twice.",
                         "labels": [{"name": "bug"}], "assignees": []},
                        {"number": 2, "title": "Old rounding rule", "state": "closed", "comments": 0,
                         "user": {"login": "bo"}, "created_at": "2025-12-01T00:00:00Z", "updated_at": "2026-01-03T00:00:00Z",
                         "closed_at": "2026-01-03T00:00:00Z", "html_url": "https://github.com/acme/app/issues/2",
                         "body": "Round half up.", "labels": [], "assignees": []},
                        {"number": 3, "title": "Guard the ledger", "state": "open", "comments": 1, "pull_request": {"url": "x"},
                         "user": {"login": "cy"}, "created_at": "2026-01-04T00:00:00Z", "updated_at": "2026-01-04T00:00:00Z",
                         "html_url": "https://github.com/acme/app/pull/3", "body": "Posts a refund once. Fixes #1.",
                         "labels": [], "assignees": []}
                    ]))
                } else {
                    axum::Json(json!([
                        {"number": 2, "title": "Old rounding rule", "state": "open", "comments": 0,
                         "user": {"login": "bo"}, "created_at": "2025-12-01T00:00:00Z", "updated_at": "2026-02-01T00:00:00Z",
                         "html_url": "https://github.com/acme/app/issues/2", "body": "Round half up, again.",
                         "labels": [], "assignees": []},
                        {"number": 3, "title": "Guard the ledger", "state": "closed", "comments": 1, "pull_request": {"url": "x"},
                         "user": {"login": "cy"}, "created_at": "2026-01-04T00:00:00Z", "updated_at": "2026-02-02T00:00:00Z",
                         "html_url": "https://github.com/acme/app/pull/3", "body": "Posts a refund once. Fixes #1.",
                         "labels": [], "assignees": []}
                    ]))
                }
            }),
        )
        .route(
            "/api/v3/repos/acme/app/pulls/3",
            get(|State(state): State<FakeGitHub>| async move {
                let merged = *state.version.lock().unwrap() != 1;
                axum::Json(json!({"merged": merged, "merged_at": if merged { json!("2026-02-02T00:00:00Z") } else { Value::Null },
                    "base": {"ref": "main"}, "head": {"ref": "guard-ledger"},
                    "commits": 1, "changed_files": 2, "additions": 12, "deletions": 1}))
            }),
        )
        .route(
            "/api/v3/repos/acme/app/pulls/3/commits",
            get(|| async {
                axum::Json(json!([{"sha": "abc1230000000000000000000000000000000def",
                    "commit": {"message": "Guard the ledger against double posts", "author": {"name": "Cy"}}}]))
            }),
        )
        .route(
            "/api/v3/repos/acme/app/pulls/3/reviews",
            get(|| async {
                axum::Json(json!([{"user": {"login": "ana"}, "state": "APPROVED", "submitted_at": "2026-01-05T00:00:00Z", "body": "Looks right."}]))
            }),
        )
        .route(
            "/api/v3/repos/acme/app/pulls/3/comments",
            get(|| async {
                axum::Json(json!([{"user": {"login": "ana"}, "path": "src/ledger.rs", "line": 7,
                    "created_at": "2026-01-05T00:00:00Z", "body": "Idempotency key?"}]))
            }),
        )
        .route(
            "/api/v3/repos/acme/app/issues/3/comments",
            get(|| async {
                axum::Json(json!([{"user": {"login": "cy"}, "created_at": "2026-01-05T01:00:00Z", "body": "Added one."}]))
            }),
        )
        .route(
            "/api/v3/repos/acme/app/issues/1/comments",
            get(|| async {
                axum::Json(json!([{"user": {"login": "bo"}, "created_at": "2026-01-02T00:00:00Z", "body": "Reproduced with refund 42."}]))
            }),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            |State(state): State<FakeGitHub>, request: axum::extract::Request, next: axum::middleware::Next| async move {
                use axum::response::IntoResponse;
                if request.uri().path().ends_with("/issues") {
                    let mode = *state.mode.lock().unwrap();
                    let answer = |status: u16, retry_after: Option<&str>| {
                        let mut response = (axum::http::StatusCode::from_u16(status).unwrap(), "{}").into_response();
                        if let Some(value) = retry_after {
                            response.headers_mut().insert("retry-after", value.parse().unwrap());
                        }
                        response
                    };
                    match mode {
                        1 => {
                            let mut left = state.limited_calls.lock().unwrap();
                            if *left > 0 {
                                *left -= 1;
                                return answer(429, Some("1"));
                            }
                        }
                        2 => return answer(429, Some("3600")),
                        3 => return answer(401, None),
                        _ => {}
                    }
                }
                next.run(request).await
            },
        ))
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
    assert!(response_json(off)
        .await
        .to_string()
        .contains("REPOMEMO_GIT_ALLOWED_HOSTS"));
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

    // First sync: two issues and a pull request; the closed issue is outdated.
    let first = wait(|connector| connector["last_report"]["added"] == 3).await;
    assert_eq!(first["last_report"]["pull_requests_seen"], 1);
    assert_eq!(
        (first["open_items"].as_i64(), first["closed_items"].as_i64()),
        (Some(2), Some(1))
    );
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 3).await;
    let artifacts = response_json(
        call(
            &app,
            "GET",
            format!("/v1/workspaces/{workspace_id}/artifacts"),
            owner.clone(),
            json!({}),
        )
        .await,
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
    let pull = artifact("Guard the ledger");
    assert_eq!(double["artifact_type"], "issue");
    assert_eq!(pull["artifact_type"], "pr");
    let pull_detail = response_json(
        call(
            &app,
            "GET",
            format!("/v1/artifacts/{}", pull["id"].as_str().unwrap()),
            owner.clone(),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(pull_detail["metadata"]["origin"], "github_pull_request");
    assert_eq!(pull_detail["metadata"]["state"], "open");
    // The reviews say who approved it, in the data and in the text.
    assert_eq!(pull_detail["metadata"]["approved_by"], json!(["ana"]));
    assert_eq!(pull_detail["metadata"]["changes_requested_by"], json!([]));
    assert_eq!(
        pull_detail["metadata"]["commits"][0],
        "abc1230000000000000000000000000000000def"
    );
    let detail = response_json(
        call(
            &app,
            "GET",
            format!("/v1/artifacts/{}", double["id"].as_str().unwrap()),
            owner.clone(),
            json!({}),
        )
        .await,
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
    let synced = call(
        &app,
        "POST",
        format!("/v1/connectors/{source_id}/sync"),
        owner.clone(),
        json!({}),
    )
    .await;
    assert_eq!(synced.status(), 200);
    let second = wait(|connector| connector["last_report"]["updated"] == 2).await;
    // The reopened issue is open; the merged pull request counts as closed but stays active.
    assert_eq!(
        (second["open_items"].as_i64(), second["closed_items"].as_i64()),
        (Some(2), Some(1))
    );
    let merged = response_json(
        call(
            &app,
            "GET",
            format!("/v1/artifacts/{}", pull["id"].as_str().unwrap()),
            owner.clone(),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(merged["metadata"]["state"], "merged");
    let merged_lifecycle = response_json(
        call(
            &app,
            "GET",
            format!("/v1/artifacts/{}/lifecycle", pull["id"].as_str().unwrap()),
            owner.clone(),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(merged_lifecycle["status"], "active");
    assert!(
        fake.queries
            .lock()
            .unwrap()
            .iter()
            .any(|query| query.contains("since=2026-01-04")),
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
        call(
            &app,
            "DELETE",
            format!("/v1/connectors/{source_id}"),
            owner.clone(),
            json!({})
        )
        .await
        .status(),
        204
    );
    let left = response_json(
        call(
            &app,
            "GET",
            format!("/v1/workspaces/{workspace_id}/artifacts"),
            owner.clone(),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(left.as_array().unwrap().len(), 0);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_file_lists_the_issues_that_name_it() {
    let fake = FakeGitHub::default();
    *fake.version.lock().unwrap() = 1;
    let base = start_fake_github(fake.clone()).await;

    // A repository with the file and symbol the first issue names.
    let root = std::env::temp_dir().join(format!("repomemo-server-mentions-{}", uuid::Uuid::new_v4()));
    let repo_dir = root.join("repos").join("app");
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
    std::fs::write(
        repo_dir.join("src/ledger.rs"),
        "pub fn post_refund(amount: u64) -> u64 {\n    amount\n}\n",
    )
    .unwrap();
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "Initial"]);

    let mut config = ServerConfig::for_test(root.join("data"));
    config.git_allowed_hosts = vec!["127.0.0.1".to_owned()];
    config.repo_roots = vec![root.join("repos")];
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "mention-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &owner, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let repository = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/repositories"),
            json!({"link": repo_dir.display().to_string()}),
        )
        .await,
    )
    .await;
    let repository_id = repository["repository"]["id"].as_str().unwrap().to_owned();
    for _ in 0..150 {
        let state = response_json(call("GET", format!("/v1/repositories/{repository_id}"), json!({})).await).await;
        if state["status"] == "ready" && state["active_job"].is_null() && state["indexed_file_count"] == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let files = response_json(call("GET", format!("/v1/repositories/{repository_id}/files"), json!({})).await).await;
    let file_id = files[0]["artifact_id"].as_str().unwrap().to_owned();

    // The tracker's first issue says where the bug is.
    let connected = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/connectors"),
            json!({"link": format!("{base}/acme/app")}),
        )
        .await,
    )
    .await;
    let source_id = connected["connector"]["id"].as_str().unwrap().to_owned();
    for _ in 0..150 {
        let connector = response_json(call("GET", format!("/v1/connectors/{source_id}"), json!({})).await).await;
        if connector["status"] == "ready" && connector["active_job"].is_null() && connector["last_report"]["added"] == 3
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let mentions = response_json(call("GET", format!("/v1/artifacts/{file_id}/mentions"), json!({})).await).await;
    let named_in = mentions["mentioned_in"].as_array().unwrap();
    assert!(
        named_in
            .iter()
            .any(|mention| mention["kind"] == "file"
                && mention["title"].as_str().unwrap().contains("Refunds double post")),
        "{mentions}"
    );
    assert!(
        named_in
            .iter()
            .any(|mention| mention["kind"] == "symbol" && mention["label"] == "post_refund"),
        "{mentions}"
    );

    // And the item lists what it names.
    let artifacts =
        response_json(call("GET", format!("/v1/workspaces/{workspace_id}/artifacts"), json!({})).await).await;
    let issue = artifacts
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["title"].as_str().unwrap().contains("Refunds double post"))
        .unwrap();
    let from_issue = response_json(
        call(
            "GET",
            format!("/v1/artifacts/{}/mentions", issue["id"].as_str().unwrap()),
            json!({}),
        )
        .await,
    )
    .await;
    assert!(from_issue["mentions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|mention| mention["artifact_id"] == file_id.as_str()));

    // The pull request says "Fixes #1": it names the issue, and the issue is named in it.
    let pull = artifacts
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["title"].as_str().unwrap().contains("Guard the ledger"))
        .unwrap();
    let from_pull = response_json(
        call(
            "GET",
            format!("/v1/artifacts/{}/mentions", pull["id"].as_str().unwrap()),
            json!({}),
        )
        .await,
    )
    .await;
    assert!(
        from_pull["mentions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|mention| mention["kind"] == "item"
                && mention["label"] == "#1"
                && mention["artifact_id"] == issue["id"]),
        "{from_pull}"
    );
    assert!(
        !from_pull["mentions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|mention| mention["artifact_id"] == pull["id"]),
        "an item never names itself: {from_pull}"
    );
    let from_issue = response_json(
        call(
            "GET",
            format!("/v1/artifacts/{}/mentions", issue["id"].as_str().unwrap()),
            json!({}),
        )
        .await,
    )
    .await;
    assert!(
        from_issue["mentioned_in"]
            .as_array()
            .unwrap()
            .iter()
            .any(|mention| mention["kind"] == "item"
                && mention["label"] == "#1"
                && mention["artifact_id"] == pull["id"]),
        "{from_issue}"
    );

    // Outsiders cannot read them.
    let (outsider, _) = register_account(&app, "mention-outsider@example.com").await;
    let refused = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/artifacts/{file_id}/mentions"),
            &outsider,
        ))
        .await
        .unwrap();
    assert_eq!(refused.status(), 403);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn a_tracker_that_asks_to_wait_or_refuses_the_token_pauses_the_connector() {
    let fake = FakeGitHub::default();
    *fake.version.lock().unwrap() = 1;
    let base = start_fake_github(fake.clone()).await;
    let data_dir = temp_data_dir("connector-pauses");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.git_allowed_hosts = vec!["127.0.0.1".to_owned()];
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "pause-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &owner, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let wait_for = |status: &'static str| {
        let app = app.clone();
        let owner = owner.clone();
        async move {
            move |source_id: String| {
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
            }
        }
    };
    let ready = wait_for("ready").await;
    let paused = wait_for("paused").await;

    // A short rate limit is waited out inside the request: nothing pauses.
    *fake.mode.lock().unwrap() = 1;
    *fake.limited_calls.lock().unwrap() = 1;
    let connected = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/connectors"),
            json!({"link": format!("{base}/acme/app"), "token": "ghp_first"}),
        )
        .await,
    )
    .await;
    let source_id = connected["connector"]["id"].as_str().unwrap().to_owned();
    let first = ready(source_id.clone()).await;
    assert_eq!(first["last_report"]["added"], 3, "{first}");
    assert!(first["paused_reason"].is_null());

    // A long one pauses it, says why and until when, and is not asked again.
    *fake.mode.lock().unwrap() = 2;
    *fake.version.lock().unwrap() = 2;
    assert_eq!(
        call("POST", format!("/v1/connectors/{source_id}/sync"), json!({}))
            .await
            .status(),
        200
    );
    let limited = paused(source_id.clone()).await;
    assert!(
        limited["paused_reason"].as_str().unwrap().contains("rate limit"),
        "{limited}"
    );
    assert!(limited["paused_until"].is_string());
    let again = call("POST", format!("/v1/connectors/{source_id}/sync"), json!({})).await;
    assert_eq!(again.status(), 409);
    assert!(response_json(again).await.to_string().contains("paused until"));

    // A refused token pauses it until the token is replaced.
    *fake.mode.lock().unwrap() = 3;
    // Replacing the token tried against the tracker clears an earlier pause.
    let replaced = call(
        "PUT",
        format!("/v1/connectors/{source_id}/token"),
        json!({"token": "ghp_second"}),
    )
    .await;
    assert_eq!(replaced.status(), 200);
    let replaced = response_json(replaced).await;
    assert!(replaced["paused_reason"].is_null(), "{replaced}");
    assert_eq!(
        call("POST", format!("/v1/connectors/{source_id}/sync"), json!({}))
            .await
            .status(),
        200
    );
    let refused = paused(source_id.clone()).await;
    assert!(
        refused["paused_reason"].as_str().unwrap().contains("refused the token"),
        "{refused}"
    );
    assert!(refused["paused_until"].is_null());

    // Webhooks: signed calls sync, others are refused alike.
    *fake.mode.lock().unwrap() = 0;
    let created = response_json(call("POST", format!("/v1/connectors/{source_id}/webhook"), json!({})).await).await;
    let secret = created["secret"].as_str().unwrap().to_owned();
    assert_eq!(created["path"], format!("/v1/webhooks/connectors/{source_id}"));
    let signature = |body: &str| {
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_bytes());
        format!(
            "sha256={}",
            hex::encode(ring::hmac::sign(&key, body.as_bytes()).as_ref())
        )
    };
    let send = |event: &'static str, signature: String| {
        let app = app.clone();
        let path = format!("/v1/webhooks/connectors/{source_id}");
        async move {
            let request = Request::builder()
                .method("POST")
                .uri(path)
                .header("x-hub-signature-256", signature)
                .header("x-github-event", event)
                .body(Body::from("{}"))
                .unwrap();
            let response = app.oneshot(request).await.unwrap();
            (response.status().as_u16(), response_json(response).await)
        }
    };
    assert_eq!(send("issues", "sha256=00".to_owned()).await.0, 401);
    let (status, body) = send("ping", signature("{}")).await;
    assert_eq!((status, body["note"].as_str()), (200, Some("pong")));
    let (status, body) = send("push", signature("{}")).await;
    assert_eq!((status, body["queued"].as_bool()), (202, Some(false)));
    let (status, body) = send("issues", signature("{}")).await;
    assert_eq!((status, body["queued"].as_bool()), (202, Some(true)), "{body}");
    let after = ready(source_id.clone()).await;
    assert!(after["paused_reason"].is_null(), "{after}");
    assert!(after["last_report"]["updated"].as_u64().unwrap_or(0) >= 1, "{after}");
    let shown = response_json(call("GET", format!("/v1/connectors/{source_id}"), json!({})).await).await;
    assert_eq!(shown["has_webhook"], true);
    assert!(shown["webhook_last_received_at"].is_string());
    assert!(!shown.to_string().contains(&secret));
    assert_eq!(
        call("DELETE", format!("/v1/connectors/{source_id}/webhook"), json!({}))
            .await
            .status(),
        204
    );
    assert_eq!(send("issues", signature("{}")).await.0, 401);
    let _ = std::fs::remove_dir_all(data_dir);
}
