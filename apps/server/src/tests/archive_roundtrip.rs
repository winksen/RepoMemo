//! The archive journey for knowledge (V0.6.28): history, recorded content, review policy, templates and recovery state go out
//! and come back with the same meaning, an approval is always a record and never a local decision, and what an archive does
//! not carry is said.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

async fn archive_of(app: &axum::Router, who: &str, workspace: &str) -> Vec<u8> {
    to_bytes(
        app.clone()
            .oneshot(auth_request("GET", &format!("/v1/workspaces/{workspace}/export"), who))
            .await
            .unwrap()
            .into_body(),
        usize::MAX,
    )
    .await
    .unwrap()
    .to_vec()
}

fn workspace_json(archive: &[u8]) -> Value {
    let entries: std::collections::HashMap<String, Vec<u8>> = repomemo_api::read_archive_entries(archive, 1 << 26)
        .unwrap()
        .into_iter()
        .collect();
    serde_json::from_slice(&entries["workspace.json"]).unwrap()
}

#[tokio::test]
async fn a_workspace_leaves_and_returns_with_its_history_policy_and_template_and_says_what_it_leaves_behind() {
    let fixture = knowledge_fixture("archive-roundtrip").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let card = fixture.card_on_stale_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let read = |uri: String, who: String| {
        let app = app.clone();
        async move { response_json(app.oneshot(auth_request("GET", &uri, &who)).await.unwrap()).await }
    };
    let import = |archive: Vec<u8>, organization: String, who: String| {
        let app = app.clone();
        async move {
            let response = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/v1/workspaces/import?organization_id={organization}"))
                        .header("authorization", who)
                        .header("content-type", "application/gzip")
                        .body(Body::from(archive))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), 201);
            response_json(response).await
        }
    };
    let organization = read("/v1/organizations".into(), fixture.owner.clone()).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Knowledge: a second revision, an approval of it by someone else, the rule and a review date; a template of the
    // organization's own, adopted; and a deleted card sitting in the trash.
    assert_eq!(
        call(
            "PUT",
            format!("/v1/memory-cards/{card}"),
            fixture.owner.clone(),
            json!({"title": "Refund window", "body_markdown": "Fourteen days, in writing.", "source": "fixture"})
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        call(
            "POST",
            format!("/v1/memory-cards/{card}/approval"),
            fixture.member.clone(),
            json!({"revision": 2, "note": "Checked."})
        )
        .await
        .status(),
        200
    );
    for (uri, body) in [
        (
            format!("/v1/workspaces/{workspace}/approval-policy"),
            json!({"independent_review": true}),
        ),
        (
            format!("/v1/workspaces/{workspace}/review-assignment"),
            json!({"kind": "memory_card", "id": card, "reviewer_user_id": null, "next_review_at": "2030-01-01T00:00:00+00:00"}),
        ),
    ] {
        assert_eq!(call("PUT", uri, fixture.owner.clone(), body).await.status(), 200);
    }
    let file = app
        .clone()
        .oneshot(auth_request(
            "GET",
            &format!("/v1/workspaces/{workspace}/template/file?id=night-shift&name=Night%20shift"),
            &fixture.owner,
        ))
        .await
        .unwrap();
    let template = String::from_utf8(to_bytes(file.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
    assert_eq!(
        call(
            "PUT",
            format!("/v1/organizations/{organization}/templates"),
            fixture.owner.clone(),
            json!({"definition": template, "note": "First"})
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        call(
            "PUT",
            format!("/v1/workspaces/{workspace}/template"),
            fixture.owner.clone(),
            json!({"template_id": "night-shift"})
        )
        .await
        .status(),
        200
    );
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

    // The first archive says what it does not carry, and carries the rest.
    let first = archive_of(&app, &fixture.owner, &workspace).await;
    let first_data = workspace_json(&first);
    assert_eq!(first_data["trash_left_out"], 1);
    assert_eq!(first_data["workspace"]["settings"]["independent_review"], true);

    // Imported into the same organization: history, recorded content and review come back, the template stays, an approval
    // is a record, and the report warns about the trash.
    let report = import(first.clone(), organization.clone(), fixture.owner.clone()).await;
    assert!(report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning.as_str().unwrap().contains("trash")));
    let moved = report["workspace"]["workspace"]["id"].as_str().unwrap().to_owned();
    let template_view = read(format!("/v1/workspaces/{moved}/template"), fixture.owner.clone()).await;
    assert_eq!(template_view["source"], "workspace");
    assert_eq!(template_view["template"]["id"], "night-shift");
    let cards = read(format!("/v1/workspaces/{moved}/memory-cards"), fixture.owner.clone()).await;
    let new_card = cards
        .as_array()
        .unwrap()
        .iter()
        .find(|found| found["title"] == "Refund window")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let approval = read(format!("/v1/memory-cards/{new_card}/approval"), fixture.owner.clone()).await;
    assert_eq!(
        approval["approved"], false,
        "an imported decision approves nothing here"
    );
    assert_eq!(approval["imported"].as_array().unwrap().len(), 1);
    assert_eq!(approval["independent_review"], true);

    // A second round trip keeps the same meaning: the same revisions with the same recorded content, the imported approval
    // as a record, and the review date.
    let second_data = workspace_json(&archive_of(&app, &fixture.owner, &moved).await);
    let find = |data: &Value| {
        data["memory_cards"]
            .as_array()
            .unwrap()
            .iter()
            .find(|found| found["title"] == "Refund window")
            .unwrap()
            .clone()
    };
    let (before, after) = (find(&first_data), find(&second_data));
    let summary = |card: &Value| -> Vec<(i64, String, Option<String>)> {
        card["revisions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|revision| {
                (
                    revision["revision"].as_i64().unwrap(),
                    revision["body_markdown"].as_str().unwrap().to_owned(),
                    revision["citations"][0]["content_hash"].as_str().map(str::to_owned),
                )
            })
            .collect()
    };
    assert_eq!(
        summary(&before),
        summary(&after),
        "history and recorded content survive a second trip"
    );
    assert_eq!(after["approvals"].as_array().unwrap().len(), 1);
    assert_eq!(after["approvals"][0]["imported"], true);
    assert_eq!(before["review"]["next_review_at"], after["review"]["next_review_at"]);

    // Into another organization: its own template is not carried across, the person is told, and nothing else changes.
    let (outsider, _) = register_account(&app, "archive-roundtrip-outsider@example.com").await;
    let _ = create_workspace(&app, &outsider).await;
    let their_organization = read("/v1/organizations".into(), outsider.clone()).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let elsewhere = import(first, their_organization, outsider.clone()).await;
    assert!(elsewhere["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning.as_str().unwrap().contains("night-shift")));
    let theirs = elsewhere["workspace"]["workspace"]["id"].as_str().unwrap().to_owned();
    let their_template = read(format!("/v1/workspaces/{theirs}/template"), outsider.clone()).await;
    assert_ne!(their_template["template"]["id"], "night-shift");
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
