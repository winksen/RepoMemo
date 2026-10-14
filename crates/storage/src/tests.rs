use super::{decode_embedding, encode_embedding, NewArtifact, StorageConfig, StorageEngine};
use repomemo_domain::{ArtifactType, Chunk, Citation, SearchRequest, SourceType, Symbol, SymbolKind};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn embedding_serialization_round_trips() {
    let values = vec![0.25_f32, -1.0, 3.5];
    assert_eq!(decode_embedding(&encode_embedding(&values)), Some(values));
    assert_eq!(decode_embedding(&[0, 1]), None);
}

#[test]
fn content_hash_is_stable_sha256_hex() {
    let hash = StorageEngine::content_hash(b"repomemo");

    assert_eq!(hash.len(), 64);
    assert_eq!(hash, StorageEngine::content_hash(b"repomemo"));
    assert_ne!(hash, StorageEngine::content_hash(b"RepoMemo"));
}

#[tokio::test]
async fn fts_search_returns_snippet_and_respects_language_filter() {
    let data_dir = std::env::temp_dir().join(format!(
        "repomemo-search-test-{}",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    let storage = StorageEngine::open(StorageConfig::new(data_dir.clone())).await.unwrap();
    let workspace = storage.create_workspace("Search test").await.unwrap();
    let source = storage
        .create_or_get_source(&workspace.id, SourceType::Folder, "fixture", Some("fixture"))
        .await
        .unwrap();
    let bytes = b"RepoMemo provides local artifact retrieval.";
    let content_hash = StorageEngine::content_hash(bytes);
    storage
        .store_blob(&content_hash, bytes, Some("text/markdown"))
        .await
        .unwrap();
    let stored = storage
        .store_artifact(NewArtifact {
            workspace_id: workspace.id.clone(),
            source_id: source.id.clone(),
            artifact_type: ArtifactType::MarkdownDoc,
            title: "README.md".to_owned(),
            path: "README.md".to_owned(),
            content_hash: content_hash.clone(),
            mime_type: Some("text/markdown".to_owned()),
            language: Some("Markdown".to_owned()),
            size_bytes: bytes.len() as i64,
            metadata: json!({}),
        })
        .await
        .unwrap();
    storage
        .replace_artifact_index(
            &stored.artifact.id,
            vec![Chunk {
                id: String::new(),
                artifact_id: stored.artifact.id.clone(),
                workspace_id: workspace.id.clone(),
                chunk_index: 0,
                text: String::from_utf8(bytes.to_vec()).unwrap(),
                token_count: Some(5),
                start_line: Some(1),
                end_line: Some(1),
                heading_path: Some("Overview".to_owned()),
                content_hash,
                embedding_status: "not_configured".to_owned(),
                metadata: json!({}),
            }],
            vec![Symbol {
                id: String::new(),
                artifact_id: stored.artifact.id.clone(),
                workspace_id: workspace.id.clone(),
                kind: SymbolKind::Function,
                name: "retrieve_artifact".to_owned(),
                signature: Some("fn retrieve_artifact()".to_owned()),
                start_line: Some(7),
                end_line: Some(9),
                metadata: json!({}),
            }],
            0,
        )
        .await
        .unwrap();

    let request = SearchRequest {
        workspace_id: workspace.id,
        query: "artifact retrieval".to_owned(),
        artifact_types: vec![ArtifactType::MarkdownDoc],
        languages: vec!["Markdown".to_owned()],
        source_ids: vec![source.id],
        limit: Some(10),
        ..Default::default()
    };
    let results = storage
        .search_chunks(&request, "\"artifact\"* AND \"retrieval\"*")
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].artifact_id, stored.artifact.id);
    assert_eq!(results[0].start_line, Some(1));
    assert!(results[0].snippet.contains("<mark>artifact</mark>"));

    let symbol_results = storage
        .search_symbols(&request.workspace_id, "retrieve", 10)
        .await
        .unwrap();
    assert_eq!(symbol_results.len(), 1);
    assert_eq!(symbol_results[0].symbol.name, "retrieve_artifact");
    assert_eq!(symbol_results[0].symbol.start_line, Some(7));

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn reindexing_keeps_unchanged_chunks_and_their_embeddings() {
    let data_dir = std::env::temp_dir().join(format!(
        "repomemo-reindex-test-{}",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    let storage = StorageEngine::open(StorageConfig::new(data_dir.clone())).await.unwrap();
    let workspace = storage.create_workspace("Reindex test").await.unwrap();
    let source = storage
        .create_or_get_source(&workspace.id, SourceType::Folder, "fixture", Some("fixture"))
        .await
        .unwrap();
    let bytes = b"placeholder";
    let content_hash = StorageEngine::content_hash(bytes);
    storage
        .store_blob(&content_hash, bytes, Some("text/markdown"))
        .await
        .unwrap();
    let artifact = storage
        .store_artifact(NewArtifact {
            workspace_id: workspace.id.clone(),
            source_id: source.id,
            artifact_type: ArtifactType::MarkdownDoc,
            title: "Notes".to_owned(),
            path: "notes.md".to_owned(),
            content_hash,
            mime_type: Some("text/markdown".to_owned()),
            language: Some("Markdown".to_owned()),
            size_bytes: bytes.len() as i64,
            metadata: json!({}),
        })
        .await
        .unwrap()
        .artifact;

    let chunk = |index: i64, text: &str| Chunk {
        id: String::new(),
        artifact_id: artifact.id.clone(),
        workspace_id: workspace.id.clone(),
        chunk_index: index,
        text: text.to_owned(),
        token_count: Some(1),
        start_line: Some(index + 1),
        end_line: Some(index + 1),
        heading_path: None,
        content_hash: StorageEngine::content_hash(text.as_bytes()),
        embedding_status: "not_configured".to_owned(),
        metadata: json!({}),
    };

    storage
        .replace_artifact_index(
            &artifact.id,
            vec![chunk(0, "alpha stays"), chunk(1, "beta moves"), chunk(2, "gamma goes")],
            Vec::new(),
            1,
        )
        .await
        .unwrap();
    let before = storage.list_chunks_for_artifact(&artifact.id).await.unwrap();
    let id_of = |chunks: &[Chunk], text: &str| {
        chunks
            .iter()
            .find(|candidate| candidate.text == text)
            .map(|candidate| candidate.id.clone())
    };
    storage
        .upsert_embeddings(
            &workspace.id,
            "test-model",
            vec![
                (id_of(&before, "alpha stays").unwrap(), vec![1.0, 0.0]),
                (id_of(&before, "gamma goes").unwrap(), vec![0.0, 1.0]),
            ],
        )
        .await
        .unwrap();
    assert_eq!(storage.embedding_count(&workspace.id).await.unwrap(), 2);

    // `beta` moves to a new position, `gamma` is removed, `delta` is new.
    storage
        .replace_artifact_index(
            &artifact.id,
            vec![
                chunk(0, "alpha stays"),
                chunk(1, "delta arrives"),
                chunk(2, "beta moves"),
            ],
            Vec::new(),
            2,
        )
        .await
        .unwrap();
    let after = storage.list_chunks_for_artifact(&artifact.id).await.unwrap();

    assert_eq!(after.len(), 3);
    assert_eq!(
        after.iter().map(|chunk| chunk.chunk_index).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(id_of(&after, "alpha stays"), id_of(&before, "alpha stays"));
    assert_eq!(id_of(&after, "beta moves"), id_of(&before, "beta moves"));
    assert!(id_of(&after, "gamma goes").is_none());
    assert!(id_of(&before, "delta arrives").is_none());
    assert_eq!(
        after
            .iter()
            .find(|chunk| chunk.text == "beta moves")
            .unwrap()
            .chunk_index,
        2
    );
    // The embedding of the unchanged chunk survived; the removed chunk's went with it.
    assert_eq!(storage.embedding_count(&workspace.id).await.unwrap(), 1);
    assert_eq!(
        after
            .iter()
            .find(|chunk| chunk.text == "alpha stays")
            .unwrap()
            .embedding_status,
        "ready"
    );

    // Full-text search follows the renumbering and the removal.
    let search = |query: &str| SearchRequest {
        workspace_id: workspace.id.clone(),
        query: query.to_owned(),
        artifact_types: Vec::new(),
        languages: Vec::new(),
        source_ids: Vec::new(),
        limit: Some(10),
        ..Default::default()
    };
    for (word, expected) in [("delta", 1), ("beta", 1), ("gamma", 0)] {
        let results = storage
            .search_chunks(&search(word), &format!("\"{word}\"*"))
            .await
            .unwrap();
        assert_eq!(results.len(), expected, "search for {word}");
    }

    // Only the index version distinguishes fresh from stale artifacts.
    assert!(storage
        .list_artifacts_needing_index(Some(&workspace.id), 2, true)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        storage
            .list_artifacts_needing_index(Some(&workspace.id), 3, true)
            .await
            .unwrap()
            .len(),
        1
    );

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// The background queue indexes several artifacts at once, so concurrent
/// re-indexing must wait for the write lock instead of failing with
/// "database is locked".
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reindexing_of_different_artifacts_does_not_lock_the_database() {
    let data_dir = std::env::temp_dir().join(format!(
        "repomemo-concurrent-index-test-{}",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    let storage = StorageEngine::open(StorageConfig::new(data_dir.clone())).await.unwrap();
    let workspace = storage.create_workspace("Concurrent index").await.unwrap();
    let source = storage
        .create_or_get_source(&workspace.id, SourceType::Folder, "fixture", Some("fixture"))
        .await
        .unwrap();

    let mut artifact_ids = Vec::new();
    for number in 0..12 {
        let bytes = format!("artifact number {number}").into_bytes();
        let content_hash = StorageEngine::content_hash(&bytes);
        storage
            .store_blob(&content_hash, &bytes, Some("text/plain"))
            .await
            .unwrap();
        let artifact = storage
            .store_artifact(NewArtifact {
                workspace_id: workspace.id.clone(),
                source_id: source.id.clone(),
                artifact_type: ArtifactType::File,
                title: format!("File {number}"),
                path: format!("file-{number}.txt"),
                content_hash,
                mime_type: Some("text/plain".to_owned()),
                language: Some("Text".to_owned()),
                size_bytes: bytes.len() as i64,
                metadata: json!({}),
            })
            .await
            .unwrap()
            .artifact;
        artifact_ids.push(artifact.id);
    }

    // Two passes: the second one finds existing chunks to reuse.
    for pass in 0..2 {
        let mut tasks = Vec::new();
        for artifact_id in &artifact_ids {
            let storage = storage.clone();
            let workspace_id = workspace.id.clone();
            let artifact_id = artifact_id.clone();
            tasks.push(tokio::spawn(async move {
                let chunks = (0..3)
                    .map(|index| {
                        let text = format!("{artifact_id} chunk {index} pass 0");
                        Chunk {
                            id: String::new(),
                            artifact_id: artifact_id.clone(),
                            workspace_id: workspace_id.clone(),
                            chunk_index: index,
                            content_hash: StorageEngine::content_hash(text.as_bytes()),
                            text,
                            token_count: Some(1),
                            start_line: Some(index + 1),
                            end_line: Some(index + 1),
                            heading_path: None,
                            embedding_status: "not_configured".to_owned(),
                            metadata: json!({}),
                        }
                    })
                    .collect();
                storage
                    .replace_artifact_index(&artifact_id, chunks, Vec::new(), 1)
                    .await
            }));
        }
        for task in tasks {
            task.await.unwrap().unwrap_or_else(|error| {
                panic!("re-index failed on pass {pass}: {error}");
            });
        }
    }

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn memory_cards_link_evidence_and_are_searchable() {
    let data_dir = std::env::temp_dir().join(format!(
        "repomemo-memory-test-{}",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    let storage = StorageEngine::open(StorageConfig::new(data_dir.clone())).await.unwrap();
    let workspace = storage.create_workspace("Memory test").await.unwrap();
    let source = storage
        .create_or_get_source(&workspace.id, SourceType::Manual, "Notes", None)
        .await
        .unwrap();
    let bytes = b"Local evidence for a durable memory card.";
    let content_hash = StorageEngine::content_hash(bytes);
    storage
        .store_blob(&content_hash, bytes, Some("text/plain"))
        .await
        .unwrap();
    let artifact = storage
        .store_artifact(NewArtifact {
            workspace_id: workspace.id.clone(),
            source_id: source.id,
            artifact_type: ArtifactType::Note,
            title: "Evidence note".to_owned(),
            path: "evidence.txt".to_owned(),
            content_hash,
            mime_type: Some("text/plain".to_owned()),
            language: Some("Text".to_owned()),
            size_bytes: bytes.len() as i64,
            metadata: json!({}),
        })
        .await
        .unwrap()
        .artifact;
    let card = storage
        .create_memory_card(
            &workspace.id,
            "Keep local evidence",
            "Evidence remains inspectable.",
            "manual",
            None,
            &[Citation {
                artifact_id: artifact.id.clone(),
                chunk_id: None,
                title: artifact.title.clone(),
                path: artifact.path.clone(),
                start_line: None,
                end_line: None,
                confidence: None,
                commit: None,
            }],
        )
        .await
        .unwrap();
    let matches = storage.search_memory_cards(&workspace.id, "inspectable").await.unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].evidence_count, 1);
    let detail = storage.get_memory_card(&card.id).await.unwrap();
    assert_eq!(detail.evidence.len(), 1);
    assert!(detail.evidence[0].exists);
    let updated = storage
        .update_memory_card(
            &card.id,
            "Keep verified evidence",
            "Evidence remains local and inspectable.",
            "manual",
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(updated.title, "Keep verified evidence");

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn temp_storage(label: &str) -> (StorageEngine, std::path::PathBuf) {
    let data_dir = std::env::temp_dir().join(format!("repomemo-{label}-{}", uuid::Uuid::new_v4()));
    let storage = StorageEngine::open(StorageConfig::new(data_dir.clone())).await.unwrap();
    (storage, data_dir)
}

fn cloud_provider(workspace_id: &str, base_url: &str, api_key: Option<&str>) -> repomemo_domain::ProviderSettings {
    repomemo_domain::ProviderSettings {
        id: String::new(),
        workspace_id: Some(workspace_id.to_owned()),
        provider_type: "openrouter".to_owned(),
        name: "Cloud".to_owned(),
        base_url: Some(base_url.to_owned()),
        model: Some("openai/gpt-4o-mini".to_owned()),
        embedding_model: None,
        enabled: true,
        metadata: json!({ "cloud_content_acknowledged": true, "purpose": "text" }),
        api_key: api_key.map(str::to_owned),
    }
}

#[tokio::test]
async fn provider_api_keys_are_encrypted_and_never_follow_a_new_endpoint() {
    let (storage, data_dir) = temp_storage("provider-keys").await;
    let workspace = storage.create_workspace("Keys").await.unwrap();
    let saved = storage
        .save_provider_settings(cloud_provider(
            &workspace.id,
            "https://openrouter.ai/api/v1",
            Some("sk-or-secret-value"),
        ))
        .await
        .unwrap();
    assert_eq!(saved.api_key.as_deref(), Some("sk-or-secret-value"));
    assert!(
        saved.metadata.get("api_key_sealed").is_none(),
        "the sealed key is not exposed in metadata"
    );
    let raw: String = sqlx::query_scalar("SELECT metadata_json FROM provider_settings WHERE id = ?1")
        .bind(&saved.id)
        .fetch_one(&storage.pool)
        .await
        .unwrap();
    assert!(
        !raw.contains("sk-or-secret-value"),
        "the key is not stored in plain text"
    );
    assert!(raw.contains("api_key_sealed"));

    // Saving without a key keeps it while the endpoint stays the same.
    let mut unchanged = cloud_provider(&workspace.id, "https://openrouter.ai/api/v1/", None);
    unchanged.id = saved.id.clone();
    let kept = storage.save_provider_settings(unchanged).await.unwrap();
    assert_eq!(kept.api_key.as_deref(), Some("sk-or-secret-value"));

    // Pointing the provider somewhere else drops the stored key.
    let mut moved = cloud_provider(&workspace.id, "https://attacker.example/api", None);
    moved.id = saved.id.clone();
    let moved = storage.save_provider_settings(moved).await.unwrap();
    assert_eq!(moved.api_key, None);

    // Keys saved before encryption existed are sealed at startup.
    sqlx::query("UPDATE provider_settings SET metadata_json = ?1 WHERE id = ?2")
        .bind(r#"{"purpose":"text","api_key":"legacy-plaintext-key"}"#)
        .bind(&saved.id)
        .execute(&storage.pool)
        .await
        .unwrap();
    assert_eq!(
        storage
            .get_provider_settings(&saved.id)
            .await
            .unwrap()
            .api_key
            .as_deref(),
        Some("legacy-plaintext-key")
    );
    assert_eq!(storage.seal_plaintext_provider_keys().await.unwrap(), 1);
    let raw: String = sqlx::query_scalar("SELECT metadata_json FROM provider_settings WHERE id = ?1")
        .bind(&saved.id)
        .fetch_one(&storage.pool)
        .await
        .unwrap();
    assert!(!raw.contains("legacy-plaintext-key"));
    assert_eq!(
        storage
            .get_provider_settings(&saved.id)
            .await
            .unwrap()
            .api_key
            .as_deref(),
        Some("legacy-plaintext-key")
    );

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn ending_sessions_bumps_the_version_and_revokes_refresh_tokens() {
    let (storage, data_dir) = temp_storage("sessions").await;
    let user = storage
        .create_user("sessions@example.com", "Sessions", "hash")
        .await
        .unwrap();
    assert_eq!(storage.user_session_version(&user.id).await.unwrap(), Some(0));
    let refresh = storage.create_refresh_token(&user.id, 30).await.unwrap();
    assert_eq!(storage.end_user_sessions(&user.id).await.unwrap(), 1);
    assert_eq!(storage.user_session_version(&user.id).await.unwrap(), Some(1));
    assert_eq!(storage.consume_refresh_token(&refresh).await.unwrap(), None);
    assert_eq!(storage.user_session_version("missing").await.unwrap(), None);
    assert_eq!(storage.count_users().await.unwrap(), 1);

    // Only expired tokens are purged; revoked ones stay for reuse detection.
    storage.create_refresh_token(&user.id, 30).await.unwrap();
    storage.create_refresh_token(&user.id, -1).await.unwrap();
    let purged = storage
        .purge_expired_refresh_tokens(&chrono::Utc::now().to_rfc3339())
        .await
        .unwrap();
    assert_eq!(purged, 1);

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn unreferenced_blobs_are_collected_after_the_grace_period() {
    let (storage, data_dir) = temp_storage("blob-gc").await;
    let workspace = storage.create_workspace("Blobs").await.unwrap();
    let source = storage
        .create_or_get_source(&workspace.id, SourceType::Upload, "Uploads", None)
        .await
        .unwrap();
    let mut stored = Vec::new();
    for text in ["kept content", "deleted content"] {
        let bytes = text.as_bytes();
        let hash = StorageEngine::content_hash(bytes);
        storage.store_blob(&hash, bytes, Some("text/plain")).await.unwrap();
        let artifact = storage
            .store_artifact(NewArtifact {
                workspace_id: workspace.id.clone(),
                source_id: source.id.clone(),
                artifact_type: ArtifactType::File,
                title: text.to_owned(),
                path: format!("{text}.txt"),
                content_hash: hash.clone(),
                mime_type: Some("text/plain".to_owned()),
                language: None,
                size_bytes: bytes.len() as i64,
                metadata: json!({}),
            })
            .await
            .unwrap()
            .artifact;
        stored.push((artifact, hash));
    }
    storage.delete_artifact(&stored[1].0.id).await.unwrap();

    // Within the grace period nothing is removed.
    let early = storage
        .collect_orphan_blobs(chrono::Duration::hours(1), 100)
        .await
        .unwrap();
    assert!(early.removed.is_empty());

    let collected = storage
        .collect_orphan_blobs(chrono::Duration::seconds(-1), 100)
        .await
        .unwrap();
    assert_eq!(collected.removed, vec![stored[1].1.clone()]);
    assert!(!storage.blob_path(&stored[1].1).exists());
    assert!(storage.blob_path(&stored[0].1).exists());
    assert!(storage.read_artifact_blob(&stored[0].0.id).await.is_ok());
    assert!(!storage.blob_exists(&stored[1].1).await.unwrap());

    // Storing the same content again brings it back.
    storage
        .store_blob(&stored[1].1, b"deleted content", Some("text/plain"))
        .await
        .unwrap();
    assert!(storage.blob_path(&stored[1].1).exists());

    // A stray file with no row is swept once it is old enough.
    let stray = storage.blob_dir().join("zz").join("zz");
    std::fs::create_dir_all(&stray).unwrap();
    std::fs::write(stray.join("zzzzstray.tmp-1"), b"partial").unwrap();
    assert_eq!(
        storage
            .sweep_untracked_blob_files(std::time::Duration::from_secs(3600))
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        storage
            .sweep_untracked_blob_files(std::time::Duration::ZERO)
            .await
            .unwrap(),
        1
    );
    assert!(storage.blob_path(&stored[0].1).exists());

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn interrupted_jobs_fail_and_old_finished_jobs_are_pruned() {
    let (storage, data_dir) = temp_storage("jobs").await;
    let workspace = storage.create_workspace("Jobs").await.unwrap();
    let running = storage
        .create_job(&workspace.id, None, "embedding", "embedding_chunks", Some(4))
        .await
        .unwrap();
    let done = storage
        .create_job(&workspace.id, None, "indexing", "chunking", Some(1))
        .await
        .unwrap();
    storage
        .update_indexing_job(&done.id, "completed", "chunked", 1, None)
        .await
        .unwrap();

    assert_eq!(
        storage
            .fail_all_interrupted_jobs("Interrupted by a server restart.")
            .await
            .unwrap(),
        1
    );
    let failed = storage.get_job(&running.id).await.unwrap().unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(
        failed.error_message.as_deref(),
        Some("Interrupted by a server restart.")
    );

    let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
    assert_eq!(storage.prune_finished_jobs(&future).await.unwrap(), 2);
    assert!(storage.get_job(&done.id).await.unwrap().is_none());

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn workspace_settings_keep_other_keys() {
    let (storage, data_dir) = temp_storage("settings").await;
    let workspace = storage.create_workspace("Settings").await.unwrap();
    assert_eq!(storage.workspace_settings(&workspace.id).await.unwrap(), json!({}));
    storage
        .set_workspace_setting(&workspace.id, "a", json!(1))
        .await
        .unwrap();
    let settings = storage
        .set_workspace_setting(&workspace.id, "b", json!("two"))
        .await
        .unwrap();
    assert_eq!(settings, json!({ "a": 1, "b": "two" }));
    assert_eq!(storage.workspace_settings(&workspace.id).await.unwrap(), settings);
    assert!(storage.workspace_settings("missing").await.is_err());

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn vector_search_uses_the_index_and_agrees_with_a_scan() {
    let (storage, data_dir) = temp_storage("vectors").await;
    let workspace = storage.create_workspace("Vectors").await.unwrap();
    let source = storage
        .create_or_get_source(&workspace.id, SourceType::Folder, "fixture", Some("fixture"))
        .await
        .unwrap();
    let bytes = b"vectors";
    let content_hash = StorageEngine::content_hash(bytes);
    storage
        .store_blob(&content_hash, bytes, Some("text/markdown"))
        .await
        .unwrap();
    let artifact = storage
        .store_artifact(NewArtifact {
            workspace_id: workspace.id.clone(),
            source_id: source.id,
            artifact_type: ArtifactType::MarkdownDoc,
            title: "Vectors".to_owned(),
            path: "vectors.md".to_owned(),
            content_hash,
            mime_type: Some("text/markdown".to_owned()),
            language: Some("Markdown".to_owned()),
            size_bytes: bytes.len() as i64,
            metadata: json!({}),
        })
        .await
        .unwrap()
        .artifact;
    let chunk = |index: i64, text: &str| Chunk {
        id: String::new(),
        artifact_id: artifact.id.clone(),
        workspace_id: workspace.id.clone(),
        chunk_index: index,
        text: text.to_owned(),
        token_count: Some(1),
        start_line: Some(index + 1),
        end_line: Some(index + 1),
        heading_path: None,
        content_hash: StorageEngine::content_hash(text.as_bytes()),
        embedding_status: "not_configured".to_owned(),
        metadata: json!({}),
    };
    storage
        .replace_artifact_index(
            &artifact.id,
            vec![
                chunk(0, "east"),
                chunk(1, "north"),
                chunk(2, "north east"),
                chunk(3, "west"),
            ],
            Vec::new(),
            1,
        )
        .await
        .unwrap();
    let chunks = storage.list_chunks_for_artifact(&artifact.id).await.unwrap();
    let id_of = |text: &str| chunks.iter().find(|chunk| chunk.text == text).unwrap().id.clone();
    storage
        .upsert_embeddings(
            &workspace.id,
            "m",
            vec![
                (id_of("east"), vec![1.0, 0.0]),
                (id_of("north"), vec![0.0, 1.0]),
                (id_of("north east"), vec![1.0, 1.0]),
            ],
        )
        .await
        .unwrap();

    let request = SearchRequest {
        workspace_id: workspace.id.clone(),
        limit: Some(2),
        ..Default::default()
    };
    let query = [1.0_f32, 0.1];
    let indexed = storage.search_chunks_by_embedding(&request, "m", &query).await.unwrap();
    let scanned = storage
        .scan_chunks_by_embedding(&request, &[], "m", &query)
        .await
        .unwrap();
    assert_eq!(indexed.len(), 2);
    assert_eq!(indexed[0].chunk_id, id_of("east"));
    assert_eq!(indexed[1].chunk_id, id_of("north east"));
    assert_eq!(
        indexed.iter().map(|result| &result.chunk_id).collect::<Vec<_>>(),
        scanned
            .iter()
            .take(2)
            .map(|result| &result.chunk_id)
            .collect::<Vec<_>>()
    );
    assert!((indexed[0].score - scanned[0].score).abs() < 1e-4);
    assert_eq!(storage.vector_index_usage().1, 1, "the workspace has an index now");

    // A vector written after the index was built is found without a rebuild.
    storage
        .upsert_embeddings(&workspace.id, "m", vec![(id_of("west"), vec![1.0, 0.12])])
        .await
        .unwrap();
    let after = storage.search_chunks_by_embedding(&request, "m", &query).await.unwrap();
    assert_eq!(after[0].chunk_id, id_of("west"));

    // Another model has no vectors here.
    assert!(storage
        .search_chunks_by_embedding(&request, "other", &query)
        .await
        .unwrap()
        .is_empty());

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Timings on a large workspace (roadmap V0.1.74 targets). Slow, so it only
/// runs on request:
/// `cargo test -p repomemo-storage --release performance_at_scale -- --ignored --nocapture`
/// `REPOMEMO_PERF_PASSAGES` sets the size (default 50,000).
#[tokio::test]
#[ignore = "a performance run, not a check"]
async fn performance_at_scale() {
    use std::time::Instant;
    let passages: usize = std::env::var("REPOMEMO_PERF_PASSAGES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(50_000);
    let per_file = 100;
    let dims = 384;
    let (storage, data_dir) = temp_storage("scale").await;
    let workspace = storage.create_workspace("Scale").await.unwrap();
    let source = storage
        .create_or_get_source(&workspace.id, SourceType::Folder, "fixture", Some("fixture"))
        .await
        .unwrap();

    let started = Instant::now();
    let mut seed = 0x2545_f491_u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % 10_000) as f32 / 10_000.0 - 0.5
    };
    let mut batch: Vec<(String, Vec<f32>)> = Vec::new();
    for file in 0..passages.div_ceil(per_file) {
        let bytes = format!("file {file}").into_bytes();
        let content_hash = StorageEngine::content_hash(&bytes);
        storage
            .store_blob(&content_hash, &bytes, Some("text/markdown"))
            .await
            .unwrap();
        let artifact = storage
            .store_artifact(NewArtifact {
                workspace_id: workspace.id.clone(),
                source_id: source.id.clone(),
                artifact_type: ArtifactType::MarkdownDoc,
                title: format!("Document {file}"),
                path: format!("docs/{file}.md"),
                content_hash,
                mime_type: Some("text/markdown".to_owned()),
                language: Some("Markdown".to_owned()),
                size_bytes: bytes.len() as i64,
                metadata: json!({}),
            })
            .await
            .unwrap()
            .artifact;
        let count = per_file.min(passages - file * per_file);
        let chunks = (0..count)
            .map(|index| {
                let text = format!(
                    "passage {index} of document {file} covers topic{} and release",
                    (file + index) % 97
                );
                Chunk {
                    id: String::new(),
                    artifact_id: artifact.id.clone(),
                    workspace_id: workspace.id.clone(),
                    chunk_index: index as i64,
                    token_count: Some(12),
                    start_line: Some(index as i64 + 1),
                    end_line: Some(index as i64 + 1),
                    heading_path: None,
                    content_hash: StorageEngine::content_hash(text.as_bytes()),
                    embedding_status: "not_configured".to_owned(),
                    metadata: json!({}),
                    text,
                }
            })
            .collect();
        storage
            .replace_artifact_index(&artifact.id, chunks, Vec::new(), 1)
            .await
            .unwrap();
        for chunk in storage.list_chunks_for_artifact(&artifact.id).await.unwrap() {
            batch.push((chunk.id, (0..dims).map(|_| next()).collect()));
        }
        if batch.len() >= 2_000 {
            storage
                .upsert_embeddings(&workspace.id, "perf", std::mem::take(&mut batch))
                .await
                .unwrap();
        }
    }
    if !batch.is_empty() {
        storage.upsert_embeddings(&workspace.id, "perf", batch).await.unwrap();
    }
    println!("built {passages} passages in {:?}", started.elapsed());

    let percentile = |mut samples: Vec<u128>| {
        samples.sort_unstable();
        samples[(samples.len() * 95 / 100).min(samples.len() - 1)]
    };
    let request = SearchRequest {
        workspace_id: workspace.id.clone(),
        query: "release".to_owned(),
        limit: Some(12),
        ..Default::default()
    };
    let query: Vec<f32> = (0..dims).map(|index| ((index * 31 % 17) as f32) - 8.0).collect();

    let first = Instant::now();
    storage
        .search_chunks_by_embedding(&request, "perf", &query)
        .await
        .unwrap();
    println!("vector search, first (builds the index): {:?}", first.elapsed());
    let mut samples = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        let found = storage
            .search_chunks_by_embedding(&request, "perf", &query)
            .await
            .unwrap();
        samples.push(started.elapsed().as_millis());
        assert_eq!(found.len(), 12);
    }
    println!(
        "vector search p95: {} ms (target 300 ms with keyword search)",
        percentile(samples)
    );

    let mut samples = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        storage.search_chunks(&request, "\"release\"*").await.unwrap();
        samples.push(started.elapsed().as_millis());
    }
    println!("keyword search p95: {} ms", percentile(samples));

    let mut samples = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        storage
            .workspace_artifact_stats(&workspace.id, "2000-01-01")
            .await
            .unwrap();
        storage.workspace_overview(&workspace.id).await.unwrap();
        samples.push(started.elapsed().as_millis());
    }
    println!("figures (artifact stats and overview) p95: {} ms", percentile(samples));

    let started = Instant::now();
    storage.workspace_content_version(&workspace.id).await.unwrap();
    println!("content version: {:?}", started.elapsed());
    let started = Instant::now();
    let (page, total) = storage
        .query_artifacts(&super::ArtifactQuery {
            workspace_id: workspace.id.clone(),
            text: "document 1".to_owned(),
            limit: Some(50),
            ..Default::default()
        })
        .await
        .unwrap();
    println!("artifact query: {:?} ({} of {total})", started.elapsed(), page.len());

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}
