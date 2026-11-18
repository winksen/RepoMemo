use super::*;
use std::sync::{Arc, Mutex};

/// A stand-in for Jira Cloud's REST API: one project, three issues over two pages, one of them with a security level.
#[derive(Clone, Default)]
struct FakeJira {
    authorizations: Arc<Mutex<Vec<String>>>,
    queries: Arc<Mutex<Vec<String>>>,
    secured_project: Arc<Mutex<bool>>,
}

async fn start_fake_jira(state: FakeJira) -> String {
    use axum::{
        extract::{RawQuery, State},
        http::HeaderMap,
        routing::get,
    };
    fn note(state: &FakeJira, headers: &HeaderMap) {
        if let Some(value) = headers.get("authorization") {
            state
                .authorizations
                .lock()
                .unwrap()
                .push(value.to_str().unwrap().to_owned());
        }
    }
    let issue = |id: &str, key: &str, summary: &str, security: bool| {
        json!({"id": id, "key": key, "fields": {
            "summary": summary, "labels": ["billing"], "created": "2026-01-01T00:00:00.000+0000",
            "updated": format!("2026-01-0{id}T00:00:00.000+0000"),
            "status": {"name": "To Do", "statusCategory": {"name": "To Do", "key": "new"}},
            "issuetype": {"name": "Bug"}, "priority": {"name": "High"},
            "reporter": {"displayName": "Ana"}, "assignee": null,
            "security": if security { json!({"name": "Staff only"}) } else { Value::Null },
            "description": {"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": format!("Quokka report for {key}.")}]}]}
        }})
    };
    let app = axum::Router::new()
        .route(
            "/rest/api/3/project/APP",
            get(|State(state): State<FakeJira>, headers: HeaderMap| async move {
                note(&state, &headers);
                axum::Json(json!({"key": "APP", "name": "Apps", "isPrivate": true}))
            }),
        )
        .route(
            "/rest/api/3/mypermissions",
            get(|| async { axum::Json(json!({"permissions": {"BROWSE_PROJECTS": {"havePermission": true}}})) }),
        )
        .route(
            "/rest/api/3/project/APP/issuesecuritylevelscheme",
            get(|State(state): State<FakeJira>| async move {
                if *state.secured_project.lock().unwrap() {
                    axum::Json(json!({"levels": [{"name": "Staff only"}]}))
                } else {
                    axum::Json(json!({"levels": []}))
                }
            }),
        )
        .route(
            "/rest/api/3/search/jql",
            get(
                move |State(state): State<FakeJira>, headers: HeaderMap, RawQuery(query): RawQuery| async move {
                    note(&state, &headers);
                    let query = query.unwrap_or_default();
                    state.queries.lock().unwrap().push(query.clone());
                    if query.contains("nextPageToken=p2") {
                        return axum::Json(json!({"issues": [issue("3", "APP-3", "Third issue", false)], "isLast": true}));
                    }
                    axum::Json(json!({
                        "issues": [issue("1", "APP-1", "First issue", false), issue("2", "APP-2", "Hidden issue", true)],
                        "nextPageToken": "p2", "isLast": false
                    }))
                },
            ),
        )
        .route(
            "/rest/api/3/issue/APP-1/comment",
            get(|| async {
                axum::Json(json!({"comments": [{"author": {"displayName": "Bo"}, "created": "2026-01-02",
                    "body": {"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "Reproduced on staging."}]}]}}]}))
            }),
        )
        .route(
            "/rest/api/3/issue/APP-3/comment",
            get(|| async { axum::Json(json!({"comments": []})) }),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

#[tokio::test]
async fn jira_issues_are_read_with_a_dry_run_and_restricted_ones_are_never_copied() {
    let fake = FakeJira::default();
    let base = start_fake_jira(fake.clone()).await;
    let link = format!("{base}/browse/APP");
    let data_dir = temp_data_dir("connectors-jira");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.git_allowed_hosts = vec!["127.0.0.1".to_owned()];
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "jira-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &owner, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let token = "ana@acme.com:jira-secret-token";

    // Jira needs an account; a bare token is refused before anything is asked.
    let bare = call(
        "POST",
        format!("/v1/workspaces/{workspace_id}/connectors/check"),
        json!({"link": link, "provider": "jira", "token": "jira-secret-token"}),
    )
    .await;
    assert_eq!(bare.status(), 400);

    // The dry run reports the scope and the permission, changes nothing and shows no secret.
    let checked = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/connectors/check"),
            json!({"link": link, "provider": "jira", "token": token}),
        )
        .await,
    )
    .await;
    assert_eq!(checked["provider"], "jira");
    assert_eq!(checked["preview"]["sharing"], "uniform");
    assert!(checked["name"].as_str().unwrap().contains("APP"));
    assert!(!checked.to_string().contains("jira-secret-token"));
    let listed = response_json(call("GET", format!("/v1/workspaces/{workspace_id}/connectors"), json!({})).await).await;
    assert_eq!(
        listed["connectors"].as_array().unwrap().len(),
        0,
        "a dry run stores nothing"
    );
    assert!(
        fake.authorizations
            .lock()
            .unwrap()
            .iter()
            .all(|value| value.starts_with("Basic ")),
        "the account is sent as basic authentication"
    );

    // A project with security levels is refused.
    *fake.secured_project.lock().unwrap() = true;
    let refused = call(
        "POST",
        format!("/v1/workspaces/{workspace_id}/connectors"),
        json!({"link": link, "provider": "jira", "token": token, "approve_sharing": true}),
    )
    .await;
    assert_eq!(refused.status(), 409);
    assert!(response_json(refused).await.to_string().contains("security levels"));
    *fake.secured_project.lock().unwrap() = false;

    // Connecting reads both pages; the restricted issue is counted and never stored.
    let connected = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/connectors"),
            json!({"link": link, "provider": "jira", "token": token, "approve_sharing": true}),
        )
        .await,
    )
    .await;
    assert!(!connected.to_string().contains("jira-secret-token"));
    let source_id = connected["connector"]["id"].as_str().unwrap().to_owned();
    assert_eq!(connected["connector"]["provider"], "jira");
    let mut done = Value::Null;
    for _ in 0..150 {
        done = response_json(call("GET", format!("/v1/connectors/{source_id}"), json!({})).await).await;
        if done["status"] == "ready" && done["active_job"].is_null() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(done["status"], "ready", "{done}");
    assert_eq!(done["last_report"]["added"], 2, "{done}");
    assert_eq!(done["last_report"]["restricted_skipped"], 1, "{done}");
    assert_eq!(done["health"]["state"], "complete");
    let artifacts =
        response_json(call("GET", format!("/v1/workspaces/{workspace_id}/artifacts"), json!({})).await).await;
    let text = artifacts.to_string();
    assert!(text.contains("APP-1") && text.contains("APP-3"));
    assert!(!text.contains("Hidden issue"), "a restricted issue is never copied");
    let found = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/search"),
            json!({"query": "quokka", "limit": 10}),
        )
        .await,
    )
    .await;
    assert!(
        found.as_array().unwrap().len() >= 2,
        "the description text is searchable: {found}"
    );
    let found = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/search"),
            json!({"query": "staging", "limit": 10}),
        )
        .await,
    )
    .await;
    assert!(!found.as_array().unwrap().is_empty(), "the comments are searchable");

    // The next sync asks only for what changed, and stores nothing twice.
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
            .any(|query| query.contains("updated+%3E%3D") || query.contains("updated%20%3E%3D"))
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
            .any(|query| query.contains("updated+%3E%3D") || query.contains("updated%20%3E%3D")),
        "{:?}",
        fake.queries.lock().unwrap()
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
