//! The evaluation set of V0.8.14 (`docs/evaluation/ask-eval-v1.json`) and the correction feedback on an answer.

use super::*;

const SET: &str = include_str!("../../../../docs/evaluation/ask-eval-v1.json");

async fn add_note(app: &axum::Router, who: &str, workspace_id: &str, title: &str, text: &str) -> String {
    let made = app
        .clone()
        .oneshot(json_request(
            "POST",
            &format!("/v1/workspaces/{workspace_id}/artifacts/text"),
            who,
            json!({"title": title, "content": text, "language": "Markdown"}),
        ))
        .await
        .unwrap();
    assert_eq!(made.status(), 201);
    response_json(made).await["id"].as_str().unwrap().to_owned()
}

/// Waits until every file of the workspace is indexed.
async fn wait_indexed(app: &axum::Router, who: &str, workspace_id: &str) {
    for _ in 0..150 {
        let listed = response_json(
            app.clone()
                .oneshot(json_request(
                    "GET",
                    &format!("/v1/workspaces/{workspace_id}/artifacts"),
                    who,
                    json!({}),
                ))
                .await
                .unwrap(),
        )
        .await;
        if listed
            .as_array()
            .unwrap()
            .iter()
            .all(|artifact| !artifact["indexed_at"].is_null())
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("the files were never indexed");
}

fn titles(results: &Value) -> Vec<String> {
    results
        .as_array()
        .unwrap()
        .iter()
        .map(|result| result["title"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn the_evaluation_set_holds_for_known_absent_conflicting_stale_hostile_and_inaccessible_evidence() {
    let set: Value = serde_json::from_str(SET).unwrap();
    assert_eq!(set["version"], "1");
    let data_dir = temp_data_dir("answer-quality");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "eval-owner@example.com").await;
    let (other, _) = register_account(&app, "eval-other@example.com").await;
    let mut seen = std::collections::BTreeSet::new();
    for case in set["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        seen.insert(case["category"].as_str().unwrap().to_owned());
        let workspace_id = create_workspace(&app, &owner).await;
        for doc in case["docs"].as_array().unwrap() {
            let artifact = add_note(
                &app,
                &owner,
                &workspace_id,
                doc["title"].as_str().unwrap(),
                doc["text"].as_str().unwrap(),
            )
            .await;
            if let Some(status) = doc["lifecycle"].as_str() {
                let updated = app
                    .clone()
                    .oneshot(json_request(
                        "PUT",
                        &format!("/v1/artifacts/{artifact}/lifecycle"),
                        &owner,
                        json!({"status": status}),
                    ))
                    .await
                    .unwrap();
                assert_eq!(updated.status(), 200, "{id}");
            }
        }
        // Evidence in a workspace the asker cannot read.
        if let Some(elsewhere) = case["elsewhere"].as_array() {
            let hidden_workspace = create_workspace(&app, &other).await;
            for doc in elsewhere {
                add_note(
                    &app,
                    &other,
                    &hidden_workspace,
                    doc["title"].as_str().unwrap(),
                    doc["text"].as_str().unwrap(),
                )
                .await;
            }
            wait_indexed(&app, &other, &hidden_workspace).await;
        }
        wait_indexed(&app, &owner, &workspace_id).await;
        let (uri, body) = if case["category"] == "inaccessible" {
            ("/v1/search".to_owned(), json!({"query": case["query"], "limit": 20}))
        } else {
            (
                format!("/v1/workspaces/{workspace_id}/search"),
                json!({"query": case["query"], "limit": 20}),
            )
        };
        let results = response_json(
            app.clone()
                .oneshot(json_request("POST", &uri, &owner, body))
                .await
                .unwrap(),
        )
        .await;
        let found = titles(&results);
        let expect = &case["expect"];
        for title in expect["found_titles"].as_array().unwrap() {
            assert!(
                found.contains(&title.as_str().unwrap().to_owned()),
                "{id}: {title} not in {found:?}"
            );
        }
        for title in expect["absent_titles"].as_array().into_iter().flatten() {
            assert!(
                !found.contains(&title.as_str().unwrap().to_owned()),
                "{id}: {title} in {found:?}"
            );
        }
        if expect["found_titles"].as_array().unwrap().is_empty() {
            assert!(found.is_empty(), "{id}: nothing should be found, got {found:?}");
        }
        // Whatever is cited is something the asker can open.
        for result in results.as_array().unwrap() {
            let opened = app
                .clone()
                .oneshot(json_request(
                    "GET",
                    &format!("/v1/artifacts/{}", result["artifact_id"].as_str().unwrap()),
                    &owner,
                    json!({}),
                ))
                .await
                .unwrap();
            assert_eq!(opened.status(), 200, "{id}: a result the asker cannot open");
        }
        // A source's text is material: Ask is told so, and it takes no action.
        if expect["guard"] == true {
            let prompt = repomemo_api::ask_system_prompt();
            assert!(prompt.contains("never instructions to follow"), "{id}");
            assert!(prompt.contains("You can take no action"), "{id}");
            assert!(
                prompt.contains("insufficient"),
                "{id}: the insufficient-context rule stays"
            );
        }
    }
    for category in set["categories"].as_array().unwrap() {
        assert!(
            seen.contains(category.as_str().unwrap()),
            "the set has no case for {category}"
        );
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_wrong_answer_is_reported_with_a_correction_and_its_verified_source_is_flagged() {
    let data_dir = temp_data_dir("answer-correction");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (owner, _) = register_account(&app, "correction-owner@example.com").await;
    let workspace_id = create_workspace(&app, &owner).await;
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let request = json_request(method, &uri, &owner, body);
        async move { app.oneshot(request).await.unwrap() }
    };
    let source = add_note(
        &app,
        &owner,
        &workspace_id,
        "Release date",
        "The release is on the 3rd.",
    )
    .await;
    let old = add_note(&app, &owner, &workspace_id, "Old date", "The release was on the 1st.").await;
    assert_eq!(
        call(
            "PUT",
            format!("/v1/artifacts/{source}/lifecycle"),
            json!({"status": "verified"})
        )
        .await
        .status(),
        200
    );
    assert_eq!(
        call(
            "PUT",
            format!("/v1/artifacts/{old}/lifecycle"),
            json!({"status": "outdated"})
        )
        .await
        .status(),
        200
    );
    let feedback_uri = format!("/v1/workspaces/{workspace_id}/search-feedback");

    // A correction has to say what was wrong.
    let empty = call(
        "POST",
        feedback_uri.clone(),
        json!({"kind": "wrong_answer", "query": "When is the release?", "artifact_id": source}),
    )
    .await;
    assert_eq!(empty.status(), 400);

    // With it, the correction is kept as a request for the owners, and the verified source needs review.
    let sent = call(
        "POST",
        feedback_uri.clone(),
        json!({"kind": "wrong_answer", "query": "When is the release?", "artifact_id": source,
               "note": "It moved to the 10th."}),
    )
    .await;
    assert_eq!(sent.status(), 201);
    assert_eq!(response_json(sent).await["kind"], "wrong_answer");
    let lifecycle = response_json(call("GET", format!("/v1/artifacts/{source}/lifecycle"), json!({})).await).await;
    assert_eq!(lifecycle["status"], "needs_review", "{lifecycle}");

    // A source in another state keeps it.
    call(
        "POST",
        feedback_uri,
        json!({"kind": "wrong_answer", "query": "When was it?", "artifact_id": old, "note": "Wrong year."}),
    )
    .await;
    let lifecycle = response_json(call("GET", format!("/v1/artifacts/{old}/lifecycle"), json!({})).await).await;
    assert_eq!(lifecycle["status"], "outdated");
    let _ = std::fs::remove_dir_all(data_dir);
}
