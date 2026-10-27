use super::*;

/// A passage the vector index named, with what a result needs.
#[derive(Debug, sqlx::FromRow)]
struct EmbeddingCandidateRow {
    chunk_id: String,
    artifact_id: String,
    title: String,
    path: String,
    artifact_type: String,
    language: Option<String>,
    text: String,
    start_line: Option<i64>,
    end_line: Option<i64>,
    source_name: String,
}

/// Adds one stored vector, scaled to length one, to its file's running sum.
fn add_unit_vector(sums: &mut HashMap<String, Vec<f32>>, row: &sqlx::sqlite::SqliteRow) {
    let Some(vector) = decode_embedding(&row.get::<Vec<u8>, _>("vector_blob")) else {
        return;
    };
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm == 0.0 {
        return;
    }
    let sum = sums
        .entry(row.get::<String, _>("artifact_id"))
        .or_insert_with(|| vec![0.0; vector.len()]);
    if sum.len() != vector.len() {
        return;
    }
    for (total, value) in sum.iter_mut().zip(&vector) {
        *total += value / norm;
    }
}

/// Scales each running sum to length one: the file's direction in meaning space.
fn normalise_sums(sums: &mut HashMap<String, Vec<f32>>) {
    for sum in sums.values_mut() {
        let norm = sum.iter().map(|value| value * value).sum::<f32>().sqrt();
        if norm > 0.0 {
            sum.iter_mut().for_each(|value| *value /= norm);
        }
    }
}

impl StorageEngine {
    pub async fn list_symbols(&self, artifact_id: &str) -> Result<Vec<Symbol>> {
        let rows = sqlx::query_as::<_, SymbolRow>(
            r#"
            SELECT id, artifact_id, workspace_id, kind, name, signature,
                   start_line, end_line, metadata_json
            FROM symbols
            WHERE artifact_id = ?1
            ORDER BY COALESCE(start_line, 2147483647), name
            "#,
        )
        .bind(artifact_id)
        .fetch_all(self.reader())
        .await?;

        Ok(rows.into_iter().map(Symbol::from).collect())
    }

