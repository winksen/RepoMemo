//! Review ownership (V0.6.6): an accountable reviewer and a due date, overdue told apart from changed evidence, and a
//! reviewer who left surfaced instead of hidden.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn reviewers_and_due_dates_separate_overdue_from_changed_evidence_and_surface_departures() {
    let fixture = knowledge_fixture("review-owner").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let (helper, helper_account) = register_account(&app, "review-owner-helper@example.com").await;
    let helper_id = helper_account["user"]["id"].as_str().unwrap().to_owned();
    join(
        &app,
        &fixture.owner,
        "workspaces",
        &workspace,
        "review-owner-helper@example.com",
        "member",
    )
    .await;
    let guest_id = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace}/members"),
                &fixture.owner,
            ))
            .await
            .unwrap(),
    )
    .await
    .as_array()
    .unwrap()
    .iter()
    .find(|member| member["role"] == "viewer")
    .map(|member| member["user"]["id"].as_str().unwrap().to_owned())
    .expect("the fixture has a viewer");
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let assign = |who: String, kind: &str, id: &str, reviewer: Option<&str>, due: Option<&str>| {
        call(
            "PUT",
            format!("/v1/workspaces/{workspace}/review-assignment"),
            who,
            json!({"kind": kind, "id": id, "reviewer_user_id": reviewer, "next_review_at": due}),
        )
    };
    let queue = || {
        let app = app.clone();
        let who = fixture.owner.clone();
        let uri = format!("/v1/workspaces/{workspace}/review");
        async move { response_json(app.oneshot(auth_request("GET", &uri, &who)).await.unwrap()).await }
    };
    let item = |queue: &Value, id: &str| {
        queue["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == id)
            .cloned()
    };

    // A reviewer must be a member who can edit: the viewer is refused.
    let refused = assign(
        fixture.owner.clone(),
        "evidence",
        &fixture.current_id,
        Some(&guest_id),
        None,
    )
    .await;
    assert_eq!(refused.status(), 400);

    // A past date puts a file in the queue although nothing about it changed.
    let set = assign(
        fixture.owner.clone(),
        "evidence",
        &fixture.current_id,
        Some(&helper_id),
        Some("2000-01-01"),
    )
    .await;
    assert_eq!(set.status(), 200);
    let waiting = queue().await;
    let overdue = item(&waiting, &fixture.current_id).expect("the overdue file is listed");
    assert_eq!(overdue["cause"], "overdue");
    assert_eq!(overdue["overdue"], true);
    assert_eq!(overdue["reviewer"]["email"], "review-owner-helper@example.com");

    // A card whose evidence changed stays "changed evidence" even when its date also passed.
    let stale_card = item(&waiting, &fixture.card_on_stale_id).expect("the stale card is listed");
    assert_eq!(stale_card["cause"], "changed_evidence");
    assert_eq!(stale_card["overdue"], false);
    assert_eq!(
        assign(
            fixture.owner.clone(),
            "memory_card",
            &fixture.card_on_stale_id,
            None,
            Some("2000-01-02")
        )
        .await
        .status(),
        200
    );
    let both = item(&queue().await, &fixture.card_on_stale_id).unwrap();
    assert_eq!(both["cause"], "changed_evidence");
    assert_eq!(both["overdue"], true);
    assert_eq!(queue().await["overdue_count"], 2);

    // The reviewer sees it in My work.
    let work = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/profile/work", &helper))
            .await
            .unwrap(),
    )
    .await;
    assert!(work["reviews"]
        .as_array()
        .unwrap()
        .iter()
        .any(|review| review["artifact_id"] == fixture.current_id.as_str() && review["kind"] == "evidence"));

    // Taking a review away needs the reviewer or an administrator; the member cannot, the reviewer can hand it back.
    assert_eq!(
        assign(fixture.member.clone(), "evidence", &fixture.current_id, None, None)
            .await
            .status(),
        403
    );
    // Only the reviewer (or an administrator) marks it reviewed.
    let complete = |who: String| {
        call(
            "POST",
            format!("/v1/workspaces/{workspace}/review-assignment/complete"),
            who,
            json!({"kind": "evidence", "id": fixture.current_id, "next_review_at": "2999-01-01"}),
        )
    };
    assert_eq!(complete(fixture.member.clone()).await.status(), 403);
    let done = complete(helper.clone()).await;
    assert_eq!(done.status(), 200);
    let done = response_json(done).await;
    assert_eq!(done["overdue"], false);
    assert!(done["last_reviewed_at"].is_string());
    assert!(
        item(&queue().await, &fixture.current_id).is_none(),
        "reviewed and not due again"
    );

    // The reviewer leaves the workspace: the item says so rather than showing nobody.
    assert_eq!(
        assign(
            fixture.owner.clone(),
            "evidence",
            &fixture.current_id,
            Some(&helper_id),
            Some("2000-02-02")
        )
        .await
        .status(),
        200
    );
    let removed = call(
        "DELETE",
        format!("/v1/workspaces/{workspace}/members/{helper_id}"),
        fixture.owner.clone(),
        Value::Null,
    )
    .await;
    assert!(removed.status().is_success());
    let left = item(&queue().await, &fixture.current_id).unwrap();
    assert_eq!(left["reviewer_departed"], true);
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
