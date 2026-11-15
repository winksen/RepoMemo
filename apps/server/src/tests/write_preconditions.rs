//! Write preconditions (V0.6.18): the check that a file was not changed since it was opened is part of the write, so two
//! uploads that both started from the same state cannot both succeed.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn of_two_uploads_from_the_same_state_exactly_one_is_stored() {
    let fixture = knowledge_fixture("write-preconditions").await;
    let app = fixture.app.clone();
    let id = fixture.current_id.clone();
    let detail = response_json(
        app.clone()
            .oneshot(auth_request("GET", &format!("/v1/artifacts/{id}"), &fixture.owner))
            .await
            .unwrap(),
    )
    .await;
    let seen = detail["summary"]["updated_at"].as_str().unwrap().to_owned();
    let upload = |body: &'static str| {
        let request = Request::builder()
            .method("POST")
            .uri(format!("/v1/artifacts/{id}/versions"))
            .header("authorization", &fixture.owner)
            .header("x-repomemo-expected-updated-at", &seen)
            .header("x-repomemo-filename", "deploy.md")
            .body(Body::from(body))
            .unwrap();
        let app = app.clone();
        async move { app.oneshot(request).await.unwrap().status().as_u16() }
    };
    let (first, second) = tokio::join!(
        upload("# First writer\n\nFrom one editor."),
        upload("# Second writer\n\nFrom another.")
    );
    let mut results = [first, second];
    results.sort_unstable();
    assert_eq!(
        results,
        [200, 409],
        "one upload is stored and the other is refused, not overwritten"
    );

    // The file has exactly one new version, and a client that names no state still writes unchecked (docs/API.md).
    let versions = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/artifacts/{id}/versions"),
                &fixture.owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(versions.as_array().unwrap().len(), 2, "{versions}");
    let unchecked = Request::builder()
        .method("POST")
        .uri(format!("/v1/artifacts/{id}/versions"))
        .header("authorization", &fixture.owner)
        .header("x-repomemo-filename", "deploy.md")
        .body(Body::from("# Third writer\n\nNames no state."))
        .unwrap();
    assert_eq!(app.clone().oneshot(unchecked).await.unwrap().status(), 200);
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
