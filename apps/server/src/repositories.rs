//! Git repositories connected to shared workspaces.
//!
//! The sync engine lives in `RepoMemoCore`; this layer decides who may connect
//! and sync what, and runs syncs in the background so a request never waits on
//! a large repository. A server only reads repositories under the folders an
//! operator listed in `REPOMEMO_REPO_ROOTS`; without it, the feature is off,
//! because a workspace administrator must not be able to read arbitrary
//! folders on the server's disk.

use std::{
    collections::HashSet,
    path::{Path as FsPath, PathBuf},
    sync::{Arc, Mutex},
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_api::RepoMemoCore;
use repomemo_domain::{
    IndexingJobStatus, RepoFile, RepoSettings, RepoSource, RepoSyncReport, WorkspaceRole,
    REPO_SYNC_JOB_KIND,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::{
    embedding::EmbeddingQueue, record_workspace_activity, require_workspace_admin,
    require_workspace_read, require_workspace_write, ApiError, AppState, AuthenticatedSubject,
};

/// Reads the allowed repository folders from `REPOMEMO_REPO_ROOTS`, a list in
/// the platform's PATH format (`;` on Windows, `:` elsewhere).
pub(crate) fn repo_roots_from_env() -> Vec<PathBuf> {
    std::env::var_os("REPOMEMO_REPO_ROOTS")
        .map(|value| {
            std::env::split_paths(&value)
                .filter(|path| !path.as_os_str().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Runs repository syncs in the background, one at a time across the server,
/// and never two for the same repository.
#[derive(Clone)]
pub(crate) struct RepoSyncRunner {
    core: RepoMemoCore,
    embeddings: EmbeddingQueue,
    running: Arc<Mutex<HashSet<String>>>,
    permits: Arc<Semaphore>,
    /// Canonical forms of the configured roots, paired with how they were
    /// written, for messages.
    roots: Arc<Vec<(PathBuf, String)>>,
}

impl RepoSyncRunner {
    pub(crate) fn new(core: RepoMemoCore, embeddings: EmbeddingQueue, roots: Vec<PathBuf>) -> Self {
        let roots = roots
            .into_iter()
            .filter_map(|root| match std::fs::canonicalize(&root) {
                Ok(canonical) => Some((canonical, root.display().to_string())),
                Err(error) => {
                    tracing::warn!(root = %root.display(), error = %error, "Ignoring a repository root that cannot be read");
                    None
                }
            })
            .collect();
        Self {
            core,
            embeddings,
            running: Arc::new(Mutex::new(HashSet::new())),
            permits: Arc::new(Semaphore::new(1)),
            roots: Arc::new(roots),
        }
    }

    fn enabled(&self) -> bool {
        !self.roots.is_empty()
    }

    fn root_labels(&self) -> Vec<String> {
        self.roots.iter().map(|(_, label)| label.clone()).collect()
    }

    fn is_allowed(&self, path: &FsPath) -> bool {
        match std::fs::canonicalize(path) {
            Ok(path) => self.roots.iter().any(|(root, _)| path.starts_with(root)),
            Err(_) => false,
        }
    }

    fn ensure_allowed(&self, path: &FsPath) -> Result<(), ApiError> {
        if !self.enabled() {
            return Err(ApiError::bad_request(
                "Local repositories are turned off on this server. An operator can allow them by setting REPOMEMO_REPO_ROOTS to the folders that hold repositories.",
            ));
        }
        if !path.is_dir() {
            return Err(ApiError::bad_request(format!(
                "The folder {} does not exist on the server.",
                path.display()
            )));
        }
        if !self.is_allowed(path) {
            return Err(ApiError::bad_request(format!(
                "{} is outside the folders this server may read repositories from: {}.",
                path.display(),
                self.root_labels().join(", ")
            )));
        }
        Ok(())
    }

    /// Queues a sync. Returns `None` when one is already running for the
    /// repository.
    pub(crate) async fn start(
        &self,
        source: &RepoSource,
        actor_user_id: Option<String>,
    ) -> anyhow::Result<Option<IndexingJobStatus>> {
        let newly_running = self
            .running
            .lock()
            .map(|mut running| running.insert(source.id.clone()))
            .unwrap_or(false);
        if !newly_running {
            return Ok(None);
        }
        let job = match self.core.begin_repo_sync(&source.id).await {
            Ok(job) => job,
            Err(error) => {
                self.forget(&source.id);
                return Err(error);
            }
        };

        let runner = self.clone();
        let source = source.clone();
        let job_id = job.id.clone();
        tokio::spawn(async move {
            let permit = runner.permits.clone().acquire_owned().await;
            let result = runner.core.run_repo_sync(&source.id, &job_id).await;
            drop(permit);
            runner.forget(&source.id);
            runner.finish(&source, actor_user_id.as_deref(), result).await;
        });
        Ok(Some(job))
    }

    async fn finish(
        &self,
        source: &RepoSource,
        actor_user_id: Option<&str>,
        result: anyhow::Result<RepoSyncReport>,
    ) {
        let (action, summary) = match result {
            Ok(report) => {
                if report.indexed > 0 || report.removed > 0 {
                    self.embeddings.request(&source.workspace_id);
                }
                let changed = report.added + report.updated + report.renamed + report.removed + report.restored;
                // A background sync that found nothing new is not news.
                if changed == 0 && actor_user_id.is_none() && !report.cancelled {
                    return;
                }
                let short = &report.commit_sha[..report.commit_sha.len().min(7)];
                let verb = if report.cancelled { "Stopped syncing" } else { "Synced" };
                (
                    "repository_synced",
                    format!(
                        "{verb} repository {} at {short}: {} added, {} updated, {} renamed, {} removed.",
                        source.name, report.added + report.restored, report.updated, report.renamed, report.removed
                    ),
                )
            }
            Err(error) => {
                tracing::warn!(repository = %source.name, error = %format!("{error:#}"), "Repository sync failed");
                ("repository_sync_failed", format!("Syncing repository {} failed.", source.name))
            }
        };
        if let Err(error) = self
            .core
            .storage()
            .record_workspace_activity(
                &source.workspace_id,
                actor_user_id,
                action,
                "repository",
                Some(&source.id),
                &summary,
            )
            .await
        {
            tracing::error!(error = %error, "Failed to record repository activity");
        }
    }

    fn forget(&self, source_id: &str) {
        if let Ok(mut running) = self.running.lock() {
            running.remove(source_id);
        }
    }

    /// After a restart: fails the syncs the previous process left running and
    /// syncs every repository, which also picks up commits made while the
    /// server was down.
    pub(crate) async fn resume_all(&self) {
        if let Err(error) = self
            .core
            .storage()
            .fail_interrupted_jobs(REPO_SYNC_JOB_KIND, "Interrupted by a server restart.")
            .await
        {
            tracing::error!(error = %error, "Failed to close interrupted repository syncs");
        }
        let source_ids = match self.core.list_all_repo_source_ids().await {
            Ok(ids) => ids,
            Err(error) => {
                tracing::error!(error = %error, "Failed to list repositories to sync");
                return;
            }
        };
        for source_id in source_ids {
            let Ok(source) = self.core.repo_source(&source_id).await else {
                continue;
            };
            if !self.is_allowed(FsPath::new(&source.root_path)) {
                tracing::warn!(repository = %source.name, "Not syncing a repository outside REPOMEMO_REPO_ROOTS");
                continue;
            }
            if let Err(error) = self.start(&source, None).await {
                tracing::error!(repository = %source.name, error = %error, "Failed to queue repository sync");
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct RepositoriesResponse {
    local_repositories_enabled: bool,
    /// The folders repositories may be connected from; shown to the members
    /// who can connect one.
    allowed_roots: Vec<String>,
    repositories: Vec<RepoSource>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConnectRepositoryRequest {
    path: String,
    name: Option<String>,
    branch: Option<String>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateRepositoryRequest {
    name: Option<String>,
    branch: Option<String>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RepositorySyncResponse {
    repository: RepoSource,
    job: Option<IndexingJobStatus>,
}

pub(crate) async fn list_repositories(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<RepositoriesResponse>, ApiError> {
    let role = require_workspace_read(&state, &subject, &workspace_id).await?;
    let repositories = state
        .core
        .list_repo_sources(&workspace_id)
        .await
        .map_err(map_repo_error)?;
    let can_connect = matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin);
    Ok(Json(RepositoriesResponse {
        local_repositories_enabled: state.repo_sync.enabled(),
        allowed_roots: if can_connect { state.repo_sync.root_labels() } else { Vec::new() },
        repositories,
    }))
}

pub(crate) async fn connect_repository(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<ConnectRepositoryRequest>,
) -> Result<(StatusCode, Json<RepositorySyncResponse>), ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    // "Copy as path" in Windows Explorer wraps the path in quotes.
    let path = request.path.trim().trim_matches('"').trim();
    if path.is_empty() {
        return Err(ApiError::bad_request("Enter the folder of a git repository."));
    }
    // Check the folder before running git in it, then the repository root
    // git reports, which may be a parent of the folder.
    let path = PathBuf::from(path);
    state.repo_sync.ensure_allowed(&path)?;
    let root = state
        .core
        .resolve_repo_root(&path)
        .await
        .map_err(map_repo_error)?;
    state.repo_sync.ensure_allowed(&root)?;

    let repository = state
        .core
        .connect_local_repo(
            &workspace_id,
            &root,
            request.name,
            RepoSettings {
                branch: request.branch,
                include: request.include,
                exclude: request.exclude,
            },
        )
        .await
        .map_err(map_repo_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "repository_connected",
        "repository",
        Some(&repository.id),
        format!("Connected repository {}.", repository.name),
    )
    .await;
    let job = state
        .repo_sync
        .start(&repository, Some(subject.user_id.clone()))
        .await
        .map_err(map_repo_error)?;
    let repository = state.core.repo_source(&repository.id).await.map_err(map_repo_error)?;
    Ok((StatusCode::CREATED, Json(RepositorySyncResponse { repository, job })))
}

pub(crate) async fn get_repository(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<Json<RepoSource>, ApiError> {
    let repository = load_repository(&state, &source_id).await?;
    require_workspace_read(&state, &subject, &repository.workspace_id).await?;
    Ok(Json(repository))
}

pub(crate) async fn update_repository(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Json(request): Json<UpdateRepositoryRequest>,
) -> Result<Json<RepoSource>, ApiError> {
    let repository = load_repository(&state, &source_id).await?;
    require_workspace_admin(&state, &subject, &repository.workspace_id).await?;
    let updated = state
        .core
        .update_repo_source(
            &source_id,
            request.name,
            RepoSettings {
                branch: request.branch,
                include: request.include,
                exclude: request.exclude,
            },
        )
        .await
        .map_err(map_repo_error)?;
    record_workspace_activity(
        &state,
        &repository.workspace_id,
        &subject.user_id,
        "repository_updated",
        "repository",
        Some(&source_id),
        format!("Changed the settings of repository {}.", updated.name),
    )
    .await;
    Ok(Json(updated))
}

pub(crate) async fn delete_repository(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let repository = load_repository(&state, &source_id).await?;
    require_workspace_admin(&state, &subject, &repository.workspace_id).await?;
    if repository.active_job.is_some() {
        return Err(ApiError::conflict(
            "This repository is syncing. Stop the sync, or wait for it to finish, before removing it.",
        ));
    }
    state
        .core
        .remove_repo_source(&source_id)
        .await
        .map_err(map_repo_error)?;
    record_workspace_activity(
        &state,
        &repository.workspace_id,
        &subject.user_id,
        "repository_removed",
        "repository",
        Some(&source_id),
        format!("Removed repository {} and its {} files.", repository.name, repository.file_count),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn sync_repository(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<Json<RepositorySyncResponse>, ApiError> {
    let repository = load_repository(&state, &source_id).await?;
    require_workspace_write(&state, &subject, &repository.workspace_id).await?;
    state
        .repo_sync
        .ensure_allowed(FsPath::new(&repository.root_path))?;
    let job = state
        .repo_sync
        .start(&repository, Some(subject.user_id.clone()))
        .await
        .map_err(map_repo_error)?
        .ok_or_else(|| ApiError::conflict("This repository is already syncing."))?;
    let repository = state.core.repo_source(&source_id).await.map_err(map_repo_error)?;
    Ok(Json(RepositorySyncResponse {
        repository,
        job: Some(job),
    }))
}

pub(crate) async fn list_repository_files(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<Json<Vec<RepoFile>>, ApiError> {
    let repository = load_repository(&state, &source_id).await?;
    require_workspace_read(&state, &subject, &repository.workspace_id).await?;
    state
        .core
        .list_repo_files(&source_id)
        .await
        .map(Json)
        .map_err(map_repo_error)
}

/// Refuses edits that a repository sync would undo.
pub(crate) async fn ensure_not_repository_file(
    state: &AppState,
    artifact_id: &str,
) -> Result<(), ApiError> {
    let repository = state
        .storage
        .repo_name_for_artifact(artifact_id)
        .await
        .map_err(ApiError::internal)?;
    match repository {
        Some(name) => Err(ApiError::bad_request(format!(
            "This file comes from the repository {name}. Change it in the repository, or exclude its path in the repository settings."
        ))),
        None => Ok(()),
    }
}

async fn load_repository(state: &AppState, source_id: &str) -> Result<RepoSource, ApiError> {
    state.core.repo_source(source_id).await.map_err(map_repo_error)
}

/// Repository errors are mostly about the user's input (a folder that is not
/// a repository, a branch that does not exist), so their message is shown.
fn map_repo_error(error: anyhow::Error) -> ApiError {
    let message = format!("{error:#}");
    if message.contains("was not found") {
        ApiError::not_found(error.to_string())
    } else if message.contains("already connected") {
        ApiError::conflict(error.to_string())
    } else if message.contains("database") || message.contains("sqlx") {
        ApiError::internal(error)
    } else {
        ApiError::bad_request(message)
    }
}
