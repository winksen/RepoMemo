//! Knowledge in workspace archives (V0.6.14): revisions, the content their citations recorded, approvals as records,
//! the review assignment and the independent-review setting travel with an export, and an import never turns a
//! decision made elsewhere into one made here.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn an_archive_carries_a_cards_history_and_review_and_returns_approvals_as_records() {
    let fixture = knowledge_fixture("knowledge-archive").await;
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

    // Revision 2 (an edit), an approval of it, a due review and the independent-review rule.
    let edited = call(
        "PUT",
        format!("/v1/memory-cards/{card}"),
        fixture.owner.clone(),
        json!({"title": "Refund window, revised", "body_markdown": "Refunds take fourteen days.", "source": "fixture"}),
    )
    .await;
    assert_eq!(edited.status(), 200);
    assert_eq!(
        call(
            "POST",
            format!("/v1/memory-cards/{card}/approval"),
            fixture.member.clone(),
            json!({"revision": 2, "note": "Read against the policy."}),
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        call(
            "PUT",
            format!("/v1/workspaces/{workspace}/approval-policy"),
            fixture.owner.clone(),
            json!({"independent_review": true}),
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        call(
            "PUT",
            format!("/v1/workspaces/{workspace}/review-assignment"),
            fixture.owner.clone(),
            json!({"kind": "memory_card", "id": card, "reviewer_user_id": null, "next_review_at": "2030-01-01T00:00:00+00:00"}),
        )
        .await
        .status(),
        200
    );

    // The archive is format 3 and counts what it carries.
    let archive = to_bytes(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace}/export"),
                &fixture.owner,
            ))
            .await
            .unwrap()
            .into_body(),
        usize::MAX,
    )
    .await
    .unwrap()
    .to_vec();
    let entries: std::collections::HashMap<String, Vec<u8>> =
        repomemo_api::read_archive_entries(archive.as_slice(), 1 << 26)
            .unwrap()
            .into_iter()
            .collect();
    let manifest: Value = serde_json::from_slice(&entries["manifest.json"]).unwrap();
    assert_eq!(manifest["format_version"], 3);
    assert!(manifest["counts"]["revisions"].as_u64().unwrap() >= 3);
    assert_eq!(manifest["counts"]["approvals"], 1);
    assert_eq!(manifest["counts"]["reviews"], 1);
    let data = String::from_utf8_lossy(&entries["workspace.json"]).into_owned();
    assert!(!data.contains("user_id"), "nobody is named by account id");

    // The import reads it back: same history, the recorded content, the date and the rule, and no approval.
    let listed = read("/v1/organizations".into(), fixture.owner.clone()).await;
    let organization_id = listed[0]["id"].as_str().unwrap().to_owned();
    let imported = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/v1/workspaces/import?organization_id={organization_id}&name=Moved"
                ))
                .header("authorization", &fixture.owner)
                .header("content-type", "application/gzip")
                .body(Body::from(archive))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(imported.status(), 201);
    let report = response_json(imported).await;
    assert!(
        report["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("imported record")),
        "{report}"
    );
    let new_workspace = report["workspace"]["workspace"]["id"].as_str().unwrap().to_owned();
    let cards = read(
        format!("/v1/workspaces/{new_workspace}/memory-cards"),
        fixture.owner.clone(),
    )
    .await;
    let moved = cards
        .as_array()
        .unwrap()
        .iter()
        .find(|found| found["title"] == "Refund window, revised")
        .expect("the edited card came across")["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let revisions = read(format!("/v1/memory-cards/{moved}/revisions"), fixture.owner.clone()).await;
    let numbers: Vec<i64> = revisions
        .as_array()
        .unwrap()
        .iter()
        .map(|revision| revision["revision"].as_i64().unwrap())
        .collect();
    assert_eq!(numbers, [2, 1], "{revisions}");
    assert_eq!(revisions[1]["body_markdown"], "Refund window.");
    // The recorded content came with it, so a citation is judged against what it cited.
    assert!(revisions[1]["citations"][0]["content_hash"].as_str().unwrap().len() >= 32);
    // The author is a name, not an account here.
    assert!(revisions[0]["author"].is_null());
    assert!(revisions[0]["author_label"]
        .as_str()
        .unwrap()
        .contains("knowledge-archive-owner"));

    // The decision made before the move is a record; nothing is approved here, and the rule came along.
    let approval = read(format!("/v1/memory-cards/{moved}/approval"), fixture.owner.clone()).await;
    assert_eq!(approval["revision"], 2);
    assert_eq!(approval["approved"], false);
    assert_eq!(approval["history"].as_array().unwrap().len(), 0);
    assert_eq!(approval["imported"][0]["decision"], "approved");
    assert!(approval["imported"][0]["person"]
        .as_str()
        .unwrap()
        .contains("knowledge-archive-member"));
    assert_eq!(approval["independent_review"], true);
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
