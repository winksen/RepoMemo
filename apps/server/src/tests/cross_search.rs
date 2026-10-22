use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// A stand-in for Ollama that answers embeddings (every text the same vector, so meaning matches)
/// and counts the calls it gets.
async fn start_counting_ollama() -> (String, Arc<AtomicUsize>) {
    use axum::routing::{get, post};
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let app = axum::Router::new()
        .route(
            "/api/tags",
            get(|| async { axum::Json(json!({"models": [{"name": "mock-embed"}]})) }),
        )
        .route(
            "/api/embed",
            post(move |axum::Json(body): axum::Json<Value>| {
                let counted = counted.clone();
                async move {
                    counted.fetch_add(1, Ordering::SeqCst);
                    let vectors = body["input"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|_| json!([1.0, 0.0, 0.1]))
                        .collect::<Vec<_>>();
                    axum::Json(json!({ "embeddings": vectors }))
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), calls)
}

/// V0.2.158: a search across workspaces embeds the query once for the provider they share, and the
/// answer is the same one a workspace-by-workspace search would give.
#[tokio::test]
async fn a_search_across_workspaces_embeds_the_query_once_per_provider() {
    let (ollama_url, embed_calls) = start_counting_ollama().await;
    let data_dir = temp_data_dir("cross-search-once");
    let app = router(ServerConfig::for_test(data_dir.clone())).await.unwrap();
    let (_, _) = register_account(&app, "cross-admin@example.com").await;
    let (owner, _) = register_account(&app, "cross-owner@example.com").await;
    let call = |method: &'static str, uri: String, body: Value| {
        let app = app.clone();
        let owner = owner.clone();
        async move { app.oneshot(json_request(method, &uri, &owner, body)).await.unwrap() }
    };

    let mut workspaces = Vec::new();
    for index in 0..5 {
        let workspace = create_workspace(&app, &owner).await;
        let note = call(
            "POST",
            format!("/v1/workspaces/{workspace}/artifacts/text"),
            json!({"title": format!("Runbook {index}"), "content": format!("Steps to ship release {index}."), "language": "Markdown"}),
        )
        .await;
        assert_eq!(note.status(), 201);
        let saved = call(
            "PUT",
            format!("/v1/workspaces/{workspace}/ai-providers"),
            json!({"provider_type": "ollama", "name": "Mock embeddings", "base_url": ollama_url, "model": "mock-embed", "enabled": true, "purpose": "embedding"}),
        )
        .await;
        assert_eq!(saved.status(), 200);
        workspaces.push(workspace);
    }
    for workspace in &workspaces {
        let mut embedded = false;
        for _ in 0..100 {
            let metrics =
                response_json(call("GET", format!("/v1/workspaces/{workspace}/metrics"), json!({})).await).await;
            if metrics["embedded_chunk_count"]
                .as_i64()
                .is_some_and(|count| count > 0 && Some(count) == metrics["chunk_count"].as_i64())
            {
                embedded = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(embedded, "chunks were not embedded in time");
    }

    let before = embed_calls.load(Ordering::SeqCst);
    let found = response_json(call("POST", "/v1/search".to_owned(), json!({"query": "shipping"})).await).await;
    assert_eq!(
        embed_calls.load(Ordering::SeqCst) - before,
        1,
        "five workspaces on one provider embed the query once"
    );
    let listed = found["workspaces"].as_array().unwrap();
    assert_eq!(listed.len(), 5, "{found}");
    assert!(listed
        .iter()
        .all(|entry| entry["by_meaning"] == true && entry["result_count"] == 1));

    // The merged list is what searching each workspace on its own gives, in the order the merge sets.
    let mut alone = Vec::new();
    for workspace in &workspaces {
        let one = response_json(
            call(
                "POST",
                "/v1/search".to_owned(),
                json!({"query": "shipping", "workspace_ids": [workspace]}),
            )
            .await,
        )
        .await;
        alone.push(one["results"][0]["result"]["title"].as_str().unwrap().to_owned());
    }
    let mut together: Vec<String> = found["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["result"]["title"].as_str().unwrap().to_owned())
        .collect();
    together.sort();
    alone.sort();
    assert_eq!(together, alone);
    let _ = std::fs::remove_dir_all(data_dir);
}
