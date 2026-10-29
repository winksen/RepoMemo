//! The access matrix (V0.5.18): who may reach what, across roles, guests, limited tokens and a restricted workspace.
//!
//! Every actor probes the same routes of one restricted workspace. A route either reads (the list, a search, the event
//! stream, the artifact list), administers (the people, who-can-open-it, an export) or owns (the access policy). The
//! table says, for each actor, which of the three it may do; anything else must be refused, and a refusal must not
//! depend on the route.
use super::*;
use axum::http::StatusCode;

struct Actor {
    name: &'static str,
    bearer: String,
    read: bool,
    administer: bool,
    own: bool,
}

async fn probe(
    app: &axum::Router,
    actor: &Actor,
    workspace: &str,
) -> (Vec<(String, StatusCode)>, Vec<(String, StatusCode)>, StatusCode) {
    let send = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &actor.bearer, body);
        async move { (uri, app.oneshot(request).await.unwrap().status()) }
    };
    let reads = vec![
        send("GET", format!("/v1/workspaces/{workspace}/overview"), Value::Null).await,
        send("GET", format!("/v1/workspaces/{workspace}/artifacts"), Value::Null).await,
        send("GET", format!("/v1/workspaces/{workspace}/tasks"), Value::Null).await,
        send(
            "POST",
            format!("/v1/workspaces/{workspace}/search"),
            json!({"query":"matrix"}),
        )
        .await,
        send("GET", format!("/v1/workspaces/{workspace}/events"), Value::Null).await,
    ];
    let administers = vec![
        send("GET", format!("/v1/workspaces/{workspace}/members"), Value::Null).await,
        send(
            "GET",
            format!("/v1/workspaces/{workspace}/effective-access"),
            Value::Null,
        )
        .await,
        send("GET", format!("/v1/workspaces/{workspace}/export"), Value::Null).await,
    ];
    let owns = send(
        "PUT",
        format!("/v1/workspaces/{workspace}/access-policy"),
        json!({"policy":"restricted"}),
    )
    .await
    .1;
    (reads, administers, owns)
}

