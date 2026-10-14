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
