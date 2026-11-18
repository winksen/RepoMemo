use super::*;

const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

fn files(guide: &str) -> Value {
    json!([
        {"path": "readme.md", "text": "# Home\nRead the [guide](docs/guide.md#start), see ![logo](img/logo.png), a [gone page](missing.md), [outside](../../etc/passwd), [web](https://example.com) and [[a wiki link]].\n"},
        {"path": "docs/guide.md", "text": guide},
        {"path": "img/logo.png", "base64": PNG},
        {"path": ".git/config", "text": "[core]"},
        {"path": "tool.exe", "base64": "AAAA"},
        {"path": "../escape.md", "text": "x"}
    ])
}

#[tokio::test]
async fn a_markdown_directory_is_previewed_mapped_and_imported_once() {
    let data_dir = temp_data_dir("directory-import");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "import-owner@example.com").await;
    let (stranger, _) = register_account(&app, "import-stranger@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |who: String, method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let uri = format!("/v1/workspaces/{workspace_id}/imports/directory");
    let count = || async {
        response_json(
            call(
                owner.clone(),
                "GET",
                format!("/v1/workspaces/{workspace_id}/artifacts"),
                json!({}),
            )
            .await,
        )
        .await
        .as_array()
        .unwrap()
        .len()
    };

    // Only someone who can write may import.
    let by_stranger = call(
        stranger,
        "POST",
        uri.clone(),
        json!({"source_key": "wiki", "files": files("# The Guide\nnarwhal\n")}),
    )
    .await;
    assert!(by_stranger.status() == 403 || by_stranger.status() == 404);

    // A dry run is the default: it explains every file and writes nothing.
    let preview = call(
        owner.clone(),
        "POST",
        uri.clone(),
        json!({"source_key": "wiki", "files": files("# The Guide\nnarwhal\n")}),
    )
    .await;
    assert_eq!(preview.status(), 200);
    let preview = response_json(preview).await;
    assert_eq!(preview["committed"], false);
    assert_eq!(preview["create"], 3, "{preview}");
    assert_eq!(preview["skipped"], 3);
    assert_eq!(preview["folders"], 2);
    assert_eq!(preview["links_resolved"], 2);
    let readme = &preview["entries"][0];
    assert_eq!(readme["links"]["missing"], 1, "{readme}");
    assert_eq!(readme["links"]["unsafe_links"], 1);
    assert_eq!(readme["links"]["external"], 1);
    assert_eq!(readme["links"]["wiki"], 1);
    let problems = preview["entries"].to_string();
    for expected in ["hidden file", "not supported", "climbs out"] {
        assert!(problems.contains(expected), "{expected}: {preview}");
    }
    assert_eq!(count().await, 0, "a dry run writes nothing");

    // One file with a problem refuses the import, unless the valid ones are asked for.
    let refused = call(
        owner.clone(),
        "POST",
        uri.clone(),
        json!({"source_key": "wiki", "dry_run": false, "files": files("# The Guide\nnarwhal\n")}),
    )
    .await;
    assert_eq!(refused.status(), 409);
    assert_eq!(count().await, 0);

    let done = call(
        owner.clone(),
        "POST",
        uri.clone(),
        json!({"source_key": "wiki", "dry_run": false, "skip_invalid": true, "files": files("# The Guide\nnarwhal\n")}),
    )
    .await;
    assert_eq!(done.status(), 200);
    let done = response_json(done).await;
    assert_eq!(done["committed"], true);
    assert_eq!(done["create"], 3);
    assert_eq!(count().await, 3);
    let id_of = |report: &Value, path: &str| {
        report["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["path"] == path)
            .unwrap()["artifact_id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let guide_id = id_of(&done, "docs/guide.md");
    let readme_id = id_of(&done, "readme.md");

    // The link to the guide now points at it in the app, with its position; links that did not resolve are left as written.
    let stored = call(
        owner.clone(),
        "GET",
        format!("/v1/artifacts/{readme_id}/file"),
        json!({}),
    )
    .await;
    assert_eq!(stored.status(), 200);
    let text = String::from_utf8(
        axum::body::to_bytes(stored.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        text.contains(&format!("(/workspaces/{workspace_id}/artifacts/{guide_id}#start)")),
        "{text}"
    );
    assert!(
        text.contains("(missing.md)") && text.contains("(https://example.com)"),
        "{text}"
    );

    // The guide is found by search once indexed.
    let mut found = 0;
    for _ in 0..100 {
        found = response_json(
            call(
                owner.clone(),
                "POST",
                format!("/v1/workspaces/{workspace_id}/search"),
                json!({"query": "narwhal", "limit": 5}),
            )
            .await,
        )
        .await
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
        if found > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(found > 0, "the imported text is searchable");

    // The same directory again changes nothing and makes nothing twice.
    let again = response_json(
        call(
            owner.clone(),
            "POST",
            uri.clone(),
            json!({"source_key": "wiki", "dry_run": false, "skip_invalid": true, "files": files("# The Guide\nnarwhal\n")}),
        )
        .await,
    )
    .await;
    assert_eq!(again["create"], 0, "{again}");
    assert_eq!(again["unchanged"], 3);
    assert_eq!(count().await, 3);

    // A changed file replaces the content in place: the same file, the same id.
    let changed = response_json(
        call(
            owner.clone(),
            "POST",
            uri,
            json!({"source_key": "wiki", "dry_run": false, "skip_invalid": true, "files": files("# The Guide\nbeluga\n")}),
        )
        .await,
    )
    .await;
    assert_eq!(changed["replace"], 1, "{changed}");
    assert_eq!(id_of(&changed, "docs/guide.md"), guide_id);
    assert_eq!(count().await, 3);
    let _ = std::fs::remove_dir_all(data_dir);
}

fn export() -> Value {
    json!([
        {"path": "index.html", "text": "<ul><li><a href=\"Home_1.html\">Home</a><ul><li><a href=\"Guide_2.html\">Guide</a></li></ul></li></ul>"},
        {"path": "Home_1.html", "text": "<html><head><title>Docs : Home</title></head><body><h1>Home</h1><p>Welcome to the manatee wiki.</p></body></html>"},
        {"path": "Guide_2.html", "text": "<html><head><title>Docs : Guide</title></head><body><h1>Guide</h1><p>Back to <a href=\"Home_1.html\">home</a>.</p><img src=\"attachments/2/pic.png\" alt=\"pic\"/><ac:structured-macro ac:name=\"toc\"></ac:structured-macro></body></html>"},
        {"path": "attachments/2/pic.png", "base64": PNG},
        {"path": "styles/site.css", "text": "body{}"}
    ])
}

#[tokio::test]
async fn a_confluence_html_export_keeps_its_tree_and_links_and_needs_the_restrictions_acknowledged() {
    let data_dir = temp_data_dir("confluence-import");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "confluence-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &owner, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let uri = format!("/v1/workspaces/{workspace_id}/imports/confluence");

    // No index, no import.
    let no_index = call(
        "POST",
        uri.clone(),
        json!({"source_key": "space", "files": [{"path": "Home_1.html", "text": "<p>x</p>"}]}),
    )
    .await;
    assert_eq!(no_index.status(), 400);

    // The dry run shows the tree, what did not translate and the restrictions that were not carried.
    let preview = response_json(
        call(
            "POST",
            uri.clone(),
            json!({"source_key": "space", "space_key": "DOCS", "files": export()}),
        )
        .await,
    )
    .await;
    assert_eq!(preview["committed"], false);
    assert_eq!(preview["create"], 3, "{preview}");
    assert_eq!(preview["links_resolved"], 2, "{preview}");
    let paths = preview["entries"].to_string();
    assert!(
        paths.contains("Home/Guide.md") && paths.contains("\"Home.md\""),
        "{paths}"
    );
    let notes = preview["notes"].to_string();
    assert!(notes.contains("toc"), "{notes}");
    assert!(notes.contains("no page restrictions"), "{notes}");

    // Committing needs the caller to accept that everyone in the workspace reads every page.
    let refused = call(
        "POST",
        uri.clone(),
        json!({"source_key": "space", "dry_run": false, "files": export()}),
    )
    .await;
    assert_eq!(refused.status(), 409);
    let listed = response_json(call("GET", format!("/v1/workspaces/{workspace_id}/artifacts"), json!({})).await).await;
    assert_eq!(listed.as_array().unwrap().len(), 0);

    let done = response_json(
        call(
            "POST",
            uri.clone(),
            json!({"source_key": "space", "space_key": "DOCS", "dry_run": false, "acknowledge_restrictions": true, "files": export()}),
        )
        .await,
    )
    .await;
    assert_eq!(done["committed"], true, "{done}");
    let id_of = |path: &str| {
        done["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["path"] == path)
            .unwrap()["artifact_id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let home = id_of("Home.md");
    let guide = id_of("Home/Guide.md");

    // The guide says where it came from, and its link to the home page now points at it.
    let text = String::from_utf8(
        axum::body::to_bytes(
            call("GET", format!("/v1/artifacts/{guide}/file"), json!({}))
                .await
                .into_body(),
            usize::MAX,
        )
        .await
        .unwrap()
        .to_vec(),
    )
    .unwrap();
    assert!(text.contains("Imported from Confluence space DOCS, page 2"), "{text}");
    assert!(
        text.contains(&format!("/workspaces/{workspace_id}/artifacts/{home}")),
        "{text}"
    );

    // Sent again, nothing is made twice.
    let again = response_json(
        call(
            "POST",
            uri,
            json!({"source_key": "space", "space_key": "DOCS", "dry_run": false, "acknowledge_restrictions": true, "files": export()}),
        )
        .await,
    )
    .await;
    assert_eq!(again["create"], 0, "{again}");
    assert_eq!(again["unchanged"], 3);
    let _ = std::fs::remove_dir_all(data_dir);
}
