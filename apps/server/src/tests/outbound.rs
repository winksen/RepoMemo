use super::*;
use std::sync::{Arc, Mutex};

type Call = (std::collections::HashMap<String, String>, String);

/// A destination that records what it receives and answers with a status the test chooses.
#[derive(Clone, Default)]
struct Receiver {
    calls: Arc<Mutex<Vec<Call>>>,
    status: Arc<Mutex<u16>>,
}

async fn start_receiver(state: Receiver) -> String {
    use axum::{extract::State, http::HeaderMap, http::StatusCode, routing::post};
    let app = axum::Router::new()
        .route(
            "/hook",
            post(
                |State(state): State<Receiver>, headers: HeaderMap, body: String| async move {
                    let headers = headers
                        .iter()
                        .map(|(name, value)| (name.as_str().to_owned(), value.to_str().unwrap_or_default().to_owned()))
                        .collect();
                    state.calls.lock().unwrap().push((headers, body));
                    StatusCode::from_u16(*state.status.lock().unwrap()).unwrap()
                },
            ),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}/hook")
}

#[tokio::test]
async fn outbound_webhooks_need_an_approved_destination_and_sign_what_they_send() {
    // Off until the operator names a host.
    let off_dir = temp_data_dir("outbound-off");
    let off = router(ServerConfig::for_test(off_dir.clone())).await.unwrap();
    let (owner_off, _) = register_account(&off, "outbound-off@example.com").await;
    let workspace_off = create_workspace(&off, &owner_off).await;
    let refused = off
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_off}/outbound-endpoints"),
            &owner_off,
            json!({"name": "Ops", "url": "https://hooks.example.com/x", "events": ["ping"]}),
        ))
        .await
        .unwrap();
    assert_eq!(refused.status(), 400);
    assert!(response_json(refused)
        .await
        .to_string()
        .contains("REPOMEMO_OUTBOUND_HOSTS"));
    let _ = std::fs::remove_dir_all(off_dir);

    let receiver = Receiver::default();
    *receiver.status.lock().unwrap() = 200;
    let hook = start_receiver(receiver.clone()).await;
    let data_dir = temp_data_dir("outbound");
    let mut config = ServerConfig::for_test(data_dir.clone());
    config.outbound_allowed_hosts = vec!["127.0.0.1".to_owned()];
    let app = router(config).await.unwrap();
    let (owner, _) = register_account(&app, "outbound-owner@example.com").await;
    let (stranger, _) = register_account(&app, "outbound-stranger@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |who: String, method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let endpoints_uri = format!("/v1/workspaces/{workspace_id}/outbound-endpoints");

    // Addresses that are not approved destinations, and events that do not exist.
    for (url, expected) in [
        ("http://hooks.example.com/x", "https"),
        ("https://hooks.example.com/x", "only sends events to"),
        ("https://user:secret@127.0.0.1/x", "password"),
        ("not a url", "valid address"),
    ] {
        let response = call(
            owner.clone(),
            "POST",
            endpoints_uri.clone(),
            json!({"name": "Ops", "url": url, "events": ["ping"]}),
        )
        .await;
        assert_eq!(response.status(), 400, "{url}");
        assert!(response_json(response).await.to_string().contains(expected), "{url}");
    }
    let unknown = call(
        owner.clone(),
        "POST",
        endpoints_uri.clone(),
        json!({"name": "Ops", "url": hook, "events": ["task.deleted"]}),
    )
    .await;
    assert_eq!(unknown.status(), 400);

    // Only an administrator can read or add endpoints.
    let by_stranger = call(stranger, "GET", endpoints_uri.clone(), json!({})).await;
    assert!(by_stranger.status() == 403 || by_stranger.status() == 404);

    // An approved destination: the secret is shown once and never listed.
    let created = response_json(
        call(
            owner.clone(),
            "POST",
            endpoints_uri.clone(),
            json!({"name": "Ops", "url": hook, "events": ["ping", "connector.released"]}),
        )
        .await,
    )
    .await;
    let secret = created["secret"].as_str().unwrap().to_owned();
    assert!(secret.starts_with("whsec_"));
    let endpoint_id = created["endpoint"]["id"].as_str().unwrap().to_owned();
    let listed = response_json(call(owner.clone(), "GET", endpoints_uri.clone(), json!({})).await).await;
    assert_eq!(listed["endpoints"].as_array().unwrap().len(), 1);
    assert_eq!(listed["available"], true);
    assert!(!listed.to_string().contains(&secret), "the secret is shown once");

    // A test event arrives signed, with the ids that identify a repeat.
    let queued = call(
        owner.clone(),
        "POST",
        format!("/v1/outbound-endpoints/{endpoint_id}/test"),
        json!({}),
    )
    .await;
    assert_eq!(queued.status(), 202);
    let event_id = response_json(queued).await["event_id"].as_str().unwrap().to_owned();
    let deliveries_uri = format!("/v1/outbound-endpoints/{endpoint_id}/deliveries");
    let wait_for = |status: &'static str| {
        let call = &call;
        let owner = owner.clone();
        let deliveries_uri = deliveries_uri.clone();
        async move {
            for _ in 0..100 {
                let deliveries =
                    response_json(call(owner.clone(), "GET", deliveries_uri.clone(), json!({})).await).await;
                if let Some(found) = deliveries
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|delivery| delivery["status"] == status)
                {
                    return found.clone();
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            panic!("no delivery became {status}");
        }
    };
    let delivered = wait_for("delivered").await;
    assert_eq!(delivered["event_id"], event_id.as_str());
    assert_eq!(delivered["attempts"], 1);
    {
        let calls = receiver.calls.lock().unwrap();
        let (headers, body) = &calls[0];
        assert_eq!(headers["x-repomemo-event"], "ping");
        assert_eq!(headers["x-repomemo-event-id"], event_id);
        assert_eq!(headers["x-repomemo-attempt"], "1");
        assert_eq!(
            headers["x-repomemo-signature"],
            crate::outbound::sign(&secret, &headers["x-repomemo-timestamp"], body)
        );
        let payload: Value = serde_json::from_str(body).unwrap();
        assert_eq!(payload["id"], event_id.as_str());
        assert_eq!(payload["type"], "ping");
        assert_eq!(payload["workspace_id"], workspace_id.as_str());
    }

    // A destination that fails is retried later with the same event id; it is not given up on at once.
    *receiver.status.lock().unwrap() = 500;
    let before = receiver.calls.lock().unwrap().len();
    let second = response_json(
        call(
            owner.clone(),
            "POST",
            format!("/v1/outbound-endpoints/{endpoint_id}/test"),
            json!({}),
        )
        .await,
    )
    .await["event_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let pending = wait_for("pending").await;
    assert_eq!(pending["event_id"], second.as_str());
    assert_eq!(pending["attempts"], 1);
    assert_eq!(pending["last_status_code"], 500);
    assert!(pending["next_attempt_at"].is_string());
    assert_eq!(receiver.calls.lock().unwrap().len(), before + 1);

    // Turning it off cancels what was queued: nothing more is sent to it.
    let off = response_json(
        call(
            owner.clone(),
            "PUT",
            format!("/v1/outbound-endpoints/{endpoint_id}"),
            json!({"enabled": false}),
        )
        .await,
    )
    .await;
    assert_eq!(off["enabled"], false);
    let cancelled = wait_for("cancelled").await;
    assert_eq!(cancelled["event_id"], second.as_str());
    assert_eq!(
        call(
            owner.clone(),
            "POST",
            format!("/v1/outbound-endpoints/{endpoint_id}/test"),
            json!({})
        )
        .await
        .status(),
        409
    );
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert_eq!(receiver.calls.lock().unwrap().len(), before + 1);

    // A new secret ends the old one; removing the endpoint removes its deliveries.
    let rotated = response_json(
        call(
            owner.clone(),
            "POST",
            format!("/v1/outbound-endpoints/{endpoint_id}/secret"),
            json!({}),
        )
        .await,
    )
    .await;
    assert_ne!(rotated["secret"], secret.as_str());
    assert_eq!(
        call(
            owner.clone(),
            "DELETE",
            format!("/v1/outbound-endpoints/{endpoint_id}"),
            json!({})
        )
        .await
        .status(),
        204
    );
    assert_eq!(call(owner, "GET", deliveries_uri, json!({})).await.status(), 404);
    let _ = std::fs::remove_dir_all(data_dir);
}
