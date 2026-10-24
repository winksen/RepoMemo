use super::*;

/// A workspace's earlier file versions and saved searches travel in its archive, and an import on another
/// workspace shows the same history and the same searches.
#[tokio::test]
async fn an_export_imported_elsewhere_shows_the_version_history_and_the_saved_searches() {
    let data_dir = temp_data_dir("archive-history");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (_, _) = register_account(&app, "history-admin@example.com").await;
    let (owner, _) = register_account(&app, "history-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let listed = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/workspaces", &owner))
            .await
            .unwrap(),
    )
    .await;
    let organization_id = listed[0]["organization_id"].as_str().unwrap().to_owned();

    // A file with three editions, and a saved search.
    let upload = |name: &'static str, uri: String, body: &'static str| {
        let app = app.clone();
        let request = Request::builder()
            .method("POST")
            .uri(uri)
            .header("authorization", &owner)
            .header("content-type", "text/markdown")
            .header("x-repomemo-filename", name)
            .body(Body::from(body))
            .unwrap();
        async move { app.oneshot(request).await.unwrap() }
    };
    let created = upload(
        "guide.md",
        format!("/v1/workspaces/{workspace_id}/artifacts/upload"),
        "# Guide\n\nFirst edition.\n",
    )
    .await;
    assert_eq!(created.status(), 201);
    let guide_id = response_json(created).await["id"].as_str().unwrap().to_owned();
    for (name, body) in [
        ("guide-2.md", "# Guide\n\nSecond edition, with more.\n"),
        ("guide-3.md", "# Guide\n\nThird edition, with even more words.\n"),
    ] {
        assert_eq!(
            upload(name, format!("/v1/artifacts/{guide_id}/versions"), body)
                .await
                .status(),
            200
        );
    }
    let saved = send(
        "POST",
        format!("/v1/workspaces/{workspace_id}/saved-searches"),
        &owner,
        json!({"name": "Editions", "query": "edition", "artifact_types": ["markdown_doc"], "languages": [], "source_ids": [], "result_limit": 20}),
    )
    .await;
    assert_eq!(saved.status(), 201);

    // The archive holds the two earlier editions' content, their records and the search.
    let archive = to_bytes(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/export"),
                &owner,
            ))
            .await
            .unwrap()
            .into_body(),
        usize::MAX,
    )
    .await
    .unwrap()
    .to_vec();
    let entries: std::collections::HashMap<String, Vec<u8>> =
        repomemo_api::read_archive_entries(archive.as_slice(), 1 << 26)
            .unwrap()
            .into_iter()
            .collect();
    let manifest: Value = serde_json::from_slice(&entries["manifest.json"]).unwrap();
    assert_eq!(manifest["format_version"], 2);
    assert_eq!(manifest["counts"]["artifacts"], 1);
    assert_eq!(manifest["counts"]["versions"], 2);
    assert_eq!(manifest["counts"]["saved_searches"], 1);
    assert_eq!(
        manifest["counts"]["blobs"], 3,
        "the current content and the two earlier editions"
    );
    let data: Value = serde_json::from_slice(&entries["workspace.json"]).unwrap();
    assert_eq!(data["versions"].as_array().unwrap().len(), 2);
    assert_eq!(data["versions"][0]["version"], 1);
    assert_eq!(data["saved_searches"][0]["name"], "Editions");
    // Nobody is named by account id.
    assert!(!String::from_utf8_lossy(&entries["workspace.json"]).contains("user_id"));

    // A dry run says so, and the import shows the same history and the same search.
    let import = |archive: Vec<u8>, query: &str| {
        let app = app.clone();
        let request = Request::builder()
            .method("POST")
            .uri(format!(
                "/v1/workspaces/import?organization_id={organization_id}{query}"
            ))
            .header("authorization", &owner)
            .header("content-type", "application/gzip")
            .body(Body::from(archive))
            .unwrap();
        async move { app.oneshot(request).await.unwrap() }
    };
    let dry = response_json(import(archive.clone(), "&dry_run=true").await).await;
    assert_eq!(dry["ok"], true, "{dry}");
    assert_eq!(dry["counts"]["versions"], 2);
    assert_eq!(dry["counts"]["saved_searches"], 1);
    let imported = import(archive.clone(), "&name=Moved").await;
    assert_eq!(imported.status(), 201);
    let new_workspace = response_json(imported).await["workspace"]["workspace"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let artifacts = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{new_workspace}/artifacts"),
                &owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    let new_guide = artifacts[0]["id"].as_str().unwrap().to_owned();
    assert_ne!(new_guide, guide_id);
    let history = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/artifacts/{new_guide}/versions"),
                &owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    let numbers: Vec<i64> = history
        .as_array()
        .unwrap()
        .iter()
        .map(|version| version["version"].as_i64().unwrap())
        .collect();
    assert_eq!(numbers, [3, 2, 1], "{history}");
    assert!(history
        .as_array()
        .unwrap()
        .iter()
        .all(|version| version["uploaded_by"].is_null() || version["uploaded_by"] == "Person"));
    for (number, words) in [(1, "First edition."), (2, "Second edition"), (3, "Third edition")] {
        let text = response_json(
            app.clone()
                .oneshot(auth_request(
                    "GET",
                    &format!("/v1/artifacts/{new_guide}/versions/{number}"),
                    &owner,
                ))
                .await
                .unwrap(),
        )
        .await;
        assert!(
            text["text"].as_str().unwrap().contains(words),
            "version {number}: {text}"
        );
    }
    let searches = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{new_workspace}/saved-searches"),
                &owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(searches.as_array().unwrap().len(), 1);
    assert_eq!(searches[0]["name"], "Editions");
    assert_eq!(searches[0]["query"], "edition");
    let _ = std::fs::remove_dir_all(data_dir);
}

