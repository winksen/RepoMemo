//! Revision limits (V0.6.19): a card's history is read a page at a time, with the number of revisions and where the next
//! page starts in headers, while opening or approving a card reads only the revision it needs.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_long_history_is_paged_and_a_card_with_many_revisions_still_opens_and_approves() {
    let fixture = knowledge_fixture("revision-limits").await;
    let app = fixture.app.clone();
    let card = fixture.card_on_stale_id.clone();
    for number in 2..=5 {
        let saved = app
            .clone()
            .oneshot(json_request(
                "PUT",
                &format!("/v1/memory-cards/{card}"),
                &fixture.owner,
                json!({"title": "Refund window", "body_markdown": format!("Edit {number}."), "source": "fixture"}),
            ))
            .await
            .unwrap();
        assert_eq!(saved.status(), 200);
    }
    let page = |query: &'static str| {
        let app = app.clone();
        let request = auth_request(
            "GET",
            &format!("/v1/memory-cards/{card}/revisions{query}"),
            &fixture.owner,
        );
        async move {
            let response = app.oneshot(request).await.unwrap();
            let header = |name: &str| {
                response
                    .headers()
                    .get(name)
                    .map(|value| value.to_str().unwrap().to_owned())
            };
            let (total, next) = (header("x-total-count"), header("x-next-cursor"));
            let numbers: Vec<i64> = response_json(response)
                .await
                .as_array()
                .unwrap()
                .iter()
                .map(|revision| revision["revision"].as_i64().unwrap())
                .collect();
            (numbers, total, next)
        }
    };
    // The newest come first; the header says how many there are and where the next page starts.
    assert_eq!(page("?limit=2").await, (vec![5, 4], Some("5".into()), Some("4".into())));
    assert_eq!(
        page("?limit=2&before=4").await,
        (vec![3, 2], Some("5".into()), Some("2".into()))
    );
    assert_eq!(page("?limit=2&before=2").await, (vec![1], Some("5".into()), None));
    // With no limit the first page holds up to fifty, so a short history still comes in one piece.
    assert_eq!(page("").await, (vec![5, 4, 3, 2, 1], Some("5".into()), None));

    // Opening one revision, comparing two and approving read only what they name.
    let source = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/memory-cards/{card}/revisions/3/source?index=0"),
            &fixture.owner,
        ))
        .await
        .unwrap();
    assert_eq!(source.status(), 200);
    let missing = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/memory-cards/{card}/revisions/9/source?index=0"),
            &fixture.owner,
        ))
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);
    let status = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/memory-cards/{card}/approval"),
                &fixture.owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status["revision"], 5);
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
