//! Revision attribution (V0.6.11): a revision's author is written with the revision, so near-simultaneous saves each
//! credit their own writer and no new revision is left without one.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn each_save_credits_its_own_writer_even_when_two_saves_overlap() {
    let fixture = knowledge_fixture("revision-attribution").await;
    let app = fixture.app.clone();
    let card = fixture.card_on_stale_id.clone();
    let save = |who: String, title: &'static str| {
        let app = app.clone();
        let request = json_request(
            "PUT",
            &format!("/v1/memory-cards/{card}"),
            &who,
            json!({"title": title, "body_markdown": "Refund window.", "source": "fixture"}),
        );
        async move { app.oneshot(request).await.unwrap().status() }
    };
    // Both saves are in flight at once; neither names the revision it started from.
    let (first, second) = tokio::join!(
        save(fixture.owner.clone(), "Saved by the owner"),
        save(fixture.member.clone(), "Saved by the member")
    );
    assert_eq!((first.as_u16(), second.as_u16()), (200, 200));

    let revisions = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/memory-cards/{card}/revisions"),
                &fixture.owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    for revision in revisions.as_array().unwrap() {
        let title = revision["title"].as_str().unwrap();
        let email = revision["author"]["email"].as_str().unwrap_or("");
        if title == "Saved by the owner" {
            assert!(email.starts_with("revision-attribution-owner"), "{title}: {email}");
        } else if title == "Saved by the member" {
            assert!(email.starts_with("revision-attribution-member"), "{title}: {email}");
        }
        // Revisions written since V0.6.11 always have an author; only the card's first one may predate it.
        if revision["revision"].as_i64().unwrap() > 1 {
            assert!(revision["author"].is_object(), "revision without an author: {revision}");
        }
    }
    assert_eq!(revisions.as_array().unwrap().len(), 3);
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
