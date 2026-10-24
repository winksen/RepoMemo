use super::*;

async fn ask(app: &axum::Router, artifact_id: &str, who: &str, user_id: &str, note: &str) -> axum::response::Response {
    call(
        app,
        "POST",
        &format!("/v1/artifacts/{artifact_id}/review-requests"),
        who,
        json!({"user_id": user_id, "note": note}),
    )
    .await
}

async fn call(app: &axum::Router, method: &str, uri: &str, who: &str, body: Value) -> axum::response::Response {
    app.clone().oneshot(json_request(method, uri, who, body)).await.unwrap()
}

/// CODEOWNERS names an address that is a member, a handle, a team and an address nobody has. The member is
/// matched, can be asked to review as a task assigned to them, and nobody else can be asked this way.
#[tokio::test]
async fn code_owners_written_as_an_address_are_matched_to_members_who_can_be_asked_to_review() {
    let base = std::env::temp_dir().join(format!("repomemo-server-owners-{}", uuid::Uuid::new_v4()));
    let repo_dir = base.join("repos").join("payments");
    let data_dir = base.join("data");
    std::fs::create_dir_all(repo_dir.join("src")).unwrap();
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo_dir)
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    };
    git(&["init", "--quiet", "--initial-branch=main"]);
    std::fs::write(
        repo_dir.join("README.md"),
        "# Payments\n\nRefunds are issued by the ledger.\n",
    )
    .unwrap();
    std::fs::write(repo_dir.join("src/ledger.rs"), "pub fn post_refund() {}\n").unwrap();
    std::fs::write(
        repo_dir.join("CODEOWNERS"),
        "*  @payments-team\nsrc/  owner-member@example.com @owner-member @org/ledger nobody@example.com\n",
    )
    .unwrap();
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "Initial"]);

    let mut config = ServerConfig::for_test(data_dir.clone());
    config.repo_roots = vec![base.join("repos")];
    let app = router(config).await.unwrap();
    let (_, _) = register_account(&app, "owners-admin@example.com").await;
    let (owner, _) = register_account(&app, "owners-owner@example.com").await;
    let (member, member_tokens) = register_account(&app, "owner-member@example.com").await;
    let member_id = member_tokens["user"]["id"].as_str().unwrap().to_owned();
    let (viewer, viewer_tokens) = register_account(&app, "owners-viewer@example.com").await;
    let viewer_id = viewer_tokens["user"]["id"].as_str().unwrap().to_owned();
    let (outsider, _) = register_account(&app, "owners-outsider@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "owner-member@example.com",
        "member",
    )
    .await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "owners-viewer@example.com",
        "viewer",
    )
    .await;

    let connected = call(
        &app,
        "POST",
        &format!("/v1/workspaces/{workspace_id}/repositories"),
        &owner,
        json!({"link": repo_dir.display().to_string()}),
    )
    .await;
    assert_eq!(connected.status(), 201);
    let repository_id = response_json(connected).await["repository"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut repository = Value::Null;
    for _ in 0..100 {
        repository = response_json(
            app.clone()
                .oneshot(auth_request(
                    "GET",
                    &format!("/v1/repositories/{repository_id}"),
                    &owner,
                ))
                .await
                .unwrap(),
        )
        .await;
        if repository["status"] == "ready" && repository["active_job"].is_null() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(repository["status"], "ready", "{repository}");
    let artifacts = response_json(
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
    let id_of = |path: &str| {
        artifacts
            .as_array()
            .unwrap()
            .iter()
            .find(|artifact| artifact["path"] == path)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let (ledger, readme) = (id_of("src/ledger.rs"), id_of("README.md"));
    let owners_of = |id: String, who: String| {
        let request = auth_request("GET", &format!("/v1/artifacts/{id}/code-owners"), &who);
        let app = app.clone();
        async move {
            let response = app.oneshot(request).await.unwrap();
            let status = response.status().as_u16();
            (status, response_json(response).await)
        }
    };

    // The address of a member is matched to them, whatever the case; handles, teams and strangers stay hints.
    let (status, found) = owners_of(ledger.clone(), viewer.clone()).await;
    assert_eq!(status, 200);
    let entries: Vec<(&str, Option<&str>)> = found
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| (entry["owner"].as_str().unwrap(), entry["member"]["id"].as_str()))
        .collect();
    assert_eq!(
        entries,
        [
            ("owner-member@example.com", Some(member_id.as_str())),
            ("@owner-member", None),
            ("@org/ledger", None),
            ("nobody@example.com", None),
        ],
        "{found}"
    );
    // A file whose owners are all handles has no member to match.
    let (_, readme_owners) = owners_of(readme.clone(), viewer.clone()).await;
    assert!(
        readme_owners
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["member"].is_null()),
        "{readme_owners}"
    );
    // Reading is for people who can read the workspace.
    assert_eq!(owners_of(ledger.clone(), outsider.clone()).await.0, 403);

    // Asking for a review: a task for the matched member, about the file.
    assert_eq!(
        ask(&app, &ledger, &viewer, &member_id, "").await.status(),
        403,
        "a viewer cannot ask"
    );
    assert_eq!(ask(&app, &ledger, &outsider, &member_id, "").await.status(), 403);
    assert_eq!(
        ask(&app, &ledger, &owner, &viewer_id, "").await.status(),
        400,
        "a member CODEOWNERS does not name"
    );
    assert_eq!(ask(&app, &ledger, &owner, "not-a-user", "").await.status(), 400);
    assert_eq!(
        ask(&app, &ledger, &owner, &member_id, &"x".repeat(1_001))
            .await
            .status(),
        400
    );
    let asked = ask(&app, &ledger, &owner, &member_id, "").await;
    assert_eq!(asked.status(), 201);
    let task = response_json(asked).await;
    assert_eq!(task["title"], "Review src/ledger.rs");
    assert_eq!(task["assignee"]["id"], member_id.as_str());
    assert_eq!(task["artifact_id"], ledger.as_str());
    assert_eq!(task["status"], "open");
    assert!(task["description"]
        .as_str()
        .unwrap()
        .contains("owner-member@example.com"));
    // It reaches them like any assignment, and is asked once while it is open.
    let notices = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/notifications", &member))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(notices[0]["notification_type"], "task_assigned");
    assert_eq!(ask(&app, &ledger, &owner, &member_id, "").await.status(), 409);
    let done = call(
        &app,
        "PUT",
        &format!("/v1/tasks/{}", task["id"].as_str().unwrap()),
        &member,
        json!({"title": task["title"], "description": task["description"], "status": "done", "priority": "medium", "assignee_user_id": member_id, "artifact_id": ledger, "due_at": null}),
    )
    .await;
    assert_eq!(done.status(), 200);
    let again = ask(
        &app,
        &ledger,
        &owner,
        &member_id,
        "Please look at the refund path again.",
    )
    .await;
    assert_eq!(again.status(), 201, "a finished request can be made again");
    assert_eq!(
        response_json(again).await["description"],
        "Please look at the refund path again."
    );
    let _ = std::fs::remove_dir_all(base);
}
