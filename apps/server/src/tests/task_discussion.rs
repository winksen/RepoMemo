//! Task discussion (V0.7.4): a comment stays with its task, a mention notifies a person once, a person follows a task by asking
//! (and the creator and assignee follow it by default), and muting stops the optional notices but never an assignment.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_discussion_stays_with_its_task_and_muting_keeps_assignment_notices() {
    let fixture = knowledge_fixture("task-discussion").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let notices = |who: String, kind: &'static str| {
        let app = app.clone();
        async move {
            response_json(
                app.oneshot(auth_request("GET", "/v1/notifications", &who))
                    .await
                    .unwrap(),
            )
            .await
            .as_array()
            .unwrap()
            .iter()
            .filter(|notice| notice["notification_type"] == kind)
            .count()
        }
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
    let id_of = |email: &str| {
        members
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["user"]["email"] == email)
            .map(|entry| entry["user"]["id"].as_str().unwrap().to_owned())
            .unwrap_or_else(|| panic!("{email} is a member"))
    };
    let member_id = id_of("task-discussion-member@example.com");

    // The owner makes a task for the member; the assignment notice reaches them.
    let task = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace}/tasks"),
            fixture.owner.clone(),
            json!({"title": "Move the worker", "status": "open", "priority": "high", "assignee_user_id": member_id}),
        )
        .await,
    )
    .await;
    let task_id = task["id"].as_str().unwrap().to_owned();
    assert_eq!(notices(fixture.member.clone(), "task_assigned").await, 1);
    let comments = format!("/v1/tasks/{task_id}/comments");
    let watch = format!("/v1/tasks/{task_id}/watch");

    // A reader follows the task by asking, but cannot comment; a stranger sees nothing.
    let watching = call("PUT", watch.clone(), fixture.guest.clone(), json!({"mode": "watching"})).await;
    assert_eq!(watching.status(), 200);
    assert_eq!(
        call(
            "PUT",
            watch.clone(),
            fixture.guest.clone(),
            json!({"mode": "sometimes"})
        )
        .await
        .status(),
        400
    );
    assert_eq!(
        call(
            "POST",
            comments.clone(),
            fixture.guest.clone(),
            json!({"body": "Hello"})
        )
        .await
        .status(),
        403
    );
    let (stranger, _) = register_account(&app, "task-discussion-stranger@example.com").await;
    assert_eq!(
        call("GET", comments.clone(), stranger.clone(), Value::Null)
            .await
            .status(),
        403
    );
    assert_eq!(
        call("PUT", watch.clone(), stranger, json!({"mode": "watching"}))
            .await
            .status(),
        403
    );

    // What is not a comment is refused.
    for body in [json!({"body": "   "}), json!({"body": "x".repeat(5_001)})] {
        assert_eq!(
            call("POST", comments.clone(), fixture.owner.clone(), body)
                .await
                .status(),
            400
        );
    }

    // A comment that mentions the assignee tells them once, as a mention, and not a second time as a follower.
    let first = call(
        "POST",
        comments.clone(),
        fixture.owner.clone(),
        json!({"body": "Can you take the queue part? @task-discussion-member@example.com"}),
    )
    .await;
    assert_eq!(first.status(), 201);
    let first = response_json(first).await;
    assert_eq!(first["task_id"], task_id.as_str());
    assert_eq!(notices(fixture.member.clone(), "task_mention").await, 1);
    assert_eq!(notices(fixture.member.clone(), "task_comment").await, 0);
    // The watching reader is told of the comment; the author is not told of their own.
    assert_eq!(notices(fixture.guest.clone(), "task_comment").await, 1);
    assert_eq!(notices(fixture.owner.clone(), "task_comment").await, 0);

    // A comment with no mention reaches the assignee as a follower.
    call(
        "POST",
        comments.clone(),
        fixture.owner.clone(),
        json!({"body": "Thanks, no rush."}),
    )
    .await;
    assert_eq!(notices(fixture.member.clone(), "task_comment").await, 1);
    assert_eq!(notices(fixture.member.clone(), "task_mention").await, 1);

    // Editing: only the author, and only newly added people are told.
    let comment_uri = format!("/v1/task-comments/{}", first["id"].as_str().unwrap());
    let guest_mention =
        "Can you take the queue part? @task-discussion-member@example.com and @task-discussion-guest@example.com";
    assert_eq!(
        call(
            "PUT",
            comment_uri.clone(),
            fixture.member.clone(),
            json!({"body": "Rewritten"})
        )
        .await
        .status(),
        403
    );
    for _ in 0..2 {
        assert_eq!(
            call(
                "PUT",
                comment_uri.clone(),
                fixture.owner.clone(),
                json!({"body": guest_mention})
            )
            .await
            .status(),
            200
        );
    }
    assert_eq!(notices(fixture.guest.clone(), "task_mention").await, 1);
    assert_eq!(notices(fixture.member.clone(), "task_mention").await, 1);

    // Muting stops the optional notice, and the assignment notice still arrives.
    assert_eq!(
        call("PUT", watch.clone(), fixture.member.clone(), json!({"mode": "muted"}))
            .await
            .status(),
        200
    );
    call(
        "POST",
        comments.clone(),
        fixture.owner.clone(),
        json!({"body": "One more thing."}),
    )
    .await;
    assert_eq!(
        notices(fixture.member.clone(), "task_comment").await,
        1,
        "muted: nothing new"
    );
    let task_uri = format!("/v1/tasks/{task_id}");
    for assignee in [Value::Null, json!(member_id)] {
        call(
            "PUT",
            task_uri.clone(),
            fixture.owner.clone(),
            json!({"title": "Move the worker", "status": "open", "priority": "high", "assignee_user_id": assignee}),
        )
        .await;
    }
    assert_eq!(
        notices(fixture.member.clone(), "task_assigned").await,
        2,
        "assignment is never muted"
    );

    // The detail shows the reader's own state and the count; a comment is removed by its author or an administrator.
    let detail = response_json(
        call(
            "GET",
            format!("/v1/workspaces/{workspace}/tasks/T-1"),
            fixture.member.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(detail["watch"], "muted");
    assert_eq!(detail["comment_count"], 3);
    assert_eq!(
        call("DELETE", comment_uri.clone(), fixture.member.clone(), Value::Null)
            .await
            .status(),
        403
    );
    assert_eq!(
        call("DELETE", comment_uri, fixture.owner.clone(), Value::Null)
            .await
            .status(),
        204
    );
    let remaining = response_json(call("GET", comments, fixture.guest.clone(), Value::Null).await).await;
    assert_eq!(remaining.as_array().unwrap().len(), 2);

    // The discussion goes with its task.
    assert_eq!(
        call("DELETE", task_uri, fixture.owner.clone(), Value::Null)
            .await
            .status(),
        204
    );
    assert_eq!(
        call(
            "GET",
            format!("/v1/tasks/{task_id}/comments"),
            fixture.owner.clone(),
            Value::Null
        )
        .await
        .status(),
        404
    );
}
