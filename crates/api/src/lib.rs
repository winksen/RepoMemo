use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use repomemo_ai::{
    provider_from_settings, validate_settings, AiProvider, GenerateRequest, ImageAnalysisRequest,
};
use repomemo_domain::{
    AppSettings, ArtifactDetail, ArtifactSummary, ArtifactType, AskAnswer, AskRequest, DocumentPreview,
    CreateMemoryCardRequest, ImportReport, ImportRequest, IndexingJobStatus, MemoryCard,
    MemoryCardDetail, MemoryCardSummary, ProviderSettings, ProviderTestResult, SearchRequest,
    SearchResult, SourceType, SummaryResult, Symbol, SymbolSearchResult, UpdateMemoryCardRequest,
    Workspace, WorkspaceOverview,
};
use repomemo_indexer::{index_artifact, index_image_description, INDEXER_VERSION};
use repomemo_ingestion::{
    detect_artifact_type, detect_language, detect_mime, discover_import_candidates,
    document_preview, extract_document_text, is_document, ImportCandidate, ImportOptions,
};
use repomemo_retrieval::{KeywordMode, RetrievalService};
use repomemo_storage::{NewArtifact, StorageConfig, StorageEngine};
use serde_json::json;

mod agent;
mod embeddings;
mod health;
mod knowledge_map;

pub use agent::{agent_capabilities, agent_conversation_title, agent_turn_label};
pub use embeddings::EmbeddingRun;

/// Passages Ask puts in front of the model, after reranking a wider pool.
const ASK_CONTEXT_PASSAGES: usize = 8;
/// Candidates Ask retrieves before reranking.
const ASK_CANDIDATES: i64 = 20;
/// Longest passage text Ask sends per citation.
const ASK_PASSAGE_CHARS: usize = 2_500;
/// Files whose indexed text fits in this many characters are summarized in
/// one request; longer ones are summarized part by part, then combined.
const SUMMARY_SINGLE_PASS_CHARS: usize = 14_000;
const SUMMARY_PART_CHARS: usize = 12_000;
const SUMMARY_MAX_PARTS: usize = 8;

#[derive(Debug, Clone)]
pub struct RepoMemoCore {
    storage: StorageEngine,
    retrieval: RetrievalService,
}

impl RepoMemoCore {
    pub async fn boot(data_dir: PathBuf) -> Result<Self> {
        let storage = StorageEngine::open(StorageConfig { data_dir }).await?;
        Ok(Self::from_storage(storage))
    }

    /// Build a core over an already-open storage engine. The shared server
    /// uses this so that its own `StorageEngine` handle and the core's handle
    /// point at the same pool and share observers (job events, activity feed).
    pub fn from_storage(storage: StorageEngine) -> Self {
        let retrieval = RetrievalService::new(storage.clone());
        Self { storage, retrieval }
    }

    pub fn storage(&self) -> &StorageEngine {
        &self.storage
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
            // Shared notes are written in Markdown but kept as their own type
            // with their own extension, so they are not mistaken for files.
            Some("Note") => (ArtifactType::Note, "text/markdown", "note"),
            Some("Text") | None => (ArtifactType::File, "text/plain", "txt"),
            Some(_) => (ArtifactType::CodeFile, "text/plain", "txt"),
        };

