use super::*;

#[tokio::test]
async fn a_snapshot_is_previewed_then_recorded_with_only_the_reviewed_text() {
    let off_dir = temp_data_dir("publications-off");
    let off = router(ServerConfig::for_test(off_dir.clone())).await.unwrap();
    let (who, _) = register_account(&off, "pub-off@example.com").await;
    let ws = create_workspace(&off, &who).await;
    let made = response_json(
        off.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{ws}/artifacts/text"),
                &who,
                json!({"title": "Plan", "content": "x", "language": "Markdown"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let id = made["id"].as_str().unwrap().to_owned();
    let off_preview = off
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/artifacts/{id}/publication-preview"),
            &who,
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(
        off_preview.status(),
        404,
        "publishing is off until the operator turns it on"
    );
    let _ = std::fs::remove_dir_all(off_dir);

    let dir = temp_data_dir("publications");
    let mut config = ServerConfig::for_test(dir.clone());
    config.public_publishing = true;
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "pub-owner@example.com").await;
    let (stranger, _) = register_account(&app, "pub-stranger@example.com").await;
    let ws = create_workspace(&app, &owner).await;
    let call = |who: String, method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let text = "# Guide\nSee [the plan](/workspaces/w/artifacts/a1) and [docs](https://example.com).\n![chart](img/a.png)\nWritten by ana@example.com.";
    let made = response_json(
        call(
            owner.clone(),
            "POST",
            format!("/v1/workspaces/{ws}/artifacts/text"),
            json!({"title": "Guide", "content": text, "language": "Markdown"}),
        )
        .await,
    )
    .await;
    let id = made["id"].as_str().unwrap().to_owned();

    // Only an administrator previews or publishes.
    let by_stranger = call(
        stranger,
        "POST",
        format!("/v1/artifacts/{id}/publication-preview"),
        json!({}),
    )
    .await;
    assert!(by_stranger.status() == 403 || by_stranger.status() == 404);

    let preview = response_json(
        call(
            owner.clone(),
            "POST",
            format!("/v1/artifacts/{id}/publication-preview"),
            json!({}),
        )
        .await,
    )
    .await;
    let body = preview["body_markdown"].as_str().unwrap();
    assert!(body.contains("See the plan and [docs](https://example.com)."), "{body}");
    assert!(!body.contains("artifacts/a1") && !body.contains("img/a.png"));
    assert!(!body.contains("ana@example.com"), "{body}");
    assert_eq!(preview["removed"].as_array().unwrap().len(), 3);

    // A wrong token publishes nothing; the right one records exactly the previewed text.
    let wrong = call(
        owner.clone(),
        "POST",
        format!("/v1/artifacts/{id}/publication"),
        json!({"review_token": "nope"}),
    )
    .await;
    assert_eq!(wrong.status(), 409);
    let done = call(
        owner.clone(),
        "POST",
        format!("/v1/artifacts/{id}/publication"),
        json!({"review_token": preview["review_token"]}),
    )
    .await;
    assert_eq!(done.status(), 201);
    let publication = response_json(done).await;
    assert_eq!(publication["status"], "published");
    let slug = publication["slug"].as_str().unwrap();
    assert!(
        !slug.contains(&id) && !slug.contains(&ws),
        "the public address says nothing private"
    );
    assert!(!publication.to_string().contains("ana@example.com"));
    assert_eq!(
        call(
            owner.clone(),
            "POST",
            format!("/v1/artifacts/{id}/publication"),
            json!({"review_token": preview["review_token"]})
        )
        .await
        .status(),
        409,
        "one live snapshot per file"
    );
    let listed = response_json(call(owner, "GET", format!("/v1/workspaces/{ws}/publications"), json!({})).await).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn a_publisher_sees_what_readers_get_and_can_withdraw_and_update_a_snapshot() {
    let dir = temp_data_dir("publication-controls");
    let mut config = ServerConfig::for_test(dir.clone());
    config.public_publishing = true;
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "pub-controls@example.com").await;
    let ws = create_workspace(&app, &owner).await;
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &owner, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let made = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{ws}/artifacts/text"),
            json!({"title": "Notes", "content": "First text.", "language": "Markdown"}),
        )
        .await,
    )
    .await;
    let id = made["id"].as_str().unwrap().to_owned();
    let preview = response_json(call("POST", format!("/v1/artifacts/{id}/publication-preview"), json!({})).await).await;
    let published = response_json(
        call(
            "POST",
            format!("/v1/artifacts/{id}/publication"),
            json!({"review_token": preview["review_token"]}),
        )
        .await,
    )
    .await;
    let pid = published["id"].as_str().unwrap().to_owned();

    // The public view is exactly the snapshot.
    let seen = response_json(call("GET", format!("/v1/publications/{pid}/public-preview"), json!({})).await).await;
    assert_eq!(seen["served"], true);
    assert_eq!(seen["body_markdown"], "First text.");
    assert_eq!(seen["cache_seconds"], 60);
    let listed = response_json(call("GET", format!("/v1/workspaces/{ws}/publications"), json!({})).await).await;
    assert_eq!(listed[0]["original"], "current");

    // Withdrawing serves nothing, keeps the record and raises the version.
    let withdrawn = response_json(call("POST", format!("/v1/publications/{pid}/withdraw"), json!({})).await).await;
    assert_eq!(withdrawn["status"], "withdrawn");
    assert_eq!(withdrawn["version"], 2);
    let seen = response_json(call("GET", format!("/v1/publications/{pid}/public-preview"), json!({})).await).await;
    assert_eq!(seen["served"], false);
    assert!(seen["body_markdown"].is_null());

    // Publishing again needs a review of the original as it is now.
    let old_token = call(
        "POST",
        format!("/v1/publications/{pid}/republish"),
        json!({"review_token": "old"}),
    )
    .await;
    assert_eq!(old_token.status(), 409);
    let again = response_json(
        call(
            "POST",
            format!("/v1/publications/{pid}/republish"),
            json!({"review_token": preview["review_token"]}),
        )
        .await,
    )
    .await;
    assert_eq!(again["status"], "published");
    assert_eq!(again["version"], 3);

    // The original going away is shown, and the snapshot is untouched.
    assert_eq!(
        call("DELETE", format!("/v1/artifacts/{id}"), json!({})).await.status(),
        204
    );
    let listed = response_json(call("GET", format!("/v1/workspaces/{ws}/publications"), json!({})).await).await;
    assert_eq!(listed[0]["original"], "source_removed");
    let seen = response_json(call("GET", format!("/v1/publications/{pid}/public-preview"), json!({})).await).await;
    assert_eq!(seen["body_markdown"], "First text.", "the private original is separate");
    let _ = std::fs::remove_dir_all(dir);
}
