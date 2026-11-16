//! Milestones (V0.7.7): a project's tasks are grouped under named, dated milestones; a lead sees what is done, what remains and
//! what is overdue with the denominator named; changing a date moves no task; a task's milestone is always in its project.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_lead_plans_a_milestone_and_sees_what_is_done_remaining_and_overdue() {
    let fixture = knowledge_fixture("milestones").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
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
    let milestones = format!("/v1/projects/{project_id}/milestones");

    // A milestone has a name, a date and a rule; what is not one is refused, and a reader cannot make it.
    let created = call(
        "POST",
        milestones.clone(),
        fixture.member.clone(),
        json!({"name": "Dual write", "due_date": "2020-01-31", "description": "Write to both queues."}),
    )
    .await;
    assert_eq!(created.status(), 201);
    let milestone = response_json(created).await;
    let milestone_id = milestone["id"].as_str().unwrap().to_owned();
    assert_eq!(milestone["completion_rule"], "all_tasks");
    assert_eq!(milestone["task_count"], 0);
    assert_eq!(milestone["completed"], false, "no tasks: not complete by rule");
    assert!(milestone["explanation"].as_str().unwrap().contains("no tasks yet"));
    for body in [
        json!({"name": "  "}),
        json!({"name": "x".repeat(121)}),
        json!({"name": "Plan", "due_date": "soon"}),
        json!({"name": "Plan", "completion_rule": "magic"}),
    ] {
        assert_eq!(
            call("POST", milestones.clone(), fixture.member.clone(), body)
                .await
                .status(),
            400
        );
    }
    assert_eq!(
        call(
            "POST",
            milestones.clone(),
            fixture.guest.clone(),
            json!({"name": "Reader's"})
        )
        .await
        .status(),
        403
    );

    // Tasks join the milestone through the project. Three tasks: one done, one overdue, one with a date in the future.
    let tasks = format!("/v1/workspaces/{workspace}/tasks");
    let make = |title: &'static str, status: &'static str, due: Value| {
        call(
            "POST",
            tasks.clone(),
            fixture.member.clone(),
            json!({"title": title, "status": status, "priority": "medium", "project_id": project_id,
                   "milestone_id": milestone_id, "due_at": due}),
        )
    };
    let done = response_json(make("Dual write the queue", "done", Value::Null).await).await;
    let overdue = response_json(make("Backfill", "open", json!("2020-01-10")).await).await;
    let later = response_json(make("Cut over", "in_progress", json!("2999-01-01")).await).await;
    assert_eq!(done["milestone_name"], "Dual write");
    let seen = response_json(call("GET", milestones.clone(), fixture.guest.clone(), Value::Null).await).await;
    assert_eq!(seen.as_array().unwrap().len(), 1);
    let seen = &seen[0];
    assert_eq!(
        (
            seen["task_count"].as_i64(),
            seen["done_count"].as_i64(),
            seen["remaining_count"].as_i64(),
            seen["overdue_task_count"].as_i64()
        ),
        (Some(3), Some(1), Some(2), Some(1))
    );
    assert_eq!(seen["overdue"], true, "its date is past and it is not complete");
    assert_eq!(seen["completed"], false);
    let explanation = seen["explanation"].as_str().unwrap();
    assert!(explanation.contains("1 of 3 tasks are done") && explanation.contains("1 of them overdue"));

    // Changing the date moves no task's own date.
    let changed = call(
        "PUT",
        format!("/v1/milestones/{milestone_id}"),
        fixture.member.clone(),
        json!({"name": "Dual write", "due_date": "2999-06-30"}),
    )
    .await;
    assert_eq!(changed.status(), 200);
    let changed = response_json(changed).await;
    assert_eq!(changed["overdue"], false);
    assert_eq!(changed["overdue_task_count"], 1, "the task's own date did not move");
    let still = response_json(
        call(
            "GET",
            format!("/v1/tasks/{}", overdue["id"].as_str().unwrap()),
            fixture.member.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(still["due_at"], "2020-01-10");

    // A task's milestone belongs to the project it is in: another project's milestone, or no project, is refused, and moving the
    // task out of the project drops the milestone.
    let other_project = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace}/projects"),
            fixture.member.clone(),
            json!({"name": "Other"}),
        )
        .await,
    )
    .await;
    let other_milestone = response_json(
        call(
            "POST",
            format!("/v1/projects/{}/milestones", other_project["id"].as_str().unwrap()),
            fixture.member.clone(),
            json!({"name": "Elsewhere"}),
        )
        .await,
    )
    .await;
    let later_uri = format!("/v1/tasks/{}", later["id"].as_str().unwrap());
    assert_eq!(
        call(
            "PUT",
            later_uri.clone(),
            fixture.member.clone(),
            json!({"title": "Cut over", "status": "in_progress", "priority": "medium",
                   "milestone_id": other_milestone["id"]})
        )
        .await
        .status(),
        400
    );
    let moved = response_json(
        call(
            "PUT",
            later_uri,
            fixture.member.clone(),
            json!({"title": "Cut over", "status": "in_progress", "priority": "medium", "project_id": ""}),
        )
        .await,
    )
    .await;
    assert!(moved["project_id"].is_null() && moved["milestone_id"].is_null());

    // By rule it completes when every task is done; closing the last two tasks completes it, and nothing was completed for them.
    call(
        "PUT",
        format!("/v1/tasks/{}", overdue["id"].as_str().unwrap()),
        fixture.member.clone(),
        json!({"title": "Backfill", "status": "done", "priority": "medium"}),
    )
    .await;
    let complete = response_json(
        call(
            "GET",
            format!("/v1/milestones/{milestone_id}"),
            fixture.guest.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(
        (complete["task_count"].as_i64(), complete["completed"].as_bool()),
        (Some(2), Some(true))
    );

    // A manual milestone is completed by a person; a rule milestone cannot be completed by hand.
    assert_eq!(
        call(
            "POST",
            format!("/v1/milestones/{milestone_id}/complete"),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        409
    );
    let manual = response_json(
        call(
            "POST",
            milestones.clone(),
            fixture.member.clone(),
            json!({"name": "Go live", "completion_rule": "manual"}),
        )
        .await,
    )
    .await;
    let manual_uri = format!("/v1/milestones/{}", manual["id"].as_str().unwrap());
    let completed = response_json(
        call(
            "POST",
            format!("{manual_uri}/complete"),
            fixture.member.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(completed["completed"], true);
    let reopened = response_json(
        call(
            "POST",
            format!("{manual_uri}/reopen"),
            fixture.member.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(reopened["completed"], false);

    // Removing a milestone leaves its tasks in the project.
    assert_eq!(
        call(
            "DELETE",
            format!("/v1/milestones/{milestone_id}"),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        204
    );
    let kept = response_json(
        call(
            "GET",
            format!("/v1/tasks/{}", done["id"].as_str().unwrap()),
            fixture.member.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(kept["project_id"], project_id.as_str());
    assert!(kept["milestone_id"].is_null());
}
