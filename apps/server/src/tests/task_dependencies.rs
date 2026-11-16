//! Dependencies (V0.7.9): a task can block another; a self-link, a repeat, a cycle and a link across workspaces are refused;
//! the blocked task says in words what blocks it; closing a blocker updates that and completes nothing.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn blockers_are_explained_cycles_are_refused_and_closing_a_blocker_completes_nothing() {
    let fixture = knowledge_fixture("task-dependencies").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let tasks = format!("/v1/workspaces/{workspace}/tasks");
    let make = |title: &'static str| {
        call(
            "POST",
            tasks.clone(),
            fixture.member.clone(),
            json!({"title": title, "status": "open", "priority": "medium"}),
        )
    };
    let backfill = response_json(make("Backfill").await).await;
    let cut_over = response_json(make("Cut over").await).await;
    let announce = response_json(make("Announce").await).await;
    let id = |task: &Value| task["id"].as_str().unwrap().to_owned();
    let (backfill_id, cut_over_id, announce_id) = (id(&backfill), id(&cut_over), id(&announce));
    let links = |task_id: &str| format!("/v1/tasks/{task_id}/dependencies");

    // "Cut over" is blocked by "Backfill"; the same link can be made from the other side, but only once.
    let made = call(
        "POST",
        links(&cut_over_id),
        fixture.member.clone(),
        json!({"blocker_task_id": backfill_id}),
    )
    .await;
    assert_eq!(made.status(), 201);
    let made = response_json(made).await;
    assert_eq!(
        (made["blocker"]["key"].as_str(), made["blocked"]["key"].as_str()),
        (Some("T-1"), Some("T-2"))
    );
    assert_eq!(
        call(
            "POST",
            links(&backfill_id),
            fixture.member.clone(),
            json!({"blocked_task_id": cut_over_id})
        )
        .await
        .status(),
        409
    );

    // What is refused: itself, a cycle (directly and through another task), a bad body, another workspace, a reader.
    assert_eq!(
        call(
            "POST",
            links(&backfill_id),
            fixture.member.clone(),
            json!({"blocker_task_id": backfill_id})
        )
        .await
        .status(),
        400
    );
    assert_eq!(
        call(
            "POST",
            links(&backfill_id),
            fixture.member.clone(),
            json!({"blocker_task_id": cut_over_id})
        )
        .await
        .status(),
        400,
        "a direct cycle"
    );
    call(
        "POST",
        links(&announce_id),
        fixture.member.clone(),
        json!({"blocker_task_id": cut_over_id}),
    )
    .await;
    assert_eq!(
        call(
            "POST",
            links(&backfill_id),
            fixture.member.clone(),
            json!({"blocker_task_id": announce_id})
        )
        .await
        .status(),
        400,
        "a cycle through another task"
    );
    assert_eq!(
        call("POST", links(&backfill_id), fixture.member.clone(), json!({}))
            .await
            .status(),
        400
    );
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
    assert_eq!(
        call(
            "POST",
            links(&backfill_id),
            fixture.member.clone(),
            json!({"blocker_task_id": foreign["id"]})
        )
        .await
        .status(),
        400
    );
    assert_eq!(
        call(
            "POST",
            links(&backfill_id),
            fixture.guest.clone(),
            json!({"blocked_task_id": announce_id})
        )
        .await
        .status(),
        403
    );

    // The blocked task says what blocks it, and its counts say the same.
    let cut_over_now = response_json(
        call(
            "GET",
            format!("/v1/tasks/{cut_over_id}"),
            fixture.guest.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(
        (
            cut_over_now["open_blocker_count"].as_i64(),
            cut_over_now["blocks_count"].as_i64()
        ),
        (Some(1), Some(1))
    );
    let detail = response_json(call("GET", format!("{tasks}/T-2"), fixture.guest.clone(), Value::Null).await).await;
    assert_eq!(detail["dependencies"]["blocked_by"].as_array().unwrap().len(), 1);
    assert_eq!(detail["dependencies"]["blocks"].as_array().unwrap().len(), 1);
    assert_eq!(detail["dependencies"]["explanation"], "Blocked by T-1 Backfill (open).");

    // Closing the blocker updates the indication and completes nothing that depends on it.
    call(
        "PUT",
        format!("/v1/tasks/{backfill_id}"),
        fixture.member.clone(),
        json!({"title": "Backfill", "status": "done", "priority": "medium"}),
    )
    .await;
    let after = response_json(call("GET", format!("{tasks}/T-2"), fixture.member.clone(), Value::Null).await).await;
    assert_eq!(after["task"]["open_blocker_count"], 0);
    assert_eq!(
        after["task"]["status"], "open",
        "the dependent task is not completed for anyone"
    );
    assert_eq!(
        after["dependencies"]["explanation"],
        "Not blocked: 1 of its blockers is done."
    );

    // A link is removed by anyone who can edit; deleting a task takes its links with it.
    let link_id = after["dependencies"]["blocked_by"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        call(
            "DELETE",
            format!("/v1/task-dependencies/{link_id}"),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        204
    );
    assert_eq!(
        call(
            "DELETE",
            format!("/v1/task-dependencies/{link_id}"),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        404
    );
    assert_eq!(
        call(
            "DELETE",
            format!("/v1/tasks/{cut_over_id}"),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        204
    );
    let listed = response_json(call("GET", links(&announce_id), fixture.guest.clone(), Value::Null).await).await;
    assert!(listed["blocked_by"].as_array().unwrap().is_empty());
    assert!(listed["explanation"].is_null());
}
