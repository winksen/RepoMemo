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

async fn anonymous(app: &axum::Router, uri: &str, etag: Option<&str>) -> axum::response::Response {
    let mut request = axum::http::Request::builder().method("GET").uri(uri);
    if let Some(etag) = etag {
        request = request.header("if-none-match", etag);
    }
    app.clone()
        .oneshot(request.body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn text_of(response: axum::response::Response) -> String {
    String::from_utf8(
        axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

#[tokio::test]
async fn anonymous_readers_see_only_live_snapshots_and_everything_else_is_the_same_404() {
    let dir = temp_data_dir("public-reading");
    let mut config = ServerConfig::for_test(dir.clone());
    config.public_publishing = true;
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "public-reader-owner@example.com").await;
    let ws = create_workspace(&app, &owner).await;
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &owner, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let publish = |title: &'static str, content: &'static str| {
        let call = &call;
        let ws = ws.clone();
        async move {
            let made = response_json(
                call(
                    "POST",
                    format!("/v1/workspaces/{ws}/artifacts/text"),
                    json!({"title": title, "content": content, "language": "Markdown"}),
                )
                .await,
            )
            .await;
            let id = made["id"].as_str().unwrap().to_owned();
            let preview =
                response_json(call("POST", format!("/v1/artifacts/{id}/publication-preview"), json!({})).await).await;
            response_json(
                call(
                    "POST",
                    format!("/v1/artifacts/{id}/publication"),
                    json!({"review_token": preview["review_token"], "title": title}),
                )
                .await,
            )
            .await
        }
    };
    let live = publish("Tide <i>tables</i>", "The harbour opens at dawn.").await;
    let _private = call(
        "POST",
        format!("/v1/workspaces/{ws}/artifacts/text"),
        json!({"title": "Private budget", "content": "The harbour budget is secret.", "language": "Markdown"}),
    )
    .await;
    let slug = live["slug"].as_str().unwrap().to_owned();
    let pid = live["id"].as_str().unwrap().to_owned();

    // The index lists the snapshot, with the title escaped, and nothing private.
    let index = anonymous(&app, "/public", None).await;
    assert_eq!(index.status(), 200);
    assert_eq!(index.headers()["x-content-type-options"], "nosniff");
    assert_eq!(index.headers()["referrer-policy"], "no-referrer");
    assert!(index.headers()["content-security-policy"]
        .to_str()
        .unwrap()
        .contains("default-src 'none'"));
    assert!(index.headers().get("set-cookie").is_none());
    let html = text_of(index).await;
    assert!(
        html.contains("Tide &lt;i&gt;tables&lt;/i&gt;") && !html.contains("<i>"),
        "{html}"
    );
    assert!(!html.contains("Private budget"));

    // A page carries a short cache time and an ETag, and a repeat is not modified.
    let read = anonymous(&app, &format!("/public/p/{slug}"), None).await;
    assert_eq!(read.status(), 200);
    assert_eq!(read.headers()["cache-control"], "public, max-age=60, must-revalidate");
    let etag = read.headers()["etag"].to_str().unwrap().to_owned();
    assert!(text_of(read).await.contains("The harbour opens at dawn."));
    assert_eq!(
        anonymous(&app, &format!("/public/p/{slug}"), Some(&etag))
            .await
            .status(),
        304
    );

    // Search covers published snapshots only, and counts nothing private.
    let found = text_of(anonymous(&app, "/public/search?q=harbour", None).await).await;
    assert!(found.contains("Tide") && !found.contains("Private budget"), "{found}");
    let none = text_of(anonymous(&app, "/public/search?q=budget", None).await).await;
    assert!(none.contains("Nothing is published."), "{none}");

    // Withdrawn, unknown and malformed addresses are the same answer, never cached.
    call("POST", format!("/v1/publications/{pid}/withdraw"), json!({})).await;
    let withdrawn = anonymous(&app, &format!("/public/p/{slug}"), None).await;
    let unknown = anonymous(&app, &format!("/public/p/{}", "0".repeat(32)), None).await;
    let malformed = anonymous(&app, "/public/p/not-a-slug", None).await;
    for response in [&withdrawn, &unknown, &malformed] {
        assert_eq!(response.status(), 404);
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    assert_eq!(text_of(withdrawn).await, text_of(unknown).await);
    assert!(text_of(anonymous(&app, "/public", None).await)
        .await
        .contains("Nothing is published."));

    // Off, the whole surface is not there.
    let off_dir = temp_data_dir("public-reading-off");
    let off = router(ServerConfig::for_test(off_dir.clone())).await.unwrap();
    assert_eq!(anonymous(&off, "/public", None).await.status(), 404);
    let _ = std::fs::remove_dir_all(off_dir);
    let _ = std::fs::remove_dir_all(dir);
}
