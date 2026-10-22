use super::*;

/// V0.2.167: the catalogue is for anyone signed in, a workspace's template is read by its members, and
/// only an owner or administrator with a session or an `admin` token changes it (decision 26); a change
/// is in the activity feed, an unknown template writes nothing and choosing the same one again records nothing.
#[tokio::test]
async fn templates_are_listed_read_by_members_and_changed_only_by_administrators() {
    let data_dir = temp_data_dir("templates");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let send = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &who, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let (_, _) = register_account(&app, "templates-admin@example.com").await;
    let (owner, _) = register_account(&app, "templates-owner@example.com").await;
    let (viewer, _) = register_account(&app, "templates-viewer@example.com").await;
    let (outsider, _) = register_account(&app, "templates-outsider@example.com").await;
    let workspace = create_workspace(&app, &owner).await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace,
        "templates-viewer@example.com",
        "viewer",
    )
    .await;
    let template_uri = format!("/v1/workspaces/{workspace}/template");

    // The catalogue: General first, only what identifies each template, and not for the anonymous.
    let listed = response_json(send("GET", "/v1/templates".to_owned(), viewer.clone(), json!(null)).await).await;
    assert_eq!(listed[0]["id"], "general");
    assert_eq!(listed[0]["name"], "General");
    assert!(listed[0]["version"].is_u64() && listed[0]["summary"].is_string() && listed[0]["icon"].is_string());
    assert!(
        listed[0].as_object().unwrap().len() == 5,
        "no block is listed: {}",
        listed[0]
    );
    let unsigned = Request::builder()
        .method("GET")
        .uri("/v1/templates")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(unsigned).await.unwrap().status(), 401);

    // A workspace that was never given one is on General, by default; a member reads it, a stranger does not.
    let by_default = response_json(send("GET", template_uri.clone(), viewer.clone(), json!(null)).await).await;
    assert_eq!(by_default["template"]["id"], "general");
    assert_eq!(by_default["source"], "default");
    assert!(by_default["reason"].is_null() && by_default["chosen_id"].is_null());
    assert_eq!(
        send("GET", template_uri.clone(), outsider.clone(), json!(null))
            .await
            .status(),
        403
    );

    // A viewer cannot change it, and neither can a read-and-write token of the owner.
    let choose_general = json!({"template_id": "general"});
    assert_eq!(
        send("PUT", template_uri.clone(), viewer.clone(), choose_general.clone())
            .await
            .status(),
        403
    );
    let mint = |scope: &'static str| {
        let send = &send;
        let owner = owner.clone();
        async move {
            let created = response_json(
                send(
                    "POST",
                    "/v1/profile/tokens".to_owned(),
                    owner,
                    json!({"name": scope, "scope": scope, "expires_in_days": 7}),
                )
                .await,
            )
            .await;
            format!("Bearer {}", created["secret"].as_str().unwrap())
        }
    };
    let write_token = mint("write").await;
    let refused = send("PUT", template_uri.clone(), write_token.clone(), choose_general.clone()).await;
    assert_eq!(refused.status(), 403);
    assert!(response_json(refused).await["error"]["message"]
        .as_str()
        .unwrap()
        .contains("admin scope"));
    let read_with_write_token = send("GET", template_uri.clone(), write_token, json!(null)).await;
    assert_eq!(
        read_with_write_token.status(),
        200,
        "a token that may read the workspace may read its template"
    );
    assert_eq!(
        send("GET", template_uri.clone(), viewer.clone(), json!(null))
            .await
            .status(),
        200
    );

    // An unknown template is not found and nothing is written.
    let unknown = send(
        "PUT",
        template_uri.clone(),
        owner.clone(),
        json!({"template_id": "no-such-template"}),
    )
    .await;
    assert_eq!(unknown.status(), 404);
    let after_unknown = response_json(send("GET", template_uri.clone(), owner.clone(), json!(null)).await).await;
    assert_eq!(after_unknown["source"], "default");
    let changes = |activity: &Value| {
        activity
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["action"] == "workspace_template_changed")
            .count()
    };
    let feed_uri = format!("/v1/workspaces/{workspace}/activity");
    let feed = response_json(send("GET", feed_uri.clone(), owner.clone(), json!(null)).await).await;
    assert_eq!(changes(&feed), 0);

    // The owner chooses General on purpose: it is the workspace's own choice, and it is in the feed once.
    let chosen = send("PUT", template_uri.clone(), owner.clone(), choose_general.clone()).await;
    assert_eq!(chosen.status(), 200);
    let chosen = response_json(chosen).await;
    assert_eq!(chosen["source"], "workspace");
    assert_eq!(chosen["chosen_id"], "general");
    let again = send("PUT", template_uri.clone(), owner.clone(), choose_general).await;
    assert_eq!(again.status(), 200);
    let feed = response_json(send("GET", feed_uri.clone(), owner.clone(), json!(null)).await).await;
    assert_eq!(changes(&feed), 1, "choosing the same template again records nothing");
    let event = feed
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["action"] == "workspace_template_changed")
        .unwrap();
    assert!(event["summary"].as_str().unwrap().contains("General"));

    // A token granted the admin scope may return the workspace to the default; that is a second change.
    let admin_token = mint("admin").await;
    let reset = send("PUT", template_uri.clone(), admin_token, json!({"template_id": null})).await;
    assert_eq!(reset.status(), 200);
    assert_eq!(response_json(reset).await["source"], "default");
    let feed = response_json(send("GET", feed_uri, owner.clone(), json!(null)).await).await;
    assert_eq!(changes(&feed), 2);

    let _ = std::fs::remove_dir_all(data_dir);
}