        let language = if matches!(artifact_type, ArtifactType::Note) {
            Some("Markdown".to_owned())
        } else {
            language
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
        let detected_mime = detect_mime(path);
        let fallback_mime = if is_document(path) {
            detected_mime
                .as_deref()
                .unwrap_or("application/octet-stream")
        } else if matches!(artifact_type, ArtifactType::Image) {
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
        let mut detail = self.storage.get_artifact(&artifact_id).await?;
        if is_document(Path::new(&detail.summary.path)) {
            let bytes = self.storage.read_artifact_blob(&detail.summary.id).await?;
            // An unreadable document still opens; it just has no text preview.
            let text = extract_document_text(Path::new(&detail.summary.path), &bytes)
                .ok()
                .flatten()
                .filter(|value| !value.trim().is_empty());
            detail.content_truncated = text.as_ref().is_some_and(|value| value.len() > 120_000);
            detail.content_preview = text.map(|value| value.chars().take(120_000).collect());
        }
        Ok(detail)
    }

    /// The stored original of an artifact, for download or opening in its app.
    pub async fn read_artifact_file(&self, artifact_id: &str) -> Result<(ArtifactSummary, Vec<u8>)> {
        let summary = self.storage.get_artifact_summary(artifact_id).await?;
        let bytes = self.storage.read_artifact_blob(artifact_id).await?;
        Ok((summary, bytes))
    }

    /// A reading preview of a business document (Word, Excel, PowerPoint, PDF,
    /// OneNote or Outlook message).
    pub async fn document_preview(&self, artifact_id: &str) -> Result<DocumentPreview> {
        let (summary, bytes) = self.read_artifact_file(artifact_id).await?;
        let path = summary.path.clone();
        tokio::task::spawn_blocking(move || document_preview(Path::new(&path), &bytes))
            .await
            .context("the document preview task failed")
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
                let _ = self.storage.clear_index_failure(&artifact_id).await;
                self.storage
                    .update_indexing_job(&job.id, "completed", &result.stage, 1, None)
                    .await
            }
            Err(error) => {
                let _ = self
                    .storage
                    .update_indexing_job(&job.id, "failed", "failed", 0, Some(&format!("{error:#}")))
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

        // Incremental: artifacts already indexed by the current indexer are left alone.
        let artifacts = self
            .storage
            .list_artifacts_needing_index(Some(&workspace_id), INDEXER_VERSION)
            .await?;
        let total = artifacts.len() as i64;
        let job = self
            .storage
            .create_job(
                &workspace_id,
                None,
                "indexing",
                "chunking_workspace",
                Some(total),
            )
            .await?;

        let mut indexed = 0_i64;
        for artifact in artifacts {
            if self.storage.is_job_cancel_requested(&job.id).await? {
                return self
                    .storage
                    .update_indexing_job(&job.id, "cancelled", "cancelled", indexed, None)
                    .await;
            }

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

    /// Artifacts that were never indexed, or were indexed by an older indexer
    /// version, across every workspace. Images are only listed until their
    /// first index, so refreshing never repeats vision analysis.
    pub async fn artifacts_needing_index(&self) -> Result<Vec<ArtifactSummary>> {
        self.storage
            .list_artifacts_needing_index(None, INDEXER_VERSION)
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

    /// Keyword search, plus passages found by meaning when the workspace has
    /// an embedding provider. A provider failure falls back to keywords so
    /// search keeps working when the embedding model is unreachable.
    pub async fn search_workspace(&self, request: SearchRequest) -> Result<Vec<SearchResult>> {
        if request.query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let embedding = self
            .query_embedding(&request.workspace_id, None, &request.query)
            .await
            .unwrap_or(None);
        let (results, _) = self
            .retrieval
            .hybrid_search(
                request,
                KeywordMode::AllTerms,
                embedding
                    .as_ref()
                    .map(|(model, vector)| (model.as_str(), vector.as_slice())),
            )
            .await?;
        Ok(results)
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
        if settings.purpose() == "embedding" {
            // An embedding model cannot chat, so test what it is used for.
            let model = settings.embedding_model_name().to_owned();
            let provider = provider_from_settings(settings)?;
            let (success, message) = match provider
                .embed(vec!["RepoMemo connection test".to_owned()], json!({}))
                .await
            {
                Ok(vectors) => (true, format!(
                    "Embeddings work with {model} ({} dimensions). Indexed passages are embedded in the background.",
                    vectors.first().map_or(0, Vec::len)
                )),
                Err(error) => (false, format!("{error:#}")),
            };
            return Ok(ProviderTestResult { provider_id, success, message });
        }
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
        let title = detail.summary.title.clone();
        let sections = detail
            .chunks
            .iter()
            .map(|chunk| match chunk.heading_path.as_deref().filter(|heading| !heading.is_empty()) {
                Some(heading) => format!("{heading}\n{}", chunk.text),
                None => chunk.text.clone(),
            })
            .collect::<Vec<_>>();
        let total_chars = sections.iter().map(|section| section.chars().count()).sum::<usize>();
        let mut warnings = vec!["Generated by the configured AI provider. Verify cited source text before relying on the summary.".to_owned()];
        let system = "You summarize technical documents faithfully for engineers. Use only the supplied text, keep names, numbers and decisions exact, and never add facts.".to_owned();

        let (summary_markdown, used) = if total_chars <= SUMMARY_SINGLE_PASS_CHARS {
            let summary = provider
                .generate(GenerateRequest {
                    system: Some(system),
                    prompt: format!("Summarize '{title}'. Open with one sentence on what it is, then the key points as a short list. Be concise."),
                    context: sections.join("\n\n"),
                    options: json!({ "temperature": 0.2 }),
                })
                .await?;
            (summary, (0..sections.len()).collect::<Vec<_>>())
        } else {
            // Too long for one request: take notes on each part, then write
            // the summary from the notes.
            let mut parts = group_sections(&sections, SUMMARY_PART_CHARS);
            if parts.len() > SUMMARY_MAX_PARTS {
                let all = parts.len();
                parts = evenly_spaced(parts, SUMMARY_MAX_PARTS);
                warnings.push(format!("This file is long, so the summary is based on {SUMMARY_MAX_PARTS} of its {all} parts, spread evenly through it."));
            }
            let mut notes = Vec::with_capacity(parts.len());
            for (number, part) in parts.iter().enumerate() {
                let context = part
                    .iter()
                    .map(|&index| sections[index].chars().take(SUMMARY_PART_CHARS).collect::<String>())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                let note = provider
                    .generate(GenerateRequest {
                        system: Some(system.clone()),
                        prompt: format!("This is part {} of {} of '{title}'. List its key points as short bullets: purpose, behaviour, decisions, names and numbers.", number + 1, parts.len()),
                        context,
                        options: json!({ "temperature": 0.1 }),
                    })
                    .await?;
                notes.push(format!("Part {}:\n{note}", number + 1));
            }
            let summary = provider
                .generate(GenerateRequest {
                    system: Some(system),
                    prompt: format!("These are notes on consecutive parts of '{title}'. Write one concise summary of the whole file: one sentence on what it is, then the key points as a short list. Merge repeated points."),
                    context: notes.join("\n\n"),
                    options: json!({ "temperature": 0.2 }),
                })
                .await?;
            (summary, parts.into_iter().flatten().collect())
        };
        let citations = used
            .into_iter()
            .map(|index| &detail.chunks[index])
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
            warnings,
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
            system: Some("You brief engineers joining a project, using only the supplied excerpts from its files. Never add facts that are not in them.".to_owned()),
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
            .create_job(
                &workspace_id,
                None,
                "embedding",
                "embedding_workspace",
                Some(chunks.len() as i64),
            )
            .await?;
        let provider = provider_from_settings(settings.clone())?;
        let mut completed = 0_i64;
        for batch in chunks.chunks(16) {
            if self.storage.is_job_cancel_requested(&job.id).await? {
                return self
                    .storage
                    .update_indexing_job(&job.id, "cancelled", "cancelled", completed, None)
                    .await;
            }
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
        let mut warnings = Vec::new();
        let semantic_configured = self
            .embedding_provider_for_workspace(&request.workspace_id)
            .await?
            .is_some();
        let query_embedding = match self
            .query_embedding(&request.workspace_id, Some(provider_id), &request.question)
            .await
        {
            Ok(embedding) => embedding,
            Err(error) => {
                warnings.push(format!("Search by meaning was skipped because the embedding provider failed: {error:#}"));
                None
            }
        };
        let (candidates, used_embeddings) = self
            .retrieval
            .hybrid_search(
                SearchRequest {
                    workspace_id: request.workspace_id.clone(),
                    query: request.question.clone(),
                    artifact_types: Vec::new(),
                    languages: Vec::new(),
                    source_ids: Vec::new(),
                    limit: Some(ASK_CANDIDATES),
                },
                KeywordMode::AnyTerm,
                query_embedding
                    .as_ref()
                    .map(|(model, vector)| (model.as_str(), vector.as_slice())),
            )
            .await?;
        if candidates.is_empty() {
            return Ok(AskAnswer {
                answer_markdown: "Indexed context is insufficient for a reliable answer."
                    .to_owned(),
                citations: Vec::new(),
                retrieved_context: candidates,
                confidence: Some(0.0),
                warnings: vec![
                    "No matching indexed context was found; no provider generation was requested."
                        .to_owned(),
                ],
            });
        }

        // Search results carry short snippets; the model needs the passages.
        let texts = self
            .storage
            .chunk_texts(&candidates.iter().map(|result| result.chunk_id.clone()).collect::<Vec<_>>())
            .await?;
        let passage = |result: &SearchResult| {
            texts
                .get(&result.chunk_id)
                .cloned()
                .unwrap_or_else(|| result.snippet.clone())
        };

        // Keyword and vector scores only approximate relevance; the chat model
        // judges it far better, so it picks which candidates fill the context.
        let mut ranked = candidates;
        if ranked.len() > ASK_CONTEXT_PASSAGES {
            let listing = ranked
                .iter()
                .map(|result| format!("{} ({})\n{}", result.title, result.path, passage(result)))
                .collect::<Vec<_>>();
            match provider.rerank(request.question.clone(), listing).await {
                Ok(order) => {
                    let mut slots = ranked.into_iter().map(Some).collect::<Vec<_>>();
                    ranked = order.into_iter().filter_map(|index| slots.get_mut(index).and_then(Option::take)).collect();
                }
                Err(error) => warnings.push(format!("Results were not reranked because the AI provider failed: {error:#}")),
            }
        }
        ranked.truncate(request.limit.map_or(ASK_CONTEXT_PASSAGES, |limit| (limit.max(1) as usize).min(ASK_CONTEXT_PASSAGES)));
        let retrieved_context = ranked;

        let context = retrieved_context
            .iter()
            .enumerate()
            .map(|(index, result)| {
                let text = passage(result).chars().take(ASK_PASSAGE_CHARS).collect::<String>();
                format!("[{}] {} ({})\n{text}", index + 1, result.title, result.path)
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let answer_markdown = provider
            .generate(GenerateRequest {
                system: Some("You answer questions about a workspace of technical material using only the numbered passages supplied. Cite the passages you rely on as [1], [2] and so on. If the passages do not answer the question, say that the indexed context is insufficient instead of guessing.".to_owned()),
                prompt: format!("Question: {}", request.question),
                context,
                options: json!({ "temperature": 0.1 }),
            })
            .await?;
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
        let confidence = retrieved_context
            .first()
            .map(|result| result.score.clamp(0.0, 1.0));
        if !used_embeddings && !semantic_configured {
            warnings.push("Answer used keyword search only. An administrator can set up AI for search in Settings so passages are also found by meaning.".to_owned());
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
                .vision_provider_for_workspace(&summary.workspace_id)
                .await?
            {
                Some(settings) => {
                    let provider = provider_from_settings(settings)?;
                    // Fail fast with a readable reason (Ollama stopped, model
                    // missing) instead of waiting on a doomed image request.
                    let check = provider
                        .test_connection()
                        .await
                        .context("the image AI provider is not reachable")?;
                    if !check.success {
                        bail!("{}", check.message);
                    }
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
            .replace_artifact_index(&summary.id, output.chunks, output.symbols, INDEXER_VERSION)
            .await?;

        Ok(ArtifactIndexResult { stage })
    }

    /// The provider that describes images: an enabled provider set up for
    /// vision, or else an enabled provider saved before purposes existed (the
    /// single-provider desktop setup).
    async fn vision_provider_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ProviderSettings>> {
        let providers = self.storage.list_provider_settings(workspace_id).await?;
        Ok(providers
            .iter()
            .find(|settings| settings.enabled && settings.purpose() == "vision")
            .or_else(|| {
                providers
                    .iter()
                    .find(|settings| settings.enabled && !settings.has_explicit_purpose())
            })
            .cloned())
    }

    /// Image uploads are only accepted once an image AI provider is enabled, so
    /// a picture is never stored without a way to make it searchable.
    pub async fn ensure_upload_allowed(&self, workspace_id: &str, filename: &str) -> Result<()> {
        let is_image = matches!(
            detect_artifact_type(Path::new(filename)),
            Some(ArtifactType::Image)
        );
        if !is_image {
            return Ok(());
        }
        let has_vision = self
            .storage
            .list_provider_settings(workspace_id)
            .await?
            .iter()
            .any(|settings| settings.enabled && settings.purpose() == "vision");
        if !has_vision {
            bail!(
                "An image AI provider is required to upload images. An administrator can set one up in Settings."
            );
        }
        Ok(())
    }
}

struct ArtifactIndexResult {
    stage: String,
}

/// Splits consecutive sections into parts of at most `max_chars` (a single
/// longer section forms its own part), returning section indices per part.
fn group_sections(sections: &[String], max_chars: usize) -> Vec<Vec<usize>> {
    let mut parts: Vec<Vec<usize>> = Vec::new();
    let mut size = 0_usize;
    for (index, section) in sections.iter().enumerate() {
        let length = section.chars().count();
        match parts.last_mut() {
            Some(part) if size + length <= max_chars => {
                part.push(index);
                size += length;
            }
            _ => {
                parts.push(vec![index]);
                size = length;
            }
        }
    }
    parts
}

/// `count` items spread evenly from first to last, keeping their order.
fn evenly_spaced<T>(items: Vec<T>, count: usize) -> Vec<T> {
    if items.len() <= count || count == 0 {
        return items;
    }
    let last = items.len() - 1;
    let keep = (0..count)
        .map(|step| step * last / (count - 1).max(1))
        .collect::<std::collections::BTreeSet<_>>();
    items
        .into_iter()
        .enumerate()
        .filter_map(|(index, item)| keep.contains(&index).then_some(item))
        .collect()
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
    use super::{evenly_spaced, group_sections, RepoMemoCore};
    use repomemo_domain::{Citation, CreateMemoryCardRequest};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn long_files_are_split_into_bounded_parts() {
        let sections = ["aaaa", "bbbb", "cccccccccc", "dd"].map(str::to_owned);
        assert_eq!(group_sections(&sections, 8), vec![vec![0, 1], vec![2], vec![3]]);
        assert_eq!(evenly_spaced((0..10).collect(), 4), vec![0, 3, 6, 9]);
        assert_eq!(evenly_spaced(vec![1, 2], 4), vec![1, 2]);
    }

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

    #[tokio::test]
    async fn workspace_indexing_skips_artifacts_that_are_already_current() {
        let data_dir = std::env::temp_dir().join(format!(
            "repomemo-incremental-index-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
        let workspace = core
            .create_workspace("Incremental".to_owned())
            .await
            .unwrap();
        let first = core
            .import_text(
                workspace.id.clone(),
                "First".to_owned(),
                "# First
One.".to_owned(),
                Some("Markdown".to_owned()),
            )
            .await
            .unwrap();
        assert_eq!(core.artifacts_needing_index().await.unwrap().len(), 1);

        let job = core.index_workspace(workspace.id.clone()).await.unwrap();
        assert_eq!(job.status, "completed");
        assert_eq!(job.progress_total, Some(1));
        let chunk_ids = |detail: repomemo_domain::ArtifactDetail| {
            detail.chunks.into_iter().map(|chunk| chunk.id).collect::<Vec<_>>()
        };
        let indexed_ids = chunk_ids(core.get_artifact(first.id.clone()).await.unwrap());
        assert_eq!(indexed_ids.len(), 1);
        assert!(core.artifacts_needing_index().await.unwrap().is_empty());

        // Nothing is stale, so a second pass does no work and keeps every chunk.
        let job = core.index_workspace(workspace.id.clone()).await.unwrap();
        assert_eq!(job.status, "completed");
        assert_eq!(job.progress_total, Some(0));
        assert_eq!(
            chunk_ids(core.get_artifact(first.id.clone()).await.unwrap()),
            indexed_ids
        );

        // A forced re-index of unchanged content keeps chunk identity too.
        core.index_artifact(first.id.clone()).await.unwrap();
        assert_eq!(
            chunk_ids(core.get_artifact(first.id).await.unwrap()),
            indexed_ids
        );

        drop(core);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
