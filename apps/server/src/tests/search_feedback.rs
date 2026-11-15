//! Search feedback (V0.6.25): anyone who can read the workspace can say a question went unanswered or a result did not
//! help; they see their own requests, administrators see all and answer them with evidence or a memory card, and nobody
//! outside the workspace can send or read one.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_question_the_evidence_could_not_answer_reaches_the_owners_and_is_answered_once() {
    let fixture = knowledge_fixture("search-feedback").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let uri = format!("/v1/workspaces/{workspace}/search-feedback");
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let read = |uri: String, who: String| {
        let app = app.clone();
        async move { response_json(app.oneshot(auth_request("GET", &uri, &who)).await.unwrap()).await }
    };

    // A member and a guest who can only read both can ask; the search they came from is not needed or touched.
    let asked = call(
        "POST",
        uri.clone(),
        fixture.member.clone(),
        json!({"kind": "no_answer", "query": "How do refunds work for gift cards?", "note": "Nothing came back."}),
    )
    .await;
    assert_eq!(asked.status(), 201);
    let asked = response_json(asked).await;
    assert_eq!(asked["status"], "open");
    assert!(
        asked["expires_at"].as_str().unwrap().len() > 10,
        "the retention is stated on the request"
    );
    let unhelpful = call(
        "POST",
        uri.clone(),
        fixture.guest.clone(),
        json!({"kind": "unhelpful_result", "query": "refund window", "artifact_id": fixture.current_id}),
    )
    .await;
    assert_eq!(unhelpful.status(), 201);
    assert_eq!(response_json(unhelpful).await["artifact_title"], "Deploy checklist");

    // What is not a request is refused with a reason.
    for body in [
        json!({"kind": "praise", "query": "x"}),
        json!({"kind": "no_answer", "query": "   "}),
        json!({"kind": "no_answer", "query": "x".repeat(600)}),
    ] {
        assert_eq!(
            call("POST", uri.clone(), fixture.member.clone(), body).await.status(),
            400
        );
    }
    assert_eq!(
        call(
            "POST",
            uri.clone(),
            fixture.member.clone(),
            json!({"kind": "unhelpful_result", "query": "x", "artifact_id": "not-a-file"})
        )
        .await
        .status(),
        404
    );

    // A person sees their own; an administrator sees everyone's.
    assert_eq!(
        read(uri.clone(), fixture.member.clone())
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        read(uri.clone(), fixture.guest.clone()).await.as_array().unwrap().len(),
        1
    );
    let all = read(uri.clone(), fixture.owner.clone()).await;
    assert_eq!(all.as_array().unwrap().len(), 2);
    let first = asked["id"].as_str().unwrap().to_owned();
    let second = all
        .as_array()
        .unwrap()
        .iter()
        .find(|found| found["id"] != first.as_str())
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Only an administrator answers, with something that exists in the workspace, and only once.
    let resolve = |who: String, id: String, body: Value| call("POST", format!("{uri}/{id}/resolve"), who, body);
    assert_eq!(
        resolve(fixture.member.clone(), first.clone(), json!({"resolution": "declined"}))
            .await
            .status(),
        403
    );
    assert_eq!(
        resolve(
            fixture.owner.clone(),
            first.clone(),
            json!({"resolution": "memory_card", "target_id": "not-a-card"})
        )
        .await
        .status(),
        404
    );
    assert_eq!(
        resolve(
            fixture.owner.clone(),
            first.clone(),
            json!({"resolution": "memory_card"})
        )
        .await
        .status(),
        400
    );
    let answered = resolve(
        fixture.owner.clone(),
        first.clone(),
        json!({"resolution": "memory_card", "target_id": fixture.card_on_stale_id, "note": "Added a card."}),
    )
    .await;
    assert_eq!(answered.status(), 200);
    let answered = response_json(answered).await;
    assert_eq!(answered["status"], "resolved");
    assert_eq!(answered["resolution_title"], "Refund window");
    assert_eq!(
        resolve(fixture.owner.clone(), first.clone(), json!({"resolution": "declined"}))
            .await
            .status(),
        409,
        "an answered request is not answered twice"
    );
    let declined = response_json(resolve(fixture.owner.clone(), second, json!({"resolution": "declined"})).await).await;
    assert_eq!(declined["status"], "declined");

    // The person who asked sees the answer; the open list is now empty.
    let mine = read(format!("{uri}?status=resolved"), fixture.member.clone()).await;
    assert_eq!(mine[0]["resolution_title"], "Refund window");
    assert!(read(format!("{uri}?status=open"), fixture.owner.clone())
        .await
        .as_array()
        .unwrap()
        .is_empty());

    // Someone outside the workspace can neither send nor read.
    let (outsider, _) = register_account(&app, "search-feedback-outsider@example.com").await;
    assert!(call(
        "POST",
        uri.clone(),
        outsider.clone(),
        json!({"kind": "no_answer", "query": "anything"})
    )
    .await
    .status()
    .is_client_error());
    assert!(app
        .clone()
        .oneshot(auth_request("GET", &uri, &outsider))
        .await
        .unwrap()
        .status()
        .is_client_error());
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
