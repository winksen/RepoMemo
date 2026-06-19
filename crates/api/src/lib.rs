use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use repomemo_domain::{
    AppSettings, ArtifactDetail, ArtifactSummary, ImportReport, ImportRequest, IndexingJobStatus,
    SearchRequest, SearchResult, Workspace, WorkspaceOverview,
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

    pub fn app_settings(&self) -> AppSettings {
        AppSettings {
            data_dir: self.storage.data_dir().display().to_string(),
            ai_enabled: false,
            active_provider: None,
        }
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
            .replace_artifact_chunks(&summary.id, output.chunks)
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
