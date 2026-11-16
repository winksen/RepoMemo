//! Recurring work (V0.7.16): a schedule makes its tasks once for each occurrence date, missed occurrences follow the stated
//! policy, a paused schedule makes nothing and makes nothing up, and a stopped one keeps its history.

use super::knowledge_fixture::knowledge_fixture;
use super::*;
use chrono::Datelike;

#[tokio::test]
async fn a_schedule_makes_each_occurrence_once_and_stopping_keeps_the_history() {
    let fixture = knowledge_fixture("recurring").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let admin_session = response_json(
        app.clone()
            .oneshot(public_post(
                "/v1/auth/login",
                json!({"email": "recurring-root@example.com", "password": "not-a-real-password"}),
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
            assert_eq!(
                call("POST", "/v1/system/maintenance/run".to_owned(), admin, json!({}))
                    .await
                    .status(),
                200
            );
        }
    };
    let titles = || {
        let call = &call;
        let who = fixture.owner.clone();
        let uri = format!("/v1/workspaces/{workspace}/tasks");
        async move {
            let tasks = response_json(call("GET", uri, who, Value::Null).await).await;
            let mut titles: Vec<String> = tasks
                .as_array()
                .unwrap()
                .iter()
                .map(|task| task["title"].as_str().unwrap().to_owned())
                .collect();
            titles.sort();
            titles
        }
    };
    let now = chrono::Utc::now();
    let day = |offset: i64| (now + chrono::Duration::days(offset)).format("%Y-%m-%d").to_string();
    let weekday_of = |offset: i64| (now + chrono::Duration::days(offset)).weekday().num_days_from_monday();
    let recurring = format!("/v1/workspaces/{workspace}/recurring-tasks");
    let make = |body: Value| call("POST", recurring.clone(), fixture.member.clone(), body);

    // What is not a schedule is refused with a reason; a reader cannot make one.
    for body in [
        json!({"title": "Bad", "schedule": {"kind": "hourly", "interval": 1, "start_date": day(0)}}),
        json!({"title": "Bad", "schedule": {"kind": "weekly", "interval": 1, "start_date": day(0)}}),
        json!({"title": "Bad", "schedule": {"kind": "monthly", "interval": 1, "day_of_month": 31, "start_date": day(0)}}),
        json!({"title": "Bad", "schedule": {"kind": "daily", "interval": 0, "start_date": day(0)}}),
        json!({"title": "Bad", "schedule": {"kind": "daily", "interval": 1, "start_date": "soon"}}),
        json!({"title": " ", "schedule": {"kind": "daily", "interval": 1, "start_date": day(0)}}),
        json!({"title": "Bad", "missed_policy": "all", "schedule": {"kind": "daily", "interval": 1, "start_date": day(0)}}),
        json!({"title": "Bad", "due_after_days": 400, "schedule": {"kind": "daily", "interval": 1, "start_date": day(0)}}),
        json!({"title": "Bad", "assignee_user_id": "nobody", "schedule": {"kind": "daily", "interval": 1, "start_date": day(0)}}),
    ] {
        assert_eq!(make(body).await.status(), 400);
    }
    assert_eq!(
        call(
            "POST",
            recurring.clone(),
            fixture.guest.clone(),
            json!({"title": "Reader's", "schedule": {"kind": "daily", "interval": 1, "start_date": day(0)}})
        )
        .await
        .status(),
        403
    );

    // A daily schedule that started three days ago, with the default policy, makes only the newest occurrence, once.
    let daily = make(
        json!({"title": "Stand-up notes", "priority": "low", "due_after_days": 2,
                            "schedule": {"kind": "daily", "interval": 1, "start_date": day(-3)}}),
    )
    .await;
    assert_eq!(daily.status(), 201);
    let daily = response_json(daily).await;
    assert_eq!(daily["status"], "active");
    assert_eq!(
        daily["next_occurrences"][0],
        day(0).as_str(),
        "the schedule shows its next dates before it runs"
    );
    // The same schedule, on the weekday of yesterday, with each policy: the newest missed occurrence was yesterday's.
    let schedule = json!({"kind": "weekly", "interval": 1, "weekday": weekday_of(-1), "start_date": day(-20)});
    let latest =
        response_json(make(json!({"title": "Weekly (latest)", "schedule": schedule, "missed_policy": "latest"})).await)
            .await;
    let skipping =
        response_json(make(json!({"title": "Weekly (skip)", "schedule": schedule, "missed_policy": "skip"})).await)
            .await;
    assert_eq!(titles().await.len(), 0);
    run_pass().await;
    assert_eq!(
        titles().await,
        ["Stand-up notes", "Weekly (latest)"],
        "skip makes nothing for a missed day"
    );
    // The task made is open, due after its occurrence date, and says where it came from.
    let tasks = response_json(
        call(
            "GET",
            format!("/v1/workspaces/{workspace}/tasks"),
            fixture.owner.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    let made = tasks
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["title"] == "Stand-up notes")
        .unwrap();
    assert_eq!(
        (made["status"].as_str(), made["priority"].as_str()),
        (Some("open"), Some("low"))
    );
    assert_eq!(made["due_at"], day(2).as_str());
    assert!(made["description"]
        .as_str()
        .unwrap()
        .contains(&format!("recurring task for {}", day(0))));

    // Again, and again: an occurrence is one task however often the pass runs.
    run_pass().await;
    run_pass().await;
    assert_eq!(titles().await.len(), 2);
    let detail = response_json(
        call(
            "GET",
            format!("/v1/recurring-tasks/{}", daily["id"].as_str().unwrap()),
            fixture.guest.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    let outcomes: Vec<(String, String)> = detail["occurrences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|occurrence| {
            (
                occurrence["occurrence_date"].as_str().unwrap().to_owned(),
                occurrence["outcome"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        outcomes,
        [
            (day(0), "created".to_owned()),
            (day(-1), "skipped".to_owned()),
            (day(-2), "skipped".to_owned()),
            (day(-3), "skipped".to_owned())
        ]
    );
    assert!(detail["occurrences"][0]["task_key"].is_string());
    assert_eq!(detail["recurring"]["occurrences_made"], 1);
    let skip_detail = response_json(
        call(
            "GET",
            format!("/v1/recurring-tasks/{}", skipping["id"].as_str().unwrap()),
            fixture.member.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(skip_detail["recurring"]["occurrences_made"], 0);
    assert_eq!(skip_detail["occurrences"][0]["outcome"], "skipped");
    let _ = latest;

    // Paused, nothing is made; resumed, only today's occurrence can be made, not the days that passed.
    let paused = response_json(
        make(json!({"title": "While away", "paused": true, "schedule": {"kind": "daily", "interval": 1, "start_date": day(-5)}})).await,
    )
    .await;
    assert_eq!(paused["status"], "paused");
    run_pass().await;
    assert_eq!(titles().await.len(), 2);
    let paused_uri = format!("/v1/recurring-tasks/{}", paused["id"].as_str().unwrap());
    assert_eq!(
        call(
            "POST",
            format!("{paused_uri}/resume"),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        200
    );
    run_pass().await;
    assert_eq!(titles().await, ["Stand-up notes", "Weekly (latest)", "While away"]);
    let resumed = response_json(call("GET", paused_uri.clone(), fixture.member.clone(), Value::Null).await).await;
    assert_eq!(
        resumed["occurrences"].as_array().unwrap().len(),
        1,
        "nothing is made up for the time it was paused"
    );

    // Stopping ends the schedule and keeps what it made; a stopped one is not edited or started again.
    assert_eq!(
        call(
            "POST",
            format!("{paused_uri}/stop"),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        200
    );
    run_pass().await;
    assert_eq!(titles().await.len(), 3);
    let stopped = response_json(call("GET", paused_uri.clone(), fixture.guest.clone(), Value::Null).await).await;
    assert_eq!(stopped["recurring"]["status"], "stopped");
    assert!(stopped["recurring"]["next_occurrences"].as_array().unwrap().is_empty());
    assert_eq!(stopped["recurring"]["occurrences_made"], 1);
    assert_eq!(
        call(
            "POST",
            format!("{paused_uri}/resume"),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        409
    );
    assert_eq!(
        call(
            "PUT",
            paused_uri,
            fixture.member.clone(),
            json!({"title": "Renamed", "schedule": {"kind": "daily", "interval": 1, "start_date": day(0)}})
        )
        .await
        .status(),
        409
    );
    // A change to an active schedule keeps what was already looked at: the past is not made again.
    let uri = format!("/v1/recurring-tasks/{}", daily["id"].as_str().unwrap());
    assert_eq!(
        call(
            "PUT",
            uri,
            fixture.member.clone(),
            json!({"title": "Stand-up notes", "schedule": {"kind": "daily", "interval": 1, "start_date": day(-30)}})
        )
        .await
        .status(),
        200
    );
    run_pass().await;
    assert_eq!(titles().await.len(), 3);
    assert_eq!(
        response_json(call("GET", recurring, fixture.guest.clone(), Value::Null).await)
            .await
            .as_array()
            .unwrap()
            .len(),
        4
    );
}
