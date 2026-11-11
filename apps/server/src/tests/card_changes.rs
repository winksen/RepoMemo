//! Change explanation (V0.6.9): each reason a card needs review says what changed, or why that cannot be shown.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_card_that_needs_review_explains_retired_and_missing_evidence_to_everyone_who_can_read_it() {
    let fixture = knowledge_fixture("card-changes").await;
    let app = fixture.app.clone();
    let get = |card: &str, who: &str| {
        let app = app.clone();
        let request = auth_request("GET", &format!("/v1/memory-cards/{card}/changes"), who);
        async move { app.oneshot(request).await.unwrap() }
    };

    // The card on retired evidence: the file's text did not change, and that is said rather than left blank.
    let retired = get(&fixture.card_on_stale_id, &fixture.guest).await;
    assert_eq!(retired.status(), 200);
    let retired = response_json(retired).await;
    assert_eq!(retired["changes"].as_array().unwrap().len(), 1);
    let change = &retired["changes"][0];
    assert_eq!(change["kind"], "retired");
    assert_eq!(change["title"], "Refund policy");
    assert_eq!(change["artifact_id"], fixture.stale_id.as_str());
    assert!(change["comparison"].is_null());
    assert!(change["unavailable_reason"]
        .as_str()
        .unwrap()
        .contains("did not change"));

    // The card whose file was deleted names the source and says the old text is not kept.
    let missing = response_json(get(&fixture.card_on_missing_id, &fixture.owner).await).await;
    let change = &missing["changes"][0];
    assert_eq!(change["kind"], "missing");
    assert!(change["unavailable_reason"]
        .as_str()
        .unwrap()
        .contains("no longer exists"));

    // Replacing the card's citations with current evidence clears the explanation.
    let updated = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/memory-cards/{}", fixture.card_on_stale_id),
            &fixture.owner,
            json!({"title": "Refund window", "body_markdown": "Refund window.", "source": "fixture", "citations": []}),
        ))
        .await
        .unwrap();
    assert_eq!(updated.status(), 200);
    let cleared = response_json(get(&fixture.card_on_stale_id, &fixture.owner).await).await;
    assert!(cleared["changes"].as_array().unwrap().is_empty());

    // Someone outside the workspace learns nothing.
    let (outsider, _) = register_account(&app, "card-changes-outsider@example.com").await;
    let refused = get(&fixture.card_on_stale_id, &outsider).await;
    assert!(refused.status() == 403 || refused.status() == 404);
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
