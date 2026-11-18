use super::*;

#[tokio::test]
async fn ai_usage_and_limits_are_visible_to_admins_and_keyword_search_needs_no_ai() {
    let data_dir = temp_data_dir("ai-admin");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "ai-admin-owner@example.com").await;
    let (stranger, _) = register_account(&app, "ai-admin-stranger@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |who: String, method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let usage_uri = format!("/v1/workspaces/{workspace_id}/ai-usage");
    let limit_uri = format!("/v1/workspaces/{workspace_id}/ai-limit");

    // Only an administrator reads usage or sets a limit.
    let by_stranger = call(stranger.clone(), "GET", usage_uri.clone(), json!({})).await;
    assert!(by_stranger.status() == 403 || by_stranger.status() == 404);
    let limit_by_stranger = call(stranger, "PUT", limit_uri.clone(), json!({"daily_calls": 1})).await;
    assert!(limit_by_stranger.status() == 403 || limit_by_stranger.status() == 404);

    // A fresh workspace has used nothing and has no AI provider; the page says what still works.
    let usage = response_json(call(owner.clone(), "GET", usage_uri.clone(), json!({})).await).await;
    assert_eq!(usage["spent_today"], 0, "{usage}");
    assert_eq!(usage["refused_today"], 0);
    assert_eq!(usage["stored"]["files"], 0);
    assert_eq!(usage["providers"].as_array().unwrap().len(), 0);
    assert_eq!(usage["text_available"], false);
    assert!(usage["when_unavailable"]
        .as_str()
        .unwrap()
        .contains("Keyword search never calls an AI provider"));

    // A limit of the workspace's own replaces the default, and says so.
    let set = call(owner.clone(), "PUT", limit_uri.clone(), json!({"daily_calls": 5})).await;
    assert_eq!(set.status(), 200);
    let usage = response_json(call(owner.clone(), "GET", usage_uri.clone(), json!({})).await).await;
    assert_eq!(usage["limit"], 5);
    assert_eq!(usage["limit_source"], "workspace");
    assert_eq!(usage["remaining"], 5);

    // 0 is no limit; a value that is not sensible is refused; clearing returns to the server's default.
    assert_eq!(
        call(owner.clone(), "PUT", limit_uri.clone(), json!({"daily_calls": 0}))
            .await
            .status(),
        200
    );
    let usage = response_json(call(owner.clone(), "GET", usage_uri.clone(), json!({})).await).await;
    assert_eq!(usage["limit"], 0);
    assert!(usage["remaining"].is_null());
    assert_eq!(
        call(
            owner.clone(),
            "PUT",
            limit_uri.clone(),
            json!({"daily_calls": 2_000_000})
        )
        .await
        .status(),
        400
    );
    call(owner.clone(), "PUT", limit_uri, json!({"daily_calls": null})).await;
    let usage = response_json(call(owner.clone(), "GET", usage_uri, json!({})).await).await;
    assert_ne!(usage["limit_source"], "workspace");

    // With no provider at all, a file is still found by its words, and the answer says semantic search is not there.
    let made = call(
        owner.clone(),
        "POST",
        format!("/v1/workspaces/{workspace_id}/artifacts/text"),
        json!({"title": "Harbour notes", "content": "The pelican schedule changed on Tuesday.", "language": "Markdown"}),
    )
    .await;
    assert_eq!(made.status(), 201);
    let mut found = 0;
    for _ in 0..100 {
        let response = call(
            owner.clone(),
            "POST",
            format!("/v1/workspaces/{workspace_id}/search"),
            json!({"query": "pelican", "limit": 5}),
        )
        .await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["x-repomemo-semantic-search"], "unavailable");
        found = response_json(response).await.as_array().map(Vec::len).unwrap_or(0);
        if found > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(found > 0, "keyword search works with no AI provider");
    let _ = std::fs::remove_dir_all(data_dir);
}
