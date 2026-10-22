use super::*;

/// A request that stays in flight until the sender is dropped: an upload whose body does not end.
fn holding(
    who: &str,
    workspace_id: &str,
    name: &str,
) -> (
    Request<Body>,
    tokio::sync::mpsc::Sender<Result<Vec<u8>, std::io::Error>>,
) {
    let (sender, receiver) = tokio::sync::mpsc::channel::<Result<Vec<u8>, std::io::Error>>(4);
    let request = Request::builder()
        .method("POST")
        .uri(format!("/v1/workspaces/{workspace_id}/artifacts/upload"))
        .header("authorization", who)
        .header("x-repomemo-filename", name)
        .header("content-length", "2048")
        .body(Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(receiver)))
        .unwrap();
    (request, sender)
}

/// V0.2.161: past the cap on requests being served a request is refused at once, `503` when the
/// server is full and `429` when one account holds more than its share; health checks and sign-in
/// are outside the count.
#[tokio::test]
async fn requests_past_the_cap_are_refused_at_once_and_health_and_sign_in_keep_room() {
    let data_dir = temp_data_dir("load-shedding");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.max_in_flight = 2;
    config.max_in_flight_per_account = 1;
    let app = router(config).await.unwrap();
    let (admin, _) = register_account(&app, "load-admin@example.com").await;
    let (ada, _) = register_account(&app, "load-ada@example.com").await;
    let (bob, _) = register_account(&app, "load-bob@example.com").await;
    let (cleo, _) = register_account(&app, "load-cleo@example.com").await;
    let ada_workspace = create_workspace(&app, &ada).await;
    let bob_workspace = create_workspace(&app, &bob).await;
    let status = |who: &str, uri: &str| {
        let app = app.clone();
        let request = auth_request("GET", uri, who);
        async move { app.oneshot(request).await.unwrap() }
    };

    // Ada has a request in flight. Her next one is over her share; Bob's is not.
    let (held, ada_sender) = holding(&ada, &ada_workspace, "ada-hold.txt");
    let ada_upload = tokio::spawn(app.clone().oneshot(held));
    let mut over_share = None;
    for _ in 0..40 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let attempt = status(&ada, "/v1/workspaces").await;
        if attempt.status() == 429 {
            over_share = Some(attempt);
            break;
        }
    }
    let over_share = over_share.expect("a second request of the same account is refused");
    assert!(over_share.headers().contains_key("retry-after"));
    assert_eq!(
        status(&bob, "/v1/workspaces").await.status(),
        200,
        "another account is not affected"
    );

    // Bob has one in flight too: the server is full, and a third account is told to wait.
    let (held, bob_sender) = holding(&bob, &bob_workspace, "bob-hold.txt");
    let bob_upload = tokio::spawn(app.clone().oneshot(held));
    let mut full = None;
    for _ in 0..40 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let attempt = status(&cleo, "/v1/workspaces").await;
        if attempt.status() == 503 {
            full = Some(attempt);
            break;
        }
    }
    let full = full.expect("a request is refused while the server is full");
    assert_eq!(full.headers()["retry-after"], "1");
    assert_eq!(response_json(full).await["error"]["code"], "overloaded");

    // Health checks and sign-in keep room of their own.
    assert_eq!(status(&cleo, "/health").await.status(), 200);
    assert_eq!(status(&cleo, "/health/ready").await.status(), 200);
    let login = app
        .clone()
        .oneshot(public_post(
            "/v1/auth/login",
            json!({"email": "load-cleo@example.com", "password": "not-a-real-password"}),
        ))
        .await
        .unwrap();
    assert_eq!(login.status(), 200);

    // When the two end, requests are served again, and the refusals were counted.
    ada_sender.send(Ok(b"ada's file".to_vec())).await.unwrap();
    bob_sender.send(Ok(b"bob's file".to_vec())).await.unwrap();
    drop(ada_sender);
    drop(bob_sender);
    assert_eq!(ada_upload.await.unwrap().unwrap().status(), 201);
    assert_eq!(bob_upload.await.unwrap().unwrap().status(), 201);
    assert_eq!(status(&cleo, "/v1/workspaces").await.status(), 200);
    let metrics = status(&admin, "/v1/system/metrics").await;
    assert_eq!(metrics.status(), 200);
    let metrics = String::from_utf8(to_bytes(metrics.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
    assert!(metrics.contains("repomemo_requests_shed_server_total "), "{metrics}");
    assert!(metrics.contains("repomemo_requests_shed_account_total "), "{metrics}");
    assert!(
        !metrics.contains("repomemo_requests_shed_account_total 0\n"),
        "an account was refused"
    );
    assert!(
        !metrics.contains("repomemo_requests_shed_server_total 0\n"),
        "the server was full"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
