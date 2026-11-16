//! Task context (V0.7.5): a task cites several pieces of evidence and memory cards, the old single file link is kept as one of
//! them, a deleted target shows as unavailable, and a task can cite the decision that created it.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_task_cites_several_sources_and_keeps_its_old_file_link() {
    let fixture = knowledge_fixture("task-references").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let tasks = format!("/v1/workspaces/{workspace}/tasks");

    // The old single link keeps working and is one of the task's references.
    let task = response_json(
        call(
            "POST",
            tasks.clone(),
            fixture.member.clone(),
            json!({"title": "Check the deploy steps", "status": "open", "priority": "medium", "artifact_id": fixture.current_id}),
        )
        .await,
    )
    .await;
    let task_id = task["id"].as_str().unwrap().to_owned();
    assert_eq!(task["artifact_id"], fixture.current_id.as_str());
    let references = format!("/v1/tasks/{task_id}/references");
    let listed = response_json(call("GET", references.clone(), fixture.guest.clone(), Value::Null).await).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["kind"], "evidence");
    assert_eq!(listed[0]["title"], "Deploy checklist");

    // More can be added: another file and a decision memory, with a note. Only titles come back.
    let card = call(
        "POST",
        references.clone(),
        fixture.member.clone(),
        json!({"kind": "memory_card", "target_id": fixture.card_on_stale_id, "note": "Why we do this."}),
    )
    .await;
    assert_eq!(card.status(), 201);
    let card = response_json(card).await;
    assert_eq!(card["title"], "Refund window");
    assert!(card.get("body_markdown").is_none());
    assert_eq!(
        call(
            "POST",
            references.clone(),
            fixture.member.clone(),
            json!({"kind": "evidence", "target_id": fixture.stale_id})
        )
        .await
        .status(),
        201
    );

    // What cannot be cited is refused: the same item twice (the old link included), an unknown or foreign target, a bad kind,
    // a reader.
    for (body, status) in [
        (json!({"kind": "evidence", "target_id": fixture.current_id}), 409),
        (json!({"kind": "evidence", "target_id": "not-a-file"}), 404),
        (json!({"kind": "note", "target_id": "x"}), 400),
        (
            json!({"kind": "evidence", "target_id": fixture.stale_id, "note": "x".repeat(501)}),
            400,
        ),
    ] {
        assert_eq!(
            call("POST", references.clone(), fixture.member.clone(), body)
                .await
                .status(),
            status
        );
    }
    let other_workspace = create_workspace(&app, &fixture.owner).await;
    let foreign = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{other_workspace}/artifacts/text"),
            fixture.owner.clone(),
            json!({"title": "Elsewhere", "content": "Not in this workspace."}),
        )
        .await,
    )
    .await;
    assert_eq!(
        call(
            "POST",
            references.clone(),
            fixture.member.clone(),
            json!({"kind": "evidence", "target_id": foreign["id"]})
        )
        .await
        .status(),
        404
    );
    assert_eq!(
        call(
            "POST",
            references.clone(),
            fixture.guest.clone(),
            json!({"kind": "evidence", "target_id": fixture.stale_id})
        )
        .await
        .status(),
        403
    );

    // The task page lists them all.
    let detail = response_json(call("GET", format!("{tasks}/T-1"), fixture.guest.clone(), Value::Null).await).await;
    assert_eq!(detail["references"].as_array().unwrap().len(), 3);

    // A task can cite the decision that created it, and a bad citation leaves no task behind.
    let created = call(
        "POST",
        tasks.clone(),
        fixture.owner.clone(),
        json!({"title": "Act on the decision", "status": "open", "priority": "high",
               "references": [{"kind": "memory_card", "target_id": fixture.card_on_stale_id, "note": "Created from this."},
                              {"kind": "memory_card", "target_id": fixture.card_on_stale_id}]}),
    )
    .await;
    assert_eq!(created.status(), 201);
    let created = response_json(created).await;
    let cited = response_json(
        call(
            "GET",
            format!("/v1/tasks/{}/references", created["id"].as_str().unwrap()),
            fixture.owner.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(
        cited.as_array().unwrap().len(),
        1,
        "the same item twice is one citation"
    );
    assert_eq!(cited[0]["note"], "Created from this.");
    let before = response_json(call("GET", tasks.clone(), fixture.owner.clone(), Value::Null).await)
        .await
        .as_array()
        .unwrap()
        .len();
    assert_eq!(
        call(
            "POST",
            tasks.clone(),
            fixture.owner.clone(),
            json!({"title": "Never made", "status": "open", "priority": "low",
                   "references": [{"kind": "evidence", "target_id": "not-a-file"}]})
        )
        .await
        .status(),
        404
    );
    let after = response_json(call("GET", tasks, fixture.owner.clone(), Value::Null).await)
        .await
        .as_array()
        .unwrap()
        .len();
    assert_eq!(before, after);

    // A deleted file stays cited and shows as unavailable; a citation is removed by anyone who can edit.
    let removed = call(
        "POST",
        format!("/v1/workspaces/{workspace}/artifacts/bulk"),
        fixture.owner.clone(),
        json!({"action": "delete", "artifact_ids": [fixture.stale_id]}),
    )
    .await;
    assert_eq!(removed.status(), 200);
    let listed = response_json(call("GET", references.clone(), fixture.member.clone(), Value::Null).await).await;
    let gone = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|reference| reference["target_id"] == fixture.stale_id.as_str())
        .unwrap();
    assert_eq!(gone["available"], false);
    assert!(gone["title"].is_null());
    assert_eq!(
        call(
            "DELETE",
            format!("{references}/{}", gone["id"].as_str().unwrap()),
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
            format!("{references}/{}", gone["id"].as_str().unwrap()),
            fixture.member.clone(),
            Value::Null
        )
        .await
        .status(),
        404
    );
}
