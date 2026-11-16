//! Due reminders (V0.7.15): the person a task is assigned to is told once when it falls due and once when it is overdue, a done
//! task is left alone, and a second pass, or a moved due date, behaves as the rules say.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn an_assignee_is_reminded_once_per_due_date_and_completed_work_is_left_alone() {
    let fixture = knowledge_fixture("task-reminders").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    // The first account of the fixture administers the server and runs the maintenance pass.
    let admin_session = response_json(
        app.clone()
            .oneshot(public_post(
                "/v1/auth/login",
                json!({"email": "task-reminders-root@example.com", "password": "not-a-real-password"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let admin = format!("Bearer {}", admin_session["access_token"].as_str().unwrap());
    let run_pass = || {
        let call = &call;
        let admin = admin.clone();
        async move {
            let ran = call("POST", "/v1/system/maintenance/run".to_owned(), admin, json!({})).await;
            assert_eq!(ran.status(), 200);
        }
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
            .unwrap()
    };
    let (member_id, owner_id) = (
        id_of("task-reminders-member@example.com"),
        id_of("task-reminders-owner@example.com"),
    );
    let now = chrono::Utc::now();
    let day = |offset: i64| (now + chrono::Duration::days(offset)).format("%Y-%m-%d").to_string();
    let tasks = format!("/v1/workspaces/{workspace}/tasks");
    let make = |title: &'static str, status: &'static str, assignee: Option<&String>, due: String| {
        call(
            "POST",
            tasks.clone(),
            fixture.owner.clone(),
            json!({"title": title, "status": status, "priority": "medium", "assignee_user_id": assignee, "due_at": due}),
        )
    };
    let overdue = response_json(make("Overdue", "open", Some(&member_id), day(-1)).await).await;
    let due_today = response_json(make("Due today", "in_progress", Some(&member_id), day(0)).await).await;
    make("Later", "open", Some(&member_id), day(7)).await;
    make("Finished", "done", Some(&member_id), day(-3)).await;
    make("Nobody's", "open", None, day(-2)).await;
    assert_eq!(notices(fixture.member.clone(), "task_overdue").await, 0);

    // One pass: the overdue task and the one due today are reminded; a later, a done and an unassigned task are not.
    run_pass().await;
    assert_eq!(notices(fixture.member.clone(), "task_overdue").await, 1);
    assert_eq!(notices(fixture.member.clone(), "task_due").await, 1);
    let inbox = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/notifications", &fixture.member))
            .await
            .unwrap(),
    )
    .await;
    let overdue_notice = inbox
        .as_array()
        .unwrap()
        .iter()
        .find(|notice| notice["notification_type"] == "task_overdue")
        .unwrap();
    assert!(overdue_notice["body"].as_str().unwrap().contains("Overdue"));
    assert!(overdue_notice["body"].as_str().unwrap().contains(&day(-1)));
    assert!(overdue_notice["href"].as_str().unwrap().ends_with("/tasks/T-1"));

    // Again, and again: nothing is sent twice.
    run_pass().await;
    run_pass().await;
    assert_eq!(notices(fixture.member.clone(), "task_overdue").await, 1);
    assert_eq!(notices(fixture.member.clone(), "task_due").await, 1);
    assert_eq!(notices(fixture.owner.clone(), "task_overdue").await, 0);

    // Moving the due date allows one new reminder for the new date.
    call(
        "PUT",
        format!("/v1/tasks/{}", overdue["id"].as_str().unwrap()),
        fixture.owner.clone(),
        json!({"title": "Overdue", "status": "open", "priority": "medium", "assignee_user_id": member_id, "due_at": day(-2)}),
    )
    .await;
    run_pass().await;
    run_pass().await;
    assert_eq!(notices(fixture.member.clone(), "task_overdue").await, 2);

    // A task that is completed is not reminded about; a new assignee is reminded as a new person.
    call(
        "PUT",
        format!("/v1/tasks/{}", due_today["id"].as_str().unwrap()),
        fixture.owner.clone(),
        json!({"title": "Due today", "status": "open", "priority": "medium", "assignee_user_id": owner_id, "due_at": day(0)}),
    )
    .await;
    run_pass().await;
    assert_eq!(notices(fixture.owner.clone(), "task_due").await, 1);
    assert_eq!(notices(fixture.member.clone(), "task_due").await, 1);
    let finished = response_json(make("Soon done", "open", Some(&member_id), day(-1)).await).await;
    call(
        "PUT",
        format!("/v1/tasks/{}", finished["id"].as_str().unwrap()),
        fixture.owner.clone(),
        json!({"title": "Soon done", "status": "done", "priority": "medium", "assignee_user_id": member_id, "due_at": day(-1)}),
    )
    .await;
    run_pass().await;
    assert_eq!(
        notices(fixture.member.clone(), "task_overdue").await,
        2,
        "completed work is excluded"
    );

    // The person's own choice is respected: with the channel off, nothing is put in the inbox.
    assert_eq!(
        call(
            "PUT",
            "/v1/profile/notification-preferences".to_owned(),
            fixture.member.clone(),
            json!({"kind": "task_due", "channel": "none"})
        )
        .await
        .status(),
        200
    );
    make("Quiet", "open", Some(&member_id), day(-1)).await;
    run_pass().await;
    assert_eq!(notices(fixture.member.clone(), "task_overdue").await, 2);
}
