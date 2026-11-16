//! Task editing (V0.7.6): a save that says which version it started from is applied only if that is still the current one, a
//! refused save returns the task as it is and the change that was attempted, and two saves from the same version cannot both
//! succeed.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn concurrent_task_edits_cannot_silently_drop_each_others_changes() {
    let fixture = knowledge_fixture("task-conflicts").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let members = response_json(
        call(
            "GET",
            format!("/v1/workspaces/{workspace}/members"),
            fixture.owner.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    let member_id = members
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["user"]["email"] == "task-conflicts-member@example.com")
        .map(|entry| entry["user"]["id"].as_str().unwrap().to_owned())
        .unwrap();

    // A new task is at version 1, and every save adds one.
    let task = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace}/tasks"),
            fixture.owner.clone(),
            json!({"title": "Move the worker", "description": "First draft.", "status": "open", "priority": "medium"}),
        )
        .await,
    )
    .await;
    assert_eq!(task["version"], 1);
    let uri = format!("/v1/tasks/{}", task["id"].as_str().unwrap());

    // The owner and a member both open version 1. The owner assigns the task and starts it.
    let owner_save = call(
        "PUT",
        uri.clone(),
        fixture.owner.clone(),
        json!({"title": "Move the worker", "description": "First draft.", "status": "in_progress", "priority": "medium",
               "assignee_user_id": member_id, "expected_version": 1}),
    )
    .await;
    assert_eq!(owner_save.status(), 200);
    let saved = response_json(owner_save).await;
    assert_eq!(saved["version"], 2);

    // The member saves a new description from version 1: refused, nothing is dropped, and their text comes back.
    let stale = call(
        "PUT",
        uri.clone(),
        fixture.member.clone(),
        json!({"title": "Move the worker", "description": "A better description.", "status": "open", "priority": "medium",
               "expected_version": 1}),
    )
    .await;
    assert_eq!(stale.status(), 409);
    let stale = response_json(stale).await;
    assert_eq!(stale["error"]["code"], "conflict");
    assert_eq!(stale["current"]["version"], 2);
    assert_eq!(stale["current"]["status"], "in_progress");
    assert_eq!(stale["current"]["assignee"]["id"], member_id.as_str());
    assert_eq!(stale["attempted"]["description"], "A better description.");
    let unchanged = response_json(call("GET", uri.clone(), fixture.member.clone(), Value::Null).await).await;
    assert_eq!(unchanged["description"], "First draft.");
    assert_eq!(unchanged["status"], "in_progress");
    assert_eq!(unchanged["assignee"]["id"], member_id.as_str());

    // The member keeps their change on top of the current task and saves again.
    let retried = call(
        "PUT",
        uri.clone(),
        fixture.member.clone(),
        json!({"title": "Move the worker", "description": "A better description.", "status": "in_progress",
               "priority": "medium", "assignee_user_id": member_id, "expected_version": 2}),
    )
    .await;
    assert_eq!(retried.status(), 200);
    assert_eq!(response_json(retried).await["version"], 3);

    // Two saves from the same version: exactly one is stored.
    let save = |who: String, title: &'static str| {
        call(
            "PUT",
            uri.clone(),
            who,
            json!({"title": title, "description": "A better description.", "status": "in_progress",
                   "priority": "medium", "assignee_user_id": member_id, "expected_version": 3}),
        )
    };
    let (first, second) = tokio::join!(
        save(fixture.owner.clone(), "Owner's title"),
        save(fixture.member.clone(), "Member's title")
    );
    let mut results = [first.status().as_u16(), second.status().as_u16()];
    results.sort_unstable();
    assert_eq!(results, [200, 409]);
    let current = response_json(call("GET", uri.clone(), fixture.owner.clone(), Value::Null).await).await;
    assert_eq!(current["version"], 4);

    // A save that names no version is not checked and still overwrites, as before; it counts as a new version.
    let unchecked = call(
        "PUT",
        uri,
        fixture.owner.clone(),
        json!({"title": "Final title", "description": "A better description.", "status": "done", "priority": "medium",
               "assignee_user_id": member_id}),
    )
    .await;
    assert_eq!(unchecked.status(), 200);
    assert_eq!(response_json(unchecked).await["version"], 5);
}
