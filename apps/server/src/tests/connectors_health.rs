use super::*;
use std::sync::{Arc, Mutex};

/// A tracker with two pages of issues whose second page can fail.
#[derive(Clone, Default)]
struct FakeTracker {
    base: Arc<Mutex<String>>,
    second_page_fails: Arc<Mutex<bool>>,
    queries: Arc<Mutex<Vec<String>>>,
}

async fn start_fake_tracker(state: FakeTracker) -> String {
    use axum::{
        extract::{RawQuery, State},
        http::{header, StatusCode},
        response::{IntoResponse, Response},
        routing::get,
    };
    let app = axum::Router::new()
        .route(
            "/api/v3/repos/acme/app",
            get(|| async { axum::Json(json!({"full_name": "acme/app", "open_issues_count": 2, "private": false})) }),
        )
        .route(
            "/api/v3/repos/acme/app/issues",
            get(|State(state): State<FakeTracker>, RawQuery(query): RawQuery| async move {
                let query = query.unwrap_or_default();
                state.queries.lock().unwrap().push(query.clone());
                let issue = |number: u32, day: &str| {
                    json!({"number": number, "title": format!("Issue number {number}"), "state": "open", "comments": 0,
                           "user": {"login": "ana"}, "created_at": format!("2026-01-{day}T00:00:00Z"),
                           "updated_at": format!("2026-01-{day}T00:00:00Z"),
                           "html_url": format!("https://github.com/acme/app/issues/{number}"),
                           "body": format!("Body of issue {number}."), "labels": [], "assignees": []})
                };
                if query.contains("page=2") {
                    if *state.second_page_fails.lock().unwrap() {
                        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                    }
                    return axum::Json(json!([issue(2, "02")])).into_response();
                }
                // The first page, which says there is a second, and which a retry reads again from its cursor.
                let next = format!("<{}/api/v3/repos/acme/app/issues?page=2>; rel=\"next\"", state.base.lock().unwrap());
                let mut response: Response = axum::Json(json!([issue(1, "01")])).into_response();
                response.headers_mut().insert(header::LINK, next.parse().unwrap());
                response
            }),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

#[tokio::test]
async fn health_reports_a_failed_sync_resumes_after_the_stored_page_and_duplicates_nothing() {
    let fake = FakeTracker::default();
    let base = start_fake_tracker(fake.clone()).await;
    *fake.base.lock().unwrap() = base.clone();
    *fake.second_page_fails.lock().unwrap() = true;

    let data_dir = temp_data_dir("connector-health");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.git_allowed_hosts = vec!["127.0.0.1".to_owned()];
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "health-owner@example.com").await;
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

    // The first page is stored, then the tracker fails: the sync fails and says what to do.
    let connected = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/connectors"),
            json!({"link": format!("{base}/acme/app"), "approve_sharing": true}),
        )
        .await,
    )
    .await;
    let source_id = connected["connector"]["id"].as_str().unwrap().to_owned();
    let failed = wait_for(source_id.clone(), "error").await;
    let health = &failed["health"];
    assert_eq!(health["state"], "failed", "{failed}");
    assert_eq!(health["problem"]["kind"], "other");
    assert!(health["problem"]["action"].as_str().unwrap().contains("Sync now"));
    assert_eq!(
        health["resume_from"], "2026-01-01T00:00:00Z",
        "the stored page is the resume point"
    );
    assert_eq!(failed["open_items"], 1, "the first page is kept");
    assert!(health["last_success_at"].is_null());
    assert!(health["exclusions"].as_array().unwrap().len() >= 4);

    // The retry resumes from the stored page, reads the first item again as unchanged and adds the second.
    *fake.second_page_fails.lock().unwrap() = false;
    fake.queries.lock().unwrap().clear();
    call("POST", format!("/v1/connectors/{source_id}/sync"), json!({})).await;
    let done = wait_for(source_id.clone(), "ready").await;
    assert_eq!(done["health"]["state"], "complete", "{done}");
    assert!(done["health"]["problem"].is_null());
    assert!(done["health"]["last_success_at"].is_string());
    assert!(done["health"]["last_complete_at"].is_string());
    assert_eq!(done["health"]["items_kept"], 2);
    assert_eq!(done["last_report"]["added"], 1, "{done}");
    assert_eq!(done["last_report"]["unchanged"], 1, "a retry stores nothing twice");
    assert!(
        fake.queries
            .lock()
            .unwrap()
            .iter()
            .any(|query| query.contains("since=2026-01-01")),
        "{:?}",
        fake.queries.lock().unwrap()
    );
    let listed = response_json(call("GET", format!("/v1/workspaces/{workspace_id}/artifacts"), json!({})).await).await;
    assert_eq!(listed.as_array().unwrap().len(), 2, "no item twice");
    let _ = std::fs::remove_dir_all(data_dir);
}
