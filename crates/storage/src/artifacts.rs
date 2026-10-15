use super::*;

impl StorageEngine {
    pub async fn create_or_get_source(
        &self,
        workspace_id: &str,
        source_type: SourceType,
        name: &str,
        root_uri: Option<&str>,
    ) -> Result<Source> {
        if let Some(row) = sqlx::query_as::<_, SourceRow>(
            r#"
            SELECT
              id,
              workspace_id,
              type AS source_type,
              name,
              root_uri,
              last_indexed_at,
              status,
              metadata_json,
              created_at,
              updated_at
            FROM sources
            WHERE workspace_id = ?1
              AND type = ?2
              AND COALESCE(root_uri, '') = COALESCE(?3, '')
            LIMIT 1
            "#,
        )
        .bind(workspace_id)
        .bind(source_type_to_db(&source_type))
        .bind(root_uri)
        .fetch_optional(self.reader())
        .await?
        {
            return Ok(Source::from(row));
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO sources (
              id,
              workspace_id,
              type,
              name,
              root_uri,
              status,
              metadata_json,
              created_at,
              updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, 'ready', '{}', ?6, ?7)
            "#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(source_type_to_db(&source_type))
        .bind(name)
        .bind(root_uri)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(Source {
            id,
            workspace_id: workspace_id.to_owned(),
            source_type,
            name: name.to_owned(),
            root_uri: root_uri.map(str::to_owned),
            last_indexed_at: None,
            status: "ready".to_owned(),
            metadata: Value::Object(Default::default()),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub async fn store_blob(&self, content_hash: &str, bytes: &[u8], mime_type: Option<&str>) -> Result<String> {
        if content_hash.len() < 4 || !content_hash.chars().all(|value| value.is_ascii_alphanumeric()) {
            bail!("content hash is invalid");
        }

        let blob_path = self.blob_path(content_hash);
        if let Some(parent) = blob_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let storage_uri = self.blob_storage_uri(content_hash);
        let now = Utc::now().to_rfc3339();

        let _guard = self.blob_lock.lock().await;
        // The row is written (or refreshed) first: `created_at` doubles as
        // "last stored at", so garbage collection treats a blob that is being
        // stored again as fresh even if no artifact references it yet.
        sqlx::query(
            r#"
            INSERT INTO blobs (hash, storage_uri, size_bytes, mime_type, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(hash) DO UPDATE SET created_at = excluded.created_at
            "#,
        )
        .bind(content_hash)
        .bind(&storage_uri)
        .bind(bytes.len() as i64)
        .bind(mime_type)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        if tokio::fs::metadata(&blob_path).await.is_err() {
            // Write to a temporary name and rename, so a crash mid-write never
            // leaves a truncated blob that later looks complete.
            let staging = blob_path.with_extension(format!("tmp-{}", Uuid::new_v4().simple()));
            let written = async {
                tokio::fs::write(&staging, bytes).await?;
                tokio::fs::rename(&staging, &blob_path).await
            }
            .await;
            if let Err(error) = written {
                let _ = tokio::fs::remove_file(&staging).await;
                return Err(error).with_context(|| format!("failed to write blob {}", blob_path.display()));
            }
        }

        Ok(storage_uri)
    }

    /// Deletes blobs that no artifact references any more and that were not
    /// stored again within `grace`, at most `limit` per call. Deleting an
    /// artifact or a workspace, or a repository file changing, leaves its old
    /// content behind; this reclaims it.
    pub async fn collect_orphan_blobs(&self, grace: chrono::Duration, limit: i64) -> Result<BlobCollection> {
        let cutoff = (Utc::now() - grace).to_rfc3339();
        let candidates = sqlx::query_as::<_, (String, i64)>(
            r#"
            SELECT hash, size_bytes FROM blobs
            WHERE created_at < ?1
              AND NOT EXISTS (SELECT 1 FROM artifacts WHERE artifacts.content_hash = blobs.hash)
            LIMIT ?2
            "#,
        )
        .bind(&cutoff)
        .bind(limit.max(1))
        .fetch_all(self.reader())
        .await?;

        let mut collection = BlobCollection::default();
        for (hash, size_bytes) in candidates {
            let _guard = self.blob_lock.lock().await;
            // Re-check under the lock: the blob may have been stored again or
            // referenced since it was listed.
            let deleted = sqlx::query(
                r#"
                DELETE FROM blobs
                WHERE hash = ?1 AND created_at < ?2
                  AND NOT EXISTS (SELECT 1 FROM artifacts WHERE artifacts.content_hash = ?1)
                "#,
            )
            .bind(&hash)
            .bind(&cutoff)
            .execute(&self.pool)
            .await?
            .rows_affected();
            if deleted == 0 {
                continue;
            }
            match tokio::fs::remove_file(self.blob_path(&hash)).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    tracing::warn!(hash = %hash, error = %error, "Could not delete an unreferenced blob file");
                }
            }
            collection.bytes += size_bytes;
            collection.removed.push(hash);
        }
        Ok(collection)
    }

    /// Deletes files in the blob directory that have no `blobs` row (left by a
    /// crash between writing a file and recording it, or by an interrupted
    /// write) once they are older than `grace`. Returns how many were removed.
    pub async fn sweep_untracked_blob_files(&self, grace: std::time::Duration) -> Result<u64> {
        let mut files = Vec::new();
        let mut directories = vec![self.blob_dir.clone()];
        while let Some(directory) = directories.pop() {
            let mut entries = match tokio::fs::read_dir(&directory).await {
                Ok(entries) => entries,
                Err(_) => continue,
            };
            while let Some(entry) = entries.next_entry().await? {
                let file_type = entry.file_type().await?;
                if file_type.is_dir() {
                    directories.push(entry.path());
                } else if file_type.is_file() {
                    files.push(entry.path());
                }
            }
        }

        let mut removed = 0;
        for path in files {
            let old_enough = tokio::fs::metadata(&path)
                .await
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age >= grace);
            if !old_enough {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_owned();
            let _guard = self.blob_lock.lock().await;
            let tracked = !name.contains('.')
                && sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM blobs WHERE hash = ?1")
                    .bind(&name)
                    .fetch_one(self.reader())
                    .await?
                    > 0;
            if !tracked && tokio::fs::remove_file(&path).await.is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// Whether a blob with this content hash is still stored.
    pub async fn blob_exists(&self, content_hash: &str) -> Result<bool> {
        Ok(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM blobs WHERE hash = ?1")
                .bind(content_hash)
                .fetch_one(self.reader())
                .await?
                > 0,
        )
    }

    pub async fn store_artifact(&self, artifact: NewArtifact) -> Result<StoredArtifact> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let metadata_json = serde_json::to_string(&artifact.metadata)?;

        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO artifacts (
              id,
              workspace_id,
              source_id,
              type,
              title,
              path,
              content_hash,
              mime_type,
              language,
              size_bytes,
              created_at,
              updated_at,
              metadata_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
        )
        .bind(&id)
        .bind(&artifact.workspace_id)
        .bind(&artifact.source_id)
        .bind(artifact_type_to_db(&artifact.artifact_type))
        .bind(&artifact.title)
        .bind(&artifact.path)
        .bind(&artifact.content_hash)
        .bind(&artifact.mime_type)
        .bind(&artifact.language)
        .bind(artifact.size_bytes)
        .bind(&now)
        .bind(&now)
        .bind(&metadata_json)
        .execute(&self.pool)
        .await?;

        let created = result.rows_affected() > 0;
        let stored = self
            .get_artifact_summary_by_identity(
                &artifact.workspace_id,
                &artifact.source_id,
                &artifact.path,
                &artifact.content_hash,
            )
            .await?;

        Ok(StoredArtifact {
            artifact: stored,
            created,
        })
    }

    pub async fn list_artifacts(&self, workspace_id: &str) -> Result<Vec<ArtifactSummary>> {
        let rows = sqlx::query_as::<_, ArtifactSummaryRow>(
            r#"
            SELECT
              artifacts.id,
              artifacts.workspace_id,
              artifacts.source_id,
              sources.name AS source_name,
              artifacts.type AS artifact_type,
              artifacts.title,
              artifacts.path,
              artifacts.content_hash,
              artifacts.mime_type,
              artifacts.language,
              artifacts.size_bytes,
              artifacts.created_at,
              artifacts.updated_at,
              artifacts.indexed_at
            FROM artifacts
            JOIN sources ON sources.id = artifacts.source_id
            WHERE artifacts.workspace_id = ?1
            ORDER BY artifacts.updated_at DESC, artifacts.path ASC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;

        Ok(rows.into_iter().map(ArtifactSummary::from).collect())
    }

    /// Artifacts whose index is missing or was produced by an older indexer,
    /// oldest first. Images are only listed while they have never been
    /// indexed, so a version bump never re-runs paid vision analysis.
    /// `workspace_id` limits the scan to one workspace.
    /// Files of a repository that left its tree are never listed. Files still
    /// in a repository are listed only with `include_repo_files`, because
    /// repository syncs index their own files in one job.
    pub async fn list_artifacts_needing_index(
        &self,
        workspace_id: Option<&str>,
        current_index_version: i64,
        include_repo_files: bool,
    ) -> Result<Vec<ArtifactSummary>> {
        let rows = sqlx::query_as::<_, ArtifactSummaryRow>(
            r#"
            SELECT
              artifacts.id,
              artifacts.workspace_id,
              artifacts.source_id,
              sources.name AS source_name,
              artifacts.type AS artifact_type,
              artifacts.title,
              artifacts.path,
              artifacts.content_hash,
              artifacts.mime_type,
              artifacts.language,
              artifacts.size_bytes,
              artifacts.created_at,
              artifacts.updated_at,
              artifacts.indexed_at
            FROM artifacts
            JOIN sources ON sources.id = artifacts.source_id
            LEFT JOIN repo_files ON repo_files.artifact_id = artifacts.id
            WHERE (?1 IS NULL OR artifacts.workspace_id = ?1)
              AND (
                artifacts.indexed_at IS NULL
                OR (artifacts.type != 'image' AND artifacts.index_version < ?2)
              )
              AND (repo_files.artifact_id IS NULL OR (?3 AND repo_files.removed_at IS NULL))
            ORDER BY artifacts.created_at ASC
            "#,
        )
        .bind(workspace_id)
        .bind(current_index_version)
        .bind(include_repo_files)
        .fetch_all(self.reader())
        .await?;

        Ok(rows.into_iter().map(ArtifactSummary::from).collect())
    }

    pub async fn get_artifact(&self, artifact_id: &str) -> Result<ArtifactDetail> {
        let row = sqlx::query_as::<_, ArtifactDetailRow>(
            r#"
            SELECT
              artifacts.id,
              artifacts.workspace_id,
              artifacts.source_id,
              sources.name AS source_name,
              artifacts.type AS artifact_type,
              artifacts.title,
              artifacts.path,
              artifacts.content_hash,
              artifacts.mime_type,
              artifacts.language,
              artifacts.size_bytes,
              artifacts.created_at,
              artifacts.updated_at,
              artifacts.indexed_at,
              artifacts.metadata_json
            FROM artifacts
            JOIN sources ON sources.id = artifacts.source_id
            WHERE artifacts.id = ?1
            "#,
        )
        .bind(artifact_id)
        .fetch_one(self.reader())
        .await?;

        let summary = ArtifactSummary::from(row.summary_row());
        let metadata = serde_json::from_str(&row.metadata_json).unwrap_or_else(|_| Value::Object(Default::default()));
        let bytes = tokio::fs::read(self.blob_path(&row.content_hash))
            .await
            .unwrap_or_default();
        let preview_limit = 120_000;
        let content_truncated = bytes.len() > preview_limit;
        let preview_bytes = if content_truncated {
            &bytes[..preview_limit]
        } else {
            &bytes
        };
        let content_preview = String::from_utf8(preview_bytes.to_vec()).ok();
        let chunks = self.list_chunks_for_artifact(artifact_id).await?;

        Ok(ArtifactDetail {
            summary,
            metadata,
            content_preview,
            content_truncated,
            chunks,
        })
    }

    pub async fn get_artifact_summary(&self, artifact_id: &str) -> Result<ArtifactSummary> {
        let row = sqlx::query_as::<_, ArtifactSummaryRow>(
            r#"
            SELECT
              artifacts.id,
              artifacts.workspace_id,
              artifacts.source_id,
              sources.name AS source_name,
              artifacts.type AS artifact_type,
              artifacts.title,
              artifacts.path,
              artifacts.content_hash,
              artifacts.mime_type,
              artifacts.language,
              artifacts.size_bytes,
              artifacts.created_at,
              artifacts.updated_at,
              artifacts.indexed_at
            FROM artifacts
            JOIN sources ON sources.id = artifacts.source_id
            WHERE artifacts.id = ?1
            "#,
        )
        .bind(artifact_id)
        .fetch_one(self.reader())
        .await?;

        Ok(ArtifactSummary::from(row))
    }

    pub async fn update_artifact_title(&self, artifact_id: &str, title: &str) -> Result<ArtifactSummary> {
        let title = require_name(title, "Artifact title")?;
        let now = Utc::now().to_rfc3339();
        let changed = sqlx::query("UPDATE artifacts SET title = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(&title)
            .bind(&now)
            .bind(artifact_id)
            .execute(&self.pool)
            .await?;
        if changed.rows_affected() == 0 {
            repomemo_domain::bail_not_found!("Artifact was not found.");
        }
        self.get_artifact_summary(artifact_id).await
    }

    pub async fn delete_artifact(&self, artifact_id: &str) -> Result<()> {
        let summary = self.get_artifact_summary(artifact_id).await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM links WHERE workspace_id = ?1 AND (to_id = ?2 OR (to_type = 'chunk' AND to_id IN (SELECT id FROM chunks WHERE artifact_id = ?2)))",
        )
        .bind(&summary.workspace_id)
        .bind(artifact_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM artifacts WHERE id = ?1")
            .bind(artifact_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn read_artifact_blob(&self, artifact_id: &str) -> Result<Vec<u8>> {
        let content_hash = sqlx::query_scalar::<_, String>(
            r#"
            SELECT content_hash
            FROM artifacts
            WHERE id = ?1
            "#,
        )
        .bind(artifact_id)
        .fetch_one(self.reader())
        .await?;

        tokio::fs::read(self.blob_path(&content_hash))
            .await
            .with_context(|| format!("failed to read blob for artifact {artifact_id}"))
    }

    pub async fn replace_artifact_chunks(&self, artifact_id: &str, chunks: Vec<Chunk>) -> Result<()> {
        self.replace_artifact_index(artifact_id, chunks, Vec::new(), 0).await
    }

    /// Replaces the chunks and symbols of an artifact.
    ///
    /// A new chunk whose text matches an existing chunk of the artifact takes
    /// over that row instead of being re-created. Its id, embedding and any
    /// memory-card links therefore survive a re-index; only chunks whose text
    /// actually changed are dropped and inserted.
    pub async fn replace_artifact_index(
        &self,
        artifact_id: &str,
        chunks: Vec<Chunk>,
        symbols: Vec<Symbol>,
        index_version: i64,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;

        // Take the write lock before reading. A transaction that starts with a
        // read and then tries to write fails at once with "database is locked"
        // when another indexing task commits in between, instead of waiting.
        sqlx::query("UPDATE artifacts SET index_version = index_version WHERE id = ?1")
            .bind(artifact_id)
            .execute(&mut *tx)
            .await?;

        let existing = sqlx::query_as::<_, (String, String)>(
            "SELECT id, content_hash FROM chunks WHERE artifact_id = ?1 ORDER BY chunk_index",
        )
        .bind(artifact_id)
        .fetch_all(&mut *tx)
        .await?;
        let mut reusable: HashMap<String, VecDeque<String>> = HashMap::new();
        for (id, content_hash) in existing {
            reusable.entry(content_hash).or_default().push_back(id);
        }
        let planned = chunks
            .into_iter()
            .map(|chunk| {
                let reused_id = reusable.get_mut(&chunk.content_hash).and_then(VecDeque::pop_front);
                (chunk, reused_id)
            })
            .collect::<Vec<_>>();

        for id in reusable.into_values().flatten() {
            sqlx::query("DELETE FROM chunks WHERE id = ?1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        // Park the surviving rows on negative indexes so renumbering cannot
        // collide with UNIQUE(artifact_id, chunk_index) midway.
        sqlx::query("UPDATE chunks SET chunk_index = -chunk_index - 1 WHERE artifact_id = ?1")
            .bind(artifact_id)
            .execute(&mut *tx)
            .await?;

        for (mut chunk, reused_id) in planned {
            let metadata_json = serde_json::to_string(&chunk.metadata)?;

            if let Some(id) = reused_id {
                sqlx::query(
                    r#"
                    UPDATE chunks
                    SET chunk_index = ?2,
                        text = ?3,
                        token_count = ?4,
                        start_line = ?5,
                        end_line = ?6,
                        heading_path = ?7,
                        metadata_json = ?8
                    WHERE id = ?1
                    "#,
                )
                .bind(&id)
                .bind(chunk.chunk_index)
                .bind(&chunk.text)
                .bind(chunk.token_count)
                .bind(chunk.start_line)
                .bind(chunk.end_line)
                .bind(&chunk.heading_path)
                .bind(&metadata_json)
                .execute(&mut *tx)
                .await?;
                continue;
            }

            if chunk.id.is_empty() {
                chunk.id = Uuid::new_v4().to_string();
            }

            sqlx::query(
                r#"
                INSERT INTO chunks (
                  id,
                  artifact_id,
                  workspace_id,
                  chunk_index,
                  text,
                  token_count,
                  start_line,
                  end_line,
                  heading_path,
                  content_hash,
                  embedding_status,
                  metadata_json
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                "#,
            )
            .bind(&chunk.id)
            .bind(&chunk.artifact_id)
            .bind(&chunk.workspace_id)
            .bind(chunk.chunk_index)
            .bind(&chunk.text)
            .bind(chunk.token_count)
            .bind(chunk.start_line)
            .bind(chunk.end_line)
            .bind(&chunk.heading_path)
            .bind(&chunk.content_hash)
            .bind(&chunk.embedding_status)
            .bind(&metadata_json)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query("DELETE FROM symbols WHERE artifact_id = ?1")
            .bind(artifact_id)
            .execute(&mut *tx)
            .await?;

        for mut symbol in symbols {
            if symbol.id.is_empty() {
                symbol.id = Uuid::new_v4().to_string();
            }
            let metadata_json = serde_json::to_string(&symbol.metadata)?;
            sqlx::query(
                r#"
                INSERT INTO symbols (
                  id, artifact_id, workspace_id, kind, name, signature,
                  start_line, end_line, metadata_json
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
            )
            .bind(&symbol.id)
            .bind(&symbol.artifact_id)
            .bind(&symbol.workspace_id)
            .bind(symbol_kind_to_db(&symbol.kind))
            .bind(&symbol.name)
            .bind(&symbol.signature)
            .bind(symbol.start_line)
            .bind(symbol.end_line)
            .bind(&metadata_json)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            r#"
            UPDATE artifacts
            SET updated_at = CASE WHEN indexed_at IS NULL THEN ?1 ELSE updated_at END,
                indexed_at = ?1,
                index_version = ?3
            WHERE id = ?2
            "#,
        )
        .bind(&now)
        .bind(artifact_id)
        .bind(index_version)
        .execute(&mut *tx)
        .await?;
        let workspace_id = sqlx::query_scalar::<_, String>("SELECT workspace_id FROM artifacts WHERE id = ?1")
            .bind(artifact_id)
            .fetch_optional(&mut *tx)
            .await?;

        tx.commit().await?;
        if let Some(workspace_id) = workspace_id {
            self.vector_changes.touch(&workspace_id, [artifact_id]);
        }

        Ok(())
    }

    pub async fn get_folder(&self, folder_id: &str) -> Result<Folder> {
        let row = sqlx::query("SELECT id, workspace_id, parent_id, name, created_at FROM folders WHERE id = ?1")
            .bind(folder_id)
            .fetch_optional(self.reader())
            .await?
            .ok_or_else(|| repomemo_domain::not_found!("Folder was not found."))?;
        Ok(folder_from_row(&row))
    }

    pub async fn list_folders(&self, workspace_id: &str) -> Result<Vec<Folder>> {
        let rows = sqlx::query(
            r#"
            SELECT id, workspace_id, parent_id, name, created_at
            FROM folders WHERE workspace_id = ?1
            ORDER BY name COLLATE NOCASE
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.iter().map(folder_from_row).collect())
    }

    /// Creates a folder, enforcing a non-empty name, a unique name among its
    /// siblings, and the nesting limit.
    pub async fn create_folder(
        &self,
        workspace_id: &str,
        parent_id: Option<&str>,
        name: &str,
        created_by: Option<&str>,
    ) -> Result<Folder> {
        let name = validate_folder_name(name)?;

        let mut depth = 1;
        if let Some(parent_id) = parent_id {
            let mut cursor = Some(self.get_folder(parent_id).await?);
            if cursor.as_ref().map(|folder| folder.workspace_id.as_str()) != Some(workspace_id) {
                repomemo_domain::bail_not_found!("Folder was not found.");
            }
            while let Some(folder) = cursor {
                depth += 1;
                if depth > MAX_FOLDER_DEPTH {
                    repomemo_domain::bail_invalid!("Folders can be nested up to {MAX_FOLDER_DEPTH} levels deep.");
                }
                cursor = match folder.parent_id {
                    Some(parent) => Some(self.get_folder(&parent).await?),
                    None => None,
                };
            }
        }

        let duplicate: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT 1 FROM folders
            WHERE workspace_id = ?1 AND COALESCE(parent_id, '') = COALESCE(?2, '')
              AND name = ?3 COLLATE NOCASE
            "#,
        )
        .bind(workspace_id)
        .bind(parent_id)
        .bind(name)
        .fetch_optional(self.reader())
        .await?;
        if duplicate.is_some() {
            repomemo_domain::bail_conflict!("A folder with this name already exists here.");
        }

        let id = Uuid::new_v4().to_string();
        let created_at = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO folders (id, workspace_id, parent_id, name, created_by, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&id)
        .bind(workspace_id)
        .bind(parent_id)
        .bind(name)
        .bind(created_by)
        .bind(&created_at)
        .execute(&self.pool)
        .await?;
        self.get_folder(&id).await
    }

    pub async fn rename_folder(&self, folder_id: &str, name: &str) -> Result<Folder> {
        let name = validate_folder_name(name)?;
        let folder = self.get_folder(folder_id).await?;
        let duplicate: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT 1 FROM folders
            WHERE workspace_id = ?1 AND COALESCE(parent_id, '') = COALESCE(?2, '')
              AND name = ?3 COLLATE NOCASE AND id <> ?4
            "#,
        )
        .bind(&folder.workspace_id)
        .bind(&folder.parent_id)
        .bind(name)
        .bind(folder_id)
        .fetch_optional(self.reader())
        .await?;
        if duplicate.is_some() {
            repomemo_domain::bail_conflict!("A folder with this name already exists here.");
        }
        sqlx::query("UPDATE folders SET name = ?2 WHERE id = ?1")
            .bind(folder_id)
            .bind(name)
            .execute(&self.pool)
            .await?;
        self.get_folder(folder_id).await
    }

    /// Moves a folder, with everything in it, below another folder of the same
    /// workspace (or to the top level). A folder cannot move into itself or
    /// into one of its own subfolders, the nesting limit still holds for the
    /// deepest folder it contains, and the name must be free in its new place.
    pub async fn move_folder(&self, folder_id: &str, new_parent_id: Option<&str>) -> Result<Folder> {
        let folder = self.get_folder(folder_id).await?;
        let all = self.list_folders(&folder.workspace_id).await?;
        let by_id: HashMap<&str, &Folder> = all.iter().map(|candidate| (candidate.id.as_str(), candidate)).collect();
        // Levels above the new place.
        let mut above = 0;
        if let Some(parent_id) = new_parent_id {
            let mut cursor = by_id.get(parent_id).copied();
            if cursor.is_none() {
                repomemo_domain::bail_not_found!("Folder was not found.");
            }
            while let Some(current) = cursor {
                if current.id == folder.id {
                    repomemo_domain::bail_invalid!("A folder cannot be moved into itself or into a folder inside it.");
                }
                above += 1;
                cursor = current.parent_id.as_deref().and_then(|id| by_id.get(id).copied());
            }
        }
        // Levels the moved folder spans, itself included.
        let mut span = 1;
        let mut level = vec![folder.id.as_str()];
        loop {
            let next = all
                .iter()
                .filter(|candidate| {
                    candidate
                        .parent_id
                        .as_deref()
                        .is_some_and(|parent| level.contains(&parent))
                })
                .map(|candidate| candidate.id.as_str())
                .collect::<Vec<_>>();
            if next.is_empty() {
                break;
            }
            span += 1;
            level = next;
        }
        if above + span > MAX_FOLDER_DEPTH {
            repomemo_domain::bail_invalid!("Folders can be nested up to {MAX_FOLDER_DEPTH} levels deep.");
        }
        if folder.parent_id.as_deref() == new_parent_id {
            return Ok(folder);
        }
        let duplicate: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT 1 FROM folders
            WHERE workspace_id = ?1 AND COALESCE(parent_id, '') = COALESCE(?2, '')
              AND name = ?3 COLLATE NOCASE AND id <> ?4
            "#,
        )
        .bind(&folder.workspace_id)
        .bind(new_parent_id)
        .bind(&folder.name)
        .bind(folder_id)
        .fetch_optional(self.reader())
        .await?;
        if duplicate.is_some() {
            repomemo_domain::bail_conflict!("A folder with this name already exists there.");
        }
        sqlx::query("UPDATE folders SET parent_id = ?2 WHERE id = ?1")
            .bind(folder_id)
            .bind(new_parent_id)
            .execute(&self.pool)
            .await?;
        self.get_folder(folder_id).await
    }

    /// Replaces the content of an artifact in place: it keeps its id, folder,
    /// comments, lifecycle and the memory cards that cite it, and is indexed
    /// again (passages with unchanged text keep their identity).
    pub async fn replace_artifact_content(
        &self,
        artifact_id: &str,
        content_hash: &str,
        size_bytes: i64,
        mime_type: Option<&str>,
    ) -> Result<ArtifactSummary> {
        let current = self.get_artifact_summary(artifact_id).await?;
        if current.content_hash == content_hash {
            repomemo_domain::bail_conflict!("This file already holds exactly that content.");
        }
        let now = Utc::now().to_rfc3339();
        let updated = sqlx::query(
            "UPDATE artifacts SET content_hash = ?2, size_bytes = ?3, mime_type = COALESCE(?4, mime_type), updated_at = ?5, indexed_at = NULL WHERE id = ?1",
        )
        .bind(artifact_id)
        .bind(content_hash)
        .bind(size_bytes)
        .bind(mime_type)
        .bind(&now)
        .execute(&self.pool)
        .await;
        match updated {
            Ok(_) => {}
            Err(error) => {
                let error: anyhow::Error = error.into();
                if is_unique_violation(&error) {
                    repomemo_domain::bail_conflict!("Another version of this file already holds that content.");
                }
                return Err(error);
            }
        }
        self.get_artifact_summary(artifact_id).await
    }

    /// Ids of a folder, every folder below it, and every artifact inside any of them.
    pub async fn folder_subtree(&self, folder_id: &str) -> Result<(Vec<String>, Vec<String>)> {
        let folder = self.get_folder(folder_id).await?;
        let all = self.list_folders(&folder.workspace_id).await?;
        let mut folder_ids = vec![folder.id.clone()];
        let mut index = 0;
        while index < folder_ids.len() {
            let parent = folder_ids[index].clone();
            folder_ids.extend(
                all.iter()
                    .filter(|candidate| candidate.parent_id.as_deref() == Some(parent.as_str()))
                    .map(|candidate| candidate.id.clone()),
            );
            index += 1;
        }
        let in_folders: HashSet<&str> = folder_ids.iter().map(String::as_str).collect();
        let rows = sqlx::query("SELECT id, folder_id FROM artifacts WHERE workspace_id = ?1 AND folder_id IS NOT NULL")
            .bind(&folder.workspace_id)
            .fetch_all(self.reader())
            .await?;
        let artifact_ids = rows
            .into_iter()
            .filter(|row| in_folders.contains(row.get::<String, _>("folder_id").as_str()))
            .map(|row| row.get("id"))
            .collect();
        Ok((folder_ids, artifact_ids))
    }

    pub async fn delete_folders(&self, folder_ids: &[String]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for id in folder_ids {
            sqlx::query("DELETE FROM folders WHERE id = ?1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Moves an artifact into a folder, or to the top level with `None`.
    pub async fn move_artifact_to_folder(&self, artifact_id: &str, folder_id: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE artifacts SET folder_id = ?2 WHERE id = ?1")
            .bind(artifact_id)
            .bind(folder_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Places an unfiled artifact in a folder. An artifact that already sits in
    /// a folder stays where it is: a duplicate upload resolves to the existing
    /// artifact and must not pull it out of its folder.
    pub async fn set_artifact_folder(&self, artifact_id: &str, folder_id: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE artifacts SET folder_id = ?2 WHERE id = ?1 AND folder_id IS NULL")
            .bind(artifact_id)
            .bind(folder_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Artifact id to folder id for every artifact in the workspace that sits
    /// inside a folder.
    pub async fn artifact_folder_ids(&self, workspace_id: &str) -> Result<HashMap<String, String>> {
        let rows = sqlx::query("SELECT id, folder_id FROM artifacts WHERE workspace_id = ?1 AND folder_id IS NOT NULL")
            .bind(workspace_id)
            .fetch_all(self.reader())
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.get("id"), row.get("folder_id")))
            .collect())
    }

    pub async fn record_index_failure(&self, artifact_id: &str, message: &str, attempts: i64) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO artifact_index_failures (artifact_id, message, attempts, failed_at)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(artifact_id) DO UPDATE SET
              message = excluded.message, attempts = excluded.attempts,
              failed_at = excluded.failed_at
            "#,
        )
        .bind(artifact_id)
        .bind(message)
        .bind(attempts)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// `artifact id -> (status, superseded by)` for files whose lifecycle was
    /// ever set. Files without an entry are active.
    pub async fn workspace_lifecycle_states(
        &self,
        workspace_id: &str,
    ) -> Result<HashMap<String, (String, Option<String>)>> {
        let rows = sqlx::query(
            "SELECT artifact_id, status, superseded_by_artifact_id FROM artifact_lifecycle WHERE workspace_id = ?1",
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.get("artifact_id"),
                    (row.get("status"), row.get("superseded_by_artifact_id")),
                )
            })
            .collect())
    }

    /// Files marked *needs review*, with the note left when they were, the
    /// longest-waiting first: `(artifact id, title, path, note, since)`.
    pub async fn artifacts_needing_review(
        &self,
        workspace_id: &str,
    ) -> Result<Vec<(String, String, String, String, String)>> {
        let rows = sqlx::query(
            r#"
            SELECT artifacts.id, artifacts.title, artifacts.path,
                   artifact_lifecycle.review_note, artifact_lifecycle.updated_at
            FROM artifact_lifecycle
            JOIN artifacts ON artifacts.id = artifact_lifecycle.artifact_id
            WHERE artifact_lifecycle.workspace_id = ?1 AND artifact_lifecycle.status = 'needs_review'
            ORDER BY artifact_lifecycle.updated_at ASC, artifacts.title ASC
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.get("id"),
                    row.get("title"),
                    row.get("path"),
                    row.get("review_note"),
                    row.get("updated_at"),
                )
            })
            .collect())
    }

    pub async fn clear_index_failure(&self, artifact_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM artifact_index_failures WHERE artifact_id = ?1")
            .bind(artifact_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_index_failures(&self, workspace_id: &str) -> Result<Vec<ArtifactIndexFailure>> {
        let rows = sqlx::query(
            r#"
            SELECT failures.artifact_id, failures.message, failures.attempts, failures.failed_at
            FROM artifact_index_failures failures
            JOIN artifacts ON artifacts.id = failures.artifact_id
            WHERE artifacts.workspace_id = ?1 AND artifacts.indexed_at IS NULL
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| ArtifactIndexFailure {
                artifact_id: row.get("artifact_id"),
                message: row.get("message"),
                attempts: row.get("attempts"),
                failed_at: row.get("failed_at"),
            })
            .collect())
    }

    pub(crate) async fn get_artifact_summary_by_identity(
        &self,
        workspace_id: &str,
        source_id: &str,
        path: &str,
        content_hash: &str,
    ) -> Result<ArtifactSummary> {
        let row = sqlx::query_as::<_, ArtifactSummaryRow>(
            r#"
            SELECT
              artifacts.id,
              artifacts.workspace_id,
              artifacts.source_id,
              sources.name AS source_name,
              artifacts.type AS artifact_type,
              artifacts.title,
              artifacts.path,
              artifacts.content_hash,
              artifacts.mime_type,
              artifacts.language,
              artifacts.size_bytes,
              artifacts.created_at,
              artifacts.updated_at,
              artifacts.indexed_at
            FROM artifacts
            JOIN sources ON sources.id = artifacts.source_id
            WHERE artifacts.workspace_id = ?1
              AND artifacts.source_id = ?2
              AND artifacts.path = ?3
              AND artifacts.content_hash = ?4
            LIMIT 1
            "#,
        )
        .bind(workspace_id)
        .bind(source_id)
        .bind(path)
        .bind(content_hash)
        .fetch_one(self.reader())
        .await?;

        Ok(ArtifactSummary::from(row))
    }

    pub async fn list_chunks_for_artifact(&self, artifact_id: &str) -> Result<Vec<Chunk>> {
        let rows = sqlx::query_as::<_, ChunkRow>(
            r#"
            SELECT
              id,
              artifact_id,
              workspace_id,
              chunk_index,
              text,
              token_count,
              start_line,
              end_line,
              heading_path,
              content_hash,
              embedding_status,
              metadata_json
            FROM chunks
            WHERE artifact_id = ?1
            ORDER BY chunk_index ASC
            "#,
        )
        .bind(artifact_id)
        .fetch_all(self.reader())
        .await?;

        Ok(rows.into_iter().map(Chunk::from).collect())
    }
}
