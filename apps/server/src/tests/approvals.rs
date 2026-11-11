//! Approval workflow (V0.6.8): an approval belongs to one revision, editing invalidates it, and independent review
//! cannot be satisfied by the person who made the revision.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn an_approval_is_tied_to_a_revision_and_independent_review_forbids_self_approval() {
    let fixture = knowledge_fixture("approvals").await;
    let app = fixture.app.clone();
    let card = fixture.card_on_stale_id.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let status = |who: String| {
        let app = app.clone();
        let uri = format!("/v1/memory-cards/{card}/approval");
        async move { response_json(app.oneshot(auth_request("GET", &uri, &who)).await.unwrap()).await }
    };
    let approve = |who: String, revision: i64| {
        call(
            "POST",
            format!("/v1/memory-cards/{card}/approval"),
            who,
            json!({"revision": revision, "note": "Checked against the policy."}),
        )
    };

    // Revision 1 was made by the owner. Without a policy the owner may approve it.
    let first = status(fixture.owner.clone()).await;
    assert_eq!(first["revision"], 1);
    assert_eq!(first["approved"], false);
    assert_eq!(first["can_approve"], true);
    // A guest who can only read cannot decide.
    assert_eq!(status(fixture.guest.clone()).await["can_approve"], false);
    assert_eq!(approve(fixture.guest.clone(), 1).await.status(), 403);

    // Independent review: the author cannot approve; the member can.
    assert_eq!(
        call(
            "PUT",
            format!("/v1/workspaces/{workspace}/approval-policy"),
            fixture.member.clone(),
            json!({"independent_review": true})
        )
        .await
        .status(),
        403,
        "only an administrator sets the policy"
    );
    assert_eq!(
        call(
            "PUT",
            format!("/v1/workspaces/{workspace}/approval-policy"),
            fixture.owner.clone(),
            json!({"independent_review": true})
        )
        .await
        .status(),
        200
    );
    assert_eq!(status(fixture.owner.clone()).await["can_approve"], false);
    assert_eq!(approve(fixture.owner.clone(), 1).await.status(), 403);
    let approved = approve(fixture.member.clone(), 1).await;
    assert_eq!(approved.status(), 200);
    let approved = response_json(approved).await;
    assert_eq!(approved["approved"], true);
    assert_eq!(approved["approved_by"]["email"], "approvals-member@example.com");
    assert_eq!(approved["history"][0]["note"], "Checked against the policy.");

    // Editing makes revision 2, which is not approved; the old decision stays in the history.
    let edited = call(
        "PUT",
        format!("/v1/memory-cards/{card}"),
        fixture.owner.clone(),
        json!({"title": "Refund window", "body_markdown": "Refunds take ten days.", "source": "test"}),
    )
    .await;
    assert_eq!(edited.status(), 200);
    let after = status(fixture.member.clone()).await;
    assert_eq!(after["revision"], 2);
    assert_eq!(after["approved"], false);
    assert_eq!(after["history"].as_array().unwrap().len(), 1);

    // Approving text the reviewer did not read is refused; the owner wrote revision 2, so the member approves it.
    assert_eq!(approve(fixture.member.clone(), 1).await.status(), 409);
    assert_eq!(approve(fixture.member.clone(), 2).await.status(), 200);
    // An approved revision cannot be approved twice, and only its approver or an administrator withdraws it.
    assert_eq!(approve(fixture.member.clone(), 2).await.status(), 409);
    let withdrawn = call(
        "DELETE",
        format!("/v1/memory-cards/{card}/approval"),
        fixture.owner.clone(),
        Value::Null,
    )
    .await;
    assert_eq!(withdrawn.status(), 200);
    let withdrawn = response_json(withdrawn).await;
    assert_eq!(withdrawn["approved"], false);
    assert_eq!(withdrawn["history"][0]["decision"], "withdrawn");
    assert_eq!(withdrawn["history"].as_array().unwrap().len(), 3);
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