    pub async fn search_symbols(&self, workspace_id: &str, query: &str, limit: i64) -> Result<Vec<SymbolSearchResult>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query(
            r#"
            SELECT symbols.id, symbols.artifact_id, symbols.workspace_id,
                   symbols.kind, symbols.name, symbols.signature,
                   symbols.start_line, symbols.end_line, symbols.metadata_json,
                   artifacts.title, artifacts.path, artifacts.language,
                   sources.name AS source_name
            FROM symbols
            JOIN artifacts ON artifacts.id = symbols.artifact_id
            JOIN sources ON sources.id = artifacts.source_id
            WHERE symbols.workspace_id = ?1
              AND instr(lower(symbols.name), lower(?2)) > 0
            ORDER BY
              CASE WHEN lower(symbols.name) = lower(?2) THEN 0 ELSE 1 END,
              length(symbols.name), symbols.name, artifacts.path
            LIMIT ?3
            "#,
        )
        .bind(workspace_id)
        .bind(query)
        .bind(limit.clamp(1, 100))
        .fetch_all(self.reader())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| SymbolSearchResult {
                symbol: Symbol::from(SymbolRow {
                    id: row.get("id"),
                    artifact_id: row.get("artifact_id"),
                    workspace_id: row.get("workspace_id"),
                    kind: row.get("kind"),
                    name: row.get("name"),
                    signature: row.get("signature"),
                    start_line: row.get("start_line"),
                    end_line: row.get("end_line"),
                    metadata_json: row.get("metadata_json"),
                }),
                title: row.get("title"),
                path: row.get("path"),
                language: row.get("language"),
                source_name: row.get("source_name"),
            })
            .collect())
    }

    pub async fn list_workspace_chunks(&self, workspace_id: &str) -> Result<Vec<Chunk>> {
        let rows = sqlx::query_as::<_, ChunkRow>(
            r#"SELECT id, artifact_id, workspace_id, chunk_index, text, token_count,
               start_line, end_line, heading_path, content_hash, embedding_status, metadata_json
               FROM chunks WHERE workspace_id = ?1 ORDER BY artifact_id, chunk_index"#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows.into_iter().map(Chunk::from).collect())
    }

    pub async fn upsert_embeddings(
        &self,
        workspace_id: &str,
        model: &str,
        embeddings: Vec<(String, Vec<f32>)>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;
        for (chunk_id, vector) in &embeddings {
            if vector.is_empty() {
                continue;
            }
            let dimensions = vector.len() as i64;
            let bytes = encode_embedding(vector);
            sqlx::query(
                r#"INSERT INTO chunk_embeddings (chunk_id, workspace_id, model, dimensions, vector_blob, created_at, updated_at)
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
                   ON CONFLICT(chunk_id) DO UPDATE SET model = excluded.model, dimensions = excluded.dimensions,
                     vector_blob = excluded.vector_blob, updated_at = excluded.updated_at"#,
            )
            .bind(chunk_id).bind(workspace_id).bind(model).bind(dimensions).bind(bytes).bind(&now)
            .execute(&mut *tx).await?;
            sqlx::query("UPDATE chunks SET embedding_status = 'ready' WHERE id = ?1")
                .bind(chunk_id)
                .execute(&mut *tx)
                .await?;
        }
        // Which files these passages belong to, so derived results redo only those.
        let mut touched = std::collections::HashSet::new();
        for batch in embeddings.chunks(400) {
            let mut query = QueryBuilder::<Sqlite>::new("SELECT DISTINCT artifact_id FROM chunks WHERE id IN (");
            let mut ids = query.separated(", ");
            for (chunk_id, _) in batch {
                ids.push_bind(chunk_id);
            }
            query.push(")");
            for (artifact_id,) in query.build_query_as::<(String,)>().fetch_all(&mut *tx).await? {
                touched.insert(artifact_id);
            }
        }
        tx.commit().await?;
        self.vector_changes
            .touch(workspace_id, touched.iter().map(String::as_str));
        // A search index that exists is brought along, not rebuilt.
        self.vectors.apply(workspace_id, model, &embeddings);
        Ok(())
    }

    /// Nearest chunks to `query` among those embedded with `model`, honouring
    /// the same type, language and source filters as keyword search. Vectors
    /// from another model are skipped: their dimensions and meaning differ.
    ///
    /// The nearest vectors come from the workspace's in-memory index; the
    /// database is asked only about those passages (and the filters). A
    /// workspace too large for the index is scanned in the database instead.
    pub async fn search_chunks_by_embedding(
        &self,
        request: &SearchRequest,
        model: &str,
        query: &[f32],
    ) -> Result<Vec<SearchResult>> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let folders = self.search_folder_scope(request).await?;
        let limit = request.limit.unwrap_or(12).clamp(1, 100) as usize;
        let mut results = match self.vector_index(&request.workspace_id, model).await? {
            Some(index) => self.search_vector_index(request, &folders, index, query, limit).await?,
            None => self.scan_chunks_by_embedding(request, &folders, model, query).await?,
        };
        results.truncate(limit);
        self.attach_commits(&mut results).await?;
        Ok(results)
    }

    /// Takes the nearest vectors from `index`, widening the net until enough
    /// of them pass the request's filters or the index is exhausted.
    pub(crate) async fn search_vector_index(
        &self,
        request: &SearchRequest,
        folders: &[String],
        index: Arc<RwLock<vectors::WorkspaceVectors>>,
        query: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let Some(unit) = vectors::normalised(query) else {
            return Ok(Vec::new());
        };
        let unit = Arc::new(unit);
        let mut wanted = (limit * 4).max(64);
        loop {
            let nearest = {
                let index = index.clone();
                let unit = unit.clone();
                tokio::task::spawn_blocking(move || {
                    index
                        .read()
                        .map(|index| index.nearest(&unit, wanted))
                        .map_err(|_| anyhow::anyhow!("the vector index is unavailable"))
                })
                .await??
            };
            let exhausted = nearest.len() < wanted;
            let rows = self.passages_of_vectors(request, folders, &nearest).await?;
            if rows.len() >= limit || exhausted {
                return Ok(rows);
            }
            wanted = wanted.saturating_mul(8);
        }
    }

    /// The passages among `nearest` that pass the request's filters, in the
    /// order of `nearest`, with their similarity as the score.
    async fn passages_of_vectors(
        &self,
        request: &SearchRequest,
        folders: &[String],
        nearest: &[(String, f32)],
    ) -> Result<Vec<SearchResult>> {
        let mut found: HashMap<String, EmbeddingCandidateRow> = HashMap::new();
        for batch in nearest.chunks(400) {
            let mut builder = QueryBuilder::<Sqlite>::new(
                r#"SELECT chunks.id AS chunk_id, chunks.artifact_id, artifacts.title, artifacts.path,
                     artifacts.type AS artifact_type, artifacts.language, chunks.text, chunks.start_line,
                     chunks.end_line, sources.name AS source_name
                   FROM chunks
                   JOIN artifacts ON artifacts.id = chunks.artifact_id
                   JOIN sources ON sources.id = artifacts.source_id
                   WHERE chunks.workspace_id = "#,
            );
            builder.push_bind(&request.workspace_id);
            builder.push(" AND chunks.id IN (");
            let mut separated = builder.separated(", ");
            for (chunk_id, _) in batch {
                separated.push_bind(chunk_id.clone());
            }
            separated.push_unseparated(")");
            push_search_filters(&mut builder, request, folders);
            for row in builder
                .build_query_as::<EmbeddingCandidateRow>()
                .fetch_all(self.reader())
                .await?
            {
                found.insert(row.chunk_id.clone(), row);
            }
        }
        Ok(nearest
            .iter()
            .filter_map(|(chunk_id, score)| {
                let row = found.remove(chunk_id)?;
                Some(SearchResult {
                    artifact_id: row.artifact_id,
                    chunk_id: row.chunk_id,
                    status: None,
                    title: row.title,
                    path: row.path,
                    artifact_type: artifact_type_from_db(&row.artifact_type),
                    language: row.language,
                    snippet: row.text.chars().take(360).collect(),
                    start_line: row.start_line,
                    end_line: row.end_line,
                    score: f64::from(*score),
                    source_name: row.source_name,
                    match_kind: "semantic".to_owned(),
                    commit: None,
                    last_author: None,
                    last_changed_at: None,
                })
            })
            .collect())
    }

    /// The slow path: decode every vector of the workspace and compare.
    pub(crate) async fn scan_chunks_by_embedding(
        &self,
        request: &SearchRequest,
        folders: &[String],
        model: &str,
        query: &[f32],
    ) -> Result<Vec<SearchResult>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"SELECT chunks.id AS chunk_id, chunks.artifact_id, artifacts.title, artifacts.path,
                 artifacts.type AS artifact_type, artifacts.language, chunks.text, chunks.start_line,
                 chunks.end_line, sources.name AS source_name, chunk_embeddings.vector_blob
               FROM chunk_embeddings JOIN chunks ON chunks.id = chunk_embeddings.chunk_id
               JOIN artifacts ON artifacts.id = chunks.artifact_id JOIN sources ON sources.id = artifacts.source_id
               WHERE chunk_embeddings.workspace_id = "#,
        );
        builder.push_bind(&request.workspace_id);
        builder.push(" AND chunk_embeddings.model = ");
        builder.push_bind(model);
        push_search_filters(&mut builder, request, folders);
        let rows = builder
            .build_query_as::<EmbeddingSearchRow>()
            .fetch_all(self.reader())
            .await?;
        let mut results = rows
            .into_iter()
            .filter_map(|row| {
                let vector = decode_embedding(&row.vector_blob)?;
                let score = cosine_similarity(query, &vector)?;
                Some(SearchResult {
                    artifact_id: row.artifact_id,
                    chunk_id: row.chunk_id,
                    status: None,
                    title: row.title,
                    path: row.path,
                    artifact_type: artifact_type_from_db(&row.artifact_type),
                    language: row.language,
                    snippet: row.text.chars().take(360).collect(),
                    start_line: row.start_line,
                    end_line: row.end_line,
                    score,
                    source_name: row.source_name,
                    match_kind: "semantic".to_owned(),
                    commit: None,
                    last_author: None,
                    last_changed_at: None,
                })
            })
            .collect::<Vec<_>>();
        results.sort_by(|left, right| right.score.total_cmp(&left.score));
        Ok(results)
    }

    /// Full text of the given chunks, keyed by chunk id. Search results carry
    /// short snippets; answers need the whole passage.
    pub async fn chunk_texts(&self, chunk_ids: &[String]) -> Result<HashMap<String, String>> {
        if chunk_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut builder = QueryBuilder::<Sqlite>::new("SELECT id, text FROM chunks WHERE id IN (");
        let mut separated = builder.separated(", ");
        for chunk_id in chunk_ids {
            separated.push_bind(chunk_id);
        }
        separated.push_unseparated(")");
        let rows = builder.build().fetch_all(self.reader()).await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.get::<String, _>("id"), row.get::<String, _>("text")))
            .collect())
    }

    /// The next chunks that have no embedding from `model`, either because
    /// they are new or because the workspace switched embedding models.
    pub async fn chunks_missing_embeddings(
        &self,
        workspace_id: &str,
        model: &str,
        limit: i64,
    ) -> Result<Vec<EmbeddingCandidate>> {
        let rows = sqlx::query_as::<_, EmbeddingCandidate>(
            r#"SELECT chunks.id AS chunk_id, artifacts.title, chunks.heading_path, chunks.text
               FROM chunks JOIN artifacts ON artifacts.id = chunks.artifact_id
               LEFT JOIN chunk_embeddings ON chunk_embeddings.chunk_id = chunks.id
               WHERE chunks.workspace_id = ?1
                 AND (chunk_embeddings.chunk_id IS NULL OR chunk_embeddings.model != ?2)
               ORDER BY chunks.artifact_id, chunks.chunk_index
               LIMIT ?3"#,
        )
        .bind(workspace_id)
        .bind(model)
        .bind(limit)
        .fetch_all(self.reader())
        .await?;
        Ok(rows)
    }

    /// Passage count per artifact, and how many of those have a vector from
    /// `model` (zero for every artifact when `model` is `None`).
    pub async fn chunk_counts_by_artifact(
        &self,
        workspace_id: &str,
        model: Option<&str>,
    ) -> Result<HashMap<String, (i64, i64)>> {
        let rows = sqlx::query(
            r#"SELECT chunks.artifact_id,
                 COUNT(*) AS passages,
                 COUNT(chunk_embeddings.chunk_id) AS embedded
               FROM chunks
               LEFT JOIN chunk_embeddings
                 ON chunk_embeddings.chunk_id = chunks.id AND chunk_embeddings.model = ?2
               WHERE chunks.workspace_id = ?1
               GROUP BY chunks.artifact_id"#,
        )
        .bind(workspace_id)
        .bind(model.unwrap_or(""))
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.get::<String, _>("artifact_id"),
                    (row.get::<i64, _>("passages"), row.get::<i64, _>("embedded")),
                )
            })
            .collect())
    }

    /// `chunk_counts_by_artifact` for the named files only.
    pub async fn chunk_counts_for_artifacts(
        &self,
        model: Option<&str>,
        artifact_ids: &[String],
    ) -> Result<HashMap<String, (i64, i64)>> {
        let mut counts = HashMap::new();
        for batch in artifact_ids.chunks(400) {
            let mut query = QueryBuilder::<Sqlite>::new(
                "SELECT chunks.artifact_id, COUNT(*) AS passages, COUNT(chunk_embeddings.chunk_id) AS embedded \
                 FROM chunks LEFT JOIN chunk_embeddings \
                   ON chunk_embeddings.chunk_id = chunks.id AND chunk_embeddings.model = ",
            );
            query.push_bind(model.unwrap_or(""));
            // Driven by the files' own passages (an index on `artifact_id`), not by the workspace.
            query.push(" WHERE chunks.artifact_id IN (");
            let mut ids = query.separated(", ");
            for artifact_id in batch {
                ids.push_bind(artifact_id);
            }
            query.push(") GROUP BY chunks.artifact_id");
            for row in query.build().fetch_all(self.reader()).await? {
                counts.insert(
                    row.get::<String, _>("artifact_id"),
                    (row.get::<i64, _>("passages"), row.get::<i64, _>("embedded")),
                );
            }
        }
        Ok(counts)
    }

    /// `artifact_embedding_centroids` for the named files only.
    pub async fn artifact_embedding_centroids_for(
        &self,
        model: &str,
        artifact_ids: &[String],
    ) -> Result<HashMap<String, Vec<f32>>> {
        let mut sums: HashMap<String, Vec<f32>> = HashMap::new();
        for batch in artifact_ids.chunks(200) {
            // Driven by the files' own passages, then one lookup of each vector by its id.
            let mut query = QueryBuilder::<Sqlite>::new(
                "SELECT chunks.artifact_id, chunk_embeddings.vector_blob \
                 FROM chunks JOIN chunk_embeddings ON chunk_embeddings.chunk_id = chunks.id \
                 WHERE chunk_embeddings.model = ",
            );
            query.push_bind(model);
            query.push(" AND chunks.artifact_id IN (");
            let mut ids = query.separated(", ");
            for artifact_id in batch {
                ids.push_bind(artifact_id);
            }
            query.push(")");
            for row in query.build().fetch_all(self.reader()).await? {
                add_unit_vector(&mut sums, &row);
            }
        }
        normalise_sums(&mut sums);
        Ok(sums)
    }

    /// The average direction of each artifact's passage vectors from `model`
    /// (unit-normalised), which places a whole file in meaning space.
    pub async fn artifact_embedding_centroids(
        &self,
        workspace_id: &str,
        model: &str,
    ) -> Result<HashMap<String, Vec<f32>>> {
        let rows = sqlx::query(
            r#"SELECT chunks.artifact_id, chunk_embeddings.vector_blob
               FROM chunk_embeddings JOIN chunks ON chunks.id = chunk_embeddings.chunk_id
               WHERE chunk_embeddings.workspace_id = ?1 AND chunk_embeddings.model = ?2"#,
        )
        .bind(workspace_id)
        .bind(model)
        .fetch_all(self.reader())
        .await?;
        let mut sums: HashMap<String, Vec<f32>> = HashMap::new();
        for row in &rows {
            add_unit_vector(&mut sums, row);
        }
        normalise_sums(&mut sums);
        Ok(sums)
    }

    /// `(card id, card title, artifact id)` for every file a memory card
    /// cites, directly or through one of the file's passages.
    pub async fn memory_card_artifact_links(&self, workspace_id: &str) -> Result<Vec<(String, String, String)>> {
        let rows = sqlx::query(
            r#"SELECT DISTINCT memory_cards.id AS card_id, memory_cards.title,
                 COALESCE(chunks.artifact_id, links.to_id) AS artifact_id
               FROM links
               JOIN memory_cards ON memory_cards.id = links.from_id
               LEFT JOIN chunks ON links.to_type = 'chunk' AND chunks.id = links.to_id
               WHERE links.workspace_id = ?1 AND links.from_type = 'memory_card'
                 AND (links.to_type = 'artifact' OR chunks.id IS NOT NULL)"#,
        )
        .bind(workspace_id)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.get("card_id"), row.get("title"), row.get("artifact_id")))
            .collect())
    }

    /// `(embedded with model, all chunks)` for a workspace.
    pub async fn embedding_coverage(&self, workspace_id: &str, model: &str) -> Result<(i64, i64)> {
        let row = sqlx::query(
            r#"SELECT
                 (SELECT COUNT(*) FROM chunk_embeddings WHERE workspace_id = ?1 AND model = ?2) AS embedded,
                 (SELECT COUNT(*) FROM chunks WHERE workspace_id = ?1) AS total"#,
        )
        .bind(workspace_id)
        .bind(model)
        .fetch_one(self.reader())
        .await?;
        Ok((row.get("embedded"), row.get("total")))
    }

    /// Forgets every stored vector of a workspace, whatever model made it, so
    /// the next embedding pass makes them all again. Returns how many were
    /// removed. Keyword search is unaffected; meaning search has nothing to
    /// look at until the pass has run.
    pub async fn clear_workspace_embeddings(&self, workspace_id: &str) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        let removed = sqlx::query("DELETE FROM chunk_embeddings WHERE workspace_id = ?1")
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        sqlx::query("UPDATE chunks SET embedding_status = 'not_configured' WHERE workspace_id = ?1 AND embedding_status != 'not_configured'")
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.vectors.forget(workspace_id);
        self.vector_changes.touch_everything(workspace_id);
        Ok(removed)
    }

    pub async fn embedding_count(&self, workspace_id: &str) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) AS count FROM chunk_embeddings WHERE workspace_id = ?1")
            .bind(workspace_id)
            .fetch_one(self.reader())
            .await?;
        Ok(row.get("count"))
    }

    /// `(artifact id, symbol name)` for every indexed symbol in a workspace.
    pub async fn workspace_symbol_names(&self, workspace_id: &str) -> Result<Vec<(String, String)>> {
        let rows = sqlx::query("SELECT DISTINCT artifact_id, name FROM symbols WHERE workspace_id = ?1")
            .bind(workspace_id)
            .fetch_all(self.reader())
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.get("artifact_id"), row.get("name")))
            .collect())
    }

    /// Passages matching a full-text query, with their whole text so the
    /// caller can confirm an exact mention.
    pub async fn chunks_matching(&self, workspace_id: &str, fts_query: &str, limit: i64) -> Result<Vec<ChunkMention>> {
        let rows = sqlx::query(
            r#"
            SELECT chunks.id, chunks.artifact_id, chunks.text, chunks.start_line, chunks.end_line
            FROM chunks_fts
            JOIN chunks ON chunks.rowid = chunks_fts.rowid
            WHERE chunks_fts MATCH ?1 AND chunks.workspace_id = ?2
            ORDER BY chunks.artifact_id, chunks.chunk_index
            LIMIT ?3
            "#,
        )
        .bind(fts_query)
        .bind(workspace_id)
        .bind(limit)
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| ChunkMention {
                chunk_id: row.get("id"),
                artifact_id: row.get("artifact_id"),
                text: row.get("text"),
                start_line: row.get("start_line"),
                end_line: row.get("end_line"),
            })
            .collect())
    }

    /// The folder a search is limited to and every folder below it.
    async fn search_folder_scope(&self, request: &SearchRequest) -> Result<Vec<String>> {
        match request.folder_id.as_deref().filter(|id| !id.is_empty()) {
            Some(folder_id) => Ok(self.folder_subtree(folder_id).await?.0),
            None => Ok(Vec::new()),
        }
    }

    pub async fn search_chunks(&self, request: &SearchRequest, fts_query: &str) -> Result<Vec<SearchResult>> {
        let folders = self.search_folder_scope(request).await?;
        let mut query = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
              artifacts.id AS artifact_id,
              chunks.id AS chunk_id,
              artifacts.title,
              artifacts.path,
              artifacts.type AS artifact_type,
              artifacts.language,
              snippet(chunks_fts, 0, '<mark>', '</mark>', ' ... ', 24) AS snippet,
              chunks.start_line,
              chunks.end_line,
              -bm25(chunks_fts) AS score,
              sources.name AS source_name
            FROM chunks_fts
            JOIN chunks ON chunks.rowid = chunks_fts.rowid
            JOIN artifacts ON artifacts.id = chunks.artifact_id
            JOIN sources ON sources.id = artifacts.source_id
            WHERE chunks_fts MATCH
            "#,
        );
        query.push_bind(fts_query);
        query.push(" AND chunks.workspace_id = ");
        query.push_bind(&request.workspace_id);
        push_search_filters(&mut query, request, &folders);

        query.push(" ORDER BY bm25(chunks_fts) ASC, artifacts.path ASC LIMIT ");
        query.push_bind(request.limit.unwrap_or(40).clamp(1, 100));

        let rows = query
            .build()
            .map(|row: SqliteRow| SearchResultRow {
                artifact_id: row.get("artifact_id"),
                chunk_id: row.get("chunk_id"),
                title: row.get("title"),
                path: row.get("path"),
                artifact_type: row.get("artifact_type"),
                language: row.get("language"),
                snippet: row.get("snippet"),
                start_line: row.get("start_line"),
                end_line: row.get("end_line"),
                score: row.get("score"),
                source_name: row.get("source_name"),
            })
            .fetch_all(self.reader())
            .await?;

        let mut results = rows.into_iter().map(SearchResult::from).collect::<Vec<_>>();
        self.attach_commits(&mut results).await?;
        Ok(results)
    }

    /// Fills in the commit of results that come from a repository file, so a
    /// reader knows which version of the code a passage is from.
    async fn attach_commits(&self, results: &mut [SearchResult]) -> Result<()> {
        if results.is_empty() {
            return Ok(());
        }
        let mut lifecycle_query =
            QueryBuilder::<Sqlite>::new("SELECT artifact_id, status FROM artifact_lifecycle WHERE artifact_id IN (");
        let mut ids = lifecycle_query.separated(", ");
        for result in results.iter() {
            ids.push_bind(result.artifact_id.clone());
        }
        ids.push_unseparated(")");
        let statuses = lifecycle_query
            .build()
            .map(|row: SqliteRow| (row.get::<String, _>("artifact_id"), row.get::<String, _>("status")))
            .fetch_all(self.reader())
            .await?
            .into_iter()
            .collect::<HashMap<_, _>>();
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT artifact_id, commit_sha, last_author, last_authored_at FROM repo_files WHERE removed_at IS NULL AND artifact_id IN ("
        );
        let mut separated = query.separated(", ");
        for result in results.iter() {
            separated.push_bind(result.artifact_id.clone());
        }
        separated.push_unseparated(")");
        let commits = query
            .build()
            .map(|row: SqliteRow| {
                (
                    row.get::<String, _>("artifact_id"),
                    (
                        row.get::<String, _>("commit_sha"),
                        row.get::<Option<String>, _>("last_author"),
                        row.get::<Option<String>, _>("last_authored_at"),
                    ),
                )
            })
            .fetch_all(self.reader())
            .await?
            .into_iter()
            .collect::<HashMap<_, _>>();
        for result in results.iter_mut() {
            result.status = Some(
                statuses
                    .get(&result.artifact_id)
                    .cloned()
                    .unwrap_or_else(|| "active".to_owned()),
            );
            if let Some((commit, author, at)) = commits.get(&result.artifact_id) {
                result.commit = Some(commit.clone());
                result.last_author = author.clone();
                result.last_changed_at = at.clone();
            }
        }
        Ok(())
    }
}
