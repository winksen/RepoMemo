use super::*;

#[tokio::test]
async fn memory_cards_say_when_their_evidence_changed_and_citations_can_be_edited() {
    let data_dir = temp_data_dir("memory-review");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let get = |uri: String, who: &str| {
        let app = app.clone();
        let request = auth_request("GET", &uri, who);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (_, _) = register_account(&app, "memory-admin@example.com").await;
    let (owner, _) = register_account(&app, "memory-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let stored = |title: &str, content: &str| {
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            &owner,
            json!({"title": title, "content": content}),
        )
    };
    let current =
        response_json(stored("Refund policy", "Refunds are issued within fourteen days of purchase.").await).await;
    let other = response_json(stored("Shipping notes", "Parcels leave the warehouse every weekday.").await).await;
    let current_id = current["id"].as_str().unwrap().to_owned();
    let other_id = other["id"].as_str().unwrap().to_owned();
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 2).await;

    // A card citing the policy.
    let hits = response_json(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/search"),
            &owner,
            json!({"query": "refunds"}),
        )
        .await,
    )
    .await;
    let chunk_id = hits[0]["chunk_id"].as_str().unwrap().to_owned();
    let created = send(
        "POST",
        format!("/v1/workspaces/{workspace_id}/memory-cards"),
        &owner,
        json!({
            "title": "Refund window", "body_markdown": "Refunds take fourteen days.", "source": "test",
            "citations": [{"artifact_id": current_id, "chunk_id": chunk_id, "title": "Refund policy", "path": "x", "start_line": null, "end_line": null, "confidence": null}],
        }),
    )
    .await;
    assert_eq!(created.status(), 201);
    let card_id = response_json(created).await["id"].as_str().unwrap().to_owned();
    let cards = response_json(get(format!("/v1/workspaces/{workspace_id}/memory-cards"), &owner).await).await;
    assert_eq!(cards[0]["needs_review"], false, "its evidence is as it was");
    let health = response_json(get(format!("/v1/workspaces/{workspace_id}/health"), &owner).await).await;
    assert!(health["findings"]
        .as_array()
        .unwrap()
        .iter()
        .all(|f| f["detector"] != "stale_memory_citation"));

    // The policy is retired: the card needs another look, and Health says so.
    assert_eq!(
        send(
            "PUT",
            format!("/v1/artifacts/{current_id}/lifecycle"),
            &owner,
            json!({"status": "outdated", "review_note": "Replaced by the new policy."}),
        )
        .await
        .status(),
        200
    );
    let cards = response_json(get(format!("/v1/workspaces/{workspace_id}/memory-cards"), &owner).await).await;
    assert_eq!(cards[0]["needs_review"], true);
    let detail = response_json(get(format!("/v1/memory-cards/{card_id}"), &owner).await).await;
    assert_eq!(detail["review"][0]["kind"], "retired");
    let health = response_json(get(format!("/v1/workspaces/{workspace_id}/health"), &owner).await).await;
    let finding = health["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["detector"] == "stale_memory_citation")
        .expect("a stale memory finding");
    assert!(finding["title"].as_str().unwrap().contains("Refund window"));

    // Search hides retired evidence unless a state is asked for.
    let search = |body: Value| {
        let response = send("POST", format!("/v1/workspaces/{workspace_id}/search"), &owner, body);
        async move { response_json(response.await).await }
    };
    assert_eq!(search(json!({"query": "refunds"})).await.as_array().unwrap().len(), 0);
    assert!(!search(json!({"query": "refunds", "lifecycle_statuses": ["outdated"]}))
        .await
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/search"),
            &owner,
            json!({"query": "refunds", "lifecycle_statuses": ["bogus"]}),
        )
        .await
        .status(),
        400
    );
    // Date and folder filters.
    assert!(search(json!({"query": "parcels", "modified_after": "2999-01-01"}))
        .await
        .as_array()
        .unwrap()
        .is_empty());
    assert!(search(json!({"query": "parcels", "modified_before": "2000-01-01"}))
        .await
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!search(json!({"query": "parcels", "modified_after": "2000-01-01"}))
        .await
        .as_array()
        .unwrap()
        .is_empty());
    let folder = response_json(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/folders"),
            &owner,
            json!({"name": "Empty", "parent_id": null}),
        )
        .await,
    )
    .await;
    let folder_id = folder["id"].as_str().unwrap();
    assert!(search(json!({"query": "parcels", "folder_id": folder_id}))
        .await
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/search"),
            &owner,
            json!({"query": "parcels", "modified_after": "not a date"}),
        )
        .await
        .status(),
        400
    );

    // Re-pointing the card at current evidence clears the flag.
    let updated = send(
        "PUT",
        format!("/v1/memory-cards/{card_id}"),
        &owner,
        json!({
            "title": "Refund window", "body_markdown": "Refunds take fourteen days.", "source": "test",
            "citations": [{"artifact_id": other_id, "chunk_id": null, "title": "Shipping notes", "path": "x", "start_line": null, "end_line": null, "confidence": null}],
        }),
    )
    .await;
    assert_eq!(updated.status(), 200);
    let detail = response_json(get(format!("/v1/memory-cards/{card_id}"), &owner).await).await;
    assert_eq!(detail["review"].as_array().unwrap().len(), 0);
    assert_eq!(detail["evidence"].as_array().unwrap().len(), 1);
    assert_eq!(detail["evidence"][0]["artifact_id"], other_id);
    // A citation from another workspace is refused.
    let refused = send(
        "PUT",
        format!("/v1/memory-cards/{card_id}"),
        &owner,
        json!({
            "title": "Refund window", "body_markdown": "Refunds take fourteen days.", "source": "test",
            "citations": [{"artifact_id": "not-in-this-workspace", "chunk_id": null, "title": "x", "path": "x", "start_line": null, "end_line": null, "confidence": null}],
        }),
    )
    .await;
    assert_eq!(refused.status(), 400);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn go_and_java_files_have_searchable_symbols_and_declaration_chunks() {
    let data_dir = temp_data_dir("go-java-symbols");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (_, _) = register_account(&app, "gojava-admin@example.com").await;
    let (owner, _) = register_account(&app, "gojava-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    for (name, mime, body) in [
        (
            "store.go",
            "text/x-go",
            "package store\n\n// Store keeps values.\ntype Store struct{}\n\nfunc (s *Store) FetchValue(key string) string {\n\treturn key\n}\n",
        ),
        (
            "Registry.java",
            "text/x-java-source",
            "package app;\n\npublic class Registry {\n    public String lookupEntry(String key) {\n        return key;\n    }\n}\n",
        ),
    ] {
        let upload = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                    .header("authorization", &owner)
                    .header("content-type", mime)
                    .header("x-repomemo-filename", name)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(upload.status(), 201, "{name}");
    }
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 2).await;
    for (query, symbol, path) in [
        ("FetchVal", "FetchValue", "store.go"),
        ("lookupEn", "lookupEntry", "Registry.java"),
    ] {
        let found = response_json(
            app.clone()
                .oneshot(auth_request(
                    "GET",
                    &format!("/v1/workspaces/{workspace_id}/symbols?q={query}"),
                    &owner,
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(found[0]["symbol"]["name"], symbol, "{found}");
        assert_eq!(found[0]["symbol"]["kind"], "method", "{found}");
        assert_eq!(found[0]["path"], path);
    }
    // The type is a symbol of its own, named by the declaration's chunk.
    let types = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/symbols?q=Registry"),
                &owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(types[0]["symbol"]["kind"], "class");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn code_symbols_are_searchable_over_the_api() {
    let data_dir = temp_data_dir("symbols");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (_, _) = register_account(&app, "symbols-admin@example.com").await;
    let (owner, _) = register_account(&app, "symbols-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let upload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                .header("authorization", &owner)
                .header("content-type", "text/x-rust")
                .header("x-repomemo-filename", "config.rs")
                .body(Body::from("pub fn parse_config() -> u32 {\n    1\n}\n"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload.status(), 201);
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 1).await;
    let found = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/symbols?q=parse_conf"),
                &owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(found[0]["symbol"]["name"], "parse_config");
    assert_eq!(found[0]["path"], "config.rs");
    let nothing = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/symbols?q=missing_name"),
                &owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(nothing.as_array().unwrap().is_empty());
    // The assistant answers "where is … defined" from the same table.
    let reply = response_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/agent/messages"),
                &owner,
                json!({"message": "where is parse_config defined?"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let text = reply.to_string();
    assert!(text.contains("find_symbol") && text.contains("parse_config"), "{text}");
    let _ = std::fs::remove_dir_all(data_dir);
}
