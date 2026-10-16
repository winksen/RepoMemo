use super::*;

#[test]
fn page_query_slices_and_reports_total() {
    let page = crate::PageQuery {
        limit: Some(2),
        offset: Some(3),
    };
    let (headers, items) = page.apply((0..10).collect::<Vec<_>>());
    assert_eq!(items, vec![3, 4]);
    assert_eq!(headers.get("x-total-count").unwrap(), "10");
    let (headers, items) = crate::PageQuery::default().apply((0..10).collect::<Vec<_>>());
    assert_eq!(items.len(), 10);
    assert!(headers.get("x-total-count").is_none());
}

#[tokio::test]
async fn protects_and_serves_shared_evidence_flow() {
    let data_dir = std::env::temp_dir().join(format!("repomemo-server-flow-{}", uuid::Uuid::new_v4()));
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"flow@example.com","display_name":"Flow Owner","password":"not-a-real-password"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let registration: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let authorization = format!("Bearer {}", registration["access_token"].as_str().unwrap());

    let profile = app
        .clone()
        .oneshot(auth_request("GET", "/v1/profile", &authorization))
        .await
        .unwrap();
    assert_eq!(profile.status(), 200);
    let profile: Value = serde_json::from_slice(&to_bytes(profile.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(profile["user"]["display_name"], "Flow Owner");
    assert!(profile["last_connected_at"].is_string());
    assert_eq!(profile["activity_by_day"].as_array().unwrap().len(), 365);

    let renamed_profile = app
        .clone()
        .oneshot(json_request(
            "PUT",
            "/v1/profile",
            &authorization,
            json!({"display_name":"Flow Owner Updated"}),
        ))
        .await
        .unwrap();
    assert_eq!(renamed_profile.status(), 200);

    let changed_password = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/profile/password",
            &authorization,
            json!({"current_password":"not-a-real-password","new_password":"changed-password-123"}),
        ))
        .await
        .unwrap();
    assert_eq!(changed_password.status(), 200);
    let changed_password: Value =
        serde_json::from_slice(&to_bytes(changed_password.into_body(), usize::MAX).await.unwrap()).unwrap();
    // The old token is ended with every other session; this one continues
    // with the fresh session returned by the change.
    let stale_profile = app
        .clone()
        .oneshot(auth_request("GET", "/v1/profile", &authorization))
        .await
        .unwrap();
    assert_eq!(stale_profile.status(), 401);
    let authorization = format!("Bearer {}", changed_password["access_token"].as_str().unwrap());

    let organization = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/organizations",
            &authorization,
            json!({"name":"Flow Team"}),
        ))
        .await
        .unwrap();
    assert_eq!(organization.status(), 201);
    let organization: Value =
        serde_json::from_slice(&to_bytes(organization.into_body(), usize::MAX).await.unwrap()).unwrap();
    let workspace = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/workspaces",
            &authorization,
            json!({"organization_id": organization["id"], "name":"Flow Workspace"}),
        ))
        .await
        .unwrap();
    assert_eq!(workspace.status(), 201);
    let workspace: Value = serde_json::from_slice(&to_bytes(workspace.into_body(), usize::MAX).await.unwrap()).unwrap();
    let workspace_id = workspace["workspace"]["id"].as_str().unwrap();

    let collaborator = app.clone().oneshot(Request::builder().method("POST").uri("/v1/auth/register").header("content-type", "application/json")
        .body(Body::from(r#"{"email":"collaborator@example.com","display_name":"Collaborator","password":"not-a-real-password"}"#)).unwrap()).await.unwrap();
    assert_eq!(collaborator.status(), 201);
    let collaborator: Value =
        serde_json::from_slice(&to_bytes(collaborator.into_body(), usize::MAX).await.unwrap()).unwrap();
    let collaborator_authorization = format!("Bearer {}", collaborator["access_token"].as_str().unwrap());
    join(
        &app,
        &authorization,
        "workspaces",
        workspace_id,
        "collaborator@example.com",
        "member",
    )
    .await;
    let member = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/workspaces/{workspace_id}/members"),
            &authorization,
            json!({"email":"collaborator@example.com","role":"member"}),
        ))
        .await
        .unwrap();
    assert_eq!(member.status(), 200);
    let member: Value = serde_json::from_slice(&to_bytes(member.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(member["role"], "member");
    let members = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/members"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(members.status(), 200);
    let members: Value = serde_json::from_slice(&to_bytes(members.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(members.as_array().unwrap().len(), 2);
    for endpoint in [
        format!("/v1/workspaces/{workspace_id}/members"),
        format!("/v1/workspaces/{workspace_id}/activity"),
        format!("/v1/workspaces/{workspace_id}/activity/calendar"),
    ] {
        let response = app
            .clone()
            .oneshot(auth_request("GET", &endpoint, &collaborator_authorization))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            403,
            "{endpoint} should require workspace admin access"
        );
    }
    let collaborator_overview = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/overview"),
            &collaborator_authorization,
        ))
        .await
        .unwrap();
    assert_eq!(collaborator_overview.status(), 200);

    let artifact = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            &authorization,
            json!({"title":"Shared fact", "content":"The API owns shared data.", "language":"Markdown"}),
        ))
        .await
        .unwrap();
    assert_eq!(artifact.status(), 201);
    let artifact: Value = serde_json::from_slice(&to_bytes(artifact.into_body(), usize::MAX).await.unwrap()).unwrap();
    let artifact_id = artifact["id"].as_str().unwrap();

    let lifecycle = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/artifacts/{artifact_id}/lifecycle"),
            &authorization,
            json!({
                "status":"needs_review",
                "owner_user_id":member["user"]["id"],
                "review_note":"Needs a second reviewer before this decision is relied on.",
                "superseded_by_artifact_id":null
            }),
        ))
        .await
        .unwrap();
    assert_eq!(lifecycle.status(), 200);
    let lifecycle: Value = serde_json::from_slice(&to_bytes(lifecycle.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(lifecycle["status"], "needs_review");
    assert_eq!(lifecycle["owner"]["id"], member["user"]["id"]);
    let lifecycle_history = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/artifacts/{artifact_id}/lifecycle/history"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(lifecycle_history.status(), 200);
    let lifecycle_history: Value =
        serde_json::from_slice(&to_bytes(lifecycle_history.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(lifecycle_history.as_array().unwrap().len(), 1);

    let saved_search = app.clone().oneshot(json_request("POST", &format!("/v1/workspaces/{workspace_id}/saved-searches"), &authorization, json!({"name":"Architecture review","query":"shared","artifact_types":["markdown_doc"],"languages":[],"source_ids":[],"result_limit":20}))).await.unwrap();
    assert_eq!(saved_search.status(), 201);
    let saved_search: Value =
        serde_json::from_slice(&to_bytes(saved_search.into_body(), usize::MAX).await.unwrap()).unwrap();
    let saved_search_id = saved_search["id"].as_str().unwrap();
    let saved_searches = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/saved-searches"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(saved_searches.status(), 200);

    let task = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/tasks"),
            &authorization,
            json!({
                "title":"Review shared API decision",
                "description":"Confirm the evidence and capture follow-up work.",
                "status":"open",
                "priority":"high",
                "assignee_user_id":member["user"]["id"],
                "artifact_id":artifact_id,
                "due_at":null
            }),
        ))
        .await
        .unwrap();
    assert_eq!(task.status(), 201);
    let task: Value = serde_json::from_slice(&to_bytes(task.into_body(), usize::MAX).await.unwrap()).unwrap();
    let task_id = task["id"].as_str().unwrap();
    let checklist_item = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/tasks/{task_id}/checklist"),
            &authorization,
            json!({"body":"Confirm the shared evidence"}),
        ))
        .await
        .unwrap();
    assert_eq!(checklist_item.status(), 201);
    let checklist_item: Value =
        serde_json::from_slice(&to_bytes(checklist_item.into_body(), usize::MAX).await.unwrap()).unwrap();
    let checklist_item_id = checklist_item["id"].as_str().unwrap();
    let completed_item = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/task-checklist/{checklist_item_id}"),
            &authorization,
            json!({"completed":true}),
        ))
        .await
        .unwrap();
    assert_eq!(completed_item.status(), 200);
    let delete_search = app
        .clone()
        .oneshot(auth_request(
            "DELETE",
            &format!("/v1/saved-searches/{saved_search_id}"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(delete_search.status(), 204);
    assert_eq!(task["assignee"]["display_name"], "Collaborator");
    let task_notifications = app
        .clone()
        .oneshot(auth_request("GET", "/v1/notifications", &collaborator_authorization))
        .await
        .unwrap();
    assert_eq!(task_notifications.status(), 200);
    let task_notifications: Value =
        serde_json::from_slice(&to_bytes(task_notifications.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(task_notifications[0]["notification_type"], "task_assigned");
    let assigned_tasks = app
        .clone()
        .oneshot(auth_request("GET", "/v1/profile/tasks", &collaborator_authorization))
        .await
        .unwrap();
    assert_eq!(assigned_tasks.status(), 200);
    let assigned_tasks: Value =
        serde_json::from_slice(&to_bytes(assigned_tasks.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(assigned_tasks.as_array().unwrap().len(), 1);
    let updated_task = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/tasks/{task_id}"),
            &collaborator_authorization,
            json!({
                "title":"Review shared API decision",
                "description":"Confirmed and documented.",
                "status":"done",
                "priority":"high",
                "assignee_user_id":member["user"]["id"],
                "artifact_id":artifact_id,
                "due_at":null
            }),
        ))
        .await
        .unwrap();
    assert_eq!(updated_task.status(), 200);
    let updated_task: Value =
        serde_json::from_slice(&to_bytes(updated_task.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(updated_task["status"], "done");
    assert!(updated_task["completed_at"].is_string());
    let tasks = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/tasks"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(tasks.status(), 200);
    let tasks: Value = serde_json::from_slice(&to_bytes(tasks.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 1);

    let comment = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/artifacts/{artifact_id}/comments"),
            &collaborator_authorization,
            json!({"body":"@flow@example.com This confirms the server-authoritative decision."}),
        ))
        .await
        .unwrap();
    assert_eq!(comment.status(), 201);
    let comment: Value = serde_json::from_slice(&to_bytes(comment.into_body(), usize::MAX).await.unwrap()).unwrap();
    let comment_id = comment["id"].as_str().unwrap();
    let mention_notifications = app
        .clone()
        .oneshot(auth_request("GET", "/v1/notifications", &authorization))
        .await
        .unwrap();
    assert_eq!(mention_notifications.status(), 200);
    let mention_notifications: Value =
        serde_json::from_slice(&to_bytes(mention_notifications.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(mention_notifications[0]["notification_type"], "evidence_mention");
    let mention_notification_id = mention_notifications[0]["id"].as_str().unwrap();
    let marked_notification = app
        .clone()
        .oneshot(auth_request(
            "POST",
            &format!("/v1/notifications/{mention_notification_id}/read"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(marked_notification.status(), 200);
    let updated_comment = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/comments/{comment_id}"),
            &collaborator_authorization,
            json!({"body":"Confirmed: the API remains authoritative for shared data."}),
        ))
        .await
        .unwrap();
    assert_eq!(updated_comment.status(), 200);
    let comments = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/artifacts/{artifact_id}/comments"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(comments.status(), 200);
    let comments: Value = serde_json::from_slice(&to_bytes(comments.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(comments.as_array().unwrap().len(), 1);

    let uploaded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                .header("authorization", &authorization)
                .header("content-type", "text/x-rust")
                .header("x-repomemo-filename", "upload.rs")
                .body(Body::from("fn shared_upload() {}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(uploaded.status(), 201);

    let filtered_artifacts = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/artifacts/query"),
            &authorization,
            json!({
                "query":"shared fact",
                "artifact_types":["markdown_doc"],
                "languages":["markdown"],
                "source_ids":[]
            }),
        ))
        .await
        .unwrap();
    assert_eq!(filtered_artifacts.status(), 200);
    let filtered_artifacts: Value =
        serde_json::from_slice(&to_bytes(filtered_artifacts.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(filtered_artifacts.as_array().unwrap().len(), 1);
    assert_eq!(filtered_artifacts[0]["id"], artifact_id);

    // Notes and uploads are indexed in the background right after storing.
    wait_for_indexed_artifacts(&app, &authorization, workspace_id, 2).await;
    let indexed_only = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/artifacts/query"),
            &authorization,
            json!({"query":"", "artifact_types":[], "languages":[], "source_ids":[], "indexed":true}),
        ))
        .await
        .unwrap();
    let indexed_only: Value =
        serde_json::from_slice(&to_bytes(indexed_only.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(indexed_only.as_array().unwrap().len(), 2);
    let metrics = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/metrics"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(metrics.status(), 200);
    let metrics: Value = serde_json::from_slice(&to_bytes(metrics.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(metrics["artifact_count"], 2);
    assert_eq!(metrics["indexed_artifact_count"], 2);
    assert_eq!(metrics["pending_artifact_count"], 0);
    assert_eq!(metrics["member_count"], 2);
    assert_eq!(metrics["completed_task_count"], 1);
    assert_eq!(metrics["comment_count"], 1);
    assert!(metrics["total_artifact_bytes"].as_i64().unwrap() > 0);
    assert!(metrics["indexed_artifact_bytes"].as_i64().unwrap() > 0);
    assert_eq!(metrics["activity_by_day"].as_array().unwrap().len(), 14);
    assert!(!metrics["artifact_bytes_by_type"].as_array().unwrap().is_empty());
    let calendar = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/activity/calendar"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(calendar.status(), 200);
    let calendar: Value = serde_json::from_slice(&to_bytes(calendar.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(calendar["activity_by_day"].as_array().unwrap().len(), 365);
    assert!(calendar["total_activity_count"].as_i64().unwrap() > 0);
    let search = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/search"),
            &authorization,
            json!({"query":"shared data", "artifact_types": [], "languages": [], "source_ids": [], "limit": 20}),
        ))
        .await
        .unwrap();
    assert_eq!(search.status(), 200);
    let search: Value = serde_json::from_slice(&to_bytes(search.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(search.as_array().unwrap().len(), 1);
    assert_eq!(search[0]["artifact_id"], artifact_id);
    let retrieval_facets = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/retrieval-facets"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(retrieval_facets.status(), 200);
    let retrieval_facets: Value =
        serde_json::from_slice(&to_bytes(retrieval_facets.into_body(), usize::MAX).await.unwrap()).unwrap();
    // Both the pasted note and the uploaded Rust file were indexed automatically.
    assert_eq!(retrieval_facets["artifact_types"], json!(["code_file", "markdown_doc"]));
    assert_eq!(retrieval_facets["languages"], json!(["Markdown", "Rust"]));
    let memory = app.clone().oneshot(json_request("POST", &format!("/v1/workspaces/{workspace_id}/memory-cards"), &authorization, json!({"title":"Rule", "body_markdown":"Keep shared data on the API.", "source":"Flow", "confidence": null, "citations": []}))).await.unwrap();
    assert_eq!(memory.status(), 201);
    let memory: Value = serde_json::from_slice(&to_bytes(memory.into_body(), usize::MAX).await.unwrap()).unwrap();
    let memory_id = memory["id"].as_str().unwrap();
    let memories = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/memory-cards"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(memories.status(), 200);
    let memories: Value = serde_json::from_slice(&to_bytes(memories.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(memories.as_array().unwrap().len(), 1);
    let matching_memories = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/memory-cards/search"),
            &authorization,
            json!({"query":"shared data"}),
        ))
        .await
        .unwrap();
    assert_eq!(matching_memories.status(), 200);
    let matching_memories: Value =
        serde_json::from_slice(&to_bytes(matching_memories.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(matching_memories.as_array().unwrap().len(), 1);
    let memory_detail = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/memory-cards/{memory_id}"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(memory_detail.status(), 200);
    let exported = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/memory-cards/{memory_id}/export"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(exported.status(), 200);
    let exported = String::from_utf8(to_bytes(exported.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
    assert!(exported.contains("Keep shared data on the API."));
    let capabilities = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/capabilities"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(capabilities.status(), 200);
    let capabilities: Value =
        serde_json::from_slice(&to_bytes(capabilities.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(capabilities["role"], "owner");
    assert_eq!(capabilities["can_manage_workspace"], true);
    let ai_overview = app
        .clone()
        .oneshot(auth_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/ai-overview"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(ai_overview.status(), 200);
    let ai_overview: Value =
        serde_json::from_slice(&to_bytes(ai_overview.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(ai_overview["provider_configured"], false);
    let ask_without_provider = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/ask"),
            &authorization,
            json!({"question":"What is shared?"}),
        ))
        .await
        .unwrap();
    assert_eq!(ask_without_provider.status(), 400);
    let renamed_artifact = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/artifacts/{artifact_id}"),
            &authorization,
            json!({"title":"Updated shared fact"}),
        ))
        .await
        .unwrap();
    assert_eq!(renamed_artifact.status(), 200);
    let updated_memory = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/memory-cards/{memory_id}"),
            &authorization,
            json!({"title":"Updated rule", "body_markdown":"Keep shared data on the API.", "source":"Flow", "confidence":null}),
        ))
        .await
        .unwrap();
    assert_eq!(updated_memory.status(), 200);
    let overview = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/overview"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(overview.status(), 200);
    let activity = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/activity"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(activity.status(), 200);
    let activity: Value = serde_json::from_slice(&to_bytes(activity.into_body(), usize::MAX).await.unwrap()).unwrap();
    let activity = activity.as_array().unwrap();
    assert!(activity.iter().any(|event| event["action"] == "workspace_created"));
    assert!(activity.iter().any(|event| event["action"] == "evidence_stored"));
    assert!(activity.iter().any(|event| event["action"] == "memory_created"));
    let collaborator_id = collaborator["user"]["id"].as_str().unwrap();
    let removed = app
        .clone()
        .oneshot(auth_request(
            "DELETE",
            &format!("/v1/workspaces/{workspace_id}/members/{collaborator_id}"),
            &authorization,
        ))
        .await
        .unwrap();
    assert_eq!(removed.status(), 204);
    let collaborator_overview = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/overview"),
            &collaborator_authorization,
        ))
        .await
        .unwrap();
    assert_eq!(collaborator_overview.status(), 403);
    let renamed_workspace = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/workspaces/{workspace_id}"),
            &authorization,
            json!({"name":"Renamed Flow Workspace"}),
        ))
        .await
        .unwrap();
    assert_eq!(renamed_workspace.status(), 200);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn organization_members_inherit_workspace_access_and_roles() {
    let data_dir = std::env::temp_dir().join(format!("repomemo-organization-test-{}", uuid::Uuid::new_v4()));
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let owner = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"org-owner@example.com","display_name":"Org Owner","password":"not-a-real-password"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let owner: Value = serde_json::from_slice(&to_bytes(owner.into_body(), usize::MAX).await.unwrap()).unwrap();
    let owner_authorization = format!("Bearer {}", owner["access_token"].as_str().unwrap());
    let member = app.clone().oneshot(Request::builder().method("POST").uri("/v1/auth/register").header("content-type", "application/json").body(Body::from(r#"{"email":"org-member@example.com","display_name":"Org Member","password":"not-a-real-password"}"#)).unwrap()).await.unwrap();
    let member: Value = serde_json::from_slice(&to_bytes(member.into_body(), usize::MAX).await.unwrap()).unwrap();
    let member_authorization = format!("Bearer {}", member["access_token"].as_str().unwrap());

    let organization = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/v1/organizations",
            &owner_authorization,
            json!({"name":"Platform"}),
        ))
        .await
        .unwrap();
    let organization: Value =
        serde_json::from_slice(&to_bytes(organization.into_body(), usize::MAX).await.unwrap()).unwrap();
    let organization_id = organization["id"].as_str().unwrap();
    assert_eq!(organization["role"], "owner");
    for workspace_name in ["Core", "Docs"] {
        let workspace = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/v1/workspaces",
                &owner_authorization,
                json!({"organization_id": organization_id, "name": workspace_name}),
            ))
            .await
            .unwrap();
        assert_eq!(workspace.status(), 201);
    }

    join(
        &app,
        &owner_authorization,
        "organizations",
        organization_id,
        "org-member@example.com",
        "admin",
    )
    .await;
    let added = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/organizations/{organization_id}/members"),
            &owner_authorization,
            json!({"email":"org-member@example.com","role":"admin"}),
        ))
        .await
        .unwrap();
    assert_eq!(added.status(), 200);
    let member_workspaces = app
        .clone()
        .oneshot(auth_request("GET", "/v1/workspaces", &member_authorization))
        .await
        .unwrap();
    assert_eq!(member_workspaces.status(), 200);
    let member_workspaces: Value =
        serde_json::from_slice(&to_bytes(member_workspaces.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(member_workspaces.as_array().unwrap().len(), 2);
    assert!(member_workspaces
        .as_array()
        .unwrap()
        .iter()
        .all(|workspace| workspace["role"] == "admin"));

    let renamed = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/organizations/{organization_id}"),
            &owner_authorization,
            json!({"name":"Platform Engineering"}),
        ))
        .await
        .unwrap();
    assert_eq!(renamed.status(), 200);
    let member_id = member["user"]["id"].as_str().unwrap();
    let removed = app
        .clone()
        .oneshot(auth_request(
            "DELETE",
            &format!("/v1/organizations/{organization_id}/members/{member_id}"),
            &owner_authorization,
        ))
        .await
        .unwrap();
    assert_eq!(removed.status(), 204);
    let member_workspaces = app
        .clone()
        .oneshot(auth_request("GET", "/v1/workspaces", &member_authorization))
        .await
        .unwrap();
    let member_workspaces: Value =
        serde_json::from_slice(&to_bytes(member_workspaces.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert!(member_workspaces.as_array().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn folders_nest_to_a_limit_hold_notes_and_images_need_a_vision_provider() {
    let data_dir = std::env::temp_dir().join(format!("repomemo-server-folders-{}", uuid::Uuid::new_v4()));
    let app = router(ServerConfig::for_test(data_dir)).await.unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"email": "folders@example.com", "display_name": "Folders", "password": "not-a-real-password"})
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
            json!({"name":"Folder Team"}),
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
            json!({"organization_id": organization["id"], "name":"Folder Workspace"}),
        ))
        .await
        .unwrap();
    let workspace: Value = serde_json::from_slice(&to_bytes(workspace.into_body(), usize::MAX).await.unwrap()).unwrap();
    let workspace_id = workspace["workspace"]["id"].as_str().unwrap().to_owned();
    let folders_uri = format!("/v1/workspaces/{workspace_id}/folders");

    // Five levels are allowed, the sixth is refused.
    let mut parent: Option<String> = None;
    let mut deepest = String::new();
    for level in 1..=5 {
        let response = app
            .clone()
            .oneshot(json_request(
                "POST",
                &folders_uri,
                &owner,
                json!({"name": format!("level {level}"), "parent_id": parent}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), 201, "level {level}");
        let folder: Value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
        deepest = folder["id"].as_str().unwrap().to_owned();
        parent = Some(deepest.clone());
    }
    let too_deep = app
        .clone()
        .oneshot(json_request(
            "POST",
            &folders_uri,
            &owner,
            json!({"name": "level 6", "parent_id": deepest}),
        ))
        .await
        .unwrap();
    assert_eq!(too_deep.status(), 400);

    // A note can be filed in a folder and is reported with it.
    let note = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            &owner,
            json!({"title": "Filed note", "content": "# Hello", "language": "Note", "folder_id": deepest}),
        ))
        .await
        .unwrap();
    assert_eq!(note.status(), 201);
    let note: Value = serde_json::from_slice(&to_bytes(note.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(note["folder_id"], json!(deepest));
    assert_eq!(note["artifact_type"], "note");
    assert!(note["path"].as_str().unwrap().ends_with(".note"));

    // Renaming, moving a file out, and deleting a folder with everything in it.
    let note_id = note["id"].as_str().unwrap().to_owned();
    let renamed = app
        .clone()
        .oneshot(json_request(
            "PATCH",
            &format!("{folders_uri}/{deepest}"),
            &owner,
            json!({"name": "Renamed"}),
        ))
        .await
        .unwrap();
    assert_eq!(renamed.status(), 200);
    let moved = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/artifacts/{note_id}/folder"),
            &owner,
            json!({"folder_id": null}),
        ))
        .await
        .unwrap();
    assert_eq!(moved.status(), 200);
    let moved: Value = serde_json::from_slice(&to_bytes(moved.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert!(moved["folder_id"].is_null());
    let back_in = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/artifacts/{note_id}/folder"),
            &owner,
            json!({"folder_id": deepest}),
        ))
        .await
        .unwrap();
    assert_eq!(back_in.status(), 200);
    let folders = app
        .clone()
        .oneshot(json_request("GET", &folders_uri, &owner, json!(null)))
        .await
        .unwrap();
    let folders: Value = serde_json::from_slice(&to_bytes(folders.into_body(), usize::MAX).await.unwrap()).unwrap();
    let top = folders
        .as_array()
        .unwrap()
        .iter()
        .find(|folder| folder["parent_id"].is_null())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let deleted = app
        .clone()
        .oneshot(json_request(
            "DELETE",
            &format!("{folders_uri}/{top}"),
            &owner,
            json!(null),
        ))
        .await
        .unwrap();
    assert_eq!(deleted.status(), 204);
    let remaining = app
        .clone()
        .oneshot(json_request("GET", &folders_uri, &owner, json!(null)))
        .await
        .unwrap();
    let remaining: Value = serde_json::from_slice(&to_bytes(remaining.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert!(remaining.as_array().unwrap().is_empty());
    let note_gone = app
        .clone()
        .oneshot(json_request(
            "GET",
            &format!("/v1/artifacts/{note_id}"),
            &owner,
            json!(null),
        ))
        .await
        .unwrap();
    assert_ne!(note_gone.status(), 200);

    // Images are refused until an image AI provider exists.
    let image = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                .header("authorization", &owner)
                .header("x-repomemo-filename", "diagram.png")
                .header("content-type", "image/png")
                .body(Body::from(vec![1_u8, 2, 3]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(image.status(), 400);
}

#[tokio::test]
async fn uploads_accept_percent_encoded_file_names() {
    let data_dir = temp_data_dir("upload-names");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "names@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let upload = |name: &str| {
        Request::builder()
            .method("POST")
            .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
            .header("authorization", &owner)
            .header("content-type", "text/markdown")
            .header("x-repomemo-filename", name)
            .body(Body::from("# Notes\nContent."))
            .unwrap()
    };
    let encoded = app
        .clone()
        .oneshot(upload("R%C3%A9sum%C3%A9%20%25%20notes.md"))
        .await
        .unwrap();
    assert_eq!(encoded.status(), 201);
    assert_eq!(response_json(encoded).await["title"], "Résumé % notes.md");
    let plain = app.clone().oneshot(upload("plain.md")).await.unwrap();
    assert_eq!(response_json(plain).await["title"], "plain.md");
    let too_long = app
        .clone()
        .oneshot(upload(&format!("{}.md", "a".repeat(300))))
        .await
        .unwrap();
    assert_eq!(too_long.status(), 400);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn workspace_members_stay_in_their_workspace() {
    let data_dir = temp_data_dir("workspace-guests");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
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
    let (_, _) = register_account(&app, "server-admin@example.com").await;
    // Not the first account, so no system administrator's wildcard.
    let (owner, _) = register_account(&app, "team-owner@example.com").await;
    let (guest, _) = register_account(&app, "contractor@example.com").await;
    let core = create_workspace(&app, &owner).await;
    let organization_id = response_json(get("/v1/organizations".to_owned(), &owner).await).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let sibling = response_json(
        send(
            "POST",
            "/v1/workspaces".to_owned(),
            &owner,
            json!({"organization_id": organization_id, "name": "Finance"}),
        )
        .await,
    )
    .await["workspace"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // A viewer of one workspace gets that workspace only.
    join(&app, &owner, "workspaces", &core, "contractor@example.com", "viewer").await;
    let added = send(
        "PUT",
        format!("/v1/workspaces/{core}/members"),
        &owner,
        json!({"email": "contractor@example.com", "role": "viewer"}),
    )
    .await;
    assert_eq!(added.status(), 200);
    let workspaces = response_json(get("/v1/workspaces".to_owned(), &guest).await).await;
    assert_eq!(workspaces.as_array().unwrap().len(), 1, "{workspaces}");
    assert_eq!(workspaces[0]["role"], "viewer");
    assert_eq!(
        get(format!("/v1/workspaces/{sibling}/overview"), &guest).await.status(),
        403,
        "no role in the sibling workspace"
    );
    assert_eq!(
        send(
            "POST",
            format!("/v1/workspaces/{sibling}/artifacts/text"),
            &guest,
            json!({"title": "x", "content": "y"})
        )
        .await
        .status(),
        403
    );

    // The organization is listed as a guest, without its members or administration.
    let organizations = response_json(get("/v1/organizations".to_owned(), &guest).await).await;
    assert_eq!(organizations.as_array().unwrap().len(), 1);
    assert_eq!(organizations[0]["guest"], true);
    assert_eq!(
        get(format!("/v1/organizations/{organization_id}/members"), &guest)
            .await
            .status(),
        403
    );
    let members = response_json(get(format!("/v1/organizations/{organization_id}/members"), &owner).await).await;
    assert!(
        members
            .as_array()
            .unwrap()
            .iter()
            .all(|member| member["user"]["email"] != "contractor@example.com"),
        "{members}"
    );

    // Workspaces created later are not opened to them either.
    let later = response_json(
        send(
            "POST",
            "/v1/workspaces".to_owned(),
            &owner,
            json!({"organization_id": organization_id, "name": "Later"}),
        )
        .await,
    )
    .await["workspace"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        get(format!("/v1/workspaces/{later}/overview"), &guest).await.status(),
        403
    );

    // Organization membership still flows to every workspace, and the guest flag goes.
    join(
        &app,
        &owner,
        "organizations",
        &organization_id,
        "contractor@example.com",
        "member",
    )
    .await;
    assert_eq!(
        response_json(get("/v1/workspaces".to_owned(), &guest).await)
            .await
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        response_json(get("/v1/organizations".to_owned(), &guest).await).await[0]["guest"],
        false
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn owners_authors_and_served_types_hold() {
    let data_dir = temp_data_dir("second-pass");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (admin, _) = register_account(&app, "first-admin@example.com").await;
    let (owner, _) = register_account(&app, "ws-owner@example.com").await;
    let (writer, _) = register_account(&app, "ws-writer@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "ws-writer@example.com",
        "member",
    )
    .await;
    assert_eq!(
        send(
            "PUT",
            format!("/v1/workspaces/{workspace_id}/members"),
            &owner,
            json!({"email": "ws-writer@example.com", "role": "member"})
        )
        .await
        .status(),
        200
    );

    // The owner keeps the owner role, even against their own request.
    let demoted = send(
        "PUT",
        format!("/v1/workspaces/{workspace_id}/members"),
        &owner,
        json!({"email": "ws-owner@example.com", "role": "member"}),
    )
    .await;
    assert_eq!(demoted.status(), 400, "a workspace cannot lose its owner");
    let organization_id = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/organizations", &owner))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        send(
            "PUT",
            format!("/v1/organizations/{organization_id}/members"),
            &owner,
            json!({"email": "ws-owner@example.com", "role": "member"})
        )
        .await
        .status(),
        400,
        "nor an organization"
    );

    // A writer's comment and task stop being theirs to change once they are a viewer.
    let artifact = response_json(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            &writer,
            json!({"title": "Notes", "content": "Plain text"}),
        )
        .await,
    )
    .await;
    let artifact_id = artifact["id"].as_str().unwrap().to_owned();
    let comment = response_json(
        send(
            "POST",
            format!("/v1/artifacts/{artifact_id}/comments"),
            &writer,
            json!({"body": "Mine"}),
        )
        .await,
    )
    .await;
    let comment_id = comment["id"].as_str().unwrap().to_owned();
    let search = response_json(send("POST", format!("/v1/workspaces/{workspace_id}/saved-searches"), &owner, json!({"name": "Owner's", "query": "plain", "artifact_types": [], "languages": [], "source_ids": [], "result_limit": 20})).await).await;
    let search_id = search["id"].as_str().unwrap().to_owned();
    assert_eq!(
        send("DELETE", format!("/v1/saved-searches/{search_id}"), &writer, json!({}))
            .await
            .status(),
        403,
        "someone else's saved search"
    );
    assert_eq!(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/index"),
            &writer,
            json!({})
        )
        .await
        .status(),
        403,
        "a workspace-wide re-index inside the request is for administrators"
    );
    assert_eq!(
        send(
            "PUT",
            format!("/v1/workspaces/{workspace_id}/members"),
            &owner,
            json!({"email": "ws-writer@example.com", "role": "viewer"})
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        send(
            "PUT",
            format!("/v1/comments/{comment_id}"),
            &writer,
            json!({"body": "Changed"})
        )
        .await
        .status(),
        403,
        "viewers change nothing, their old comments included"
    );
    assert_eq!(
        send("DELETE", format!("/v1/comments/{comment_id}"), &writer, json!({}))
            .await
            .status(),
        403
    );
    assert_eq!(
        send("DELETE", format!("/v1/saved-searches/{search_id}"), &owner, json!({}))
            .await
            .status(),
        204
    );

    // Served types come from the file name, whatever the uploader claimed.
    let uploaded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
                .header("authorization", &owner)
                .header("content-type", "text/html")
                .header("x-repomemo-filename", "page.html")
                .body(Body::from("<script>alert(1)</script>"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(uploaded.status(), 201);
    let page_id = response_json(uploaded).await["id"].as_str().unwrap().to_owned();
    let file = app
        .clone()
        .oneshot(auth_request("GET", &format!("/v1/artifacts/{page_id}/file"), &owner))
        .await
        .unwrap();
    assert_eq!(file.headers()["content-type"], "text/plain; charset=utf-8");
    assert!(file.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .starts_with("attachment"));

    // Run-time settings need a signed-in session, not a token.
    let token = response_json(
        send(
            "POST",
            "/v1/profile/tokens".to_owned(),
            &admin,
            json!({"name": "Ops", "scope": "write", "expires_in_days": 7}),
        )
        .await,
    )
    .await;
    let token = format!("Bearer {}", token["secret"].as_str().unwrap());
    assert_eq!(
        send(
            "PUT",
            "/v1/system/settings".to_owned(),
            &token,
            json!({"values": {"allow_registration": false}})
        )
        .await
        .status(),
        403
    );
    assert_eq!(
        send(
            "DELETE",
            "/v1/system/settings/allow_registration".to_owned(),
            &token,
            json!({})
        )
        .await
        .status(),
        403
    );
    assert_eq!(
        send(
            "POST",
            "/v1/system/console".to_owned(),
            &token,
            json!({"command": "settings set allow_registration false --yes"})
        )
        .await
        .status(),
        403
    );
    assert_eq!(
        send(
            "POST",
            "/v1/system/console".to_owned(),
            &token,
            json!({"command": "settings set allow_registration false"})
        )
        .await
        .status(),
        200,
        "dry runs still work"
    );
    assert_eq!(
        send(
            "POST",
            "/v1/system/console".to_owned(),
            &admin,
            json!({"command": "settings set allow_registration false --yes"})
        )
        .await
        .status(),
        200,
        "a signed-in console may"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn event_streams_end_when_the_member_is_removed() {
    let data_dir = temp_data_dir("event-access");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "stream-owner@example.com").await;
    let (member, registration) = register_account(&app, "stream-member@example.com").await;
    let member_id = registration["user"]["id"].as_str().unwrap().to_owned();
    let workspace_id = create_workspace(&app, &owner).await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "stream-member@example.com",
        "member",
    )
    .await;
    let added = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/workspaces/{workspace_id}/members"),
            &owner,
            json!({"email": "stream-member@example.com", "role": "member"}),
        ))
        .await
        .unwrap();
    assert_eq!(added.status(), 200);

    let stream = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/events"),
            &member,
        ))
        .await
        .unwrap();
    assert_eq!(stream.status(), 200);
    let removed = app
        .clone()
        .oneshot(auth_request(
            "DELETE",
            &format!("/v1/workspaces/{workspace_id}/members/{member_id}"),
            &owner,
        ))
        .await
        .unwrap();
    assert_eq!(removed.status(), 204);
    let ended = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        to_bytes(stream.into_body(), usize::MAX),
    )
    .await;
    assert!(ended.is_ok(), "the stream should close once the member is removed");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn mention_suggestions_are_the_workspaces_members_for_people_who_can_comment() {
    let data_dir = temp_data_dir("mention-suggestions");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (_, _) = register_account(&app, "mentions-admin@example.com").await;
    let (owner, _) = register_account(&app, "mentions-owner@example.com").await;
    let (writer, _) = register_account(&app, "mentions-ada@example.com").await;
    let (viewer, _) = register_account(&app, "mentions-grace@example.com").await;
    let (outsider, _) = register_account(&app, "mentions-eve@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "mentions-ada@example.com",
        "member",
    )
    .await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        "mentions-grace@example.com",
        "viewer",
    )
    .await;
    let suggest = |who: &str, query: &str| {
        let app = app.clone();
        let request = auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace_id}/mention-suggestions?q={query}"),
            who,
        );
        async move { app.oneshot(request).await.unwrap() }
    };
    let emails = |found: &Value| {
        let mut emails = found
            .as_array()
            .unwrap()
            .iter()
            .map(|user| user["email"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        emails.sort();
        emails
    };

    // A writer is offered the other members, not themselves and not strangers.
    let all = response_json(suggest(&writer, "").await).await;
    assert_eq!(
        emails(&all),
        ["mentions-grace@example.com", "mentions-owner@example.com"]
    );
    let typed = response_json(suggest(&writer, "gra").await).await;
    assert_eq!(emails(&typed), ["mentions-grace@example.com"]);
    let by_address = response_json(suggest(&owner, "mentions-ad").await).await;
    assert_eq!(emails(&by_address), ["mentions-ada@example.com"]);
    assert!(response_json(suggest(&owner, "eve").await)
        .await
        .as_array()
        .unwrap()
        .is_empty());

    // Viewers cannot comment, so they are not offered anyone; outsiders see nothing of the workspace.
    assert_eq!(suggest(&viewer, "").await.status(), 403);
    assert_eq!(suggest(&outsider, "").await.status(), 403);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_workspace_exports_as_an_archive_that_holds_everything_it_has() {
    let data_dir = temp_data_dir("workspace-export");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: &str, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (_, _) = register_account(&app, "export-admin@example.com").await;
    let (owner, _) = register_account(&app, "export-owner@example.com").await;
    let (writer, _) = register_account(&app, "export-ada@example.com").await;
    let (viewer, _) = register_account(&app, "export-grace@example.com").await;
    let (outsider, _) = register_account(&app, "export-eve@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    for (email, role) in [("export-ada@example.com", "member"), ("export-grace@example.com", "viewer")] {
        join(&app, &owner, "workspaces", &workspace_id, email, role).await;
    }

    // A folder, three files (two with the same text), a memory card citing a
    // passage and a whole file, a lifecycle change, a task and a comment.
    let folder = response_json(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/folders"),
            &owner,
            json!({"name": "Policies", "parent_id": null}),
        )
        .await,
    )
    .await;
    let folder_id = folder["id"].as_str().unwrap().to_owned();
    let mut ids = Vec::new();
    for (title, content) in [
        ("Refund policy", "Refunds are issued within fourteen days of purchase."),
        ("Shipping notes", "Parcels leave the warehouse every weekday."),
        ("Refund policy copy", "Refunds are issued within fourteen days of purchase."),
    ] {
        let created = send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            &owner,
            json!({"title": title, "content": content}),
        )
        .await;
        ids.push(response_json(created).await["id"].as_str().unwrap().to_owned());
    }
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 3).await;
    let moved = send(
        "POST",
        format!("/v1/workspaces/{workspace_id}/artifacts/bulk"),
        &owner,
        json!({"action": "move", "artifact_ids": [ids[0], ids[1]], "folder_id": folder_id}),
    )
    .await;
    assert_eq!(moved.status(), 200);
    let hits = response_json(
        send(
            "POST",
            format!("/v1/workspaces/{workspace_id}/search"),
            &owner,
            json!({"query": "parcels"}),
        )
        .await,
    )
    .await;
    let chunk_id = hits[0]["chunk_id"].as_str().unwrap().to_owned();
    let card = send(
        "POST",
        format!("/v1/workspaces/{workspace_id}/memory-cards"),
        &owner,
        json!({
            "title": "Dispatch rhythm", "body_markdown": "Parcels leave every weekday.", "source": "test",
            "citations": [
                {"artifact_id": ids[1], "chunk_id": chunk_id, "title": "Shipping notes", "path": "x", "start_line": null, "end_line": null, "confidence": null},
                {"artifact_id": ids[0], "chunk_id": null, "title": "Refund policy", "path": "x", "start_line": null, "end_line": null, "confidence": null},
            ],
        }),
    )
    .await;
    assert_eq!(card.status(), 201);
    assert_eq!(
        send(
            "PUT",
            format!("/v1/artifacts/{}/lifecycle", ids[2]),
            &owner,
            json!({"status": "outdated", "review_note": "A copy."}),
        )
        .await
        .status(),
        200
    );
    let task = send(
        "POST",
        format!("/v1/workspaces/{workspace_id}/tasks"),
        &owner,
        json!({"title": "Check the policy", "status": "open", "priority": "high", "artifact_id": ids[0]}),
    )
    .await;
    assert_eq!(task.status(), 201);
    let task_id = response_json(task).await["id"].as_str().unwrap().to_owned();
    assert_eq!(
        send(
            "POST",
            format!("/v1/tasks/{task_id}/checklist"),
            &owner,
            json!({"body": "Read it twice"}),
        )
        .await
        .status(),
        201
    );
    assert_eq!(
        send(
            "POST",
            format!("/v1/artifacts/{}/comments", ids[0]),
            &writer,
            json!({"body": "Looks right to me."}),
        )
        .await
        .status(),
        201
    );

    // Only owners and administrators export.
    let export_uri = format!("/v1/workspaces/{workspace_id}/export");
    for who in [&writer, &viewer, &outsider] {
        let denied = app.clone().oneshot(auth_request("GET", &export_uri, who)).await.unwrap();
        assert_eq!(denied.status(), 403);
    }

    let response = app
        .clone()
        .oneshot(auth_request("GET", &export_uri, &owner))
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["content-type"], "application/gzip");
    let disposition = response.headers()["content-disposition"].to_str().unwrap().to_owned();
    assert!(
        disposition.contains("repomemo-workspace-") && disposition.ends_with(".tar.gz\""),
        "{disposition}"
    );
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let entries: std::collections::HashMap<String, Vec<u8>> =
        repomemo_api::read_archive_entries(bytes.as_ref(), 1 << 26)
            .unwrap()
            .into_iter()
            .collect();

    let manifest: Value = serde_json::from_slice(&entries["manifest.json"]).unwrap();
    assert_eq!(manifest["format"], "repomemo-workspace-archive");
    assert_eq!(manifest["format_version"], 1);
    let counts = &manifest["counts"];
    assert_eq!(counts["artifacts"], 3);
    assert_eq!(counts["blobs"], 2, "two files share their text, so one stored file");
    assert_eq!(counts["folders"], 1);
    assert_eq!(counts["memory_cards"], 1);
    assert_eq!(counts["citations"], 2);
    assert_eq!(counts["tasks"], 1);
    assert_eq!(counts["checklist_items"], 1);
    assert_eq!(counts["comments"], 1);
    assert_eq!(counts["lifecycle_events"], 1);
    // Every stored file is there under its hash, and the data matches its checksum.
    assert_eq!(entries.len(), 2 + 2);
    for blob in manifest["blobs"].as_array().unwrap() {
        let hash = blob["hash"].as_str().unwrap();
        let bytes = &entries[&format!("blobs/{hash}")];
        assert_eq!(repomemo_storage::StorageEngine::content_hash(bytes), hash);
        assert_eq!(bytes.len() as u64, blob["size_bytes"].as_u64().unwrap());
    }
    let data_bytes = &entries["workspace.json"];
    assert_eq!(
        repomemo_storage::StorageEngine::content_hash(data_bytes),
        manifest["data_sha256"].as_str().unwrap()
    );

    let data: Value = serde_json::from_slice(data_bytes).unwrap();
    assert_eq!(data["workspace"]["name"], "Workspace");
    let artifacts = data["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 3);
    let in_folder = artifacts
        .iter()
        .filter(|artifact| artifact["folder_id"] == folder_id)
        .count();
    assert_eq!(in_folder, 2);
    assert_eq!(data["folders"][0]["name"], "Policies");
    let outdated = data["lifecycle"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["artifact_id"] == ids[2])
        .expect("the changed file has its lifecycle");
    assert_eq!(outdated["status"], "outdated");
    assert_eq!(outdated["review_note"], "A copy.");
    assert_eq!(outdated["events"].as_array().unwrap().len(), 1);
    assert_eq!(outdated["events"][0]["actor"]["email"], "export-owner@example.com");

    let citations = data["memory_cards"][0]["citations"].as_array().unwrap();
    let passage = citations
        .iter()
        .find(|citation| !citation["chunk"].is_null())
        .unwrap();
    assert_eq!(passage["artifact_id"], ids[1]);
    assert!(passage["chunk"]["content_hash"].as_str().unwrap().len() >= 32);
    let whole_file = citations
        .iter()
        .find(|citation| citation["chunk"].is_null())
        .unwrap();
    assert_eq!(whole_file["artifact_id"], ids[0]);

    assert_eq!(data["tasks"][0]["title"], "Check the policy");
    assert_eq!(data["tasks"][0]["created_by"]["email"], "export-owner@example.com");
    assert_eq!(data["tasks"][0]["artifact_id"], ids[0]);
    assert_eq!(data["tasks"][0]["checklist"][0]["body"], "Read it twice");
    assert_eq!(data["comments"][0]["author"]["email"], "export-ada@example.com");
    assert_eq!(data["comments"][0]["body"], "Looks right to me.");
    // No account id of this server is in the archive.
    assert!(!String::from_utf8_lossy(data_bytes).contains("user_id"));

    // The export leaves a line in the workspace's activity.
    let activity = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace_id}/activity"),
                &owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert!(activity
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["action"] == "workspace_exported"));
    let _ = std::fs::remove_dir_all(data_dir);
}
