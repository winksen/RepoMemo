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
async fn go_java_csharp_and_php_files_have_searchable_symbols() {
    let data_dir = temp_data_dir("language-symbols");
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
        (
            "Ledger.cs",
            "text/plain",
            "namespace App;\n\npublic class Ledger\n{\n    public int PostEntry(int amount) { return amount; }\n}\n",
        ),
        (
            "audit.php",
            "text/x-php",
            "<?php\n\nclass Audit\n{\n    public function recordEvent(string $name): string { return $name; }\n}\n",
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
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 4).await;
    for (query, symbol, path) in [
        ("FetchVal", "FetchValue", "store.go"),
        ("lookupEn", "lookupEntry", "Registry.java"),
        ("PostEnt", "PostEntry", "Ledger.cs"),
        ("recordEv", "recordEvent", "audit.php"),
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

#[tokio::test]
async fn the_review_queue_lists_what_waits_and_empties_as_it_is_resolved() {
    let data_dir = temp_data_dir("review-queue");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (_, _) = register_account(&app, "review-admin@example.com").await;
    let (owner, _) = register_account(&app, "review-owner@example.com").await;
    let (outsider, _) = register_account(&app, "review-outsider@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let mut ids = Vec::new();
    for (title, content) in [
        ("Runbook", "# Runbook\nRun the migration first."),
        ("Retry notes", "# Retry\nUploads use a backoff budget."),
        ("Copy one", "# Same\nIdentical text in two files."),
        ("Copy two", "# Same\nIdentical text in two files."),
    ] {
        let created = response_json(
            call(
                "POST",
                format!("/v1/workspaces/{workspace_id}/artifacts/text"),
                &owner,
                json!({"title": title, "content": content, "language": "Markdown"}),
            )
            .await,
        )
        .await;
        ids.push(created["id"].as_str().unwrap().to_owned());
    }
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 4).await;
    let card = call(
        "POST",
        format!("/v1/workspaces/{workspace_id}/memory-cards"),
        &owner,
        json!({
            "title": "Uploads back off",
            "body_markdown": "Decided in the retry review.",
            "source": "manual",
            "citations": [{"artifact_id": ids[1], "chunk_id": null, "title": "Retry notes", "path": "Retry notes", "start_line": null, "end_line": null, "confidence": null}]
        }),
    )
    .await;
    assert_eq!(card.status(), 201);
    let queue = |who: &str| {
        let uri = format!("/v1/workspaces/{workspace_id}/review");
        let request = auth_request("GET", &uri, who);
        let app = app.clone();
        async move { app.oneshot(request).await.unwrap() }
    };
    let kinds = |queue: &Value| {
        queue["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["kind"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };

    // Only the duplicate waits at first, and only for an administrator.
    let first = response_json(queue(&owner).await).await;
    assert_eq!(kinds(&first), ["health_finding"], "{first}");
    assert_eq!(first["finding_count"], 1);
    assert_eq!(first["items"][0]["resolver"], "admin");
    assert!(first["items"][0]["detail"].as_str().unwrap().contains("Copy"));

    // A file marked for review, and a card whose evidence was retired.
    let marked = call(
        "PUT",
        format!("/v1/artifacts/{}/lifecycle", ids[0]),
        &owner,
        json!({"status": "needs_review", "review_note": "Steps changed in 2.1"}),
    )
    .await;
    assert_eq!(marked.status(), 200);
    let retired = call(
        "PUT",
        format!("/v1/artifacts/{}/lifecycle", ids[1]),
        &owner,
        json!({"status": "outdated", "review_note": ""}),
    )
    .await;
    assert_eq!(retired.status(), 200);
    let waiting = response_json(queue(&owner).await).await;
    assert_eq!(waiting["evidence_count"], 1, "{waiting}");
    assert_eq!(waiting["memory_card_count"], 1, "{waiting}");
    let items = waiting["items"].as_array().unwrap();
    let evidence = items.iter().find(|item| item["kind"] == "evidence").unwrap();
    assert_eq!(evidence["title"], "Runbook");
    assert_eq!(evidence["reason"], "Steps changed in 2.1");
    assert_eq!(evidence["resolver"], "writer");
    assert_eq!(evidence["artifact_id"], json!(ids[0]));
    let stale = items.iter().find(|item| item["kind"] == "memory_card").unwrap();
    assert_eq!(stale["title"], "Uploads back off");
    assert!(stale["reason"].as_str().unwrap().contains("outdated"), "{stale}");
    // What has waited comes first; the finding, which has no date, last.
    assert_eq!(items.last().unwrap()["kind"], "health_finding");
    // The stale card is one item, not also a health finding.
    assert_eq!(waiting["finding_count"], 1, "{waiting}");

    // Someone outside the workspace sees nothing of it.
    assert_eq!(queue(&outsider).await.status(), 403);

    // Resolved where each is worked on, the queue empties.
    for id in [&ids[0], &ids[1]] {
        let restored = call(
            "PUT",
            format!("/v1/artifacts/{id}/lifecycle"),
            &owner,
            json!({"status": "verified", "review_note": ""}),
        )
        .await;
        assert_eq!(restored.status(), 200);
    }
    let health = response_json(
        call(
            "GET",
            format!("/v1/workspaces/{workspace_id}/health"),
            &owner,
            json!({}),
        )
        .await,
    )
    .await;
    let fingerprint = health["findings"][0]["fingerprint"].as_str().unwrap().to_owned();
    let dismissed = call(
        "POST",
        format!("/v1/workspaces/{workspace_id}/health/actions"),
        &owner,
        json!({"fingerprint": fingerprint, "action": "dismiss"}),
    )
    .await;
    assert_eq!(dismissed.status(), 200);
    let empty = response_json(queue(&owner).await).await;
    assert!(empty["items"].as_array().unwrap().is_empty(), "{empty}");
    assert_eq!(empty["evidence_count"], 0);
    assert_eq!(empty["memory_card_count"], 0);
    assert_eq!(empty["finding_count"], 0);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_snoozed_review_item_leaves_the_persons_list_and_the_count_until_it_is_brought_back() {
    let data_dir = temp_data_dir("review-snooze");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (_, _) = register_account(&app, "snooze-admin@example.com").await;
    let (owner, _) = register_account(&app, "snooze-owner@example.com").await;
    let (peer, _) = register_account(&app, "snooze-peer@example.com").await;
    let (outsider, _) = register_account(&app, "snooze-outsider@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "snooze-peer@example.com",
        "member",
    )
    .await;
    let call = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let queue = |who: &str| {
        let request = auth_request("GET", &format!("/v1/workspaces/{workspace_id}/review"), who);
        let app = app.clone();
        async move { response_json(app.oneshot(request).await.unwrap()).await }
    };
    let snooze_uri = format!("/v1/workspaces/{workspace_id}/review/snoozes");
    let mut ids = Vec::new();
    for title in ["Runbook", "Retry notes"] {
        let created = response_json(
            call(
                "POST",
                format!("/v1/workspaces/{workspace_id}/artifacts/text"),
                &owner,
                json!({"title": title, "content": format!("# {title}\nSomething to check."), "language": "Markdown"}),
            )
            .await,
        )
        .await;
        let id = created["id"].as_str().unwrap().to_owned();
        let marked = call(
            "PUT",
            format!("/v1/artifacts/{id}/lifecycle"),
            &owner,
            json!({"status": "needs_review", "review_note": format!("{title} may be stale.")}),
        )
        .await;
        assert_eq!(marked.status(), 200);
        ids.push(id);
    }

    let before = queue(&owner).await;
    assert_eq!(before["items"].as_array().unwrap().len(), 2, "{before}");
    assert_eq!(before["evidence_count"], 2);
    assert!(before["snoozed"].as_array().unwrap().is_empty());

    // A snoozed item leaves the list and the count, and waits in the snoozed list with its date.
    let until = (chrono::Utc::now() + chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();
    let snoozed = call(
        "PUT",
        snooze_uri.clone(),
        &owner,
        json!({"kind": "evidence", "id": ids[0], "until": until}),
    )
    .await;
    assert_eq!(snoozed.status(), 204);
    let during = queue(&owner).await;
    let waiting: Vec<_> = during["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert_eq!(waiting, [ids[1].as_str()], "{during}");
    assert_eq!(during["evidence_count"], 1);
    assert_eq!(during["snoozed"].as_array().unwrap().len(), 1);
    assert_eq!(during["snoozed"][0]["id"], ids[0]);
    assert!(during["snoozed"][0]["snoozed_until"]
        .as_str()
        .unwrap()
        .starts_with(&until));
    // It is the person's own list: a colleague still sees both.
    assert_eq!(queue(&peer).await["items"].as_array().unwrap().len(), 2);

    // A past time, more than a year ahead, nonsense, another kind and an outsider are refused.
    for body in [
        json!({"kind": "evidence", "id": ids[1], "until": "2020-01-01"}),
        json!({"kind": "evidence", "id": ids[1], "until": "2999-01-01"}),
        json!({"kind": "evidence", "id": ids[1], "until": "someday"}),
        json!({"kind": "task", "id": ids[1], "until": until}),
        json!({"kind": "evidence", "id": "", "until": until}),
    ] {
        assert_eq!(
            call("PUT", snooze_uri.clone(), &owner, body.clone()).await.status(),
            400,
            "{body}"
        );
    }
    assert_eq!(
        call(
            "PUT",
            snooze_uri.clone(),
            &outsider,
            json!({"kind": "evidence", "id": ids[1], "until": until}),
        )
        .await
        .status(),
        403
    );
    assert_eq!(
        queue(&owner).await["items"].as_array().unwrap().len(),
        1,
        "nothing else moved"
    );

    // Bringing it back is immediate and keeps no trace.
    let back = call(
        "DELETE",
        format!("{snooze_uri}?kind=evidence&id={}", ids[0]),
        &owner,
        json!({}),
    )
    .await;
    assert_eq!(back.status(), 204);
    let after = queue(&owner).await;
    assert_eq!(after["items"].as_array().unwrap().len(), 2, "{after}");
    assert_eq!(after["evidence_count"], 2);
    assert!(after["snoozed"].as_array().unwrap().is_empty());
    assert_eq!(
        call("DELETE", format!("{snooze_uri}?kind=nonsense&id=x"), &owner, json!({}))
            .await
            .status(),
        400
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
