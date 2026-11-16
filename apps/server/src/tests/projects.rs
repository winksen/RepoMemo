//! Projects (V0.7.1): a member creates and archives a project, its access is exactly the workspace's, a task can belong to
//! one project of its own workspace, and a task with no project keeps working.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_member_creates_and_archives_a_project_and_legacy_tasks_keep_working() {
    let fixture = knowledge_fixture("projects").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let projects = format!("/v1/workspaces/{workspace}/projects");
    let tasks = format!("/v1/workspaces/{workspace}/tasks");

    // A member creates a project; the goal and the target date are kept as typed.
    let created = call(
        "POST",
        projects.clone(),
        fixture.member.clone(),
        json!({"name": "  Queue migration ", "goal": "Move the worker to the managed queue.", "target_date": "2027-02-01"}),
    )
    .await;
    assert_eq!(created.status(), 201);
    let project = response_json(created).await;
    let project_id = project["id"].as_str().unwrap().to_owned();
    assert_eq!(project["name"], "Queue migration");
    assert_eq!(project["status"], "active");
    assert_eq!(project["target_date"], "2027-02-01");
    assert_eq!(project["task_count"], 0);

    // What is not a project is refused with a reason.
    for body in [
        json!({"name": "   "}),
        json!({"name": "x".repeat(121)}),
        json!({"name": "Plan", "status": "archived"}),
        json!({"name": "Plan", "target_date": "next week"}),
        json!({"name": "Plan", "owner_user_id": "not-a-member"}),
    ] {
        assert_eq!(
            call("POST", projects.clone(), fixture.member.clone(), body)
                .await
                .status(),
            400
        );
    }

    // Access is exactly the workspace's: a reader sees it and cannot change it, a stranger sees nothing.
    let guest_list = call("GET", projects.clone(), fixture.guest.clone(), Value::Null).await;
    assert_eq!(guest_list.status(), 200);
    assert_eq!(response_json(guest_list).await.as_array().unwrap().len(), 1);
    assert_eq!(
        call(
            "POST",
            projects.clone(),
            fixture.guest.clone(),
            json!({"name": "A reader's project"})
        )
        .await
        .status(),
        403
    );
    assert_eq!(
        call(
            "PUT",
            format!("/v1/projects/{project_id}"),
            fixture.guest.clone(),
            json!({"name": "Renamed"})
        )
        .await
        .status(),
        403
    );
    let (stranger, _) = register_account(&app, "projects-stranger@example.com").await;
    assert_eq!(
        call("GET", projects.clone(), stranger.clone(), Value::Null)
            .await
            .status(),
        403
    );
    assert_eq!(
        call("GET", format!("/v1/projects/{project_id}"), stranger, Value::Null)
            .await
            .status(),
        403
    );

    // A task made the old way has no project; a task can be put in the project and counts toward it.
    let legacy = response_json(
        call(
            "POST",
            tasks.clone(),
            fixture.member.clone(),
            json!({"title": "Old task", "status": "open", "priority": "medium"}),
        )
        .await,
    )
    .await;
    assert!(legacy["project_id"].is_null());
    let in_project = call(
        "POST",
        tasks.clone(),
        fixture.member.clone(),
        json!({"title": "Dual write", "status": "open", "priority": "high", "project_id": project_id}),
    )
    .await;
    assert_eq!(in_project.status(), 201);
    let in_project = response_json(in_project).await;
    assert_eq!(in_project["project_id"], project_id.as_str());
    assert_eq!(in_project["project_name"], "Queue migration");
    let task_uri = format!("/v1/tasks/{}", in_project["id"].as_str().unwrap());

    // Saving without a project keeps it; an empty value removes it.
    let kept = response_json(
        call(
            "PUT",
            task_uri.clone(),
            fixture.member.clone(),
            json!({"title": "Dual write", "status": "in_progress", "priority": "high"}),
        )
        .await,
    )
    .await;
    assert_eq!(kept["project_id"], project_id.as_str());
    let counted = response_json(
        call(
            "GET",
            format!("/v1/projects/{project_id}"),
            fixture.member.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(
        (counted["task_count"].as_i64(), counted["open_task_count"].as_i64()),
        (Some(1), Some(1))
    );

    // A project of another workspace cannot be used.
    let other_workspace = create_workspace(&app, &fixture.owner).await;
    let foreign = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{other_workspace}/projects"),
            fixture.owner.clone(),
            json!({"name": "Elsewhere"}),
        )
        .await,
    )
    .await;
    assert_eq!(
        call(
            "PUT",
            task_uri.clone(),
            fixture.member.clone(),
            json!({"title": "Dual write", "status": "open", "priority": "high", "project_id": foreign["id"]})
        )
        .await
        .status(),
        400
    );

    // Archiving keeps the project and its tasks readable, leaves it out of the default list and stops new work in it.
    let archived = call(
        "POST",
        format!("/v1/projects/{project_id}/archive"),
        fixture.member.clone(),
        Value::Null,
    )
    .await;
    assert_eq!(archived.status(), 200);
    let archived = response_json(archived).await;
    assert_eq!(archived["status"], "archived");
    assert!(archived["archived_at"].is_string());
    assert_eq!(
        response_json(call("GET", projects.clone(), fixture.member.clone(), Value::Null).await)
            .await
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        response_json(
            call(
                "GET",
                format!("{projects}?status=archived"),
                fixture.member.clone(),
                Value::Null
            )
            .await
        )
        .await
        .as_array()
        .unwrap()
        .len(),
        1
    );
    assert_eq!(
        call(
            "POST",
            tasks.clone(),
            fixture.member.clone(),
            json!({"title": "Late", "status": "open", "priority": "low", "project_id": project_id})
        )
        .await
        .status(),
        409
    );
    assert_eq!(
        call(
            "PUT",
            format!("/v1/projects/{project_id}"),
            fixture.member.clone(),
            json!({"name": "Changed while archived"})
        )
        .await
        .status(),
        409
    );
    // The task it already had still saves.
    assert_eq!(
        call(
            "PUT",
            task_uri.clone(),
            fixture.member.clone(),
            json!({"title": "Dual write", "status": "done", "priority": "high"})
        )
        .await
        .status(),
        200
    );

    // Restoring brings it back, and a task can be taken out of a project.
    let restored = response_json(
        call(
            "POST",
            format!("/v1/projects/{project_id}/restore"),
            fixture.member.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(restored["status"], "active");
    assert_eq!(restored["open_task_count"], 0);
    let removed = response_json(
        call(
            "PUT",
            task_uri,
            fixture.member.clone(),
            json!({"title": "Dual write", "status": "done", "priority": "high", "project_id": ""}),
        )
        .await,
    )
    .await;
    assert!(removed["project_id"].is_null());
}
