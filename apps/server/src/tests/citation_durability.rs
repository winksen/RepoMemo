//! Citation durability (V0.6.10): a revision records the content it cited, so a source update never makes a historical
//! citation look current, and the cited text is shown when it is kept and explained when it is not.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_historical_citation_says_when_its_source_changed_or_is_gone_and_shows_the_kept_text() {
    let fixture = knowledge_fixture("citation-durability").await;
    let app = fixture.app.clone();
    let card = fixture.card_on_stale_id.clone();
    let get = |uri: String, who: &str| {
        let app = app.clone();
        let request = auth_request("GET", &uri, who);
        async move { app.oneshot(request).await.unwrap() }
    };
    let revisions = |card: &str| {
        let uri = format!("/v1/memory-cards/{card}/revisions");
        let owner = fixture.owner.clone();
        let get = &get;
        async move { response_json(get(uri, &owner).await).await }
    };

    // As saved, the citation records the file and its content, and the file is as cited.
    let first = revisions(&card).await;
    assert_eq!(first[0]["citations"][0]["currency"], "current");
    assert_eq!(first[0]["citations"][0]["artifact_id"], fixture.stale_id.as_str());
    assert!(first[0]["citations"][0]["content_hash"].as_str().unwrap().len() >= 32);
    let source = response_json(
        get(
            format!("/v1/memory-cards/{card}/revisions/1/source?index=0"),
            &fixture.owner,
        )
        .await,
    )
    .await;
    assert_eq!(source["state"], "current");
    assert!(source["text"].as_str().unwrap().contains("fourteen days"));

    // The file gets a new version: the revision still says what it cited and no longer calls it current.
    let upload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/artifacts/{}/versions", fixture.stale_id))
                .header("authorization", &fixture.owner)
                .header("x-repomemo-filename", "refund.txt")
                .body(Body::from("Refunds are issued within thirty days of purchase."))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload.status(), 200);
    let changed = revisions(&card).await;
    assert_eq!(changed[0]["citations"][0]["currency"], "changed");
    let kept = response_json(
        get(
            format!("/v1/memory-cards/{card}/revisions/1/source?index=0"),
            &fixture.owner,
        )
        .await,
    )
    .await;
    assert_eq!(kept["state"], "kept_version");
    assert!(
        kept["text"].as_str().unwrap().contains("fourteen days"),
        "the text it cited, not today's"
    );
    // The portable export carries the same fact.
    let exported = get(format!("/v1/memory-cards/{card}/export"), &fixture.owner).await;
    let exported = String::from_utf8(to_bytes(exported.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
    assert!(exported.contains("SOURCE CHANGED since cited"), "{exported}");
    assert!(exported.contains("Revision: 1"));

    // A title edit after the file changed leaves the citation alone, so its recorded content stays what it was.
    let edited = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/memory-cards/{card}"),
            &fixture.owner,
            json!({"title": "Refund window, renamed", "body_markdown": "Refund window.", "source": "fixture"}),
        ))
        .await
        .unwrap();
    assert_eq!(edited.status(), 200);
    let after_edit = revisions(&card).await;
    assert_eq!(after_edit[0]["revision"], 2);
    assert_eq!(after_edit[0]["citations"][0]["currency"], "changed");
    assert_eq!(
        after_edit[0]["citations"][0]["content_hash"],
        after_edit[1]["citations"][0]["content_hash"]
    );

    // Restoring revision 1 brings back what it cited, not what the file is now.
    let restored = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/memory-cards/{card}/revisions/1/restore"),
            &fixture.owner,
            json!({}),
        ))
        .await
        .unwrap();
    assert_eq!(restored.status(), 200);
    let after_restore = revisions(&card).await;
    assert_eq!(after_restore[0]["citations"][0]["currency"], "changed");

    // A deleted source is "gone", and its text is not invented.
    let gone = revisions(&fixture.card_on_missing_id).await;
    assert_eq!(gone[0]["citations"][0]["currency"], "gone");
    let gone_source = response_json(
        get(
            format!(
                "/v1/memory-cards/{}/revisions/1/source?index=0",
                fixture.card_on_missing_id
            ),
            &fixture.owner,
        )
        .await,
    )
    .await;
    assert_eq!(gone_source["state"], "gone");
    assert!(gone_source["text"].is_null());

    // Out of range is not found; someone outside the workspace is refused, not told the file is gone.
    assert_eq!(
        get(
            format!("/v1/memory-cards/{card}/revisions/1/source?index=9"),
            &fixture.owner
        )
        .await
        .status(),
        404
    );
    let (outsider, _) = register_account(&app, "citation-durability-outsider@example.com").await;
    let refused = get(format!("/v1/memory-cards/{card}/revisions/1/source?index=0"), &outsider).await;
    assert!(refused.status() == 403 || refused.status() == 404);
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
