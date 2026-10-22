use super::*;

fn asking(uri: &str, authorization: &str, encoding: Option<&str>) -> Request<Body> {
    let mut request = Request::builder()
        .method("GET")
        .uri(uri)
        .header("authorization", authorization);
    if let Some(encoding) = encoding {
        request = request.header("accept-encoding", encoding);
    }
    request.body(Body::empty()).unwrap()
}

/// V0.2.160: JSON is compressed for a client that asks; event streams, downloads and short answers
/// are left as they are.
#[tokio::test]
async fn json_is_compressed_for_a_client_that_asks_and_streams_and_downloads_are_not() {
    let data_dir = temp_data_dir("compression");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "gzip-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let content = "A line of text that repeats. ".repeat(400);
    let note = response_json(
        app.clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
                &owner,
                json!({"title": "Long note", "content": content}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let artifact_id = note["id"].as_str().unwrap().to_owned();
    let detail = format!("/v1/artifacts/{artifact_id}");

    // Asked for nothing, the answer is plain.
    let plain = app.clone().oneshot(asking(&detail, &owner, None)).await.unwrap();
    assert_eq!(plain.status(), 200);
    assert!(plain.headers().get("content-encoding").is_none());
    let plain_length = to_bytes(plain.into_body(), usize::MAX).await.unwrap().len();
    assert!(plain_length > 4_000, "a long answer: {plain_length} bytes");

    // Asked for gzip or Brotli, it is much smaller, and says what it did.
    for encoding in ["gzip", "br"] {
        let packed = app
            .clone()
            .oneshot(asking(&detail, &owner, Some(encoding)))
            .await
            .unwrap();
        assert_eq!(packed.status(), 200);
        assert_eq!(packed.headers()["content-encoding"], encoding);
        assert!(packed.headers()["vary"]
            .to_str()
            .unwrap()
            .to_lowercase()
            .contains("accept-encoding"));
        let packed_length = to_bytes(packed.into_body(), usize::MAX).await.unwrap().len();
        assert!(
            packed_length * 4 < plain_length,
            "{encoding}: {packed_length} of {plain_length} bytes"
        );
    }

    // A short answer is not worth it.
    let health = app
        .clone()
        .oneshot(asking("/health", &owner, Some("gzip")))
        .await
        .unwrap();
    assert_eq!(health.status(), 200);
    assert!(health.headers().get("content-encoding").is_none());

    // A download is the file as it is stored.
    let file = app
        .clone()
        .oneshot(asking(&format!("{detail}/file"), &owner, Some("gzip")))
        .await
        .unwrap();
    assert_eq!(file.status(), 200);
    assert!(file.headers().get("content-encoding").is_none());
    assert_eq!(
        to_bytes(file.into_body(), usize::MAX).await.unwrap(),
        content.as_bytes()
    );

    // An event stream delivers its events as they happen, so it is never compressed.
    let stream = app
        .clone()
        .oneshot(asking(
            &format!("/v1/workspaces/{workspace_id}/events"),
            &owner,
            Some("gzip, br"),
        ))
        .await
        .unwrap();
    assert_eq!(stream.status(), 200);
    assert!(stream.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/event-stream"));
    assert!(stream.headers().get("content-encoding").is_none());
    let _ = std::fs::remove_dir_all(data_dir);
}
