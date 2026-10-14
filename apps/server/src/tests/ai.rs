use super::*;

#[tokio::test]
async fn assistant_routes_messages_and_works_without_ai() {
    let data_dir = std::env::temp_dir().join(format!("repomemo-server-agent-{}", uuid::Uuid::new_v4()));
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let read = |response: axum::response::Response| async move {
        serde_json::from_slice::<Value>(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
    };

    let registered = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "agent@example.com", "display_name": "Agent", "password": "not-a-real-password"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let owner = format!("Bearer {}", read(registered).await["access_token"].as_str().unwrap());
    let organization = read(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/organizations",
                &owner,
                json!({"name":"Agent Team"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let workspace = read(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/workspaces",
                &owner,
                json!({"organization_id": organization["id"], "name":"Agent Workspace"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();
    for (title, content) in [
        ("Retry policy", "# Retries\nUploads use an exponential backoff budget."),
        (
            "Deploy runbook",
            "# Deploy\nRun the migration before switching traffic.",
        ),
    ] {
        let created = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
                &owner,
                json!({"title": title, "content": content, "language": "Markdown"}),
            ))
            .await
            .unwrap();
        assert_eq!(created.status(), 201);
    }
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 2).await;

    let capabilities = read(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/agent/capabilities"),
                &owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(capabilities["provider_name"], Value::Null);
    let available = capabilities["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|capability| capability["available"] == true)
        .map(|capability| capability["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(available, ["find_files", "find_symbol", "search_content"]);

    let send = |body: Value| {
        let app = app.clone();
        let uri = format!("/v1/workspaces/{workspace_id}/agent/messages");
        let owner = owner.clone();
        async move {
            let response = app.oneshot(json_request("POST", &uri, &owner, body)).await.unwrap();
            assert_eq!(response.status(), 200);
            read(response).await
        }
    };

    // The first message starts a conversation named after it.
    let first = send(json!({"message": "find the deploy runbook"})).await;
    let conversation_id = first["conversation"]["id"].as_str().unwrap().to_owned();
    assert_eq!(first["conversation"]["title"], "find the deploy runbook");
    let found = &first["turn"]["reply"];
    assert_eq!(found["capability"], "find_files");
    assert_eq!(found["routing"], "rules");
    assert_eq!(found["files"][0]["title"], "Deploy runbook");

    let in_chat = |body: Value| {
        let mut body = body;
        body["conversation_id"] = json!(conversation_id);
        send(body)
    };
    let searched = in_chat(json!({"message": "backoff", "capability": "search_content"})).await;
    assert_eq!(searched["turn"]["reply"]["routing"], "explicit");
    assert_eq!(searched["turn"]["reply"]["matches"][0]["title"], "Retry policy");

    // A file lookup with no matching name falls back to the indexed content.
    let fallback = in_chat(json!({"message": "find migration"})).await;
    assert_eq!(fallback["turn"]["reply"]["files"].as_array().unwrap().len(), 0);
    assert_eq!(fallback["turn"]["reply"]["matches"][0]["title"], "Deploy runbook");

    let summary = in_chat(json!({"message": "summarize the retry policy"})).await;
    assert_eq!(summary["turn"]["reply"]["capability"], "summarize_file");
    assert_eq!(summary["turn"]["reply"]["generated"], false);
    assert!(summary["turn"]["reply"]["reply_markdown"]
        .as_str()
        .unwrap()
        .contains("needs an AI provider"));

    let unmatched = in_chat(json!({"message": "write me a poem"})).await;
    assert_eq!(unmatched["turn"]["reply"]["capability"], Value::Null);
    assert_eq!(unmatched["turn"]["reply"]["routing"], "unmatched");
    assert_eq!(unmatched["turn"]["position"], 5);
    assert_eq!(unmatched["conversation"]["turn_count"], 5);

    // A second chat, then the history of both.
    let overview = send(json!({"capability": "workspace_overview"})).await;
    assert_eq!(overview["conversation"]["title"], "Workspace overview");
    assert_ne!(overview["conversation"]["id"], json!(conversation_id));
    let get = |uri: String, authorization: String| {
        let app = app.clone();
        async move { app.oneshot(auth_request("GET", &uri, &authorization)).await.unwrap() }
    };
    let chats = read(
        get(
            format!("/v1/workspaces/{workspace_id}/agent/conversations"),
            owner.clone(),
        )
        .await,
    )
    .await;
    assert_eq!(chats.as_array().unwrap().len(), 2);
    assert_eq!(chats[0]["title"], "Workspace overview");
    let history = read(get(format!("/v1/agent/conversations/{conversation_id}"), owner.clone()).await).await;
    let labels = history["turns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|turn| turn["label"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        [
            "find the deploy runbook",
            "backoff",
            "find migration",
            "summarize the retry policy",
            "write me a poem"
        ]
    );
    assert_eq!(history["turns"][1]["reply"]["matches"][0]["title"], "Retry policy");

    let renamed = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/agent/conversations/{conversation_id}"),
            &owner,
            json!({"title": "  Runbook   hunt "}),
        ))
        .await
        .unwrap();
    assert_eq!(read(renamed).await["title"], "Runbook hunt");

    // Another member of the workspace cannot see or continue these chats.
    let registered = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "agent-other@example.com", "display_name": "Other", "password": "not-a-real-password"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let other = format!("Bearer {}", read(registered).await["access_token"].as_str().unwrap());
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "agent-other@example.com",
        "member",
    )
    .await;
    let added = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/workspaces/{workspace_id}/members"),
            &owner,
            json!({"email": "agent-other@example.com", "role": "member"}),
        ))
        .await
        .unwrap();
    assert_eq!(added.status(), 200);
    let others_chats = read(
        get(
            format!("/v1/workspaces/{workspace_id}/agent/conversations"),
            other.clone(),
        )
        .await,
    )
    .await;
    assert_eq!(others_chats.as_array().unwrap().len(), 0);
    assert_eq!(
        get(format!("/v1/agent/conversations/{conversation_id}"), other.clone())
            .await
            .status(),
        404
    );
    let hijack = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/agent/messages"),
            &other,
            json!({"message": "find runbook", "conversation_id": conversation_id}),
        ))
        .await
        .unwrap();
    assert_eq!(hijack.status(), 404);

    let deleted = app
        .clone()
        .oneshot(auth_request(
            "DELETE",
            &format!("/v1/agent/conversations/{conversation_id}"),
            &owner,
        ))
        .await
        .unwrap();
    assert_eq!(deleted.status(), 204);
    assert_eq!(
        get(format!("/v1/agent/conversations/{conversation_id}"), owner.clone())
            .await
            .status(),
        404
    );

    let outsider = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/agent/messages"),
            "Bearer not-a-token",
            json!({"message": "find runbook"}),
        ))
        .await
        .unwrap();
    assert_eq!(outsider.status(), 401);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn embeddings_power_semantic_search_and_ask_gets_full_passages() {
    let data_dir = std::env::temp_dir().join(format!("repomemo-server-embeddings-{}", uuid::Uuid::new_v4()));
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (ollama_url, prompts) = start_fake_ollama().await;
    let read = |response: axum::response::Response| async move {
        let status = response.status();
        let body = serde_json::from_slice::<Value>(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
        assert!(status.is_success(), "{status}: {body}");
        body
    };
    let registered = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(json!({"email": "vectors@example.com", "display_name": "Vectors", "password": "not-a-real-password"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let owner = format!("Bearer {}", read(registered).await["access_token"].as_str().unwrap());
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let owner = owner.clone();
        async move { app.oneshot(json_request(method, &uri, &owner, body)).await.unwrap() }
    };
    let organization = read(call("POST", "/v1/organizations".to_owned(), json!({"name": "Vector Team"})).await).await;
    let workspace = read(
        call(
            "POST",
            "/v1/workspaces".to_owned(),
            json!({"organization_id": organization["id"], "name": "Vector Workspace"}),
        )
        .await,
    )
    .await;
    let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();
    let mut artifact_ids = Vec::new();
    for (title, content) in [
        (
            "Deploy runbook",
            "# Deploy\nRun the migration before switching traffic.",
        ),
        ("Retry policy", "# Retries\nUploads use an exponential backoff budget."),
        ("Release checklist", "# Release\nTag the build and announce the window."),
    ] {
        let created = read(
            call(
                "POST",
                format!("/v1/workspaces/{workspace_id}/artifacts/text"),
                json!({"title": title, "content": content, "language": "Markdown"}),
            )
            .await,
        )
        .await;
        artifact_ids.push(created["id"].as_str().unwrap().to_owned());
    }
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 3).await;
    read(call("POST", format!("/v1/workspaces/{workspace_id}/memory-cards"), json!({
        "title": "Uploads retry with backoff",
        "body_markdown": "Decided in the retry review.",
        "source": "manual",
        "citations": [{"artifact_id": artifact_ids[1], "chunk_id": null, "title": "Retry policy", "path": "Retry policy", "start_line": null, "end_line": null, "confidence": null}]
    })).await).await;

    // Without an embedding provider the map still shows indexing and memory links.
    let map = read(call("GET", format!("/v1/workspaces/{workspace_id}/knowledge-map"), json!({})).await).await;
    assert_eq!(map["pipeline"]["file_count"], 3);
    assert_eq!(map["pipeline"]["indexed_count"], 3);
    assert_eq!(map["pipeline"]["embedded_count"], Value::Null);
    assert_eq!(map["similarity_available"], false);
    let kinds = |map: &Value, kind: &str| {
        map["edges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|edge| edge["kind"] == kind)
            .cloned()
            .collect::<Vec<_>>()
    };
    let cites = kinds(&map, "cites");
    assert_eq!(cites.len(), 1);
    assert_eq!(cites[0]["target"], json!(artifact_ids[1]));

    let provider = |purpose: &str, model: &str| json!({"provider_type": "ollama", "name": format!("Mock {purpose}"), "base_url": ollama_url, "model": model, "enabled": true, "purpose": purpose});
    let embedder = read(
        call(
            "PUT",
            format!("/v1/workspaces/{workspace_id}/ai-providers"),
            provider("embedding", "mock-embed"),
        )
        .await,
    )
    .await;
    assert_eq!(embedder["purpose"], "embedding");

    // Existing chunks are embedded in the background once the provider is saved.
    let mut embedded = false;
    for _ in 0..100 {
        let metrics = read(
            app.clone()
                .oneshot(auth_request(
                    "GET",
                    &format!("/v1/workspaces/{workspace_id}/metrics"),
                    &owner,
                ))
                .await
                .unwrap(),
        )
        .await;
        if metrics["embedded_chunk_count"]
            .as_i64()
            .is_some_and(|count| count > 0 && count == metrics["chunk_count"].as_i64().unwrap())
        {
            embedded = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(embedded, "chunks were not embedded in time");

    let tested = read(
        call(
            "POST",
            format!(
                "/v1/workspaces/{workspace_id}/ai-providers/{}/test",
                embedder["id"].as_str().unwrap()
            ),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(tested["success"], true);
    assert!(tested["message"].as_str().unwrap().contains("3 dimensions"));

    // Embedded, the map links the two shipping documents and only those.
    let map = read(call("GET", format!("/v1/workspaces/{workspace_id}/knowledge-map"), json!({})).await).await;
    assert_eq!(map["similarity_available"], true);
    assert_eq!(map["pipeline"]["embedded_count"], map["pipeline"]["passage_count"]);
    let similar = kinds(&map, "similar");
    assert_eq!(similar.len(), 1, "{similar:?}");
    let pair = [
        similar[0]["source"].as_str().unwrap(),
        similar[0]["target"].as_str().unwrap(),
    ];
    assert!(pair.contains(&artifact_ids[0].as_str()) && pair.contains(&artifact_ids[2].as_str()));

    // "shipping" appears in no file, but means the same as "deploy".
    let results = read(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/search"),
            json!({"query": "shipping", "limit": 5}),
        )
        .await,
    )
    .await;
    let top_two = [
        results[0]["title"].as_str().unwrap(),
        results[1]["title"].as_str().unwrap(),
    ];
    assert!(
        top_two.contains(&"Deploy runbook") && top_two.contains(&"Release checklist"),
        "{results}"
    );

    read(
        call(
            "PUT",
            format!("/v1/workspaces/{workspace_id}/ai-providers"),
            provider("text", "mock-chat"),
        )
        .await,
    )
    .await;
    let answer = read(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/ask"),
            json!({"question": "How do we ship safely?"}),
        )
        .await,
    )
    .await;
    // Both shipping documents are equally relevant, so either may lead.
    assert!(
        ["Deploy runbook", "Release checklist"].contains(&answer["citations"][0]["title"].as_str().unwrap()),
        "{answer}"
    );
    assert!(answer["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .all(|warning| !warning.as_str().unwrap().contains("keyword search only")));
    let last_prompt = prompts.lock().unwrap().last().cloned().unwrap();
    assert!(
        last_prompt.contains("Run the migration before switching traffic."),
        "the model should see the full passage: {last_prompt}"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_ai_policy_and_quota_decide_who_may_call_the_provider() {
    let data_dir = temp_data_dir("ai-policy");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.ai_quota = crate::Quota::per_hour(2);
    let app = router(config).await.unwrap();
    let (ollama_url, prompts) = start_fake_ollama().await;
    let (owner, _) = register_account(&app, "ai-owner@example.com").await;
    let (viewer, _) = register_account(&app, "ai-viewer@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "ai-viewer@example.com",
        "viewer",
    )
    .await;
    let added = call(
        "PUT",
        format!("/v1/workspaces/{workspace_id}/members"),
        &owner,
        json!({"email": "ai-viewer@example.com", "role": "viewer"}),
    )
    .await;
    assert_eq!(added.status(), 200);
    let provider = call("PUT", format!("/v1/workspaces/{workspace_id}/ai-providers"), &owner, json!({"provider_type": "ollama", "name": "Mock", "base_url": ollama_url, "model": "mock-chat", "enabled": true, "purpose": "text"})).await;
    assert_eq!(provider.status(), 200);

    let policy = response_json(
        call(
            "GET",
            format!("/v1/workspaces/{workspace_id}/ai-policy"),
            &viewer,
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(policy["min_role"], "viewer", "everyone may use AI by default");
    let capabilities = response_json(
        call(
            "GET",
            format!("/v1/workspaces/{workspace_id}/capabilities"),
            &viewer,
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(capabilities["can_use_ai"], true);

    let refused = call(
        "PUT",
        format!("/v1/workspaces/{workspace_id}/ai-policy"),
        &viewer,
        json!({"min_role": "admin"}),
    )
    .await;
    assert_eq!(refused.status(), 403, "viewers cannot change the policy");
    let changed = call(
        "PUT",
        format!("/v1/workspaces/{workspace_id}/ai-policy"),
        &owner,
        json!({"min_role": "member"}),
    )
    .await;
    assert_eq!(changed.status(), 200);

    let capabilities = response_json(
        call(
            "GET",
            format!("/v1/workspaces/{workspace_id}/capabilities"),
            &viewer,
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(capabilities["can_use_ai"], false);
    assert_eq!(capabilities["can_generate_ai_overview"], false);
    let asked = call(
        "POST",
        format!("/v1/workspaces/{workspace_id}/ask"),
        &viewer,
        json!({"question": "How do we deploy?"}),
    )
    .await;
    assert_eq!(asked.status(), 403);
    assert!(response_json(asked).await["error"]["message"]
        .as_str()
        .unwrap()
        .contains("available to members"));
    let agent = response_json(
        call(
            "GET",
            format!("/v1/workspaces/{workspace_id}/agent/capabilities"),
            &viewer,
            json!({}),
        )
        .await,
    )
    .await;
    assert!(agent["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["requires_ai"] == true)
        .all(|entry| entry["available"] == false));
    let reply = call(
        "POST",
        format!("/v1/workspaces/{workspace_id}/agent/messages"),
        &viewer,
        json!({"message": "What is this workspace about?", "capability": "workspace_overview"}),
    )
    .await;
    assert_eq!(reply.status(), 200);
    assert!(response_json(reply).await["turn"]["reply"]["reply_markdown"]
        .as_str()
        .unwrap()
        .contains("available to members"));
    assert!(
        prompts.lock().unwrap().is_empty(),
        "nothing reached the provider for the viewer"
    );

    // The owner may, up to the hourly quota.
    for _ in 0..2 {
        let asked = call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/ask"),
            &owner,
            json!({"question": "How do we deploy?"}),
        )
        .await;
        assert_eq!(asked.status(), 200);
    }
    let limited = call(
        "POST",
        format!("/v1/workspaces/{workspace_id}/ask"),
        &owner,
        json!({"question": "How do we deploy?"}),
    )
    .await;
    assert_eq!(limited.status(), 429);
    assert!(limited.headers().contains_key("retry-after"));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn saved_provider_keys_only_go_to_the_address_they_were_saved_for() {
    let data_dir = temp_data_dir("provider-keys");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "keys@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let save = |body: Value| {
        let app = app.clone();
        let request = json_request(
            "PUT",
            &format!("/v1/workspaces/{workspace_id}/ai-providers"),
            &owner,
            body,
        );
        async move { app.oneshot(request).await.unwrap() }
    };
    let saved = save(json!({"provider_type": "openrouter", "name": "Cloud", "base_url": "https://openrouter.ai/api/v1", "model": "openai/gpt-4o-mini", "api_key": "sk-or-v1-secret", "enabled": true, "cloud_content_acknowledged": true, "purpose": "text"})).await;
    assert_eq!(saved.status(), 200);
    let saved = response_json(saved).await;
    assert!(saved.get("api_key").is_none(), "keys are never sent back");
    let id = saved["id"].as_str().unwrap().to_owned();

    let moved = save(json!({"id": id, "provider_type": "openrouter", "name": "Cloud", "base_url": "https://collector.example/api/v1", "model": "openai/gpt-4o-mini", "enabled": true, "purpose": "text"})).await;
    assert_eq!(moved.status(), 400);
    assert!(response_json(moved).await["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Enter the API key again"));
    let renamed = save(json!({"id": id, "provider_type": "openrouter", "name": "Cloud renamed", "base_url": "https://openrouter.ai/api/v1/", "model": "openai/gpt-4o-mini", "enabled": true, "purpose": "text"})).await;
    assert_eq!(
        renamed.status(),
        200,
        "the key is kept while the address stays the same"
    );

    let metadata = save(json!({"provider_type": "ollama", "name": "Metadata", "base_url": "http://169.254.169.254/latest", "model": "x", "enabled": true, "purpose": "vision"})).await;
    assert_eq!(metadata.status(), 400);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_console_rebuilds_a_workspaces_embeddings_after_a_dry_run() {
    let data_dir = temp_data_dir("console-embeddings");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (ollama_url, _) = start_fake_ollama().await;
    let (admin, _) = register_account(&app, "console-embed-admin@example.com").await;
    let workspace_id = create_workspace(&app, &admin).await;
    let run = |command: String| {
        let app = app.clone();
        let request = json_request("POST", "/v1/system/console", &admin, json!({ "command": command }));
        async move { app.oneshot(request).await.unwrap() }
    };
    for (title, content) in [
        (
            "Deploy runbook",
            "# Deploy\nRun the migration before switching traffic.",
        ),
        ("Retry policy", "# Retries\nUploads use an exponential backoff budget."),
    ] {
        let created = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
                &admin,
                json!({"title": title, "content": content, "language": "Markdown"}),
            ))
            .await
            .unwrap();
        assert_eq!(created.status(), 201);
    }
    wait_for_indexed_artifacts(&app, &admin, &workspace_id, 2).await;
    let saved = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/workspaces/{workspace_id}/ai-providers"),
            &admin,
            json!({"provider_type": "ollama", "name": "Mock embedding", "base_url": ollama_url, "model": "mock-embed", "enabled": true, "purpose": "embedding"}),
        ))
        .await
        .unwrap();
    assert_eq!(saved.status(), 200);
    let metrics = |app: axum::Router, admin: String, workspace_id: String| async move {
        response_json(
            app.oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/metrics"),
                &admin,
            ))
            .await
            .unwrap(),
        )
        .await
    };
    let wait_embedded = || async {
        for _ in 0..100 {
            let figures = metrics(app.clone(), admin.clone(), workspace_id.clone()).await;
            let (embedded, total) = (
                figures["embedded_chunk_count"].as_i64(),
                figures["chunk_count"].as_i64(),
            );
            if embedded.is_some_and(|count| count > 0 && Some(count) == total) {
                return total.unwrap();
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        panic!("chunks were not embedded in time");
    };
    let passages = wait_embedded().await;

    let dry = response_json(run(format!("embeddings rebuild {workspace_id}")).await).await;
    assert_eq!(dry["dry_run"], true);
    let text = dry["summary"].as_str().unwrap();
    assert!(
        text.contains(&format!("{passages} passage(s)")) && text.contains("mock-embed"),
        "{text}"
    );
    assert_eq!(
        metrics(app.clone(), admin.clone(), workspace_id.clone()).await["embedded_chunk_count"],
        passages,
        "a dry run removes nothing"
    );

    let applied = response_json(run(format!("embeddings rebuild {workspace_id} --yes")).await).await;
    assert_eq!(applied["dry_run"], false);
    assert_eq!(applied["data"]["removed"], passages);
    assert_eq!(wait_embedded().await, passages, "every passage is embedded again");
    let _ = std::fs::remove_dir_all(data_dir);
}
