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
    assert_eq!(
        storage.consume_refresh_token(&refresh).await.unwrap(),
        crate::RefreshOutcome::Refused
    );
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

/// V0.2.153: a received file is written to `<data>/tmp/` and hashed as it
/// arrives, then moved into the blob store; nothing is left behind.
#[tokio::test]
async fn a_received_file_is_hashed_as_it_arrives_and_moved_into_the_blob_store() {
    let data_dir = std::env::temp_dir().join(format!(
        "repomemo-staged-blob-test-{}",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    let storage = StorageEngine::open(StorageConfig::new(data_dir.clone())).await.unwrap();
    let leftovers = || {
        std::fs::read_dir(data_dir.join("tmp"))
            .map(|entries| entries.count())
            .unwrap_or(0)
    };
    let bytes: Vec<u8> = (0..300_000_u32).map(|n| (n % 251) as u8).collect();

    let mut writer = storage.begin_staged_blob().await.unwrap();
    for chunk in bytes.chunks(64 * 1024) {
        writer.write(chunk).await.unwrap();
    }
    assert_eq!(writer.size(), bytes.len() as u64);
    let staged = writer.finish().await.unwrap();
    assert_eq!(staged.content_hash, StorageEngine::content_hash(&bytes));
    assert_eq!(staged.size_bytes, bytes.len() as u64);
    let hash = staged.content_hash.clone();
    storage
        .store_staged_blob(staged, Some("application/octet-stream"))
        .await
        .unwrap();
    assert_eq!(std::fs::read(storage.blob_path_for_hash(&hash)).unwrap(), bytes);
    assert_eq!(leftovers(), 0, "the received file moved, it was not copied");

    // The same content again keeps the one stored file and deletes the received copy.
    let mut again = storage.begin_staged_blob().await.unwrap();
    again.write(&bytes).await.unwrap();
    storage
        .store_staged_blob(again.finish().await.unwrap(), Some("application/octet-stream"))
        .await
        .unwrap();
    assert_eq!(std::fs::read(storage.blob_path_for_hash(&hash)).unwrap(), bytes);
    assert_eq!(leftovers(), 0);

    // An upload that stops half way, or a received file nobody stores, leaves nothing.
    let mut stopped = storage.begin_staged_blob().await.unwrap();
    stopped.write(b"partial").await.unwrap();
    assert_eq!(leftovers(), 1);
    drop(stopped);
    let mut unused = storage.begin_staged_blob().await.unwrap();
    unused.write(b"complete but unused").await.unwrap();
    let unused = unused.finish().await.unwrap();
    assert_eq!(leftovers(), 1);
    drop(unused);
    assert_eq!(leftovers(), 0);

    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// V0.2.154: a page of a workspace's files is read in SQL, newest change first,
/// with the total; the folders and repositories of a page come for its ids only.
#[tokio::test]
async fn a_page_of_files_is_read_in_sql_with_the_total() {
    let (storage, data_dir) = temp_storage("file-pages").await;
    let workspace = storage.create_workspace("Pages").await.unwrap();
    let source = storage
        .create_or_get_source(&workspace.id, SourceType::Folder, "fixture", Some("fixture"))
        .await
        .unwrap();
    let bytes = b"one shared file";
    let content_hash = StorageEngine::content_hash(bytes);
    storage
        .store_blob(&content_hash, bytes, Some("text/markdown"))
        .await
        .unwrap();
    let first = storage
        .store_artifact(NewArtifact {
            workspace_id: workspace.id.clone(),
            source_id: source.id.clone(),
            artifact_type: ArtifactType::MarkdownDoc,
            title: "Seed".to_owned(),
            path: "seed.md".to_owned(),
            content_hash: content_hash.clone(),
            mime_type: Some("text/markdown".to_owned()),
            language: Some("Markdown".to_owned()),
            size_bytes: bytes.len() as i64,
            metadata: json!({}),
        })
        .await
        .unwrap()
        .artifact;
    // 2,099 more rows of the same kind, made in one statement; the dates differ so the order is a real one.
    sqlx::query(
        r#"
        WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 2099)
        INSERT INTO artifacts (id, workspace_id, source_id, type, title, path, content_hash, mime_type, language,
                               size_bytes, created_at, updated_at, indexed_at, metadata_json)
        SELECT 'bulk-' || n.i, a.workspace_id, a.source_id, a.type, 'Bulk ' || n.i, 'bulk/' || n.i || '.md',
               a.content_hash, a.mime_type, a.language, a.size_bytes, a.created_at,
               printf('2020-%02d-%02dT00:00:00+00:00', n.i % 12 + 1, n.i % 28 + 1), NULL, '{}'
        FROM n, artifacts a WHERE a.id = ?1
        "#,
    )
    .bind(&first.id)
    .execute(&storage.pool)
    .await
    .unwrap();

    let (page, total) = storage.list_artifacts_page(&workspace.id, 2_000, 0).await.unwrap();
    assert_eq!(
        (page.len(), total),
        (2_000, 2_100),
        "a page is capped at its size and the total is all of them"
    );
    let all = storage.list_artifacts(&workspace.id).await.unwrap();
    assert_eq!(
        page.iter().map(|artifact| artifact.id.as_str()).collect::<Vec<_>>(),
        all.iter()
            .take(2_000)
            .map(|artifact| artifact.id.as_str())
            .collect::<Vec<_>>(),
        "the same order as the whole list"
    );
    let (rest, total) = storage.list_artifacts_page(&workspace.id, 50, 2_090).await.unwrap();
    assert_eq!((rest.len(), total), (10, 2_100));
    let (nothing, total) = storage.list_artifacts_page(&workspace.id, 50, 5_000).await.unwrap();
    assert_eq!((nothing.len(), total), (0, 2_100));

    // Folders and repositories are looked up for the ids of a page only.
    let folder = storage.create_folder(&workspace.id, None, "Plans", None).await.unwrap();
    storage.set_artifact_folder(&first.id, Some(&folder.id)).await.unwrap();
    let ids = vec![first.id.clone(), "bulk-7".to_owned()];
    let folders = storage.artifact_folder_ids_for(&ids).await.unwrap();
    assert_eq!(folders.len(), 1);
    assert_eq!(folders.get(&first.id), Some(&folder.id));
    assert!(storage.repo_artifact_sources_for(&ids).await.unwrap().is_empty());
    assert!(storage.artifact_folder_ids_for(&[]).await.unwrap().is_empty());

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// V0.2.159: the read connections get a larger page cache, memory-mapped reads and temporary
/// tables in memory; the connection that writes keeps SQLite's defaults.
#[tokio::test]
async fn read_connections_get_a_larger_cache_memory_mapped_reads_and_memory_temporaries() {
    let pragma = |engine: &StorageEngine, name: &'static str, writer: bool| {
        let pool = if writer {
            engine.pool.clone()
        } else {
            engine.read_pool.clone()
        };
        async move {
            sqlx::query_scalar::<_, i64>(&format!("PRAGMA {name}"))
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let (storage, data_dir) = temp_storage("read-tuning").await;
    assert_eq!(
        pragma(&storage, "cache_size", false).await,
        -32 * 1024,
        "32 MiB by default"
    );
    assert_eq!(
        pragma(&storage, "mmap_size", false).await,
        256 * 1024 * 1024,
        "256 MiB by default"
    );
    assert_eq!(pragma(&storage, "temp_store", false).await, 2, "in memory");
    assert_ne!(
        pragma(&storage, "cache_size", true).await,
        -32 * 1024,
        "the writer keeps SQLite's default"
    );
    assert_ne!(pragma(&storage, "mmap_size", true).await, 256 * 1024 * 1024);
    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);

    // Set by the operator, and 0 leaves SQLite's own default.
    let data_dir = std::env::temp_dir().join(format!("repomemo-read-tuning-set-{}", uuid::Uuid::new_v4()));
    let mut config = StorageConfig::new(data_dir.clone());
    config.read_cache_mb = 64;
    config.read_mmap_mb = 0;
    let storage = StorageEngine::open(config).await.unwrap();
    assert_eq!(pragma(&storage, "cache_size", false).await, -64 * 1024);
    assert_eq!(pragma(&storage, "mmap_size", false).await, 0, "memory-mapped reads off");
    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);

    let data_dir = std::env::temp_dir().join(format!("repomemo-read-tuning-off-{}", uuid::Uuid::new_v4()));
    let mut config = StorageConfig::new(data_dir.clone());
    config.read_cache_mb = 0;
    let storage = StorageEngine::open(config).await.unwrap();
    assert_ne!(
        pragma(&storage, "cache_size", false).await,
        -32 * 1024,
        "0 keeps SQLite's default cache"
    );
    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// V0.2.155 Bis: the artifact query also filters by folder, leaves out repository files and takes
/// kinds of file (a type, an extension), which a view of the evidence section is made of.
#[tokio::test]
async fn the_artifact_query_filters_by_folder_repository_and_kind_and_folders_count_their_files() {
    let (storage, data_dir) = temp_storage("query-views").await;
    let workspace = storage.create_workspace("Views").await.unwrap();
    let source = storage
        .create_or_get_source(&workspace.id, SourceType::Folder, "fixture", Some("fixture"))
        .await
        .unwrap();
    let mut ids = std::collections::HashMap::new();
    for (path, artifact_type) in [
        ("plan.docx", ArtifactType::File),
        ("budget.xlsx", ArtifactType::File),
        ("notes.txt", ArtifactType::File),
        ("guide.md", ArtifactType::MarkdownDoc),
        ("main.rs", ArtifactType::CodeFile),
    ] {
        let bytes = path.as_bytes();
        let content_hash = StorageEngine::content_hash(bytes);
        storage.store_blob(&content_hash, bytes, None).await.unwrap();
        let artifact = storage
            .store_artifact(NewArtifact {
                workspace_id: workspace.id.clone(),
                source_id: source.id.clone(),
                artifact_type,
                title: path.to_owned(),
                path: path.to_owned(),
                content_hash,
                mime_type: None,
                language: None,
                size_bytes: bytes.len() as i64,
                metadata: json!({}),
            })
            .await
            .unwrap()
            .artifact;
        ids.insert(path, artifact.id);
    }
    let folder = storage.create_folder(&workspace.id, None, "Plans", None).await.unwrap();
    storage
        .set_artifact_folder(&ids["plan.docx"], Some(&folder.id))
        .await
        .unwrap();
    storage
        .set_artifact_folder(&ids["notes.txt"], Some(&folder.id))
        .await
        .unwrap();
    // A repository file: tracked by a repo_files row.
    sqlx::query(
        "INSERT INTO repo_files (source_id, path, artifact_id, blob_sha, commit_sha, updated_at) VALUES (?1, 'main.rs', ?2, 'b', 'c', 'now')",
    )
    .bind(&source.id)
    .bind(&ids["main.rs"])
    .execute(&storage.pool)
    .await
    .unwrap();

    let titles = |found: Vec<repomemo_domain::ArtifactSummary>| {
        let mut titles: Vec<String> = found.into_iter().map(|artifact| artifact.title).collect();
        titles.sort();
        titles
    };
    let ask = |query: super::ArtifactQuery| {
        let storage = &storage;
        async move { storage.query_artifacts(&query).await.unwrap() }
    };
    let base = || super::ArtifactQuery {
        workspace_id: workspace.id.clone(),
        ..Default::default()
    };

    // A folder, the top level, and no repository files.
    let (found, total) = ask(super::ArtifactQuery {
        in_folder: Some(folder.id.clone()),
        ..base()
    })
    .await;
    assert_eq!(
        (titles(found), total),
        (vec!["notes.txt".to_owned(), "plan.docx".to_owned()], 2)
    );
    let (found, _) = ask(super::ArtifactQuery {
        top_level_only: true,
        ledger_only: true,
        ..base()
    })
    .await;
    assert_eq!(
        titles(found),
        ["budget.xlsx", "guide.md"],
        "the repository file is left out"
    );

    // Kinds are alternatives: Word files, or Markdown.
    let word = super::KindFilter {
        extensions: vec!["doc".into(), "docx".into()],
        ..Default::default()
    };
    let markdown = super::KindFilter {
        artifact_types: vec![ArtifactType::MarkdownDoc],
        ..Default::default()
    };
    let (found, _) = ask(super::ArtifactQuery {
        kinds: vec![word.clone(), markdown],
        ..base()
    })
    .await;
    assert_eq!(titles(found), ["guide.md", "plan.docx"]);
    // Plain text is a file that is not one of the document formats.
    let text = super::KindFilter {
        artifact_types: vec![ArtifactType::File],
        not_extensions: vec!["docx".into(), "xlsx".into()],
        ..Default::default()
    };
    let (found, _) = ask(super::ArtifactQuery {
        kinds: vec![text],
        ..base()
    })
    .await;
    assert_eq!(titles(found), ["notes.txt"]);
    // A value that is not an extension cannot reach the query as a pattern.
    let odd = super::KindFilter {
        extensions: vec!["%".into(), "docx'--".into()],
        ..Default::default()
    };
    let (found, _) = ask(super::ArtifactQuery {
        kinds: vec![odd],
        ..base()
    })
    .await;
    assert_eq!(titles(found), ["plan.docx"], "only the letters and digits count: docx");

    // A folder knows how many files sit directly in it.
    let folders = storage.list_folders(&workspace.id).await.unwrap();
    assert_eq!(
        folders.iter().find(|entry| entry.id == folder.id).unwrap().file_count,
        2
    );

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
async fn a_workspace_remembers_its_template_and_an_import_keeps_it() {
    use repomemo_domain::WorkspaceTemplateRef;

    let (storage, data_dir) = temp_storage("template-ref").await;
    let workspace = storage.create_workspace("Runbooks").await.unwrap();
    assert_eq!(storage.workspace_template_ref(&workspace.id).await.unwrap(), None);
    assert!(storage.workspace_template_ref("missing").await.is_err());

    // Saved with its adjustments, and the other settings stay.
    let chosen = WorkspaceTemplateRef {
        id: "operations".to_owned(),
        version: 2,
        overrides: json!({ "landing": "review" }),
    };
    storage
        .set_workspace_setting(&workspace.id, "icon", json!("rocket"))
        .await
        .unwrap();
    storage
        .set_workspace_template_ref(&workspace.id, Some(&chosen))
        .await
        .unwrap();
    assert_eq!(
        storage.workspace_template_ref(&workspace.id).await.unwrap(),
        Some(chosen.clone())
    );
    assert_eq!(
        storage.workspace_settings(&workspace.id).await.unwrap()["icon"],
        json!("rocket")
    );
    let plain = WorkspaceTemplateRef {
        id: "general".to_owned(),
        version: 1,
        overrides: json!(null),
    };
    storage
        .set_workspace_template_ref(&workspace.id, Some(&plain))
        .await
        .unwrap();
    assert_eq!(
        storage.workspace_settings(&workspace.id).await.unwrap()["template"],
        json!({ "id": "general", "version": 1 }),
        "no adjustments are written when there are none"
    );

    // A setting that is not shaped like a reference reads as no template.
    let too_big = json!({ "id": "ops", "version": 1, "overrides": { "x": "y".repeat(20_000) } });
    for bad in [
        json!("operations"),
        json!({ "id": "Ops", "version": 1 }),
        json!({ "id": "", "version": 1 }),
        json!({ "id": "ops", "version": 0 }),
        json!({ "id": "ops", "version": 1, "overrides": [1] }),
        too_big,
        json!(null),
    ] {
        storage
            .set_workspace_setting(&workspace.id, "template", bad.clone())
            .await
            .unwrap();
        assert_eq!(
            storage.workspace_template_ref(&workspace.id).await.unwrap(),
            None,
            "{bad}"
        );
    }

    // The archive carries it, and an import writes it into the new workspace.
    storage
        .set_workspace_template_ref(&workspace.id, Some(&chosen))
        .await
        .unwrap();
    let data = storage.export_workspace_data(&workspace.id).await.unwrap();
    assert_eq!(data.workspace.settings["template"]["id"], json!("operations"));
    let copy = storage.create_workspace("Copy").await.unwrap();
    storage
        .import_workspace_data(&copy.id, "importer", &data)
        .await
        .unwrap();
    assert_eq!(
        storage.workspace_template_ref(&copy.id).await.unwrap(),
        Some(chosen.clone())
    );

    // A reference an archive should not carry is left behind.
    let mut tampered = data.clone();
    tampered.workspace.settings["template"] = json!({ "id": "../../etc", "version": 1 });
    let other = storage.create_workspace("Other").await.unwrap();
    storage
        .import_workspace_data(&other.id, "importer", &tampered)
        .await
        .unwrap();
    assert_eq!(storage.workspace_template_ref(&other.id).await.unwrap(), None);

    // Clearing it returns the workspace to no template.
    storage.set_workspace_template_ref(&workspace.id, None).await.unwrap();
    assert_eq!(storage.workspace_template_ref(&workspace.id).await.unwrap(), None);

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_workspace_made_from_a_template_starts_with_its_choice_and_folders_or_not_at_all() {
    use crate::StarterSetup;
    use repomemo_domain::WorkspaceTemplateRef;

    let (storage, data_dir) = temp_storage("starter-folders").await;
    let owner = storage
        .create_user("starter-owner@example.com", "Owner", "hash")
        .await
        .unwrap();
    let organization = storage.create_organization(&owner.id, "Team").await.unwrap();
    let setup = |folders: Vec<(&str, Vec<&str>)>| StarterSetup {
        reference: WorkspaceTemplateRef {
            id: "operations".to_owned(),
            version: 2,
            overrides: json!(null),
        },
        template_name: "Operations and incidents".to_owned(),
        folders: folders
            .into_iter()
            .map(|(name, children)| (name.to_owned(), children.into_iter().map(str::to_owned).collect()))
            .collect(),
    };

    // The template's folders exist, nested as listed, with the choice saved and the feed saying where it came from.
    let from_template = setup(vec![("Runbooks", vec![]), ("Incidents", vec!["2026", "Postmortems"])]);
    let created = storage
        .create_shared_workspace_from(&owner.id, &organization.id, "Ops", Some(&from_template))
        .await
        .unwrap();
    let id = created.workspace.id.clone();
    let folders = storage.list_folders(&id).await.unwrap();
    let incidents = folders.iter().find(|folder| folder.name == "Incidents").unwrap();
    assert_eq!(folders.len(), 4);
    assert!(folders
        .iter()
        .find(|folder| folder.name == "Runbooks")
        .unwrap()
        .parent_id
        .is_none());
    assert!(incidents.parent_id.is_none());
    let mut below: Vec<&str> = folders
        .iter()
        .filter(|folder| folder.parent_id.as_deref() == Some(incidents.id.as_str()))
        .map(|folder| folder.name.as_str())
        .collect();
    below.sort();
    assert_eq!(below, ["2026", "Postmortems"]);
    assert_eq!(
        storage.workspace_template_ref(&id).await.unwrap(),
        Some(from_template.reference.clone())
    );
    assert_eq!(created.workspace.settings["template"]["id"], json!("operations"));
    let activity = storage.list_workspace_activity(&id, 10).await.unwrap();
    assert!(
        activity
            .iter()
            .any(|event| event.summary == "Created workspace Ops from the Operations and incidents template."),
        "{activity:?}"
    );

    // Without a template there are no folders and no choice, as before.
    let plain = storage
        .create_shared_workspace(&owner.id, &organization.id, "Plain")
        .await
        .unwrap();
    assert!(storage.list_folders(&plain.workspace.id).await.unwrap().is_empty());
    assert_eq!(storage.workspace_template_ref(&plain.workspace.id).await.unwrap(), None);
    assert_eq!(plain.workspace.settings, json!({}));
    let empty_template = setup(vec![]);
    let bare = storage
        .create_shared_workspace_from(&owner.id, &organization.id, "Bare", Some(&empty_template))
        .await
        .unwrap();
    assert!(storage.list_folders(&bare.workspace.id).await.unwrap().is_empty());
    assert!(storage
        .workspace_template_ref(&bare.workspace.id)
        .await
        .unwrap()
        .is_some());

    // A folder that cannot be made leaves no workspace behind: a repeated name, a slash.
    let before = storage.list_workspaces().await.unwrap().len();
    for broken in [
        setup(vec![("Same", vec![]), ("same", vec![])]),
        setup(vec![("Fine", vec!["Kid", "kid"])]),
        setup(vec![("Fine", vec![]), ("a/b", vec![])]),
    ] {
        assert!(storage
            .create_shared_workspace_from(&owner.id, &organization.id, "Broken", Some(&broken))
            .await
            .is_err());
    }
    assert_eq!(storage.list_workspaces().await.unwrap().len(), before);

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_times_files_were_last_changed_are_read_for_the_files_asked_about_only() {
    let (storage, data_dir) = temp_storage("updated-times").await;
    let workspace = storage.create_workspace("Times").await.unwrap();
    let other = storage.create_workspace("Other").await.unwrap();
    let source = storage
        .create_or_get_source(&workspace.id, SourceType::Folder, "fixture", Some("fixture"))
        .await
        .unwrap();
    let other_source = storage
        .create_or_get_source(&other.id, SourceType::Folder, "fixture", Some("fixture"))
        .await
        .unwrap();
    let mut ids = Vec::new();
    for (workspace_id, source_id, path) in [
        (&workspace.id, &source.id, "a.md"),
        (&workspace.id, &source.id, "b.md"),
        (&other.id, &other_source.id, "c.md"),
    ] {
        let bytes = path.as_bytes();
        let content_hash = StorageEngine::content_hash(bytes);
        storage.store_blob(&content_hash, bytes, None).await.unwrap();
        let artifact = storage
            .store_artifact(NewArtifact {
                workspace_id: workspace_id.clone(),
                source_id: source_id.clone(),
                artifact_type: ArtifactType::MarkdownDoc,
                title: path.to_owned(),
                path: path.to_owned(),
                content_hash,
                mime_type: None,
                language: None,
                size_bytes: bytes.len() as i64,
                metadata: json!({}),
            })
            .await
            .unwrap()
            .artifact;
        ids.push(artifact);
    }
    let asked = vec![ids[0].id.clone(), ids[2].id.clone(), "no-such-file".to_owned()];
    let times = storage.artifact_updated_times(&workspace.id, &asked).await.unwrap();
    // The file asked about is there with its time; the other workspace's file and the missing one are not.
    assert_eq!(times.len(), 1);
    assert_eq!(times[&ids[0].id], ids[0].updated_at);
    assert!(storage
        .artifact_updated_times(&workspace.id, &[])
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        storage
            .artifact_updated_times(&workspace.id, &[ids[1].id.clone()])
            .await
            .unwrap()
            .len(),
        1
    );

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_time_a_file_was_last_marked_verified_is_read_for_verified_files_only() {
    use repomemo_domain::ArtifactType;
    let (storage, data_dir) = temp_storage("verified-times").await;
    let workspace = storage.create_workspace("Verified").await.unwrap();
    let source = storage
        .create_or_get_source(&workspace.id, SourceType::Folder, "fixture", Some("fixture"))
        .await
        .unwrap();
    let mut ids = Vec::new();
    for path in ["checked.md", "plain.md"] {
        let bytes = path.as_bytes();
        let content_hash = StorageEngine::content_hash(bytes);
        storage.store_blob(&content_hash, bytes, None).await.unwrap();
        let artifact = storage
            .store_artifact(NewArtifact {
                workspace_id: workspace.id.clone(),
                source_id: source.id.clone(),
                artifact_type: ArtifactType::MarkdownDoc,
                title: path.to_owned(),
                path: path.to_owned(),
                content_hash,
                mime_type: None,
                language: None,
                size_bytes: bytes.len() as i64,
                metadata: json!({}),
            })
            .await
            .unwrap()
            .artifact;
        ids.push(artifact.id);
    }
    assert!(storage.verified_times(&workspace.id).await.unwrap().is_empty());
    let reviewer = storage
        .create_user("verifier@example.com", "Verifier", "hash")
        .await
        .unwrap();
    storage
        .save_artifact_lifecycle(
            &ids[0],
            &reviewer.id,
            crate::SaveArtifactLifecycle {
                status: "verified".to_owned(),
                owner_user_id: None,
                review_note: "Checked.".to_owned(),
                superseded_by_artifact_id: None,
            },
        )
        .await
        .unwrap();
    let times = storage.verified_times(&workspace.id).await.unwrap();
    assert_eq!(times.len(), 1);
    assert!(times.contains_key(&ids[0]) && !times.contains_key(&ids[1]));
    assert!(chrono::DateTime::parse_from_rfc3339(&times[&ids[0]]).is_ok());

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
    // Every write to a file's passages or vectors moves its number in the journal
    // that derived results (the knowledge map, health) compare against.
    let after_indexing = storage.vector_stamps(&workspace.id).of(&artifact.id);
    assert!(after_indexing > 0, "indexing is journalled");
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
    let after_embedding = storage.vector_stamps(&workspace.id).of(&artifact.id);
    assert!(after_embedding > after_indexing, "embedding is journalled");
    assert_eq!(storage.vector_stamps("another").of(&artifact.id), 0, "per workspace");
    let counts = storage
        .chunk_counts_for_artifacts(Some("m"), std::slice::from_ref(&artifact.id))
        .await
        .unwrap();
    assert_eq!(counts[&artifact.id], (4, 3), "four passages, three with a vector");
    let centroids = storage
        .artifact_embedding_centroids_for("m", std::slice::from_ref(&artifact.id))
        .await
        .unwrap();
    let everywhere = storage.artifact_embedding_centroids(&workspace.id, "m").await.unwrap();
    assert_eq!(
        centroids, everywhere,
        "reading one file is reading the workspace, for that file"
    );

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

    // Clearing a workspace's vectors is journalled for every file, and the
    // search index forgets them.
    let before_clear = storage.vector_stamps(&workspace.id).of(&artifact.id);
    assert!(storage.clear_workspace_embeddings(&workspace.id).await.unwrap() >= 4);
    assert!(storage.vector_stamps(&workspace.id).of(&artifact.id) > before_clear);
    assert!(storage
        .search_chunks_by_embedding(&request, "m", &query)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(storage.embedding_count(&workspace.id).await.unwrap(), 0);

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn mail_settings_keep_the_password_sealed_and_the_outbox_retries_then_gives_up() {
    let (storage, data_dir) = temp_storage("mail").await;
    assert!(storage.mail_settings().await.unwrap().is_none());
    storage
        .save_mail_settings(
            crate::MailSettingsUpdate {
                enabled: true,
                host: "smtp.example.com".to_owned(),
                port: 587,
                security: "starttls".to_owned(),
                username: "memo".to_owned(),
                password: Some("smtp-password-1".to_owned()),
                from_address: "memo@example.com".to_owned(),
                from_name: "RepoMemo".to_owned(),
            },
            None,
        )
        .await
        .unwrap();
    let raw = sqlx::query_scalar::<_, Option<String>>("SELECT password_sealed FROM mail_settings WHERE id = 1")
        .fetch_one(&storage.pool)
        .await
        .unwrap()
        .unwrap();
    assert!(crate::secrets::SecretBox::is_sealed(&raw) && !raw.contains("smtp-password-1"));
    let stored = storage.mail_settings().await.unwrap().unwrap();
    assert_eq!(stored.password.as_deref(), Some("smtp-password-1"));
    let view = storage.mail_settings_view().await.unwrap();
    assert!(view.configured && view.password_set);
    assert!(!format!("{view:?}").contains("smtp-password-1"));

    // The outbox: claimed once, retried later, then given up on with the body dropped.
    let entry = storage
        .enqueue_mail("ada@example.com", "Hello", "Body with a one-time link", "test")
        .await
        .unwrap();
    assert_eq!(entry.status, "queued");
    let claimed = storage.claim_due_mail(10).await.unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].body, "Body with a one-time link");
    assert!(
        storage.claim_due_mail(10).await.unwrap().is_empty(),
        "a claimed message is not claimed twice"
    );
    let later = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    storage.retry_mail_later(&entry.id, "busy", &later).await.unwrap();
    assert!(storage.claim_due_mail(10).await.unwrap().is_empty(), "not due yet");
    let retried = storage.mail_entry(&entry.id).await.unwrap();
    assert_eq!((retried.status.as_str(), retried.attempts), ("queued", 1));
    storage.mark_mail_failed(&entry.id, "gave up").await.unwrap();
    let body = sqlx::query_scalar::<_, String>("SELECT body FROM mail_outbox WHERE id = ?1")
        .bind(&entry.id)
        .fetch_one(&storage.pool)
        .await
        .unwrap();
    assert!(body.is_empty(), "a message that is finished keeps no body");
    let counts = storage.mail_outbox_counts().await.unwrap();
    assert_eq!((counts.queued, counts.failed_last_day), (0, 1));

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

    // A page of the file list in a workspace of many files (V0.2.154, target under 100 ms at
    // 100,000 files). The files are bare rows made in one statement; `REPOMEMO_PERF_LISTED_FILES`
    // sets how many (default 100,000).
    let listed: usize = std::env::var("REPOMEMO_PERF_LISTED_FILES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(100_000);
    let seed_id: String = sqlx::query_scalar("SELECT id FROM artifacts WHERE workspace_id = ?1 LIMIT 1")
        .bind(&workspace.id)
        .fetch_one(&storage.pool)
        .await
        .unwrap();
    sqlx::query(
        r#"
        WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < ?2)
        INSERT INTO artifacts (id, workspace_id, source_id, type, title, path, content_hash, mime_type, language,
                               size_bytes, created_at, updated_at, indexed_at, metadata_json)
        SELECT 'listed-' || n.i, a.workspace_id, a.source_id, a.type, 'Listed ' || n.i, 'listed/' || n.i || '.md',
               a.content_hash, a.mime_type, a.language, a.size_bytes, a.created_at,
               printf('2020-%02d-%02dT00:00:00+00:00', n.i % 12 + 1, n.i % 28 + 1), NULL, '{}'
        FROM n, artifacts a WHERE a.id = ?1
        "#,
    )
    .bind(&seed_id)
    .bind(listed as i64)
    .execute(&storage.pool)
    .await
    .unwrap();
    let mut samples = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        let (page, total) = storage.list_artifacts_page(&workspace.id, 50, 0).await.unwrap();
        let ids: Vec<String> = page.iter().map(|artifact| artifact.id.clone()).collect();
        storage.artifact_folder_ids_for(&ids).await.unwrap();
        storage.repo_artifact_sources_for(&ids).await.unwrap();
        samples.push(started.elapsed().as_millis());
        assert_eq!(page.len(), 50);
        assert!(total >= listed as i64);
    }
    println!(
        "file list, a page of 50 among {listed}+ files, p95: {} ms (target 100 ms)",
        percentile(samples)
    );
    let started = Instant::now();
    let (page, total) = storage.list_artifacts_page(&workspace.id, 50, 50_000).await.unwrap();
    println!(
        "file list, a page at offset 50,000: {:?} ({} of {total})",
        started.elapsed(),
        page.len()
    );

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Environments written by older versions open in place. The schemas of
/// V0.1.40 (migration 13), V0.1.52 (17) and V0.1.65 (23) are rebuilt from the
/// migration files up to that number, filled with data the way those versions
/// stored it, and then opened by the current engine.
#[tokio::test]
async fn environments_from_older_versions_upgrade_in_place() {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    let migrations = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    for (version, last_migration) in [("0.1.40", 13), ("0.1.52", 17), ("0.1.65", 23)] {
        let data_dir = std::env::temp_dir().join(format!("repomemo-upgrade-{version}-{}", uuid::Uuid::new_v4()));
        let old_schema = data_dir.join("old-migrations");
        std::fs::create_dir_all(&old_schema).unwrap();
        for entry in std::fs::read_dir(&migrations).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let number = name.split('_').next().and_then(|digits| digits.parse::<u32>().ok());
            if number.is_some_and(|number| number <= last_migration) {
                std::fs::copy(entry.path(), old_schema.join(&name)).unwrap();
            }
        }
        let options = SqliteConnectOptions::new()
            .filename(data_dir.join("repomemo.sqlite"))
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::migrate::Migrator::new(old_schema.as_path())
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();
        let hash = StorageEngine::content_hash(version.as_bytes());
        for statement in [
            "INSERT INTO workspaces (id, name, created_at, updated_at) VALUES ('w1', 'Old workspace', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            "INSERT INTO sources (id, workspace_id, type, name, created_at, updated_at) VALUES ('s1', 'w1', 'folder', 'docs', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            "INSERT INTO artifacts (id, workspace_id, source_id, type, title, path, content_hash, size_bytes, created_at, updated_at) VALUES ('a1', 'w1', 's1', 'markdown_doc', 'Runbook', 'runbook.md', ?1, 10, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            "INSERT INTO chunks (id, artifact_id, workspace_id, chunk_index, text, content_hash) VALUES ('c1', 'a1', 'w1', 0, 'Restart the ingestion worker after a deploy', 'h1')",
        ] {
            if statement.contains("INTO artifacts") {
                sqlx::query("INSERT INTO blobs (hash, storage_uri, size_bytes, created_at) VALUES (?1, 'blobs/x', 10, '2026-01-01T00:00:00Z')")
                    .bind(&hash)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
            let query = sqlx::query(statement);
            let query = if statement.contains("?1") { query.bind(&hash) } else { query };
            query.execute(&pool).await.unwrap_or_else(|error| panic!("{version}: {statement}: {error}"));
        }
        pool.close().await;

        // The current engine migrates the rest and serves the old data.
        let storage = StorageEngine::open(StorageConfig::new(data_dir.clone()))
            .await
            .unwrap_or_else(|error| {
                panic!("{version} did not upgrade: {error:#}");
            });
        let workspaces = storage.list_workspaces().await.unwrap();
        assert_eq!(workspaces.len(), 1, "{version}");
        let artifacts = storage.list_artifacts("w1").await.unwrap();
        assert_eq!(artifacts.len(), 1, "{version}");
        assert_eq!(artifacts[0].title, "Runbook");
        let found = storage
            .search_chunks(
                &SearchRequest {
                    workspace_id: "w1".to_owned(),
                    query: "ingestion".to_owned(),
                    limit: Some(5),
                    ..Default::default()
                },
                "\"ingestion\"*",
            )
            .await
            .unwrap();
        assert_eq!(found.len(), 1, "{version}: full-text search follows the upgrade");
        let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&storage.pool)
            .await
            .unwrap();
        assert_eq!(integrity, "ok", "{version}");
        let broken: Vec<_> = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&storage.pool)
            .await
            .unwrap();
        assert!(broken.is_empty(), "{version}: foreign keys hold after the upgrade");
        storage.pool.close().await;
        drop(storage);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

#[tokio::test]
async fn earlier_versions_are_kept_and_their_blobs_survive_collection() {
    let (storage, data_dir) = temp_storage("versions").await;
    let workspace = storage.create_workspace("Versions").await.unwrap();
    let source = storage
        .create_or_get_source(&workspace.id, SourceType::Upload, "Uploads", None)
        .await
        .unwrap();
    let first = b"first text";
    let first_hash = StorageEngine::content_hash(first);
    storage
        .store_blob(&first_hash, first, Some("text/plain"))
        .await
        .unwrap();
    let artifact = storage
        .store_artifact(NewArtifact {
            workspace_id: workspace.id.clone(),
            source_id: source.id.clone(),
            artifact_type: ArtifactType::File,
            title: "notes".to_owned(),
            path: "notes.txt".to_owned(),
            content_hash: first_hash.clone(),
            mime_type: Some("text/plain".to_owned()),
            language: None,
            size_bytes: first.len() as i64,
            metadata: json!({}),
        })
        .await
        .unwrap()
        .artifact;
    let user = storage
        .create_user("versions@example.com", "Versions", "hash")
        .await
        .unwrap();

    // Never replaced: one version, the current one.
    let alone = storage.list_artifact_versions(&artifact.id).await.unwrap();
    assert_eq!(alone.len(), 1);
    assert!(alone[0].is_current && alone[0].version == 1 && alone[0].uploaded_by.is_none());

    let mut hashes = vec![first_hash.clone()];
    for text in ["second text", "third text, longer"] {
        let hash = StorageEngine::content_hash(text.as_bytes());
        storage
            .store_blob(&hash, text.as_bytes(), Some("text/plain"))
            .await
            .unwrap();
        storage
            .replace_artifact_content(
                &artifact.id,
                &hash,
                text.len() as i64,
                Some("text/plain"),
                Some(&user.id),
            )
            .await
            .unwrap();
        hashes.push(hash);
    }
    // The same content again is refused and leaves no version behind.
    assert!(storage
        .replace_artifact_content(&artifact.id, &hashes[2], 1, None, Some(&user.id))
        .await
        .is_err());

    let versions = storage.list_artifact_versions(&artifact.id).await.unwrap();
    assert_eq!(
        versions.iter().map(|version| version.version).collect::<Vec<_>>(),
        [3, 2, 1]
    );
    assert_eq!(
        versions
            .iter()
            .map(|version| version.content_hash.clone())
            .collect::<Vec<_>>(),
        [hashes[2].clone(), hashes[1].clone(), hashes[0].clone()]
    );
    assert!(versions[0].is_current && !versions[1].is_current && versions[0].replaced_at.is_none());
    // Version 1 has no recorded author; the others were uploaded by the user.
    assert_eq!(versions[2].uploaded_by, None);
    assert_eq!(versions[1].uploaded_by.as_deref(), Some("Versions"));
    assert_eq!(versions[0].uploaded_by.as_deref(), Some("Versions"));
    assert_eq!(versions[0].created_at, versions[1].replaced_at.clone().unwrap());

    // Earlier content is not unreferenced: collecting leaves every blob.
    let collected = storage
        .collect_orphan_blobs(chrono::Duration::seconds(-1), 100)
        .await
        .unwrap();
    assert!(collected.removed.is_empty(), "{:?}", collected.removed);
    assert_eq!(storage.read_blob(&hashes[0]).await.unwrap(), first);

    // They count in what the workspace holds, until the file goes.
    assert_eq!(
        storage.workspace_stored_bytes(&workspace.id).await.unwrap(),
        (first.len() + "second text".len() + "third text, longer".len()) as i64
    );

    // Deleting the file lets them go.
    storage.delete_artifact(&artifact.id).await.unwrap();
    let collected = storage
        .collect_orphan_blobs(chrono::Duration::seconds(-1), 100)
        .await
        .unwrap();
    assert_eq!(collected.removed.len(), 3);

    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// The last migration of RepoMemo 0.2.0 (the tag `RM-V0.2.0`). Every release after it only adds migrations.
const V0_2_0_LAST_MIGRATION: i64 = 24;

/// Roadmap R9 (V0.2.53). The environment folder of a server made by 0.2.0 (its schema, the rows it wrote
/// and the files it stored) is opened by the current code, which migrates it in place, and everything in it
/// is still there and still works: the people, the workspace, its files and their text, the memory card
/// with the passage and the file it cites, the task, the comment, the lifecycle and the system setting.
/// What 0.2.0 wrote is kept in `tests/fixtures/v0.2.0-seed.sql`; the migrations are the ones of the code
/// itself, cut at 0.2.0 (`scripts/check-migrations.mjs` keeps a released migration from being edited).
#[tokio::test]
async fn an_environment_made_by_v0_2_0_upgrades_in_place_and_keeps_its_data() {
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

    const REFUNDS: &[u8] = b"Refunds are issued within fourteen days of purchase.";
    const SHIPPING: &[u8] = b"Parcels leave the warehouse every weekday.";
    let data_dir = std::env::temp_dir().join(format!(
        "repomemo-upgrade-test-{}",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&data_dir).unwrap();

    // The environment as 0.2.0 left it: its migrations only, the rows it wrote, the files it stored.
    let options = SqliteConnectOptions::new()
        .filename(data_dir.join("repomemo.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let mut released = sqlx::migrate::Migrator::DEFAULT;
    released.migrations = std::borrow::Cow::Owned(
        super::MIGRATOR
            .migrations
            .iter()
            .filter(|migration| migration.version <= V0_2_0_LAST_MIGRATION)
            .cloned()
            .collect(),
    );
    assert_eq!(
        released.migrations.len() as i64,
        V0_2_0_LAST_MIGRATION,
        "no migration is missing or renumbered"
    );
    released.run(&pool).await.unwrap();
    sqlx::raw_sql(include_str!("../tests/fixtures/v0.2.0-seed.sql"))
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    for (bytes, hash) in [
        (
            REFUNDS,
            "25b9033a79705adbe5e5e258fee1e70adab08041373f9b0078fc0b9c3ffbe561",
        ),
        (
            SHIPPING,
            "5eb51d20b9012cdff150b9825024a9ae9514028e20e158524e8e0861020df504",
        ),
    ] {
        assert_eq!(
            StorageEngine::content_hash(bytes),
            hash,
            "the seed names the file it holds"
        );
        let place = data_dir.join("blobs").join(&hash[0..2]).join(&hash[2..4]);
        std::fs::create_dir_all(&place).unwrap();
        std::fs::write(place.join(hash), bytes).unwrap();
    }

    // The current code opens it: the migrations after 0.2.0 run in place.
    let storage = StorageEngine::open(StorageConfig::new(data_dir.clone())).await.unwrap();
    let (applied, failed): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END), 0) FROM _sqlx_migrations",
    )
    .fetch_one(storage.reader())
    .await
    .unwrap();
    assert_eq!(applied as usize, super::MIGRATOR.migrations.len());
    assert_eq!(failed, 0);
    assert!(
        applied > V0_2_0_LAST_MIGRATION,
        "the migrations after 0.2.0 were applied"
    );

    // People, organization and workspace.
    let grace = storage
        .find_user_by_email("grace@example.com")
        .await
        .unwrap()
        .expect("an account is kept");
    assert_eq!(grace.display_name, "Grace");
    let workspaces = storage.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].name, "Atlas");

    // The workspace as the current export reads it: every record, with what the new migrations added.
    let data = storage.export_workspace_data("w-atlas").await.unwrap();
    assert_eq!(data.folders.len(), 1);
    assert_eq!(data.artifacts.len(), 2);
    let refunds = data
        .artifacts
        .iter()
        .find(|artifact| artifact.title == "Refund policy")
        .unwrap();
    assert_eq!(refunds.folder_id.as_deref(), Some("f-policies"));
    assert_eq!(data.lifecycle.len(), 1);
    assert_eq!(data.lifecycle[0].status, "verified");
    assert_eq!(data.lifecycle[0].events.len(), 1);
    assert_eq!(data.memory_cards.len(), 1);
    assert_eq!(
        data.memory_cards[0].citations.len(),
        2,
        "the passage and the file it cites"
    );
    assert_eq!(data.omitted_citations, 0);
    assert_eq!(data.tasks.len(), 1);
    assert_eq!(data.tasks[0].checklist.len(), 1);
    assert_eq!(data.comments.len(), 1);
    assert_eq!(data.comments[0].body, "Looks right to me.");

    // The stored files and the text search.
    assert_eq!(storage.read_blob(&refunds.content_hash).await.unwrap(), REFUNDS);
    let request = SearchRequest {
        workspace_id: "w-atlas".to_owned(),
        query: "parcels".to_owned(),
        limit: Some(5),
        ..Default::default()
    };
    let found = storage.search_chunks(&request, "\"parcels\"*").await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title, "Shipping notes");

    // The system setting and a first write after the upgrade.
    let settings = storage.list_system_settings().await.unwrap();
    assert!(settings
        .iter()
        .any(|setting| setting.key == "log_http_level" && setting.value == json!("debug")));
    let added = storage.create_workspace("Added after the upgrade").await.unwrap();
    assert_eq!(storage.list_workspaces().await.unwrap().len(), 2);
    assert!(storage.workspace_exists(&added.id).await.unwrap());

    // And the folder opens again: nothing is migrated twice.
    storage.close().await;
    let again = StorageEngine::open(StorageConfig::new(data_dir.clone())).await.unwrap();
    let (reapplied,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(again.reader())
        .await
        .unwrap();
    assert_eq!(reapplied, applied);
    drop(again);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// S5-4: two tabs that refresh at the same moment present the same cookie. The second must not end the
/// account's sessions; an older rotated token (real reuse) still does.
#[tokio::test]
async fn a_token_rotated_a_moment_ago_is_refused_without_ending_the_sessions_but_older_reuse_ends_them() {
    use crate::RefreshOutcome::{JustRotated, Refused, Rotated};
    let (storage, data_dir) = temp_storage("rotation-grace").await;
    let user = storage.create_user("grace@example.com", "Grace", "hash").await.unwrap();
    let first = storage.create_refresh_token(&user.id, 30).await.unwrap();
    assert_eq!(
        storage.consume_refresh_token(&first).await.unwrap(),
        Rotated(user.id.clone())
    );
    // The cookie the first tab received.
    let second = storage.create_refresh_token(&user.id, 30).await.unwrap();

    // The second tab presents the old cookie a moment later: refused, nothing revoked.
    assert_eq!(storage.consume_refresh_token(&first).await.unwrap(), JustRotated);
    assert_eq!(
        storage.consume_refresh_token(&first).await.unwrap(),
        JustRotated,
        "and again"
    );
    assert_eq!(
        storage.refresh_token_user(&second).await.unwrap().as_deref(),
        Some(user.id.as_str()),
        "the new token still works"
    );

    // Presented long after its rotation, the same token is theft: every session ends.
    sqlx::query("UPDATE refresh_tokens SET replaced_at = '2000-01-01T00:00:00+00:00' WHERE token_hash = ?1")
        .bind(crate::hash_refresh_token(&first))
        .execute(&storage.pool)
        .await
        .unwrap();
    assert_eq!(storage.consume_refresh_token(&first).await.unwrap(), Refused);
    assert_eq!(
        storage.consume_refresh_token(&second).await.unwrap(),
        Refused,
        "reuse ended every session"
    );

    // A token revoked for another reason is reuse at once: signing out, then presenting it again.
    let third = storage.create_refresh_token(&user.id, 30).await.unwrap();
    let fourth = storage.create_refresh_token(&user.id, 30).await.unwrap();
    storage.revoke_refresh_token(&third).await.unwrap();
    assert_eq!(storage.consume_refresh_token(&third).await.unwrap(), Refused);
    assert_eq!(storage.consume_refresh_token(&fourth).await.unwrap(), Refused);

    // An unknown token is just refused.
    assert_eq!(storage.consume_refresh_token("unknown").await.unwrap(), Refused);
    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// E13: a new address or name refuses control and text-direction characters; looking an address up stays as
/// lenient as before, so accounts made earlier still sign in.
#[tokio::test]
async fn new_addresses_and_names_refuse_hidden_characters_but_lookups_stay_lenient() {
    let (storage, data_dir) = temp_storage("hidden-characters").await;
    for refused in [
        "ada lovelace@example.com",
        "ada@exam ple.com",
        "ada\n@example.com",
        "ada@example.com\u{0}",
        "ada\u{202E}@example.com",
        "ada\u{2066}@example.com",
        "ada@@example.com",
        "ada@b@example.com",
        "@example.com",
        "ada@",
    ] {
        assert!(
            storage.create_user(refused, "Ada", "hash").await.is_err(),
            "{refused:?} should be refused for a new account"
        );
    }
    let user = storage.create_user("Ada@Example.com ", "Ada", "hash").await.unwrap();
    assert_eq!(user.email.as_deref(), Some("ada@example.com"));
    assert!(
        storage.find_user_by_email(" ADA@example.com").await.unwrap().is_some(),
        "lookups stay lenient"
    );

    assert!(storage
        .change_user_email(&user.id, "ada@lovelace\u{202E}.com")
        .await
        .is_err());
    assert!(storage.update_user_display_name(&user.id, "Ada\u{202E}").await.is_err());
    assert!(storage
        .update_user_display_name(&user.id, "Ada\nLovelace")
        .await
        .is_err());
    assert!(storage.update_user_display_name(&user.id, "Ada Lovelace").await.is_ok());
    assert!(storage.create_organization(&user.id, "Team\u{2067}").await.is_err());
    assert!(storage.create_organization(&user.id, "Engineering").await.is_ok());

    // What is written to a log is the Debug form, which cannot start a line of its own.
    let hostile = "ada@example.com\nINFO forged line";
    assert!(!format!("{hostile:?}").contains('\n'));
    storage.pool.close().await;
    drop(storage);
    let _ = std::fs::remove_dir_all(data_dir);
}
