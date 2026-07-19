use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use repomemo_ai::{validate_settings, AiProvider, GenerateRequest, OllamaProvider};
use repomemo_domain::{
    AppSettings, ArtifactDetail, ArtifactSummary, ArtifactType, ImportReport, ImportRequest,
    IndexingJobStatus, ProviderSettings, ProviderTestResult, SearchRequest, SearchResult,
    SourceType, SummaryResult, Symbol, SymbolSearchResult, Workspace, WorkspaceOverview,
};
use repomemo_indexer::index_artifact;
use repomemo_ingestion::{discover_import_candidates, ImportCandidate, ImportOptions};
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

    pub async fn list_artifacts(&self, workspace_id: String) -> Result<Vec<ArtifactSummary>> {
        self.storage.list_artifacts(&workspace_id).await
    }

    pub async fn get_artifact(&self, artifact_id: String) -> Result<ArtifactDetail> {
        self.storage.get_artifact(&artifact_id).await
    }

    pub async fn index_artifact(&self, artifact_id: String) -> Result<IndexingJobStatus> {
        if artifact_id.trim().is_empty() {
            bail!("Artifact id is required.");
        }

        let summary = self.storage.get_artifact_summary(&artifact_id).await?;
        let job = self
            .storage
            .create_indexing_job(
                &summary.workspace_id,
                Some(&summary.source_id),
                "extracting_text",
                Some(1),
            )
            .await?;

        match self.index_artifact_inner(&summary).await {
            Ok(chunk_count) => {
                self.storage
                    .update_indexing_job(
                        &job.id,
                        "completed",
                        &format!("chunked_{chunk_count}_chunks"),
                        1,
                        None,
                    )
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
        let provider = OllamaProvider::from_settings(settings)?;
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
        let provider = OllamaProvider::from_settings(settings)?;
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

    pub async fn app_settings(&self) -> Result<AppSettings> {
        let (ai_enabled, active_provider) = self.storage.app_ai_status().await?;
        Ok(AppSettings {
            data_dir: self.storage.data_dir().display().to_string(),
            ai_enabled,
            active_provider,
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

    async fn index_artifact_inner(&self, summary: &ArtifactSummary) -> Result<usize> {
        let bytes = self.storage.read_artifact_blob(&summary.id).await?;
        let output = index_artifact(summary, &bytes)?;
        let chunk_count = output.chunks.len();

        self.storage
            .replace_artifact_index(&summary.id, output.chunks, output.symbols)
            .await?;

        Ok(chunk_count)
    }
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
