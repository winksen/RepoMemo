//! Task details (V0.7.3): a task has a number that names it for good (`T-12`), a link keeps opening the same task after its
//! title changes, a guest gets an authorized view, and a deleted or unknown task answers with a reason.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_task_keeps_its_number_through_renames_and_a_deleted_one_is_never_reused() {
    let fixture = knowledge_fixture("task-details").await;
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

    // Numbers count up from one and the identifier is built from them.
    let first = response_json(make("Write the runbook").await).await;
    let second = response_json(make("Move the worker").await).await;
    assert_eq!(
        (first["number"].as_i64(), first["key"].as_str()),
        (Some(1), Some("T-1"))
    );
    assert_eq!(
        (second["number"].as_i64(), second["key"].as_str()),
        (Some(2), Some("T-2"))
    );

    // The identifier, the bare number and the id all open the same task, with its checklist and its address.
    for reference in ["T-1", "t-1", "1", first["id"].as_str().unwrap()] {
        let detail = call(
            "GET",
            format!("{tasks}/{reference}"),
            fixture.member.clone(),
            Value::Null,
        )
        .await;
        assert_eq!(detail.status(), 200, "reference {reference}");
        let detail = response_json(detail).await;
        assert_eq!(detail["task"]["id"], first["id"]);
        assert_eq!(detail["path"], format!("/workspaces/{workspace}/tasks/T-1"));
        assert!(detail["checklist"].as_array().unwrap().is_empty());
    }

    // A new title does not change what the link opens.
    let renamed = call(
        "PUT",
        format!("/v1/tasks/{}", first["id"].as_str().unwrap()),
        fixture.member.clone(),
        json!({"title": "Write the on-call runbook", "status": "open", "priority": "medium"}),
    )
    .await;
    assert_eq!(renamed.status(), 200);
    let detail = response_json(call("GET", format!("{tasks}/T-1"), fixture.member.clone(), Value::Null).await).await;
    assert_eq!(detail["task"]["title"], "Write the on-call runbook");
    assert_eq!(detail["task"]["number"], 1);

    // A guest who can read the workspace gets the view; a stranger does not.
    assert_eq!(
        call("GET", format!("{tasks}/T-1"), fixture.guest.clone(), Value::Null)
            .await
            .status(),
        200
    );
    let (stranger, _) = register_account(&app, "task-details-stranger@example.com").await;
    assert_eq!(
        call("GET", format!("{tasks}/T-1"), stranger, Value::Null)
            .await
            .status(),
        403
    );

    // A task of another workspace is not reachable through this one, by number or by id.
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
    assert_eq!(foreign["number"], 1, "numbers are per workspace");
    assert_eq!(
        call(
            "GET",
            format!("{tasks}/{}", foreign["id"].as_str().unwrap()),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        404
    );

    // A deleted task says it was deleted, an unknown one says it does not exist, and the number is not given out again.
    assert_eq!(
        call(
            "DELETE",
            format!("/v1/tasks/{}", second["id"].as_str().unwrap()),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        204
    );
    let gone = call("GET", format!("{tasks}/T-2"), fixture.member.clone(), Value::Null).await;
    assert_eq!(gone.status(), 410);
    assert!(response_json(gone).await["error"]["message"]
        .as_str()
        .unwrap()
        .contains("deleted"));
    let missing = call("GET", format!("{tasks}/T-99"), fixture.member.clone(), Value::Null).await;
    assert_eq!(missing.status(), 404);
    assert_eq!(
        call(
            "GET",
            format!("{tasks}/not-a-task"),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        404
    );
    let third = response_json(make("Close the migration").await).await;
    assert_eq!(third["key"], "T-3");
}
