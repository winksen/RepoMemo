//! The task board (V0.7.13): the board is the task list's filtered work grouped by workflow state, so both show the same tasks; a
//! move between columns is a checked save that can be refused with a reason, and the client puts the card back.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn the_board_and_the_list_show_the_same_work_and_a_move_is_a_checked_save() {
    let fixture = knowledge_fixture("task-board").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let tasks = format!("/v1/workspaces/{workspace}/tasks");
    let review = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace}/workflow/states"),
            fixture.owner.clone(),
            json!({"name": "Review", "category": "in_progress"}),
        )
        .await,
    )
    .await;
    let review_id = review["id"].as_str().unwrap().to_owned();
    let mut made = Vec::new();
    for (title, status, priority) in [
        ("One", "open", "high"),
        ("Two", "open", "low"),
        ("Three", "in_progress", "urgent"),
        ("Four", "blocked", "medium"),
        ("Five", "done", "high"),
        ("Six", "open", "low"),
    ] {
        made.push(
            response_json(
                call(
                    "POST",
                    tasks.clone(),
                    fixture.member.clone(),
                    json!({"title": title, "status": status, "priority": priority}),
                )
                .await,
            )
            .await,
        );
    }
    // One task sits in the named state.
    call(
        "PUT",
        format!("/v1/tasks/{}", made[2]["id"].as_str().unwrap()),
        fixture.member.clone(),
        json!({"title": "Three", "state_id": review_id, "priority": "urgent"}),
    )
    .await;
    let board = format!("/v1/workspaces/{workspace}/task-board");
    let list = format!("/v1/workspaces/{workspace}/task-list");
    let read = |uri: String, who: String| {
        let call = &call;
        async move { response_json(call("GET", uri, who, Value::Null).await).await }
    };

    // One column for each state; every task is in exactly one, and its status is its column's category.
    let all = read(board.clone(), fixture.guest.clone()).await;
    let columns = all["columns"].as_array().unwrap();
    assert_eq!(
        columns
            .iter()
            .map(|column| column["state"]["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Open", "In progress", "Blocked", "Done", "Review"]
    );
    assert_eq!(all["total"], 6);
    for column in columns {
        for task in column["tasks"].as_array().unwrap() {
            assert_eq!(task["status"], column["state"]["category"]);
            assert_eq!(task["state_id"], column["state"]["id"]);
        }
        assert_eq!(
            column["total"].as_i64().unwrap(),
            column["tasks"].as_array().unwrap().len() as i64
        );
    }

    // The board and the list show the same filtered work.
    for query in [
        "priority=high,urgent",
        "status=open",
        "q=t",
        "overdue=false&assignee=none",
    ] {
        let on_board = read(format!("{board}?{query}"), fixture.guest.clone()).await;
        let mut board_ids: Vec<String> = on_board["columns"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|column| column["tasks"].as_array().unwrap().clone())
            .map(|task| task["id"].as_str().unwrap().to_owned())
            .collect();
        let on_list = read(format!("{list}?{query}"), fixture.guest.clone()).await;
        let mut list_ids: Vec<String> = on_list["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|task| task["id"].as_str().unwrap().to_owned())
            .collect();
        board_ids.sort();
        list_ids.sort();
        assert_eq!(board_ids, list_ids, "{query}");
        assert_eq!(on_board["total"], on_list["total"], "{query}");
    }

    // A small column says there is more, and the list's `state` filter reads the rest.
    let small = read(format!("{board}?column_limit=1"), fixture.guest.clone()).await;
    let open = &small["columns"][0];
    assert_eq!(
        (
            open["tasks"].as_array().unwrap().len(),
            open["total"].as_i64(),
            open["next_offset"].as_i64()
        ),
        (1, Some(3), Some(1))
    );
    let rest = read(format!("{list}?state=open&offset=1&sort=number"), fixture.guest.clone()).await;
    assert_eq!(rest["tasks"].as_array().unwrap().len(), 2);
    assert_eq!(
        read(format!("{list}?state={review_id}"), fixture.guest.clone()).await["total"],
        1
    );
    assert_eq!(
        call(
            "GET",
            format!("{board}?column_limit=0"),
            fixture.guest.clone(),
            Value::Null
        )
        .await
        .status(),
        400
    );

    // Moving a card is a checked save: it takes the column's category and adds a version.
    let one_uri = format!("/v1/tasks/{}/move", made[0]["id"].as_str().unwrap());
    let moved = call(
        "POST",
        one_uri.clone(),
        fixture.member.clone(),
        json!({"state_id": "blocked", "expected_version": 1}),
    )
    .await;
    assert_eq!(moved.status(), 200);
    let moved = response_json(moved).await;
    assert_eq!(
        (moved["status"].as_str(), moved["version"].as_i64()),
        (Some("blocked"), Some(2))
    );
    assert_eq!(moved["title"], "One", "a move changes nothing but the column");
    assert_eq!(moved["priority"], "high");
    // A move from a stale board is refused with the task as it is now, so the card can be put back.
    let stale = call(
        "POST",
        one_uri.clone(),
        fixture.member.clone(),
        json!({"state_id": "done", "expected_version": 1}),
    )
    .await;
    assert_eq!(stale.status(), 409);
    let stale = response_json(stale).await;
    assert_eq!(
        (
            stale["current"]["status"].as_str(),
            stale["attempted"]["status"].as_str()
        ),
        (Some("blocked"), Some("done"))
    );
    // A named column works, and what cannot be done is refused: an unknown column, a reader.
    let into_review = response_json(
        call(
            "POST",
            one_uri.clone(),
            fixture.member.clone(),
            json!({"state_id": review_id}),
        )
        .await,
    )
    .await;
    assert_eq!(
        (into_review["status"].as_str(), into_review["state_name"].as_str()),
        (Some("in_progress"), Some("Review"))
    );
    assert_eq!(
        call(
            "POST",
            one_uri.clone(),
            fixture.member.clone(),
            json!({"state_id": "nowhere"})
        )
        .await
        .status(),
        400
    );
    assert_eq!(
        call("POST", one_uri, fixture.guest.clone(), json!({"state_id": "done"}))
            .await
            .status(),
        403
    );
    let after = read(board, fixture.guest.clone()).await;
    let review_column = after["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|column| column["state"]["name"] == "Review")
        .unwrap();
    assert_eq!(review_column["total"], 2);
    assert_eq!(after["total"], 6, "a move loses and duplicates nothing");
}
