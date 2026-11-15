//! Folder move preview (V0.6.24): before a folder is moved, the person sees what is inside it, what stays attached, where it
//! ends up and, when the move would be refused, why; the move itself keeps citations and comments on their files.

use super::knowledge_fixture::knowledge_fixture;
use super::*;

#[tokio::test]
async fn a_folder_move_is_previewed_with_what_stays_attached_and_refusals_are_explained_first() {
    let fixture = knowledge_fixture("folder-preview").await;
    let app = fixture.app.clone();
    let workspace = fixture.workspace_id.clone();
    let call = |method: &'static str, uri: String, who: String, body: Value| {
        let app = app.clone();
        async move { app.oneshot(json_request(method, &uri, &who, body)).await.unwrap() }
    };
    let read = |uri: String, who: String| {
        let app = app.clone();
        async move { app.oneshot(auth_request("GET", &uri, &who)).await.unwrap() }
    };
    let make_folder = |name: &'static str, parent: Option<String>| {
        let call = &call;
        let uri = format!("/v1/workspaces/{workspace}/folders");
        let owner = fixture.owner.clone();
        async move {
            let created = call("POST", uri, owner, json!({"name": name, "parent_id": parent})).await;
            assert_eq!(created.status(), 201, "creating {name}");
            response_json(created).await["id"].as_str().unwrap().to_owned()
        }
    };
    let archive = make_folder("Archive", None).await;
    let policies = make_folder("Policies", None).await;
    let drafts = make_folder("Drafts", Some(policies.clone())).await;

    // The cited file goes into Drafts, and it has a comment.
    let moved = call(
        "POST",
        format!("/v1/workspaces/{workspace}/artifacts/bulk"),
        fixture.owner.clone(),
        json!({"action": "move", "artifact_ids": [fixture.stale_id], "folder_id": drafts}),
    )
    .await;
    assert_eq!(moved.status(), 200);
    assert_eq!(
        call(
            "POST",
            format!("/v1/artifacts/{}/comments", fixture.stale_id),
            fixture.owner.clone(),
            json!({"body": "Check this before the audit."}),
        )
        .await
        .status(),
        201
    );

    // Policies holds Drafts, one file, one citing card and one comment; moving it under Archive changes none of that.
    let preview = response_json(
        read(
            format!("/v1/workspaces/{workspace}/folders/{policies}/move-preview?parent_id={archive}"),
            fixture.owner.clone(),
        )
        .await,
    )
    .await;
    assert_eq!(preview["subfolders"], 1, "{preview}");
    assert_eq!(preview["files"], 1);
    assert_eq!(preview["citing_cards"], 1);
    assert_eq!(preview["citations"], 1);
    assert_eq!(preview["comments"], 1);
    assert_eq!(preview["from_path"], "Policies");
    assert_eq!(preview["to_path"], "Archive / Policies");
    assert_eq!(preview["allowed"], true);

    // A cycle and the folder's own place are told apart from a move that would work.
    let cycle = response_json(
        read(
            format!("/v1/workspaces/{workspace}/folders/{policies}/move-preview?parent_id={drafts}"),
            fixture.owner.clone(),
        )
        .await,
    )
    .await;
    assert_eq!(cycle["allowed"], false);
    assert!(cycle["refusal"].as_str().unwrap().contains("inside it"), "{cycle}");
    let same = response_json(
        read(
            format!("/v1/workspaces/{workspace}/folders/{drafts}/move-preview?parent_id={policies}"),
            fixture.owner.clone(),
        )
        .await,
    )
    .await;
    assert_eq!(same["unchanged"], true);

    // Previewing writes nothing; the move keeps the file's citation and comment on it.
    let still = response_json(read(format!("/v1/workspaces/{workspace}/folders"), fixture.owner.clone()).await).await;
    assert!(still
        .as_array()
        .unwrap()
        .iter()
        .any(|folder| folder["id"] == policies.as_str() && folder["parent_id"].is_null()));
    assert_eq!(
        call(
            "POST",
            format!("/v1/workspaces/{workspace}/folders/{policies}/move"),
            fixture.owner.clone(),
            json!({"parent_id": archive}),
        )
        .await
        .status(),
        200
    );
    let card = response_json(
        read(
            format!("/v1/memory-cards/{}", fixture.card_on_stale_id),
            fixture.owner.clone(),
        )
        .await,
    )
    .await;
    assert_eq!(
        card["evidence"].as_array().unwrap().len(),
        1,
        "the citation stayed attached"
    );
    let comments = response_json(
        read(
            format!("/v1/artifacts/{}/comments", fixture.stale_id),
            fixture.owner.clone(),
        )
        .await,
    )
    .await;
    assert_eq!(comments.as_array().unwrap().len(), 1, "the comment stayed on its file");

    // The preview is for people who can edit; a guest who can only read is refused.
    assert_eq!(
        read(
            format!("/v1/workspaces/{workspace}/folders/{policies}/move-preview"),
            fixture.guest.clone()
        )
        .await
        .status(),
        403
    );
    let _ = std::fs::remove_dir_all(&fixture.data_dir);
}
