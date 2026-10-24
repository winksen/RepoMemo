use super::*;

async fn call(app: &axum::Router, method: &str, uri: &str, who: &str, body: Value) -> axum::response::Response {
    app.clone().oneshot(json_request(method, uri, who, body)).await.unwrap()
}

#[tokio::test]
async fn ownership_moves_between_members_and_cannot_be_left_behind() {
    let data_dir = temp_data_dir("ownership");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (_, _) = register_account(&app, "ownership-admin@example.com").await;
    let (owner, _) = register_account(&app, "ownership-owner@example.com").await;
    let (colleague, colleague_tokens) = register_account(&app, "ownership-colleague@example.com").await;
    let colleague_id = colleague_tokens["user"]["id"].as_str().unwrap().to_owned();
    let (_, outsider_tokens) = register_account(&app, "ownership-outsider@example.com").await;
    let outsider_id = outsider_tokens["user"]["id"].as_str().unwrap().to_owned();
    let workspace_id = create_workspace(&app, &owner).await;
    let organizations = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/organizations", &owner))
            .await
            .unwrap(),
    )
    .await;
    let organization_id = organizations[0]["id"].as_str().unwrap().to_owned();
    join(
        &app,
        &owner,
        "organizations",
        &organization_id,
        "ownership-colleague@example.com",
        "member",
    )
    .await;

    // Someone who is not in the organization cannot receive it; yourself neither.
    let outsider = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{workspace_id}/transfer-ownership"),
        &owner,
        json!({"user_id": outsider_id}),
    )
    .await;
    assert_eq!(outsider.status(), 400);
    let unknown = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{workspace_id}/transfer-ownership"),
        &owner,
        json!({"user_id": "x"}),
    )
    .await;
    assert_eq!(unknown.status(), 404, "an unknown account");

    // Hand the workspace over.
    let handed = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{workspace_id}/transfer-ownership"),
        &owner,
        json!({"user_id": colleague_id}),
    )
    .await;
    assert_eq!(handed.status(), 204);
    // The previous owner is an administrator now and cannot hand it on again.
    let again = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{workspace_id}/transfer-ownership"),
        &owner,
        json!({"user_id": colleague_id}),
    )
    .await;
    assert_eq!(again.status(), 403);
    let workspaces = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/workspaces", &colleague))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(workspaces[0]["role"], "owner");
    let workspaces = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/workspaces", &owner))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(workspaces[0]["role"], "admin");
    // The new owner cannot be removed from the organization while they own it.
    let removal = app
        .clone()
        .oneshot(auth_request(
            "DELETE",
            &format!("/v1/organizations/{organization_id}/members/{colleague_id}"),
            &owner,
        ))
        .await
        .unwrap();
    assert_eq!(removal.status(), 409);
    // The hand-over is in the workspace activity.
    let activity = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/activity"),
                &colleague,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(activity.to_string().contains("ownership_transferred"));

    // Organizations: owners only, to a member.
    let org_handover = call(
        &app,
        "POST",
        &format!("/v1/organizations/{organization_id}/transfer-ownership"),
        &colleague,
        json!({"user_id": colleague_id}),
    )
    .await;
    assert_eq!(org_handover.status(), 403, "not the organization owner");
    let org_handover = call(
        &app,
        "POST",
        &format!("/v1/organizations/{organization_id}/transfer-ownership"),
        &owner,
        json!({"user_id": colleague_id}),
    )
    .await;
    assert_eq!(org_handover.status(), 204);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn folders_move_files_are_handled_in_bulk_and_files_take_new_versions() {
    let data_dir = temp_data_dir("evidence-tools");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (_, _) = register_account(&app, "tools-admin@example.com").await;
    let (owner, _) = register_account(&app, "tools-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let folder = |name: &str, parent: Option<&str>| {
        let app = app.clone();
        let request = json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/folders"),
            &owner,
            json!({"name": name, "parent_id": parent}),
        );
        async move {
            response_json(app.oneshot(request).await.unwrap()).await["id"]
                .as_str()
                .unwrap()
                .to_owned()
        }
    };
    let a = folder("A", None).await;
    let b = folder("B", None).await;
    let c = folder("C", Some(&a)).await;

    // Moving a folder: into another, never into itself or below itself.
    let moved = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{workspace_id}/folders/{c}/move"),
        &owner,
        json!({"parent_id": b}),
    )
    .await;
    assert_eq!(moved.status(), 200);
    assert_eq!(response_json(moved).await["parent_id"], b);
    let cyclic = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{workspace_id}/folders/{b}/move"),
        &owner,
        json!({"parent_id": c}),
    )
    .await;
    assert_eq!(cyclic.status(), 400);
    let own = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{workspace_id}/folders/{b}/move"),
        &owner,
        json!({"parent_id": b}),
    )
    .await;
    assert_eq!(own.status(), 400);
    let _other_c = folder("C", None).await;
    let clash = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{workspace_id}/folders/{c}/move"),
        &owner,
        json!({"parent_id": null}),
    )
    .await;
    assert_eq!(clash.status(), 409, "a folder named C is already at the top");
    let top = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{workspace_id}/folders/{a}/move"),
        &owner,
        json!({"parent_id": null}),
    )
    .await;
    assert_eq!(top.status(), 200, "already there is fine");

    // Several files at once.
    let mut ids = Vec::new();
    for title in ["One", "Two", "Three"] {
        let created = call(
            &app,
            "POST",
            &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            &owner,
            json!({"title": title, "content": format!("{title} body")}),
        )
        .await;
        ids.push(response_json(created).await["id"].as_str().unwrap().to_owned());
    }
    let bulk_uri = format!("/v1/workspaces/{workspace_id}/artifacts/bulk");
    let moved = response_json(
        call(
            &app,
            "POST",
            &bulk_uri,
            &owner,
            json!({"action": "move", "artifact_ids": [ids[0], ids[1]], "folder_id": a}),
        )
        .await,
    )
    .await;
    assert_eq!(moved["done"].as_array().unwrap().len(), 2);
    let listing = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/artifacts"),
                &owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    let in_folder = listing
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| item["folder_id"] == a)
        .count();
    assert_eq!(in_folder, 2);
    let state = response_json(
        call(
            &app,
            "POST",
            &bulk_uri,
            &owner,
            json!({"action": "lifecycle", "artifact_ids": ids, "status": "verified"}),
        )
        .await,
    )
    .await;
    assert_eq!(state["done"].as_array().unwrap().len(), 3);
    let lifecycle = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/artifacts/{}/lifecycle", ids[2]),
                &owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(lifecycle["status"], "verified");
    let partial = response_json(
        call(
            &app,
            "POST",
            &bulk_uri,
            &owner,
            json!({"action": "delete", "artifact_ids": [ids[0], "missing"]}),
        )
        .await,
    )
    .await;
    assert_eq!(partial["done"].as_array().unwrap().len(), 1);
    assert_eq!(partial["failed"][0]["artifact_id"], "missing");
    assert_eq!(
        call(
            &app,
            "POST",
            &bulk_uri,
            &owner,
            json!({"action": "nonsense", "artifact_ids": ids})
        )
        .await
        .status(),
        400
    );
    assert_eq!(
        call(
            &app,
            "POST",
            &bulk_uri,
            &owner,
            json!({"action": "delete", "artifact_ids": []})
        )
        .await
        .status(),
        400
    );

    // A new version goes into the same artifact.
    let upload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                .header("authorization", &owner)
                .header("content-type", "text/markdown")
                .header("x-repomemo-filename", "guide.md")
                .body(Body::from("# Guide\n\nFirst edition.\n"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload.status(), 201);
    let guide = response_json(upload).await;
    let guide_id = guide["id"].as_str().unwrap().to_owned();
    let version = |name: &'static str, body: &'static str| {
        let app = app.clone();
        let request = Request::builder()
            .method("POST")
            .uri(format!("/v1/artifacts/{guide_id}/versions"))
            .header("authorization", &owner)
            .header("content-type", "text/markdown")
            .header("x-repomemo-filename", name)
            .body(Body::from(body))
            .unwrap();
        async move { app.oneshot(request).await.unwrap() }
    };
    let second = version("guide-v2.md", "# Guide\n\nSecond edition, with more.\n").await;
    assert_eq!(second.status(), 200);
    let second = response_json(second).await;
    assert_eq!(second["id"], guide_id, "the same artifact");
    assert_ne!(second["content_hash"], guide["content_hash"]);
    assert_eq!(version("guide.txt", "wrong kind").await.status(), 400);
    assert_eq!(
        version("guide-v3.md", "# Guide\n\nSecond edition, with more.\n")
            .await
            .status(),
        409
    );
    // The first edition is kept: its text stays readable and the two compare.
    let history = response_json(
        call(
            &app,
            "GET",
            &format!("/v1/artifacts/{guide_id}/versions"),
            &owner,
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(history.as_array().unwrap().len(), 2, "{history}");
    assert_eq!(history[0]["version"], 2);
    assert_eq!(history[0]["is_current"], true);
    assert_eq!(history[1]["version"], 1);
    assert_eq!(history[1]["is_current"], false);
    assert_eq!(history[0]["uploaded_by"], "Person");
    assert_eq!(history[0]["created_at"], history[1]["replaced_at"]);
    let first_text = response_json(
        call(
            &app,
            "GET",
            &format!("/v1/artifacts/{guide_id}/versions/1"),
            &owner,
            json!({}),
        )
        .await,
    )
    .await;
    assert!(
        first_text["text"].as_str().unwrap().contains("First edition."),
        "{first_text}"
    );
    let second_text = response_json(
        call(
            &app,
            "GET",
            &format!("/v1/artifacts/{guide_id}/versions/2"),
            &owner,
            json!({}),
        )
        .await,
    )
    .await;
    assert!(second_text["text"]
        .as_str()
        .unwrap()
        .contains("Second edition, with more."));
    let changes = response_json(
        call(
            &app,
            "GET",
            &format!("/v1/artifacts/{guide_id}/compare?from=1&to=2"),
            &owner,
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(
        (changes["added"].as_i64(), changes["removed"].as_i64()),
        (Some(1), Some(1)),
        "{changes}"
    );
    let line = |kind: &str| {
        changes["lines"]
            .as_array()
            .unwrap()
            .iter()
            .find(|line| line["kind"] == kind)
            .map(|line| line["text"].as_str().unwrap().to_owned())
    };
    assert_eq!(line("removed").as_deref(), Some("First edition."));
    assert_eq!(line("added").as_deref(), Some("Second edition, with more."));
    assert_eq!(
        call(
            &app,
            "GET",
            &format!("/v1/artifacts/{guide_id}/versions/9"),
            &owner,
            json!({})
        )
        .await
        .status(),
        404
    );
    let (outsider, _) = register_account(&app, "versions-outsider@example.com").await;
    for uri in [
        format!("/v1/artifacts/{guide_id}/versions"),
        format!("/v1/artifacts/{guide_id}/versions/1"),
        format!("/v1/artifacts/{guide_id}/compare?from=1&to=2"),
    ] {
        assert_eq!(
            call(&app, "GET", &uri, &outsider, json!({})).await.status(),
            403,
            "{uri}"
        );
    }
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 3).await;
    let hits = response_json(
        call(
            &app,
            "POST",
            &format!("/v1/workspaces/{workspace_id}/search"),
            &owner,
            json!({"query": "second edition"}),
        )
        .await,
    )
    .await;
    assert!(hits
        .as_array()
        .unwrap()
        .iter()
        .any(|hit| hit["artifact_id"] == guide_id));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn restoring_an_earlier_version_makes_a_new_version_with_its_text_and_keeps_the_history() {
    let data_dir = temp_data_dir("restore-version");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (_, _) = register_account(&app, "restore-admin@example.com").await;
    let (owner, _) = register_account(&app, "restore-owner@example.com").await;
    let (viewer, _) = register_account(&app, "restore-viewer@example.com").await;
    let (outsider, _) = register_account(&app, "restore-outsider@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "restore-viewer@example.com",
        "viewer",
    )
    .await;
    let upload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                .header("authorization", &owner)
                .header("content-type", "text/markdown")
                .header("x-repomemo-filename", "guide.md")
                .body(Body::from("# Guide\n\nFirst edition.\n"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload.status(), 201);
    let guide_id = response_json(upload).await["id"].as_str().unwrap().to_owned();
    for (name, body) in [
        ("guide-v2.md", "# Guide\n\nSecond edition.\n"),
        ("guide-v3.md", "# Guide\n\nThird edition.\n"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/artifacts/{guide_id}/versions"))
                    .header("authorization", &owner)
                    .header("content-type", "text/markdown")
                    .header("x-repomemo-filename", name)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }
    let restore = |version: i64, who: &str| {
        let request = json_request(
            "POST",
            &format!("/v1/artifacts/{guide_id}/versions/{version}/restore"),
            who,
            json!({}),
        );
        let app = app.clone();
        async move { app.oneshot(request).await.unwrap() }
    };
    let history = |who: String| {
        let request = auth_request("GET", &format!("/v1/artifacts/{guide_id}/versions"), &who);
        let app = app.clone();
        async move { response_json(app.oneshot(request).await.unwrap()).await }
    };

    // Restoring version 2 of three makes a fourth with version 2's text.
    let restored = restore(2, &owner).await;
    assert_eq!(restored.status(), 200);
    assert_eq!(response_json(restored).await["id"], guide_id, "the same file");
    let after = history(owner.clone()).await;
    let versions = after.as_array().unwrap();
    assert_eq!(versions.len(), 4, "the history keeps all four: {after}");
    assert_eq!(versions[0]["version"], 4);
    assert_eq!(versions[0]["is_current"], true);
    assert_eq!(versions[0]["uploaded_by"], "Person");
    assert_eq!(
        versions[0]["content_hash"], versions[2]["content_hash"],
        "version 4 holds version 2's content"
    );
    for (position, number) in [(1, 3), (2, 2), (3, 1)] {
        assert_eq!(versions[position]["version"], number);
        assert_eq!(versions[position]["is_current"], false);
    }
    let text_of = |number: i64| {
        let request = auth_request("GET", &format!("/v1/artifacts/{guide_id}/versions/{number}"), &owner);
        let app = app.clone();
        async move {
            response_json(app.oneshot(request).await.unwrap()).await["text"]
                .as_str()
                .unwrap()
                .to_owned()
        }
    };
    assert!(text_of(4).await.contains("Second edition."));
    assert!(
        text_of(3).await.contains("Third edition."),
        "the version it replaced is kept"
    );
    assert!(text_of(1).await.contains("First edition."));

    // The current version cannot be restored over itself, an unknown one is not found.
    assert_eq!(restore(4, &owner).await.status(), 409);
    assert_eq!(restore(9, &owner).await.status(), 404);
    // Restoring is changing the file: a viewer and an outsider are refused, and nothing moved.
    assert_eq!(restore(1, &viewer).await.status(), 403);
    assert_eq!(restore(1, &outsider).await.status(), 403);
    assert_eq!(history(owner.clone()).await.as_array().unwrap().len(), 4);
    // A version restored again is another new version.
    assert_eq!(restore(3, &owner).await.status(), 200);
    assert_eq!(history(owner.clone()).await.as_array().unwrap().len(), 5);
    assert!(text_of(5).await.contains("Third edition."));

    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 1).await;
    let hits = response_json(
        call(
            &app,
            "POST",
            &format!("/v1/workspaces/{workspace_id}/search"),
            &owner,
            json!({"query": "third edition"}),
        )
        .await,
    )
    .await;
    assert!(hits
        .as_array()
        .unwrap()
        .iter()
        .any(|hit| hit["artifact_id"] == guide_id));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn mentions_in_tasks_and_memory_cards_tell_workspace_members_once_and_nobody_else() {
    let data_dir = temp_data_dir("mentions-everywhere");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (_, _) = register_account(&app, "mentions-admin@example.com").await;
    let (owner, _) = register_account(&app, "mentions-owner@example.com").await;
    let (member, _) = register_account(&app, "mentions-member@example.com").await;
    let (orgmate, _) = register_account(&app, "mentions-orgmate@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let organizations = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/organizations", &owner))
            .await
            .unwrap(),
    )
    .await;
    let organization_id = organizations[0]["id"].as_str().unwrap().to_owned();
    join(
        &app,
        &owner,
        "organizations",
        &organization_id,
        "mentions-orgmate@example.com",
        "member",
    )
    .await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "mentions-member@example.com",
        "member",
    )
    .await;
    // The kinds of notice a person has, in alphabetical order.
    let kinds = |who: String| {
        let request = auth_request("GET", "/v1/notifications", &who);
        let app = app.clone();
        async move {
            let mut kinds = response_json(app.oneshot(request).await.unwrap())
                .await
                .as_array()
                .unwrap()
                .iter()
                .map(|notice| notice["notification_type"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            kinds.sort();
            kinds
        }
    };

    // A task whose description names a member, an organization member who is not in the workspace, and
    // its own author: only the member is told.
    let created = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{workspace_id}/tasks"),
        &owner,
        json!({
            "title": "Check the API",
            "description": "@mentions-member@example.com please look, @mentions-orgmate@example.com too, and @mentions-owner@example.com",
            "status": "open",
            "priority": "medium",
            "assignee_user_id": null,
            "artifact_id": null,
            "due_at": null
        }),
    )
    .await;
    assert_eq!(created.status(), 201);
    let task = response_json(created).await;
    let task_id = task["id"].as_str().unwrap().to_owned();
    assert_eq!(kinds(member.clone()).await, ["task_mention"]);
    assert!(
        kinds(orgmate.clone()).await.is_empty(),
        "not in the workspace: not told"
    );
    assert!(kinds(owner.clone()).await.is_empty(), "never the author");

    // Saving the task again without a new name tells nobody again; a new name tells only that person.
    let save = |description: &'static str| {
        let request = json_request(
            "PUT",
            &format!("/v1/tasks/{task_id}"),
            &owner,
            json!({
                "title": "Check the API",
                "description": description,
                "status": "open",
                "priority": "medium",
                "assignee_user_id": null,
                "artifact_id": null,
                "due_at": null
            }),
        );
        let app = app.clone();
        async move { app.oneshot(request).await.unwrap() }
    };
    assert_eq!(
        save("@mentions-member@example.com please look, and thanks")
            .await
            .status(),
        200
    );
    assert_eq!(
        kinds(member.clone()).await,
        ["task_mention"],
        "the same name is not told twice"
    );
    assert_eq!(
        save("@mentions-member@example.com please look, @mentions-owner@example.com")
            .await
            .status(),
        200
    );
    assert_eq!(kinds(member.clone()).await.len(), 1);

    // A memory card names the member; changing the text without a new name is quiet, a second card is not.
    let card = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{workspace_id}/memory-cards"),
        &owner,
        json!({
            "title": "Rule",
            "body_markdown": "Keep shared data on the API, says @mentions-member@example.com.",
            "source": "Flow",
            "confidence": null,
            "citations": []
        }),
    )
    .await;
    assert_eq!(card.status(), 201);
    let card_id = response_json(card).await["id"].as_str().unwrap().to_owned();
    assert_eq!(kinds(member.clone()).await, ["memory_mention", "task_mention"]);
    let edited = call(
        &app,
        "PUT",
        &format!("/v1/memory-cards/{card_id}"),
        &owner,
        json!({
            "title": "Rule",
            "body_markdown": "Keep shared data on the API, says @mentions-member@example.com. Edited.",
            "source": "Flow",
            "confidence": null,
            "citations": []
        }),
    )
    .await;
    assert_eq!(edited.status(), 200);
    assert_eq!(
        kinds(member.clone()).await.len(),
        2,
        "an edit that names nobody new is quiet"
    );
    assert!(kinds(orgmate).await.is_empty());

    // The notice leads to the task board and to the card, and the preference covers both.
    let notices = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/notifications", &member))
            .await
            .unwrap(),
    )
    .await;
    let hrefs = notices
        .as_array()
        .unwrap()
        .iter()
        .map(|notice| notice["href"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert!(
        hrefs.contains(&format!("/workspaces/{workspace_id}/tasks")),
        "{hrefs:?}"
    );
    assert!(
        hrefs.contains(&format!("/workspaces/{workspace_id}/memory-cards/{card_id}")),
        "{hrefs:?}"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_console_creates_reset_links_and_lists_tokens() {
    let data_dir = temp_data_dir("console-tools");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (admin, _) = register_account(&app, "console-tools-admin@example.com").await;
    let (_, _) = register_account(&app, "console-tools-person@example.com").await;
    let run = |command: &str| {
        let app = app.clone();
        let request = json_request("POST", "/v1/system/console", &admin, json!({"command": command}));
        async move { app.oneshot(request).await.unwrap() }
    };
    let dry = response_json(run("users reset-link console-tools-person@example.com").await).await;
    assert_eq!(dry["dry_run"], true);
    let applied = response_json(run("users reset-link console-tools-person@example.com --yes").await).await;
    assert_eq!(applied["dry_run"], false);
    assert!(applied["data"]["path"]
        .as_str()
        .unwrap()
        .starts_with("/reset-password#"));
    // The link works.
    let token = applied["data"]["token"].as_str().unwrap();
    let preview = app
        .clone()
        .oneshot(public_post("/v1/auth/password-reset/preview", json!({"token": token})))
        .await
        .unwrap();
    assert_eq!(preview.status(), 200);

    let created = call(
        &app,
        "POST",
        "/v1/profile/tokens",
        &admin,
        json!({"name": "CI", "scope": "read", "expires_in_days": 7}),
    )
    .await;
    assert_eq!(created.status(), 201);
    let listed = response_json(run("tokens list").await).await;
    assert_eq!(listed["data"][0]["name"], "CI");
    let others = response_json(run("tokens list --user console-tools-person@example.com").await).await;
    assert!(others["data"].as_array().unwrap().is_empty());
    // `after` reaches the logs command.
    assert_eq!(run("logs --after 0 --limit 5").await.status(), 200);
    assert_eq!(run("logs --after soon").await.status(), 400);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_console_starts_workspace_work_as_dry_runs_then_audited_jobs() {
    let data_dir = temp_data_dir("console-workspace");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (admin, _) = register_account(&app, "console-ws-admin@example.com").await;
    let workspace_id = create_workspace(&app, &admin).await;
    let run = |who: &str, command: String| {
        let app = app.clone();
        let request = json_request("POST", "/v1/system/console", who, json!({ "command": command }));
        async move { app.oneshot(request).await.unwrap() }
    };

    // Finding the workspace: by id from the list, then by name.
    let listed = response_json(run(&admin, "workspace list".to_owned()).await).await;
    assert_eq!(listed["command"], "workspace list");
    assert!(listed["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == workspace_id && row["name"] == "Workspace"));
    assert_eq!(
        run(&admin, "workspace reindex nowhere".to_owned()).await.status(),
        404,
        "an unknown workspace"
    );

    // A dry run says what would happen and starts nothing.
    let jobs_before = response_json(run(&admin, "jobs list".to_owned()).await).await["data"]
        .as_array()
        .unwrap()
        .len();
    let dry = response_json(run(&admin, "workspace reindex workspace".to_owned()).await).await;
    assert_eq!(dry["dry_run"], true);
    assert!(dry["summary"].as_str().unwrap().contains("Would re-index"));
    let jobs_after_dry = response_json(run(&admin, "jobs list".to_owned()).await).await["data"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(jobs_before, jobs_after_dry, "a dry run starts no job");

    // Confirmed, it answers with the job and leaves an audit line with the exact command.
    let started = response_json(run(&admin, format!("workspace reindex {workspace_id} --yes")).await).await;
    assert_eq!(started["dry_run"], false);
    assert_eq!(started["data"]["kind"], "indexing");
    assert_eq!(started["data"]["stage"], "chunking_workspace");

    // Repositories: none are linked, and an unknown one is not found.
    let repositories = response_json(run(&admin, "repo".to_owned()).await).await;
    assert_eq!(repositories["command"], "repo list");
    assert_eq!(repositories["data"].as_array().unwrap().len(), 0);
    assert_eq!(run(&admin, "repo sync nothing --yes".to_owned()).await.status(), 404);

    // A workspace without an embedding provider has nothing to rebuild with.
    assert_eq!(
        run(&admin, format!("embeddings rebuild {workspace_id}")).await.status(),
        409
    );

    let audit = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/system/audit", &admin))
            .await
            .unwrap(),
    )
    .await;
    let lines = audit
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["action"] == "console_command")
        .map(|event| event["detail"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 1, "only the confirmed command is audited: {lines:?}");
    assert!(
        lines[0].contains(&format!("workspace reindex {workspace_id} --yes")),
        "{lines:?}"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

/// V0.2.149: rights come from the artifact's summary, so a refused request
/// never starts a document reader (review E7a, P12).
#[tokio::test]
async fn a_refused_artifact_request_starts_no_document_reader() {
    let data_dir = temp_data_dir("refused-readings");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (_, _) = register_account(&app, "readings-admin@example.com").await;
    let (owner, _) = register_account(&app, "readings-owner@example.com").await;
    let (outsider, _) = register_account(&app, "readings-outsider@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;

    // A name no other test uses, so only this test's readings are counted.
    let name = format!("readings-{}.eml", uuid::Uuid::new_v4());
    let eml = "From: Ada <ada@example.com>\r\nTo: bob@example.com\r\nSubject: Plan\r\nContent-Type: text/plain\r\n\r\nPlease review.\r\n";
    let upload = |who: &str, uri: String| {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("authorization", who)
            .header("x-repomemo-filename", name.clone())
            .header("content-type", "message/rfc822")
            .body(Body::from(eml))
            .unwrap()
    };
    let uploaded = app
        .clone()
        .oneshot(upload(
            &owner,
            format!("/v1/workspaces/{workspace_id}/artifacts/upload"),
        ))
        .await
        .unwrap();
    assert_eq!(uploaded.status(), 201);
    let artifact_id = response_json(uploaded).await["id"].as_str().unwrap().to_owned();
    // The background indexing reads the file once; let it finish before counting.
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 1).await;
    let before = repomemo_api::recent_readings_of(&name);

    let artifact = format!("/v1/artifacts/{artifact_id}");
    let lifecycle =
        json!({"status": "verified", "owner_user_id": null, "review_note": "", "superseded_by_artifact_id": null});
    let routes: Vec<(&str, String, Value)> = vec![
        ("GET", artifact.clone(), json!(null)),
        ("PUT", artifact.clone(), json!({"title": "Taken"})),
        ("DELETE", artifact.clone(), json!(null)),
        ("PUT", format!("{artifact}/folder"), json!({"folder_id": null})),
        ("GET", format!("{artifact}/lifecycle"), json!(null)),
        ("PUT", format!("{artifact}/lifecycle"), lifecycle),
        ("GET", format!("{artifact}/lifecycle/history"), json!(null)),
        ("POST", format!("{artifact}/index"), json!({})),
        ("GET", format!("{artifact}/versions"), json!(null)),
        ("GET", format!("{artifact}/versions/1"), json!(null)),
        ("GET", format!("{artifact}/comments"), json!(null)),
        ("POST", format!("{artifact}/comments"), json!({"body": "hello"})),
        ("GET", format!("{artifact}/file"), json!(null)),
        ("GET", format!("{artifact}/document-preview"), json!(null)),
        ("POST", format!("{artifact}/open-link"), json!({})),
    ];
    for (method, uri, body) in routes {
        let answer = app
            .clone()
            .oneshot(json_request(method, &uri, &outsider, body))
            .await
            .unwrap();
        assert_eq!(answer.status(), 403, "{method} {uri} for a non-member");
    }
    let new_version = app
        .clone()
        .oneshot(upload(&outsider, format!("{artifact}/versions")))
        .await
        .unwrap();
    assert_eq!(new_version.status(), 403, "a new version from a non-member");
    assert_eq!(
        repomemo_api::recent_readings_of(&name),
        before,
        "a refused request started a reader"
    );

    // A member sees the text from the stored passages (V0.2.150), with no new reading.
    let shown = app
        .clone()
        .oneshot(auth_request("GET", &artifact, &owner))
        .await
        .unwrap();
    assert_eq!(shown.status(), 200);
    assert!(response_json(shown).await["content_preview"]
        .as_str()
        .is_some_and(|text| text.contains("Please review")));
    assert_eq!(repomemo_api::recent_readings_of(&name), before);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// V0.2.151: a job is cancelled by the person who started it or by an
/// administrator, not by any other writer (review S5-9).
#[tokio::test]
async fn a_job_is_cancelled_by_its_starter_or_an_administrator() {
    let data_dir = temp_data_dir("job-owners");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (_, _) = register_account(&app, "jobs-admin@example.com").await;
    let (owner, _) = register_account(&app, "jobs-owner@example.com").await;
    let (ada, _) = register_account(&app, "jobs-ada@example.com").await;
    let (bob, _) = register_account(&app, "jobs-bob@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "jobs-ada@example.com",
        "member",
    )
    .await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "jobs-bob@example.com",
        "member",
    )
    .await;

    let note = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
        &owner,
        json!({"title": "Queued note", "content": "Line one\nLine two", "language": "Markdown"}),
    )
    .await;
    assert_eq!(note.status(), 201);
    let artifact_id = response_json(note).await["id"].as_str().unwrap().to_owned();
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 1).await;

    // An administrator's re-index of the whole workspace is not a member's to stop.
    let whole = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{workspace_id}/index"),
        &owner,
        json!({}),
    )
    .await;
    assert_eq!(whole.status(), 202);
    let whole_job = response_json(whole).await["id"].as_str().unwrap().to_owned();
    let refused = call(&app, "POST", &format!("/v1/jobs/{whole_job}/cancel"), &ada, json!({})).await;
    assert_eq!(
        refused.status(),
        403,
        "a member cannot cancel an administrator's re-index"
    );

    // A member's own re-index of one file: theirs and an administrator's to stop, no other member's.
    let single = call(
        &app,
        "POST",
        &format!("/v1/artifacts/{artifact_id}/index"),
        &ada,
        json!({}),
    )
    .await;
    assert_eq!(single.status(), 202);
    let single_job = response_json(single).await["id"].as_str().unwrap().to_owned();
    let other_member = call(&app, "POST", &format!("/v1/jobs/{single_job}/cancel"), &bob, json!({})).await;
    assert_eq!(other_member.status(), 403, "another member cannot cancel it");
    let starter = call(&app, "POST", &format!("/v1/jobs/{single_job}/cancel"), &ada, json!({})).await;
    assert_eq!(starter.status(), 200, "the person who started it can");
    let administrator = call(
        &app,
        "POST",
        &format!("/v1/jobs/{single_job}/cancel"),
        &owner,
        json!({}),
    )
    .await;
    assert_eq!(administrator.status(), 200, "an administrator can");
    let _ = std::fs::remove_dir_all(data_dir);
}

/// V0.2.154: the file list is paged in SQL, with the total in `X-Total-Count`,
/// and the folder of each file on the page comes with it.
#[tokio::test]
async fn the_file_list_is_paged_in_sql_with_its_total() {
    let data_dir = temp_data_dir("paged-files");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "paged-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let folder = response_json(
        call(
            &app,
            "POST",
            &format!("/v1/workspaces/{workspace_id}/folders"),
            &owner,
            json!({"name": "Plans"}),
        )
        .await,
    )
    .await;
    let folder_id = folder["id"].as_str().unwrap().to_owned();
    for (index, title) in ["First", "Second", "Third", "Fourth"].iter().enumerate() {
        let note = call(
            &app,
            "POST",
            &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            &owner,
            json!({"title": title, "content": format!("Note {index}."), "folder_id": if index == 2 { json!(folder_id) } else { json!(null) }}),
        )
        .await;
        assert_eq!(note.status(), 201);
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    let everything = call(
        &app,
        "GET",
        &format!("/v1/workspaces/{workspace_id}/artifacts"),
        &owner,
        json!(null),
    )
    .await;
    assert_eq!(everything.status(), 200);
    assert_eq!(everything.headers()["x-total-count"], "4");
    assert!(everything.headers().get("x-repomemo-truncated").is_none());
    let everything = response_json(everything).await;
    let everything = everything.as_array().unwrap();
    assert_eq!(everything.len(), 4);

    // A page is a slice of the same order, with the total of all of them.
    let page = call(
        &app,
        "GET",
        &format!("/v1/workspaces/{workspace_id}/artifacts?limit=2&offset=1"),
        &owner,
        json!(null),
    )
    .await;
    assert_eq!(page.headers()["x-total-count"], "4");
    let page = response_json(page).await;
    let page = page.as_array().unwrap();
    assert_eq!(page.len(), 2);
    assert_eq!(page[0]["id"], everything[1]["id"]);
    assert_eq!(page[1]["id"], everything[2]["id"]);

    // The folder of a file comes with its page, and only a file in a folder has one.
    let in_folder: Vec<&Value> = everything
        .iter()
        .filter(|item| item["folder_id"] == folder_id)
        .collect();
    assert_eq!(in_folder.len(), 1);
    assert_eq!(in_folder[0]["title"], "Third");
    let past_the_end = call(
        &app,
        "GET",
        &format!("/v1/workspaces/{workspace_id}/artifacts?limit=10&offset=40"),
        &owner,
        json!(null),
    )
    .await;
    assert_eq!(past_the_end.headers()["x-total-count"], "4");
    assert!(response_json(past_the_end).await.as_array().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}
