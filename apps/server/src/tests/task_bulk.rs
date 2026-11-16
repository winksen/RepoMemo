//! Bulk work (V0.7.14): one change for many tasks is previewed first and reports an outcome for each; authorization and the
//! version check are made for each task, and a refused task is left exactly as it was.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_bulk_change_is_previewed_and_each_task_reports_its_own_outcome() {
    let fixture = knowledge_fixture("task-bulk").await;
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
    let owner_id = members
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["user"]["email"] == "task-bulk-owner@example.com")
        .map(|entry| entry["user"]["id"].as_str().unwrap().to_owned())
        .unwrap();
    let project = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace}/projects"),
            fixture.member.clone(),
            json!({"name": "Queue migration"}),
        )
        .await,
    )
    .await;
    let project_id = project["id"].as_str().unwrap().to_owned();
    let milestone = response_json(
        call(
            "POST",
            format!("/v1/projects/{project_id}/milestones"),
            fixture.member.clone(),
            json!({"name": "Dual write"}),
        )
        .await,
    )
    .await;
    let milestone_id = milestone["id"].as_str().unwrap().to_owned();
    let tasks = format!("/v1/workspaces/{workspace}/tasks");
    let make = |title: &'static str, in_project: bool| {
        let body = if in_project {
            json!({"title": title, "status": "open", "priority": "medium", "project_id": project_id})
        } else {
            json!({"title": title, "status": "open", "priority": "medium"})
        };
        call("POST", tasks.clone(), fixture.member.clone(), body)
    };
    let first = response_json(make("First", true).await).await;
    let second = response_json(make("Second", true).await).await;
    let outside = response_json(make("Outside the project", false).await).await;
    let id = |task: &Value| task["id"].as_str().unwrap().to_owned();
    let (first_id, second_id, outside_id) = (id(&first), id(&second), id(&outside));
    let bulk = format!("{tasks}/bulk");
    let read = |task_id: String| {
        let call = &call;
        let who = fixture.member.clone();
        async move { response_json(call("GET", format!("/v1/tasks/{task_id}"), who, Value::Null).await).await }
    };
    let outcomes = |result: &Value| {
        result["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| {
                format!(
                    "{}:{}",
                    item["title"].as_str().unwrap(),
                    item["outcome"].as_str().unwrap()
                )
            })
            .collect::<Vec<_>>()
    };

    // The preview reports what would happen to each task and writes nothing.
    let changes = json!({"assignee_user_id": owner_id, "milestone_id": milestone_id});
    let preview = call(
        "POST",
        bulk.clone(),
        fixture.member.clone(),
        json!({"task_ids": [first_id, second_id, outside_id], "changes": changes, "dry_run": true}),
    )
    .await;
    assert_eq!(preview.status(), 200);
    let preview = response_json(preview).await;
    assert_eq!(preview["dry_run"], true);
    assert_eq!(
        outcomes(&preview),
        ["First:ok", "Second:ok", "Outside the project:refused"]
    );
    assert!(preview["items"][2]["reason"]
        .as_str()
        .unwrap()
        .contains("not in the milestone's project"));
    assert!(
        read(first_id.clone()).await["assignee"].is_null(),
        "a preview changes nothing"
    );

    // Applied: the two tasks in the project change, the third is refused and left exactly as it was.
    let applied = response_json(
        call(
            "POST",
            bulk.clone(),
            fixture.member.clone(),
            json!({"task_ids": [first_id, second_id, outside_id], "changes": changes}),
        )
        .await,
    )
    .await;
    assert_eq!(
        outcomes(&applied),
        ["First:ok", "Second:ok", "Outside the project:refused"]
    );
    assert_eq!(
        (
            applied["succeeded"].as_i64(),
            applied["refused"].as_i64(),
            applied["unchanged"].as_i64()
        ),
        (Some(2), Some(1), Some(0))
    );
    let changed = read(first_id.clone()).await;
    assert_eq!(changed["assignee"]["id"], owner_id.as_str());
    assert_eq!(changed["milestone_id"], milestone_id.as_str());
    assert_eq!(changed["version"], 2);
    let untouched = read(outside_id.clone()).await;
    assert!(untouched["assignee"].is_null() && untouched["milestone_id"].is_null() && untouched["version"] == 1);
    // The new assignee is told once for each task.
    let notices = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/notifications", &fixture.owner))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        notices
            .as_array()
            .unwrap()
            .iter()
            .filter(|notice| notice["notification_type"] == "task_assigned")
            .count(),
        2
    );
    // Doing it again changes nothing.
    let again = response_json(
        call(
            "POST",
            bulk.clone(),
            fixture.member.clone(),
            json!({"task_ids": [first_id, second_id], "changes": changes}),
        )
        .await,
    )
    .await;
    assert_eq!(outcomes(&again), ["First:unchanged", "Second:unchanged"]);

    // The version each task had when it was shown is checked for each task on its own.
    call(
        "PUT",
        format!("/v1/tasks/{first_id}"),
        fixture.owner.clone(),
        json!({"title": "First, renamed", "status": "open", "priority": "medium", "assignee_user_id": owner_id}),
    )
    .await;
    let label = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace}/task-labels"),
            fixture.member.clone(),
            json!({"name": "Bug", "color": "red"}),
        )
        .await,
    )
    .await;
    let labelled = response_json(
        call(
            "POST",
            bulk.clone(),
            fixture.member.clone(),
            json!({"task_ids": [first_id, second_id], "changes": {"add_label_ids": [label["id"]]},
                   "expected_versions": {first_id.clone(): 2, second_id.clone(): 2}}),
        )
        .await,
    )
    .await;
    assert_eq!(outcomes(&labelled), ["First, renamed:refused", "Second:ok"]);
    assert!(labelled["items"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("Changed since you looked"));
    assert!(read(first_id.clone()).await["labels"].as_array().unwrap().is_empty());
    assert_eq!(read(second_id.clone()).await["labels"][0]["name"], "Bug");

    // Removing a label, and the limit of ten labels on a task, are per task.
    let mut many = Vec::new();
    for index in 0..10 {
        let created = response_json(
            call(
                "POST",
                format!("/v1/workspaces/{workspace}/task-labels"),
                fixture.member.clone(),
                json!({"name": format!("Kind {index}")}),
            )
            .await,
        )
        .await;
        many.push(created["id"].clone());
    }
    call(
        "PUT",
        format!("/v1/tasks/{outside_id}/labels"),
        fixture.member.clone(),
        json!({"label_ids": many}),
    )
    .await;
    let mixed = response_json(
        call(
            "POST",
            bulk.clone(),
            fixture.member.clone(),
            json!({"task_ids": [second_id, outside_id, "nowhere"], "changes": {"add_label_ids": [many[0], many[1]]}}),
        )
        .await,
    )
    .await;
    assert_eq!(
        outcomes(&mixed),
        ["Second:ok", "Outside the project:unchanged", ":refused"],
        "a task that has them already is unchanged, an unknown one is refused"
    );
    let full = response_json(
        call(
            "POST",
            bulk.clone(),
            fixture.member.clone(),
            json!({"task_ids": [outside_id], "changes": {"add_label_ids": [label["id"]]}}),
        )
        .await,
    )
    .await;
    assert_eq!(outcomes(&full), ["Outside the project:refused"]);
    assert!(full["items"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("more than 10 labels"));
    let removed = response_json(
        call(
            "POST",
            bulk.clone(),
            fixture.member.clone(),
            json!({"task_ids": [second_id], "changes": {"remove_label_ids": [label["id"]]}}),
        )
        .await,
    )
    .await;
    assert_eq!(outcomes(&removed), ["Second:ok"]);

    // What is wrong for every task is a bad request; a reader cannot bulk-change; another workspace's task is not found.
    let other_workspace = create_workspace(&app, &fixture.owner).await;
    let foreign_task = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{other_workspace}/tasks"),
            fixture.owner.clone(),
            json!({"title": "Elsewhere", "status": "open", "priority": "low"}),
        )
        .await,
    )
    .await;
    let foreign_label = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{other_workspace}/task-labels"),
            fixture.owner.clone(),
            json!({"name": "Elsewhere"}),
        )
        .await,
    )
    .await;
    for body in [
        json!({"task_ids": [first_id], "changes": {}}),
        json!({"task_ids": [], "changes": {"assignee_user_id": owner_id}}),
        json!({"task_ids": [first_id], "changes": {"add_label_ids": [label["id"]], "remove_label_ids": [label["id"]]}}),
        json!({"task_ids": [first_id], "changes": {"add_label_ids": [foreign_label["id"]]}}),
        json!({"task_ids": [first_id], "changes": {"assignee_user_id": "not-a-member"}}),
        json!({"task_ids": [first_id], "changes": {"milestone_id": "not-a-milestone"}}),
        json!({"task_ids": vec![first_id.clone(); 1].into_iter().chain((0..100).map(|index| format!("id-{index}"))).collect::<Vec<_>>(),
               "changes": {"assignee_user_id": owner_id}}),
    ] {
        assert_eq!(
            call("POST", bulk.clone(), fixture.member.clone(), body).await.status(),
            400
        );
    }
    assert_eq!(
        call(
            "POST",
            bulk.clone(),
            fixture.guest.clone(),
            json!({"task_ids": [first_id], "changes": {"assignee_user_id": owner_id}})
        )
        .await
        .status(),
        403
    );
    let foreign_result = response_json(
        call(
            "POST",
            bulk,
            fixture.member.clone(),
            json!({"task_ids": [foreign_task["id"]], "changes": {"assignee_user_id": owner_id}}),
        )
        .await,
    )
    .await;
    assert_eq!(foreign_result["items"][0]["outcome"], "refused");
    assert_eq!(
        foreign_result["items"][0]["reason"],
        "Task was not found in this workspace."
    );
}
