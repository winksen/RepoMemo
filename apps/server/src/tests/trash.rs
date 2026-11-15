//! Trash (V0.6.15): a deleted card comes back with its history, a deleted uploaded file comes back with the citations
//! that pointed at it, and purging is an administrator's explicit, recorded act.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_deleted_card_and_file_come_back_with_their_history_and_citations_and_purge_is_explicit() {
    let fixture = knowledge_fixture("trash").await;
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
    let delete = |uri: String, who: String| {
        let app = app.clone();
        async move { app.oneshot(auth_request("DELETE", &uri, &who)).await.unwrap().status() }
    };

    // Give the card a second revision and an approval, then delete it.
    assert_eq!(
        call(
            "PUT",
            format!("/v1/memory-cards/{card}"),
            fixture.owner.clone(),
            json!({"title": "Refund window", "body_markdown": "Fourteen days, revised.", "source": "fixture"}),
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
            json!({"revision": 2, "note": "Checked."}),
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        delete(format!("/v1/memory-cards/{card}"), fixture.owner.clone()).await,
        204
    );
    assert_eq!(
        read(format!("/v1/memory-cards/{card}"), fixture.owner.clone()).await["card"],
        Value::Null,
        "the card is gone from the workspace"
    );

    // It is in the trash for editors and not for a guest, and it comes back as it was.
    let trash = read(format!("/v1/workspaces/{workspace}/trash"), fixture.owner.clone()).await;
    let item = trash
        .as_array()
        .unwrap()
        .iter()
        .find(|found| found["kind"] == "memory_card")
        .expect("the card is in the trash")
        .clone();
    assert_eq!(item["title"], "Refund window");
    assert_eq!(
        app.clone()
            .oneshot(auth_request(
                "GET",
                &format!("/v1/workspaces/{workspace}/trash"),
                &fixture.guest
            ))
            .await
            .unwrap()
            .status(),
        403
    );
    let trash_id = item["id"].as_str().unwrap();
    let restored = call(
        "POST",
        format!("/v1/workspaces/{workspace}/trash/{trash_id}/restore"),
        fixture.owner.clone(),
        Value::Null,
    )
    .await;
    assert_eq!(restored.status(), 200);
    let restored = response_json(restored).await;
    assert_eq!(restored["item_id"], card.as_str());
    let revisions = read(format!("/v1/memory-cards/{card}/revisions"), fixture.owner.clone()).await;
    assert_eq!(revisions.as_array().unwrap().len(), 2, "{revisions}");
    assert_eq!(revisions[0]["citations"].as_array().unwrap().len(), 1);
    let approval = read(format!("/v1/memory-cards/{card}/approval"), fixture.owner.clone()).await;
    assert_eq!(
        approval["approved"], true,
        "the approval of revision 2 came back with it"
    );
    let back = read(format!("/v1/memory-cards/{card}"), fixture.owner.clone()).await;
    assert_eq!(back["evidence"].as_array().unwrap().len(), 1);

    // A deleted uploaded file comes back and the card cites it again; an item cannot be restored twice.
    assert_eq!(
        delete(format!("/v1/artifacts/{}", fixture.stale_id), fixture.owner.clone()).await,
        204
    );
    let cited = read(format!("/v1/memory-cards/{card}"), fixture.owner.clone()).await;
    assert_eq!(
        cited["evidence"].as_array().unwrap().len(),
        0,
        "the citation went with the file"
    );
    let trash = read(format!("/v1/workspaces/{workspace}/trash"), fixture.owner.clone()).await;
    let file = trash
        .as_array()
        .unwrap()
        .iter()
        .find(|found| found["kind"] == "artifact")
        .expect("the file is in the trash")
        .clone();
    let file_id = file["id"].as_str().unwrap().to_owned();
    // Restoring needs edit access; purging needs an administrator.
    assert_eq!(
        call(
            "POST",
            format!("/v1/workspaces/{workspace}/trash/{file_id}/restore"),
            fixture.guest.clone(),
            Value::Null
        )
        .await
        .status(),
        403
    );
    assert_eq!(
        delete(
            format!("/v1/workspaces/{workspace}/trash/{file_id}"),
            fixture.member.clone()
        )
        .await,
        403
    );
    let restored = call(
        "POST",
        format!("/v1/workspaces/{workspace}/trash/{file_id}/restore"),
        fixture.owner.clone(),
        Value::Null,
    )
    .await;
    assert_eq!(restored.status(), 200);
    let cited = read(format!("/v1/memory-cards/{card}"), fixture.owner.clone()).await;
    assert_eq!(cited["evidence"].as_array().unwrap().len(), 1, "the citation is back");
    let again = call(
        "POST",
        format!("/v1/workspaces/{workspace}/trash/{file_id}/restore"),
        fixture.owner.clone(),
        Value::Null,
    )
    .await;
    assert_eq!(again.status(), 404);

    // Purge is explicit: an administrator removes the item and it can no longer be restored.
    assert_eq!(
        delete(format!("/v1/memory-cards/{card}"), fixture.owner.clone()).await,
        204
    );
    let trash = read(format!("/v1/workspaces/{workspace}/trash"), fixture.owner.clone()).await;
    let trash_id = trash[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(
        delete(
            format!("/v1/workspaces/{workspace}/trash/{trash_id}"),
            fixture.owner.clone()
        )
        .await,
        204
    );
    let gone = call(
        "POST",
        format!("/v1/workspaces/{workspace}/trash/{trash_id}/restore"),
        fixture.owner.clone(),
        Value::Null,
    )
    .await;
    assert_eq!(gone.status(), 404);
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