/// An import in the background answers as soon as the archive is received, and its progress and result are read
/// from a job that only its owner can see.
#[tokio::test]
async fn an_import_can_run_in_the_background_and_report_its_progress_and_its_result() {
    let data_dir = temp_data_dir("import-background");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (_, _) = register_account(&app, "bg-admin@example.com").await;
    let (owner, _) = register_account(&app, "bg-owner@example.com").await;
    let (stranger, _) = register_account(&app, "bg-stranger@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let listed = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/workspaces", &owner))
            .await
            .unwrap(),
    )
    .await;
    let organization_id = listed[0]["organization_id"].as_str().unwrap().to_owned();
    for (title, content) in [
        ("Refund policy", "Refunds are issued within fourteen days of purchase."),
        ("Shipping notes", "Parcels leave the warehouse every weekday."),
    ] {
        let created = send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            &owner,
            json!({"title": title, "content": content}),
        )
        .await;
        assert_eq!(created.status(), 201);
    }
    let archive = to_bytes(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/export"),
                &owner,
            ))
            .await
            .unwrap()
            .into_body(),
        usize::MAX,
    )
    .await
    .unwrap()
    .to_vec();
    let import = |archive: Vec<u8>, who: &str, query: &str| {
        let app = app.clone();
        let request = Request::builder()
            .method("POST")
            .uri(format!(
                "/v1/workspaces/import?organization_id={organization_id}{query}"
            ))
            .header("authorization", who)
            .header("content-type", "application/gzip")
            .body(Body::from(archive))
            .unwrap();
        async move { app.oneshot(request).await.unwrap() }
    };
    let job = |id: String, who: String| {
        let app = app.clone();
        let request = auth_request("GET", &format!("/v1/workspaces/import/jobs/{id}"), &who);
        async move {
            let response = app.oneshot(request).await.unwrap();
            let status = response.status().as_u16();
            (status, response_json(response).await)
        }
    };
    let finished = |id: String, who: String| {
        let job = &job;
        async move {
            for _ in 0..200 {
                let (status, body) = job(id.clone(), who.clone()).await;
                assert_eq!(status, 200, "{body}");
                if body["status"] != "running" {
                    return body;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            panic!("the import did not finish");
        }
    };
    let count = || {
        let app = app.clone();
        let request = auth_request("GET", "/v1/workspaces", &owner);
        async move {
            response_json(app.oneshot(request).await.unwrap())
                .await
                .as_array()
                .unwrap()
                .len()
        }
    };

    // A dry run in the background: a job, then the report, and no workspace.
    let started = import(archive.clone(), &owner, "&dry_run=true&background=true").await;
    assert_eq!(started.status(), 202);
    let dry_id = response_json(started).await["job_id"].as_str().unwrap().to_owned();
    assert!(dry_id.len() >= 32);
    let dry = finished(dry_id.clone(), owner.clone()).await;
    assert_eq!(dry["status"], "done", "{dry}");
    assert_eq!(dry["stage"], "finished");
    assert_eq!(dry["report"]["dry_run"], true);
    assert_eq!(dry["report"]["counts"]["artifacts"], 2);
    assert!(dry["error"].is_null());
    assert_eq!(count().await, 1);
    // Only the person who started it can read it.
    assert_eq!(job(dry_id, stranger.clone()).await.0, 404);
    assert_eq!(job("not-a-job".to_owned(), owner.clone()).await.0, 404);

    // The real import: the same, and the workspace is there once the job is done.
    let started = import(archive.clone(), &owner, "&background=true&name=Moved").await;
    assert_eq!(started.status(), 202);
    let id = response_json(started).await["job_id"].as_str().unwrap().to_owned();
    let done = finished(id, owner.clone()).await;
    assert_eq!(done["status"], "done", "{done}");
    assert_eq!(done["report"]["workspace"]["workspace"]["name"], "Moved");
    assert_eq!(count().await, 2);

    // An archive that is not one fails the job with the reason, and leaves nothing.
    let started = import(b"hello".to_vec(), &owner, "&background=true").await;
    assert_eq!(started.status(), 202);
    let id = response_json(started).await["job_id"].as_str().unwrap().to_owned();
    let failed = finished(id, owner.clone()).await;
    assert_eq!(failed["status"], "failed", "{failed}");
    assert!(
        failed["error"]
            .as_str()
            .unwrap()
            .contains("not a readable workspace archive"),
        "{failed}"
    );
    assert!(failed["report"].is_null());
    assert_eq!(count().await, 2);

    // Without the option the answer still comes with the request, as before.
    let direct = import(archive, &owner, "&dry_run=true").await;
    assert_eq!(direct.status(), 200);
    assert_eq!(response_json(direct).await["dry_run"], true);
    let _ = std::fs::remove_dir_all(data_dir);
}
