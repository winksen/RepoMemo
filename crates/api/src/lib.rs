use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use repomemo_ai::{provider_from_settings, validate_settings, AiProvider, GenerateRequest, ImageAnalysisRequest};
use repomemo_domain::{
    AgentHistoryTurn, AppSettings, ArtifactDetail, ArtifactSummary, ArtifactType, AskAnswer, AskRequest,
    CreateMemoryCardRequest, DocumentPreview, ImportReport, ImportRequest, IndexingJobStatus, MemoryCard,
    MemoryCardDetail, MemoryCardSummary, ProviderSettings, ProviderTestResult, SearchRequest, SearchResult, SourceType,
    SummaryResult, Symbol, SymbolSearchResult, UpdateMemoryCardRequest, Workspace, WorkspaceOverview,
};
use repomemo_indexer::{index_artifact, index_image_description, INDEXER_VERSION};
use repomemo_ingestion::{
    detect_artifact_type, detect_language, discover_import_candidates, document_preview, extract_document_text,
    is_document, ImportCandidate, ImportOptions,
};
use repomemo_retrieval::{KeywordMode, RetrievalService};
use repomemo_storage::{NewArtifact, StagedBlob, StorageConfig, StorageEngine};
use serde_json::json;

mod agent;
mod archive;
mod budget;
mod codeowners;
mod connectors;
mod embeddings;
mod health;
mod knowledge_map;
mod meaning;
mod mentions;
mod repo_overview;
mod remote_policy;
mod remote_repo;
mod repo_policy;
mod repo_stack;
mod repo_sync;
mod template;
mod versions;

pub use agent::{
    agent_capabilities, agent_conversation_title, agent_turn_label, HISTORY_MAX_TURNS, HISTORY_TOKEN_BUDGET,
};
pub use archive::{read_archive_entries, write_workspace_archive, ArchiveEncoder, ArchiveReader};
pub use budget::BackgroundAiBudget;
pub use embeddings::{EmbeddingRebuildPlan, EmbeddingRun};
pub use remote_policy::{RemoteGitPolicy, RemoteRepository, RemoteScheme};
pub use remote_repo::{looks_remote, CloneLimitHandles, CloneSize, RemoteAuth, RemoteDeployKey, RemoteToken};
pub use repo_policy::{is_network_path, RepoLinkPolicy};
pub use repo_sync::RepoHead;
pub use repomemo_ingestion::{
    abandoned_readings, killed_readings, reads_in_a_process, recent_readings_of, run_as_document_reader_if_requested,
    served_mime_type, set_document_memory_limit_mb, set_document_time_limit, set_reader_process, started_readings,
    LimitedAllocator, ReaderProcess, DEFAULT_MEMORY_LIMIT_MB,
};

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
/// Longest text the artifact page shows for a document.
const DOCUMENT_PREVIEW_CHARS: usize = 120_000;

/// What an upload holds: bytes in memory, or a file already received into a
/// temporary file and hashed.
enum UploadContent {
    Bytes(Vec<u8>),
    Staged(StagedBlob),
}

impl UploadContent {
    fn size_bytes(&self) -> u64 {
        match self {
            Self::Bytes(bytes) => bytes.len() as u64,
            Self::Staged(staged) => staged.size_bytes,
        }
    }

    fn is_empty(&self) -> bool {
        self.size_bytes() == 0
    }

    fn content_hash(&self) -> String {
        match self {
            Self::Bytes(bytes) => StorageEngine::content_hash(bytes),
            Self::Staged(staged) => staged.content_hash.clone(),
        }
    }

