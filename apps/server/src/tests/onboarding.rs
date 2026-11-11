//! The first-value checklist (V0.6.1): steps follow what exists, the dismissal is personal, and nothing needs AI or mail.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn the_checklist_follows_real_progress_and_hiding_it_is_personal() {
    let fixture = knowledge_fixture("onboarding").await;
    let app = fixture.app.clone();
    let uri = format!("/v1/workspaces/{}/onboarding", fixture.workspace_id);
    let read = |who: String| {
        let app = app.clone();
        let uri = uri.clone();
        async move { response_json(app.oneshot(auth_request("GET", &uri, &who)).await.unwrap()).await }
    };
    let steps = |view: &Value| -> Vec<(String, bool)> {
        view["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|step| (step["id"].as_str().unwrap().to_owned(), step["done"].as_bool().unwrap()))
            .collect()
    };

    // The fixture has indexed evidence, cards that cite it, an organization member and a workspace guest.
    let view = read(fixture.owner.clone()).await;
    assert_eq!(
        steps(&view),
        vec![
            ("add_evidence".to_owned(), true),
            ("searchable".to_owned(), true),
            ("cited_memory".to_owned(), true),
            ("invite".to_owned(), true),
        ]
    );
    assert_eq!(view["done_count"], 4);
    assert_eq!(view["dismissed"], false);

    // Hiding it is the caller's own choice: the member still sees it.
    let hidden = app
        .clone()
        .oneshot(json_request("PUT", &uri, &fixture.owner, json!({"dismissed": true})))
        .await
        .unwrap();
    assert_eq!(hidden.status(), 204);
    assert_eq!(read(fixture.owner.clone()).await["dismissed"], true);
    assert_eq!(read(fixture.member.clone()).await["dismissed"], false);
    app.clone()
        .oneshot(json_request("PUT", &uri, &fixture.owner, json!({"dismissed": false})))
        .await
        .unwrap();
    assert_eq!(read(fixture.owner.clone()).await["dismissed"], false);

    // A brand-new workspace has done nothing yet, and a clicked button cannot change that: only content moves a step.
    let empty = create_workspace(&app, &fixture.owner).await;
    let view = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{empty}/onboarding"),
                &fixture.owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(view["done_count"], 0);
    let created = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{empty}/artifacts/text"),
            &fixture.owner,
            json!({"title": "First", "content": "A first note about deploys."}),
        ))
        .await
        .unwrap();
    assert_eq!(created.status(), 201);
    let view = response_json(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{empty}/onboarding"),
                &fixture.owner,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(steps(&view)[0], ("add_evidence".to_owned(), true));
    assert_eq!(steps(&view)[2], ("cited_memory".to_owned(), false));

    // An outsider learns nothing about the workspace.
    let (outsider, _) = register_account(&app, "onboarding-outsider@example.com").await;
    let refused = app.clone().oneshot(auth_request("GET", &uri, &outsider)).await.unwrap();
    assert!(refused.status() == 403 || refused.status() == 404);
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
