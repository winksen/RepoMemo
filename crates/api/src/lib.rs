use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use repomemo_ai::{
    provider_from_settings, validate_settings, AiProvider, GenerateRequest, ImageAnalysisRequest,
};
use repomemo_domain::{
    AppSettings, ArtifactDetail, ArtifactSummary, ArtifactType, AskAnswer, AskRequest,
    CreateMemoryCardRequest, ImportReport, ImportRequest, IndexingJobStatus, MemoryCard,
    MemoryCardDetail, MemoryCardSummary, ProviderSettings, ProviderTestResult, SearchRequest,
    SearchResult, SourceType, SummaryResult, Symbol, SymbolSearchResult, UpdateMemoryCardRequest,
    Workspace, WorkspaceOverview,
};
use repomemo_indexer::{index_artifact, index_image_description};
use repomemo_ingestion::{
    detect_artifact_type, detect_language, discover_import_candidates, ImportCandidate,
    ImportOptions,
};
use repomemo_retrieval::RetrievalService;
use repomemo_storage::{NewArtifact, StorageConfig, StorageEngine};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct RepoMemoCore {
    storage: StorageEngine,
    retrieval: RetrievalService,
}

impl RepoMemoCore {
    pub async fn boot(data_dir: PathBuf) -> Result<Self> {
        let storage = StorageEngine::open(StorageConfig { data_dir }).await?;
        let retrieval = RetrievalService::new(storage.clone());
        Ok(Self { storage, retrieval })
    }

