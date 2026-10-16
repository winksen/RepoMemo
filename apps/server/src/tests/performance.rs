use super::*;

async fn add_note(app: &axum::Router, who: &str, workspace_id: &str, title: &str, content: &str) {
    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            who,
            json!({"title": title, "content": content, "language": "Markdown"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 201, "note {title} is created");
}

#[tokio::test]
async fn the_dashboard_is_one_request_and_only_lists_your_workspaces() {
    let data_dir = temp_data_dir("dashboard");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (_, _) = register_account(&app, "dashboard-first@example.com").await;
    let (owner, _) = register_account(&app, "dashboard-owner@example.com").await;
    let (stranger, _) = register_account(&app, "dashboard-stranger@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    add_note(&app, &owner, &workspace_id, "Plan", "First note").await;
    add_note(&app, &owner, &workspace_id, "Notes", "Second note").await;
    app.clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/tasks"),
            &owner,
            json!({"title": "Write it down", "description": "", "status": "open", "priority": "medium"}),
        ))
        .await
        .unwrap();

    let mine = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/dashboard", &owner))
            .await
            .unwrap(),
    )
    .await;
    let entries = mine["workspaces"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["workspace_id"], workspace_id);
    assert_eq!(entries[0]["artifact_count"], 2);
    assert_eq!(entries[0]["memory_card_count"], 0);
    assert_eq!(entries[0]["active_task_count"], 1);

    let theirs = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/dashboard", &stranger))
            .await
            .unwrap(),
    )
    .await;
    assert!(theirs["workspaces"].as_array().unwrap().is_empty());
    let anonymous = app
        .clone()
        .oneshot(Request::builder().uri("/v1/dashboard").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(anonymous.status(), 401);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_artifact_query_filters_and_pages_in_the_database() {
    let data_dir = temp_data_dir("artifact-query");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "query-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    for title in ["Alpha plan", "Beta plan", "Gamma notes"] {
        add_note(&app, &owner, &workspace_id, title, "Some content to keep").await;
    }
    let query = |uri: String, body: Value| {
        let app = app.clone();
        let owner = owner.clone();
        async move {
            let response = app.oneshot(json_request("POST", &uri, &owner, body)).await.unwrap();
            let total = response
                .headers()
                .get("x-total-count")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            (total, response_json(response).await)
        }
    };
    let base = format!("/v1/workspaces/{workspace_id}/artifacts/query");

    let (total, all) = query(base.clone(), json!({"query": ""})).await;
    assert_eq!(all.as_array().unwrap().len(), 3);
    assert_eq!(total, None, "an unpaged answer has no total");

    let (total, filtered) = query(base.clone(), json!({"query": "PLAN"})).await;
    assert_eq!(filtered.as_array().unwrap().len(), 2, "the match ignores case");
    assert_eq!(total, None);

    let (total, page) = query(format!("{base}?limit=2&offset=0"), json!({"query": ""})).await;
    assert_eq!(page.as_array().unwrap().len(), 2);
    assert_eq!(total.as_deref(), Some("3"));
    let (_, rest) = query(format!("{base}?limit=2&offset=2"), json!({"query": ""})).await;
    assert_eq!(rest.as_array().unwrap().len(), 1);

    let (_, none) = query(base, json!({"query": "", "languages": ["python"]})).await;
    assert!(none.as_array().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn heavy_endpoints_are_limited_per_person() {
    let data_dir = temp_data_dir("heavy-limit");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.heavy_quota = crate::Quota::per_minute(2);
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "heavy-owner@example.com").await;
    let (other, _) = register_account(&app, "heavy-other@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let metrics = |who: String| {
        let app = app.clone();
        let uri = format!("/v1/workspaces/{workspace_id}/metrics");
        async move { app.oneshot(auth_request("GET", &uri, &who)).await.unwrap() }
    };
    assert_eq!(metrics(owner.clone()).await.status(), 200);
    assert_eq!(metrics(owner.clone()).await.status(), 200);
    let limited = metrics(owner.clone()).await;
    assert_eq!(limited.status(), 429);
    assert!(limited.headers().get("retry-after").is_some());
    // Ordinary reads are not counted, and the limit is per person.
    let ordinary = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/artifacts"),
            &owner,
        ))
        .await
        .unwrap();
    assert_eq!(ordinary.status(), 200);
    assert_ne!(metrics(other).await.status(), 429);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_cached_map_follows_changes_to_the_workspace() {
    let data_dir = temp_data_dir("map-cache");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "map-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let map = || {
        let app = app.clone();
        let owner = owner.clone();
        let uri = format!("/v1/workspaces/{workspace_id}/knowledge-map");
        async move { response_json(app.oneshot(auth_request("GET", &uri, &owner)).await.unwrap()).await }
    };
    let before = map().await;
    assert_eq!(before["nodes"].as_array().unwrap().len(), 0);
    assert_eq!(map().await, before, "the second answer is the same");
    add_note(&app, &owner, &workspace_id, "Release plan", "Ship on Friday").await;
    let after = map().await;
    assert_eq!(
        after["nodes"].as_array().unwrap().len(),
        1,
        "a change is not hidden by the cache"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

/// A stand-in for GitHub with `count` issues, one per second of `updated_at`, served the way GitHub pages them
/// (`per_page`, `since`, a `Link` header for the next page).
async fn start_tracker_with_issues(count: usize) -> String {
    use axum::{extract::RawQuery, http::HeaderMap, routing::get};
    let app = axum::Router::new()
        .route(
            "/api/v3/repos/acme/big",
            get(|| async { axum::Json(json!({"full_name": "acme/big", "open_issues_count": 1, "private": false})) }),
        )
        .route(
            "/api/v3/repos/acme/big/issues",
            get(move |RawQuery(query): RawQuery| async move {
                let query = query.unwrap_or_default();
                let find = |name: &str| {
                    query
                        .split('&')
                        .find_map(|pair| pair.strip_prefix(&format!("{name}=")))
                        .map(str::to_owned)
                };
                let per_page: usize = find("per_page").and_then(|value| value.parse().ok()).unwrap_or(30);
                let page: usize = find("page").and_then(|value| value.parse().ok()).unwrap_or(1);
                // `since` is the cursor of an earlier sync: an issue number's second, inclusive.
                let since = find("since")
                    .and_then(|value| chrono::DateTime::parse_from_rfc3339(&value.replace("%2B", "+").replace("%3A", ":")).ok())
                    .map_or(0, |time| (time.timestamp() - 1_700_000_000).max(0) as usize);
                let first = since + (page - 1) * per_page;
                let issues = (first..count.min(first + per_page))
                    .map(|index| {
                        let moment = chrono::DateTime::from_timestamp(1_700_000_000 + index as i64, 0).unwrap().to_rfc3339();
                        json!({
                            "number": index + 1,
                            "title": format!("Issue {index} about topic{}", index % 97),
                            "state": if index % 3 == 0 { "closed" } else { "open" },
                            "comments": 0,
                            "user": {"login": "ana"},
                            "created_at": moment, "updated_at": moment,
                            "html_url": format!("https://github.com/acme/big/issues/{}", index + 1),
                            "body": format!("Reproduced on build {index}: the refund_{} step posts twice.", index % 211),
                            "labels": [], "assignees": []
                        })
                    })
                    .collect::<Vec<_>>();
                let mut headers = HeaderMap::new();
                if first + per_page < count {
                    let next = format!(
                        "<http://{}/api/v3/repos/acme/big/issues?{}>; rel=\"next\"",
                        "{host}",
                        query.replace(&format!("page={page}"), &format!("page={}", page + 1))
                            + if find("page").is_none() { "&page=2" } else { "" }
                    );
                    headers.insert("link", next.parse().unwrap());
                }
                (headers, axum::Json(Value::Array(issues)))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    // The Link header names the listener, which is only known now: serve a router that fills it in.
    let app = app.layer(axum::middleware::map_response(move |mut response: axum::response::Response| async move {
        if let Some(link) = response.headers().get("link").and_then(|value| value.to_str().ok()).map(str::to_owned) {
            response
                .headers_mut()
                .insert("link", link.replace("{host}", &address.to_string()).parse().unwrap());
        }
        response
    }));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

/// Roadmap R10 (V0.2.54): a tracker with 10,000 issues, synced through a connector as many times as it takes
/// (a sync reads at most 30 pages of 100 and resumes from its cursor), then asked what a workspace is asked.
/// Not a check: `REPOMEMO_PERF_ISSUES` sets the size (default 10,000).
///
/// `cargo test -p repomemo-server --release ten_thousand_issues -- --ignored --nocapture`
#[tokio::test]
#[ignore = "a performance run, not a check"]
async fn ten_thousand_issues_sync_and_stay_searchable() {
    use std::time::{Duration, Instant};
    let count: usize = std::env::var("REPOMEMO_PERF_ISSUES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10_000);
    let base = start_tracker_with_issues(count).await;
    let data_dir = temp_data_dir("issues-scale");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.git_allowed_hosts = vec!["127.0.0.1".to_owned()];
    let app = router(config).await.unwrap();
    let (_, _) = register_account(&app, "scale-first@example.com").await;
    let (owner, _) = register_account(&app, "scale-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &owner, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let connected = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/connectors"),
            json!({"link": format!("{base}/acme/big"), "token": "ghp_scale"}),
        )
        .await,
    )
    .await;
    let source_id = connected["connector"]["id"].as_str().unwrap().to_owned();

    // Sync until every issue is in, counting the passes and the time each takes.
    let started = Instant::now();
    let mut passes = 0;
    loop {
        let pass = Instant::now();
        let mut connector = Value::Null;
        for _ in 0..6_000 {
            connector = response_json(call("GET", format!("/v1/connectors/{source_id}"), json!({})).await).await;
            if connector["status"] == "ready" && connector["active_job"].is_null() && connector["last_report"].is_object() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        passes += 1;
        let total = connector["open_items"].as_i64().unwrap_or(0) + connector["closed_items"].as_i64().unwrap_or(0);
        println!("pass {passes}: {:?}, {total} issues so far, more to fetch: {}", pass.elapsed(), connector["last_report"]["more_to_fetch"]);
        if total as usize >= count {
            break;
        }
        assert!(passes < 40, "the connector is not getting through the issues");
        call("POST", format!("/v1/connectors/{source_id}/sync"), json!({})).await;
    }
    println!("{count} issues read in {passes} passes, {:?} in all", started.elapsed());

    let started = Instant::now();
    wait_for_indexed_artifacts_for(&app, &owner, &workspace_id, count, 36_000).await;
    println!("{count} issues indexed in {:?}", started.elapsed());

    // What a workspace is asked, at this size.
    let mut searches = Vec::new();
    for round in 0..20 {
        let started = Instant::now();
        let hits = response_json(
            call(
                "POST",
                format!("/v1/workspaces/{workspace_id}/search"),
                json!({"query": format!("refund_{}", round * 7), "limit": 10}),
            )
            .await,
        )
        .await;
        searches.push(started.elapsed());
        assert!(!hits.as_array().unwrap().is_empty());
    }
    searches.sort();
    let p95 = searches[searches.len() * 95 / 100];
    let started = Instant::now();
    call("GET", "/v1/dashboard".to_owned(), json!({})).await;
    let dashboard = started.elapsed();
    let started = Instant::now();
    call("GET", format!("/v1/workspaces/{workspace_id}/knowledge-map"), json!({})).await;
    let map = started.elapsed();
    let started = Instant::now();
    call("GET", format!("/v1/workspaces/{workspace_id}/health"), json!({})).await;
    let health = started.elapsed();
    println!("search p95 {p95:?}, dashboard {dashboard:?}, knowledge map {map:?}, health {health:?}");

    // A sync with nothing new is quick, and the targets of V0.1.74 hold with the issues in.
    let started = Instant::now();
    call("POST", format!("/v1/connectors/{source_id}/sync"), json!({})).await;
    let _ = response_json(call("GET", format!("/v1/connectors/{source_id}"), json!({})).await).await;
    println!("asking again: {:?}", started.elapsed());
    assert!(p95 < Duration::from_millis(300), "search p95 {p95:?}");
    assert!(dashboard < Duration::from_millis(500), "dashboard {dashboard:?}");
    assert!(map < Duration::from_secs(2) && health < Duration::from_secs(2), "map {map:?}, health {health:?}");
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Like `wait_for_indexed_artifacts`, for a run that takes minutes: polls every 100 ms, up to `polls` times.
async fn wait_for_indexed_artifacts_for(app: &axum::Router, who: &str, workspace_id: &str, expected: usize, polls: usize) {
    for _ in 0..polls {
        let response = response_json(
            app.clone()
                .oneshot(auth_request("GET", &format!("/v1/workspaces/{workspace_id}/metrics"), who))
                .await
                .unwrap(),
        )
        .await;
        let indexed = response["indexed_artifact_count"].as_u64().unwrap_or(0) as usize;
        if indexed >= expected {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("{expected} files were not indexed in time");
}
