//! My work (V0.6.2): assigned tasks, owned files that need review and the counts follow current access.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn my_work_lists_what_waits_and_loses_a_workspace_the_person_left() {
    let fixture = knowledge_fixture("my-work").await;
    let app = fixture.app.clone();
    let (helper, helper_account) = register_account(&app, "my-work-helper@example.com").await;
    let helper_id = helper_account["user"]["id"].as_str().unwrap().to_owned();
    join(
        &app,
        &fixture.owner,
        "workspaces",
        &fixture.workspace_id,
        "my-work-helper@example.com",
        "member",
    )
    .await;
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let work = || {
        let app = app.clone();
        let who = helper.clone();
        async move {
            response_json(
                app.oneshot(auth_request("GET", "/v1/profile/work", &who))
                    .await
                    .unwrap(),
            )
            .await
        }
    };
    assert_eq!(work().await["total_count"], 0, "nothing yet");

    // An overdue task and a file the helper owns that needs review.
    let task = call(
        "POST",
        format!("/v1/workspaces/{}/tasks", fixture.workspace_id),
        fixture.owner.clone(),
        json!({"title":"Update the runbook","status":"open","priority":"high","assignee_user_id":helper_id,"due_at":"2000-01-01T00:00:00Z"}),
    )
    .await;
    assert!(task.status().is_success(), "{}", task.status());
    let lifecycle = call(
        "PUT",
        format!("/v1/artifacts/{}/lifecycle", fixture.current_id),
        fixture.owner.clone(),
        json!({"status":"needs_review","owner_user_id":helper_id,"review_note":"Check the Tuesday rule.","superseded_by_artifact_id":null}),
    )
    .await;
    assert_eq!(lifecycle.status(), 200);
    let view = work().await;
    assert_eq!(view["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(view["reviews"].as_array().unwrap().len(), 1);
    assert_eq!(view["reviews"][0]["title"], "Deploy checklist");
    assert_eq!(view["overdue_task_count"], 1);
    assert_eq!(view["total_count"], 2);

    // Someone else's work is not in the owner's list.
    let owners = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/profile/work", &fixture.owner))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(owners["total_count"], 0);

    // After the helper leaves the workspace, neither the items nor their count remain.
    let removed = call(
        "DELETE",
        format!("/v1/workspaces/{}/members/{helper_id}", fixture.workspace_id),
        fixture.owner.clone(),
        Value::Null,
    )
    .await;
    assert!(removed.status().is_success(), "{}", removed.status());
    let view = work().await;
    assert_eq!(view["total_count"], 0);
    assert!(view["tasks"].as_array().unwrap().is_empty());
    assert!(view["reviews"].as_array().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