    pub async fn create_workspace(&self, name: String) -> Result<Workspace> {
        self.storage.create_workspace(&name).await
    }

    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        self.storage.list_workspaces().await
    }

    pub async fn update_workspace_name(
        &self,
        workspace_id: String,
        name: String,
    ) -> Result<Workspace> {
        self.storage
            .update_workspace_name(&workspace_id, &name)
            .await
    }

    pub async fn delete_workspace(&self, workspace_id: String) -> Result<()> {
        self.storage.delete_workspace(&workspace_id).await
    }

    pub async fn import_paths(&self, request: ImportRequest) -> Result<ImportReport> {
        if request.workspace_id.trim().is_empty() {
            bail!("Workspace id is required.");
        }

        if request.paths.is_empty() {
            bail!("Choose at least one file or folder to import.");
        }

        if !self.storage.workspace_exists(&request.workspace_id).await? {
            bail!("Workspace was not found.");
        }

        let paths = request.paths.iter().map(PathBuf::from).collect::<Vec<_>>();
        let discovery = discover_import_candidates(&paths, &ImportOptions::default())?;
        let mut report = ImportReport {
            workspace_id: request.workspace_id.clone(),
            scanned: discovery.scanned,
            imported: 0,
            duplicates: 0,
            skipped: discovery.skipped_items.len(),
            failed: 0,
            imported_artifacts: Vec::new(),
            skipped_items: discovery.skipped_items,
        };

        for candidate in discovery.candidates {
            match self
                .import_candidate(&request.workspace_id, candidate)
                .await
            {
                Ok(stored) if stored.created => {
                    report.imported += 1;
                    report.imported_artifacts.push(stored.artifact);
                }
                Ok(stored) => {
                    report.duplicates += 1;
                    report.imported_artifacts.push(stored.artifact);
                }
                Err(error) => {
                    report.failed += 1;
                    report
                        .skipped_items
                        .push(repomemo_domain::ImportSkippedItem {
                            path: error.path,
                            reason: error.reason,
                        });
                }
            }
        }

        report.skipped = report.skipped_items.len();

        Ok(report)
    }

    pub async fn import_text(
        &self,
        workspace_id: String,
        title: String,
        content: String,
        language: Option<String>,
    ) -> Result<ArtifactSummary> {
        if workspace_id.trim().is_empty() {
            bail!("Workspace id is required.");
        }
        if content.trim().is_empty() {
            bail!("Pasted content cannot be empty.");
        }
        if !self.storage.workspace_exists(&workspace_id).await? {
            bail!("Workspace was not found.");
        }

        let language = language
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);

        let (artifact_type, mime_type, extension) = match language.as_deref() {
            Some("Markdown") => (ArtifactType::MarkdownDoc, "text/markdown", "md"),
            Some("Text") | None => (ArtifactType::File, "text/plain", "txt"),
            Some(_) => (ArtifactType::CodeFile, "text/plain", "txt"),
        };

        let safe_title = title.trim();
        let safe_title = if safe_title.is_empty() {
            format!(
                "Pasted note {}",
                chrono_like_now_iso().split('T').next().unwrap_or("note")
            )
        } else {
            safe_title.to_owned()
        };
        let filename = sanitize_filename(&safe_title, extension);

        let source = self
            .storage
            .create_or_get_source(&workspace_id, SourceType::Manual, "Pasted notes", None)
            .await
            .with_context(|| "failed to create pasted-notes source")?;

        let bytes = content.into_bytes();
        let content_hash = StorageEngine::content_hash(&bytes);
        let size_bytes = bytes.len() as i64;

        self.storage
            .store_blob(&content_hash, &bytes, Some(mime_type))
            .await?;

        self.storage
            .store_artifact(NewArtifact {
                workspace_id: workspace_id.clone(),
                source_id: source.id,
                artifact_type,
                title: safe_title,
                path: filename,
                content_hash,
                mime_type: Some(mime_type.to_owned()),
                language,
                size_bytes,
                metadata: json!({ "origin": "paste" }),
            })
            .await
            .map(|stored| stored.artifact)
    }

    pub async fn import_upload(
        &self,
        workspace_id: String,
        filename: String,
        bytes: Vec<u8>,
        mime_type: Option<String>,
    ) -> Result<ArtifactSummary> {
        if workspace_id.trim().is_empty() {
            bail!("Workspace id is required.");
        }
        if bytes.is_empty() {
            bail!("Uploaded files cannot be empty.");
        }
        if !self.storage.workspace_exists(&workspace_id).await? {
            bail!("Workspace was not found.");
        }

        let path = Path::new(&filename);
        let safe_filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("A valid upload filename is required."))?
            .to_owned();
        let artifact_type = detect_artifact_type(path)
            .ok_or_else(|| anyhow::anyhow!("This file type is not supported for shared upload."))?;
        let language = detect_language(path);
        let fallback_mime = if matches!(artifact_type, ArtifactType::Image) {
            "application/octet-stream"
        } else if matches!(artifact_type, ArtifactType::MarkdownDoc) {
            "text/markdown"
        } else {
            "text/plain"
        };
        let mime_type = mime_type
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| fallback_mime.to_owned());
        let source = self
            .storage
            .create_or_get_source(&workspace_id, SourceType::Upload, "Shared uploads", None)
            .await
            .with_context(|| "failed to create shared-uploads source")?;
        let content_hash = StorageEngine::content_hash(&bytes);
        let size_bytes = bytes.len() as i64;

        self.storage
            .store_blob(&content_hash, &bytes, Some(&mime_type))
            .await?;
        self.storage
            .store_artifact(NewArtifact {
                workspace_id,
                source_id: source.id,
                artifact_type,
                title: safe_filename.clone(),
                path: safe_filename,
                content_hash,
                mime_type: Some(mime_type),
                language,
                size_bytes,
                metadata: json!({ "origin": "shared_upload" }),
            })
            .await
            .map(|stored| stored.artifact)
    }

    pub async fn list_artifacts(&self, workspace_id: String) -> Result<Vec<ArtifactSummary>> {
        self.storage.list_artifacts(&workspace_id).await
    }

    pub async fn get_artifact(&self, artifact_id: String) -> Result<ArtifactDetail> {
        self.storage.get_artifact(&artifact_id).await
    }

    pub async fn update_artifact_title(
        &self,
        artifact_id: String,
        title: String,
    ) -> Result<ArtifactSummary> {
        self.storage
            .update_artifact_title(&artifact_id, &title)
            .await
    }

    pub async fn delete_artifact(&self, artifact_id: String) -> Result<()> {
        self.storage.delete_artifact(&artifact_id).await
    }

    pub async fn index_artifact(&self, artifact_id: String) -> Result<IndexingJobStatus> {
        if artifact_id.trim().is_empty() {
            bail!("Artifact id is required.");
        }

        let summary = self.storage.get_artifact_summary(&artifact_id).await?;
        let stage = if matches!(summary.artifact_type, ArtifactType::Image) {
            "analyzing_image"
        } else {
            "extracting_text"
        };
        let job = self
            .storage
            .create_indexing_job(
                &summary.workspace_id,
                Some(&summary.source_id),
                stage,
                Some(1),
            )
            .await?;

        match self.index_artifact_inner(&summary).await {
            Ok(result) => {
                self.storage
                    .update_indexing_job(&job.id, "completed", &result.stage, 1, None)
                    .await
            }
            Err(error) => {
                let _ = self
                    .storage
                    .update_indexing_job(&job.id, "failed", "failed", 0, Some(&error.to_string()))
                    .await;
                Err(error)
            }
        }
    }

    pub async fn index_workspace(&self, workspace_id: String) -> Result<IndexingJobStatus> {
        if workspace_id.trim().is_empty() {
            bail!("Workspace id is required.");
        }

        if !self.storage.workspace_exists(&workspace_id).await? {
            bail!("Workspace was not found.");
        }

        let artifacts = self.storage.list_artifacts(&workspace_id).await?;
        let total = artifacts.len() as i64;
        let job = self
            .storage
            .create_indexing_job(&workspace_id, None, "chunking_workspace", Some(total))
            .await?;

        let mut indexed = 0_i64;
        for artifact in artifacts {
            if let Err(error) = self.index_artifact_inner(&artifact).await {
                let _ = self
                    .storage
                    .update_indexing_job(
                        &job.id,
                        "failed",
                        "failed",
                        indexed,
                        Some(&format!("{}: {error}", artifact.path)),
                    )
                    .await;
                return Err(error);
            }

            indexed += 1;
            let _ = self
                .storage
                .update_indexing_job(&job.id, "running", "chunking_workspace", indexed, None)
                .await?;
        }

        self.storage
            .update_indexing_job(&job.id, "completed", "chunked_workspace", indexed, None)
            .await
    }

    pub async fn workspace_overview(&self, workspace_id: String) -> Result<WorkspaceOverview> {
        self.storage.workspace_overview(&workspace_id).await
    }

    pub async fn create_memory_card(&self, request: CreateMemoryCardRequest) -> Result<MemoryCard> {
        validate_memory_card_fields(&request.title, &request.body_markdown, &request.source)?;
        if !self.storage.workspace_exists(&request.workspace_id).await? {
            bail!("Workspace was not found.");
        }
        self.storage
            .create_memory_card(
                &request.workspace_id,
                &request.title,
                &request.body_markdown,
                &request.source,
                request.confidence,
                &request.citations,
            )
            .await
    }

    pub async fn update_memory_card(&self, request: UpdateMemoryCardRequest) -> Result<MemoryCard> {
        validate_memory_card_fields(&request.title, &request.body_markdown, &request.source)?;
        self.storage
            .update_memory_card(
                &request.card_id,
                &request.title,
                &request.body_markdown,
                &request.source,
                request.confidence,
            )
            .await
    }

    pub async fn delete_memory_card(&self, card_id: String) -> Result<()> {
        if card_id.trim().is_empty() {
            bail!("Memory card id is required.");
        }
        self.storage.delete_memory_card(&card_id).await
    }

    pub async fn list_memory_cards(&self, workspace_id: String) -> Result<Vec<MemoryCardSummary>> {
        if workspace_id.trim().is_empty() {
            bail!("Workspace id is required.");
        }
        self.storage.list_memory_cards(&workspace_id).await
    }

    pub async fn search_memory_cards(
        &self,
        workspace_id: String,
        query: String,
    ) -> Result<Vec<MemoryCardSummary>> {
        if workspace_id.trim().is_empty() {
            bail!("Workspace id is required.");
        }
        self.storage
            .search_memory_cards(&workspace_id, &query)
            .await
    }

    pub async fn get_memory_card(&self, card_id: String) -> Result<MemoryCardDetail> {
        if card_id.trim().is_empty() {
            bail!("Memory card id is required.");
        }
        self.storage.get_memory_card(&card_id).await
    }

    pub async fn export_memory_card(&self, card_id: String) -> Result<String> {
        let detail = self.get_memory_card(card_id).await?;
        let mut markdown = format!(
            "# {}\n\n{}\n\n## Record\n\n- Source: {}\n- Updated: {}\n\n## Evidence\n",
            detail.card.title,
            detail.card.body_markdown.trim(),
            detail.card.source,
            detail.card.updated_at,
        );
        if detail.evidence.is_empty() {
            markdown.push_str("\n_No linked evidence._\n");
        } else {
            for evidence in detail.evidence {
                if evidence.exists {
                    let location = match (evidence.start_line, evidence.end_line) {
                        (Some(start), Some(end)) if start != end => format!(" lines {start}-{end}"),
                        (Some(line), _) => format!(" line {line}"),
                        _ => String::new(),
                    };
                    markdown.push_str(&format!(
                        "\n- [{}]({}){}\n",
                        evidence
                            .title
                            .unwrap_or_else(|| "Untitled evidence".to_owned()),
                        evidence.path.unwrap_or_else(|| evidence.target_id.clone()),
                        location,
                    ));
                } else {
                    markdown.push_str(&format!("\n- Missing evidence ({})\n", evidence.target_id));
                }
            }
        }
        Ok(markdown)
    }

    pub async fn search_workspace(&self, request: SearchRequest) -> Result<Vec<SearchResult>> {
        self.retrieval.search(request).await
    }

    pub async fn list_symbols(&self, artifact_id: String) -> Result<Vec<Symbol>> {
        if artifact_id.trim().is_empty() {
            bail!("Artifact id is required.");
        }
        self.storage.list_symbols(&artifact_id).await
    }

    pub async fn search_symbols(
        &self,
        workspace_id: String,
        query: String,
    ) -> Result<Vec<SymbolSearchResult>> {
        if workspace_id.trim().is_empty() {
            bail!("Workspace id is required.");
        }
        self.storage.search_symbols(&workspace_id, &query, 30).await
    }

    pub async fn list_provider_settings(
        &self,
        workspace_id: String,
    ) -> Result<Vec<ProviderSettings>> {
        if workspace_id.trim().is_empty() {
            bail!("Workspace id is required.");
        }
        self.storage.list_provider_settings(&workspace_id).await
    }

    pub async fn save_provider_settings(
        &self,
        settings: ProviderSettings,
    ) -> Result<ProviderSettings> {
        let workspace_id = settings
            .workspace_id
            .as_deref()
            .context("Provider settings must belong to a workspace.")?;
        if !self.storage.workspace_exists(workspace_id).await? {
            bail!("Workspace was not found.");
        }
        validate_settings(&settings)?;
        self.storage.save_provider_settings(settings).await
    }

    pub async fn test_provider(&self, provider_id: String) -> Result<ProviderTestResult> {
        let settings = self.storage.get_provider_settings(&provider_id).await?;
        let provider = provider_from_settings(settings)?;
        provider.test_connection().await
    }

    pub async fn summarize_artifact(
        &self,
        artifact_id: String,
        provider_id: String,
    ) -> Result<SummaryResult> {
        let detail = self.storage.get_artifact(&artifact_id).await?;
        if detail.chunks.is_empty() {
            bail!(
                "Index this artifact before requesting a summary so RepoMemo can cite its content."
            );
        }
        let settings = self.storage.get_provider_settings(&provider_id).await?;
        if !settings.enabled {
            bail!("Enable this local provider before using AI. No content was sent.");
        }
        if settings.workspace_id.as_deref() != Some(detail.summary.workspace_id.as_str()) {
            bail!("The selected provider belongs to a different workspace.");
        }
        let provider = provider_from_settings(settings)?;
        let selected_chunks = detail.chunks.iter().take(8).collect::<Vec<_>>();
        let context = selected_chunks
            .iter()
            .scan(0_usize, |total, chunk| {
                if *total >= 12_000 {
                    return None;
                }
                let text = chunk.text.chars().take(2_000).collect::<String>();
                *total += text.len();
                Some(format!(
                    "[{}] {}\n{}",
                    chunk.id,
                    chunk.heading_path.as_deref().unwrap_or("Artifact content"),
                    text
                ))
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let summary_markdown = provider
            .generate(GenerateRequest {
                prompt: format!("Summarize the artifact '{}'. Be concise, factual, and use only the supplied local context. Do not claim to have read content outside this context.", detail.summary.title),
                context,
                options: json!({ "temperature": 0.2 }),
            })
            .await?;
        let citations = selected_chunks
            .into_iter()
            .map(|chunk| repomemo_domain::Citation {
                artifact_id: detail.summary.id.clone(),
                chunk_id: Some(chunk.id.clone()),
                title: detail.summary.title.clone(),
                path: detail.summary.path.clone(),
                start_line: chunk.start_line,
                end_line: chunk.end_line,
                confidence: None,
            })
            .collect();
        Ok(SummaryResult {
            summary_markdown,
            citations,
            warnings: vec!["Generated by the configured local provider. Verify cited source text before relying on the summary.".to_owned()],
        })
    }

    pub async fn summarize_workspace(
        &self,
        workspace_id: String,
        provider_id: String,
    ) -> Result<SummaryResult> {
        let settings = self.storage.get_provider_settings(&provider_id).await?;
        if !settings.enabled {
            bail!("Enable this provider before using AI. No content was sent.");
        }
        if settings.workspace_id.as_deref() != Some(workspace_id.as_str()) {
            bail!("The selected provider belongs to a different workspace.");
        }
        let artifacts = self.storage.list_artifacts(&workspace_id).await?;
        let mut context_sections = Vec::new();
        let mut citations = Vec::new();
        let mut used_chars = 0_usize;
        for artifact in artifacts {
            if used_chars >= 18_000 || context_sections.len() >= 30 {
                break;
            }
            let detail = self.storage.get_artifact(&artifact.id).await?;
            for chunk in detail.chunks.into_iter().take(2) {
                if used_chars >= 18_000 || context_sections.len() >= 30 {
                    break;
                }
                let text = chunk.text.chars().take(1_200).collect::<String>();
                used_chars += text.len();
                context_sections.push(format!(
                    "[{}] {} ({})\n{}",
                    chunk.id, detail.summary.title, detail.summary.path, text
                ));
                citations.push(repomemo_domain::Citation {
                    artifact_id: detail.summary.id.clone(),
                    chunk_id: Some(chunk.id),
                    title: detail.summary.title.clone(),
                    path: detail.summary.path.clone(),
                    start_line: chunk.start_line,
                    end_line: chunk.end_line,
                    confidence: None,
                });
            }
        }
        if context_sections.is_empty() {
            bail!("Index at least one artifact before requesting a workspace summary so RepoMemo can cite its content.");
        }
        let provider = provider_from_settings(settings)?;
        let summary_markdown = provider.generate(GenerateRequest {
            prompt: "Summarize this workspace for an engineer joining the project. Cover the major components, important behavior, and any notable gaps. Use only the supplied local context.".to_owned(),
            context: context_sections.join("\n\n"),
            options: json!({ "temperature": 0.2 }),
        }).await?;
        Ok(SummaryResult { summary_markdown, citations, warnings: vec!["This overview is based on the cited indexed excerpts, not necessarily every file in the workspace.".to_owned()] })
    }

    pub async fn app_settings(&self) -> Result<AppSettings> {
        let (ai_enabled, active_provider) = self.storage.app_ai_status().await?;
        Ok(AppSettings {
            data_dir: self.storage.data_dir().display().to_string(),
            ai_enabled,
            active_provider,
        })
    }

    pub async fn embed_workspace(
        &self,
        workspace_id: String,
        provider_id: String,
    ) -> Result<IndexingJobStatus> {
        let settings = self.storage.get_provider_settings(&provider_id).await?;
        if !settings.enabled {
            bail!("Enable this provider before building embeddings.");
        }
        if settings.workspace_id.as_deref() != Some(workspace_id.as_str()) {
            bail!("The selected provider belongs to a different workspace.");
        }
        let chunks = self.storage.list_workspace_chunks(&workspace_id).await?;
        if chunks.is_empty() {
            bail!("Index artifacts before building embeddings.");
        }
        let job = self
            .storage
            .create_indexing_job(
                &workspace_id,
                None,
                "embedding_workspace",
                Some(chunks.len() as i64),
            )
            .await?;
        let provider = provider_from_settings(settings.clone())?;
        let mut completed = 0_i64;
        for batch in chunks.chunks(16) {
            let vectors = provider
                .embed(
                    batch.iter().map(|chunk| chunk.text.clone()).collect(),
                    json!({}),
                )
                .await?;
            if vectors.len() != batch.len() {
                bail!("Provider returned an unexpected embedding count.");
            }
            self.storage
                .upsert_embeddings(
                    &workspace_id,
                    settings
                        .embedding_model
                        .as_deref()
                        .or(settings.model.as_deref())
                        .unwrap_or("unknown"),
                    batch
                        .iter()
                        .zip(vectors)
                        .map(|(chunk, vector)| (chunk.id.clone(), vector))
                        .collect(),
                )
                .await?;
            completed += batch.len() as i64;
            self.storage
                .update_indexing_job(&job.id, "running", "embedding_workspace", completed, None)
                .await?;
        }
        self.storage
            .update_indexing_job(&job.id, "completed", "embedded_workspace", completed, None)
            .await
    }

    pub async fn ask_workspace(&self, request: AskRequest) -> Result<AskAnswer> {
        if request.workspace_id.trim().is_empty() || request.question.trim().is_empty() {
            bail!("A workspace and question are required.");
        }
        let provider_id = request
            .provider_id
            .as_deref()
            .context("Enable a provider before using Ask.")?;
        let settings = self.storage.get_provider_settings(provider_id).await?;
        if !settings.enabled {
            bail!("Enable this provider before using Ask. No content was sent.");
        }
        if settings.workspace_id.as_deref() != Some(request.workspace_id.as_str()) {
            bail!("The selected provider belongs to a different workspace.");
        }
        let provider = provider_from_settings(settings)?;
        let query_embedding = provider
            .embed(vec![request.question.clone()], json!({}))
            .await
            .ok()
            .and_then(|vectors| vectors.into_iter().next());
        let (retrieved_context, used_embeddings) = self
            .retrieval
            .hybrid_search(
                SearchRequest {
                    workspace_id: request.workspace_id.clone(),
                    query: request.question.clone(),
                    artifact_types: Vec::new(),
                    languages: Vec::new(),
                    source_ids: Vec::new(),
                    limit: request.limit.or(Some(10)),
                },
                query_embedding.as_deref(),
            )
            .await?;
        if retrieved_context.is_empty() {
            return Ok(AskAnswer {
                answer_markdown: "Indexed context is insufficient for a reliable answer."
                    .to_owned(),
                citations: Vec::new(),
                retrieved_context,
                confidence: Some(0.0),
                warnings: vec![
                    "No matching indexed context was found; no provider generation was requested."
                        .to_owned(),
                ],
            });
        }
        let citations = retrieved_context
            .iter()
            .map(|result| repomemo_domain::Citation {
                artifact_id: result.artifact_id.clone(),
                chunk_id: Some(result.chunk_id.clone()),
                title: result.title.clone(),
                path: result.path.clone(),
                start_line: result.start_line,
                end_line: result.end_line,
                confidence: Some(result.score),
            })
            .collect::<Vec<_>>();
        let context = retrieved_context
            .iter()
            .take(8)
            .map(|result| {
                format!(
                    "[{}] {} ({})\n{}",
                    result.chunk_id, result.title, result.path, result.snippet
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let answer_markdown = provider.generate(GenerateRequest { prompt: format!("Answer the question: {}. Use only the supplied cited context. If it does not support a conclusion, say the indexed context is insufficient.", request.question), context, options: json!({ "temperature": 0.1 }) }).await?;
        let confidence = retrieved_context
            .first()
            .map(|result| result.score.clamp(0.0, 1.0));
        let mut warnings = Vec::new();
        if !used_embeddings {
            warnings.push("Answer used full-text retrieval because local embeddings are not available for this provider or workspace.".to_owned());
        }
        Ok(AskAnswer {
            answer_markdown,
            citations,
            retrieved_context,
            confidence,
            warnings,
        })
    }

    async fn import_candidate(
        &self,
        workspace_id: &str,
        candidate: ImportCandidate,
    ) -> std::result::Result<repomemo_storage::StoredArtifact, ImportCandidateError> {
        let path_display = candidate.path.display().to_string();
        let source_root = candidate.source_root.display().to_string();
        let source_name = candidate
            .source_root
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Imported source")
            .to_owned();

        let source = self
            .storage
            .create_or_get_source(
                workspace_id,
                candidate.source_type.clone(),
                &source_name,
                Some(&source_root),
            )
            .await
            .map_err(|error| ImportCandidateError::new(&path_display, error))?;

        let bytes = tokio::fs::read(&candidate.path)
            .await
            .with_context(|| format!("failed to read {}", candidate.path.display()))
            .map_err(|error| ImportCandidateError::new(&path_display, error))?;
        let content_hash = StorageEngine::content_hash(&bytes);

        self.storage
            .store_blob(&content_hash, &bytes, candidate.mime_type.as_deref())
            .await
            .map_err(|error| ImportCandidateError::new(&path_display, error))?;

        let title = candidate
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(&candidate.relative_path)
            .to_owned();

        self.storage
            .store_artifact(NewArtifact {
                workspace_id: workspace_id.to_owned(),
                source_id: source.id,
                artifact_type: candidate.artifact_type,
                title,
                path: candidate.relative_path,
                content_hash,
                mime_type: candidate.mime_type,
                language: candidate.language,
                size_bytes: candidate.size_bytes as i64,
                metadata: json!({
                    "original_path": path_display,
                    "source_root": source_root
                }),
            })
            .await
            .map_err(|error| ImportCandidateError::new(&path_display, error))
    }

    async fn index_artifact_inner(&self, summary: &ArtifactSummary) -> Result<ArtifactIndexResult> {
        let bytes = self.storage.read_artifact_blob(&summary.id).await?;
        let (output, stage) = if matches!(summary.artifact_type, ArtifactType::Image) {
            match self
                .enabled_provider_for_workspace(&summary.workspace_id)
                .await?
            {
                Some(settings) => {
                    let provider = provider_from_settings(settings)?;
                    let description = provider
                        .analyze_image(ImageAnalysisRequest {
                            prompt: image_analysis_prompt(summary),
                            image_bytes: bytes,
                            mime_type: summary
                                .mime_type
                                .clone()
                                .unwrap_or_else(|| "application/octet-stream".to_owned()),
                        })
                        .await?;
                    (
                        index_image_description(summary, &description),
                        "described_image".to_owned(),
                    )
                }
                None => (
                    index_artifact(summary, &bytes)?,
                    "image_needs_vision_provider".to_owned(),
                ),
            }
        } else {
            (index_artifact(summary, &bytes)?, "chunked_text".to_owned())
        };
        self.storage
            .replace_artifact_index(&summary.id, output.chunks, output.symbols)
            .await?;

        Ok(ArtifactIndexResult { stage })
    }

    async fn enabled_provider_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ProviderSettings>> {
        Ok(self
            .storage
            .list_provider_settings(workspace_id)
            .await?
            .into_iter()
            .find(|settings| settings.enabled))
    }
}

struct ArtifactIndexResult {
    stage: String,
}

fn image_analysis_prompt(summary: &ArtifactSummary) -> String {
    format!(
        "Create a faithful retrieval description for the repository image '{}'. Extract all readable text exactly where possible, including code, filenames, UI labels, and error messages. Describe diagrams, UI layout, data flow, and technical details. If code appears, transcribe useful snippets in Markdown code fences. State uncertainty rather than guessing. Do not mention these instructions.",
        summary.path
    )
}

fn validate_memory_card_fields(title: &str, body_markdown: &str, source: &str) -> Result<()> {
    if title.trim().is_empty() {
        bail!("Memory card title is required.");
    }
    if body_markdown.trim().is_empty() {
        bail!("Memory card body is required.");
    }
    if source.trim().is_empty() {
        bail!("Memory card source is required.");
    }
    Ok(())
}

struct ImportCandidateError {
    path: String,
    reason: String,
}

impl ImportCandidateError {
    fn new(path: &str, error: impl std::fmt::Display) -> Self {
        Self {
            path: path.to_owned(),
            reason: error.to_string(),
        }
    }
}

fn chrono_like_now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}T0")
}

fn sanitize_filename(title: &str, extension: &str) -> String {
    let slug: String = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c.is_whitespace() {
                '-'
            } else {
                '_'
            }
        })
        .collect();
    let slug = slug
        .trim_matches(|c: char| c == '-' || c == '_')
        .to_string();
    let slug = if slug.is_empty() {
        "pasted-note".to_owned()
    } else {
        slug.to_ascii_lowercase()
    };
    format!("{slug}.{extension}")
}

