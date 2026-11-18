//! Conformance fixtures for the Jira connector (V0.8.6): edits, removals, a restriction added later, a repeated page, a rate
//! limit and lost access, each against a fake Jira whose answers the test changes between syncs.

use super::*;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Issue {
    id: u32,
    summary: String,
    updated: String,
    security: bool,
}

#[derive(Clone, Default)]
struct FakeJira {
    issues: Arc<Mutex<Vec<Issue>>>,
    /// A status the project and its search answer with instead of their data (`0` answers normally).
    deny: Arc<Mutex<u16>>,
    /// The search answers a page that always points at itself.
    repeat_page: Arc<Mutex<bool>>,
    /// The search asks to wait for an hour.
    rate_limited: Arc<Mutex<bool>>,
}

async fn start_fake_jira(state: FakeJira) -> String {
    use axum::{
        extract::{Path, State},
        http::{header, StatusCode},
        response::{IntoResponse, Response},
        routing::get,
    };
    let app = axum::Router::new()
        .route(
            "/rest/api/3/project/APP",
            get(|State(state): State<FakeJira>| async move {
                match *state.deny.lock().unwrap() {
                    0 => axum::Json(json!({"key": "APP", "name": "Apps"})).into_response(),
                    code => StatusCode::from_u16(code).unwrap().into_response(),
                }
            }),
        )
        .route(
            "/rest/api/3/search/jql",
            get(|State(state): State<FakeJira>| async move {
                if *state.rate_limited.lock().unwrap() {
                    let mut response: Response = StatusCode::TOO_MANY_REQUESTS.into_response();
                    response
                        .headers_mut()
                        .insert(header::RETRY_AFTER, "3600".parse().unwrap());
                    return response;
                }
                let code = *state.deny.lock().unwrap();
                if code != 0 {
                    return StatusCode::from_u16(code).unwrap().into_response();
                }
                let issues = state
                    .issues
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|issue| {
                        json!({"id": issue.id.to_string(), "key": format!("APP-{}", issue.id), "fields": {
                            "summary": issue.summary, "labels": [], "created": "2026-01-01T00:00:00.000+0000",
                            "updated": issue.updated,
                            "status": {"name": "To Do", "statusCategory": {"name": "To Do", "key": "new"}},
                            "issuetype": {"name": "Task"}, "priority": {"name": "Low"},
                            "reporter": {"displayName": "Ana"}, "assignee": null,
                            "security": if issue.security { json!({"name": "Staff only"}) } else { Value::Null },
                            "description": {"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": format!("Body of {}", issue.summary)}]}]}
                        }})
                    })
                    .collect::<Vec<_>>();
                if *state.repeat_page.lock().unwrap() {
                    return axum::Json(json!({"issues": issues, "nextPageToken": "same", "isLast": false}))
                        .into_response();
                }
                axum::Json(json!({"issues": issues, "isLast": true})).into_response()
            }),
        )
        .route(
            "/rest/api/3/issue/{key}/comment",
            get(|Path(_key): Path<String>| async { axum::Json(json!({"comments": []})) }),
        )
        .route(
            "/rest/api/3/mypermissions",
            get(|| async { axum::Json(json!({"permissions": {"BROWSE_PROJECTS": {"havePermission": true}}})) }),
        )
        .route(
            "/rest/api/3/project/APP/issuesecuritylevelscheme",
            get(|| async { axum::Json(json!({"levels": []})) }),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

fn issue(id: u32, summary: &str, day: u32) -> Issue {
    Issue {
        id,
        summary: summary.to_owned(),
        updated: format!("2026-01-{day:02}T00:00:00.000+0000"),
        security: false,
    }
}

struct Scene {
    app: axum::Router,
    owner: String,
    workspace_id: String,
    source_id: String,
    fake: FakeJira,
    dir: std::path::PathBuf,
}

impl Scene {
    async fn new(name: &str) -> Scene {
        let fake = FakeJira::default();
        *fake.issues.lock().unwrap() = vec![
            issue(1, "Alpacas arrive", 1),
            issue(2, "Llamas depart", 2),
            issue(3, "Vicunas wait", 3),
        ];
        let base = start_fake_jira(fake.clone()).await;
        let dir = temp_data_dir(name);
        let mut config = ServerConfig::for_test(dir.clone());
        config.git_allowed_hosts = vec!["127.0.0.1".to_owned()];
        let app = router(config).await.unwrap();
        let (owner, _) = register_account(&app, &format!("{name}@example.com")).await;
        let workspace_id = create_workspace(&app, &owner).await;
        let connected = response_json(
            app.clone()
                .oneshot(json_request(
                    "POST",
                    &format!("/v1/workspaces/{workspace_id}/connectors"),
                    &owner,
                    json!({"link": format!("{base}/browse/APP"), "provider": "jira",
                           "token": "ana@acme.com:secret", "approve_sharing": true}),
                ))
                .await
                .unwrap(),
        )
        .await;
        let source_id = connected["connector"]["id"].as_str().unwrap().to_owned();
        let scene = Scene {
            app,
            owner,
            workspace_id,
            source_id,
            fake,
            dir,
        };
        scene.wait_for("ready").await;
        scene
    }

    async fn call(&self, method: &'static str, uri: String, body: Value) -> axum::response::Response {
        self.app
            .clone()
            .oneshot(json_request(method, &uri, &self.owner, body))
            .await
            .unwrap()
    }

    async fn wait_for(&self, status: &str) -> Value {
        for _ in 0..150 {
            let connector = response_json(
                self.call("GET", format!("/v1/connectors/{}", self.source_id), json!({}))
                    .await,
            )
            .await;
            if connector["status"] == status && connector["active_job"].is_null() {
                return connector;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        panic!("the connector never became {status}");
    }

    async fn sync(&self, status: &str) -> Value {
        let started = self
            .call("POST", format!("/v1/connectors/{}/sync", self.source_id), json!({}))
            .await;
        assert_eq!(started.status(), 200);
        self.wait_for(status).await
    }

    async fn titles(&self) -> String {
        response_json(
            self.call(
                "GET",
                format!("/v1/workspaces/{}/artifacts", self.workspace_id),
                json!({}),
            )
            .await,
        )
        .await
        .to_string()
    }

    async fn found(&self, query: &str) -> usize {
        response_json(
            self.call(
                "POST",
                format!("/v1/workspaces/{}/search", self.workspace_id),
                json!({"query": query, "limit": 10}),
            )
            .await,
        )
        .await
        .as_array()
        .map(Vec::len)
        .unwrap_or(0)
    }
}

#[tokio::test]
async fn jira_edits_removals_and_restrictions_follow_the_tracker_without_duplicates() {
    let scene = Scene::new("jira-lifecycle").await;
    let titles = scene.titles().await;
    assert!(titles.contains("APP-1") && titles.contains("APP-2") && titles.contains("APP-3"));
    assert!(scene.found("llamas").await >= 1);

    // An edit changes the same file: nothing is added twice.
    {
        let mut issues = scene.fake.issues.lock().unwrap();
        issues[1] = issue(2, "Guanacos depart", 9);
    }
    let done = scene.sync("ready").await;
    assert_eq!(done["last_report"]["updated"], 1, "{done}");
    assert_eq!(done["last_report"]["added"], 0);
    assert_eq!(done["open_items"], 3);
    assert!(scene.found("guanacos").await >= 1);
    assert_eq!(scene.found("llamas").await, 0, "the old text is gone");

    // A deleted issue leaves the evidence and the search.
    scene.fake.issues.lock().unwrap().remove(2);
    let done = scene.sync("ready").await;
    assert_eq!(done["last_report"]["removed"], 1, "{done}");
    assert!(!scene.titles().await.contains("APP-3"));
    assert_eq!(scene.found("vicunas").await, 0);
    assert_eq!(done["open_items"], 2);

    // An issue that is given a security level leaves too, and is counted as restricted.
    scene.fake.issues.lock().unwrap()[0].security = true;
    scene.fake.issues.lock().unwrap()[0].updated = "2026-01-10T00:00:00.000+0000".to_owned();
    let done = scene.sync("ready").await;
    assert_eq!(done["last_report"]["restricted_skipped"], 1, "{done}");
    assert_eq!(done["last_report"]["removed"], 1, "{done}");
    assert!(!scene.titles().await.contains("APP-1"));
    assert_eq!(scene.found("alpacas").await, 0);

    // A list that would remove everything is not trusted: the last issue stays.
    scene.fake.issues.lock().unwrap().clear();
    let done = scene.sync("ready").await;
    assert_eq!(done["last_report"]["removed"], 0, "{done}");
    assert!(scene.titles().await.contains("APP-2"));

    // A tracker that repeats a page cannot make the sync loop.
    *scene.fake.repeat_page.lock().unwrap() = true;
    scene.fake.issues.lock().unwrap().push(issue(2, "Guanacos depart", 9));
    let failed = scene.sync("error").await;
    assert!(
        failed["last_error"].as_str().unwrap().contains("repeated a page"),
        "{failed}"
    );
    assert_eq!(failed["health"]["state"], "failed");
    let _ = std::fs::remove_dir_all(&scene.dir);
}

#[tokio::test]
async fn a_jira_rate_limit_pauses_and_lost_access_holds_the_evidence_back() {
    let scene = Scene::new("jira-limits").await;

    // A long rate limit pauses the connector with a reason and an end, and it is not asked again.
    *scene.fake.rate_limited.lock().unwrap() = true;
    let paused = scene.sync("paused").await;
    assert!(
        paused["paused_reason"].as_str().unwrap().contains("rate limit"),
        "{paused}"
    );
    assert!(paused["paused_until"].is_string());
    assert_eq!(paused["health"]["state"], "paused");
    assert_eq!(
        scene
            .call("POST", format!("/v1/connectors/{}/sync", scene.source_id), json!({}))
            .await
            .status(),
        409
    );
    assert!(scene.found("llamas").await >= 1, "a pause keeps the evidence served");
    let _ = std::fs::remove_dir_all(&scene.dir);

    // A project that stops answering holds the copy back instead.
    let scene = Scene::new("jira-lost").await;
    *scene.fake.deny.lock().unwrap() = 404;
    let held = scene.sync("quarantined").await;
    assert_eq!(held["quarantine"]["reason"], "not_found", "{held}");
    assert_eq!(scene.found("llamas").await, 0);
    let _ = std::fs::remove_dir_all(&scene.dir);
}

#[tokio::test]
async fn a_mirrored_issue_and_its_local_follow_up_link_both_ways_and_keep_their_own_state() {
    let scene = Scene::new("jira-follow-ups").await;
    let artifacts = response_json(
        scene
            .call(
                "GET",
                format!("/v1/workspaces/{}/artifacts", scene.workspace_id),
                json!({}),
            )
            .await,
    )
    .await;
    let item_id = artifacts
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["title"].as_str().unwrap().contains("APP-1"))
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // A file that is not a mirrored item has no tracker state to follow up.
    let missing = scene
        .call("POST", "/v1/artifacts/does-not-exist/follow-up".to_owned(), json!({}))
        .await;
    assert!(missing.status() == 404 || missing.status() == 400);

    // A follow-up is a local task that cites the item.
    let made = scene
        .call("POST", format!("/v1/artifacts/{item_id}/follow-up"), json!({}))
        .await;
    assert_eq!(made.status(), 201);
    let task = response_json(made).await;
    let task_id = task["id"].as_str().unwrap().to_owned();
    assert!(task["title"].as_str().unwrap().starts_with("Follow up: APP-1"));
    assert_eq!(task["status"], "open");

    // One open follow-up per item, unless another is asked for.
    let again = scene
        .call("POST", format!("/v1/artifacts/{item_id}/follow-up"), json!({}))
        .await;
    assert_eq!(again.status(), 409);
    assert!(response_json(again)
        .await
        .to_string()
        .contains(task["key"].as_str().unwrap()));
    assert_eq!(
        scene
            .call(
                "POST",
                format!("/v1/artifacts/{item_id}/follow-up"),
                json!({"allow_another": true, "title": "Check the ledger"}),
            )
            .await
            .status(),
        201
    );

    // From the task, the citation carries the tracker's state as the tracker's.
    let references = response_json(
        scene
            .call("GET", format!("/v1/tasks/{task_id}/references"), json!({}))
            .await,
    )
    .await;
    let remote = &references[0]["remote"];
    assert_eq!(remote["provider"], "jira", "{references}");
    assert_eq!(remote["state"], "open");
    assert_eq!(remote["owner"], "remote");
    assert!(remote["url"].as_str().unwrap().ends_with("/browse/APP-1"));

    // From the item, the local work that follows it.
    let links = response_json(
        scene
            .call("GET", format!("/v1/artifacts/{item_id}/follow-ups"), json!({}))
            .await,
    )
    .await;
    assert_eq!(links["remote"]["state"], "open");
    assert_eq!(links["tasks"].as_array().unwrap().len(), 2, "{links}");

    // Finishing the local task changes the task, never the tracker's state.
    let updated = scene
        .call(
            "PUT",
            format!("/v1/tasks/{task_id}"),
            json!({"title": task["title"], "status": "done", "priority": "medium"}),
        )
        .await;
    assert_eq!(updated.status(), 200);
    let links = response_json(
        scene
            .call("GET", format!("/v1/artifacts/{item_id}/follow-ups"), json!({}))
            .await,
    )
    .await;
    assert_eq!(links["remote"]["state"], "open", "a local edit is not a remote update");
    assert!(links["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|linked| linked["task_id"] == task_id.as_str() && linked["status"] == "done"));
    let connector = response_json(
        scene
            .call("GET", format!("/v1/connectors/{}", scene.source_id), json!({}))
            .await,
    )
    .await;
    assert_eq!(connector["open_items"], 3, "the tracker's items are untouched");

    // Held back by a lost source, the item is not there: no links, no state, no title.
    *scene.fake.deny.lock().unwrap() = 404;
    scene.sync("quarantined").await;
    assert_eq!(
        scene
            .call("GET", format!("/v1/artifacts/{item_id}/follow-ups"), json!({}))
            .await
            .status(),
        404
    );
    let references = response_json(
        scene
            .call("GET", format!("/v1/tasks/{task_id}/references"), json!({}))
            .await,
    )
    .await;
    assert_eq!(references[0]["available"], false);
    assert!(references[0]["remote"].is_null());
    let _ = std::fs::remove_dir_all(&scene.dir);
}
