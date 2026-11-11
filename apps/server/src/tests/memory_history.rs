//! Memory history (V0.6.3): attributed revisions, compare, and a restore that adds a revision and never rewrites one.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_card_keeps_attributed_revisions_that_compare_and_restore_without_rewriting_history() {
    let fixture = knowledge_fixture("memory-history").await;
    let app = fixture.app.clone();
    let card = fixture.card_on_stale_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let revisions = || {
        let app = app.clone();
        let who = fixture.owner.clone();
        let uri = format!("/v1/memory-cards/{card}/revisions");
        async move { response_json(app.oneshot(auth_request("GET", &uri, &who)).await.unwrap()).await }
    };

    // Created through the API: revision 1, by the owner, citing the refund policy.
    let first = revisions().await;
    assert_eq!(first.as_array().unwrap().len(), 1);
    assert_eq!(first[0]["revision"], 1);
    assert_eq!(first[0]["author"]["email"], "memory-history-owner@example.com");
    assert_eq!(first[0]["citations"][0]["title"], "Refund policy");

    // The member edits the statement and re-points it at current evidence: revision 2, by the member.
    let edited = call(
        "PUT",
        format!("/v1/memory-cards/{card}"),
        fixture.member.clone(),
        json!({
            "title": "Refund window", "body_markdown": "Refunds take fourteen days.\nExceptions need approval.", "source": "test",
            "citations": [{"artifact_id": fixture.current_id, "chunk_id": null, "title": "Deploy checklist", "path": "x",
                "start_line": null, "end_line": null, "confidence": null}],
        }),
    )
    .await;
    assert_eq!(edited.status(), 200);
    let after = revisions().await;
    assert_eq!(after.as_array().unwrap().len(), 2);
    assert_eq!(after[0]["revision"], 2);
    assert_eq!(after[0]["author"]["email"], "memory-history-member@example.com");
    assert_eq!(
        after[1]["body_markdown"], "Refunds take fourteen days.",
        "an earlier revision is untouched"
    );

    // Comparing shows the added line and the swapped citation.
    let compared = response_json(
        call(
            "GET",
            format!("/v1/memory-cards/{card}/revisions/compare?from=2&to=1"),
            fixture.owner.clone(),
            Value::Null,
        )
        .await,
    )
    .await;
    assert_eq!(compared["from_revision"], 1);
    assert_eq!(compared["to_revision"], 2);
    assert_eq!(compared["body"]["added"], 1);
    assert_eq!(compared["citations_added"][0]["title"], "Deploy checklist");
    assert_eq!(compared["citations_removed"][0]["title"], "Refund policy");

    // The refund policy is deleted. Restoring revision 1 still works, as revision 3, and says what could not come back.
    let deleted = call(
        "POST",
        format!("/v1/workspaces/{}/artifacts/bulk", fixture.workspace_id),
        fixture.owner.clone(),
        json!({"action": "delete", "artifact_ids": [fixture.stale_id]}),
    )
    .await;
    assert_eq!(deleted.status(), 200);
    let restored = call(
        "POST",
        format!("/v1/memory-cards/{card}/revisions/1/restore"),
        fixture.owner.clone(),
        Value::Null,
    )
    .await;
    assert_eq!(restored.status(), 200);
    let restored = response_json(restored).await;
    assert_eq!(restored["revision"], 3);
    assert_eq!(restored["card"]["body_markdown"], "Refunds take fourteen days.");
    assert_eq!(restored["skipped_citations"][0]["title"], "Refund policy");
    let all = revisions().await;
    assert_eq!(all.as_array().unwrap().len(), 3, "history only grew");
    assert_eq!(all[0]["restored_from"], 1);
    assert_eq!(all[0]["author"]["email"], "memory-history-owner@example.com");
    assert_eq!(all[2]["body_markdown"], "Refunds take fourteen days.");

    // The guest can read the history but not restore from it; an outsider sees nothing.
    let guest_read = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/memory-cards/{card}/revisions"),
            &fixture.guest,
        ))
        .await
        .unwrap();
    assert_eq!(guest_read.status(), 200);
    let guest_restore = call(
        "POST",
        format!("/v1/memory-cards/{card}/revisions/2/restore"),
        fixture.guest.clone(),
        Value::Null,
    )
    .await;
    assert_eq!(guest_restore.status(), 403);
    let (outsider, _) = register_account(&app, "memory-history-outsider@example.com").await;
    let outsider_read = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/memory-cards/{card}/revisions"),
            &outsider,
        ))
        .await
        .unwrap();
    assert!(outsider_read.status() == 403 || outsider_read.status() == 404);
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