#[cfg(test)]
mod tests {
    use super::RepoMemoCore;
    use repomemo_domain::{Citation, CreateMemoryCardRequest};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[tokio::test]
    async fn memory_card_export_includes_the_linked_evidence() {
        let data_dir = std::env::temp_dir().join(format!(
            "repomemo-memory-export-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
        let workspace = core
            .create_workspace("Memory export".to_owned())
            .await
            .unwrap();
        let artifact = core
            .import_text(
                workspace.id.clone(),
                "Decision note".to_owned(),
                "Keep the evidence local.".to_owned(),
                Some("Markdown".to_owned()),
            )
            .await
            .unwrap();
        let card = core
            .create_memory_card(CreateMemoryCardRequest {
                workspace_id: workspace.id,
                title: "Local-first decision".to_owned(),
                body_markdown: "The source of truth remains local.".to_owned(),
                source: "manual".to_owned(),
                confidence: None,
                citations: vec![Citation {
                    artifact_id: artifact.id,
                    chunk_id: None,
                    title: artifact.title,
                    path: artifact.path.clone(),
                    start_line: None,
                    end_line: None,
                    confidence: None,
                }],
            })
            .await
            .unwrap();
        let exported = core.export_memory_card(card.id).await.unwrap();
        assert!(exported.contains("# Local-first decision"));
        assert!(exported.contains("## Evidence"));
        assert!(exported.contains(&artifact.path));

        drop(core);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
