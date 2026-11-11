//! The representative workspace of the 0.6 line (V0.6.0): no AI provider, no mail, one piece of evidence whose
//! lifecycle says it is outdated, one whose file was deleted, a memory card citing each, and a guest. Later 0.6
//! releases build on `knowledge_fixture` so their tests start from the same, named situation.

use super::*;

pub(super) struct KnowledgeFixture {
    pub app: axum::Router,
    pub data_dir: std::path::PathBuf,
    pub owner: String,
    pub member: String,
    pub guest: String,
    pub workspace_id: String,
    pub current_id: String,
    pub stale_id: String,
    pub card_on_stale_id: String,
    pub card_on_missing_id: String,
}

pub(super) async fn knowledge_fixture(label: &str) -> KnowledgeFixture {
    let data_dir = temp_data_dir(label);
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    // The first account administers the server; nobody in the workspace is that account.
    register_account(&app, &format!("{label}-root@example.com")).await;
    let (owner, _) = register_account(&app, &format!("{label}-owner@example.com")).await;
    let (member, _) = register_account(&app, &format!("{label}-member@example.com")).await;
    let (guest, _) = register_account(&app, &format!("{label}-guest@example.com")).await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { response_json(app.oneshot(json_request(method, &uri, &who, body)).await.unwrap()).await }
    };
    // The member joins the organization's shared workspace; the guest is added to this workspace only.
    let organization = call("GET", "/v1/organizations".into(), owner.clone(), Value::Null).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    join(
        &app,
        &owner,
        "organizations",
        &organization,
        &format!("{label}-member@example.com"),
        "member",
    )
    .await;
    join(
        &app,
        &owner,
        "workspaces",
        &workspace_id,
        &format!("{label}-guest@example.com"),
        "viewer",
    )
    .await;

    let mut ids = Vec::new();
    for (title, content) in [
        ("Deploy checklist", "Deploys run on Tuesdays after the migration check."),
        ("Refund policy", "Refunds are issued within fourteen days of purchase."),
        ("Retired runbook", "Restart the worker by hand when the queue stalls."),
    ] {
        let stored = call(
            "POST",
            format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            owner.clone(),
            json!({"title": title, "content": content}),
        )
        .await;
        ids.push(stored["id"].as_str().unwrap().to_owned());
    }
    wait_for_indexed_artifacts(&app, &owner, &workspace_id, 3).await;
    let (current_id, stale_id, missing_id) = (ids[0].clone(), ids[1].clone(), ids[2].clone());

    let card = |title: &str, artifact_id: &str| {
        json!({
            "title": title, "body_markdown": format!("{title}."), "source": "fixture",
            "citations": [{"artifact_id": artifact_id, "chunk_id": null, "title": title, "path": "x",
                "start_line": null, "end_line": null, "confidence": null}],
        })
    };
    let on_stale = call(
        "POST",
        format!("/v1/workspaces/{workspace_id}/memory-cards"),
        owner.clone(),
        card("Refund window", &stale_id),
    )
    .await;
    let on_missing = call(
        "POST",
        format!("/v1/workspaces/{workspace_id}/memory-cards"),
        owner.clone(),
        card("Worker restart", &missing_id),
    )
    .await;
    // Stale evidence: retired by its lifecycle. Missing source: the file is deleted after it was cited.
    let retired = app
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/v1/artifacts/{stale_id}/lifecycle"),
            &owner,
            json!({"status": "outdated", "review_note": "Replaced by the new policy."}),
        ))
        .await
        .unwrap();
    assert_eq!(retired.status(), 200);
    let removed = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/artifacts/bulk"),
            &owner,
            json!({"action": "delete", "artifact_ids": [missing_id]}),
        ))
        .await
        .unwrap();
    assert_eq!(removed.status(), 200);

    KnowledgeFixture {
        app,
        data_dir,
        owner,
        member,
        guest,
        workspace_id,
        current_id,
        stale_id,
        card_on_stale_id: on_stale["id"].as_str().unwrap().to_owned(),
        card_on_missing_id: on_missing["id"].as_str().unwrap().to_owned(),
    }
}

#[tokio::test]
async fn the_knowledge_fixture_has_stale_and_missing_evidence_a_guest_and_no_ai_or_mail() {
    let fixture = knowledge_fixture("knowledge-fixture").await;
    let get = |uri: String, who: &str| {
        let app = fixture.app.clone();
        let request = auth_request("GET", &uri, who);
        async move { app.oneshot(request).await.unwrap() }
    };
    let workspace_id = &fixture.workspace_id;
    // The cards: the one on retired evidence needs another look.
    let cards = response_json(get(format!("/v1/workspaces/{workspace_id}/memory-cards"), &fixture.owner).await).await;
    let find = |id: &str| cards.as_array().unwrap().iter().find(|card| card["id"] == id).cloned();
    assert_eq!(find(&fixture.card_on_stale_id).unwrap()["needs_review"], true);
    assert!(
        find(&fixture.card_on_missing_id).is_some(),
        "a card outlives a deleted source"
    );
    // The current evidence is still there, searchable without any AI provider.
    let search = response_json(
        fixture
            .app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/search"),
                &fixture.owner,
                json!({"query": "deploys"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(search[0]["artifact_id"], fixture.current_id);
    let retired = response_json(
        fixture
            .app
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/v1/workspaces/{workspace_id}/search"),
                &fixture.owner,
                json!({"query": "refunds", "lifecycle_statuses": ["outdated"]}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(retired[0]["artifact_id"], fixture.stale_id);
    // The member and the guest can both read; the guest is a viewer of this workspace only.
    for who in [&fixture.member, &fixture.guest] {
        assert_eq!(
            get(format!("/v1/workspaces/{workspace_id}/memory-cards"), who)
                .await
                .status(),
            200
        );
    }
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
