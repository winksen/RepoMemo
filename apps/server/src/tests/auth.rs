use super::*;
use axum::http::StatusCode;

#[tokio::test]
async fn identity_completion_issues_the_existing_session_once() {
    let dir = temp_data_dir("identity-complete");
    let app = router(ServerConfig::for_test(dir.clone())).await.unwrap();
    let (_, account) = register_account(&app, "identity@example.com").await;
    let user = account["user"]["id"].as_str().unwrap();
    let storage = repomemo_storage::StorageEngine::open(repomemo_storage::StorageConfig::new(dir))
        .await
        .unwrap();
    let provider = repomemo_storage::IdentityProvider {
        enabled: true,
        checked_at: Some("checked".into()),
        ..Default::default()
    };
    storage
        .save_identity_provider(&provider, "test-secret", user)
        .await
        .unwrap();
    storage
        .create_identity_flow("state", "proof", &json!({}))
        .await
        .unwrap();
    storage.take_identity_callback("state").await.unwrap();
    storage
        .finish_identity_flow(
            "state",
            &json!({"user_id":user,"version":0,"provider":provider,"confirm":repomemo_storage::identity_confirmation_hash("12345678")}),
            true,
        )
        .await
        .unwrap();
    let response = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/identity/complete",
            json!({"state":"state","proof":"wrong"}),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    let complete = |code: Option<&str>| {
        public_post(
            "/v1/auth/identity/complete",
            json!({"state":"state","proof":"proof","code":code}),
        )
    };
    // The phished-link case: the initiating client holds the proof but never saw the code.
    let response = app.clone().oneshot(complete(None)).await.unwrap();
    assert_eq!(response.status(), 202);
    assert_eq!(response_json(response).await["status"], "confirm");
    assert_eq!(
        app.clone().oneshot(complete(Some("00000000"))).await.unwrap().status(),
        409
    );
    let response = app.clone().oneshot(complete(Some("12345678"))).await.unwrap();
    assert_eq!(response.status(), 200);
    let session = response_json(response).await;
    assert!(session["access_token"].is_string());
    assert!(session["refresh_token"].is_string());
    assert_eq!(session["user"]["id"], user);
    assert_eq!(app.oneshot(complete(Some("12345678"))).await.unwrap().status(), 400);
}