async fn session(app: &axum::Router, email: &str) -> String {
    let login = response_json(
        app.clone()
            .oneshot(public_post(
                "/v1/auth/login",
                json!({"email":email,"password":"not-a-real-password"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    format!("Bearer {}", login["access_token"].as_str().unwrap())
}

#[tokio::test]
async fn the_access_matrix_holds_for_every_role_guest_token_and_restricted_workspace() {
    let dir = temp_data_dir("access-matrix");
    let app = router(ServerConfig::for_test(dir.clone())).await.unwrap();
    // The first account administers the server: the one stated exception.
    let (sysadmin, _) = register_account(&app, "root@example.com").await;
    let (owner, owner_account) = register_account(&app, "owner@example.com").await;
    for email in [
        "admin@example.com",
        "member@example.com",
        "viewer@example.com",
        "outsider@example.com",
        "orgadmin@example.com",
        "guest@example.com",
        "expired@example.com",
        "tokenholder@example.com",
    ] {
        register_account(&app, email).await;
    }
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let workspace = create_workspace(&app, &owner).await;
    let other = create_workspace(&app, &owner).await;
    let organization = response_json(call("GET", "/v1/organizations".into(), owner.clone(), Value::Null).await).await
        [0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        call(
            "PUT",
            format!("/v1/workspaces/{workspace}/access-policy"),
            owner.clone(),
            json!({"policy":"restricted"})
        )
        .await
        .status(),
        200
    );
    // Roles are added one by one, as a restricted workspace requires; the organization administrator is not added.
    join(
        &app,
        &owner,
        "organizations",
        &organization,
        "orgadmin@example.com",
        "admin",
    )
    .await;
    for (email, role) in [
        ("admin@example.com", "admin"),
        ("member@example.com", "member"),
        ("viewer@example.com", "viewer"),
        ("guest@example.com", "member"),
        ("expired@example.com", "member"),
        ("tokenholder@example.com", "member"),
    ] {
        join(&app, &owner, "workspaces", &workspace, email, role).await;
    }
    join(&app, &owner, "workspaces", &other, "tokenholder@example.com", "member").await;
    // A guest whose end date has passed: the end must be in the future when set, so it is set two seconds ahead.
    let storage = repomemo_storage::StorageEngine::open(repomemo_storage::StorageConfig::new(dir.clone()))
        .await
        .unwrap();
    let expired_id = response_json(
        call(
            "GET",
            format!("/v1/workspaces/{workspace}/guests"),
            owner.clone(),
            Value::Null,
        )
        .await,
    )
    .await["guests"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["email"] == "expired@example.com")
        .unwrap()["user_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let soon = (chrono::Utc::now() + chrono::Duration::seconds(2)).to_rfc3339();
    storage
        .set_guest_expiry(&workspace, &expired_id, Some(&soon))
        .await
        .unwrap();
    // Tokens of one member: one limited to this workspace, one limited to another.
    let holder = session(&app, "tokenholder@example.com").await;
    let token = |ids: Vec<String>| {
        let holder = holder.clone();
        async move {
            let made = response_json(
                call(
                    "POST",
                    "/v1/profile/tokens".into(),
                    holder,
                    json!({"name":"matrix","scope":"write","expires_in_days":7,"workspace_ids":ids}),
                )
                .await,
            )
            .await;
            format!("Bearer {}", made["secret"].as_str().unwrap())
        }
    };
    let limited_here = token(vec![workspace.clone()]).await;
    let limited_elsewhere = token(vec![other.clone()]).await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let actors = vec![
        Actor {
            name: "system administrator",
            bearer: sysadmin,
            read: true,
            administer: true,
            own: false,
        },
        Actor {
            name: "owner",
            bearer: owner,
            read: true,
            administer: true,
            own: true,
        },
        Actor {
            name: "admin",
            bearer: session(&app, "admin@example.com").await,
            read: true,
            administer: true,
            own: false,
        },
        Actor {
            name: "member",
            bearer: session(&app, "member@example.com").await,
            read: true,
            administer: false,
            own: false,
        },
        Actor {
            name: "viewer",
            bearer: session(&app, "viewer@example.com").await,
            read: true,
            administer: false,
            own: false,
        },
        Actor {
            name: "guest",
            bearer: session(&app, "guest@example.com").await,
            read: true,
            administer: false,
            own: false,
        },
        Actor {
            name: "expired guest",
            bearer: session(&app, "expired@example.com").await,
            read: false,
            administer: false,
            own: false,
        },
        Actor {
            name: "outsider",
            bearer: session(&app, "outsider@example.com").await,
            read: false,
            administer: false,
            own: false,
        },
        Actor {
            name: "organization administrator not added",
            bearer: session(&app, "orgadmin@example.com").await,
            read: false,
            administer: false,
            own: false,
        },
        Actor {
            name: "member session of the token holder",
            bearer: holder.clone(),
            read: true,
            administer: false,
            own: false,
        },
        Actor {
            name: "token limited to this workspace",
            bearer: limited_here,
            read: true,
            administer: false,
            own: false,
        },
        Actor {
            name: "token limited to another workspace",
            bearer: limited_elsewhere,
            read: false,
            administer: false,
            own: false,
        },
    ];
    for actor in &actors {
        let (reads, administers, owns) = probe(&app, actor, &workspace).await;
        for (route, status) in &reads {
            assert_eq!(
                status.is_success(),
                actor.read,
                "{} reads {route}: {status}",
                actor.name
            );
            assert!(
                actor.read || *status == 403,
                "{} must be refused, not told anything else, on {route}: {status}",
                actor.name
            );
        }
        for (route, status) in &administers {
            assert_eq!(
                status.is_success(),
                actor.administer,
                "{} administers {route}: {status}",
                actor.name
            );
        }
        assert_eq!(owns.is_success(), actor.own, "{} owns the policy: {owns}", actor.name);
    }
    // A hidden workspace answers exactly as one that does not exist, for every actor that was refused.
    for actor in actors.iter().filter(|a| !a.read) {
        let hidden = call(
            "GET",
            format!("/v1/workspaces/{workspace}/tasks"),
            actor.bearer.clone(),
            Value::Null,
        )
        .await;
        let missing = call(
            "GET",
            "/v1/workspaces/no-such-workspace/tasks".into(),
            actor.bearer.clone(),
            Value::Null,
        )
        .await;
        assert_eq!(hidden.status(), missing.status(), "{}", actor.name);
        assert_eq!(
            response_json(hidden).await,
            response_json(missing).await,
            "{}",
            actor.name
        );
    }
    // Nobody refused sees it listed or searched either.
    for actor in actors.iter().filter(|a| !a.read) {
        let listed = response_json(call("GET", "/v1/workspaces".into(), actor.bearer.clone(), Value::Null).await).await;
        assert!(
            !listed
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w["workspace"]["id"] == workspace.as_str()),
            "{} lists it",
            actor.name
        );
        let found = response_json(
            call(
                "POST",
                "/v1/search".into(),
                actor.bearer.clone(),
                json!({"query":"matrix"}),
            )
            .await,
        )
        .await;
        assert!(
            !found["workspaces"]
                .as_array()
                .unwrap()
                .iter()
                .any(|w| w["id"] == workspace.as_str()),
            "{} finds it",
            actor.name
        );
    }
    // The owner's grants are intact: the matrix did not change anyone's membership.
    let access = response_json(
        call(
            "GET",
            format!("/v1/workspaces/{workspace}/effective-access"),
            actors[1].bearer.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert!(access["members"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["user_id"] == owner_account["user"]["id"] && m["role"] == "owner"));
}
