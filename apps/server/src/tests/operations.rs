use super::*;

async fn upload(app: &axum::Router, who: &str, workspace_id: &str, name: &str, bytes: Vec<u8>) -> u16 {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                .header("authorization", who)
                .header("x-repomemo-filename", name)
                .header("content-type", "text/plain")
                .body(Body::from(bytes))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
        .as_u16()
}

async fn wait_for_backup_work(app: &axum::Router, admin: &str) -> Value {
    for _ in 0..200 {
        let overview = response_json(
            app.clone()
                .oneshot(auth_request("GET", "/v1/system/backups", admin))
                .await
                .unwrap(),
        )
        .await;
        if overview["status"]["running"] == false && overview["status"]["succeeded"].is_boolean() {
            return overview;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("the backup did not finish in time");
}

#[tokio::test]
async fn a_backup_restores_into_a_new_environment_that_opens() {
    let data_dir = temp_data_dir("backup");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (admin, _) = register_account(&app, "backup-admin@example.com").await;
    let (member, _) = register_account(&app, "backup-member@example.com").await;
    let workspace_id = create_workspace(&app, &admin).await;
    for (name, text) in [("plan.txt", "Ship on Friday"), ("notes.txt", "Rollback plan")] {
        assert_eq!(
            upload(&app, &admin, &workspace_id, name, text.as_bytes().to_vec()).await,
            201
        );
    }

    // Only system administrators back up.
    let denied = app
        .clone()
        .oneshot(auth_request("GET", "/v1/system/backups", &member))
        .await
        .unwrap();
    assert_eq!(denied.status(), 403);

    let started = app
        .clone()
        .oneshot(json_request("POST", "/v1/system/backups", &admin, json!({})))
        .await
        .unwrap();
    assert_eq!(started.status(), 202);
    let overview = wait_for_backup_work(&app, &admin).await;
    assert_eq!(overview["status"]["succeeded"], true, "{overview}");
    let backups = overview["backups"].as_array().unwrap();
    assert_eq!(backups.len(), 1);
    let name = backups[0]["name"].as_str().unwrap().to_owned();
    assert_eq!(backups[0]["blob_count"], 2);
    assert_eq!(backups[0]["includes_key"], true);
    assert!(data_dir.join("backups").join(&name).join("repomemo.sqlite").is_file());

    // A second backup reuses the files of the first (names carry the second).
    tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
    app.clone()
        .oneshot(json_request("POST", "/v1/system/backups", &admin, json!({})))
        .await
        .unwrap();
    let overview = {
        let mut latest = Value::Null;
        for _ in 0..200 {
            latest = wait_for_backup_work(&app, &admin).await;
            if latest["backups"].as_array().unwrap().len() == 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        latest
    };
    assert_eq!(overview["backups"].as_array().unwrap().len(), 2);
    assert_eq!(overview["backups"][0]["reused_blobs"], 2);

    // Restore into a new environment folder, which must open.
    let environment = format!("restore-test-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
    let bad_name = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/system/backups/{name}/restore"),
            &admin,
            json!({"environment": "../escape"}),
        ))
        .await
        .unwrap();
    assert_eq!(bad_name.status(), 400);
    let unknown = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/system/backups/backup-19990101-000000/restore",
            &admin,
            json!({"environment": environment}),
        ))
        .await
        .unwrap();
    assert_eq!(unknown.status(), 404);
    let restore = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/system/backups/{name}/restore"),
            &admin,
            json!({"environment": environment}),
        ))
        .await
        .unwrap();
    assert_eq!(restore.status(), 202);
    let overview = loop {
        let overview = wait_for_backup_work(&app, &admin).await;
        if overview["status"]["operation"] == "restore" {
            break overview;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    };
    assert_eq!(overview["status"]["succeeded"], true, "{overview}");
    let message = overview["status"]["message"].as_str().unwrap();
    assert!(message.contains("1 workspaces, 2 files, 2 people"), "{message}");
    let root = crate::environment::environments_root().unwrap();
    let restored = root.join(&environment);
    assert!(restored.join("repomemo.sqlite").is_file());
    assert!(restored.join("blobs").is_dir());
    let _ = std::fs::remove_dir_all(&restored);
    let _ = std::fs::remove_dir(&root);

    // Delete one.
    let deleted = app
        .clone()
        .oneshot(json_request(
            "DELETE",
            &format!("/v1/system/backups/{name}"),
            &admin,
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(deleted.status(), 204);
    let overview = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/system/backups", &admin))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(overview["backups"].as_array().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

fn files_under(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(folder) = pending.pop() {
        for entry in std::fs::read_dir(&folder).unwrap().flatten() {
            if entry.file_type().unwrap().is_dir() {
                pending.push(entry.path());
            } else {
                found.push(entry.path());
            }
        }
    }
    found
}

#[tokio::test]
async fn an_encrypted_backup_holds_nothing_readable_and_restores_only_with_its_key() {
    let data_dir = temp_data_dir("backup-sealed");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.backup_key = Some("a backup passphrase that is long enough".to_owned());
    let app = router(config).await.unwrap();
    let (admin, _) = register_account(&app, "sealed-admin@example.com").await;
    let workspace_id = create_workspace(&app, &admin).await;
    for (name, text) in [("plan.txt", "Ship on Friday, quietly"), ("notes.txt", "Rollback plan for the release")] {
        assert_eq!(
            upload(&app, &admin, &workspace_id, name, text.as_bytes().to_vec()).await,
            201
        );
    }
    let start = || {
        let app = app.clone();
        let request = json_request("POST", "/v1/system/backups", &admin, json!({}));
        async move { app.oneshot(request).await.unwrap() }
    };
    assert_eq!(start().await.status(), 202);
    let overview = wait_for_backup_work(&app, &admin).await;
    assert_eq!(overview["status"]["succeeded"], true, "{overview}");
    assert_eq!(overview["encryption_enabled"], true);
    let backup = &overview["backups"][0];
    let name = backup["name"].as_str().unwrap().to_owned();
    assert_eq!(backup["encryption"]["cipher"], "aes-256-gcm");
    assert_eq!(backup["encryption"]["kdf"], "pbkdf2-hmac-sha256");
    assert_eq!(backup["includes_key"], true);
    assert!(!backup.to_string().contains("passphrase"), "the manifest never holds the key");

    // Every file but the manifest is sealed: names say so and no text shows.
    let folder = data_dir.join("backups").join(&name);
    assert!(folder.join("repomemo.sqlite.enc").is_file());
    assert!(folder.join("secret.key.enc").is_file());
    assert!(!folder.join("repomemo.sqlite").exists());
    assert!(!folder.join("secret.key").exists());
    let files = files_under(&folder);
    for file in files.iter().filter(|file| file.file_name().unwrap() != "manifest.json") {
        assert!(file.to_string_lossy().ends_with(".enc"), "{}", file.display());
        let bytes = std::fs::read(file).unwrap();
        for secret in ["Ship on Friday", "Rollback plan", "SQLite format 3"] {
            assert!(
                !bytes.windows(secret.len()).any(|window| window == secret.as_bytes()),
                "{} shows {secret}",
                file.display()
            );
        }
    }

    // The next backup keeps the same key material, so unchanged files are shared.
    tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
    start().await;
    let overview = {
        let mut latest = Value::Null;
        for _ in 0..200 {
            latest = wait_for_backup_work(&app, &admin).await;
            if latest["backups"].as_array().unwrap().len() == 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        latest
    };
    assert_eq!(overview["backups"][0]["reused_blobs"], 2);
    assert_eq!(
        overview["backups"][0]["encryption"]["salt"],
        overview["backups"][1]["encryption"]["salt"]
    );

    // A key that does not match is refused before any folder is made.
    let restore = |environment: String| {
        let app = app.clone();
        let request = json_request(
            "POST",
            &format!("/v1/system/backups/{name}/restore"),
            &admin,
            json!({"environment": environment}),
        );
        async move { app.oneshot(request).await.unwrap() }
    };
    let manifest_path = folder.join("manifest.json");
    let genuine = std::fs::read_to_string(&manifest_path).unwrap();
    let mut forged: Value = serde_json::from_str(&genuine).unwrap();
    forged["encryption"]["key_check"] = json!("0".repeat(64));
    std::fs::write(&manifest_path, serde_json::to_vec(&forged).unwrap()).unwrap();
    let environment = format!("sealed-test-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
    assert_eq!(restore(environment.clone()).await.status(), 202);
    let refused = loop {
        let overview = wait_for_backup_work(&app, &admin).await;
        if overview["status"]["operation"] == "restore" {
            break overview;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    };
    assert_eq!(refused["status"]["succeeded"], false, "{refused}");
    assert!(refused["status"]["message"].as_str().unwrap().contains("different key"), "{refused}");
    let root = crate::environment::environments_root().unwrap();
    assert!(!root.join(&environment).exists());
    assert!(!root.join(format!("{environment}.restoring")).exists());

    // The right key restores it into an environment that opens.
    std::fs::write(&manifest_path, genuine).unwrap();
    assert_eq!(restore(environment.clone()).await.status(), 202);
    let restored = loop {
        let overview = wait_for_backup_work(&app, &admin).await;
        if overview["status"]["operation"] == "restore" && overview["status"]["succeeded"] == true {
            break overview;
        }
        assert_ne!(overview["status"]["succeeded"], false, "{overview}");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    };
    let message = restored["status"]["message"].as_str().unwrap();
    assert!(message.contains("1 workspaces, 2 files, 1 people"), "{message}");
    let environment_folder = root.join(&environment);
    assert!(environment_folder.join("repomemo.sqlite").is_file());
    assert!(environment_folder.join("secret.key").is_file());
    let _ = std::fs::remove_dir_all(&environment_folder);
    let _ = std::fs::remove_dir(&root);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn each_backup_is_copied_to_a_second_place_and_checked() {
    let data_dir = temp_data_dir("backup-copy");
    let copies = temp_data_dir("backup-copy-elsewhere").join("share");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.backup_copy_dir = Some(copies.clone());
    let app = router(config).await.unwrap();
    let (admin, _) = register_account(&app, "copy-admin@example.com").await;
    let workspace_id = create_workspace(&app, &admin).await;
    for (name, text) in [("plan.txt", "Ship on Friday"), ("notes.txt", "Rollback plan")] {
        assert_eq!(upload(&app, &admin, &workspace_id, name, text.as_bytes().to_vec()).await, 201);
    }
    let start = |path: &'static str| {
        let app = app.clone();
        let request = json_request("POST", path, &admin, json!({}));
        async move { app.oneshot(request).await.unwrap() }
    };
    assert_eq!(start("/v1/system/backups").await.status(), 202);
    let overview = wait_for_backup_work(&app, &admin).await;
    assert_eq!(overview["status"]["succeeded"], true, "{overview}");
    let message = overview["status"]["message"].as_str().unwrap();
    assert!(message.contains("Copied 1 backup to") && message.contains("and checked it"), "{message}");
    assert!(overview["copy_directory"].as_str().unwrap().contains("share"));
    let backup = &overview["backups"][0];
    let name = backup["name"].as_str().unwrap().to_owned();
    assert!(backup["files"].as_u64().unwrap() >= 4, "database, key and two files");
    assert_eq!(backup["copy"]["files"], backup["files"]);
    assert_eq!(backup["copy"]["bytes"], backup["bytes"]);

    // The copy is whole: the same files, their checksums and the manifest.
    let copy = copies.join(&name);
    assert!(copy.join("manifest.json").is_file());
    assert!(copy.join("repomemo.sqlite").is_file());
    let sums = std::fs::read_to_string(copy.join("SHA256SUMS")).unwrap();
    assert_eq!(sums.lines().count() as u64, backup["files"].as_u64().unwrap());
    assert!(sums.lines().all(|line| line.split_once("  ").is_some_and(|(hash, _)| hash.len() == 64)));
    assert_eq!(
        std::fs::read(copy.join("repomemo.sqlite")).unwrap(),
        std::fs::read(data_dir.join("backups").join(&name).join("repomemo.sqlite")).unwrap()
    );

    // The next one links what has not changed to the first copy.
    tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
    start("/v1/system/backups").await;
    let overview = {
        let mut latest = Value::Null;
        for _ in 0..200 {
            latest = wait_for_backup_work(&app, &admin).await;
            if latest["backups"].as_array().unwrap().len() == 2 && latest["backups"][0]["copy"].is_object() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        latest
    };
    assert_eq!(overview["status"]["succeeded"], true, "{overview}");
    assert!(overview["backups"][0]["copy"]["linked"].as_u64().unwrap() >= 2, "{overview}");
    assert_eq!(std::fs::read_dir(&copies).unwrap().count(), 2);

    // A destination that cannot be used is a failure to see, and the backup stays.
    let blocked = temp_data_dir("backup-copy-blocked");
    let blocker = blocked.join("not-a-folder");
    std::fs::write(&blocker, b"a file").unwrap();
    let broken_data = temp_data_dir("backup-copy-broken");
    let mut broken = ServerConfig::for_test(broken_data.clone());
    broken.backup_copy_dir = Some(blocker.join("share"));
    let broken_app = router(broken).await.unwrap();
    let (broken_admin, _) = register_account(&broken_app, "copy-broken@example.com").await;
    broken_app
        .clone()
        .oneshot(json_request("POST", "/v1/system/backups", &broken_admin, json!({})))
        .await
        .unwrap();
    let failed = wait_for_backup_work(&broken_app, &broken_admin).await;
    assert_eq!(failed["status"]["succeeded"], false, "{failed}");
    assert!(failed["status"]["message"].as_str().unwrap().contains("Backed up to backup-"), "{failed}");
    assert!(failed["status"]["message"].as_str().unwrap().contains("The copy to"), "{failed}");
    assert_eq!(failed["backups"].as_array().unwrap().len(), 1, "the backup itself is kept");
    assert!(failed["backups"][0].get("copy").is_none());

    // A second copy that is not set up cannot be asked for.
    let plain_data = temp_data_dir("backup-copy-off");
    let plain_app = router(ServerConfig::for_test(plain_data.clone())).await.unwrap();
    let (plain_admin, _) = register_account(&plain_app, "copy-off@example.com").await;
    let refused = plain_app
        .clone()
        .oneshot(json_request("POST", "/v1/system/backups/copy", &plain_admin, json!({})))
        .await
        .unwrap();
    assert_eq!(refused.status(), 400);
    for path in [data_dir, copies.parent().unwrap().to_path_buf(), blocked, broken_data, plain_data] {
        let _ = std::fs::remove_dir_all(path);
    }
}

#[tokio::test]
async fn quotas_protect_the_disk_and_the_server() {
    let data_dir = temp_data_dir("quotas");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.workspace_storage_mb = 1;
    config.upload_quota = crate::Quota::per_hour(3);
    config.workspaces_per_person = 1;
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "quota-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;

    // A second workspace is over the person's allowance.
    let organizations = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/organizations", &owner))
            .await
            .unwrap(),
    )
    .await;
    let second = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/workspaces",
            &owner,
            json!({"organization_id": organizations[0]["id"], "name": "Another"}),
        ))
        .await
        .unwrap();
    assert_eq!(second.status(), 409);
    let body = response_json(second).await;
    assert_eq!(body["error"]["code"], "workspace_limit");

    // Within the allowance, then over it.
    assert_eq!(
        upload(&app, &owner, &workspace_id, "small.txt", vec![b'a'; 600 * 1024]).await,
        201
    );
    assert_eq!(
        upload(&app, &owner, &workspace_id, "big.txt", vec![b'b'; 600 * 1024]).await,
        413
    );
    // The note box counts as stored evidence too.
    let note = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            &owner,
            json!({"title": "Long", "content": "x".repeat(600 * 1024)}),
        ))
        .await
        .unwrap();
    assert_eq!(note.status(), 413);

    // Uploads per hour: three are allowed; refusals for room do not count.
    for name in ["tiny-1.txt", "tiny-2.txt"] {
        assert_eq!(upload(&app, &owner, &workspace_id, name, b"1".to_vec()).await, 201);
    }
    assert_eq!(
        upload(&app, &owner, &workspace_id, "tiny-3.txt", b"1".to_vec()).await,
        429
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn uploads_stop_when_the_disk_is_nearly_full() {
    let data_dir = temp_data_dir("disk-floor");
    let mut config = ServerConfig::for_test(data_dir.clone());
    // About a petabyte: more than any test machine has free.
    config.disk_floor_mb = 1_000_000_000;
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "disk-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    assert_eq!(
        upload(&app, &owner, &workspace_id, "any.txt", b"text".to_vec()).await,
        507
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

/// V0.2.152: a JSON body stops at 1 MiB whatever the upload limit is; only the
/// routes that take a file (an upload, a new version) use that limit.
#[tokio::test]
async fn json_bodies_stop_at_a_mebibyte_and_files_keep_the_upload_limit() {
    let data_dir = temp_data_dir("body-limits");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.max_upload_bytes = 4 * 1024 * 1024;
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "limits-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let note = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            &owner,
            json!({"title": "Small note", "content": "A short note."}),
        ))
        .await
        .unwrap();
    assert_eq!(note.status(), 201);
    let artifact_id = response_json(note).await["id"].as_str().unwrap().to_owned();

    // Two mebibytes of JSON are refused on a route that takes JSON, with the upload limit at four.
    let long = "x".repeat(2 * 1024 * 1024);
    let comment = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/artifacts/{artifact_id}/comments"),
            &owner,
            json!({ "body": long }),
        ))
        .await
        .unwrap();
    assert_eq!(comment.status(), 413, "a 2 MiB comment");
    let long_note = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            &owner,
            json!({ "title": "Long note", "content": long }),
        ))
        .await
        .unwrap();
    assert_eq!(long_note.status(), 413, "a 2 MiB note");
    let search = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/search"),
            &owner,
            json!({ "query": long }),
        ))
        .await
        .unwrap();
    assert_eq!(search.status(), 413, "a 2 MiB search");
    let ordinary = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/artifacts/{artifact_id}/comments"),
            &owner,
            json!({"body": "Looks right."}),
        ))
        .await
        .unwrap();
    assert_eq!(ordinary.status(), 201, "an ordinary comment");

    // A file may be as large as the upload limit, as a first upload or as a new version, and no larger.
    assert_eq!(
        upload(&app, &owner, &workspace_id, "three.txt", vec![b'a'; 3 * 1024 * 1024]).await,
        201
    );
    assert_eq!(
        upload(&app, &owner, &workspace_id, "five.txt", vec![b'a'; 5 * 1024 * 1024]).await,
        413
    );
    let version = |bytes: Vec<u8>| {
        Request::builder()
            .method("POST")
            .uri(format!("/v1/artifacts/{artifact_id}/versions"))
            .header("authorization", &owner)
            .header("x-repomemo-filename", "note.txt")
            .body(Body::from(bytes))
            .unwrap()
    };
    // The note was stored as plain text, so the new version keeps that extension.
    let big_version = app.clone().oneshot(version(vec![b'b'; 5 * 1024 * 1024])).await.unwrap();
    assert_eq!(big_version.status(), 413, "a new version above the upload limit");
    let version_within = app.clone().oneshot(version(vec![b'b'; 3 * 1024 * 1024])).await.unwrap();
    assert_eq!(version_within.status(), 200, "a new version within it");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn metrics_are_open_metrics_for_system_administrators() {
    let data_dir = temp_data_dir("metrics");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (admin, _) = register_account(&app, "metrics-admin@example.com").await;
    let (member, _) = register_account(&app, "metrics-member@example.com").await;
    let workspace_id = create_workspace(&app, &admin).await;
    assert_eq!(
        upload(&app, &admin, &workspace_id, "a.txt", b"hello".to_vec()).await,
        201
    );

    let denied = app
        .clone()
        .oneshot(auth_request("GET", "/v1/system/metrics", &member))
        .await
        .unwrap();
    assert_eq!(denied.status(), 403);
    let anonymous = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/system/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(anonymous.status(), 401);

    let response = app
        .clone()
        .oneshot(auth_request("GET", "/v1/system/metrics", &admin))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("application/openmetrics-text"));
    let text = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
    assert!(text.contains("repomemo_build_info{version=\""));
    assert!(text.contains("repomemo_users 2\n"), "{text}");
    assert!(text.contains("repomemo_artifacts 1\n"));
    assert!(text.contains("repomemo_http_requests_total "));
    assert!(text.ends_with("# EOF\n"));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_console_lists_backups_and_asks_before_changing_anything() {
    let data_dir = temp_data_dir("console-backup");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (admin, _) = register_account(&app, "console-backup@example.com").await;
    let run = |command: String| {
        let app = app.clone();
        let admin = admin.clone();
        async move {
            response_json(
                app.oneshot(json_request(
                    "POST",
                    "/v1/system/console",
                    &admin,
                    json!({"command": command}),
                ))
                .await
                .unwrap(),
            )
            .await
        }
    };
    let list = run("backup list".to_owned()).await;
    assert_eq!(list["command"], "backup list");
    assert_eq!(list["data"].as_array().unwrap().len(), 0);
    let dry = run("backup create".to_owned()).await;
    assert_eq!(dry["dry_run"], true);
    let started = run("backup create --yes".to_owned()).await;
    assert_eq!(started["dry_run"], false);
    let overview = wait_for_backup_work(&app, &admin).await;
    assert_eq!(overview["backups"].as_array().unwrap().len(), 1);
    let listed = run("backup".to_owned()).await;
    assert_eq!(listed["data"].as_array().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn mail_is_set_up_in_system_settings_and_sent_from_the_outbox() {
    let data_dir = temp_data_dir("mail");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (admin, _) = register_account(&app, "mail-admin@example.com").await;
    let (member, _) = register_account(&app, "mail-member@example.com").await;
    let call = |method: &'static str, uri: &str, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let settings = |host: &str| {
        json!({"enabled": true, "host": host, "port": 587, "security": "starttls", "username": "memo",
               "password": "smtp-password-1", "from_address": "memo@example.com", "from_name": "RepoMemo"})
    };

    // Nothing is set up at first, and only administrators look.
    let first = response_json(call("GET", "/v1/system/mail", &admin, json!({})).await).await;
    assert_eq!(first["settings"]["configured"], false);
    assert_eq!(call("GET", "/v1/system/mail", &member, json!({})).await.status(), 403);
    assert_eq!(
        call("PUT", "/v1/system/mail", &member, settings("mail.invalid"))
            .await
            .status(),
        403
    );

    // The test mail needs mail to be set up.
    let early = call("POST", "/v1/system/mail/test", &admin, json!({"to": "ada@example.com"})).await;
    assert_eq!(early.status(), 409);

    // Hosts that must not be called are refused when they are saved.
    for host in ["169.254.169.254", "metadata.google.internal", "10.0.0.5", "127.0.0.1"] {
        assert_eq!(
            call("PUT", "/v1/system/mail", &admin, settings(host)).await.status(),
            400,
            "{host}"
        );
    }
    let mut plain = settings("mail.invalid");
    plain["security"] = json!("none");
    assert_eq!(call("PUT", "/v1/system/mail", &admin, plain).await.status(), 400);

    // Saved: the password is never given back, only that there is one.
    let saved = call("PUT", "/v1/system/mail", &admin, settings("mail.invalid")).await;
    assert_eq!(saved.status(), 200);
    let saved = response_json(saved).await;
    assert_eq!(saved["settings"]["password_set"], true);
    assert_eq!(saved["settings"]["host"], "mail.invalid");
    assert!(!saved.to_string().contains("smtp-password-1"));
    let mut keep = settings("mail.invalid");
    keep.as_object_mut().unwrap().remove("password");
    let kept = response_json(call("PUT", "/v1/system/mail", &admin, keep.clone()).await).await;
    assert_eq!(
        kept["settings"]["password_set"], true,
        "absent keeps the saved password"
    );
    keep["password"] = json!("");
    let cleared = response_json(call("PUT", "/v1/system/mail", &admin, keep.clone()).await).await;
    assert_eq!(cleared["settings"]["password_set"], false, "empty removes it");
    let _ = call("PUT", "/v1/system/mail", &admin, settings("mail.invalid")).await;

    // A test mail is accepted at once, whatever the mail server does. Here the
    // name never resolves, so the message waits for a retry and says why.
    assert_eq!(
        call("POST", "/v1/system/mail/test", &admin, json!({"to": "not an address"}))
            .await
            .status(),
        400
    );
    let queued = call("POST", "/v1/system/mail/test", &admin, json!({"to": "ada@example.com"})).await;
    assert_eq!(queued.status(), 202);
    let entry = response_json(queued).await;
    assert_eq!(entry["kind"], "test");
    assert_eq!(entry["to_address"], "ada@example.com");
    let id = entry["id"].as_str().unwrap().to_owned();
    let mut last_error = Value::Null;
    for _ in 0..100 {
        let outbox = response_json(call("GET", "/v1/system/mail/outbox", &admin, json!({})).await).await;
        let found = outbox
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == id)
            .cloned()
            .unwrap();
        last_error = found["last_error"].clone();
        if found["attempts"].as_i64().unwrap_or(0) >= 1 {
            assert_eq!(found["status"], "queued", "a lookup failure is retried");
            assert!(found["next_attempt_at"].is_string());
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(
        last_error.as_str().is_some_and(|reason| reason.contains("looked up")),
        "{last_error}"
    );

    // The console reads the same state and asks before queueing.
    let run = |command: &str| call("POST", "/v1/system/console", &admin, json!({ "command": command }));
    let status = response_json(run("mail").await).await;
    assert_eq!(status["command"], "mail status");
    let dry = response_json(run("mail test grace@example.com").await).await;
    assert_eq!(dry["dry_run"], true);
    assert_eq!(run("mail test nobody").await.status(), 400);

    // The audit trail names the change, never the password.
    let audit = response_json(call("GET", "/v1/system/audit", &admin, json!({})).await).await;
    let details = audit
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["action"] == "mail_settings_changed")
        .map(|event| event["detail"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert!(
        details.len() >= 3 && details.iter().all(|detail| !detail.contains("smtp-password-1")),
        "{details:?}"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn an_invitation_is_mailed_through_the_outbox_when_mail_is_set_up() {
    let data_dir = temp_data_dir("mail-invitation");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.public_url = Some("https://memo.example.com".to_owned());
    let app = router(config).await.unwrap();
    let (admin, _) = register_account(&app, "invite-mail-admin@example.com").await;
    let workspace_id = create_workspace(&app, &admin).await;
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &admin, body);
        async move { app.oneshot(request).await.unwrap() }
    };

    // Without mail the link is still returned, and the answer says nothing was mailed.
    let before = call(
        "POST",
        format!("/v1/workspaces/{workspace_id}/invitations"),
        json!({"email": "first@example.com", "role": "member"}),
    )
    .await;
    assert_eq!(before.status(), 201);
    let before = response_json(before).await;
    assert_eq!(before["mail"], "not_set_up");
    assert!(before["path"].as_str().unwrap().starts_with("/invite#"));

    let saved = call(
        "PUT",
        "/v1/system/mail".to_owned(),
        json!({"enabled": true, "host": "mail.invalid", "port": 587, "security": "starttls", "username": "",
               "from_address": "memo@example.com", "from_name": "RepoMemo"}),
    )
    .await;
    assert_eq!(saved.status(), 200);
    let sent = call(
        "POST",
        format!("/v1/workspaces/{workspace_id}/invitations"),
        json!({"email": "second@example.com", "role": "viewer"}),
    )
    .await;
    assert_eq!(sent.status(), 201);
    let sent = response_json(sent).await;
    assert_eq!(sent["mail"], "queued");
    assert!(sent["token"].is_string(), "the link is returned as well");

    let outbox = response_json(call("GET", "/v1/system/mail/outbox".to_owned(), json!({})).await).await;
    let entry = outbox
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["kind"] == "invitation")
        .expect("the invitation is in the outbox");
    assert_eq!(entry["to_address"], "second@example.com");
    let subject = entry["subject"].as_str().unwrap();
    assert!(
        subject.ends_with("on RepoMemo") && !subject.chars().any(char::is_control),
        "{subject:?}"
    );
    assert!(
        !outbox.to_string().contains(sent["token"].as_str().unwrap()),
        "the outbox listing never carries the link"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn forgot_password_answers_the_same_for_every_address_and_mails_only_a_real_one() {
    let data_dir = temp_data_dir("forgot-password");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.public_url = Some("https://memo.example.com".to_owned());
    let app = router(config).await.unwrap();
    let (admin, _) = register_account(&app, "forgot-admin@example.com").await;
    let (_, _) = register_account(&app, "forgot-person@example.com").await;
    let saved = app
        .clone()
        .oneshot(json_request(
            "PUT",
            "/v1/system/mail",
            &admin,
            json!({"enabled": true, "host": "mail.invalid", "port": 587, "security": "starttls", "username": "",
                   "from_address": "memo@example.com", "from_name": "RepoMemo"}),
        ))
        .await
        .unwrap();
    assert_eq!(saved.status(), 200);
    let ask = |email: &str| {
        let app = app.clone();
        let request = public_post("/v1/auth/forgot-password", json!({ "email": email }));
        async move {
            let response = app.oneshot(request).await.unwrap();
            (response.status(), response_json(response).await)
        }
    };

    let known = ask("forgot-person@example.com").await;
    let unknown = ask("nobody-here@example.com").await;
    let malformed = ask("not an address").await;
    assert_eq!(known.0, 202);
    assert_eq!(known, unknown, "a known and an unknown address are answered alike");
    assert_eq!(known, malformed, "so is something that is not an address");

    // The mail for the known address reaches the outbox; none for the others.
    let outbox = |app: axum::Router, admin: String| async move {
        response_json(
            app.oneshot(auth_request("GET", "/v1/system/mail/outbox", &admin))
                .await
                .unwrap(),
        )
        .await
    };
    let mut listed = Vec::new();
    for _ in 0..50 {
        listed = outbox(app.clone(), admin.clone()).await.as_array().unwrap().clone();
        if listed.iter().any(|entry| entry["kind"] == "password_reset") {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let resets = listed
        .iter()
        .filter(|entry| entry["kind"] == "password_reset")
        .collect::<Vec<_>>();
    assert_eq!(resets.len(), 1, "{listed:?}");
    assert_eq!(resets[0]["to_address"], "forgot-person@example.com");

    // Asking again and again is limited per address, with the same answer.
    for _ in 0..4 {
        assert_eq!(ask("forgot-person@example.com").await, known);
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let after = outbox(app.clone(), admin.clone()).await;
    let count = after
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["kind"] == "password_reset")
        .count();
    assert!(count <= 3, "at most three mails an hour per address, got {count}");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn assignments_and_mentions_are_mailed_once_and_never_to_the_person_who_caused_them() {
    let data_dir = temp_data_dir("mail-notifications");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.public_url = Some("https://memo.example.com".to_owned());
    let app = router(config).await.unwrap();
    let (admin, admin_body) = register_account(&app, "notify-admin@example.com").await;
    let (_, member_body) = register_account(&app, "notify-member@example.com").await;
    let admin_id = admin_body["user"]["id"].as_str().unwrap().to_owned();
    let member_id = member_body["user"]["id"].as_str().unwrap().to_owned();
    let workspace_id = create_workspace(&app, &admin).await;
    join(
        &app,
        &admin,
        "workspaces",
        &workspace_id,
        "notify-member@example.com",
        "member",
    )
    .await;
    let saved = app
        .clone()
        .oneshot(json_request(
            "PUT",
            "/v1/system/mail",
            &admin,
            json!({"enabled": true, "host": "mail.invalid", "port": 587, "security": "starttls", "username": "",
                   "from_address": "memo@example.com", "from_name": "RepoMemo"}),
        ))
        .await
        .unwrap();
    assert_eq!(saved.status(), 200);
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &admin, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let task = |assignee: &str, title: &str| json!({"title": title, "description": "", "status": "open", "priority": "medium", "assignee_user_id": assignee});

    // Assigned to someone else: one mail. To oneself: none.
    let created = call(
        "POST",
        format!("/v1/workspaces/{workspace_id}/tasks"),
        task(&member_id, "Fix the deploy"),
    )
    .await;
    assert_eq!(created.status(), 201);
    let created = response_json(created).await;
    assert_eq!(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/tasks"),
            task(&admin_id, "My own task")
        )
        .await
        .status(),
        201
    );
    // Saving the task again with the same assignee sends nothing more.
    let task_id = created["id"].as_str().unwrap().to_owned();
    assert_eq!(
        call(
            "PUT",
            format!("/v1/tasks/{task_id}"),
            task(&member_id, "Fix the deploy (edited)")
        )
        .await
        .status(),
        200
    );

    // A mention of someone else in a comment: one mail; of oneself: none.
    let artifact = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            json!({"title": "Runbook", "content": "Steps.", "language": "Markdown"}),
        )
        .await,
    )
    .await;
    let artifact_id = artifact["id"].as_str().unwrap().to_owned();
    for body in [
        "Please check @notify-member@example.com",
        "Note to self @notify-admin@example.com",
    ] {
        let response = call(
            "POST",
            format!("/v1/artifacts/{artifact_id}/comments"),
            json!({ "body": body }),
        )
        .await;
        assert_eq!(response.status(), 201);
    }

    for _ in 0..30 {
        let outbox = response_json(call("GET", "/v1/system/mail/outbox".to_owned(), json!({})).await).await;
        let waiting = outbox
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["kind"] != "invitation")
            .count();
        if waiting >= 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let outbox = response_json(call("GET", "/v1/system/mail/outbox".to_owned(), json!({})).await).await;
    let mut kinds = outbox
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["kind"] != "invitation")
        .map(|entry| {
            (
                entry["kind"].as_str().unwrap().to_owned(),
                entry["to_address"].as_str().unwrap().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    kinds.sort();
    assert_eq!(
        kinds,
        vec![
            ("evidence_mention".to_owned(), "notify-member@example.com".to_owned()),
            ("task_assigned".to_owned(), "notify-member@example.com".to_owned()),
        ],
        "one mail per event, none to the person who acted"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn unverified_accounts_are_held_back_until_an_invitation_or_a_link_proves_the_address() {
    let data_dir = temp_data_dir("email-verification");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.public_url = Some("https://memo.example.com".to_owned());
    let app = router(config).await.unwrap();
    let (admin, _) = register_account(&app, "verify-admin@example.com").await;
    let workspace_id = create_workspace(&app, &admin).await;

    // Mail is not set up yet: nothing is withheld, since nobody could verify.
    let (early, _) = register_account(&app, "verify-early@example.com").await;
    let created = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/organizations",
            &early,
            json!({"name": "Early"}),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), 201);

    let saved = app
        .clone()
        .oneshot(json_request(
            "PUT",
            "/v1/system/mail",
            &admin,
            json!({"enabled": true, "host": "mail.invalid", "port": 587, "security": "starttls", "username": "",
                   "from_address": "memo@example.com", "from_name": "RepoMemo"}),
        ))
        .await
        .unwrap();
    assert_eq!(saved.status(), 200);

    // Registered on an open server with mail on: held back from creating
    // anything. With mail on the answer is the same for every address (V0.2.18),
    // so the person signs in with the password they chose.
    let accepted = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/register",
            json!({"email": "verify-person@example.com", "display_name": "Person", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(accepted.status(), 202);
    let signed_in = response_json(
        app.clone()
            .oneshot(public_post(
                "/v1/auth/login",
                json!({"email": "verify-person@example.com", "password": "not-a-real-password"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let person = format!("Bearer {}", signed_in["access_token"].as_str().unwrap());
    let profile = |who: String| {
        let app = app.clone();
        async move { response_json(app.oneshot(auth_request("GET", "/v1/profile", &who)).await.unwrap()).await }
    };
    let before = profile(person.clone()).await;
    assert_eq!(before["email_verified"], false);
    assert_eq!(before["verification_required"], true);
    let refused = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/organizations",
            &person,
            json!({"name": "Mine"}),
        ))
        .await
        .unwrap();
    assert_eq!(refused.status(), 403);

    // A link was mailed, and another can be asked for, three an hour.
    let outbox = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/system/mail/outbox", &admin))
            .await
            .unwrap(),
    )
    .await;
    let mut mailed = false;
    for _ in 0..30 {
        let listed = response_json(
            app.clone()
                .oneshot(auth_request("GET", "/v1/system/mail/outbox", &admin))
                .await
                .unwrap(),
        )
        .await;
        if listed
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["kind"] == "email_verification" && entry["to_address"] == "verify-person@example.com")
        {
            mailed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(mailed, "{outbox}");
    let resend = |who: String| {
        let app = app.clone();
        async move {
            app.oneshot(json_request("POST", "/v1/profile/email/verify", &who, json!({})))
                .await
                .unwrap()
                .status()
        }
    };
    assert_eq!(resend(person.clone()).await, 200);
    assert_eq!(resend(person.clone()).await, 200);
    assert_eq!(resend(person.clone()).await, 200);
    assert_eq!(resend(person.clone()).await, 429);

    // An unknown link is refused the same way as a used one.
    let unknown = app
        .clone()
        .oneshot(public_post("/v1/auth/verify-email", json!({"token": "nope"})))
        .await
        .unwrap();
    assert_eq!(unknown.status(), 410);

    // Accepting an invitation sent to the address proves it.
    join(
        &app,
        &admin,
        "workspaces",
        &workspace_id,
        "verify-person@example.com",
        "member",
    )
    .await;
    assert_eq!(profile(person.clone()).await["email_verified"], true);
    let allowed = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/organizations",
            &person,
            json!({"name": "Mine"}),
        ))
        .await
        .unwrap();
    assert_eq!(allowed.status(), 201);

    // Changing the address asks for the password and starts unverified again.
    let wrong = app
        .clone()
        .oneshot(json_request(
            "PUT",
            "/v1/profile/email",
            &person,
            json!({"email": "verify-new@example.com", "current_password": "wrong password!!"}),
        ))
        .await
        .unwrap();
    assert_eq!(wrong.status(), 403);
    let changed = app
        .clone()
        .oneshot(json_request(
            "PUT",
            "/v1/profile/email",
            &person,
            json!({"email": "verify-new@example.com", "current_password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(changed.status(), 200);
    let after = profile(person.clone()).await;
    assert_eq!(after["user"]["email"], "verify-new@example.com");
    assert_eq!(after["email_verified"], false);
    let taken = app
        .clone()
        .oneshot(json_request(
            "PUT",
            "/v1/profile/email",
            &person,
            json!({"email": "verify-admin@example.com", "current_password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(taken.status(), 409);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_person_chooses_where_each_kind_of_notice_reaches_them() {
    let data_dir = temp_data_dir("notification-preferences");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.public_url = Some("https://memo.example.com".to_owned());
    let app = router(config).await.unwrap();
    let (admin, _) = register_account(&app, "prefs-admin@example.com").await;
    let (member, member_body) = register_account(&app, "prefs-member@example.com").await;
    let member_id = member_body["user"]["id"].as_str().unwrap().to_owned();
    let workspace_id = create_workspace(&app, &admin).await;
    join(
        &app,
        &admin,
        "workspaces",
        &workspace_id,
        "prefs-member@example.com",
        "member",
    )
    .await;
    let saved = app
        .clone()
        .oneshot(json_request(
            "PUT",
            "/v1/system/mail",
            &admin,
            json!({"enabled": true, "host": "mail.invalid", "port": 587, "security": "starttls", "username": "",
                   "from_address": "memo@example.com", "from_name": "RepoMemo"}),
        ))
        .await
        .unwrap();
    assert_eq!(saved.status(), 200);
    let call = |method: &'static str, uri: &str, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };

    // The defaults, and what cannot be chosen.
    let preferences =
        response_json(call("GET", "/v1/profile/notification-preferences", &member, json!({})).await).await;
    for preference in preferences.as_array().unwrap() {
        let expected = if preference["kind"] == "daily_digest" {
            "none"
        } else {
            "both"
        };
        assert_eq!(preference["channel"], expected, "{preference}");
    }
    assert_eq!(
        call(
            "PUT",
            "/v1/profile/notification-preferences",
            &member,
            json!({"kind": "password_reset_issued", "channel": "none"})
        )
        .await
        .status(),
        400,
        "security notices cannot be turned off"
    );
    assert_eq!(
        call(
            "PUT",
            "/v1/profile/notification-preferences",
            &member,
            json!({"kind": "task_assigned", "channel": "sometimes"})
        )
        .await
        .status(),
        400
    );

    let assign = |title: &'static str| {
        call(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/tasks"),
            &admin,
            json!({"title": title, "description": "", "status": "open", "priority": "medium", "assignee_user_id": member_id}),
        )
    };
    let notices = || async {
        response_json(call("GET", "/v1/notifications", &member, json!({})).await)
            .await
            .as_array()
            .unwrap()
            .iter()
            .filter(|notice| notice["notification_type"] == "task_assigned")
            .count()
    };
    let mails = || async {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        response_json(call("GET", "/v1/system/mail/outbox", &admin, json!({})).await)
            .await
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["kind"] == "task_assigned")
            .count()
    };

    for (channel, expected_notices, expected_mails) in [("none", 0, 0), ("app", 1, 0), ("mail", 1, 1), ("both", 2, 2)] {
        let chosen = call(
            "PUT",
            "/v1/profile/notification-preferences",
            &member,
            json!({"kind": "task_assigned", "channel": channel}),
        )
        .await;
        assert_eq!(chosen.status(), 200);
        assert_eq!(assign("A task").await.status(), 201);
        assert_eq!(notices().await, expected_notices, "{channel}: notices");
        assert_eq!(mails().await, expected_mails, "{channel}: mails");
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_daily_digest_is_asked_for_and_sent_once_when_something_is_unread() {
    let data_dir = temp_data_dir("daily-digest");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.public_url = Some("https://memo.example.com".to_owned());
    let app = router(config).await.unwrap();
    let (admin, _) = register_account(&app, "digest-admin@example.com").await;
    let (member, member_body) = register_account(&app, "digest-member@example.com").await;
    let member_id = member_body["user"]["id"].as_str().unwrap().to_owned();
    let workspace_id = create_workspace(&app, &admin).await;
    join(
        &app,
        &admin,
        "workspaces",
        &workspace_id,
        "digest-member@example.com",
        "member",
    )
    .await;
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let saved = call(
        "PUT",
        "/v1/system/mail".to_owned(),
        admin.clone(),
        json!({"enabled": true, "host": "mail.invalid", "port": 587, "security": "starttls", "username": "",
               "from_address": "memo@example.com", "from_name": "RepoMemo"}),
    )
    .await;
    assert_eq!(saved.status(), 200);
    let digests = || {
        let call = &call;
        let admin = admin.clone();
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            response_json(call("GET", "/v1/system/mail/outbox".to_owned(), admin, json!({})).await)
                .await
                .as_array()
                .unwrap()
                .iter()
                .filter(|entry| entry["kind"] == "digest")
                .count()
        }
    };

    // Notices individually kept to the app, so only the digest would mail them.
    assert_eq!(
        call(
            "PUT",
            "/v1/profile/notification-preferences".to_owned(),
            member.clone(),
            json!({"kind": "task_assigned", "channel": "app"})
        )
        .await
        .status(),
        200
    );
    let task = |title: &str| json!({"title": title, "description": "", "status": "open", "priority": "medium", "assignee_user_id": member_id});
    assert_eq!(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/tasks"),
            admin.clone(),
            task("First")
        )
        .await
        .status(),
        201
    );

    // Not asked for yet: nothing is sent. The digest takes only mail or off.
    assert_eq!(digests().await, 0);
    assert_eq!(
        call(
            "PUT",
            "/v1/profile/notification-preferences".to_owned(),
            member.clone(),
            json!({"kind": "daily_digest", "channel": "both"})
        )
        .await
        .status(),
        400
    );
    assert_eq!(
        call(
            "PUT",
            "/v1/profile/notification-preferences".to_owned(),
            member.clone(),
            json!({"kind": "daily_digest", "channel": "mail"})
        )
        .await
        .status(),
        200
    );
    assert_eq!(digests().await, 1, "asked for, with something unread: one mail");

    // Another notice the same day does not make a second mail.
    assert_eq!(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/tasks"),
            admin.clone(),
            task("Second")
        )
        .await
        .status(),
        201
    );
    assert_eq!(digests().await, 1, "at most one a day");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn open_registration_answers_the_same_for_a_known_address_and_mails_its_owner() {
    let data_dir = temp_data_dir("registration-same-answer");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.public_url = Some("https://memo.example.com".to_owned());
    let app = router(config).await.unwrap();
    let (admin, _) = register_account(&app, "same-admin@example.com").await;
    let register = |email: &str, password: &str| {
        let app = app.clone();
        let request = public_post(
            "/v1/auth/register",
            json!({"email": email, "display_name": "Person", "password": password}),
        );
        async move {
            let response = app.oneshot(request).await.unwrap();
            (response.status().as_u16(), response_json(response).await)
        }
    };
    let login = |email: &str, password: &str| {
        let app = app.clone();
        let request = public_post("/v1/auth/login", json!({"email": email, "password": password}));
        async move { app.oneshot(request).await.unwrap().status().as_u16() }
    };

    // Without mail the old answers stand: a session, and a refusal for a known address.
    assert_eq!(register("same-early@example.com", "not-a-real-password").await.0, 201);
    assert_eq!(register("same-early@example.com", "not-a-real-password").await.0, 409);

    let saved = app
        .clone()
        .oneshot(json_request(
            "PUT",
            "/v1/system/mail",
            &admin,
            json!({"enabled": true, "host": "mail.invalid", "port": 587, "security": "starttls", "username": "",
                   "from_address": "memo@example.com", "from_name": "RepoMemo"}),
        ))
        .await
        .unwrap();
    assert_eq!(saved.status(), 200);

    // With mail on, a new address and a known one get the very same answer.
    let fresh = register("same-new@example.com", "a-password-just-chosen").await;
    let known = register("same-admin@example.com", "another-password-chosen").await;
    assert_eq!(fresh.0, 202);
    assert_eq!(fresh, known, "status and body do not tell the two apart");
    assert!(fresh.1["access_token"].is_null());

    // The new account exists and works; the known one is untouched.
    assert_eq!(login("same-new@example.com", "a-password-just-chosen").await, 200);
    assert_eq!(login("same-admin@example.com", "another-password-chosen").await, 401);
    assert_eq!(login("same-admin@example.com", "not-a-real-password").await, 200);

    // The owner of a known address is told, at most twice an hour.
    assert_eq!(register("same-admin@example.com", "yet-another-password").await.0, 202);
    assert_eq!(register("same-admin@example.com", "and-one-more-password").await.0, 202);
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    let outbox = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/system/mail/outbox", &admin))
            .await
            .unwrap(),
    )
    .await;
    let entries = outbox.as_array().unwrap();
    let notices = entries
        .iter()
        .filter(|entry| entry["kind"] == "registration_attempt" && entry["to_address"] == "same-admin@example.com")
        .count();
    assert_eq!(notices, 2);
    assert!(entries
        .iter()
        .any(|entry| entry["kind"] == "email_verification" && entry["to_address"] == "same-new@example.com"));
    assert!(!entries
        .iter()
        .any(|entry| entry["kind"] == "email_verification" && entry["to_address"] == "same-admin@example.com"));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn an_address_cannot_be_flooded_with_invitations_through_the_server() {
    let data_dir = temp_data_dir("mail-flood");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.public_url = Some("https://memo.example.com".to_owned());
    let app = router(config).await.unwrap();
    let (admin, _) = register_account(&app, "flood-admin@example.com").await;
    let workspace_id = create_workspace(&app, &admin).await;
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &admin, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let saved = call(
        "PUT",
        "/v1/system/mail".to_owned(),
        json!({"enabled": true, "host": "mail.invalid", "port": 587, "security": "starttls", "username": "",
               "from_address": "memo@example.com", "from_name": "RepoMemo"}),
    )
    .await;
    assert_eq!(saved.status(), 200);

    let mut answers = Vec::new();
    for _ in 0..14 {
        let invited = call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/invitations"),
            json!({"email": "victim@example.com", "role": "viewer"}),
        )
        .await;
        assert_eq!(invited.status(), 201, "the invitation itself is still made, with its link");
        answers.push(response_json(invited).await["mail"].as_str().unwrap().to_owned());
    }
    assert_eq!(answers.iter().filter(|answer| *answer == "queued").count(), 12, "{answers:?}");
    assert!(answers[12..].iter().all(|answer| answer == "rate_limited"), "{answers:?}");

    // Only twelve are in the outbox, and another address is not affected.
    let outbox = response_json(call("GET", "/v1/system/mail/outbox?limit=100".to_owned(), json!({})).await).await;
    let to_victim = outbox
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["to_address"] == "victim@example.com")
        .count();
    assert_eq!(to_victim, 12);
    let other = call(
        "POST",
        format!("/v1/workspaces/{workspace_id}/invitations"),
        json!({"email": "someone-else@example.com", "role": "viewer"}),
    )
    .await;
    assert_eq!(response_json(other).await["mail"], "queued");
    let _ = std::fs::remove_dir_all(data_dir);
}