#[tokio::test]
async fn identity_settings_require_a_system_session_and_never_return_secrets() {
    let app = router(ServerConfig::for_test(temp_data_dir("identity-settings")))
        .await
        .unwrap();
    let (admin, _) = register_account(&app, "owner@example.com").await;
    let (member, _) = register_account(&app, "member@example.com").await;
    let config = json!({"name":"Example","issuer":"https://identity.example.com","client_id":"repomemo","client_secret":"only-stored-sealed","enabled":false});
    let response = app
        .clone()
        .oneshot(json_request("PUT", "/v1/system/identity", &member, config.clone()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = app
        .clone()
        .oneshot(json_request("PUT", "/v1/system/identity", &admin, config))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["has_secret"], true);
    assert!(!body.to_string().contains("only-stored-sealed"));
    let response = app
        .clone()
        .oneshot(auth_request("GET", "/v1/system/identity", &admin))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response_json(response).await.to_string().contains("only-stored-sealed"));
    let changed = json!({"name":"Example","issuer":"https://other.example.com","client_id":"repomemo","enabled":false});
    assert_eq!(
        app.oneshot(json_request("PUT", "/v1/system/identity", &admin, changed))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn refresh_tokens_rotate_and_detect_reuse() {
    let data_dir = std::env::temp_dir().join(format!("repomemo-server-test-{}", uuid::Uuid::new_v4()));
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let post = |uri: &str, body: String| {
        Request::builder()
            .method("POST")
            .uri(uri.to_owned())
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap()
    };
    let response = app
        .clone()
        .oneshot(post(
            "/v1/auth/register",
            r#"{"email":"refresh@example.com","display_name":"Refresh","password":"not-a-real-password"}"#.to_owned(),
        ))
        .await
        .unwrap();
    let first: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let first_refresh = first["refresh_token"].as_str().unwrap().to_owned();

    let response = app
        .clone()
        .oneshot(post(
            "/v1/auth/refresh",
            format!(r#"{{"refresh_token":"{first_refresh}"}}"#),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let second: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let second_refresh = second["refresh_token"].as_str().unwrap().to_owned();
    assert_ne!(first_refresh, second_refresh);

    // Replaying the rotated token a moment later (the other tab of a browser) is refused with a
    // conflict, and the session it was rotated into goes on.
    let response = app
        .clone()
        .oneshot(post(
            "/v1/auth/refresh",
            format!(r#"{{"refresh_token":"{first_refresh}"}}"#),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    let response = app
        .clone()
        .oneshot(post(
            "/v1/auth/refresh",
            format!(r#"{{"refresh_token":"{second_refresh}"}}"#),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), 200, "two tabs at once end no session");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn registers_and_protects_shared_workspace_routes() {
    let data_dir = std::env::temp_dir().join(format!("repomemo-server-test-{}", uuid::Uuid::new_v4()));
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let register = Request::builder()
        .method("POST")
        .uri("/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"owner@example.com","display_name":"Owner","password":"not-a-real-password"}"#,
        ))
        .unwrap();
    let response = app.clone().oneshot(register).await.unwrap();
    assert_eq!(response.status(), 201);
    let registration: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let _token = registration["access_token"].as_str().unwrap();
    let protected = Request::builder().uri("/v1/workspaces").body(Body::empty()).unwrap();
    let response = app.oneshot(protected).await.unwrap();
    assert_eq!(response.status(), 401);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn signing_out_everywhere_ends_access_and_refresh_tokens() {
    let data_dir = temp_data_dir("logout-all");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, tokens) = register_account(&app, "everywhere@example.com").await;
    let session = app
        .clone()
        .oneshot(auth_request("GET", "/v1/session", &owner))
        .await
        .unwrap();
    assert_eq!(session.status(), 200);

    let ended = app
        .clone()
        .oneshot(auth_request("POST", "/v1/auth/logout-all", &owner))
        .await
        .unwrap();
    assert_eq!(ended.status(), 204);
    let session = app
        .clone()
        .oneshot(auth_request("GET", "/v1/session", &owner))
        .await
        .unwrap();
    assert_eq!(session.status(), 401, "the access token stops working at once");
    let refreshed = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/refresh",
            json!({"refresh_token": tokens["refresh_token"]}),
        ))
        .await
        .unwrap();
    assert_eq!(refreshed.status(), 401);

    // Signing in again starts a new, working session.
    let login = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/login",
            json!({"email": "everywhere@example.com", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(login.status(), 200);
    let fresh = format!(
        "Bearer {}",
        response_json(login).await["access_token"].as_str().unwrap()
    );
    let session = app
        .clone()
        .oneshot(auth_request("GET", "/v1/session", &fresh))
        .await
        .unwrap();
    assert_eq!(session.status(), 200);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_web_app_keeps_its_refresh_token_in_a_cookie() {
    let data_dir = temp_data_dir("refresh-cookie");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let web = |uri: &str, cookie: Option<&str>, body: Value| {
        let mut request = Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .header("x-repomemo-client", "web");
        if let Some(cookie) = cookie {
            request = request.header("cookie", format!("repomemo_refresh={cookie}"));
        }
        request.body(Body::from(body.to_string())).unwrap()
    };
    let set_cookie = |response: &axum::response::Response| {
        response
            .headers()
            .get("set-cookie")
            .map(|value| value.to_str().unwrap().to_owned())
            .unwrap_or_default()
    };
    let token_of = |cookie: &str| {
        cookie
            .split(';')
            .next()
            .unwrap()
            .trim_start_matches("repomemo_refresh=")
            .to_owned()
    };

    let registered = app
        .clone()
        .oneshot(web(
            "/v1/auth/register",
            None,
            json!({"email": "cookie-user@example.com", "display_name": "Cookie", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(registered.status(), 201);
    let cookie = set_cookie(&registered);
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict") && cookie.contains("Path=/v1/auth"),
        "{cookie}"
    );
    let body = response_json(registered).await;
    assert_eq!(
        body["refresh_token"], "",
        "the token is not in the body for the web app"
    );
    let first = token_of(&cookie);
    assert!(!first.is_empty());

    // The cookie alone renews the session, and rotates.
    let refreshed = app
        .clone()
        .oneshot(web("/v1/auth/refresh", Some(&first), json!({})))
        .await
        .unwrap();
    assert_eq!(refreshed.status(), 200);
    let second = token_of(&set_cookie(&refreshed));
    assert!(!second.is_empty() && second != first);
    assert!(response_json(refreshed).await["access_token"].as_str().is_some());
    // The second tab presents the cookie the first one just replaced: refused, and its cookie is not cleared
    // (that would delete the new one the first tab received), so it renews again with the cookie it now holds.
    let crossed = app
        .clone()
        .oneshot(web("/v1/auth/refresh", Some(&first), json!({})))
        .await
        .unwrap();
    assert_eq!(crossed.status(), 409, "a token used a moment ago is refused");
    assert!(
        crossed.headers().get("set-cookie").is_none(),
        "and the cookie is left alone"
    );

    // Other clients still get the token in the body and send it there.
    let script = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/login",
            json!({"email": "cookie-user@example.com", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert!(script.headers().get("set-cookie").is_none());
    assert!(!response_json(script).await["refresh_token"]
        .as_str()
        .unwrap()
        .is_empty());

    // Signing out revokes the token and clears the cookie.
    let out = app
        .clone()
        .oneshot(web("/v1/auth/logout", Some(&second), json!({})))
        .await
        .unwrap();
    assert_eq!(out.status(), 204);
    assert!(set_cookie(&out).contains("Max-Age=0"));
    assert_eq!(
        app.clone()
            .oneshot(web("/v1/auth/refresh", Some(&second), json!({})))
            .await
            .unwrap()
            .status(),
        401
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn passwords_must_not_be_common_or_built_from_the_email() {
    assert!(
        crate::validate_password("Password1234", None).is_err(),
        "well known, whatever the case"
    );
    assert!(
        crate::validate_password("aaaabbbbaaaabbbb", None).is_err(),
        "too few different characters"
    );
    assert!(
        crate::validate_password("jane.doe-is-here", Some("Jane.Doe@example.com")).is_err(),
        "contains the address"
    );
    assert!(crate::validate_password("short", None).is_err());
    assert!(crate::validate_password("correct horse battery", Some("jane@example.com")).is_ok());
    assert!(
        crate::validate_password("with-a-tiny-local-ab-part", Some("ab@example.com")).is_ok(),
        "short local parts are not compared"
    );
}

#[tokio::test]
async fn repeated_failed_sign_ins_lock_the_account_for_a_while() {
    let data_dir = temp_data_dir("lockout");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.login_max_failures = 3;
    let app = router(config).await.unwrap();
    register_account(&app, "locked@example.com").await;
    register_account(&app, "unaffected@example.com").await;
    let login =
        |email: &str, password: &str| public_post("/v1/auth/login", json!({"email": email, "password": password}));

    for _ in 0..3 {
        let failed = app
            .clone()
            .oneshot(login("locked@example.com", "wrong-password-123"))
            .await
            .unwrap();
        assert_eq!(failed.status(), 401);
    }
    let locked = app
        .clone()
        .oneshot(login("locked@example.com", "not-a-real-password"))
        .await
        .unwrap();
    assert_eq!(locked.status(), 429, "even the right password waits out the lockout");
    assert!(locked.headers().contains_key("retry-after"));
    assert_eq!(response_json(locked).await["error"]["code"], "rate_limited");

    let other = app
        .clone()
        .oneshot(login("unaffected@example.com", "not-a-real-password"))
        .await
        .unwrap();
    assert_eq!(other.status(), 200, "other accounts are not affected");
    // A device that still holds the account's session is not locked out by
    // other people's failures.
    let session = response_json(
        app.clone()
            .oneshot(login("unaffected@example.com", "not-a-real-password"))
            .await
            .unwrap(),
    )
    .await;
    let refresh_token = session["refresh_token"].as_str().unwrap().to_owned();
    for _ in 0..3 {
        let failed = app
            .clone()
            .oneshot(login("unaffected@example.com", "wrong-password-123"))
            .await
            .unwrap();
        assert_eq!(failed.status(), 401);
    }
    assert_eq!(
        app.clone()
            .oneshot(login("unaffected@example.com", "not-a-real-password"))
            .await
            .unwrap()
            .status(),
        429
    );
    let known = app
        .clone()
        .oneshot(public_post("/v1/auth/login", json!({"email": "unaffected@example.com", "password": "not-a-real-password", "refresh_token": refresh_token})))
        .await
        .unwrap();
    assert_eq!(known.status(), 200, "the account's own device gets through the lockout");
    let unknown = app
        .clone()
        .oneshot(login("nobody@example.com", "not-a-real-password"))
        .await
        .unwrap();
    assert_eq!(unknown.status(), 401);
    assert_eq!(response_json(unknown).await["error"]["code"], "invalid_credentials");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_authentication_rate_limit_answers_429() {
    let data_dir = temp_data_dir("auth-quota");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.auth_quota = crate::Quota::per_minute(2);
    let app = router(config).await.unwrap();
    let sign_in = || {
        public_post(
            "/v1/auth/login",
            json!({"email": "nobody@example.com", "password": "not-a-real-password"}),
        )
    };
    for _ in 0..2 {
        assert_eq!(app.clone().oneshot(sign_in()).await.unwrap().status(), 401);
    }
    assert_eq!(app.clone().oneshot(sign_in()).await.unwrap().status(), 429);

    // Refreshes have their own, wider bucket.
    let refresh = || public_post("/v1/auth/refresh", json!({"refresh_token": "unknown"}));
    for _ in 0..8 {
        assert_eq!(app.clone().oneshot(refresh()).await.unwrap().status(), 401);
    }
    assert_eq!(app.clone().oneshot(refresh()).await.unwrap().status(), 429);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn health_names_the_build_that_answers() {
    let data_dir = temp_data_dir("health-build");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let health = response_json(
        app.oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(health["version"], env!("CARGO_PKG_VERSION"));
    for field in ["commit", "built_at", "image"] {
        assert!(health[field].is_string(), "/health names the {field}");
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn closed_registration_only_admits_the_first_account() {
    let data_dir = temp_data_dir("closed-registration");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.allow_registration = false;
    let app = router(config).await.unwrap();
    let health = response_json(
        app.clone()
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await;
    assert!(
        health.get("registration_open").is_none(),
        "the health check does not advertise registration"
    );
    register_account(&app, "first@example.com").await;

    let second = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/register",
            json!({"email": "second@example.com", "display_name": "Second", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(second.status(), 403);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn responses_carry_security_headers_and_readiness_checks_the_database() {
    let data_dir = temp_data_dir("headers");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let health = app
        .clone()
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let headers = health.headers();
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(headers["x-frame-options"], "DENY");
    assert_eq!(headers["referrer-policy"], "no-referrer");
    assert_eq!(headers["cache-control"], "no-store");
    assert!(headers.contains_key("content-security-policy"));

    let ready = app
        .clone()
        .oneshot(Request::builder().uri("/health/ready").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(ready.status(), 200);
    assert_eq!(response_json(ready).await["database"], "ok");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn invitations_reset_links_and_personal_tokens() {
    let data_dir = temp_data_dir("access-links");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    // The first account administers the server.
    let (owner, _) = register_account(&app, "access-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
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
    let public = |uri: &'static str, body: Value| {
        let app = app.clone();
        async move { app.oneshot(public_post(uri, body)).await.unwrap() }
    };

    // Invitation: registration is closed, but the invited address may join.
    assert_eq!(
        send(
            "PUT",
            "/v1/system/settings".to_owned(),
            &owner,
            json!({"values": {"allow_registration": false}})
        )
        .await
        .status(),
        200
    );
    let created = response_json(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/invitations"),
            &owner,
            json!({"email": "Invited@example.com", "role": "viewer"}),
        )
        .await,
    )
    .await;
    let token = created["token"].as_str().unwrap().to_owned();
    assert_eq!(
        created["path"],
        json!(format!("/invite#{token}")),
        "the secret is in the fragment"
    );
    assert_eq!(created["invitation"]["email"], "invited@example.com");
    let preview = public("/v1/invitations/preview", json!({"token": token})).await;
    assert_eq!(preview.status(), 200);
    assert_eq!(response_json(preview).await["account_exists"], false);
    let other = public("/v1/auth/register", json!({"email": "other@example.com", "display_name": "Other", "password": "not-a-real-password", "invitation_token": token})).await;
    assert_eq!(other.status(), 400, "the invitation is for one address");
    let uninvited = public(
        "/v1/auth/register",
        json!({"email": "other@example.com", "display_name": "Other", "password": "not-a-real-password"}),
    )
    .await;
    assert_eq!(uninvited.status(), 403, "registration stays closed for everyone else");
    let registered = public("/v1/auth/register", json!({"email": "invited@example.com", "display_name": "Invited", "password": "not-a-real-password", "invitation_token": token})).await;
    assert_eq!(registered.status(), 201);
    let registered = response_json(registered).await;
    let invited = format!("Bearer {}", registered["access_token"].as_str().unwrap());
    let invited_id = registered["user"]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        get(format!("/v1/workspaces/{workspace_id}/overview"), &invited)
            .await
            .status(),
        403,
        "joining needs the acceptance"
    );
    let organization_id = response_json(get("/v1/organizations".to_owned(), &owner).await).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let sibling = response_json(
        send(
            "POST",
            "/v1/workspaces".to_owned(),
            &owner,
            json!({"organization_id": organization_id, "name": "Sibling"}),
        )
        .await,
    )
    .await["workspace"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let accepted = send(
        "POST",
        "/v1/invitations/accept".to_owned(),
        &invited,
        json!({"token": token}),
    )
    .await;
    assert_eq!(accepted.status(), 200);
    assert_eq!(response_json(accepted).await["role"], "viewer");
    assert_eq!(
        get(format!("/v1/workspaces/{workspace_id}/overview"), &invited)
            .await
            .status(),
        200
    );
    assert_eq!(
        get(format!("/v1/workspaces/{sibling}/overview"), &invited)
            .await
            .status(),
        403,
        "a workspace invitation opens that workspace only"
    );
    assert_eq!(
        send(
            "POST",
            "/v1/invitations/accept".to_owned(),
            &invited,
            json!({"token": token})
        )
        .await
        .status(),
        410,
        "a link works once"
    );
    assert_eq!(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/invitations"),
            &invited,
            json!({"email": "x@example.com", "role": "viewer"})
        )
        .await
        .status(),
        403,
        "viewers cannot invite"
    );

    // Password reset link from the system administrator.
    assert_eq!(
        send(
            "POST",
            format!("/v1/system/users/{invited_id}/password-reset"),
            &invited,
            json!({})
        )
        .await
        .status(),
        403
    );
    let issued = response_json(
        send(
            "POST",
            format!("/v1/system/users/{invited_id}/password-reset"),
            &owner,
            json!({}),
        )
        .await,
    )
    .await;
    let reset = issued["token"].as_str().unwrap().to_owned();
    assert_eq!(
        public("/v1/auth/password-reset/preview", json!({"token": reset}))
            .await
            .status(),
        200
    );
    assert_eq!(
        public("/v1/auth/password-reset", json!({"token": reset, "password": "short"}))
            .await
            .status(),
        400
    );
    assert_eq!(
        public(
            "/v1/auth/password-reset",
            json!({"token": reset, "password": "a-brand-new-password"})
        )
        .await
        .status(),
        200,
        "a refused password does not use up the link"
    );
    assert_eq!(
        get("/v1/session".to_owned(), &invited).await.status(),
        401,
        "older sessions end"
    );
    assert_eq!(
        public(
            "/v1/auth/password-reset",
            json!({"token": reset, "password": "another-new-password"})
        )
        .await
        .status(),
        410
    );
    assert_eq!(
        public(
            "/v1/auth/login",
            json!({"email": "invited@example.com", "password": "a-brand-new-password"})
        )
        .await
        .status(),
        200
    );

    // Personal access tokens.
    let read = response_json(
        send(
            "POST",
            "/v1/profile/tokens".to_owned(),
            &owner,
            json!({"name": "CI", "scope": "read", "expires_in_days": 30}),
        )
        .await,
    )
    .await;
    let read = format!("Bearer {}", read["secret"].as_str().unwrap());
    assert_eq!(
        get(format!("/v1/workspaces/{workspace_id}/artifacts"), &read)
            .await
            .status(),
        200
    );
    assert!(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/search"),
            &read,
            json!({"query": "anything"})
        )
        .await
        .status()
        .is_success(),
        "read tokens search"
    );
    assert_eq!(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            &read,
            json!({"title": "x", "content": "y"})
        )
        .await
        .status(),
        403,
        "read-only"
    );
    assert_eq!(
        get("/v1/profile/tokens".to_owned(), &read).await.status(),
        403,
        "tokens cannot manage tokens"
    );
    assert_eq!(
        send(
            "POST",
            "/v1/profile/tokens".to_owned(),
            &owner,
            json!({"name": "Forever", "scope": "write", "expires_in_days": 0})
        )
        .await
        .status(),
        400,
        "every token expires"
    );
    let write = response_json(
        send(
            "POST",
            "/v1/profile/tokens".to_owned(),
            &owner,
            json!({"name": "Deploy", "scope": "write", "expires_in_days": 7}),
        )
        .await,
    )
    .await;
    let write_bearer = format!("Bearer {}", write["secret"].as_str().unwrap());
    assert!(send(
        "POST",
        format!("/v1/workspaces/{workspace_id}/artifacts/text"),
        &write_bearer,
        json!({"title": "From CI", "content": "Release notes"})
    )
    .await
    .status()
    .is_success());
    assert_eq!(
        send(
            "POST",
            "/v1/profile/password".to_owned(),
            &write_bearer,
            json!({"current_password": "x", "new_password": "y"})
        )
        .await
        .status(),
        403,
        "tokens never change passwords"
    );
    assert_eq!(
        send(
            "POST",
            format!("/v1/system/users/{invited_id}/password-reset"),
            &write_bearer,
            json!({})
        )
        .await
        .status(),
        403
    );
    // Every handler that takes a `SessionSubject` refuses tokens, whatever their scope.
    let session_only = [
        ("GET", "/v1/profile/tokens".to_owned(), json!({})),
        (
            "POST",
            "/v1/profile/tokens".to_owned(),
            json!({"name": "Nested", "scope": "write", "expires_in_days": 7}),
        ),
        ("DELETE", "/v1/profile/tokens/any".to_owned(), json!({})),
        ("POST", "/v1/auth/logout-all".to_owned(), json!({})),
        ("POST", "/v1/invitations/accept".to_owned(), json!({"token": "unused"})),
        (
            "PUT",
            format!("/v1/system/users/{invited_id}/system-admin"),
            json!({"enabled": true}),
        ),
        (
            "PUT",
            format!("/v1/system/users/{invited_id}/app-admin"),
            json!({"enabled": true}),
        ),
        ("POST", "/v1/system/environment/detach".to_owned(), json!({})),
    ];
    for (method, uri, body) in session_only {
        assert_eq!(
            send(method, uri.clone(), &write_bearer, body).await.status(),
            403,
            "{method} {uri} must need a browser session"
        );
    }
    assert_eq!(
        get("/v1/session".to_owned(), &owner).await.status(),
        200,
        "nothing above took effect"
    );
    // A read-only token may make the POSTs that only read, and nothing else.
    assert_eq!(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/memory-cards/search"),
            &read,
            json!({"query": "anything"})
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/artifacts/query"),
            &read,
            json!({"query": ""})
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        send(
            "POST",
            "/v1/system/console".to_owned(),
            &read,
            json!({"command": "status"})
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        send(
            "POST",
            "/v1/system/console".to_owned(),
            &read,
            json!({"command": "maintenance run --yes"})
        )
        .await
        .status(),
        403,
        "read-only tokens run dry runs only"
    );
    assert_eq!(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/memory-cards"),
            &read,
            json!({"title": "x", "body_markdown": "y", "source": "test", "citations": []})
        )
        .await
        .status(),
        403,
        "other POSTs stay closed to read-only tokens"
    );
    let token_id = write["token"]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        send("DELETE", format!("/v1/profile/tokens/{token_id}"), &owner, json!({}))
            .await
            .status(),
        204
    );
    assert_eq!(
        get(format!("/v1/workspaces/{workspace_id}/artifacts"), &write_bearer)
            .await
            .status(),
        401,
        "revoked"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_new_server_is_set_up_once_through_the_onboarding() {
    let data_dir = temp_data_dir("onboarding");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.setup_wizard = true;
    config.setup_code = Some("ABCD-EFGH-JKLM".to_owned());
    let app = router(config).await.unwrap();
    let public_get = |uri: &str| Request::builder().uri(uri.to_owned()).body(Body::empty()).unwrap();
    let admin_body = |code: &str| json!({"code": code, "email": "root@example.com", "display_name": "Root", "password": "not-a-real-password"});

    let status = response_json(app.clone().oneshot(public_get("/v1/setup")).await.unwrap()).await;
    assert_eq!(
        (status["state"].clone(), status["onboarding"].clone()),
        (json!("new"), json!(true))
    );
    let health = response_json(app.clone().oneshot(public_get("/health")).await.unwrap()).await;
    assert_eq!(health["setup_required"], json!(true));
    let early = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/register",
            json!({"email": "early@example.com", "display_name": "Early", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(early.status(), 403, "nobody registers before setup");

    assert_eq!(
        app.clone()
            .oneshot(public_post("/v1/setup/verify-code", json!({"code": "WRONG-CODE-0000"})))
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        app.clone()
            .oneshot(public_post("/v1/setup/verify-code", json!({"code": "abcd efgh jklm"})))
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        app.clone()
            .oneshot(public_post("/v1/setup/admin", admin_body("WRONG-CODE-0000")))
            .await
            .unwrap()
            .status(),
        403
    );
    let created = app
        .clone()
        .oneshot(public_post("/v1/setup/admin", admin_body("ABCD-EFGH-JKLM")))
        .await
        .unwrap();
    assert_eq!(created.status(), 201);
    let admin = format!(
        "Bearer {}",
        response_json(created).await["access_token"].as_str().unwrap()
    );
    assert_eq!(
        app.clone()
            .oneshot(public_post("/v1/setup/admin", admin_body("ABCD-EFGH-JKLM")))
            .await
            .unwrap()
            .status(),
        409,
        "one first administrator only"
    );
    let session = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/session", &admin))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(session["is_system_admin"], true);

    let status = response_json(app.clone().oneshot(public_get("/v1/setup")).await.unwrap()).await;
    assert_eq!(status["state"], "finishing");
    let still_closed = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/register",
            json!({"email": "early@example.com", "display_name": "Early", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(still_closed.status(), 403, "registration waits for the end of setup");
    assert_eq!(
        app.clone()
            .oneshot(auth_request("GET", "/v1/setup/checks", "Bearer not-a-token"))
            .await
            .unwrap()
            .status(),
        401
    );
    let checks = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/setup/checks", &admin))
            .await
            .unwrap(),
    )
    .await;
    assert!(checks
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check["key"] == "database" && check["status"] == "ok"));
    // The test server is neither on HTTPS nor backing up: both are reported
    // with what to set, not left silent.
    for key in ["public_url", "backups"] {
        assert!(
            checks
                .as_array()
                .unwrap()
                .iter()
                .any(|check| check["key"] == key && check["status"] == "warning" && check["advice"].is_string()),
            "{key} is a warning with advice"
        );
    }

    let done = app
        .clone()
        .oneshot(auth_request("POST", "/v1/setup/complete", &admin))
        .await
        .unwrap();
    assert_eq!(done.status(), 200);
    assert_eq!(response_json(done).await["onboarding"], false);

    // From now on the onboarding is gone for good.
    assert_eq!(
        app.clone()
            .oneshot(auth_request("POST", "/v1/setup/complete", &admin))
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        app.clone()
            .oneshot(auth_request("GET", "/v1/setup/checks", &admin))
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        app.clone()
            .oneshot(public_post("/v1/setup/verify-code", json!({"code": "ABCD-EFGH-JKLM"})))
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        app.clone()
            .oneshot(public_post("/v1/setup/admin", admin_body("ABCD-EFGH-JKLM")))
            .await
            .unwrap()
            .status(),
        409
    );
    let status = response_json(app.clone().oneshot(public_get("/v1/setup")).await.unwrap()).await;
    assert_eq!(
        (status["state"].clone(), status["onboarding"].clone()),
        (json!("complete"), json!(false))
    );
    let registered = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/register",
            json!({"email": "late@example.com", "display_name": "Late", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(registered.status(), 201);
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
        .map(|event| event["action"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert!(
        actions.contains(&"server_setup_admin_created".to_owned())
            && actions.contains(&"server_setup_completed".to_owned())
    );

    // A restart does not bring the onboarding back.
    drop(app);
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.setup_wizard = true;
    let app = router(config).await.unwrap();
    let status = response_json(app.clone().oneshot(public_get("/v1/setup")).await.unwrap()).await;
    assert_eq!(status["onboarding"], false);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn anonymous_endpoints_take_small_bodies_whatever_the_upload_limit_is() {
    let data_dir = temp_data_dir("small-bodies");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let padding = "x".repeat(200 * 1024);
    for uri in [
        "/v1/auth/login",
        "/v1/auth/register",
        "/v1/auth/refresh",
        "/v1/auth/forgot-password",
        "/v1/auth/password-reset",
        "/v1/invitations/preview",
        "/v1/setup/verify-code",
    ] {
        let response = app
            .clone()
            .oneshot(public_post(
                uri,
                json!({ "email": "a@example.com", "password": padding }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), 413, "{uri} accepted a body of 200 KiB from nobody");
    }
    // A webhook takes up to a mebibyte, and not a body above it.
    let big = Request::builder()
        .method("POST")
        .uri("/v1/webhooks/repositories/anything")
        .header("x-gitlab-token", "guess")
        .body(Body::from(vec![b'x'; 2 * 1024 * 1024]))
        .unwrap();
    assert_eq!(app.clone().oneshot(big).await.unwrap().status(), 413);
    // Ordinary calls are not affected.
    let (_, tokens) = register_account(&app, "small-bodies@example.com").await;
    assert!(tokens["access_token"].is_string());
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Decision 23 (E5): a read-and-write token does content work, but granting access, deleting,
/// repointing, linking, exporting and importing need a session or the `admin` scope.
#[tokio::test]
async fn the_routes_that_grant_access_or_remove_things_need_a_session_or_an_admin_token() {
    let data_dir = temp_data_dir("managing-tokens");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (owner, _) = register_account(&app, "managing-owner@example.com").await;
    let organization = response_json(
        send(
            "POST",
            "/v1/organizations".to_owned(),
            owner.clone(),
            json!({"name": "Managing"}),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let workspace = response_json(
        send(
            "POST",
            "/v1/workspaces".to_owned(),
            owner.clone(),
            json!({"organization_id": organization, "name": "Managed"}),
        )
        .await,
    )
    .await["workspace"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let mint = |scope: &'static str| {
        let send = &send;
        let owner = owner.clone();
        async move {
            let created = response_json(
                send(
                    "POST",
                    "/v1/profile/tokens".to_owned(),
                    owner,
                    json!({"name": scope, "scope": scope, "expires_in_days": 7}),
                )
                .await,
            )
            .await;
            format!("Bearer {}", created["secret"].as_str().unwrap())
        }
    };
    let read = mint("read").await;
    let write = mint("write").await;
    let system = mint("system").await;
    let admin = mint("admin").await;

    let routes: Vec<(&'static str, String, Value)> = vec![
        ("DELETE", "/v1/invitations/any".to_owned(), json!({})),
        (
            "POST",
            format!("/v1/organizations/{organization}/invitations"),
            json!({}),
        ),
        ("POST", format!("/v1/workspaces/{workspace}/invitations"), json!({})),
        (
            "PUT",
            format!("/v1/organizations/{organization}"),
            json!({"name": "Renamed"}),
        ),
        ("PUT", format!("/v1/organizations/{organization}/members"), json!({})),
        (
            "DELETE",
            format!("/v1/organizations/{organization}/members/someone"),
            json!({}),
        ),
        ("PUT", format!("/v1/workspaces/{workspace}/members"), json!({})),
        (
            "DELETE",
            format!("/v1/workspaces/{workspace}/members/someone"),
            json!({}),
        ),
        ("PUT", format!("/v1/workspaces/{workspace}"), json!({"name": "Renamed"})),
        ("DELETE", format!("/v1/workspaces/{workspace}"), json!({})),
        (
            "PUT",
            format!("/v1/workspaces/{workspace}/icon"),
            json!({"icon": "rocket"}),
        ),
        (
            "PUT",
            format!("/v1/workspaces/{workspace}/template"),
            json!({"template_id": "general"}),
        ),
        ("PUT", format!("/v1/workspaces/{workspace}/ai-providers"), json!({})),
        ("PUT", format!("/v1/workspaces/{workspace}/ai-policy"), json!({})),
        ("POST", format!("/v1/workspaces/{workspace}/repositories"), json!({})),
        (
            "POST",
            format!("/v1/workspaces/{workspace}/repositories/check"),
            json!({}),
        ),
        ("PUT", "/v1/repositories/any".to_owned(), json!({})),
        ("DELETE", "/v1/repositories/any".to_owned(), json!({})),
        ("PUT", "/v1/repositories/any/token".to_owned(), json!({})),
        ("DELETE", "/v1/repositories/any/token".to_owned(), json!({})),
        ("PUT", "/v1/repositories/any/deploy-key".to_owned(), json!({})),
        ("POST", "/v1/repositories/any/webhook".to_owned(), json!({})),
        ("DELETE", "/v1/repositories/any/webhook".to_owned(), json!({})),
        ("POST", format!("/v1/workspaces/{workspace}/connectors"), json!({})),
        (
            "POST",
            format!("/v1/workspaces/{workspace}/connectors/check"),
            json!({}),
        ),
        ("DELETE", "/v1/connectors/any".to_owned(), json!({})),
        ("PUT", "/v1/connectors/any/token".to_owned(), json!({})),
        ("POST", "/v1/connectors/any/webhook".to_owned(), json!({})),
        ("DELETE", "/v1/connectors/any/webhook".to_owned(), json!({})),
        ("GET", format!("/v1/workspaces/{workspace}/export"), json!(null)),
        ("POST", "/v1/workspaces/import".to_owned(), json!({})),
        ("GET", "/v1/workspaces/import/jobs/any".to_owned(), json!(null)),
    ];
    // The refusals come before anything is read or changed, so none of the requests below does anything.
    for (method, uri, body) in &routes {
        for (label, token) in [("read", &read), ("write", &write), ("system", &system)] {
            let response = send(method, uri.clone(), token.clone(), body.clone()).await;
            assert_eq!(
                response.status(),
                403,
                "a {label} token must be refused on {method} {uri}"
            );
            let message = response_json(response).await["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            assert!(
                message.contains("admin scope"),
                "the guard answered {method} {uri}: {message}"
            );
        }
    }
    assert_eq!(
        send("GET", "/v1/workspaces".to_owned(), owner.clone(), json!(null))
            .await
            .status(),
        200,
        "nothing above took effect"
    );

    // A session, and a token granted the admin scope, are past the guard (what they ask for may still be refused).
    for (method, uri, body) in &routes {
        if *method == "DELETE" && uri == &format!("/v1/workspaces/{workspace}") {
            continue;
        }
        let session = send(method, uri.clone(), owner.clone(), body.clone()).await;
        let session_status = session.status();
        let by_token = send(method, uri.clone(), admin.clone(), body.clone()).await;
        assert_eq!(
            by_token.status(),
            session_status,
            "an admin token gets the session's answer on {method} {uri}"
        );
        assert_ne!(by_token.status(), 401, "the admin token is known on {method} {uri}");
        let message = response_json(by_token).await["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        assert!(
            !message.contains("admin scope"),
            "the guard let the admin token through on {method} {uri}: {message}"
        );
    }

    // Content work stays open to a read-and-write token, and a read-only token still only reads.
    assert!(send(
        "POST",
        format!("/v1/workspaces/{workspace}/artifacts/text"),
        write.clone(),
        json!({"title": "From a script", "content": "Release notes"})
    )
    .await
    .status()
    .is_success());
    assert_eq!(
        send(
            "GET",
            format!("/v1/workspaces/{workspace}/artifacts"),
            read.clone(),
            json!(null)
        )
        .await
        .status(),
        200
    );
    assert!(
        send("DELETE", format!("/v1/workspaces/{workspace}"), admin, json!({}))
            .await
            .status()
            .is_success(),
        "an admin token deletes a workspace, as its owner may"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

/// E15: revoking a personal access token ends the live streams it opened.
#[tokio::test]
async fn event_streams_end_when_the_token_that_opened_them_is_revoked() {
    let data_dir = temp_data_dir("token-streams");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "token-stream-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let created = response_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/profile/tokens",
                &owner,
                json!({"name": "Watcher", "scope": "read", "expires_in_days": 7}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let bearer = format!("Bearer {}", created["secret"].as_str().unwrap());
    let token_id = created["token"]["id"].as_str().unwrap().to_owned();

    let stream = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/events"),
            &bearer,
        ))
        .await
        .unwrap();
    assert_eq!(stream.status(), 200);
    let revoked = app
        .clone()
        .oneshot(auth_request(
            "DELETE",
            &format!("/v1/profile/tokens/{token_id}"),
            &owner,
        ))
        .await
        .unwrap();
    assert_eq!(revoked.status(), 204);
    let ended = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        to_bytes(stream.into_body(), usize::MAX),
    )
    .await;
    assert!(ended.is_ok(), "the stream should close once its token is revoked");
    for path in [
        format!("/v1/workspaces/{workspace_id}/artifacts"),
        format!("/v1/workspaces/{workspace_id}/events"),
    ] {
        assert_eq!(
            app.clone()
                .oneshot(auth_request("GET", &path, &bearer))
                .await
                .unwrap()
                .status(),
            401,
            "a revoked token cannot read or reconnect: {path}"
        );
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn ending_sessions_closes_the_stream_and_rejects_reads_and_reconnects() {
    let data_dir = temp_data_dir("session-streams");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "session-stream-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let stream = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/events"),
            &owner,
        ))
        .await
        .unwrap();
    assert_eq!(stream.status(), 200);
    assert_eq!(
        app.clone()
            .oneshot(auth_request("POST", "/v1/auth/logout-all", &owner))
            .await
            .unwrap()
            .status(),
        204
    );
    let ended = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        to_bytes(stream.into_body(), usize::MAX),
    )
    .await;
    assert!(ended.is_ok(), "the stream should close when its session ends");
    for path in [
        format!("/v1/workspaces/{workspace_id}/artifacts"),
        format!("/v1/workspaces/{workspace_id}/events"),
    ] {
        assert_eq!(
            app.clone()
                .oneshot(auth_request("GET", &path, &owner))
                .await
                .unwrap()
                .status(),
            401,
            "an ended session cannot read or reconnect: {path}"
        );
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

/// S5-4: over HTTPS the refresh cookie carries the `__Secure-` prefix, a cookie an earlier release set is
/// read once and replaced, and signing out clears both.
#[tokio::test]
async fn over_https_the_refresh_cookie_has_the_secure_prefix_and_the_old_one_is_replaced() {
    let data_dir = temp_data_dir("secure-cookie");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.public_url = Some("https://repomemo.example.test".to_owned());
    let app = router(config).await.unwrap();
    let web = |uri: &str, cookie: Option<&str>, body: Value| {
        let mut request = Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .header("x-repomemo-client", "web");
        if let Some(cookie) = cookie {
            request = request.header("cookie", cookie);
        }
        request.body(Body::from(body.to_string())).unwrap()
    };
    let cookies = |response: &axum::response::Response| {
        response
            .headers()
            .get_all("set-cookie")
            .iter()
            .map(|value| value.to_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let registered = app
        .clone()
        .oneshot(web(
            "/v1/auth/register",
            None,
            json!({"email": "secure-cookie@example.com", "display_name": "Secure", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(registered.status(), 201);
    let set = cookies(&registered);
    assert!(
        set[0].starts_with("__Secure-repomemo_refresh=") && set[0].contains("; Secure"),
        "{set:?}"
    );
    assert!(
        set.iter()
            .any(|cookie| cookie.starts_with("repomemo_refresh=;") && cookie.contains("Max-Age=0")),
        "the plain name an earlier release set is cleared: {set:?}"
    );
    let first = set[0]
        .split(';')
        .next()
        .unwrap()
        .trim_start_matches("__Secure-repomemo_refresh=")
        .to_owned();

    // The new name renews the session and rotates.
    let refreshed = app
        .clone()
        .oneshot(web(
            "/v1/auth/refresh",
            Some(&format!("__Secure-repomemo_refresh={first}")),
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(refreshed.status(), 200);
    let second = cookies(&refreshed)[0]
        .split(';')
        .next()
        .unwrap()
        .trim_start_matches("__Secure-repomemo_refresh=")
        .to_owned();
    assert!(!second.is_empty() && second != first);

    // A browser that still holds the cookie an earlier release set is read once and gets the new name.
    let legacy = app
        .clone()
        .oneshot(web(
            "/v1/auth/refresh",
            Some(&format!("repomemo_refresh={second}")),
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(legacy.status(), 200, "the old name is still read");
    let set = cookies(&legacy);
    assert!(set[0].starts_with("__Secure-repomemo_refresh="), "{set:?}");
    assert!(
        set.iter().any(|cookie| cookie.starts_with("repomemo_refresh=;")),
        "{set:?}"
    );
    let third = set[0]
        .split(';')
        .next()
        .unwrap()
        .trim_start_matches("__Secure-repomemo_refresh=")
        .to_owned();

    // Signing out clears both names.
    let out = app
        .clone()
        .oneshot(web(
            "/v1/auth/logout",
            Some(&format!("__Secure-repomemo_refresh={third}")),
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(out.status(), 204);
    let set = cookies(&out);
    assert!(
        set.iter()
            .any(|cookie| cookie.starts_with("__Secure-repomemo_refresh=;") && cookie.contains("Max-Age=0")),
        "{set:?}"
    );
    assert!(
        set.iter()
            .any(|cookie| cookie.starts_with("repomemo_refresh=;") && cookie.contains("Max-Age=0")),
        "{set:?}"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn enforced_provider_sign_in_leaves_only_break_glass_password_access() {
    let dir = temp_data_dir("identity-enforced");
    let app = router(ServerConfig::for_test(dir.clone())).await.unwrap();
    let (_, owner) = register_account(&app, "owner@example.com").await;
    register_account(&app, "member@example.com").await;
    let owner_id = owner["user"]["id"].as_str().unwrap().to_owned();
    let storage = repomemo_storage::StorageEngine::open(repomemo_storage::StorageConfig::new(dir))
        .await
        .unwrap();
    let mut provider = repomemo_storage::IdentityProvider {
        name: "Company".into(),
        enabled: true,
        enforced: true,
        break_glass: vec![owner_id.clone()],
        ..Default::default()
    };
    storage
        .save_identity_provider(&provider, "test-secret", &owner_id)
        .await
        .unwrap();
    let login = |email: &str| {
        public_post(
            "/v1/auth/login",
            json!({"email":email,"password":"not-a-real-password"}),
        )
    };
    let status = |request: axum::http::Request<axum::body::Body>| {
        let app = app.clone();
        async move { app.oneshot(request).await.unwrap().status() }
    };
    assert_eq!(status(login("member@example.com")).await, 403);
    assert_eq!(status(login("owner@example.com")).await, 200);
    assert_eq!(
        status(public_post(
            "/v1/auth/register",
            json!({"email":"new@example.com","display_name":"New","password":"not-a-real-password"})
        ))
        .await,
        403
    );
    let public = response_json(
        app.clone()
            .oneshot(
                axum::http::Request::get("/v1/auth/identity")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(public["enforced"], true);
    // With no usable break-glass account left, a system administrator still has a path; members do not.
    provider.break_glass.clear();
    storage
        .save_identity_provider(&provider, "test-secret", &owner_id)
        .await
        .unwrap();
    assert_eq!(status(login("owner@example.com")).await, 200);
    assert_eq!(status(login("member@example.com")).await, 403);
    provider.enforced = false;
    storage
        .save_identity_provider(&provider, "test-secret", &owner_id)
        .await
        .unwrap();
    assert_eq!(status(login("member@example.com")).await, 200);
}

#[tokio::test]
async fn provider_sessions_refresh_only_until_their_absolute_deadline() {
    let dir = temp_data_dir("identity-deadline");
    let app = router(ServerConfig::for_test(dir.clone())).await.unwrap();
    let (_, account) = register_account(&app, "deadline@example.com").await;
    let storage = repomemo_storage::StorageEngine::open(repomemo_storage::StorageConfig::new(dir))
        .await
        .unwrap();
    let refresh = |token: String| public_post("/v1/auth/refresh", json!({"refresh_token":token}));
    let future = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    let token = account["refresh_token"].as_str().unwrap().to_owned();
    storage
        .attach_identity_session(&token, "https://id.example.com", Some("sid-1"), &future)
        .await
        .unwrap();
    let response = app.clone().oneshot(refresh(token)).await.unwrap();
    assert_eq!(response.status(), 200);
    let renewed = response_json(response).await["refresh_token"]
        .as_str()
        .unwrap()
        .to_owned();
    let carried = storage.identity_session(&renewed).await.unwrap().unwrap();
    assert_eq!(
        (carried.0.as_str(), carried.1.as_deref(), carried.2.as_str()),
        ("https://id.example.com", Some("sid-1"), future.as_str())
    );
    let past = (chrono::Utc::now() - chrono::Duration::seconds(5)).to_rfc3339();
    storage
        .attach_identity_session(&renewed, "https://id.example.com", Some("sid-1"), &past)
        .await
        .unwrap();
    assert_eq!(
        app.clone().oneshot(refresh(renewed.clone())).await.unwrap().status(),
        401
    );
    assert_eq!(app.oneshot(refresh(renewed)).await.unwrap().status(), 401);
}

#[tokio::test]
async fn restricted_workspaces_never_enroll_organization_members_and_policy_changes_move_nobody() {
    let app = router(ServerConfig::for_test(temp_data_dir("workspace-policy")))
        .await
        .unwrap();
    let (owner, _) = register_account(&app, "owner@example.com").await;
    let (member, _) = register_account(&app, "member@example.com").await;
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let organization = response_json(
        call(
            "POST",
            "/v1/organizations".into(),
            owner.clone(),
            json!({"name":"Team"}),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let make = |name: &'static str, policy: &'static str| {
        let (owner, organization) = (owner.clone(), organization.clone());
        async move {
            let response = call(
                "POST",
                "/v1/workspaces".into(),
                owner,
                json!({"organization_id":organization,"name":name,"access_policy":policy}),
            )
            .await;
            assert_eq!(response.status(), 201);
            response_json(response).await["workspace"]["id"]
                .as_str()
                .unwrap()
                .to_owned()
        }
    };
    let open = make("Open", "shared").await;
    let closed = make("Closed", "restricted").await;
    join(
        &app,
        &owner,
        "organizations",
        &organization,
        "member@example.com",
        "member",
    )
    .await;
    let visible = |who: String| async move {
        let list = response_json(call("GET", "/v1/workspaces".into(), who, Value::Null).await).await;
        list.as_array()
            .unwrap()
            .iter()
            .map(|w| w["workspace"]["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(visible(member.clone()).await, vec![open.clone()]);
    assert_eq!(
        call(
            "PUT",
            format!("/v1/workspaces/{closed}/access-policy"),
            member.clone(),
            json!({"policy":"shared"})
        )
        .await
        .status(),
        403
    );
    // Opening a restricted workspace adds nobody; restricting a shared one removes nobody.
    assert_eq!(
        call(
            "PUT",
            format!("/v1/workspaces/{closed}/access-policy"),
            owner.clone(),
            json!({"policy":"shared"})
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        call(
            "PUT",
            format!("/v1/workspaces/{open}/access-policy"),
            owner.clone(),
            json!({"policy":"restricted"})
        )
        .await
        .status(),
        200
    );
    assert_eq!(visible(member.clone()).await, vec![open.clone()]);
    let policy = response_json(
        call(
            "GET",
            format!("/v1/workspaces/{open}/access-policy"),
            owner.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(policy["policy"], "restricted");
    assert_eq!(
        call(
            "PUT",
            format!("/v1/workspaces/{open}/access-policy"),
            owner,
            json!({"policy":"anyone"})
        )
        .await
        .status(),
        400
    );
}

#[tokio::test]
async fn a_restricted_workspace_is_invisible_to_people_who_were_not_added_and_access_has_origins() {
    let app = router(ServerConfig::for_test(temp_data_dir("workspace-hidden")))
        .await
        .unwrap();
    let (owner, _) = register_account(&app, "owner@example.com").await;
    register_account(&app, "outsider@example.com").await;
    register_account(&app, "orgadmin@example.com").await;
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let organization = response_json(
        call(
            "POST",
            "/v1/organizations".into(),
            owner.clone(),
            json!({"name":"Team"}),
        )
        .await,
    )
    .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let closed = response_json(
        call(
            "POST",
            "/v1/workspaces".into(),
            owner.clone(),
            json!({"organization_id":organization,"name":"Secret","access_policy":"restricted"}),
        )
        .await,
    )
    .await["workspace"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    join(
        &app,
        &owner,
        "organizations",
        &organization,
        "outsider@example.com",
        "member",
    )
    .await;
    join(
        &app,
        &owner,
        "organizations",
        &organization,
        "orgadmin@example.com",
        "admin",
    )
    .await;
    let login = |email: &'static str| {
        let app = app.clone();
        async move {
            let session = response_json(
                app.oneshot(public_post(
                    "/v1/auth/login",
                    json!({"email":email,"password":"not-a-real-password"}),
                ))
                .await
                .unwrap(),
            )
            .await;
            format!("Bearer {}", session["access_token"].as_str().unwrap())
        }
    };
    for who in [login("outsider@example.com").await, login("orgadmin@example.com").await] {
        let list = response_json(call("GET", "/v1/workspaces".into(), who.clone(), Value::Null).await).await;
        assert!(list.as_array().unwrap().is_empty(), "the list does not mention it");
        let search =
            response_json(call("POST", "/v1/search".into(), who.clone(), json!({"query":"Secret"})).await).await;
        assert!(search["workspaces"].as_array().unwrap().is_empty());
        assert_eq!(search["not_searched"], 0);
        // A direct address answers the same for the hidden workspace and for one that does not exist.
        let hidden = call(
            "GET",
            format!("/v1/workspaces/{closed}/members"),
            who.clone(),
            Value::Null,
        )
        .await;
        let missing = call(
            "GET",
            "/v1/workspaces/does-not-exist/members".into(),
            who.clone(),
            Value::Null,
        )
        .await;
        assert_eq!(hidden.status(), 403);
        assert_eq!(hidden.status(), missing.status());
        assert_eq!(response_json(hidden).await, response_json(missing).await);
        assert_eq!(
            call(
                "GET",
                format!("/v1/workspaces/{closed}/effective-access"),
                who,
                Value::Null
            )
            .await
            .status(),
            403
        );
    }
    let access = response_json(
        call(
            "GET",
            format!("/v1/workspaces/{closed}/effective-access"),
            owner.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(access["policy"], "restricted");
    let members = access["members"].as_array().unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(
        (members[0]["role"].as_str(), members[0]["origin"].as_str()),
        (Some("owner"), Some("direct"))
    );
    let preview = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{closed}/access-policy/preview"),
            owner,
            json!({"policy":"shared"}),
        )
        .await,
    )
    .await;
    assert_eq!(
        (preview["added"].as_i64(), preview["removed"].as_i64()),
        (Some(0), Some(0))
    );
    assert_eq!(preview["organization_members_not_added"], 2);
}
