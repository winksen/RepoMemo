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
