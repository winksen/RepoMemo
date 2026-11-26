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
    assert!(overview["background"]["indexing_retries_scheduled"].is_number());
    assert!(overview["background"]["indexing_retries_waiting"].is_number());
    assert!(overview["background"].get("indexing_oldest_queue_seconds").is_some());
    assert!(overview["background"].get("embedding_oldest_queue_seconds").is_some());
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

#[tokio::test]
async fn a_deactivated_account_is_shut_out_everywhere_and_comes_back_when_reactivated() {
    let data_dir = temp_data_dir("deactivation");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (admin, admin_body) = register_account(&app, "deactivate-admin@example.com").await;
    let admin_id = admin_body["user"]["id"].as_str().unwrap().to_owned();
    let (owner, owner_body) = register_account(&app, "deactivate-owner@example.com").await;
    let owner_id = owner_body["user"]["id"].as_str().unwrap().to_owned();
    let (member, member_body) = register_account(&app, "deactivate-member@example.com").await;
    let member_id = member_body["user"]["id"].as_str().unwrap().to_owned();
    let member_refresh = member_body["refresh_token"].as_str().unwrap().to_owned();
    let workspace_id = create_workspace(&app, &owner).await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "deactivate-member@example.com",
        "admin",
    )
    .await;

    // The member has a script's token and an invitation out.
    let token = response_json(
        call(
            "POST",
            "/v1/profile/tokens".to_owned(),
            member.clone(),
            json!({"name": "script", "scope": "read", "expires_in_days": 7}),
        )
        .await,
    )
    .await;
    let secret = format!("Bearer {}", token["secret"].as_str().unwrap());
    let invitation = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/invitations"),
            member.clone(),
            json!({"email": "someone-new@example.com", "role": "viewer"}),
        )
        .await,
    )
    .await;
    let invitation_token = invitation["token"].as_str().unwrap().to_owned();
    assert_eq!(
        call("GET", "/v1/session".to_owned(), secret.clone(), json!({}))
            .await
            .status(),
        200
    );

    // Only a system administrator, never oneself, and an owner transfers first.
    let deactivate = |id: &str| format!("/v1/system/users/{id}/deactivate");
    assert_eq!(
        call("POST", deactivate(&member_id), owner.clone(), json!({}))
            .await
            .status(),
        403
    );
    assert_eq!(
        call("POST", deactivate(&admin_id), admin.clone(), json!({}))
            .await
            .status(),
        409,
        "not oneself"
    );
    let blocked = call("POST", deactivate(&owner_id), admin.clone(), json!({})).await;
    assert_eq!(blocked.status(), 409, "an owner transfers what they own first");
    assert!(response_json(blocked).await["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("Transfer the ownership"));

    // Deactivating ends everything of the member's at once.
    let ended = call("POST", deactivate(&member_id), admin.clone(), json!({})).await;
    assert_eq!(ended.status(), 200);
    let ended = response_json(ended).await;
    assert_eq!(ended["personal_tokens_revoked"], 1);
    assert_eq!(ended["invitations_withdrawn"], 1);
    assert_eq!(
        call("GET", "/v1/session".to_owned(), member.clone(), json!({}))
            .await
            .status(),
        401
    );
    assert_eq!(
        call("GET", "/v1/session".to_owned(), secret.clone(), json!({}))
            .await
            .status(),
        401
    );
    let refreshed = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/refresh",
            json!({"refresh_token": member_refresh}),
        ))
        .await
        .unwrap();
    assert_eq!(refreshed.status(), 401);
    let signed_in = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/login",
            json!({"email": "deactivate-member@example.com", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(signed_in.status(), 403, "said once the password is right");
    let wrong = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/login",
            json!({"email": "deactivate-member@example.com", "password": "not-the-right-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(wrong.status(), 401, "a wrong password says nothing about the account");
    let preview = app
        .clone()
        .oneshot(public_post(
            "/v1/invitations/preview",
            json!({"token": invitation_token}),
        ))
        .await
        .unwrap();
    assert_eq!(preview.status(), 410, "the invitation they sent is withdrawn");
    assert_eq!(
        call("POST", deactivate(&member_id), admin.clone(), json!({}))
            .await
            .status(),
        409,
        "already deactivated"
    );
    assert_eq!(
        call(
            "POST",
            format!("/v1/system/users/{member_id}/password-reset"),
            admin.clone(),
            json!({})
        )
        .await
        .status(),
        409,
        "no reset link for a deactivated account"
    );

    // The list says so, and what they wrote and belong to stays.
    let users = response_json(call("GET", "/v1/system/users".to_owned(), admin.clone(), json!({})).await).await;
    let listed = users
        .as_array()
        .unwrap()
        .iter()
        .find(|user| user["id"] == member_id.as_str())
        .unwrap();
    assert!(listed["deactivated_at"].is_string());
    assert_eq!(listed["workspace_count"], 1);

    // Reactivated, they sign in again with the same password.
    assert_eq!(
        call(
            "POST",
            format!("/v1/system/users/{owner_id}/reactivate"),
            admin.clone(),
            json!({})
        )
        .await
        .status(),
        409,
        "an active account is not reactivated"
    );
    assert_eq!(
        call(
            "POST",
            format!("/v1/system/users/{member_id}/reactivate"),
            admin.clone(),
            json!({})
        )
        .await
        .status(),
        204
    );
    let back = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/login",
            json!({"email": "deactivate-member@example.com", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(back.status(), 200);
    assert_eq!(
        call("GET", "/v1/session".to_owned(), member.clone(), json!({}))
            .await
            .status(),
        401,
        "the sessions that ended stay ended"
    );
    let audit = response_json(call("GET", "/v1/system/audit".to_owned(), admin.clone(), json!({})).await).await;
    let actions = audit
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["action"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert!(actions.contains(&"user_deactivated".to_owned()));
    assert!(actions.contains(&"user_reactivated".to_owned()));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn an_anonymized_account_loses_its_personal_data_and_what_it_wrote_stays() {
    let data_dir = temp_data_dir("anonymize");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (admin, _) = register_account(&app, "anonymize-admin@example.com").await;
    let (owner, _) = register_account(&app, "anonymize-owner@example.com").await;
    let (member, member_body) = register_account(&app, "anonymize-member@example.com").await;
    let member_id = member_body["user"]["id"].as_str().unwrap().to_owned();
    let workspace_id = create_workspace(&app, &owner).await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "anonymize-member@example.com",
        "member",
    )
    .await;
    let comment_target = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/tasks"),
            member.clone(),
            json!({"title": "Written by the member", "description": "", "status": "open", "priority": "medium"}),
        )
        .await,
    )
    .await;
    assert!(comment_target["id"].is_string(), "{comment_target}");

    // Only a deactivated account, and only with its address typed.
    let anonymize = |confirm: &str| json!({"confirm": confirm});
    let path = format!("/v1/system/users/{member_id}/anonymize");
    assert_eq!(
        call(
            "POST",
            path.clone(),
            admin.clone(),
            anonymize("anonymize-member@example.com")
        )
        .await
        .status(),
        409,
        "deactivate first"
    );
    assert_eq!(
        call(
            "POST",
            format!("/v1/system/users/{member_id}/deactivate"),
            admin.clone(),
            json!({})
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        call(
            "POST",
            path.clone(),
            admin.clone(),
            anonymize("someone-else@example.com")
        )
        .await
        .status(),
        400,
        "the address is typed to confirm"
    );
    assert_eq!(
        call(
            "POST",
            path.clone(),
            owner.clone(),
            anonymize("anonymize-member@example.com")
        )
        .await
        .status(),
        403
    );
    let done = call(
        "POST",
        path.clone(),
        admin.clone(),
        anonymize("Anonymize-Member@example.com"),
    )
    .await;
    assert_eq!(done.status(), 200);
    let done = response_json(done).await;
    assert_eq!(done["memberships_removed"], 1);

    // The account is a former member: no address, no sign-in, no way back.
    let users = response_json(call("GET", "/v1/system/users".to_owned(), admin.clone(), json!({})).await).await;
    let listed = users
        .as_array()
        .unwrap()
        .iter()
        .find(|user| user["id"] == member_id.as_str())
        .unwrap();
    assert_eq!(listed["display_name"], "Former member");
    assert!(listed["email"].as_str().unwrap().starts_with("former-member-"));
    assert!(listed["anonymized_at"].is_string());
    assert_eq!(listed["workspace_count"], 0);
    let signed_in = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/login",
            json!({"email": "anonymize-member@example.com", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(signed_in.status(), 401);
    assert_eq!(
        call(
            "POST",
            format!("/v1/system/users/{member_id}/reactivate"),
            admin.clone(),
            json!({})
        )
        .await
        .status(),
        409
    );
    assert_eq!(
        call("POST", path, admin.clone(), anonymize("anonymize-member@example.com"))
            .await
            .status(),
        409,
        "once"
    );

    // What they wrote stays, and nothing names them any more.
    let tasks = response_json(
        call(
            "GET",
            format!("/v1/workspaces/{workspace_id}/tasks"),
            owner.clone(),
            json!({}),
        )
        .await,
    )
    .await;
    assert!(tasks.to_string().contains("Written by the member"));
    assert!(!tasks.to_string().contains("anonymize-member@example.com"));
    let activity = response_json(
        call(
            "GET",
            format!("/v1/workspaces/{workspace_id}/activity"),
            owner.clone(),
            json!({}),
        )
        .await,
    )
    .await;
    let text = activity.to_string();
    assert!(!text.contains("anonymize-member@example.com"), "{text}");
    assert!(text.contains("Former member"), "{text}");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_audit_trail_downloads_for_a_date_range_and_the_download_is_audited() {
    let data_dir = temp_data_dir("audit-export");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (admin, _) = register_account(&app, "audit-admin@example.com").await;
    let (member, _) = register_account(&app, "audit-member@example.com").await;
    let get = |uri: String, who: &str| {
        let app = app.clone();
        let request = auth_request("GET", &uri, who);
        async move { app.oneshot(request).await.unwrap() }
    };
    for _ in 0..3 {
        let run = app
            .clone()
            .oneshot(json_request("POST", "/v1/system/maintenance/run", &admin, json!({})))
            .await
            .unwrap();
        assert_eq!(run.status(), 200);
    }
    let text = |response: axum::response::Response| async move {
        let status = response.status();
        let headers = response.headers().clone();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, headers, String::from_utf8(body.to_vec()).unwrap())
    };

    // JSON lines, the default: one event per line, oldest first.
    let (status, headers, body) = text(get("/v1/system/audit/export".to_owned(), &admin).await).await;
    assert_eq!(status, 200);
    assert_eq!(headers["content-type"], "application/x-ndjson");
    assert_eq!(
        headers["content-disposition"],
        "attachment; filename=\"repomemo-audit-start-now.jsonl\""
    );
    let lines = body.lines().collect::<Vec<_>>();
    assert_eq!(lines.len().to_string(), headers["x-total-count"].to_str().unwrap());
    let events = lines
        .iter()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        events
            .iter()
            .filter(|event| event["action"] == "maintenance_run")
            .count(),
        3
    );
    assert!(events
        .windows(2)
        .all(|pair| pair[0]["time"].as_str() <= pair[1]["time"].as_str()));
    let run = events
        .iter()
        .find(|event| event["action"] == "maintenance_run")
        .unwrap();
    assert_eq!(run["actor"]["email"], "audit-admin@example.com");
    assert!(run["detail"].as_str().is_some_and(|detail| !detail.is_empty()));

    // The download is on the record, and the next export holds it.
    let trail = response_json(get("/v1/system/audit".to_owned(), &admin).await).await;
    let recorded = trail
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["action"] == "audit_exported")
        .expect("the export is audited");
    assert!(
        recorded["detail"]
            .as_str()
            .unwrap()
            .contains("from the start to now as jsonl"),
        "{recorded}"
    );
    assert_eq!(recorded["actor"]["email"], "audit-admin@example.com");
    let (_, _, again) = text(get("/v1/system/audit/export?format=jsonl".to_owned(), &admin).await).await;
    assert!(again.lines().any(|line| line.contains("\"audit_exported\"")));

    // CSV: a header, then a row per event, with the same count.
    let (status, headers, csv) = text(get("/v1/system/audit/export?format=csv".to_owned(), &admin).await).await;
    assert_eq!(status, 200);
    assert_eq!(headers["content-type"], "text/csv; charset=utf-8");
    assert!(headers["content-disposition"].to_str().unwrap().ends_with(".csv\""));
    let rows = csv.split("\r\n").filter(|row| !row.is_empty()).collect::<Vec<_>>();
    assert_eq!(rows[0], "id,time,actor_id,actor_email,actor_name,action,detail");
    assert_eq!(
        rows.len() - 1,
        headers["x-total-count"].to_str().unwrap().parse::<usize>().unwrap()
    );
    assert!(rows.iter().any(|row| row.contains(",maintenance_run,")));

    // A date range: today's events by day, none for a day long ago, and refusals.
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let (status, headers, body) =
        text(get(format!("/v1/system/audit/export?from={today}&to={today}"), &admin).await).await;
    assert_eq!(status, 200);
    assert_eq!(
        headers["content-disposition"].to_str().unwrap(),
        format!("attachment; filename=\"repomemo-audit-{today}-{today}.jsonl\"")
    );
    assert!(body.lines().count() >= 3);
    let (status, headers, body) = text(
        get(
            "/v1/system/audit/export?from=2000-01-01&to=2000-01-31&format=csv".to_owned(),
            &admin,
        )
        .await,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(headers["x-total-count"], "0");
    assert_eq!(body, "id,time,actor_id,actor_email,actor_name,action,detail\r\n");
    for refused in [
        "format=xml",
        "from=yesterday",
        "to=2026-13-40",
        "from=2999-01-01",
        "from=2026-10-20&to=2026-10-01",
    ] {
        let response = get(format!("/v1/system/audit/export?{refused}"), &admin).await;
        assert_eq!(response.status(), 400, "{refused}");
    }

    // Only a system administrator may.
    assert_eq!(get("/v1/system/audit/export".to_owned(), &member).await.status(), 403);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// E3 and E4 (decision 22): who creates organizations, and who links a folder of the server's disk, are system
/// settings. System administrators only by default; the settings open each of them.
#[tokio::test]
async fn organizations_and_local_links_are_for_administrators_until_the_settings_say_otherwise() {
    let data_dir = temp_data_dir("creators-and-linkers");
    let mut config = ServerConfig::for_test(data_dir.clone());
    // What a fresh server starts with (the tests of everything else start more open).
    config.organization_creators = "administrators".to_owned();
    config.local_repository_linkers = "system_admins".to_owned();
    let repo_root = temp_data_dir("creators-linkers-repos");
    std::fs::create_dir_all(&repo_root).unwrap();
    config.repo_roots = vec![repo_root.clone()];
    let app = router(config).await.unwrap();
    let send = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (admin, _) = register_account(&app, "creators-admin@example.com").await;
    let (member, _) = register_account(&app, "creators-member@example.com").await;
    let may_create = |who: String| {
        let reply = send("GET", "/v1/session".to_owned(), &who, json!(null));
        async move {
            response_json(reply.await).await["can_create_organizations"]
                .as_bool()
                .unwrap()
        }
    };

    // Organizations: the administrator creates one, an ordinary account is turned away with a message.
    assert!(may_create(admin.clone()).await);
    assert!(
        !may_create(member.clone()).await,
        "the session says what the person cannot do"
    );
    let refused = send("POST", "/v1/organizations".to_owned(), &member, json!({"name": "Mine"})).await;
    assert_eq!(refused.status(), 403);
    let message = response_json(refused).await["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(message.contains("system administrators"), "{message}");
    let organization = response_json(
        send(
            "POST",
            "/v1/organizations".to_owned(),
            &admin,
            json!({"name": "Theirs"}),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let workspace_id = response_json(
        send(
            "POST",
            "/v1/workspaces".to_owned(),
            &admin,
            json!({"organization_id": organization, "name": "Workspace"}),
        )
        .await,
    )
    .await["workspace"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Local links: a workspace administrator who is not a system administrator may not link a folder of the disk.
    join(
        &app,
        &admin,
        "organizations",
        &organization,
        "creators-member@example.com",
        "admin",
    )
    .await;
    let link = json!({"link": repo_root.join("anything").to_string_lossy()});
    for route in ["repositories/check", "repositories"] {
        let refused = send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/{route}"),
            &member,
            link.clone(),
        )
        .await;
        assert_eq!(refused.status(), 403, "{route}");
        let message = response_json(refused).await["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        assert!(message.contains("system administrators"), "{route}: {message}");
        // A system administrator is past the right (the folder itself does not exist, which is another answer).
        let past = send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/{route}"),
            &admin,
            link.clone(),
        )
        .await;
        assert_ne!(past.status(), 403, "{route}");
    }
    // A remote address is not a local link: the setting does not touch it.
    let remote = send(
        "POST",
        format!("/v1/workspaces/{workspace_id}/repositories/check"),
        &member,
        json!({"link": "https://github.com/acme/app.git"}),
    )
    .await;
    let remote_message = response_json(remote).await["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(!remote_message.contains("system administrators"), "{remote_message}");

    // Opening each setting opens that right, and only that one.
    assert_eq!(
        send(
            "PUT",
            "/v1/system/settings".to_owned(),
            &admin,
            json!({"values": {"organization_creators": "everyone"}}),
        )
        .await
        .status(),
        200
    );
    assert!(may_create(member.clone()).await);
    assert_eq!(
        send("POST", "/v1/organizations".to_owned(), &member, json!({"name": "Mine"}))
            .await
            .status(),
        201
    );
    let still = send(
        "POST",
        format!("/v1/workspaces/{workspace_id}/repositories/check"),
        &member,
        link.clone(),
    )
    .await;
    assert_eq!(still.status(), 403, "opening one setting does not open the other");
    assert_eq!(
        send(
            "PUT",
            "/v1/system/settings".to_owned(),
            &admin,
            json!({"values": {"local_repository_linkers": "workspace_admins"}}),
        )
        .await
        .status(),
        200
    );
    let allowed = send(
        "POST",
        format!("/v1/workspaces/{workspace_id}/repositories/check"),
        &member,
        link,
    )
    .await;
    assert_ne!(
        allowed.status(),
        403,
        "workspace administrators may link a folder once the setting says so"
    );
    // Values outside the choices are refused.
    assert_eq!(
        send(
            "PUT",
            "/v1/system/settings".to_owned(),
            &admin,
            json!({"values": {"organization_creators": "anyone"}}),
        )
        .await
        .status(),
        400
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_server_can_serve_the_web_app_itself() {
    let data_dir = temp_data_dir("web-dir");
    let web = temp_data_dir("web-files");
    std::fs::create_dir_all(web.join("assets")).unwrap();
    std::fs::write(web.join("index.html"), "<div id=\"root\"></div>").unwrap();
    std::fs::write(web.join("assets").join("app-1.js"), "1").unwrap();
    std::fs::write(web.join("content-security-policy.txt"), "default-src 'self'").unwrap();
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.web_dir = Some(web.clone());
    let app = router(config).await.unwrap();
    let get = |uri: &str| {
        let app = app.clone();
        let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
        async move { app.oneshot(request).await.unwrap() }
    };

    let page = get("/").await;
    assert_eq!(page.status(), 200);
    assert_eq!(page.headers()["cache-control"], "no-cache");
    assert_eq!(page.headers()["x-content-type-options"], "nosniff");
    assert_eq!(page.headers()["content-security-policy"], "default-src 'self'");
    assert_eq!(
        get("/workspaces/anything/here").await.status(),
        200,
        "a client route gets the page"
    );
    let asset = get("/assets/app-1.js").await;
    assert_eq!(asset.headers()["cache-control"], "public, max-age=31536000, immutable");
    assert_eq!(get("/assets/missing.js").await.status(), 404);
    assert_eq!(
        get("/%2e%2e/secret").await.status(),
        200,
        "an encoded dot path is only a client route"
    );
    let config_json = response_json(get("/config.json").await).await;
    assert_eq!(config_json["apiUrl"], "");
    // The API still answers its own paths.
    assert_eq!(get("/v1/workspaces").await.status(), 401);
    assert_eq!(get("/health/ready").await.status(), 200);

    let _ = std::fs::remove_dir_all(web);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_clean_stop_leaves_a_database_that_needs_no_recovery() {
    let data_dir = temp_data_dir("clean-stop");
    let server = crate::start(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let app = server.router();
    register_account(&app, "stopping@example.com").await;

    server.begin_closing().await;
    server.finish().await;

    // The write-ahead log was folded into the database and removed on the way out.
    let wal = data_dir.join("repomemo.sqlite-wal");
    assert!(
        std::fs::metadata(&wal).map_or(true, |meta| meta.len() == 0),
        "the write-ahead log is empty after a clean stop"
    );
    // And the next start opens it as it is.
    let again = crate::start(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    again.finish().await;
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_system_administrator_makes_a_cairn_administrator() {
    let data_dir = temp_data_dir("cairn-admin");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (admin, _) = register_account(&app, "cairn-root@example.com").await;
    let (helper, helper_tokens) = register_account(&app, "cairn-helper@example.com").await;
    let helper_id = helper_tokens["user"]["id"].as_str().unwrap().to_owned();
    let uri = format!("/v1/system/users/{helper_id}/cairn-admin");

    let session = |who: String| {
        let app = app.clone();
        async move { response_json(app.oneshot(auth_request("GET", "/v1/session", &who)).await.unwrap()).await }
    };
    assert_eq!(session(helper.clone()).await["is_cairn_admin"], false);
    assert_eq!(
        session(admin.clone()).await["is_cairn_admin"],
        true,
        "system administrators are CAIRN administrators by right"
    );

    // A member cannot grant the role, not even to themselves.
    assert_eq!(
        send("PUT", uri.clone(), &helper, json!({"enabled": true}))
            .await
            .status(),
        403
    );
    assert_eq!(
        send("PUT", uri.clone(), &admin, json!({"enabled": true}))
            .await
            .status(),
        204
    );
    assert_eq!(session(helper.clone()).await["is_cairn_admin"], true);

    // The role is not a System page: a CAIRN administrator still cannot read System.
    let read = app
        .clone()
        .oneshot(auth_request("GET", "/v1/system/users", &helper))
        .await
        .unwrap();
    assert_eq!(read.status(), 403);
    assert_eq!(
        send("PUT", uri.clone(), &helper, json!({"enabled": false}))
            .await
            .status(),
        403
    );

    // The console lists it and changes it as a dry run first.
    let users = response_json(
        send(
            "POST",
            "/v1/system/console".to_owned(),
            &admin,
            json!({"command": "users list"}),
        )
        .await,
    )
    .await;
    assert!(users["view"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .any(|column| column == "is_cairn_admin"));
    let dry = response_json(
        send(
            "POST",
            "/v1/system/console".to_owned(),
            &admin,
            json!({"command": "users cairn-admin cairn-helper@example.com off"}),
        )
        .await,
    )
    .await;
    assert_eq!(dry["dry_run"], true, "{dry}");
    assert_eq!(session(helper.clone()).await["is_cairn_admin"], true);
    let applied = send(
        "POST",
        "/v1/system/console".to_owned(),
        &admin,
        json!({"command": "users cairn-admin cairn-helper@example.com off --yes"}),
    )
    .await;
    assert_eq!(applied.status(), 200);
    assert_eq!(session(helper.clone()).await["is_cairn_admin"], false);
    assert_eq!(
        send(
            "POST",
            "/v1/system/console".to_owned(),
            &admin,
            json!({"command": "users cairn-admin cairn-helper@example.com maybe --yes"}),
        )
        .await
        .status(),
        400
    );

    // Both changes are in the audit trail.
    let audit = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/system/audit", &admin))
            .await
            .unwrap(),
    )
    .await
    .to_string();
    assert!(audit.contains("cairn_admin_granted") && audit.contains("cairn_admin_revoked"));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn cairn_administration_is_open_to_cairn_and_system_administrators_and_read_by_app_administrators() {
    use crate::system::{require_cairn_admin, require_cairn_reader};
    let subject =
        |system: bool, app: bool, cairn: bool, token: Option<crate::TokenGrant>| crate::AuthenticatedSubject {
            user_id: "person".to_owned(),
            expires_at: 0,
            session_version: 0,
            is_system_admin: system,
            is_app_admin: app,
            is_cairn_admin: cairn,
            token,
        };
    let token = |system: bool| crate::TokenGrant {
        token_id: "token".to_owned(),
        read_only: false,
        admin: false,
        system,
        mcp: false,
        workspaces: None,
    };
    // A member is refused.
    let member = subject(false, false, false, None);
    assert!(require_cairn_admin(&member).is_err());
    assert!(require_cairn_reader(&member).is_err());
    // A CAIRN administrator and a system administrator change and read.
    for who in [subject(false, false, true, None), subject(true, false, false, None)] {
        assert!(require_cairn_admin(&who).is_ok());
        assert!(require_cairn_reader(&who).is_ok());
    }
    // An app administrator reads and cannot change.
    let reader = subject(false, true, false, None);
    assert!(require_cairn_reader(&reader).is_ok());
    assert!(require_cairn_admin(&reader).is_err());
    // A token changes things only with the system scope.
    assert!(require_cairn_admin(&subject(false, false, true, Some(token(false)))).is_err());
    assert!(require_cairn_admin(&subject(false, false, true, Some(token(true)))).is_ok());
}

#[tokio::test]
async fn the_cairn_administration_page_shows_each_person_only_what_they_may_open() {
    let data_dir = temp_data_dir("cairn-admin-page");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (admin, _) = register_account(&app, "cairn-page-root@example.com").await;
    let (cairn, cairn_tokens) = register_account(&app, "cairn-page-cairn@example.com").await;
    let (reader, reader_tokens) = register_account(&app, "cairn-page-reader@example.com").await;
    let (owner, _) = register_account(&app, "cairn-page-owner@example.com").await;
    let (stranger, _) = register_account(&app, "cairn-page-stranger@example.com").await;
    let cairn_id = cairn_tokens["user"]["id"].as_str().unwrap().to_owned();
    let reader_id = reader_tokens["user"]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        send(
            "PUT",
            format!("/v1/system/users/{cairn_id}/cairn-admin"),
            &admin,
            json!({"enabled": true})
        )
        .await
        .status(),
        204
    );
    assert_eq!(
        send(
            "PUT",
            format!("/v1/system/users/{reader_id}/app-admin"),
            &admin,
            json!({"enabled": true})
        )
        .await
        .status(),
        204
    );
    // An owner of an organization, and an organization that has a workspace.
    let organization = response_json(
        send(
            "POST",
            "/v1/organizations".to_owned(),
            &owner,
            json!({"name": "Page Org"}),
        )
        .await,
    )
    .await;
    let organization_id = organization["id"].as_str().unwrap().to_owned();
    let workspace = send(
        "POST",
        "/v1/workspaces".to_owned(),
        &owner,
        json!({"name": "Page Workspace", "organization_id": organization_id}),
    )
    .await;
    assert!(workspace.status().is_success(), "{}", workspace.status());

    let page = |who: String| {
        let app = app.clone();
        async move { app.oneshot(auth_request("GET", "/v1/cairn/admin", &who)).await.unwrap() }
    };

    // Everyone who has no part in administering CAIRN is refused.
    assert_eq!(page(stranger).await.status(), 403);

    // A system administrator and a CAIRN administrator see every section, today's setup and every organization.
    for who in [admin, cairn] {
        let view = response_json(page(who).await).await;
        assert_eq!(view["scope"], "server");
        assert_eq!(view["can_change"], true);
        assert_eq!(view["sections"].as_array().unwrap().len(), 8);
        assert!(view["organizations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|organization| organization["name"] == "Page Org"));
        let overview = &view["overview"];
        assert_eq!(overview["budgets"]["settings"].as_array().unwrap().len(), 5);
        assert!(overview["workspaces"]
            .as_array()
            .unwrap()
            .iter()
            .any(|workspace| workspace["workspace_name"] == "Page Workspace"
                && workspace["organization_name"] == "Page Org"));
    }

    // An app administrator reads the same page and changes nothing.
    let view = response_json(page(reader).await).await;
    assert_eq!(
        (view["scope"].clone(), view["can_change"].clone()),
        (json!("server"), json!(false))
    );

    // An organization's owner sees only the sections that concern their organizations, and no server-wide overview.
    let view = response_json(page(owner).await).await;
    assert_eq!(view["scope"], "organization");
    assert_eq!(
        view["sections"],
        json!(["organizations", "guardrails", "abilities", "memory", "usage"])
    );
    assert_eq!(view["organizations"].as_array().unwrap().len(), 1);
    assert!(view["overview"].is_null());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn cairn_administrators_change_the_run_budgets_and_each_change_is_audited() {
    let data_dir = temp_data_dir("cairn-budgets");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (admin, _) = register_account(&app, "budget-root@example.com").await;
    let (cairn, cairn_tokens) = register_account(&app, "budget-cairn@example.com").await;
    let (reader, reader_tokens) = register_account(&app, "budget-reader@example.com").await;
    let (member, _) = register_account(&app, "budget-member@example.com").await;
    let cairn_id = cairn_tokens["user"]["id"].as_str().unwrap().to_owned();
    let reader_id = reader_tokens["user"]["id"].as_str().unwrap().to_owned();
    for (kind, id) in [("cairn-admin", &cairn_id), ("app-admin", &reader_id)] {
        assert_eq!(
            send(
                "PUT",
                format!("/v1/system/users/{id}/{kind}"),
                &admin,
                json!({"enabled": true})
            )
            .await
            .status(),
            204
        );
    }
    let uri = "/v1/cairn/admin/budgets".to_owned();
    let budget = |view: &Value, key: &str| {
        view["overview"]["budgets"]["settings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|setting| setting["key"] == key)
            .cloned()
            .unwrap()
    };
    let page = |who: String| {
        let app = app.clone();
        async move { response_json(app.oneshot(auth_request("GET", "/v1/cairn/admin", &who)).await.unwrap()).await }
    };
    assert_eq!(budget(&page(cairn.clone()).await, "cairn_max_steps")["value"], 8);

    // Members and app administrators cannot change a budget.
    for who in [&member, &reader] {
        assert_eq!(
            send("PUT", uri.clone(), who, json!({"values": {"cairn_max_steps": 12}}))
                .await
                .status(),
            403
        );
    }
    // A value over the fixed maximum, a budget that is not one, and an empty request are refused, and nothing is saved.
    for body in [
        json!({"values": {"cairn_max_steps": 31}}),
        json!({"values": {"allow_registration": false}}),
        json!({"values": {"cairn_max_steps": 12, "cairn_max_run_seconds": 5}}),
        json!({}),
    ] {
        assert_eq!(send("PUT", uri.clone(), &cairn, body).await.status(), 400);
    }
    assert_eq!(budget(&page(cairn.clone()).await, "cairn_max_steps")["value"], 8);

    // A CAIRN administrator changes several budgets at once; the next read follows.
    let changed = send(
        "PUT",
        uri.clone(),
        &cairn,
        json!({"values": {"cairn_max_steps": 12, "cairn_max_run_tokens": 50000}}),
    )
    .await;
    assert_eq!(changed.status(), 200);
    let view = page(cairn.clone()).await;
    assert_eq!(budget(&view, "cairn_max_steps")["value"], 12);
    assert_eq!(budget(&view, "cairn_max_steps")["overridden"], true);
    assert_eq!(budget(&view, "cairn_max_run_tokens")["value"], 50000);
    assert_eq!(budget(&view, "cairn_max_run_seconds")["overridden"], false);

    // Restoring brings back the environment's value.
    assert_eq!(
        send("PUT", uri.clone(), &cairn, json!({"reset": ["cairn_max_steps"]}))
            .await
            .status(),
        200
    );
    let view = page(admin.clone()).await;
    assert_eq!(budget(&view, "cairn_max_steps")["value"], 8);
    assert_eq!(budget(&view, "cairn_max_steps")["overridden"], false);
    assert_eq!(budget(&view, "cairn_max_run_tokens")["value"], 50000);

    // The audit trail keeps the values before and after.
    let audit = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/system/audit", &admin))
            .await
            .unwrap(),
    )
    .await
    .to_string();
    assert!(audit.contains("cairn_budgets_changed"), "{audit}");
    assert!(audit.contains("CAIRN steps per request from 8 to 12"), "{audit}");
    assert!(audit.contains("CAIRN steps per request from 12 to 8"), "{audit}");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn cairn_administrators_manage_connections_without_the_key_ever_coming_back() {
    let data_dir = temp_data_dir("cairn-connections");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (admin, _) = register_account(&app, "conn-root@example.com").await;
    let (cairn, cairn_tokens) = register_account(&app, "conn-cairn@example.com").await;
    let (reader, reader_tokens) = register_account(&app, "conn-reader@example.com").await;
    let (member, _) = register_account(&app, "conn-member@example.com").await;
    for (kind, tokens) in [("cairn-admin", &cairn_tokens), ("app-admin", &reader_tokens)] {
        let id = tokens["user"]["id"].as_str().unwrap();
        assert_eq!(
            send(
                "PUT",
                format!("/v1/system/users/{id}/{kind}"),
                &admin,
                json!({"enabled": true})
            )
            .await
            .status(),
            204
        );
    }
    let uri = "/v1/cairn/connections".to_owned();
    let local = json!({"kind": "ollama", "name": "Office Ollama", "base_url": "http://127.0.0.1:11434"});

    // A member is refused everything; an app administrator reads and changes nothing.
    assert_eq!(send("GET", uri.clone(), &member, Value::Null).await.status(), 403);
    assert_eq!(send("POST", uri.clone(), &member, local.clone()).await.status(), 403);
    assert_eq!(send("GET", uri.clone(), &reader, Value::Null).await.status(), 200);
    assert_eq!(send("POST", uri.clone(), &reader, local.clone()).await.status(), 403);

    // An address outside the allowed hosts, or a metadata service, is refused, as for a workspace's provider.
    for body in [
        json!({"kind": "openai_compatible", "name": "Elsewhere", "base_url": "https://elsewhere.example/v1"}),
        json!({"kind": "openai_compatible", "name": "Metadata", "base_url": "http://169.254.169.254/v1"}),
        json!({"kind": "ollama", "name": "Scheme", "base_url": "ftp://127.0.0.1:11434"}),
        json!({"kind": "anthropic", "name": "Unknown kind"}),
        json!({"kind": "ollama", "name": "", "base_url": "http://127.0.0.1:11434"}),
    ] {
        assert_eq!(send("POST", uri.clone(), &cairn, body).await.status(), 400);
    }

    // A local connection is made; a cloud one needs the key and the acknowledgement that content leaves the network.
    let made = send("POST", uri.clone(), &cairn, local).await;
    assert_eq!(made.status(), 201);
    let local = response_json(made).await;
    assert_eq!(
        (local["locality"].clone(), local["has_key"].clone()),
        (json!("local"), json!(false))
    );
    let secret = "sk-or-very-secret-value";
    let cloud = json!({"kind": "openrouter", "name": "OpenRouter", "api_key": secret});
    assert_eq!(
        send("POST", uri.clone(), &cairn, cloud.clone()).await.status(),
        400,
        "no acknowledgement"
    );
    assert_eq!(
        send(
            "POST",
            uri.clone(),
            &cairn,
            json!({"kind": "openrouter", "name": "OpenRouter", "cloud_content_acknowledged": true})
        )
        .await
        .status(),
        400,
        "no key"
    );
    let mut acknowledged = cloud.clone();
    acknowledged["cloud_content_acknowledged"] = json!(true);
    let made = send("POST", uri.clone(), &cairn, acknowledged).await;
    assert_eq!(made.status(), 201);
    let cloud = response_json(made).await;
    assert_eq!(
        (cloud["locality"].clone(), cloud["has_key"].clone()),
        (json!("cloud"), json!(true))
    );
    assert!(cloud["cloud_acknowledged_at"].is_string());

    // The key is in no response and no audit event.
    let listed = response_json(send("GET", uri.clone(), &reader, Value::Null).await).await;
    assert_eq!(listed.as_array().unwrap().len(), 2);
    assert!(!listed.to_string().contains(secret));
    assert!(!cloud.to_string().contains(secret));

    // A stale save is refused; the same address keeps the key, another needs it again; a rename and turning off work.
    let id = cloud["id"].as_str().unwrap().to_owned();
    let change_uri = format!("/v1/cairn/connections/{id}");
    let stale = json!({"name": "OpenRouter team", "enabled": false, "expected_version": 99});
    assert_eq!(send("PUT", change_uri.clone(), &cairn, stale).await.status(), 409);
    let renamed = send(
        "PUT",
        change_uri.clone(),
        &cairn,
        json!({"name": "OpenRouter team", "enabled": false, "expected_version": cloud["version"]}),
    )
    .await;
    assert_eq!(renamed.status(), 200);
    let renamed = response_json(renamed).await;
    assert_eq!(
        (
            renamed["name"].clone(),
            renamed["enabled"].clone(),
            renamed["has_key"].clone()
        ),
        (json!("OpenRouter team"), json!(false), json!(true))
    );
    let same_address = json!({"name": "OpenRouter team", "enabled": false, "base_url": "https://openrouter.ai/api/v1/", "expected_version": renamed["version"]});
    let kept = send("PUT", change_uri.clone(), &cairn, same_address).await;
    assert_eq!(kept.status(), 200, "the same address keeps the key");
    let kept = response_json(kept).await;
    assert_eq!(kept["has_key"], true);
    let other_address = json!({"name": "x", "enabled": false, "base_url": "http://127.0.0.1:11434", "expected_version": kept["version"]});
    assert_eq!(
        send("PUT", change_uri, &cairn, other_address).await.status(),
        400,
        "another address needs the key entered again"
    );

    // A test reports whether the server answered; nothing listens here, and the report holds no key.
    let tested = send(
        "POST",
        format!("/v1/cairn/connections/{}/test", local["id"].as_str().unwrap()),
        &cairn,
        json!({}),
    )
    .await;
    assert_eq!(tested.status(), 200);
    let tested = response_json(tested).await;
    assert!(tested["reachable"].is_boolean());
    assert!(!tested.to_string().contains(secret));

    // Deleting removes the connection.
    assert_eq!(
        send("DELETE", format!("/v1/cairn/connections/{id}"), &cairn, Value::Null)
            .await
            .status(),
        204
    );
    assert_eq!(
        response_json(send("GET", uri, &admin, Value::Null).await)
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let audit = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/system/audit", &admin))
            .await
            .unwrap(),
    )
    .await
    .to_string();
    for action in [
        "cairn_connection_added",
        "cairn_connection_changed",
        "cairn_connection_tested",
        "cairn_connection_deleted",
    ] {
        assert!(audit.contains(action), "{action} missing");
    }
    assert!(!audit.contains(secret), "the key is not in the audit trail");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn cairn_administrators_add_models_to_a_connection_and_correct_what_a_test_finds() {
    let data_dir = temp_data_dir("cairn-models");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (admin, _) = register_account(&app, "model-root@example.com").await;
    let (reader, reader_tokens) = register_account(&app, "model-reader@example.com").await;
    let (member, _) = register_account(&app, "model-member@example.com").await;
    let reader_id = reader_tokens["user"]["id"].as_str().unwrap();
    assert_eq!(
        send(
            "PUT",
            format!("/v1/system/users/{reader_id}/app-admin"),
            &admin,
            json!({"enabled": true})
        )
        .await
        .status(),
        204
    );
    let connection = response_json(
        send(
            "POST",
            "/v1/cairn/connections".to_owned(),
            &admin,
            json!({"kind": "ollama", "name": "Office Ollama", "base_url": "http://127.0.0.1:11434"}),
        )
        .await,
    )
    .await;
    let connection_id = connection["id"].as_str().unwrap().to_owned();
    let models = "/v1/cairn/models".to_owned();
    let add = json!({"connection_id": connection_id, "model_id": "llama3.1:8b", "purpose": "text"});

    // Members are refused, app administrators read, and nobody but a CAIRN or system administrator changes the catalog.
    assert_eq!(send("GET", models.clone(), &member, Value::Null).await.status(), 403);
    assert_eq!(send("GET", models.clone(), &reader, Value::Null).await.status(), 200);
    assert_eq!(send("POST", models.clone(), &reader, add.clone()).await.status(), 403);
    assert_eq!(
        send(
            "GET",
            format!("/v1/cairn/connections/{connection_id}/available-models"),
            &reader,
            Value::Null
        )
        .await
        .status(),
        403
    );

    // A model is added with a name and a purpose; the display name defaults to the model's own, and it is added once.
    let made = send("POST", models.clone(), &admin, add.clone()).await;
    assert_eq!(made.status(), 201);
    let model = response_json(made).await;
    assert_eq!(
        (
            model["display_name"].clone(),
            model["purpose"].clone(),
            model["tool_mode"].clone(),
            model["uses"].clone()
        ),
        (json!("llama3.1:8b"), json!("text"), json!("none"), json!(0))
    );
    assert_eq!(send("POST", models.clone(), &admin, add).await.status(), 409);
    for bad in [
        json!({"connection_id": connection_id, "model_id": "x", "purpose": "audio"}),
        json!({"connection_id": "missing", "model_id": "x", "purpose": "text"}),
        json!({"connection_id": connection_id, "model_id": " ", "purpose": "text"}),
    ] {
        assert!(send("POST", models.clone(), &admin, bad)
            .await
            .status()
            .is_client_error());
    }

    // The administrator corrects what a test would find; a stale save is refused.
    let model_uri = format!("/v1/cairn/models/{}", model["id"].as_str().unwrap());
    let correction = json!({"display_name": "Llama 3.1 (8B)", "tool_mode": "native", "context_tokens": 32768, "vision": false, "enabled": true, "expected_version": model["version"]});
    let corrected = send("PUT", model_uri.clone(), &admin, correction.clone()).await;
    assert_eq!(corrected.status(), 200);
    let corrected = response_json(corrected).await;
    assert_eq!(
        (
            corrected["tool_mode"].clone(),
            corrected["context_tokens"].clone(),
            corrected["version"].clone()
        ),
        (json!("native"), json!(32768), json!(2))
    );
    assert_eq!(send("PUT", model_uri.clone(), &admin, correction).await.status(), 409);
    let listed = response_json(
        send(
            "GET",
            format!("{models}?connection_id={connection_id}"),
            &reader,
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(listed.as_array().unwrap().len(), 1);

    // Finding and testing reach the server; nothing answers here, which is said without the server's words.
    let found = send(
        "GET",
        format!("/v1/cairn/connections/{connection_id}/available-models"),
        &admin,
        Value::Null,
    )
    .await;
    assert!(found.status().is_client_error() || found.status().is_success());
    let tested = send("POST", format!("{model_uri}/test"), &admin, json!({})).await;
    assert_eq!(tested.status(), 200);
    let tested = response_json(tested).await;
    assert!(tested["success"].is_boolean());
    if tested["success"] == false {
        assert_eq!(tested["model"]["tool_mode"], "native", "a failed test changes nothing");
    }

    // A model of a connection that is off is not tested, and deleting removes the model.
    let current = response_json(send("GET", "/v1/cairn/connections".to_owned(), &admin, Value::Null).await).await;
    let off = json!({"name": "Office Ollama", "enabled": false, "expected_version": current[0]["version"]});
    assert_eq!(
        send("PUT", format!("/v1/cairn/connections/{connection_id}"), &admin, off)
            .await
            .status(),
        200
    );
    assert_eq!(
        send("POST", format!("{model_uri}/test"), &admin, json!({}))
            .await
            .status(),
        400
    );
    assert_eq!(send("DELETE", model_uri, &admin, Value::Null).await.status(), 204);
    assert!(response_json(send("GET", models, &admin, Value::Null).await)
        .await
        .as_array()
        .unwrap()
        .is_empty());
    let audit = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/system/audit", &admin))
            .await
            .unwrap(),
    )
    .await
    .to_string();
    for action in ["cairn_model_added", "cairn_model_changed", "cairn_model_deleted"] {
        assert!(audit.contains(action), "{action} missing");
    }
    let _ = std::fs::remove_dir_all(data_dir);
}
