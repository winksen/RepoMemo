use super::*;

#[tokio::test]
async fn a_maintenance_pass_runs_cleanly() {
    let data_dir = temp_data_dir("maintenance");
    let config = ServerConfig::for_test(data_dir.clone());
    let (state, _tasks) = crate::build_state(&config, std::sync::Weak::new()).await.unwrap();
    let report = crate::maintenance::run_pass(&state, true, true).await;
    assert_eq!(report.failures, 0, "{report:?}");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn system_administrators_see_every_workspace_and_run_the_server() {
    let data_dir = temp_data_dir("system-admin");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.system_admin_emails = vec!["listed@example.com".to_owned()];
    let app = router(config).await.unwrap();
    let get = |uri: String, who: &str| {
        let app = app.clone();
        let request = auth_request("GET", &uri, who);
        async move { app.oneshot(request).await.unwrap() }
    };
    let send = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };

    // The first account of a new server administers it; later ones do not.
    let (admin, _) = register_account(&app, "first@example.com").await;
    let (owner, owner_tokens) = register_account(&app, "owner@example.com").await;
    let owner_id = owner_tokens["user"]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        response_json(get("/v1/session".to_owned(), &admin).await).await["is_system_admin"],
        true
    );
    assert_eq!(
        response_json(get("/v1/session".to_owned(), &owner).await).await["is_system_admin"],
        false
    );
    let (listed, _) = register_account(&app, "listed@example.com").await;
    assert_eq!(
        response_json(get("/v1/session".to_owned(), &listed).await).await["is_system_admin"],
        false,
        "registering a listed address does not grant the role (only accounts that exist at startup are promoted)"
    );

    // Someone else's organization and workspace are visible, as an administrator.
    let workspace_id = create_workspace(&app, &owner).await;
    let organizations = response_json(get("/v1/organizations".to_owned(), &admin).await).await;
    assert_eq!(organizations.as_array().unwrap().len(), 1);
    assert_eq!(organizations[0]["role"], "admin");
    let organization_id = organizations[0]["id"].as_str().unwrap().to_owned();
    let workspaces = response_json(get("/v1/workspaces".to_owned(), &admin).await).await;
    assert_eq!(workspaces[0]["role"], "admin");
    assert_eq!(
        get(format!("/v1/workspaces/{workspace_id}/overview"), &admin)
            .await
            .status(),
        200
    );
    assert_eq!(
        get(format!("/v1/workspaces/{workspace_id}/members"), &admin)
            .await
            .status(),
        200,
        "administrator sections"
    );
    assert_eq!(
        get(format!("/v1/organizations/{organization_id}/members"), &admin)
            .await
            .status(),
        200
    );
    let capabilities = response_json(get(format!("/v1/workspaces/{workspace_id}/capabilities"), &admin).await).await;
    assert_eq!(capabilities["can_manage_members"], true);
    assert_eq!(
        capabilities["can_manage_workspace"], false,
        "owner-only actions stay with the owner"
    );
    assert_eq!(
        send("DELETE", format!("/v1/workspaces/{workspace_id}"), &admin, json!({}))
            .await
            .status(),
        403
    );
    let created = send(
        "POST",
        "/v1/workspaces".to_owned(),
        &admin,
        json!({"organization_id": organization_id, "name": "Admin made"}),
    )
    .await;
    assert_eq!(
        created.status(),
        201,
        "administrators create workspaces in any organization"
    );
    assert_eq!(
        get(format!("/v1/workspaces/{workspace_id}/overview"), &listed)
            .await
            .status(),
        403
    );

    // The system API is for system administrators only.
    assert_eq!(get("/v1/system/overview".to_owned(), &owner).await.status(), 403);
    let overview = response_json(get("/v1/system/overview".to_owned(), &admin).await).await;
    assert_eq!(overview["statistics"]["user_count"], 3);
    assert_eq!(overview["statistics"]["workspace_count"], 2);
    assert_eq!(overview["instance"]["system_admin_count"], 1);
    assert!(overview["traffic"]["requests"].as_u64().unwrap() > 0);
    let usage = response_json(get("/v1/system/usage?days=7".to_owned(), &admin).await).await;
    assert_eq!(usage["activity_by_day"].as_array().unwrap().len(), 7);
    assert_eq!(usage["workspaces"].as_array().unwrap().len(), 2);
    assert_eq!(get("/v1/system/jobs".to_owned(), &admin).await.status(), 200);
    assert_eq!(get("/v1/system/events".to_owned(), &owner).await.status(), 403);
    assert_eq!(get("/v1/system/events".to_owned(), &admin).await.status(), 200);
    assert_eq!(get("/v1/system/logs?level=warn".to_owned(), &admin).await.status(), 200);

    // Run-time settings apply at once and can be reset.
    let refused = send(
        "PUT",
        "/v1/system/settings".to_owned(),
        &admin,
        json!({"values": {"allow_registration": false, "login_max_failures": -3}}),
    )
    .await;
    assert_eq!(refused.status(), 400, "a request applies completely or not at all");
    let changed = send(
        "PUT",
        "/v1/system/settings".to_owned(),
        &admin,
        json!({"values": {"allow_registration": false}}),
    )
    .await;
    assert_eq!(changed.status(), 200);
    let changed = response_json(changed).await;
    let registration = changed["settings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|setting| setting["key"] == "allow_registration")
        .unwrap()
        .clone();
    assert_eq!(
        (registration["value"].clone(), registration["overridden"].clone()),
        (json!(false), json!(true))
    );
    let closed = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/register",
            json!({"email": "late@example.com", "display_name": "Late", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(closed.status(), 403);
    assert_eq!(
        send(
            "DELETE",
            "/v1/system/settings/allow_registration".to_owned(),
            &admin,
            json!({})
        )
        .await
        .status(),
        200
    );
    let reopened = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/register",
            json!({"email": "late@example.com", "display_name": "Late", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(reopened.status(), 201);

    // Roles, sessions and the last administrator.
    let users = response_json(get("/v1/system/users".to_owned(), &admin).await).await;
    assert_eq!(users.as_array().unwrap().len(), 4);
    assert_eq!(
        send(
            "PUT",
            format!("/v1/system/users/{owner_id}/system-admin"),
            &owner,
            json!({"enabled": true})
        )
        .await
        .status(),
        403
    );
    assert_eq!(
        send(
            "PUT",
            format!("/v1/system/users/{owner_id}/system-admin"),
            &admin,
            json!({"enabled": true})
        )
        .await
        .status(),
        204
    );
    assert_eq!(
        get("/v1/system/overview".to_owned(), &owner).await.status(),
        200,
        "the new role applies at once"
    );
    assert_eq!(
        send(
            "POST",
            format!("/v1/system/users/{owner_id}/sessions/end"),
            &admin,
            json!({})
        )
        .await
        .status(),
        204
    );
    assert_eq!(get("/v1/session".to_owned(), &owner).await.status(), 401);
    assert_eq!(
        send("POST", format!("/v1/system/users/{owner_id}/unlock"), &admin, json!({}))
            .await
            .status(),
        200
    );
    for user in users.as_array().unwrap() {
        if user["is_system_admin"] == true && user["email"] != "first@example.com" {
            let id = user["id"].as_str().unwrap();
            assert_eq!(
                send(
                    "PUT",
                    format!("/v1/system/users/{id}/system-admin"),
                    &admin,
                    json!({"enabled": false})
                )
                .await
                .status(),
                204
            );
        }
    }
    let first_id = users
        .as_array()
        .unwrap()
        .iter()
        .find(|user| user["email"] == "first@example.com")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    // The owner was promoted after `users` was listed.
    assert_eq!(
        send(
            "PUT",
            format!("/v1/system/users/{owner_id}/system-admin"),
            &admin,
            json!({"enabled": false})
        )
        .await
        .status(),
        204
    );
    let last = send(
        "PUT",
        format!("/v1/system/users/{first_id}/system-admin"),
        &admin,
        json!({"enabled": false}),
    )
    .await;
    assert_eq!(last.status(), 409, "the last system administrator stays");

    // Maintenance by hand, and the audit trail of it all.
    let maintenance =
        response_json(send("POST", "/v1/system/maintenance/run".to_owned(), &admin, json!({})).await).await;
    assert_eq!(maintenance["last_trigger"], "manual");
    assert_eq!(maintenance["last_report"]["failures"], 0);
    let audit = response_json(get("/v1/system/audit".to_owned(), &admin).await).await;
    let actions = audit
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["action"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    for expected in [
        "system_admin_granted",
        "system_settings_changed",
        "system_setting_reset",
        "user_sessions_ended",
        "system_admin_revoked",
        "maintenance_run",
    ] {
        assert!(
            actions.iter().any(|action| action == expected),
            "{expected} missing from {actions:?}"
        );
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn app_administrators_read_the_system_pages_and_changes_need_the_system_scope() {
    let data_dir = temp_data_dir("app-admin");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let get = |uri: &str, who: &str| {
        let app = app.clone();
        let request = auth_request("GET", uri, who);
        async move { app.oneshot(request).await.unwrap() }
    };
    let send = |method: &'static str, uri: &str, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (admin, _) = register_account(&app, "rooted-admin@example.com").await;
    let (helper, helper_tokens) = register_account(&app, "app-helper@example.com").await;
    let helper_id = helper_tokens["user"]["id"].as_str().unwrap().to_owned();
    let (stranger, _) = register_account(&app, "plain-stranger@example.com").await;
    assert_eq!(
        send(
            "PUT",
            &format!("/v1/system/users/{helper_id}/app-admin"),
            &admin,
            json!({"enabled": true})
        )
        .await
        .status(),
        204
    );

    // Everyone who is not an administrator is turned away from every System page.
    for page in [
        "/v1/system/overview",
        "/v1/system/users",
        "/v1/system/settings",
        "/v1/system/logs",
        "/v1/system/audit",
        "/v1/system/jobs",
        "/v1/system/maintenance",
        "/v1/system/console",
    ] {
        assert_eq!(get(page, &stranger).await.status(), 403, "{page}");
        assert_eq!(
            get(page, &helper).await.status(),
            200,
            "{page} is readable by app administrators"
        );
    }
    assert_eq!(get("/v1/system/logs/files", &helper).await.status(), 200);

    // What only system administrators do stays with them (decision 1, review M8).
    let day = "2026-01-01";
    let refused = [
        (
            "PUT",
            "/v1/system/settings".to_owned(),
            json!({"values": {"allow_registration": false}}),
        ),
        ("DELETE", "/v1/system/settings/allow_registration".to_owned(), json!({})),
        (
            "PUT",
            format!("/v1/system/users/{helper_id}/system-admin"),
            json!({"enabled": true}),
        ),
        (
            "PUT",
            format!("/v1/system/users/{helper_id}/app-admin"),
            json!({"enabled": false}),
        ),
        ("POST", format!("/v1/system/users/{helper_id}/sessions/end"), json!({})),
        ("POST", format!("/v1/system/users/{helper_id}/unlock"), json!({})),
        (
            "POST",
            format!("/v1/system/users/{helper_id}/password-reset"),
            json!({}),
        ),
        ("POST", "/v1/system/environment/detach".to_owned(), json!({})),
    ];
    for (method, uri, body) in refused {
        assert_eq!(
            send(method, &uri, &helper, body).await.status(),
            403,
            "{method} {uri} is for system administrators"
        );
    }
    assert_eq!(
        get(&format!("/v1/system/logs/files/{day}"), &helper).await.status(),
        403,
        "raw log files"
    );
    assert_eq!(
        send(
            "POST",
            "/v1/system/console",
            &helper,
            json!({"command": "settings set allow_registration false --yes"})
        )
        .await
        .status(),
        403
    );
    assert_eq!(
        send("POST", "/v1/system/maintenance/run", &helper, json!({}))
            .await
            .status(),
        200,
        "jobs and maintenance are theirs"
    );

    // The system scope: only administrators can mint it, and only it changes things.
    let mint = |who: String, scope: &'static str| {
        let app = app.clone();
        let request = json_request(
            "POST",
            "/v1/profile/tokens",
            &who,
            json!({"name": "Ops", "scope": scope, "expires_in_days": 7}),
        );
        async move { app.oneshot(request).await.unwrap() }
    };
    assert_eq!(
        mint(stranger.clone(), "system").await.status(),
        403,
        "members cannot mint the system scope"
    );
    let write = response_json(mint(admin.clone(), "write").await).await;
    let write = format!("Bearer {}", write["secret"].as_str().unwrap());
    let system = response_json(mint(admin.clone(), "system").await).await;
    let system = format!("Bearer {}", system["secret"].as_str().unwrap());
    let unlock = format!("/v1/system/users/{helper_id}/unlock");
    assert_eq!(
        send("POST", &unlock, &write, json!({})).await.status(),
        403,
        "a read-and-write token does not change the System area"
    );
    assert_eq!(send("POST", &unlock, &system, json!({})).await.status(), 200);
    assert_eq!(
        send("POST", "/v1/system/maintenance/run", &write, json!({}))
            .await
            .status(),
        403
    );
    assert_eq!(
        get("/v1/system/overview", &write).await.status(),
        200,
        "reading is fine"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_admin_console_runs_commands_with_the_pages_rights() {
    let data_dir = temp_data_dir("console");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (admin, _) = register_account(&app, "console-admin@example.com").await;
    let (member, _) = register_account(&app, "console-member@example.com").await;
    let run = |who: &str, command: &str| {
        let app = app.clone();
        let request = json_request("POST", "/v1/system/console", who, json!({ "command": command }));
        async move { app.oneshot(request).await.unwrap() }
    };

    assert_eq!(run(&member, "status").await.status(), 403, "administrators only");
    let welcome = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/system/console", &admin))
            .await
            .unwrap(),
    )
    .await;
    assert!(welcome["banner"].as_str().unwrap().contains("██"));
    assert_eq!(welcome["role"], "system administrator");
    assert!(welcome["commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|command| command["name"] == "jobs list"));

    let status = response_json(run(&admin, "status").await).await;
    assert_eq!(
        (status["command"].clone(), status["view"]["kind"].clone()),
        (json!("status"), json!("facts"))
    );
    let jobs = response_json(run(&admin, "jobs").await).await;
    assert_eq!(jobs["command"], "jobs list");
    assert!(jobs["data"].is_array());

    // A change is a dry run until confirmed, then goes through the settings handler.
    let preview = response_json(run(&admin, "settings set log_http_level debug").await).await;
    assert_eq!(preview["dry_run"], true);
    let settings = response_json(run(&admin, "settings list").await).await;
    let http = |settings: &Value| {
        settings["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["key"] == "log_http_level")
            .unwrap()["value"]
            .clone()
    };
    assert_eq!(http(&settings), "warn", "a dry run changes nothing");
    let applied = response_json(run(&admin, "settings set log_http_level debug --yes").await).await;
    assert_eq!(applied["dry_run"], false);
    assert_eq!(http(&response_json(run(&admin, "settings").await).await), "debug");
    assert_eq!(
        run(&admin, "settings set log_http_level loud --yes").await.status(),
        400,
        "the handler's validation applies"
    );
    assert_eq!(run(&admin, "settings set no_such_key 1 --yes").await.status(), 404);

    let unlock = response_json(run(&admin, "users unlock CONSOLE-MEMBER@example.com --yes").await).await;
    assert_eq!(unlock["data"]["cleared"], 0);
    assert_eq!(run(&admin, "users unlock nobody@example.com --yes").await.status(), 404);

    let audit = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/system/audit", &admin))
            .await
            .unwrap(),
    )
    .await;
    let actions = audit
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["action"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        actions.iter().filter(|action| **action == "console_command").count(),
        2,
        "{actions:?}"
    );
    assert!(
        actions.contains(&"system_settings_changed") && actions.contains(&"user_unlocked"),
        "{actions:?}"
    );

    for (bad, why) in [
        ("", "empty"),
        ("rm -rf /", "unknown"),
        ("jobs list --stauts failed", "unknown option"),
        ("logs --level loud", "level"),
        ("logs --grep \"open", "quote"),
    ] {
        assert_eq!(run(&admin, bad).await.status(), 400, "{why}");
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn system_administrators_control_logging() {
    let data_dir = temp_data_dir("logging");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (admin, _) = register_account(&app, "logs-admin@example.com").await;
    let (member, _) = register_account(&app, "logs-member@example.com").await;
    let send = |method: &'static str, uri: &str, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };

    let settings = response_json(send("GET", "/v1/system/settings", &admin, json!({})).await).await;
    let logging = settings["settings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|setting| setting["group"] == "Logging")
        .collect::<Vec<_>>();
    assert_eq!(
        logging.len(),
        11,
        "default level, seven categories, format, files, retention"
    );
    let http = logging
        .iter()
        .find(|setting| setting["key"] == "log_http_level")
        .unwrap();
    assert_eq!(
        (http["kind"].clone(), http["value"].clone()),
        (json!("choice"), json!("warn"))
    );
    assert_eq!(http["options"].as_array().unwrap().len(), 6);

    let invalid = send(
        "PUT",
        "/v1/system/settings",
        &admin,
        json!({"values": {"log_console_format": "xml"}}),
    )
    .await;
    assert_eq!(invalid.status(), 400);
    let changed = send(
        "PUT",
        "/v1/system/settings",
        &admin,
        json!({"values": {"log_http_level": "debug", "log_console_format": "json", "log_retention_days": 7}}),
    )
    .await;
    assert_eq!(changed.status(), 200);
    let files = response_json(send("GET", "/v1/system/logs/files", &admin, json!({})).await).await;
    assert_eq!(files["retention_days"], 7);
    assert_eq!(files["console_format"], "json");
    assert_eq!(files["environment_filter"], false);
    let http = files["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|category| category["key"] == "http")
        .unwrap()
        .clone();
    assert_eq!(http["level"], "debug");
    assert_eq!(
        send("DELETE", "/v1/system/settings/log_http_level", &admin, json!({}))
            .await
            .status(),
        200
    );
    let files = response_json(send("GET", "/v1/system/logs/files", &admin, json!({})).await).await;
    let http = files["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|category| category["key"] == "http")
        .unwrap()
        .clone();
    assert_eq!(http["level"], "warn", "reset restores the default");

    assert_eq!(
        send(
            "GET",
            "/v1/system/logs?category=activity&workspace=w&level=info",
            &admin,
            json!({})
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        send("GET", "/v1/system/logs?day=2001-01-01", &admin, json!({}))
            .await
            .status(),
        404
    );
    assert_eq!(
        send("GET", "/v1/system/logs/files/not-a-day", &admin, json!({}))
            .await
            .status(),
        400
    );
    assert_eq!(
        send("GET", "/v1/system/logs/files", &member, json!({})).await.status(),
        403
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
