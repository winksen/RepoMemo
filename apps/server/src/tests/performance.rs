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
