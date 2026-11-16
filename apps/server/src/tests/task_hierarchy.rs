//! Task hierarchy (V0.7.8): a task can have sub-tasks of the same workspace; a cycle and a tree that is too deep are refused;
//! completing a parent hides none of its unfinished sub-tasks; removing a parent leaves them as ordinary tasks.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn sub_tasks_stay_in_their_workspace_cycles_are_refused_and_a_done_parent_hides_nothing() {
    let fixture = knowledge_fixture("task-hierarchy").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let tasks = format!("/v1/workspaces/{workspace}/tasks");
    let make = |title: String, parent: Value| {
        call(
            "POST",
            tasks.clone(),
            fixture.member.clone(),
            json!({"title": title, "status": "open", "priority": "medium", "parent_task_id": parent}),
        )
    };

    // A parent and two sub-tasks; the parent knows how many it has and how many are open.
    let parent = response_json(make("Migrate the queue".into(), Value::Null).await).await;
    let parent_id = parent["id"].as_str().unwrap().to_owned();
    let first = response_json(make("Dual write".into(), json!(parent_id)).await).await;
    let second = response_json(make("Backfill".into(), json!(parent_id)).await).await;
    assert_eq!(first["parent_task_id"], parent_id.as_str());
    assert_eq!(first["parent_key"], "T-1");
    let parent_now = response_json(
        call(
            "GET",
            format!("/v1/tasks/{parent_id}"),
            fixture.guest.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(
        (
            parent_now["child_count"].as_i64(),
            parent_now["open_child_count"].as_i64()
        ),
        (Some(2), Some(2))
    );
    let detail = response_json(call("GET", format!("{tasks}/T-1"), fixture.guest.clone(), Value::Null).await).await;
    assert_eq!(detail["children"].as_array().unwrap().len(), 2);
    let children = response_json(
        call(
            "GET",
            format!("/v1/tasks/{parent_id}/children"),
            fixture.guest.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(children.as_array().unwrap().len(), 2);

    // A task cannot be its own parent or sit under something below it; a parent from another workspace or an unknown one is refused.
    let first_uri = format!("/v1/tasks/{}", first["id"].as_str().unwrap());
    let save = |uri: String, parent: Value| {
        call(
            "PUT",
            uri,
            fixture.member.clone(),
            json!({"title": "Moved", "status": "open", "priority": "medium", "parent_task_id": parent}),
        )
    };
    assert_eq!(
        save(first_uri.clone(), first["id"].clone()).await.status(),
        400,
        "its own parent"
    );
    assert_eq!(
        save(format!("/v1/tasks/{parent_id}"), first["id"].clone())
            .await
            .status(),
        400,
        "under its own sub-task"
    );
    assert_eq!(save(first_uri.clone(), json!("not-a-task")).await.status(), 400);
    let other_workspace = create_workspace(&app, &fixture.owner).await;
    let foreign = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{other_workspace}/tasks"),
            fixture.owner.clone(),
            json!({"title": "Elsewhere", "status": "open", "priority": "low"}),
        )
        .await,
    )
    .await;
    assert_eq!(save(first_uri.clone(), foreign["id"].clone()).await.status(), 400);

    // A tree is at most five levels deep: a chain of three more under the sub-task makes five; one more is refused.
    let mut tail = first["id"].as_str().unwrap().to_owned();
    for level in 3..=5 {
        let next = make(format!("Level {level}"), json!(tail)).await;
        assert_eq!(next.status(), 201, "level {level}");
        tail = response_json(next).await["id"].as_str().unwrap().to_owned();
    }
    assert_eq!(make("Level 6".into(), json!(tail)).await.status(), 400);
    // Moving the whole branch under something deeper counts what hangs below it.
    assert_eq!(
        save(
            format!("/v1/tasks/{}", parent["id"].as_str().unwrap()),
            json!(second["id"])
        )
        .await
        .status(),
        400
    );

    // Completing the parent hides nothing: its sub-tasks stay open and listed, and none is completed for it.
    let done = response_json(
        call(
            "PUT",
            format!("/v1/tasks/{parent_id}"),
            fixture.member.clone(),
            json!({"title": "Migrate the queue", "status": "done", "priority": "medium"}),
        )
        .await,
    )
    .await;
    assert_eq!(done["status"], "done");
    assert_eq!(done["open_child_count"], 2);
    let still = response_json(
        call(
            "GET",
            format!("/v1/tasks/{}", second["id"].as_str().unwrap()),
            fixture.member.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(still["status"], "open");
    let listed = response_json(
        call(
            "GET",
            format!("/v1/tasks/{parent_id}/children"),
            fixture.member.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(listed.as_array().unwrap().len(), 2);

    // Taking a sub-task out of its parent, and removing a parent, leave ordinary tasks.
    let freed = response_json(save(format!("/v1/tasks/{}", second["id"].as_str().unwrap()), json!("")).await).await;
    assert!(freed["parent_task_id"].is_null());
    assert_eq!(
        call(
            "DELETE",
            format!("/v1/tasks/{parent_id}"),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        204
    );
    let orphan = response_json(call("GET", first_uri, fixture.member.clone(), Value::Null).await).await;
    assert!(orphan["parent_task_id"].is_null());
    assert_eq!(orphan["child_count"], 1, "its own sub-task is still under it");
}
