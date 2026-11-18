use super::*;
use std::sync::{Arc, Mutex};

/// A Slack incoming webhook that records the messages it receives.
#[derive(Clone, Default)]
struct FakeSlack {
    messages: Arc<Mutex<Vec<String>>>,
    status: Arc<Mutex<u16>>,
}

async fn start_fake_slack(state: FakeSlack) -> String {
    use axum::{extract::State, http::StatusCode, routing::post};
    let app = axum::Router::new()
        .route(
            "/services/T0/B0/secretpath",
            post(|State(state): State<FakeSlack>, body: String| async move {
                state.messages.lock().unwrap().push(body);
                StatusCode::from_u16(*state.status.lock().unwrap()).unwrap()
            }),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}/services/T0/B0/secretpath")
}

#[tokio::test]
async fn digests_reach_an_approved_channel_once_as_a_count_and_a_link() {
    let slack = FakeSlack::default();
    *slack.status.lock().unwrap() = 200;
    let hook = start_fake_slack(slack.clone()).await;
    let flaky = FakeSlack::default();
    *flaky.status.lock().unwrap() = 500;
    let flaky_hook = start_fake_slack(flaky.clone()).await;

    let data_dir = temp_data_dir("chat");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.outbound_allowed_hosts = vec!["127.0.0.1".to_owned()];
    config.public_url = Some("https://repomemo.example.test".to_owned());
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "chat-owner@example.com").await;
    let (stranger, _) = register_account(&app, "chat-stranger@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |who: String, method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let uri = format!("/v1/workspaces/{workspace_id}/chat-destinations");

    // Only an approved Slack address, only an administrator.
    let refused = call(
        owner.clone(),
        "POST",
        uri.clone(),
        json!({"name": "Ops", "url": "https://hooks.example.com/services/a/b/c", "digests": ["projects"]}),
    )
    .await;
    assert_eq!(refused.status(), 400);
    let bad_digest = call(
        owner.clone(),
        "POST",
        uri.clone(),
        json!({"name": "Ops", "url": hook, "digests": ["titles"]}),
    )
    .await;
    assert_eq!(bad_digest.status(), 400);
    let by_stranger = call(stranger, "GET", uri.clone(), json!({})).await;
    assert!(by_stranger.status() == 403 || by_stranger.status() == 404);

    // The address is a secret: it is never returned.
    let created = response_json(
        call(
            owner.clone(),
            "POST",
            uri.clone(),
            json!({"name": "Ops", "url": hook, "digests": ["projects", "review"]}),
        )
        .await,
    )
    .await;
    assert!(!created.to_string().contains("secretpath"));
    assert_eq!(created["host"], "127.0.0.1");
    let id = created["id"].as_str().unwrap().to_owned();
    let listed = response_json(call(owner.clone(), "GET", uri.clone(), json!({})).await).await;
    assert!(!listed.to_string().contains("secretpath"));

    // A test message carries no workspace content.
    let tested = response_json(
        call(
            owner.clone(),
            "POST",
            format!("/v1/chat-destinations/{id}/test"),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(tested["delivered"], true, "{tested}");
    assert!(slack.messages.lock().unwrap()[0].contains("this is a test"));

    // Nothing to report, nothing sent.
    let quiet = response_json(
        call(
            owner.clone(),
            "POST",
            format!("/v1/chat-destinations/{id}/digest"),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(quiet["sent"].as_array().unwrap().len(), 0);

    // An overdue task: the digest says how many and where, never which.
    let task = call(
        owner.clone(),
        "POST",
        format!("/v1/workspaces/{workspace_id}/tasks"),
        json!({"title": "Secret migration plan", "priority": "medium", "status": "open", "due_at": "2020-01-01T00:00:00Z"}),
    )
    .await;
    assert_eq!(task.status(), 201);
    let sent = response_json(
        call(
            owner.clone(),
            "POST",
            format!("/v1/chat-destinations/{id}/digest"),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(sent["sent"], json!(["projects"]));
    {
        let messages = slack.messages.lock().unwrap();
        let digest = messages.last().unwrap();
        assert!(digest.contains("1 open task is overdue"), "{digest}");
        assert!(digest.contains(&format!(
            "https://repomemo.example.test/workspaces/{workspace_id}/review"
        )));
        assert!(!digest.contains("Secret migration"), "a digest holds no title");
    }

    // The same digest is not sent twice in a window.
    let before = slack.messages.lock().unwrap().len();
    let again = response_json(
        call(
            owner.clone(),
            "POST",
            format!("/v1/chat-destinations/{id}/digest"),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(again["sent"].as_array().unwrap().len(), 0);
    assert_eq!(slack.messages.lock().unwrap().len(), before);

    // A failing channel gives the digest back, and the retry sends it once.
    let second = response_json(
        call(
            owner.clone(),
            "POST",
            uri.clone(),
            json!({"name": "Flaky", "url": flaky_hook, "digests": ["projects"]}),
        )
        .await,
    )
    .await;
    let flaky_id = second["id"].as_str().unwrap().to_owned();
    let failed = response_json(
        call(
            owner.clone(),
            "POST",
            format!("/v1/chat-destinations/{flaky_id}/digest"),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(failed["sent"].as_array().unwrap().len(), 0);
    let shown = response_json(call(owner.clone(), "GET", uri.clone(), json!({})).await).await;
    assert!(shown["destinations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|destination| destination["id"] == flaky_id.as_str() && destination["last_error"].is_string()));
    *flaky.status.lock().unwrap() = 200;
    let retried = response_json(
        call(
            owner.clone(),
            "POST",
            format!("/v1/chat-destinations/{flaky_id}/digest"),
            json!({}),
        )
        .await,
    )
    .await;
    assert_eq!(retried["sent"], json!(["projects"]));
    assert_eq!(
        flaky.messages.lock().unwrap().len(),
        2,
        "one failed call and one delivered"
    );

    // Turning it off or removing it stops delivery.
    let off = call(
        owner.clone(),
        "PUT",
        format!("/v1/chat-destinations/{id}"),
        json!({"enabled": false}),
    )
    .await;
    assert_eq!(off.status(), 200);
    assert_eq!(
        call(
            owner.clone(),
            "POST",
            format!("/v1/chat-destinations/{id}/digest"),
            json!({})
        )
        .await
        .status(),
        409
    );
    assert_eq!(
        call(
            owner.clone(),
            "DELETE",
            format!("/v1/chat-destinations/{flaky_id}"),
            json!({})
        )
        .await
        .status(),
        204
    );
    let left = response_json(call(owner, "GET", uri, json!({})).await).await;
    assert_eq!(left["destinations"].as_array().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}
