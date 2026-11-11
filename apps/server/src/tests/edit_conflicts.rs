//! Concurrent editing (V0.6.4): a save based on an older version is refused and changes nothing.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_stale_card_save_is_refused_and_the_losing_editor_can_retry_from_the_newer_revision() {
    let fixture = knowledge_fixture("edit-conflict").await;
    let app = fixture.app.clone();
    let card = fixture.card_on_stale_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let detail = |who: String| {
        let app = app.clone();
        let uri = format!("/v1/memory-cards/{card}");
        async move { response_json(app.oneshot(auth_request("GET", &uri, &who)).await.unwrap()).await }
    };
    let save = |who: String, body: &str, expected: Option<i64>| {
        let mut payload = json!({"title": "Refund window", "body_markdown": body, "source": "test"});
        if let Some(expected) = expected {
            payload["expected_revision"] = json!(expected);
        }
        call("PUT", format!("/v1/memory-cards/{card}"), who, payload)
    };

    // Both editors open the card at the same revision.
    let opened = detail(fixture.owner.clone()).await["revision"].as_i64().unwrap();
    assert_eq!(opened, 1);
    assert_eq!(detail(fixture.member.clone()).await["revision"], 1);

    // The first save wins and becomes revision 2.
    assert_eq!(
        save(fixture.owner.clone(), "Refunds take ten days.", Some(opened))
            .await
            .status(),
        200
    );
    assert_eq!(detail(fixture.owner.clone()).await["revision"], 2);

    // The second editor started from revision 1: refused, and the card still has the first editor's text.
    let stale = save(fixture.member.clone(), "Refunds take twenty days.", Some(opened)).await;
    assert_eq!(stale.status(), 409);
    let body = response_json(stale).await;
    assert!(body["error"]["message"].as_str().unwrap().contains("revision 2"));
    let current = detail(fixture.member.clone()).await;
    assert_eq!(current["card"]["body_markdown"], "Refunds take ten days.");
    assert_eq!(current["revision"], 2, "a refused save leaves no revision behind");

    // Having seen revision 2, the same text saves: the overwrite is a knowing one.
    assert_eq!(
        save(fixture.member.clone(), "Refunds take twenty days.", Some(2))
            .await
            .status(),
        200
    );
    assert_eq!(detail(fixture.member.clone()).await["revision"], 3);

    // A client that sends no revision keeps the old behaviour.
    assert_eq!(
        save(fixture.member.clone(), "Refunds take thirty days.", None)
            .await
            .status(),
        200
    );

    // A file changed since it was opened is not overwritten either.
    let upload = |seen: &str| {
        let request = Request::builder()
            .method("POST")
            .uri(format!("/v1/artifacts/{}/versions", fixture.current_id))
            .header("authorization", &fixture.owner)
            .header("x-repomemo-expected-updated-at", seen)
            .header("x-repomemo-filename", "deploy.md")
            .body(Body::from("A new version."))
            .unwrap();
        let app = app.clone();
        async move { app.oneshot(request).await.unwrap() }
    };
    assert_eq!(upload("1999-01-01T00:00:00+00:00").await.status(), 409);
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