    async fn store(self, storage: &StorageEngine, content_hash: &str, mime_type: &str) -> Result<()> {
        match self {
            Self::Bytes(bytes) => storage
                .store_blob(content_hash, &bytes, Some(mime_type))
                .await
                .map(|_| ()),
            Self::Staged(staged) => storage.store_staged_blob(staged, Some(mime_type)).await.map(|_| ()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RepoMemoCore {
    storage: StorageEngine,
    retrieval: RetrievalService,
    repo_policy: std::sync::Arc<RepoLinkPolicy>,
    remote_git_policy: std::sync::Arc<RemoteGitPolicy>,
    clone_limits: CloneLimitHandles,
    ai_budget: BackgroundAiBudget,
    /// Per-file results of the knowledge map and health, kept between requests.
    meaning: meaning::MeaningCache,
    /// The template each workspace uses, settled once and kept until its choice changes.
    templates: template::TemplateResolver,
}

impl RepoMemoCore {
    pub async fn boot(data_dir: PathBuf) -> Result<Self> {
        let storage = StorageEngine::open(StorageConfig::new(data_dir)).await?;
        Ok(Self::from_storage(storage))
    }

    /// Build a core over an already-open storage engine. The shared server
    /// uses this so that its own `StorageEngine` handle and the core's handle
    /// point at the same pool and share observers (job events, activity feed).
    pub fn from_storage(storage: StorageEngine) -> Self {
        let retrieval = RetrievalService::new(storage.clone());
        Self {
            storage,
            retrieval,
            repo_policy: std::sync::Arc::new(RepoLinkPolicy::unrestricted()),
            remote_git_policy: std::sync::Arc::new(RemoteGitPolicy::disabled()),
            clone_limits: CloneLimitHandles::default(),
            ai_budget: BackgroundAiBudget::default(),
            meaning: meaning::MeaningCache::default(),
            templates: template::TemplateResolver::default(),
        }
    }

    /// The daily allowance of AI calls made in the background.
    pub fn background_ai_budget(&self) -> &BackgroundAiBudget {
        &self.ai_budget
    }

    /// Restricts which folders repository links may point at. Applies to new
    /// links and to every later sync of existing ones.
    pub fn with_repo_link_policy(mut self, policy: RepoLinkPolicy) -> Self {
        self.repo_policy = std::sync::Arc::new(policy);
        self
    }

    pub fn repo_link_policy(&self) -> &RepoLinkPolicy {
        &self.repo_policy
    }

    /// Sets the hosts remote repository links may use. Without any, remote
    /// links are off (also the default of a core with no policy given).
    pub fn with_remote_git_policy(mut self, policy: RemoteGitPolicy) -> Self {
        self.remote_git_policy = std::sync::Arc::new(policy);
        self
    }

    pub fn remote_git_policy(&self) -> &RemoteGitPolicy {
        &self.remote_git_policy
    }

    pub fn storage(&self) -> &StorageEngine {
        &self.storage
    }

    pub async fn create_workspace(&self, name: String) -> Result<Workspace> {
        self.storage.create_workspace(&name).await
    }

    /// Writes a workspace as an archive file a server can import (V0.2.50, the desktop app's
    /// "move to a server"). The file is complete or not there.
    pub async fn export_workspace_archive(
        &self,
        workspace_id: &str,
        destination: &Path,
    ) -> Result<repomemo_domain::ArchiveManifest> {
        if !self.storage.workspace_exists(workspace_id).await? {
            repomemo_domain::bail_not_found!("Workspace was not found.");
        }
        write_workspace_archive(&self.storage, workspace_id, env!("CARGO_PKG_VERSION"), destination).await
    }

    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>> {
        self.storage.list_workspaces().await
    }

    pub async fn update_workspace_name(&self, workspace_id: String, name: String) -> Result<Workspace> {
        self.storage.update_workspace_name(&workspace_id, &name).await
    }

    pub async fn delete_workspace(&self, workspace_id: String) -> Result<()> {
        self.templates.forget(&workspace_id);
        self.storage.delete_workspace(&workspace_id).await
    }

    pub async fn import_paths(&self, request: ImportRequest) -> Result<ImportReport> {
        if request.workspace_id.trim().is_empty() {
            repomemo_domain::bail_invalid!("Workspace id is required.");
        }

        if request.paths.is_empty() {
            repomemo_domain::bail_invalid!("Choose at least one file or folder to import.");
        }

        if !self.storage.workspace_exists(&request.workspace_id).await? {
            repomemo_domain::bail_not_found!("Workspace was not found.");
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
            match self.import_candidate(&request.workspace_id, candidate).await {
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
                    report.skipped_items.push(repomemo_domain::ImportSkippedItem {
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
            repomemo_domain::bail_invalid!("Workspace id is required.");
        }
        if content.trim().is_empty() {
            repomemo_domain::bail_invalid!("Pasted content cannot be empty.");
        }
        if !self.storage.workspace_exists(&workspace_id).await? {
            repomemo_domain::bail_not_found!("Workspace was not found.");
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

        self.storage.store_blob(&content_hash, &bytes, Some(mime_type)).await?;

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
        // What the uploader claimed; ignored, see `served_mime_type`.
        _claimed_mime_type: Option<String>,
    ) -> Result<ArtifactSummary> {
        self.import_upload_content(workspace_id, filename, UploadContent::Bytes(bytes))
            .await
    }

    /// The same as [`RepoMemoCore::import_upload`] for a file received into a
    /// temporary file (see `StorageEngine::begin_staged_blob`), which is moved
    /// into the blob store without being read into memory.
    pub async fn import_staged_upload(
        &self,
        workspace_id: String,
        filename: String,
        staged: StagedBlob,
    ) -> Result<ArtifactSummary> {
        self.import_upload_content(workspace_id, filename, UploadContent::Staged(staged))
            .await
    }

    async fn import_upload_content(
        &self,
        workspace_id: String,
        filename: String,
        content: UploadContent,
    ) -> Result<ArtifactSummary> {
        if workspace_id.trim().is_empty() {
            repomemo_domain::bail_invalid!("Workspace id is required.");
        }
        if content.is_empty() {
            repomemo_domain::bail_invalid!("Uploaded files cannot be empty.");
        }
        if !self.storage.workspace_exists(&workspace_id).await? {
            repomemo_domain::bail_not_found!("Workspace was not found.");
        }

        let path = Path::new(&filename);
        let safe_filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| repomemo_domain::invalid!("A valid upload filename is required."))?
            .to_owned();
        let artifact_type = detect_artifact_type(path)
            .ok_or_else(|| repomemo_domain::invalid!("This file type is not supported for shared upload."))?;
        let language = detect_language(path);
        // From the file name, never from the uploader: a file labelled
        // text/html or image/svg+xml could otherwise run as a page.
        let mime_type = served_mime_type(path).to_owned();
        let source = self
            .storage
            .create_or_get_source(&workspace_id, SourceType::Upload, "Shared uploads", None)
            .await
            .with_context(|| "failed to create shared-uploads source")?;
        let content_hash = content.content_hash();
        let size_bytes = content.size_bytes() as i64;

        content.store(&self.storage, &content_hash, &mime_type).await?;
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

    /// Uploads a new version of an existing file into the same artifact, so its
    /// comments, folder, lifecycle and memory links stay with it. The new file
    /// must be the same kind of file (same extension).
    pub async fn replace_artifact_version(
        &self,
        artifact_id: &str,
        filename: &str,
        bytes: Vec<u8>,
        uploaded_by: Option<&str>,
    ) -> Result<ArtifactSummary> {
        self.replace_artifact_content_with(artifact_id, filename, UploadContent::Bytes(bytes), uploaded_by)
            .await
    }

    /// [`RepoMemoCore::replace_artifact_version`] for a file received into a
    /// temporary file, moved into the blob store without being read into memory.
    pub async fn replace_artifact_version_staged(
        &self,
        artifact_id: &str,
        filename: &str,
        staged: StagedBlob,
        uploaded_by: Option<&str>,
    ) -> Result<ArtifactSummary> {
        self.replace_artifact_content_with(artifact_id, filename, UploadContent::Staged(staged), uploaded_by)
            .await
    }

    async fn replace_artifact_content_with(
        &self,
        artifact_id: &str,
        filename: &str,
        content: UploadContent,
        uploaded_by: Option<&str>,
    ) -> Result<ArtifactSummary> {
        if content.is_empty() {
            repomemo_domain::bail_invalid!("Uploaded files cannot be empty.");
        }
        let current = self.storage.get_artifact_summary(artifact_id).await?;
        let extension = |name: &str| {
            Path::new(name)
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase)
        };
        if extension(&current.path) != extension(filename) {
            repomemo_domain::bail_invalid!(
                "A new version must be the same kind of file as {}. Upload it as a new file instead.",
                current.path
            );
        }
        let mime_type = served_mime_type(Path::new(&current.path)).to_owned();
        let content_hash = content.content_hash();
        let size_bytes = content.size_bytes() as i64;
        content.store(&self.storage, &content_hash, &mime_type).await?;
        self.storage
            .replace_artifact_content(artifact_id, &content_hash, size_bytes, Some(&mime_type), uploaded_by)
            .await
    }

    pub async fn list_artifacts(&self, workspace_id: String) -> Result<Vec<ArtifactSummary>> {
        self.storage.list_artifacts(&workspace_id).await
    }

    pub async fn get_artifact(&self, artifact_id: String) -> Result<ArtifactDetail> {
        let mut detail = self.storage.get_artifact(&artifact_id).await?;
        if is_document(Path::new(&detail.summary.path)) {
            let text = match stored_document_text(&detail) {
                Some(text) => text,
                None => {
                    // Not indexed yet (or changed since): read it once, off the async threads.
                    let bytes = self.storage.read_artifact_blob(&detail.summary.id).await?;
                    let path = detail.summary.path.clone();
                    tokio::task::spawn_blocking(move || extract_document_text(Path::new(&path), &bytes).ok().flatten())
                        .await
                        .context("the document reading task failed")?
                }
            }
            // An unreadable document still opens; it just has no text preview.
            .filter(|value| !value.trim().is_empty());
            detail.content_truncated = text.as_ref().is_some_and(|value| value.len() > DOCUMENT_PREVIEW_CHARS);
            detail.content_preview = text.map(|value| value.chars().take(DOCUMENT_PREVIEW_CHARS).collect());
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

    pub async fn update_artifact_title(&self, artifact_id: String, title: String) -> Result<ArtifactSummary> {
        self.storage.update_artifact_title(&artifact_id, &title).await
    }

    pub async fn delete_artifact(&self, artifact_id: String) -> Result<()> {
        self.storage.delete_artifact(&artifact_id).await
    }

    pub async fn index_artifact(&self, artifact_id: String) -> Result<IndexingJobStatus> {
        let pending = self.begin_index_artifact(artifact_id).await?;
        self.finish_index_artifact(pending).await
    }

    /// The first half of indexing one artifact: checks it, and records a
    /// running job so a caller can hand it back before the work is done.
    pub async fn begin_index_artifact(&self, artifact_id: String) -> Result<PendingArtifactIndex> {
        if artifact_id.trim().is_empty() {
            repomemo_domain::bail_invalid!("Artifact id is required.");
        }

        let summary = self.storage.get_artifact_summary(&artifact_id).await?;
        let stage = if matches!(summary.artifact_type, ArtifactType::Image) {
            "analyzing_image"
        } else {
            "extracting_text"
        };
        let job = self
            .storage
            .create_indexing_job(&summary.workspace_id, Some(&summary.source_id), stage, Some(1))
            .await?;
        Ok(PendingArtifactIndex { job, summary })
    }

    /// The second half: does the work and closes the job, completed or failed.
    pub async fn finish_index_artifact(&self, pending: PendingArtifactIndex) -> Result<IndexingJobStatus> {
        let PendingArtifactIndex { job, summary } = pending;
        match self.index_artifact_inner(&summary).await {
            Ok(result) => {
                let _ = self.storage.clear_index_failure(&summary.id).await;
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
        let pending = self.begin_index_workspace(workspace_id).await?;
        self.finish_index_workspace(pending).await
    }

    /// The first half of re-indexing a workspace: finds what is stale and
    /// records a running job whose total is that number.
    pub async fn begin_index_workspace(&self, workspace_id: String) -> Result<PendingWorkspaceIndex> {
        if workspace_id.trim().is_empty() {
            repomemo_domain::bail_invalid!("Workspace id is required.");
        }

        if !self.storage.workspace_exists(&workspace_id).await? {
            repomemo_domain::bail_not_found!("Workspace was not found.");
        }

        // Incremental: artifacts already indexed by the current indexer are left alone.
        let artifacts = self
            .storage
            .list_artifacts_needing_index(Some(&workspace_id), INDEXER_VERSION, true)
            .await?;
        let total = artifacts.len() as i64;
        let job = self
            .storage
            .create_job(&workspace_id, None, "indexing", "chunking_workspace", Some(total))
            .await?;
        Ok(PendingWorkspaceIndex { job, artifacts })
    }

    /// The second half: indexes one artifact after another, honouring a
    /// cancel request between them, and closes the job.
    pub async fn finish_index_workspace(&self, pending: PendingWorkspaceIndex) -> Result<IndexingJobStatus> {
        let PendingWorkspaceIndex { job, artifacts } = pending;
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
    /// first index, so refreshing never repeats vision analysis. Repository
    /// files are left to their repository's sync.
    pub async fn artifacts_needing_index(&self) -> Result<Vec<ArtifactSummary>> {
        self.storage
            .list_artifacts_needing_index(None, INDEXER_VERSION, false)
            .await
    }

    pub async fn workspace_overview(&self, workspace_id: String) -> Result<WorkspaceOverview> {
        self.storage.workspace_overview(&workspace_id).await
    }

    pub async fn create_memory_card(&self, request: CreateMemoryCardRequest) -> Result<MemoryCard> {
        validate_memory_card_fields(&request.title, &request.body_markdown, &request.source)?;
        if !self.storage.workspace_exists(&request.workspace_id).await? {
            repomemo_domain::bail_not_found!("Workspace was not found.");
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
                request.citations.as_deref(),
            )
            .await
    }

    pub async fn delete_memory_card(&self, card_id: String) -> Result<()> {
        if card_id.trim().is_empty() {
            repomemo_domain::bail_invalid!("Memory card id is required.");
        }
        self.storage.delete_memory_card(&card_id).await
    }

    pub async fn list_memory_cards(&self, workspace_id: String) -> Result<Vec<MemoryCardSummary>> {
        if workspace_id.trim().is_empty() {
            repomemo_domain::bail_invalid!("Workspace id is required.");
        }
        self.storage.list_memory_cards(&workspace_id).await
    }

    pub async fn search_memory_cards(&self, workspace_id: String, query: String) -> Result<Vec<MemoryCardSummary>> {
        if workspace_id.trim().is_empty() {
            repomemo_domain::bail_invalid!("Workspace id is required.");
        }
        self.storage.search_memory_cards(&workspace_id, &query).await
    }

    pub async fn get_memory_card(&self, card_id: String) -> Result<MemoryCardDetail> {
        if card_id.trim().is_empty() {
            repomemo_domain::bail_invalid!("Memory card id is required.");
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
                        evidence.title.unwrap_or_else(|| "Untitled evidence".to_owned()),
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
        Ok(self.search_workspace_detailed(request, true).await?.0)
    }

    /// Search, saying whether the query was also matched by meaning. With
    /// `by_meaning` false the query is never sent to the workspace's embedding
    /// provider: the search is by keywords only. A search across workspaces
    /// uses this for those where the AI policy keeps AI from the caller.
    pub async fn search_workspace_detailed(
        &self,
        request: SearchRequest,
        by_meaning: bool,
    ) -> Result<(Vec<SearchResult>, bool)> {
        if request.query.trim().is_empty() {
            return Ok((Vec::new(), false));
        }
        let embedding = if by_meaning {
            self.query_embedding(&request.workspace_id, None, &request.query)
                .await
                .unwrap_or(None)
        } else {
            None
        };
        self.search_with_embedding(request, embedding).await
    }

    async fn search_with_embedding(
        &self,
        request: SearchRequest,
        embedding: Option<(String, Vec<f32>)>,
    ) -> Result<(Vec<SearchResult>, bool)> {
        let profile = self.ranking_profile(&request.workspace_id).await;
        let (results, _) = self
            .retrieval
            .hybrid_search(
                request,
                KeywordMode::AllTerms,
                embedding
                    .as_ref()
                    .map(|(model, vector)| (model.as_str(), vector.as_slice())),
                &profile,
            )
            .await?;
        Ok((results, embedding.is_some()))
    }

    /// [`RepoMemoCore::search_workspace_detailed`] for several workspaces at once, `concurrency` of
    /// them at a time. Each input is a search and whether it may be by meaning; the answers come back
    /// in the order of the inputs. A query that is searched by meaning is embedded once for each
    /// provider (the same kind, address and model, and key) the workspaces use, not once for each
    /// workspace; a provider that fails leaves its workspaces on keywords.
    pub async fn search_workspaces_detailed(
        &self,
        inputs: Vec<(SearchRequest, bool)>,
        concurrency: usize,
    ) -> Vec<Result<(Vec<SearchResult>, bool)>> {
        let mut shared_embeddings: std::collections::HashMap<String, Option<(String, Vec<f32>)>> =
            std::collections::HashMap::new();
        let mut keys: Vec<Option<String>> = Vec::with_capacity(inputs.len());
        for (request, by_meaning) in &inputs {
            let mut key = None;
            if *by_meaning && !request.query.trim().is_empty() {
                if let Ok(Some(settings)) = self.embedding_provider_for_workspace(&request.workspace_id).await {
                    let shared = format!(
                        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
                        settings.provider_type,
                        settings.base_url.as_deref().unwrap_or_default(),
                        settings.embedding_model_name(),
                        settings.api_key.as_deref().unwrap_or_default(),
                        request.query
                    );
                    if !shared_embeddings.contains_key(&shared) {
                        let model = settings.embedding_model_name().to_owned();
                        let embedded = match provider_from_settings(settings) {
                            Ok(provider) => provider
                                .embed(vec![request.query.clone()], json!({}))
                                .await
                                .ok()
                                .and_then(|vectors| vectors.into_iter().next())
                                .filter(|vector| !vector.is_empty())
                                .map(|vector| (model, vector)),
                            Err(_) => None,
                        };
                        shared_embeddings.insert(shared.clone(), embedded);
                    }
                    key = Some(shared);
                }
            }
            keys.push(key);
        }
        let count = inputs.len();
        let gate = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency.max(1)));
        let mut running = tokio::task::JoinSet::new();
        for (index, (request, _)) in inputs.into_iter().enumerate() {
            let embedding = keys[index]
                .as_ref()
                .and_then(|key| shared_embeddings.get(key).cloned().flatten());
            let core = self.clone();
            let gate = gate.clone();
            running.spawn(async move {
                let _turn = gate.acquire_owned().await;
                let outcome = if request.query.trim().is_empty() {
                    Ok((Vec::new(), false))
                } else {
                    core.search_with_embedding(request, embedding).await
                };
                (index, outcome)
            });
        }
        let mut outcomes: Vec<Option<Result<(Vec<SearchResult>, bool)>>> = (0..count).map(|_| None).collect();
        while let Some(finished) = running.join_next().await {
            if let Ok((index, outcome)) = finished {
                outcomes[index] = Some(outcome);
            }
        }
        outcomes
            .into_iter()
            .map(|outcome| outcome.unwrap_or_else(|| Err(anyhow::anyhow!("the search of a workspace did not finish"))))
            .collect()
    }

    pub async fn list_symbols(&self, artifact_id: String) -> Result<Vec<Symbol>> {
        if artifact_id.trim().is_empty() {
            repomemo_domain::bail_invalid!("Artifact id is required.");
        }
        self.storage.list_symbols(&artifact_id).await
    }

    pub async fn search_symbols(&self, workspace_id: String, query: String) -> Result<Vec<SymbolSearchResult>> {
        if workspace_id.trim().is_empty() {
            repomemo_domain::bail_invalid!("Workspace id is required.");
        }
        self.storage.search_symbols(&workspace_id, &query, 30).await
    }

    pub async fn list_provider_settings(&self, workspace_id: String) -> Result<Vec<ProviderSettings>> {
        if workspace_id.trim().is_empty() {
            repomemo_domain::bail_invalid!("Workspace id is required.");
        }
        self.storage.list_provider_settings(&workspace_id).await
    }

    pub async fn save_provider_settings(&self, settings: ProviderSettings) -> Result<ProviderSettings> {
        let workspace_id = settings
            .workspace_id
            .as_deref()
            .context("Provider settings must belong to a workspace.")?;
        if !self.storage.workspace_exists(workspace_id).await? {
            repomemo_domain::bail_not_found!("Workspace was not found.");
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
                Ok(vectors) => (
                    true,
                    format!(
                    "Embeddings work with {model} ({} dimensions). Indexed passages are embedded in the background.",
                    vectors.first().map_or(0, Vec::len)
                ),
                ),
                Err(error) => (false, format!("{error:#}")),
            };
            return Ok(ProviderTestResult {
                provider_id,
                success,
                message,
            });
        }
        let provider = provider_from_settings(settings)?;
        provider.test_connection().await
    }

    pub async fn summarize_artifact(&self, artifact_id: String, provider_id: String) -> Result<SummaryResult> {
        let detail = self.storage.get_artifact(&artifact_id).await?;
        if detail.chunks.is_empty() {
            repomemo_domain::bail_invalid!(
                "Index this artifact before requesting a summary so RepoMemo can cite its content."
            );
        }
        let settings = self.storage.get_provider_settings(&provider_id).await?;
        if !settings.enabled {
            repomemo_domain::bail_invalid!("Enable this local provider before using AI. No content was sent.");
        }
        if settings.workspace_id.as_deref() != Some(detail.summary.workspace_id.as_str()) {
            repomemo_domain::bail_invalid!("The selected provider belongs to a different workspace.");
        }
        let provider = provider_from_settings(settings)?;
        let title = detail.summary.title.clone();
        let sections = detail
            .chunks
            .iter()
            .map(
                |chunk| match chunk.heading_path.as_deref().filter(|heading| !heading.is_empty()) {
                    Some(heading) => format!("{heading}\n{}", chunk.text),
                    None => chunk.text.clone(),
                },
            )
            .collect::<Vec<_>>();
        let total_chars = sections.iter().map(|section| section.chars().count()).sum::<usize>();
        let mut warnings = vec![
            "Generated by the configured AI provider. Verify cited source text before relying on the summary."
                .to_owned(),
        ];
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
                commit: None,
            })
            .collect();
        Ok(SummaryResult {
            summary_markdown,
            citations,
            warnings,
        })
    }

    pub async fn summarize_workspace(&self, workspace_id: String, provider_id: String) -> Result<SummaryResult> {
        let settings = self.storage.get_provider_settings(&provider_id).await?;
        if !settings.enabled {
            repomemo_domain::bail_invalid!("Enable this provider before using AI. No content was sent.");
        }
        if settings.workspace_id.as_deref() != Some(workspace_id.as_str()) {
            repomemo_domain::bail_invalid!("The selected provider belongs to a different workspace.");
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
                    commit: None,
                });
            }
        }
        if context_sections.is_empty() {
            repomemo_domain::bail_invalid!(
                "Index at least one artifact before requesting a workspace summary so RepoMemo can cite its content."
            );
        }
        let provider = provider_from_settings(settings)?;
        let summary_markdown = provider.generate(GenerateRequest {
            system: Some("You brief engineers joining a project, using only the supplied excerpts from its files. Never add facts that are not in them.".to_owned()),
            prompt: "Summarize this workspace for an engineer joining the project. Cover the major components, important behavior, and any notable gaps. Use only the supplied local context.".to_owned(),
            context: context_sections.join("\n\n"),
            options: json!({ "temperature": 0.2 }),
        }).await?;
        Ok(SummaryResult {
            summary_markdown,
            citations,
            warnings: vec![
                "This overview is based on the cited indexed excerpts, not necessarily every file in the workspace."
                    .to_owned(),
            ],
        })
    }

    pub async fn app_settings(&self) -> Result<AppSettings> {
        let (ai_enabled, active_provider) = self.storage.app_ai_status().await?;
        Ok(AppSettings {
            data_dir: self.storage.data_dir().display().to_string(),
            ai_enabled,
            active_provider,
        })
    }

    pub async fn embed_workspace(&self, workspace_id: String, provider_id: String) -> Result<IndexingJobStatus> {
        let settings = self.storage.get_provider_settings(&provider_id).await?;
        if !settings.enabled {
            repomemo_domain::bail_invalid!("Enable this provider before building embeddings.");
        }
        if settings.workspace_id.as_deref() != Some(workspace_id.as_str()) {
            repomemo_domain::bail_invalid!("The selected provider belongs to a different workspace.");
        }
        let chunks = self.storage.list_workspace_chunks(&workspace_id).await?;
        if chunks.is_empty() {
            repomemo_domain::bail_invalid!("Index artifacts before building embeddings.");
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
                .embed(batch.iter().map(|chunk| chunk.text.clone()).collect(), json!({}))
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
        self.ask_workspace_in_conversation(request, &[]).await
    }

    /// Ask with the earlier turns of a conversation (already fitted to the
    /// token budget). They are searched with, so a follow-up finds what it is
    /// about, and shown to the model only to resolve what "it" and "that file"
    /// mean; the answer still rests on the numbered passages.
    pub async fn ask_workspace_in_conversation(
        &self,
        request: AskRequest,
        history: &[AgentHistoryTurn],
    ) -> Result<AskAnswer> {
        if request.workspace_id.trim().is_empty() || request.question.trim().is_empty() {
            repomemo_domain::bail_invalid!("A workspace and question are required.");
        }
        let provider_id = request
            .provider_id
            .as_deref()
            .context("Enable a provider before using Ask.")?;
        let settings = self.storage.get_provider_settings(provider_id).await?;
        if !settings.enabled {
            repomemo_domain::bail_invalid!("Enable this provider before using Ask. No content was sent.");
        }
        if settings.workspace_id.as_deref() != Some(request.workspace_id.as_str()) {
            repomemo_domain::bail_invalid!("The selected provider belongs to a different workspace.");
        }
        let provider = provider_from_settings(settings)?;
        let search_query = agent::follow_up_query(&request.question, history);
        let mut warnings = Vec::new();
        let semantic_configured = self
            .embedding_provider_for_workspace(&request.workspace_id)
            .await?
            .is_some();
        let query_embedding = match self
            .query_embedding(&request.workspace_id, Some(provider_id), &search_query)
            .await
        {
            Ok(embedding) => embedding,
            Err(error) => {
                warnings.push(format!(
                    "Search by meaning was skipped because the embedding provider failed: {error:#}"
                ));
                None
            }
        };
        let profile = self.ranking_profile(&request.workspace_id).await;
        let (candidates, used_embeddings) = self
            .retrieval
            .hybrid_search(
                SearchRequest {
                    workspace_id: request.workspace_id.clone(),
                    query: search_query.clone(),
                    artifact_types: Vec::new(),
                    languages: Vec::new(),
                    source_ids: Vec::new(),
                    limit: Some(ASK_CANDIDATES),
                    ..Default::default()
                },
                KeywordMode::AnyTerm,
                query_embedding
                    .as_ref()
                    .map(|(model, vector)| (model.as_str(), vector.as_slice())),
                &profile,
            )
            .await?;
        if candidates.is_empty() {
            return Ok(AskAnswer {
                answer_markdown: "Indexed context is insufficient for a reliable answer.".to_owned(),
                citations: Vec::new(),
                retrieved_context: candidates,
                confidence: Some(0.0),
                warnings: vec![
                    "No matching indexed context was found; no provider generation was requested.".to_owned(),
                ],
            });
        }

        // Search results carry short snippets; the model needs the passages.
        let texts = self
            .storage
            .chunk_texts(
                &candidates
                    .iter()
                    .map(|result| result.chunk_id.clone())
                    .collect::<Vec<_>>(),
            )
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
            match provider.rerank(search_query.clone(), listing).await {
                Ok(order) => {
                    let mut slots = ranked.into_iter().map(Some).collect::<Vec<_>>();
                    ranked = order
                        .into_iter()
                        .filter_map(|index| slots.get_mut(index).and_then(Option::take))
                        .collect();
                }
                Err(error) => warnings.push(format!(
                    "Results were not reranked because the AI provider failed: {error:#}"
                )),
            }
        }
        ranked.truncate(request.limit.map_or(ASK_CONTEXT_PASSAGES, |limit| {
            (limit.max(1) as usize).min(ASK_CONTEXT_PASSAGES)
        }));
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
                system: Some("You answer questions about a workspace of technical material using only the numbered passages supplied. Cite the passages you rely on as [1], [2] and so on. If the passages do not answer the question, say that the indexed context is insufficient instead of guessing. Earlier turns of the conversation may be shown before the question; they only tell what words such as \"it\" or \"that file\" refer to and are never evidence.".to_owned()),
                prompt: format!("{}Question: {}", agent::render_history(history), request.question),
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
                commit: result.commit.clone(),
            })
            .collect::<Vec<_>>();
        let confidence = retrieved_context.first().map(|result| result.score.clamp(0.0, 1.0));
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
            match self.vision_provider_for_workspace(&summary.workspace_id).await? {
                Some(settings) => {
                    if !self.ai_budget.try_spend(&summary.workspace_id, 1) {
                        bail!(
                            "This workspace reached its daily limit of {} background AI calls. Images are described again tomorrow.",
                            self.ai_budget.daily_limit()
                        );
                    }
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
    async fn vision_provider_for_workspace(&self, workspace_id: &str) -> Result<Option<ProviderSettings>> {
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
        let is_image = matches!(detect_artifact_type(Path::new(filename)), Some(ArtifactType::Image));
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
            repomemo_domain::bail_invalid!(
                "An image AI provider is required to upload images. An administrator can set one up in Settings."
            );
        }
        Ok(())
    }
}

struct ArtifactIndexResult {
    stage: String,
}

/// One artifact that has been checked and has a running job, but is not
/// indexed yet: what a caller holds between `begin_index_artifact` and
/// `finish_index_artifact` (an HTTP handler answers `202` in between).
#[derive(Clone)]
pub struct PendingArtifactIndex {
    pub job: IndexingJobStatus,
    summary: ArtifactSummary,
}

impl PendingArtifactIndex {
    pub fn summary(&self) -> &ArtifactSummary {
        &self.summary
    }
}

/// A workspace re-index that has found what is stale and has a running job.
pub struct PendingWorkspaceIndex {
    pub job: IndexingJobStatus,
    artifacts: Vec<ArtifactSummary>,
}

/// A document's text as indexing stored it, in order, for the artifact page.
/// `None` when the file has no stored passages to trust (not indexed yet, or
/// changed since: a new version clears `indexed_at`); `Some(None)` when it was
/// indexed and has no text (a scanned PDF).
fn stored_document_text(detail: &ArtifactDetail) -> Option<Option<String>> {
    detail.summary.indexed_at.as_ref()?;
    let mut text = String::new();
    for chunk in &detail.chunks {
        if text.len() > DOCUMENT_PREVIEW_CHARS {
            break;
        }
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&chunk.text);
    }
    Some((!text.is_empty()).then_some(text))
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
        repomemo_domain::bail_invalid!("Memory card title is required.");
    }
    if body_markdown.trim().is_empty() {
        repomemo_domain::bail_invalid!("Memory card body is required.");
    }
    if source.trim().is_empty() {
        repomemo_domain::bail_invalid!("Memory card source is required.");
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
    let slug = slug.trim_matches(|c: char| c == '-' || c == '_').to_string();
    let slug = if slug.is_empty() {
        "pasted-note".to_owned()
    } else {
        slug.to_ascii_lowercase()
    };
    format!("{slug}.{extension}")
}

#[cfg(test)]
mod tests {
    use super::{evenly_spaced, group_sections, read_archive_entries, recent_readings_of, RepoMemoCore};
    use repomemo_domain::{Citation, CreateMemoryCardRequest};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn long_files_are_split_into_bounded_parts() {
        let sections = ["aaaa", "bbbb", "cccccccccc", "dd"].map(str::to_owned);
        assert_eq!(group_sections(&sections, 8), vec![vec![0, 1], vec![2], vec![3]]);
        assert_eq!(evenly_spaced((0..10).collect(), 4), vec![0, 3, 6, 9]);
        assert_eq!(evenly_spaced(vec![1, 2], 4), vec![1, 2]);
    }

    /// Timings of the knowledge map and Workspace Health on a large workspace
    /// (V0.1.74 targets: under 2 s on first load). Slow, so on request only:
    /// `cargo test -p repomemo-api --release map_and_health_at_scale -- --ignored --nocapture`
    /// `REPOMEMO_PERF_PASSAGES` sets the size (default 50,000).
    #[tokio::test]
    #[ignore = "a performance run, not a check"]
    async fn map_and_health_at_scale() {
        use std::time::Instant;
        let passages: usize = std::env::var("REPOMEMO_PERF_PASSAGES")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(50_000);
        let per_file = 100;
        let dims = 384;
        let data_dir = std::env::temp_dir().join(format!(
            "repomemo-api-scale-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
        let workspace = core.create_workspace("Scale".to_owned()).await.unwrap();
        core.storage()
            .save_provider_settings(repomemo_domain::ProviderSettings {
                id: String::new(),
                workspace_id: Some(workspace.id.clone()),
                provider_type: "ollama".to_owned(),
                name: "Embeddings".to_owned(),
                base_url: Some("http://127.0.0.1:11434".to_owned()),
                model: None,
                embedding_model: Some("perf".to_owned()),
                enabled: true,
                metadata: serde_json::json!({ "purpose": "embedding" }),
                api_key: None,
            })
            .await
            .unwrap();
        let mut seed = 0x9e37_79b9_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % 10_000) as f32 / 10_000.0 - 0.5
        };
        let started = Instant::now();
        let mut batch = Vec::new();
        for file in 0..passages.div_ceil(per_file) {
            let count = per_file.min(passages - file * per_file);
            let text = (0..count)
                .map(|index| {
                    format!(
                        "## Section {index}

Document {file} section {index} covers topic{} and the deploy_{} function.
",
                        (file + index) % 97,
                        (file * 7 + index) % 211
                    )
                })
                .collect::<String>();
            let artifact = core
                .import_text(
                    workspace.id.clone(),
                    format!("Document {file}"),
                    text,
                    Some("Markdown".to_owned()),
                )
                .await
                .unwrap();
            core.index_artifact(artifact.id.clone()).await.unwrap();
            for chunk in core.storage().list_chunks_for_artifact(&artifact.id).await.unwrap() {
                batch.push((chunk.id, (0..dims).map(|_| next()).collect::<Vec<f32>>()));
            }
            if batch.len() >= 2_000 {
                core.storage()
                    .upsert_embeddings(&workspace.id, "perf", std::mem::take(&mut batch))
                    .await
                    .unwrap();
            }
        }
        core.storage()
            .upsert_embeddings(&workspace.id, "perf", batch)
            .await
            .unwrap();
        println!("built {passages} passages in {:?}", started.elapsed());

        let mut full_run = std::time::Duration::ZERO;
        for round in ["first", "second"] {
            let started = Instant::now();
            let map = core.knowledge_map(&workspace.id).await.unwrap();
            println!(
                "knowledge map, {round}: {:?} ({} nodes, {} edges)",
                started.elapsed(),
                map.nodes.len(),
                map.edges.len()
            );
            if round == "first" {
                full_run = started.elapsed();
            }
            let started = Instant::now();
            let health = core.workspace_health(&workspace.id).await.unwrap();
            println!(
                "health, {round}: {:?} ({} findings)",
                started.elapsed(),
                health.findings.len()
            );
            if round == "first" {
                full_run += started.elapsed();
            }
        }

        // One file changes: its passages are re-indexed and re-embedded, and the
        // map and health are asked for again. The target is under a tenth of
        // the first, full run (V0.2.8).
        let listed = core.storage().list_artifacts(&workspace.id).await.unwrap();
        let changed = &listed[listed.len() / 2];
        let chunks = core.storage().list_chunks_for_artifact(&changed.id).await.unwrap();
        core.storage()
            .upsert_embeddings(
                &workspace.id,
                "perf",
                chunks
                    .into_iter()
                    .map(|chunk| (chunk.id, (0..dims).map(|_| next()).collect::<Vec<f32>>()))
                    .collect(),
            )
            .await
            .unwrap();
        let started = Instant::now();
        core.knowledge_map(&workspace.id).await.unwrap();
        core.workspace_health(&workspace.id).await.unwrap();
        let incremental = started.elapsed();
        println!(
            "map and health after one changed file: {incremental:?} ({:.1}% of the full run {full_run:?})",
            incremental.as_secs_f64() / full_run.as_secs_f64() * 100.0
        );
        assert!(
            incremental * 10 < full_run,
            "a one-file change must cost under 10% of a full run"
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    /// The knowledge map and Workspace Health keep per-file results between
    /// requests (roadmap P11). Whatever was kept, they must answer exactly what
    /// a cold computation answers, and a one-file change must read one file.
    #[tokio::test]
    async fn the_map_and_health_after_a_change_match_a_cold_computation() {
        let data_dir = std::env::temp_dir().join(format!(
            "repomemo-incremental-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
        let workspace = core.create_workspace("Incremental".to_owned()).await.unwrap();
        core.storage()
            .save_provider_settings(repomemo_domain::ProviderSettings {
                id: String::new(),
                workspace_id: Some(workspace.id.clone()),
                provider_type: "ollama".to_owned(),
                name: "Embeddings".to_owned(),
                base_url: Some("http://127.0.0.1:11434".to_owned()),
                model: None,
                embedding_model: Some("test-embed".to_owned()),
                enabled: true,
                metadata: serde_json::json!({ "purpose": "embedding" }),
                api_key: None,
            })
            .await
            .unwrap();
        // Twelve files in three topics, each embedded along its topic with a
        // small per-file variation.
        let vector = |file: usize, wobble: f32| -> Vec<f32> {
            let topic = file % 3;
            let mut vector = vec![0.05_f32; 6];
            vector[topic * 2] = 1.0;
            vector[topic * 2 + 1] = 0.3 + file as f32 * 0.02 + wobble;
            vector
        };
        let mut artifacts = Vec::new();
        for file in 0..12 {
            let artifact = core
                .import_text(
                    workspace.id.clone(),
                    format!("Note {file}"),
                    format!(
                        "# Note {file}\n\nTopic {} paragraph {file}.\n\n## More\n\nSecond part of note {file}.",
                        file % 3
                    ),
                    Some("Markdown".to_owned()),
                )
                .await
                .unwrap();
            core.index_artifact(artifact.id.clone()).await.unwrap();
            let chunks = core.storage().list_chunks_for_artifact(&artifact.id).await.unwrap();
            core.storage()
                .upsert_embeddings(
                    &workspace.id,
                    "test-embed",
                    chunks.into_iter().map(|chunk| (chunk.id, vector(file, 0.0))).collect(),
                )
                .await
                .unwrap();
            artifacts.push(artifact);
        }
        let ws = workspace.id.as_str();

        let first_map = serde_json::to_value(core.knowledge_map(ws).await.unwrap()).unwrap();
        let first_health = serde_json::to_value(core.workspace_health(ws).await.unwrap()).unwrap();
        assert_eq!(first_map["similarity_available"], true);
        // Nothing changed: the next request reads no file again.
        let listed = core.storage().list_artifacts(ws).await.unwrap();
        assert_eq!(
            core.file_meanings(ws, Some("test-embed"), &listed)
                .await
                .unwrap()
                .refreshed,
            0
        );

        // One file gets new vectors (it moved to another topic).
        let moved = &artifacts[4];
        let chunks = core.storage().list_chunks_for_artifact(&moved.id).await.unwrap();
        core.storage()
            .upsert_embeddings(
                ws,
                "test-embed",
                chunks
                    .into_iter()
                    .map(|chunk| {
                        let mut other = vec![0.05_f32; 6];
                        other[4] = 1.0;
                        other[5] = 0.9;
                        (chunk.id, other)
                    })
                    .collect(),
            )
            .await
            .unwrap();
        let listed = core.storage().list_artifacts(ws).await.unwrap();
        // Exactly the one file written is read again, and then it is current.
        assert_eq!(
            core.file_meanings(ws, Some("test-embed"), &listed)
                .await
                .unwrap()
                .refreshed,
            1
        );
        assert_eq!(
            core.file_meanings(ws, Some("test-embed"), &listed)
                .await
                .unwrap()
                .refreshed,
            0,
            "and then it is current"
        );

        let warm_map = serde_json::to_value(core.knowledge_map(ws).await.unwrap()).unwrap();
        let warm_health = serde_json::to_value(core.workspace_health(ws).await.unwrap()).unwrap();
        core.meaning.clear();
        let cold_map = serde_json::to_value(core.knowledge_map(ws).await.unwrap()).unwrap();
        let cold_health = serde_json::to_value(core.workspace_health(ws).await.unwrap()).unwrap();
        assert_eq!(
            warm_map, cold_map,
            "the map after a change is the map of a cold computation"
        );
        assert_eq!(warm_health, cold_health);
        assert_ne!(
            warm_map["edges"], first_map["edges"],
            "the moved file is linked elsewhere now"
        );
        let _ = first_health;

        // A new file is read, and only it.
        let extra = core
            .import_text(
                ws.to_owned(),
                "Late note".to_owned(),
                "# Late\n\nTopic 1.".to_owned(),
                None,
            )
            .await
            .unwrap();
        core.index_artifact(extra.id.clone()).await.unwrap();
        let listed = core.storage().list_artifacts(ws).await.unwrap();
        let meanings = core.file_meanings(ws, Some("test-embed"), &listed).await.unwrap();
        assert_eq!(meanings.refreshed, 1, "only the new file was read");
        assert_eq!(meanings.counts[&extra.id].0, 1, "its passages are counted");

        drop(core);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn memory_card_export_includes_the_linked_evidence() {
        let data_dir = std::env::temp_dir().join(format!(
            "repomemo-memory-export-test-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
        let workspace = core.create_workspace("Memory export".to_owned()).await.unwrap();
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
                    commit: None,
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
    async fn a_local_workspace_is_written_as_an_archive_with_its_memory_and_citations() {
        let data_dir = std::env::temp_dir().join(format!(
            "repomemo-local-archive-test-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
        let workspace = core.create_workspace("Local notes".to_owned()).await.unwrap();
        let artifact = core
            .import_text(
                workspace.id.clone(),
                "Decision note".to_owned(),
                "Keep the evidence local.".to_owned(),
                Some("Markdown".to_owned()),
            )
            .await
            .unwrap();
        core.index_workspace(workspace.id.clone()).await.unwrap();
        let chunk_id = core.get_artifact(artifact.id.clone()).await.unwrap().chunks[0]
            .id
            .clone();
        core.create_memory_card(CreateMemoryCardRequest {
            workspace_id: workspace.id.clone(),
            title: "Local-first decision".to_owned(),
            body_markdown: "The source of truth remains local.".to_owned(),
            source: "manual".to_owned(),
            confidence: None,
            citations: vec![Citation {
                artifact_id: artifact.id.clone(),
                chunk_id: Some(chunk_id),
                title: artifact.title.clone(),
                path: artifact.path.clone(),
                start_line: None,
                end_line: None,
                confidence: None,
                commit: None,
            }],
        })
        .await
        .unwrap();

        let exports = data_dir.join("exports");
        std::fs::create_dir_all(&exports).unwrap();
        let destination = exports.join("local-notes.tar.gz");
        let manifest = core
            .export_workspace_archive(&workspace.id, &destination)
            .await
            .unwrap();
        assert!(destination.is_file());
        assert!(
            !destination.with_extension("part").exists(),
            "no half-written file is left"
        );
        assert_eq!(manifest.workspace_name, "Local notes");
        assert_eq!(manifest.counts.artifacts, 1);
        assert_eq!(manifest.counts.blobs, 1);
        assert_eq!(manifest.counts.memory_cards, 1);
        assert_eq!(manifest.counts.citations, 1);

        // What the server reads: the manifest, the data with its checksum, and the stored file under its hash.
        let entries: std::collections::BTreeMap<String, Vec<u8>> =
            read_archive_entries(std::fs::File::open(&destination).unwrap(), 1 << 26)
                .unwrap()
                .into_iter()
                .collect();
        assert_eq!(entries.len(), 3);
        let written: repomemo_domain::ArchiveManifest = serde_json::from_slice(&entries["manifest.json"]).unwrap();
        assert_eq!(written.format, repomemo_domain::ARCHIVE_FORMAT);
        assert_eq!(
            repomemo_storage::StorageEngine::content_hash(&entries["workspace.json"]),
            written.data_sha256
        );
        let blob = &written.blobs[0];
        assert_eq!(entries[&format!("blobs/{}", blob.hash)], b"Keep the evidence local.");
        let data: repomemo_domain::WorkspaceArchiveData = serde_json::from_slice(&entries["workspace.json"]).unwrap();
        assert_eq!(data.memory_cards[0].title, "Local-first decision");
        assert!(
            data.memory_cards[0].citations[0].chunk.is_some(),
            "the cited passage is named"
        );

        drop(core);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn a_workspace_missing_stored_content_is_not_written_and_leaves_nothing_behind() {
        let data_dir = std::env::temp_dir().join(format!(
            "repomemo-local-archive-missing-test-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
        let workspace = core.create_workspace("Damaged".to_owned()).await.unwrap();
        let artifact = core
            .import_text(
                workspace.id.clone(),
                "Note".to_owned(),
                "Some text.".to_owned(),
                Some("Markdown".to_owned()),
            )
            .await
            .unwrap();
        let hash = core
            .get_artifact(artifact.id.clone())
            .await
            .unwrap()
            .summary
            .content_hash;
        std::fs::remove_file(core.storage().blob_path_for_hash(&hash)).unwrap();

        let destination = data_dir.join("damaged.tar.gz");
        let error = core
            .export_workspace_archive(&workspace.id, &destination)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("no stored content"), "{error}");
        assert!(!destination.exists());
        assert!(!destination.with_extension("part").exists());
        assert!(core
            .export_workspace_archive("no-such-workspace", &destination)
            .await
            .is_err());

        drop(core);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn workspace_indexing_skips_artifacts_that_are_already_current() {
        let data_dir = std::env::temp_dir().join(format!(
            "repomemo-incremental-index-test-{}",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
        let workspace = core.create_workspace("Incremental".to_owned()).await.unwrap();
        let first = core
            .import_text(
                workspace.id.clone(),
                "First".to_owned(),
                "# First
One."
                    .to_owned(),
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
        assert_eq!(chunk_ids(core.get_artifact(first.id).await.unwrap()), indexed_ids);

        drop(core);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    /// V0.2.150: the artifact page of an indexed document shows the stored
    /// passages and starts no reader; one not indexed yet is read once.
    #[tokio::test]
    async fn the_page_of_an_indexed_document_starts_no_reader() {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let data_dir = std::env::temp_dir().join(format!("repomemo-page-readings-{nanos}"));
        let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
        let workspace = core.create_workspace("Readings".to_owned()).await.unwrap();
        // A name no other test uses, so only this test's readings are counted.
        let name = format!("page-{nanos}.eml");
        let email = "From: Ada <ada@example.com>\r\nTo: bob@example.com\r\nSubject: Plan\r\nContent-Type: text/plain\r\n\r\nPlease review the plan.\r\n";
        let artifact = core
            .import_upload(workspace.id.clone(), name.clone(), email.as_bytes().to_vec(), None)
            .await
            .unwrap();

        // Not indexed yet: the file is read for the page, once.
        let before = recent_readings_of(&name);
        let page = core.get_artifact(artifact.id.clone()).await.unwrap();
        assert!(page
            .content_preview
            .as_deref()
            .is_some_and(|text| text.contains("Please review the plan")));
        assert_eq!(recent_readings_of(&name), before + 1);

        // Indexed: the page is built from the stored passages.
        core.index_workspace(workspace.id.clone()).await.unwrap();
        let indexed = core.get_artifact(artifact.id.clone()).await.unwrap();
        assert!(!indexed.chunks.is_empty());
        let before = recent_readings_of(&name);
        let page = core.get_artifact(artifact.id.clone()).await.unwrap();
        assert_eq!(
            recent_readings_of(&name),
            before,
            "an indexed document is not read again"
        );
        assert!(page
            .content_preview
            .as_deref()
            .is_some_and(|text| text.contains("Please review the plan")));
        assert!(!page.content_truncated);

        // A new version clears the index, so the page reads the new file.
        let changed = "From: Ada <ada@example.com>\r\nTo: bob@example.com\r\nSubject: Plan\r\nContent-Type: text/plain\r\n\r\nA different plan.\r\n";
        core.replace_artifact_version(&artifact.id, &name, changed.as_bytes().to_vec(), None)
            .await
            .unwrap();
        let page = core.get_artifact(artifact.id).await.unwrap();
        assert!(page
            .content_preview
            .as_deref()
            .is_some_and(|text| text.contains("A different plan")));
        assert_eq!(recent_readings_of(&name), before + 1);

        drop(core);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
