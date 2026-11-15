//! Token scopes on the 0.6 administrative routes (V0.6.29): a read-and-write token can do content work but cannot change the
//! approval rule, remove an item from the trash for good, answer a search request or save an organization template; a
//! session and an admin-scope token are past the guard.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_write_token_cannot_make_the_administrative_changes_of_the_knowledge_workflow() {
    let fixture = knowledge_fixture("knowledge-scopes").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let token = |scope: &'static str| {
        let call = &call;
        let owner = fixture.owner.clone();
        async move {
            let made = call(
                "POST",
                "/v1/profile/tokens".to_owned(),
                owner,
                json!({"name": scope, "scope": scope, "expires_in_days": 30}),
            )
            .await;
            format!("Bearer {}", response_json(made).await["secret"].as_str().unwrap())
        }
    };
    let writer = token("write").await;
    let admin = token("admin").await;
    let organization = response_json(
        app.clone()
            .oneshot(auth_request("GET", "/v1/organizations", &fixture.owner))
            .await
            .unwrap(),
    )
    .await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Something to purge and something to answer.
    assert_eq!(
        app.clone()
            .oneshot(auth_request(
                "DELETE",
                &format!("/v1/memory-cards/{}", fixture.card_on_missing_id),
                &fixture.owner
            ))
            .await
            .unwrap()
            .status(),
        204
    );
    let trash = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace}/trash"),
                &fixture.owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    let trash_id = trash[0]["id"].as_str().unwrap().to_owned();
    let asked = response_json(
        call(
            "POST",
            format!("/v1/workspaces/{workspace}/search-feedback"),
            fixture.owner.clone(),
            json!({"kind": "no_answer", "query": "gift cards"}),
        )
        .await,
    )
    .await;
    let request_id = asked["id"].as_str().unwrap().to_owned();

    let attempts: Vec<(&str, String, Value)> = vec![
        (
            "PUT",
            format!("/v1/workspaces/{workspace}/approval-policy"),
            json!({"independent_review": true}),
        ),
        (
            "DELETE",
            format!("/v1/workspaces/{workspace}/trash/{trash_id}"),
            Value::Null,
        ),
        (
            "POST",
            format!("/v1/workspaces/{workspace}/search-feedback/{request_id}/resolve"),
            json!({"resolution": "declined"}),
        ),
        (
            "PUT",
            format!("/v1/organizations/{organization}/templates"),
            json!({"definition": "{}", "note": ""}),
        ),
    ];
    for (method, uri, body) in &attempts {
        let refused = call(method, uri.clone(), writer.clone(), body.clone()).await;
        assert_eq!(refused.status(), 403, "a write token on {method} {uri}");
    }
    // Past the guard with the admin scope: the request is judged on its own merits (the template here is not valid).
    let (_, last_uri, last_body) = &attempts[3];
    assert_eq!(
        call("PUT", last_uri.clone(), admin.clone(), last_body.clone())
            .await
            .status(),
        400
    );
    for (method, uri, body) in &attempts[..3] {
        let allowed = call(method, uri.clone(), admin.clone(), body.clone()).await;
        assert!(
            allowed.status().is_success(),
            "an admin token on {method} {uri}: {}",
            allowed.status()
        );
    }
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
