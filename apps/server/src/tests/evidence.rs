use super::*;

#[tokio::test]
async fn indexing_publishes_jobs_that_can_be_listed_and_cancelled() {
    let data_dir = std::env::temp_dir().join(format!("repomemo-server-jobs-{}", uuid::Uuid::new_v4()));
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();

    let register = Request::builder()
        .method("POST")
        .uri("/v1/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"jobs@example.com","display_name":"Jobs Owner","password":"not-a-real-password"}"#,
        ))
        .unwrap();
    let registration: Value = serde_json::from_slice(
        &to_bytes(app.clone().oneshot(register).await.unwrap().into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let authorization = format!("Bearer {}", registration["access_token"].as_str().unwrap());

    let organization = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/organizations",
            &authorization,
            json!({"name":"Jobs Org"}),
        ))
        .await
        .unwrap();
    let organization: Value =
        serde_json::from_slice(&to_bytes(organization.into_body(), usize::MAX).await.unwrap()).unwrap();
    let workspace = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/workspaces",
            &authorization,
            json!({"organization_id": organization["id"], "name":"Jobs Workspace"}),
        ))
        .await
        .unwrap();
    let workspace: Value = serde_json::from_slice(&to_bytes(workspace.into_body(), usize::MAX).await.unwrap()).unwrap();
    let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();

    let empty_jobs = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/jobs"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(empty_jobs.status(), 200);
    let empty_jobs: Value =
        serde_json::from_slice(&to_bytes(empty_jobs.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert!(empty_jobs.as_array().unwrap().is_empty());

    let artifact = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            &authorization,
            json!({"title":"Job source","content":"Line one\nLine two\nLine three","language":"Markdown"}),
        ))
        .await
        .unwrap();
    assert_eq!(artifact.status(), 201);
    let artifact: Value = serde_json::from_slice(&to_bytes(artifact.into_body(), usize::MAX).await.unwrap()).unwrap();
    let artifact_id = artifact["id"].as_str().unwrap().to_owned();

    let indexed = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/artifacts/{artifact_id}/index"),
            &authorization,
            json!({}),
        ))
        .await
        .unwrap();
    // A manual re-index is accepted, not done inside the request: the job
    // comes back running and finishes in the background.
    assert_eq!(indexed.status(), 202);
    let indexed_job: Value = serde_json::from_slice(&to_bytes(indexed.into_body(), usize::MAX).await.unwrap()).unwrap();
    let job_id = indexed_job["id"].as_str().unwrap().to_owned();
    assert_eq!(indexed_job["kind"], "indexing");
    assert_eq!(indexed_job["status"], "running");
    assert_eq!(indexed_job["cancel_requested"], false);
    let mut finished = indexed_job.clone();
    for _ in 0..100 {
        let polled = app
            .clone()
            .oneshot(auth_request("GET", &format!("/v1/jobs/{job_id}"), &authorization))
            .await
            .unwrap();
        assert_eq!(polled.status(), 200);
        finished = serde_json::from_slice(&to_bytes(polled.into_body(), usize::MAX).await.unwrap()).unwrap();
        if finished["status"] != "running" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(finished["status"], "completed", "the job ends on its own: {finished}");

    // The whole workspace is re-indexed the same way, with the job as the answer.
    let workspace_index = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/index"),
            &authorization,
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(workspace_index.status(), 202);
    let workspace_job: Value =
        serde_json::from_slice(&to_bytes(workspace_index.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(workspace_job["kind"], "indexing");
    assert_eq!(workspace_job["stage"], "chunking_workspace");
    let workspace_job_id = workspace_job["id"].as_str().unwrap().to_owned();
    for _ in 0..100 {
        let polled = app
            .clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/jobs/{workspace_job_id}"),
                &authorization,
            ))
            .await
            .unwrap();
        let polled: Value = serde_json::from_slice(&to_bytes(polled.into_body(), usize::MAX).await.unwrap()).unwrap();
        if polled["status"] != "running" {
            assert_eq!(polled["status"], "completed", "{polled}");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let listed = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/jobs?kind=indexing"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(listed.status(), 200);
    let listed: Value = serde_json::from_slice(&to_bytes(listed.into_body(), usize::MAX).await.unwrap()).unwrap();
    let jobs = listed.as_array().unwrap();
    assert!(!jobs.is_empty(), "indexing should have produced a job");
    assert!(jobs.iter().any(|job| job["id"] == job_id));
    for job in jobs {
        assert_eq!(job["kind"], "indexing");
    }

    let single = app
        .clone()
        .oneshot(auth_request("GET", &format!("/v1/jobs/{job_id}"), &authorization))
        .await
        .unwrap();
    assert_eq!(single.status(), 200);
    let single: Value = serde_json::from_slice(&to_bytes(single.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(single["id"], job_id);

    // Cancelling a job that already finished is a no-op that returns the
    // current state; only running/pending jobs flip the cancel flag.
    let cancelled = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/jobs/{job_id}/cancel"),
            &authorization,
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(cancelled.status(), 200);
    let cancelled: Value = serde_json::from_slice(&to_bytes(cancelled.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(cancelled["status"], "completed");
    assert_eq!(cancelled["cancel_requested"], false);

    let missing = app
        .clone()
        .oneshot(auth_request("GET", "/v1/jobs/does-not-exist", &authorization))
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Polls the artifact list until `expected` artifacts report an index
/// timestamp, since indexing runs on a background queue.
#[tokio::test]
async fn workspace_repository_links_are_checked_then_synced_in_the_background() {
    let base = std::env::temp_dir().join(format!("repomemo-server-repo-{}", uuid::Uuid::new_v4()));
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
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "Initial"]);

    let mut config = ServerConfig::for_test(data_dir.clone());
    // Links are off until the operator names the folders that hold repositories.
    config.repo_roots = vec![base.join("repos")];
    let app = router(config).await.unwrap();
    let read = |response: axum::response::Response| async move {
        serde_json::from_slice::<Value>(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
            .unwrap_or(Value::Null)
    };
    let mut tokens = Vec::new();
    for email in ["repo-owner@example.com", "repo-member@example.com"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": email, "display_name": email, "password": "not-a-real-password"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        tokens.push(format!(
            "Bearer {}",
            read(response).await["access_token"].as_str().unwrap()
        ));
    }
    let (owner, member) = (&tokens[0], &tokens[1]);
    let organization = read(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/organizations",
                owner,
                json!({"name":"Repo Team"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let workspace = read(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/workspaces",
                owner,
                json!({"organization_id": organization["id"], "name":"Repo Workspace"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();
    join(
        &app,
        owner,
        "workspaces",
        &workspace_id,
        "repo-member@example.com",
        "member",
    )
    .await;
    let added = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/workspaces/{workspace_id}/members"),
            owner,
            json!({"email":"repo-member@example.com","role":"member"}),
        ))
        .await
        .unwrap();
    assert!(added.status().is_success());
    let repositories_uri = format!("/v1/workspaces/{workspace_id}/repositories");

    let listing = read(
        app.clone()
            .oneshot(auth_request("GET", &repositories_uri, member))
            .await
            .unwrap(),
    )
    .await;
    assert!(listing["repositories"].as_array().unwrap().is_empty());

    // The link is checked before anything is stored.
    let check_uri = format!("{repositories_uri}/check");
    let check = |token: &str, link: &str| json_request("POST", &check_uri, token, json!({"link": link}));
    assert_eq!(
        app.clone()
            .oneshot(check(member, &repo_dir.display().to_string()))
            .await
            .unwrap()
            .status(),
        403
    );
    for (link, expected) in [
        (
            "https://github.com/example/payments.git".to_owned(),
            "REPOMEMO_GIT_ALLOWED_HOSTS",
        ),
        (
            base.join("repos").join("missing").display().to_string(),
            "does not exist",
        ),
        (base.join("repos").display().to_string(), "not inside a git repository"),
        // Outside the configured folders the answer is the same whether or not the path exists.
        (
            base.join("missing").display().to_string(),
            "only links repositories inside",
        ),
        (data_dir.display().to_string(), "only links repositories inside"),
    ] {
        let response = app.clone().oneshot(check(owner, &link)).await.unwrap();
        assert_eq!(response.status(), 400, "{link}");
        let message = read(response).await["error"]["message"].as_str().unwrap().to_owned();
        assert!(message.contains(expected), "{message}");
    }
    let checked = read(
        app.clone()
            .oneshot(check(owner, &format!("\"{}\"", repo_dir.join("src").display())))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(checked["name"], "payments");
    assert_eq!(checked["commit"]["summary"], "Initial");
    assert_eq!(checked["commit"]["branch"], "main");
    assert_eq!(checked["tracked_files"], 2);
    assert_eq!(checked["indexable_files"], 2);
    assert_eq!(checked["already_connected"], false);

    let connect = |token: &str, path: &std::path::Path| {
        json_request(
            "POST",
            &repositories_uri,
            token,
            json!({"link": path.display().to_string()}),
        )
    };
    assert_eq!(
        app.clone().oneshot(connect(member, &repo_dir)).await.unwrap().status(),
        403
    );

    let connected = app
        .clone()
        .oneshot(connect(owner, &repo_dir.join("src")))
        .await
        .unwrap();
    assert_eq!(connected.status(), 201);
    let connected = read(connected).await;
    assert_eq!(connected["repository"]["name"], "payments");
    assert_eq!(connected["job"]["kind"], "repo_sync");
    let repository_id = connected["repository"]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        app.clone().oneshot(connect(owner, &repo_dir)).await.unwrap().status(),
        409
    );
    let checked = read(
        app.clone()
            .oneshot(check(owner, &repo_dir.display().to_string()))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(checked["already_connected"], true);

    let repository_uri = format!("/v1/repositories/{repository_id}");
    let mut repository = Value::Null;
    for _ in 0..100 {
        repository = read(
            app.clone()
                .oneshot(auth_request("GET", &repository_uri, member))
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
    assert_eq!(repository["file_count"], 2);
    assert_eq!(repository["indexed_file_count"], 2);
    assert_eq!(repository["last_synced_commit"]["summary"], "Initial");
    assert_eq!(repository["last_report"]["added"], 2);

    let files = read(
        app.clone()
            .oneshot(auth_request("GET", &format!("{repository_uri}/files"), member))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(files.as_array().unwrap().len(), 2);
    let artifacts = read(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/artifacts"),
                owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    let ledger = artifacts
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["path"] == "src/ledger.rs")
        .unwrap();
    assert_eq!(ledger["repository_id"], repository_id.as_str());
    // The repository itself is one evidence item, protected like its files.
    let items = artifacts
        .as_array()
        .unwrap()
        .iter()
        .filter(|artifact| artifact["artifact_type"] == "repository")
        .collect::<Vec<_>>();
    assert_eq!(items.len(), 1);
    assert!(items[0]["repository_id"].is_null());
    assert_eq!(items[0]["source_id"], repository_id.as_str());
    let item_id = items[0]["id"].as_str().unwrap();
    assert_eq!(
        app.clone()
            .oneshot(auth_request("DELETE", &format!("/v1/artifacts/{item_id}"), owner))
            .await
            .unwrap()
            .status(),
        400
    );
    let detail = read(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/repositories/{repository_id}/detail"),
                member,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(detail["repository"]["overview_artifact_id"], item_id);
    assert_eq!(detail["overview"]["file_count"], 2);
    assert_eq!(detail["overview"]["key_files"][0]["path"], "README.md");
    assert_eq!(detail["ai_available"], false);
    assert!(detail["summary"].is_null());
    // The knowledge map draws the repository as one node holding all of
    // its files' passages, while progress still counts every file.
    let map = read(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/knowledge-map"),
                member,
            ))
            .await
            .unwrap(),
    )
    .await;
    let file_nodes = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["kind"] == "file")
        .collect::<Vec<_>>();
    assert_eq!(file_nodes.len(), 1, "{map}");
    assert_eq!(file_nodes[0]["id"], item_id);
    assert!(file_nodes[0]["passage_count"].as_i64().unwrap() >= 3);
    assert_eq!(map["pipeline"]["file_count"], 3);
    assert!(map["coverage"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["label"] == "Repository files" && row["file_count"] == 2));
    let summary = app
        .clone()
        .oneshot(auth_request(
            "POST",
            &format!("/v1/repositories/{repository_id}/summary"),
            member,
        ))
        .await
        .unwrap();
    assert_eq!(summary.status(), 400);
    assert!(read(summary).await["error"]["message"]
        .as_str()
        .unwrap()
        .contains("No content was sent"));

    // With a text provider, the summary is generated from the overview and
    // key files, cited, and kept for the repository page.
    let (ollama_url, prompts) = start_fake_ollama().await;
    let saved = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/workspaces/{workspace_id}/ai-providers"),
            owner,
            json!({"provider_type": "ollama", "name": "Mock text", "base_url": ollama_url, "model": "mock-chat", "enabled": true, "purpose": "text"}),
        ))
        .await
        .unwrap();
    assert!(saved.status().is_success());
    let summary = app
        .clone()
        .oneshot(auth_request(
            "POST",
            &format!("/v1/repositories/{repository_id}/summary"),
            member,
        ))
        .await
        .unwrap();
    assert_eq!(summary.status(), 200);
    let summary = read(summary).await;
    assert_eq!(summary["commit_sha"], detail["overview"]["commit"]["sha"]);
    assert!(summary["citations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|citation| citation["path"] == "README.md"));
    assert!(prompts.lock().unwrap().iter().any(|prompt| prompt.contains("payments")));
    let detail = read(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/repositories/{repository_id}/detail"),
                member,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(detail["ai_available"], true);
    assert_eq!(detail["summary"]["summary_markdown"], summary["summary_markdown"]);
    let artifact_id = ledger["id"].as_str().unwrap();
    let refused = app
        .clone()
        .oneshot(auth_request("DELETE", &format!("/v1/artifacts/{artifact_id}"), owner))
        .await
        .unwrap();
    assert_eq!(refused.status(), 400);

    // Members may sync; only administrators change or remove a repository.
    let synced = app
        .clone()
        .oneshot(auth_request("POST", &format!("{repository_uri}/sync"), member))
        .await
        .unwrap();
    assert_eq!(synced.status(), 200);
    assert_eq!(
        app.clone()
            .oneshot(json_request(
                "PUT",
                &repository_uri,
                member,
                json!({"exclude": ["*.md"]})
            ))
            .await
            .unwrap()
            .status(),
        403
    );
    let updated = read(
        app.clone()
            .oneshot(json_request(
                "PUT",
                &repository_uri,
                owner,
                json!({"name": "Payments", "exclude": ["*.md"]}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(updated["name"], "Payments");
    assert_eq!(updated["settings"]["exclude"], json!(["*.md"]));

    for _ in 0..100 {
        repository = read(
            app.clone()
                .oneshot(auth_request("GET", &repository_uri, owner))
                .await
                .unwrap(),
        )
        .await;
        if repository["active_job"].is_null() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(
        app.clone()
            .oneshot(auth_request("DELETE", &repository_uri, member))
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        app.clone()
            .oneshot(auth_request("DELETE", &repository_uri, owner))
            .await
            .unwrap()
            .status(),
        204
    );
    let artifacts = read(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/artifacts"),
                owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(artifacts.as_array().unwrap().is_empty());
    assert_eq!(
        app.clone()
            .oneshot(auth_request("GET", &repository_uri, owner))
            .await
            .unwrap()
            .status(),
        404
    );

    drop(app);
    let _ = std::fs::remove_dir_all(base);
}

#[tokio::test]
async fn indexes_new_evidence_automatically_and_limits_chunks_to_admins() {
    let data_dir = std::env::temp_dir().join(format!("repomemo-server-auto-index-{}", uuid::Uuid::new_v4()));
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();

    let mut tokens = Vec::new();
    for email in ["auto-owner@example.com", "auto-member@example.com"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": email, "display_name": email, "password": "not-a-real-password"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
        let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
        tokens.push(format!("Bearer {}", body["access_token"].as_str().unwrap()));
    }
    let (owner, member) = (&tokens[0], &tokens[1]);

    let organization = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/organizations",
            owner,
            json!({"name":"Auto Team"}),
        ))
        .await
        .unwrap();
    let organization: Value =
        serde_json::from_slice(&to_bytes(organization.into_body(), usize::MAX).await.unwrap()).unwrap();
    let workspace = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/workspaces",
            owner,
            json!({"organization_id": organization["id"], "name":"Auto Workspace"}),
        ))
        .await
        .unwrap();
    let workspace: Value = serde_json::from_slice(&to_bytes(workspace.into_body(), usize::MAX).await.unwrap()).unwrap();
    let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();
    join(
        &app,
        owner,
        "workspaces",
        &workspace_id,
        "auto-member@example.com",
        "member",
    )
    .await;
    let added = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/workspaces/{workspace_id}/members"),
            owner,
            json!({"email":"auto-member@example.com","role":"member"}),
        ))
        .await
        .unwrap();
    assert_eq!(added.status(), 200);

    let artifact = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            member,
            json!({"title":"Auto note","content":"# Auto\nIndexed without a button.","language":"Markdown"}),
        ))
        .await
        .unwrap();
    assert_eq!(artifact.status(), 201);
    let artifact: Value = serde_json::from_slice(&to_bytes(artifact.into_body(), usize::MAX).await.unwrap()).unwrap();
    let artifact_id = artifact["id"].as_str().unwrap().to_owned();

    wait_for_indexed_artifacts(&app, owner, &workspace_id, 1).await;

    let capabilities = |authorization: String| {
        let app = app.clone();
        let workspace_id = workspace_id.clone();
        async move {
            let response = app
                .oneshot(auth_request(
                    "GET",
                    &format!("/v1/workspaces/{workspace_id}/capabilities"),
                    &authorization,
                ))
                .await
                .unwrap();
            serde_json::from_slice::<Value>(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
        }
    };
    assert_eq!(capabilities(owner.clone()).await["can_inspect_index"], true);
    assert_eq!(capabilities(member.clone()).await["can_inspect_index"], false);

    // A member can read the artifact but never receives its chunks.
    let detail = app
        .clone()
        .oneshot(auth_request("GET", &format!("/v1/artifacts/{artifact_id}"), member))
        .await
        .unwrap();
    assert_eq!(detail.status(), 200);
    let detail: Value = serde_json::from_slice(&to_bytes(detail.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert!(detail["chunks"].as_array().unwrap().is_empty());
    let member_chunks = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/artifacts/{artifact_id}/chunks"),
            member,
        ))
        .await
        .unwrap();
    assert_eq!(member_chunks.status(), 403);

    // Owners and admins see the stored chunks through both routes.
    let owner_detail = app
        .clone()
        .oneshot(auth_request("GET", &format!("/v1/artifacts/{artifact_id}"), owner))
        .await
        .unwrap();
    let owner_detail: Value =
        serde_json::from_slice(&to_bytes(owner_detail.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(owner_detail["chunks"].as_array().unwrap().len(), 1);
    let owner_chunks = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/artifacts/{artifact_id}/chunks"),
            owner,
        ))
        .await
        .unwrap();
    assert_eq!(owner_chunks.status(), 200);
    let owner_chunks: Value =
        serde_json::from_slice(&to_bytes(owner_chunks.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(owner_chunks[0]["heading_path"], "Auto");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn workspace_health_finds_drift_and_only_admins_act_on_it() {
    let data_dir = std::env::temp_dir().join(format!("repomemo-server-health-{}", uuid::Uuid::new_v4()));
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let read = |response: axum::response::Response| async move {
        serde_json::from_slice::<Value>(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
    };

    let mut tokens = Vec::new();
    for email in ["health-owner@example.com", "health-member@example.com"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"email": email, "display_name": email, "password": "not-a-real-password"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
        tokens.push(format!(
            "Bearer {}",
            read(response).await["access_token"].as_str().unwrap()
        ));
    }
    let (owner, member) = (&tokens[0], &tokens[1]);
    let organization = read(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/organizations",
                owner,
                json!({"name":"Health Team"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let workspace = read(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/v1/workspaces",
                owner,
                json!({"organization_id": organization["id"], "name":"Health Workspace"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();
    join(
        &app,
        owner,
        "workspaces",
        &workspace_id,
        "health-member@example.com",
        "member",
    )
    .await;
    let added = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/workspaces/{workspace_id}/members"),
            owner,
            json!({"email":"health-member@example.com","role":"member"}),
        ))
        .await
        .unwrap();
    assert_eq!(added.status(), 200);

    // Two versions of one code file: the second drops `parse_config`.
    for (expected, body) in [
        (1, "fn parse_config() {}\nfn load_settings() {}\n"),
        (2, "fn load_settings() {}\n"),
    ] {
        let uploaded = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                    .header("authorization", owner)
                    .header("content-type", "text/x-rust")
                    .header("x-repomemo-filename", "parser.rs")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(uploaded.status(), 201);
        wait_for_indexed_artifacts(&app, owner, &workspace_id, expected).await;
    }
    for (title, content) in [
        (
            "Startup runbook",
            "# Startup\nCall `parse_config` before the server starts.",
        ),
        ("Copy A", "Identical body."),
        ("Copy B", "Identical body."),
    ] {
        let created = app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
                owner,
                json!({"title": title, "content": content, "language": "Markdown"}),
            ))
            .await
            .unwrap();
        assert_eq!(created.status(), 201);
    }
    wait_for_indexed_artifacts(&app, owner, &workspace_id, 5).await;

    let health_uri = format!("/v1/workspaces/{workspace_id}/health");
    let actions_uri = format!("/v1/workspaces/{workspace_id}/health/actions");
    let health = app
        .clone()
        .oneshot(auth_request("GET", &health_uri, member))
        .await
        .unwrap();
    assert_eq!(health.status(), 200);
    let health = read(health).await;
    let finding = |health: &Value, detector: &str| {
        health["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|finding| finding["detector"] == detector)
            .cloned()
    };
    let older = finding(&health, "older_version_active").expect("older version finding");
    assert_eq!(older["files"].as_array().unwrap().len(), 2);
    let removed = finding(&health, "removed_symbol_mentioned").expect("removed symbol finding");
    assert!(removed["detail"].as_str().unwrap().contains("`parse_config`"));
    assert!(removed["evidence"][0]["excerpt"]
        .as_str()
        .unwrap()
        .contains("parse_config"));
    let duplicate = finding(&health, "duplicate_content").expect("duplicate finding");
    assert!(finding(&health, "unconnected").is_none());
    assert_eq!(health["similarity_available"], false);

    // Members can read findings but not act on them.
    let forbidden = app
        .clone()
        .oneshot(json_request(
            "POST",
            &actions_uri,
            member,
            json!({"fingerprint": older["fingerprint"], "action": "supersede"}),
        ))
        .await
        .unwrap();
    assert_eq!(forbidden.status(), 403);

    let superseded = app
        .clone()
        .oneshot(json_request(
            "POST",
            &actions_uri,
            owner,
            json!({"fingerprint": older["fingerprint"], "action": "supersede"}),
        ))
        .await
        .unwrap();
    assert_eq!(superseded.status(), 200);
    let superseded = read(superseded).await;
    let old_version_id = superseded["updated_artifact_ids"][0].as_str().unwrap().to_owned();
    assert_ne!(old_version_id, older["keep_artifact_id"].as_str().unwrap());
    let lifecycle = read(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/artifacts/{old_version_id}/lifecycle"),
                owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(lifecycle["status"], "superseded");
    assert_eq!(lifecycle["superseded_by_artifact_id"], older["keep_artifact_id"]);

    let not_allowed = app
        .clone()
        .oneshot(json_request(
            "POST",
            &actions_uri,
            owner,
            json!({"fingerprint": duplicate["fingerprint"], "action": "create_task"}),
        ))
        .await
        .unwrap();
    assert_eq!(not_allowed.status(), 400);
    let dismissed = app
        .clone()
        .oneshot(json_request(
            "POST",
            &actions_uri,
            owner,
            json!({"fingerprint": duplicate["fingerprint"], "action": "dismiss"}),
        ))
        .await
        .unwrap();
    assert_eq!(dismissed.status(), 200);
    let tasked = app
        .clone()
        .oneshot(json_request(
            "POST",
            &actions_uri,
            owner,
            json!({"fingerprint": removed["fingerprint"], "action": "create_task"}),
        ))
        .await
        .unwrap();
    assert_eq!(tasked.status(), 200);
    assert!(read(tasked).await["task_id"].is_string());

    // Handled findings stay hidden, and a stale fingerprint is refused.
    let health = read(
        app.clone()
            .oneshot(auth_request("GET", &health_uri, owner))
            .await
            .unwrap(),
    )
    .await;
    assert!(health["findings"].as_array().unwrap().is_empty());
    let stats = |detector: &str| {
        health["detectors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|stats| stats["detector"] == detector)
            .cloned()
            .unwrap()
    };
    assert_eq!(stats("older_version_active")["acted_count"], 1);
    assert_eq!(stats("duplicate_content")["dismissed_count"], 1);
    assert_eq!(stats("removed_symbol_mentioned")["acted_count"], 1);
    let stale = app
        .clone()
        .oneshot(json_request(
            "POST",
            &actions_uri,
            owner,
            json!({"fingerprint": older["fingerprint"], "action": "dismiss"}),
        ))
        .await
        .unwrap();
    assert_eq!(stale.status(), 409);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn business_documents_preview_download_and_open_link() {
    let data_dir = std::env::temp_dir().join(format!("repomemo-server-docs-{}", uuid::Uuid::new_v4()));
    let app = router(ServerConfig::for_test(data_dir)).await.unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "docs@example.com", "display_name": "Docs", "password": "not-a-real-password"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let owner = format!("Bearer {}", body["access_token"].as_str().unwrap());
    let organization = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/organizations",
            &owner,
            json!({"name":"Docs Team"}),
        ))
        .await
        .unwrap();
    let organization: Value =
        serde_json::from_slice(&to_bytes(organization.into_body(), usize::MAX).await.unwrap()).unwrap();
    let workspace = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/workspaces",
            &owner,
            json!({"organization_id": organization["id"], "name":"Docs Workspace"}),
        ))
        .await
        .unwrap();
    let workspace: Value = serde_json::from_slice(&to_bytes(workspace.into_body(), usize::MAX).await.unwrap()).unwrap();
    let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();

    let eml = "From: Ada <ada@example.com>\r\nTo: bob@example.com\r\nSubject: Plan\r\nContent-Type: text/plain\r\n\r\nPlease review.\r\n";
    let uploaded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                .header("authorization", &owner)
                .header("x-repomemo-filename", "plan.eml")
                .header("content-type", "message/rfc822")
                .body(Body::from(eml))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(uploaded.status(), 201);
    let uploaded: Value = serde_json::from_slice(&to_bytes(uploaded.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(uploaded["language"], "Email");
    let artifact_id = uploaded["id"].as_str().unwrap().to_owned();

    let preview = app
        .clone()
        .oneshot(json_request(
            "GET",
            &format!("/v1/artifacts/{artifact_id}/document-preview"),
            &owner,
            json!(null),
        ))
        .await
        .unwrap();
    assert_eq!(preview.status(), 200);
    let preview: Value = serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(preview["kind"], "email");
    assert_eq!(preview["subject"], "Plan");
    assert_eq!(preview["body"], "Please review.");

    let file = app
        .clone()
        .oneshot(json_request(
            "GET",
            &format!("/v1/artifacts/{artifact_id}/file"),
            &owner,
            json!(null),
        ))
        .await
        .unwrap();
    assert_eq!(file.status(), 200);
    assert!(file.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .starts_with("attachment"));
    assert_eq!(to_bytes(file.into_body(), usize::MAX).await.unwrap(), eml.as_bytes());

    let link = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/artifacts/{artifact_id}/open-link"),
            &owner,
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(link.status(), 200);
    let link: Value = serde_json::from_slice(&to_bytes(link.into_body(), usize::MAX).await.unwrap()).unwrap();
    let token = link["token"].as_str().unwrap();
    let public = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/shared-files/{token}/plan.eml"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(public.status(), 200);
    let forged = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/shared-files/{token}x/plan.eml"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forged.status(), 401);
    // A file link must not work as a session token.
    let misuse = app
        .clone()
        .oneshot(json_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/artifacts"),
            &format!("Bearer {token}"),
            json!(null),
        ))
        .await
        .unwrap();
    assert_eq!(misuse.status(), 401);
}

/// A stand-in for LibreOffice that "converts" by copying the input to
/// `<outdir>/input.pdf`, so the conversion pipeline can be tested anywhere.
#[cfg(windows)]
fn fake_soffice(directory: &std::path::Path) -> std::path::PathBuf {
    std::fs::create_dir_all(directory).unwrap();
    let path = directory.join("fake-soffice.bat");
    let script = [
        "@echo off",
        ":loop",
        "if \"%~1\"==\"\" goto done",
        "if \"%~1\"==\"--outdir\" set OUT=%~2",
        "set LAST=%~1",
        "shift",
        "goto loop",
        ":done",
        "copy /Y \"%LAST%\" \"%OUT%\\input.pdf\" >nul",
    ]
    .join("\r\n");
    std::fs::write(&path, script).unwrap();
    path
}

#[cfg(unix)]
fn fake_soffice(directory: &std::path::Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(directory).unwrap();
    let path = directory.join("fake-soffice.sh");
    let script = [
        "#!/bin/sh",
        "while [ $# -gt 0 ]; do",
        "  if [ \"$1\" = \"--outdir\" ]; then out=\"$2\"; fi",
        "  last=\"$1\"; shift",
        "done",
        "cp \"$last\" \"$out/input.pdf\"",
    ]
    .join("\n");
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[tokio::test]
async fn office_files_are_rendered_to_a_cached_pdf() {
    let data_dir = std::env::temp_dir().join(format!("repomemo-server-render-{}", uuid::Uuid::new_v4()));
    let mut config = ServerConfig::for_test(data_dir.clone());
    // Next to the environment, not inside it: an environment folder must
    // hold nothing but RepoMemo's own files.
    config.soffice = Some(fake_soffice(&data_dir.with_extension("bin")));
    let app = router(config).await.unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "render@example.com", "display_name": "Render", "password": "not-a-real-password"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let owner = format!("Bearer {}", body["access_token"].as_str().unwrap());
    let organization = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/organizations",
            &owner,
            json!({"name":"Render Team"}),
        ))
        .await
        .unwrap();
    let organization: Value =
        serde_json::from_slice(&to_bytes(organization.into_body(), usize::MAX).await.unwrap()).unwrap();
    let workspace = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/workspaces",
            &owner,
            json!({"organization_id": organization["id"], "name":"Render Workspace"}),
        ))
        .await
        .unwrap();
    let workspace: Value = serde_json::from_slice(&to_bytes(workspace.into_body(), usize::MAX).await.unwrap()).unwrap();
    let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();

    let upload = |name: &'static str, bytes: &'static str| {
        let app = app.clone();
        let owner = owner.clone();
        let workspace_id = workspace_id.clone();
        async move {
            let response = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                        .header("authorization", &owner)
                        .header("x-repomemo-filename", name)
                        .body(Body::from(bytes))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), 201);
            let value: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
            value["id"].as_str().unwrap().to_owned()
        }
    };
    let docx = upload("report.docx", "pretend this is a Word file").await;
    let eml = upload("mail.eml", "Subject: Hi\r\n\r\nBody").await;

    let status = |id: String| {
        let app = app.clone();
        let owner = owner.clone();
        async move {
            let response = app
                .oneshot(json_request(
                    "GET",
                    &format!("/v1/artifacts/{id}/rendered-preview/status"),
                    &owner,
                    json!(null),
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            let value: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
            value["state"].as_str().unwrap().to_owned()
        }
    };

    assert_eq!(status(eml).await, "unsupported");
    let mut state = status(docx.clone()).await;
    for _ in 0..100 {
        if state == "ready" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        state = status(docx.clone()).await;
    }
    assert_eq!(state, "ready");

    let pdf = app
        .clone()
        .oneshot(json_request(
            "GET",
            &format!("/v1/artifacts/{docx}/rendered-preview"),
            &owner,
            json!(null),
        ))
        .await
        .unwrap();
    assert_eq!(pdf.status(), 200);
    assert_eq!(
        to_bytes(pdf.into_body(), usize::MAX).await.unwrap(),
        "pretend this is a Word file".as_bytes()
    );
}

#[tokio::test]
async fn remote_links_are_off_until_hosts_are_named_and_a_token_is_never_echoed() {
    let data_dir = temp_data_dir("remote-links");
    let call = |app: &axum::Router, method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };

    // Off by default: the answer names the setting that turns it on.
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "remote-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let off = call(
        &app,
        "POST",
        format!("/v1/workspaces/{workspace_id}/repositories/check"),
        owner.clone(),
        json!({"link": "https://github.com/acme/app.git", "token": "ghp_neverechoed"}),
    )
    .await;
    assert_eq!(off.status(), 400);
    let body = response_json(off).await.to_string();
    assert!(body.contains("REPOMEMO_GIT_ALLOWED_HOSTS"), "{body}");
    assert!(!body.contains("ghp_neverechoed"));
    drop(app);

    // With hosts named, anything else is refused before the network is touched.
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.git_allowed_hosts = vec!["github.com".to_owned(), "127.0.0.1".to_owned()];
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "remote-owner2@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    for (link, expected) in [
        ("https://gitlab.com/acme/app.git", "github.com"),
        ("http://github.com/acme/app.git", "https and ssh"),
        ("https://user:secret@github.com/acme/app.git", "credentials"),
        ("ssh://git@github.com/acme/app.git", "needs a deploy key"),
    ] {
        let refused = call(
            &app,
            "POST",
            format!("/v1/workspaces/{workspace_id}/repositories/check"),
            owner.clone(),
            json!({"link": link}),
        )
        .await;
        assert_eq!(refused.status(), 400, "{link}");
        let message = response_json(refused).await.to_string();
        assert!(message.contains(expected), "{link}: {message}");
    }

    // An allowed host that does not answer: the link is not stored and the
    // token appears nowhere in the answer.
    let unreachable = call(
        &app,
        "POST",
        format!("/v1/workspaces/{workspace_id}/repositories"),
        owner.clone(),
        json!({"link": "https://127.0.0.1:9/acme/app.git", "token": "ghp_neverechoed", "token_username": "oauth2"}),
    )
    .await;
    assert_eq!(unreachable.status(), 400);
    let message = response_json(unreachable).await.to_string();
    assert!(!message.contains("ghp_neverechoed"), "{message}");
    let listed = response_json(
        call(
            &app,
            "GET",
            format!("/v1/workspaces/{workspace_id}/repositories"),
            owner.clone(),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(listed["repositories"].as_array().unwrap().len(), 0);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn push_webhooks_queue_a_sync_only_when_they_are_signed_right() {
    let base = std::env::temp_dir().join(format!("repomemo-server-hooks-{}", uuid::Uuid::new_v4()));
    let repo_dir = base.join("repos").join("ledger");
    let data_dir = base.join("data");
    std::fs::create_dir_all(&repo_dir).unwrap();
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
    std::fs::write(repo_dir.join("README.md"), "# Ledger\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "Initial"]);

    let mut config = ServerConfig::for_test(data_dir.clone());
    config.repo_roots = vec![base.join("repos")];
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "hook-owner@example.com").await;
    let (member, _) = register_account(&app, "hook-member@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    join(&app, &owner, "workspaces", &workspace_id, "hook-member@example.com", "member").await;
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let connected = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/repositories"),
            owner.clone(),
            json!({"link": repo_dir.display().to_string()}),
        )
        .await,
    )
    .await;
    let source_id = connected["repository"]["id"].as_str().unwrap().to_owned();
    let hook_path = format!("/v1/webhooks/repositories/{source_id}");
    let send = |path: String, headers: Vec<(&'static str, String)>, body: &'static str| {
        let app = app.clone();
        async move {
            let mut request = Request::builder().method("POST").uri(path);
            for (name, value) in headers {
                request = request.header(name, value);
            }
            let response = app.oneshot(request.body(Body::from(body)).unwrap()).await.unwrap();
            (response.status().as_u16(), response_json(response).await)
        }
    };
    let signature = |secret: &str, body: &str| {
        let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, secret.as_bytes());
        format!("sha256={}", hex::encode(ring::hmac::sign(&key, body.as_bytes()).as_ref()))
    };
    let push = r#"{"ref":"refs/heads/main"}"#;

    // Without a webhook nothing can be verified: the same answer as for a
    // repository that does not exist.
    let (status, _) = send(hook_path.clone(), vec![("x-gitlab-token", "guess".to_owned())], push).await;
    assert_eq!(status, 401);
    let (status, _) = send(
        "/v1/webhooks/repositories/missing".to_owned(),
        vec![("x-gitlab-token", "guess".to_owned())],
        push,
    )
    .await;
    assert_eq!(status, 401);

    // Only an administrator sets one up, and the secret is shown once.
    assert_eq!(
        call("POST", format!("/v1/repositories/{source_id}/webhook"), member.clone(), json!({}))
            .await
            .status(),
        403
    );
    let created = call("POST", format!("/v1/repositories/{source_id}/webhook"), owner.clone(), json!({})).await;
    assert_eq!(created.status(), 201);
    let created = response_json(created).await;
    let secret = created["secret"].as_str().unwrap().to_owned();
    assert_eq!(created["path"], hook_path.as_str());
    let shown = response_json(call("GET", format!("/v1/repositories/{source_id}"), owner.clone(), json!({})).await).await;
    assert_eq!(shown["has_webhook"], true);
    assert!(!shown.to_string().contains(&secret));

    // Unsigned, wrongly signed, signed for other content: refused.
    for headers in [
        vec![],
        vec![("x-hub-signature-256", signature("wrong", push)), ("x-github-event", "push".to_owned())],
        vec![("x-hub-signature-256", signature(&secret, "{}")), ("x-github-event", "push".to_owned())],
        vec![("x-gitlab-token", "wrong".to_owned()), ("x-gitlab-event", "Push Hook".to_owned())],
    ] {
        let (status, _) = send(hook_path.clone(), headers, push).await;
        assert_eq!(status, 401);
    }

    // GitHub: a ping answers, a push queues a sync (or is absorbed by one).
    let github = |event: &str| {
        vec![
            ("x-hub-signature-256", signature(&secret, push)),
            ("x-github-event", event.to_owned()),
        ]
    };
    let (status, body) = send(hook_path.clone(), github("ping"), push).await;
    assert_eq!((status, body["note"].as_str()), (200, Some("pong")));
    let (status, body) = send(hook_path.clone(), github("push"), push).await;
    assert_eq!(status, 202);
    assert!(body["queued"].is_boolean());
    let (status, body) = send(hook_path.clone(), github("issues"), push).await;
    assert_eq!((status, body["queued"].as_bool()), (202, Some(false)));

    // A delivery id is acted on once: the same call again, signed right, is only noted as received.
    let mut delivered = github("push");
    delivered.push(("x-github-delivery", "delivery-1".to_owned()));
    let (status, first) = send(hook_path.clone(), delivered.clone(), push).await;
    assert_eq!(status, 202);
    assert_ne!(first["note"].as_str(), Some("already received"));
    let (status, again) = send(hook_path.clone(), delivered, push).await;
    assert_eq!(
        (status, again["queued"].as_bool(), again["note"].as_str()),
        (202, Some(false), Some("already received"))
    );
    let mut other = github("push");
    other.push(("x-github-delivery", "delivery-2".to_owned()));
    let (_, next) = send(hook_path.clone(), other, push).await;
    assert_ne!(next["note"].as_str(), Some("already received"), "a new delivery is a new call");

    // GitLab sends the secret itself; a generic sender signs like GitHub.
    let (status, _) = send(
        hook_path.clone(),
        vec![("x-gitlab-token", secret.clone()), ("x-gitlab-event", "Push Hook".to_owned())],
        push,
    )
    .await;
    assert_eq!(status, 202);
    let (status, _) = send(
        hook_path.clone(),
        vec![("x-repomemo-signature", signature(&secret, push))],
        push,
    )
    .await;
    assert_eq!(status, 202);
    let shown = response_json(call("GET", format!("/v1/repositories/{source_id}"), owner.clone(), json!({})).await).await;
    assert!(shown["webhook_last_received_at"].is_string());

    // A new secret retires the old one; removing it ends the webhook.
    let renewed = response_json(
        call("POST", format!("/v1/repositories/{source_id}/webhook"), owner.clone(), json!({})).await,
    )
    .await;
    assert_ne!(renewed["secret"].as_str().unwrap(), secret);
    let (status, _) = send(hook_path.clone(), github("push"), push).await;
    assert_eq!(status, 401);
    assert_eq!(
        call("DELETE", format!("/v1/repositories/{source_id}/webhook"), owner.clone(), json!({}))
            .await
            .status(),
        204
    );
    let (status, _) = send(
        hook_path,
        vec![("x-gitlab-token", renewed["secret"].as_str().unwrap().to_owned())],
        push,
    )
    .await;
    assert_eq!(status, 401);
    let _ = std::fs::remove_dir_all(base);
}

#[tokio::test]
async fn a_citation_opens_the_file_as_it_was_at_the_commit_it_names() {
    let base = std::env::temp_dir().join(format!("repomemo-server-atcommit-{}", uuid::Uuid::new_v4()));
    let repo_dir = base.join("repos").join("notes");
    let data_dir = base.join("data");
    std::fs::create_dir_all(&repo_dir).unwrap();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo_dir)
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.com"])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };
    git(&["init", "--quiet", "--initial-branch=main"]);
    std::fs::write(repo_dir.join("guide.md"), "# Guide\nfirst version\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "--quiet", "-m", "First"]);
    let first = git(&["rev-parse", "HEAD"]);

    let mut config = ServerConfig::for_test(data_dir.clone());
    config.repo_roots = vec![base.join("repos")];
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "at-owner@example.com").await;
    let (outsider, _) = register_account(&app, "at-outsider@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let wait_ready = |source_id: String, commit: String| {
        let app = app.clone();
        let owner = owner.clone();
        async move {
            for _ in 0..150 {
                let repository = response_json(
                    app.clone()
                        .oneshot(auth_request("GET", &format!("/v1/repositories/{source_id}"), &owner))
                        .await
                        .unwrap(),
                )
                .await;
                if repository["status"] == "ready"
                    && repository["active_job"].is_null()
                    && repository["last_synced_commit"]["sha"] == commit.as_str()
                {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            panic!("the repository was not synced to {commit}");
        }
    };
    let connected = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/repositories"),
            owner.clone(),
            json!({"link": repo_dir.display().to_string()}),
        )
        .await,
    )
    .await;
    let source_id = connected["repository"]["id"].as_str().unwrap().to_owned();
    wait_ready(source_id.clone(), first.clone()).await;
    let files = response_json(call("GET", format!("/v1/repositories/{source_id}/files"), owner.clone(), json!({})).await).await;
    let artifact_id = files[0]["artifact_id"].as_str().unwrap().to_owned();

    std::fs::write(repo_dir.join("guide.md"), "# Guide\nsecond version\n").unwrap();
    git(&["commit", "--quiet", "-am", "Second"]);
    let second = git(&["rev-parse", "HEAD"]);
    assert_eq!(
        call("POST", format!("/v1/repositories/{source_id}/sync"), owner.clone(), json!({}))
            .await
            .status(),
        200
    );
    wait_ready(source_id.clone(), second.clone()).await;

    // The old text, labelled with its commit, and the commit the workspace holds now.
    let old = call("GET", format!("/v1/artifacts/{artifact_id}/at/{}", &first[..12]), owner.clone(), json!({})).await;
    assert_eq!(old.status(), 200);
    let old = response_json(old).await;
    assert_eq!(old["commit"], first.as_str());
    assert_eq!(old["summary"], "First");
    assert_eq!(old["current_commit"], second.as_str());
    assert!(old["text"].as_str().unwrap().contains("first version"));
    assert_eq!(old["path"], "guide.md");

    // The current commit names no other version.
    let current = response_json(
        call("GET", format!("/v1/artifacts/{artifact_id}/at/{second}"), owner.clone(), json!({})).await,
    )
    .await;
    assert!(current["current_commit"].is_null());
    assert!(current["text"].as_str().unwrap().contains("second version"));

    // Unknown commits, odd input and people outside the workspace are refused.
    for commit in ["deadbeefdeadbeef", "zzzzzzzz", "--oops"] {
        let refused = call("GET", format!("/v1/artifacts/{artifact_id}/at/{commit}"), owner.clone(), json!({})).await;
        assert!(refused.status() == 404 || refused.status() == 400, "{commit}: {}", refused.status());
    }
    assert_eq!(
        call("GET", format!("/v1/artifacts/{artifact_id}/at/{first}"), outsider, json!({}))
            .await
            .status(),
        403
    );
    let _ = std::fs::remove_dir_all(base);
}
