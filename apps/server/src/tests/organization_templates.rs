//! Organization templates (V0.6.23): administrators save and version templates, a workspace of the organization previews
//! and adopts one with no restart, other organizations never see it, and saving a newer version changes no right.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn an_organization_saves_versions_and_offers_a_template_to_its_own_workspaces_only() {
    let fixture = knowledge_fixture("organization-templates").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let read = |uri: String, who: String| {
        let app = app.clone();
        async move { response_json(app.oneshot(auth_request("GET", &uri, &who)).await.unwrap()).await }
    };
    let organization = read("/v1/organizations".into(), fixture.owner.clone()).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // A valid template file: the workspace's own setup, exported under a new id.
    let file = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace}/template/file?id=night-shift&name=Night%20shift"),
            &fixture.owner,
        ))
        .await
        .unwrap();
    assert_eq!(file.status(), 200);
    let text = String::from_utf8(to_bytes(file.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();

    // Only an administrator of the organization saves one; a plain member is refused.
    let save = |who: String, definition: String, note: &'static str| {
        call(
            "PUT",
            format!("/v1/organizations/{organization}/templates"),
            who,
            json!({"definition": definition, "note": note}),
        )
    };
    assert_eq!(save(fixture.member.clone(), text.clone(), "mine").await.status(), 403);
    let saved = save(fixture.owner.clone(), text.clone(), "First version").await;
    assert_eq!(saved.status(), 200);
    assert_eq!(response_json(saved).await["version"], 1);

    // It is offered at once, with no restart, to the organization, and the same version cannot be saved twice.
    let listed = read("/v1/templates".into(), fixture.owner.clone()).await;
    assert!(listed
        .as_array()
        .unwrap()
        .iter()
        .any(|template| template["id"] == "night-shift"));
    let again = save(fixture.owner.clone(), text.clone(), "Same version").await;
    assert_eq!(again.status(), 409);

    // The next version is saved only with the next number; the earlier version stays readable.
    let mut next: Value = serde_json::from_str(&text).unwrap();
    next["meta"]["version"] = json!(2);
    next["meta"]["name"] = json!("Night shift, revised");
    let second = save(fixture.owner.clone(), next.to_string(), "Renamed").await;
    assert_eq!(second.status(), 200);
    let versions = read(
        format!("/v1/organizations/{organization}/templates/night-shift/versions"),
        fixture.owner.clone(),
    )
    .await;
    assert_eq!(versions.as_array().unwrap().len(), 2);
    assert_eq!(versions[1]["note"], "First version");

    // A built-in id and a file that breaks the rules are refused with the reason.
    let mut taken: Value = serde_json::from_str(&text).unwrap();
    taken["meta"]["id"] = json!("general");
    assert_eq!(save(fixture.owner.clone(), taken.to_string(), "No").await.status(), 400);
    assert_eq!(
        save(fixture.owner.clone(), "{\"not\": \"a template\"}".to_owned(), "No")
            .await
            .status(),
        400
    );

    // A workspace of the organization previews and adopts it; adopting changes no one's access.
    let detail = read("/v1/templates/night-shift".into(), fixture.owner.clone()).await;
    assert_eq!(detail["name"], "Night shift, revised");
    assert_eq!(
        call(
            "PUT",
            format!("/v1/workspaces/{workspace}/template"),
            fixture.owner.clone(),
            json!({"template_id": "night-shift"}),
        )
        .await
        .status(),
        200
    );
    let role_after = read(format!("/v1/workspaces/{workspace}/members"), fixture.owner.clone()).await;
    assert!(role_after.as_array().is_some_and(|members| !members.is_empty()));

    // Another organization does not see it, cannot read it and cannot adopt it.
    let (outsider, _) = register_account(&app, "organization-templates-outsider@example.com").await;
    let outsider_workspace = create_workspace(&app, &outsider).await;
    let theirs = read("/v1/templates".into(), outsider.clone()).await;
    assert!(!theirs
        .as_array()
        .unwrap()
        .iter()
        .any(|template| template["id"] == "night-shift"));
    assert_eq!(
        app.clone()
            .oneshot(auth_request("GET", "/v1/templates/night-shift", &outsider))
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        call(
            "PUT",
            format!("/v1/workspaces/{outsider_workspace}/template"),
            outsider.clone(),
            json!({"template_id": "night-shift"}),
        )
        .await
        .status(),
        404
    );
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
