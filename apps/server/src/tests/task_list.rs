//! The task list (V0.7.12): a workspace's tasks are read a page at a time through one query model, and saved views store only
//! filters, so each reader sees what their own current rights allow.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn the_task_list_pages_and_filters_and_saved_views_are_evaluated_for_the_reader() {
    let fixture = knowledge_fixture("task-list").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let get = |uri: String, who: String| {
        let call = &call;
        async move { response_json(call("GET", uri, who, Value::Null).await).await }
    };
    let members = get(format!("/v1/workspaces/{workspace}/members"), fixture.owner.clone()).await;
    let id_of = |email: &str| {
        members
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["user"]["email"] == email)
            .map(|entry| entry["user"]["id"].as_str().unwrap().to_owned())
            .unwrap()
    };
    let member_id = id_of("task-list-member@example.com");
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
    let tasks = format!("/v1/workspaces/{workspace}/tasks");
    // Seven tasks with different properties.
    let specs = [
        (
            "Queue backfill",
            "in_progress",
            "high",
            Some(&member_id),
            Some("2020-01-10"),
            true,
        ),
        (
            "Queue cutover",
            "open",
            "urgent",
            Some(&member_id),
            Some("2999-01-01"),
            true,
        ),
        ("Write the runbook", "open", "medium", None, None, false),
        ("Fix the crash", "blocked", "high", Some(&member_id), None, false),
        ("Close the loop", "done", "low", None, Some("2020-02-02"), false),
        ("Tidy the docs", "open", "low", None, None, false),
        ("Rename the topic", "open", "medium", Some(&member_id), None, false),
    ];
    let mut made = Vec::new();
    for (title, status, priority, assignee, due, in_project) in specs {
        let mut body = json!({"title": title, "status": status, "priority": priority,
                              "assignee_user_id": assignee, "due_at": due});
        if in_project {
            body["project_id"] = json!(project_id);
            body["milestone_id"] = milestone["id"].clone();
        }
        let task = response_json(call("POST", tasks.clone(), fixture.member.clone(), body).await).await;
        made.push(task);
    }
    call(
        "PUT",
        format!("/v1/tasks/{}/labels", made[3]["id"].as_str().unwrap()),
        fixture.member.clone(),
        json!({"label_ids": [label["id"]]}),
    )
    .await;
    let list = format!("/v1/workspaces/{workspace}/task-list");
    let titles = |page: &Value| {
        page["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|task| task["title"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };

    // Pages: a small page says how many match and where the next starts; the pages together hold every task once.
    let first = get(format!("{list}?limit=3&sort=number"), fixture.guest.clone()).await;
    assert_eq!(
        (first["total"].as_i64(), first["next_offset"].as_i64()),
        (Some(7), Some(3))
    );
    let second = get(format!("{list}?limit=3&offset=3&sort=number"), fixture.guest.clone()).await;
    let third = get(format!("{list}?limit=3&offset=6&sort=number"), fixture.guest.clone()).await;
    assert!(third["next_offset"].is_null());
    let mut all = titles(&first);
    all.extend(titles(&second));
    all.extend(titles(&third));
    assert_eq!(
        all,
        [
            "Queue backfill",
            "Queue cutover",
            "Write the runbook",
            "Fix the crash",
            "Close the loop",
            "Tidy the docs",
            "Rename the topic"
        ]
    );

    // Each filter, alone.
    let only = |query: String| {
        let list = list.clone();
        let reader = fixture.guest.clone();
        let get = &get;
        async move { titles(&get(format!("{list}?{query}&sort=number"), reader).await) }
    };
    assert_eq!(
        only(format!("project_id={project_id}")).await,
        ["Queue backfill", "Queue cutover"]
    );
    assert_eq!(
        only(format!("milestone_id={}", milestone["id"].as_str().unwrap()))
            .await
            .len(),
        2
    );
    assert_eq!(only("project_id=none".into()).await.len(), 5);
    assert_eq!(only(format!("assignee={member_id}")).await.len(), 4);
    assert_eq!(only("assignee=none".into()).await.len(), 3);
    assert_eq!(
        only("priority=high,urgent".into()).await,
        ["Queue backfill", "Queue cutover", "Fix the crash"]
    );
    assert_eq!(only("status=open".into()).await.len(), 4);
    assert_eq!(
        only("status=blocked,done".into()).await,
        ["Fix the crash", "Close the loop"]
    );
    assert_eq!(
        only(format!("label={}", label["id"].as_str().unwrap())).await,
        ["Fix the crash"]
    );
    assert_eq!(only("due_before=2020-01-31".into()).await, ["Queue backfill"]);
    assert_eq!(
        only("due_after=2020-02-01&due_before=2999-12-31".into()).await,
        ["Queue cutover", "Close the loop"]
    );
    assert_eq!(
        only("overdue=true".into()).await,
        ["Queue backfill"],
        "a done task is never overdue"
    );
    assert_eq!(only("q=QUEUE".into()).await, ["Queue backfill", "Queue cutover"]);
    assert_eq!(
        only("q=100%25".into()).await.len(),
        0,
        "a percent sign is text, not a wildcard"
    );
    // Together they narrow.
    assert_eq!(
        only(format!("assignee={member_id}&priority=high&status=in_progress")).await,
        ["Queue backfill"]
    );

    // What cannot be asked is refused.
    for query in [
        "priority=huge",
        "status=later",
        "limit=0",
        "limit=500",
        "offset=-1",
        "due_before=soon",
        "sort=random",
    ] {
        assert_eq!(
            call("GET", format!("{list}?{query}"), fixture.guest.clone(), Value::Null)
                .await
                .status(),
            400,
            "{query}"
        );
    }
    let (stranger, _) = register_account(&app, "task-list-stranger@example.com").await;
    assert_eq!(
        call("GET", list.clone(), stranger.clone(), Value::Null).await.status(),
        403
    );

    // A saved view stores only filters; `me` means whoever reads it, and it is private until shared.
    let views = format!("/v1/workspaces/{workspace}/task-views");
    let saved = call(
        "POST",
        views.clone(),
        fixture.member.clone(),
        json!({"name": "My open work", "query": {"assignee": "me", "status": ["open", "in_progress"]}}),
    )
    .await;
    assert_eq!(saved.status(), 201);
    let saved = response_json(saved).await;
    let view_id = saved["id"].as_str().unwrap().to_owned();
    assert_eq!(saved["shared"], false);
    assert_eq!(
        call(
            "POST",
            views.clone(),
            fixture.member.clone(),
            json!({"name": " ", "query": {}})
        )
        .await
        .status(),
        400
    );
    assert_eq!(
        call(
            "POST",
            views.clone(),
            fixture.member.clone(),
            json!({"name": "Bad", "query": {"priority": ["huge"]}})
        )
        .await
        .status(),
        400
    );
    let mine = titles(&get(format!("{list}?view={view_id}&sort=number"), fixture.member.clone()).await);
    assert_eq!(mine, ["Queue backfill", "Queue cutover", "Rename the topic"]);
    assert_eq!(
        get(views.clone(), fixture.owner.clone())
            .await
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        call(
            "GET",
            format!("{list}?view={view_id}"),
            fixture.owner.clone(),
            Value::Null
        )
        .await
        .status(),
        404,
        "someone else's private view is not there"
    );

    // Shared, the owner can use it and `me` is then the owner; an explicit filter replaces the view's own.
    assert_eq!(
        call(
            "PUT",
            format!("/v1/task-views/{view_id}/sharing"),
            fixture.guest.clone(),
            json!({"shared": true})
        )
        .await
        .status(),
        404,
        "only its owner shares it"
    );
    assert_eq!(
        call(
            "PUT",
            format!("/v1/task-views/{view_id}/sharing"),
            fixture.member.clone(),
            json!({"shared": true})
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        get(views.clone(), fixture.owner.clone())
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let for_owner = get(format!("{list}?view={view_id}"), fixture.owner.clone()).await;
    assert_eq!(
        for_owner["total"], 0,
        "`me` is the reader: the owner is assigned nothing"
    );
    let overridden = titles(
        &get(
            format!("{list}?view={view_id}&status=blocked&sort=number"),
            fixture.member.clone(),
        )
        .await,
    );
    assert_eq!(overridden, ["Fix the crash"]);
    assert_eq!(
        call("GET", format!("{list}?view={view_id}"), stranger, Value::Null)
            .await
            .status(),
        403,
        "a view never carries rights"
    );
    assert_eq!(
        call(
            "PUT",
            format!("/v1/task-views/{view_id}"),
            fixture.guest.clone(),
            json!({"name": "Mine now", "query": {}})
        )
        .await
        .status(),
        403,
        "a shared view is not theirs to change"
    );

    // A reader keeps their own views; the owner of a view removes it.
    assert_eq!(
        call(
            "POST",
            views,
            fixture.guest.clone(),
            json!({"name": "Everything blocked", "query": {"status": ["blocked"]}})
        )
        .await
        .status(),
        201
    );
    assert_eq!(
        call(
            "DELETE",
            format!("/v1/task-views/{view_id}"),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        204
    );
    assert_eq!(
        call(
            "GET",
            format!("{list}?view={view_id}"),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        404
    );
}
