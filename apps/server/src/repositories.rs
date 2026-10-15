//! Git repositories connected to shared workspaces.
//!
//! Each workspace keeps its own repository links, set by its owners and
//! administrators in Settings. The sync engine lives in `RepoMemoCore`; this
//! layer decides who may link and sync what, checks that the server can read a
//! link before it is connected, and runs syncs in the background so a request
//! never waits on a large repository.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_api::RepoMemoCore;
use repomemo_domain::{
    IndexingJobStatus, ProviderSettings, RepoAccessCheck, RepoDetail, RepoFile, RepoSettings, RepoSource, RepoSummary,
    RepoSyncReport, REPO_SYNC_JOB_KIND,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::{
    ai_access, embedding::EmbeddingQueue, record_workspace_activity, require_workspace_admin, require_workspace_read,
    require_workspace_write, system::SettingsCell, ApiError, AppState, AuthenticatedSubject,
};

/// An automatic sync towards the same commit is not tried again sooner than
/// this, so a repository whose sync keeps failing is not retried every poll.
const AUTO_SYNC_RETRY_AFTER: Duration = Duration::from_secs(3600);

/// Runs repository syncs in the background, one at a time across the server,
/// and never two for the same repository.
#[derive(Clone)]
pub(crate) struct RepoSyncRunner {
    core: RepoMemoCore,
    embeddings: EmbeddingQueue,
    running: Arc<Mutex<HashSet<String>>>,
    permits: Arc<Semaphore>,
    /// The commit each repository was last synced towards automatically, and
    /// when.
    auto_attempts: Arc<Mutex<HashMap<String, (String, Instant)>>>,
    /// The run-time settings, for the storage allowance and disk floor.
    runtime: Arc<SettingsCell>,
}

impl RepoSyncRunner {
    pub(crate) fn new(core: RepoMemoCore, embeddings: EmbeddingQueue, runtime: Arc<SettingsCell>) -> Self {
        Self {
            runtime,
            core,
            embeddings,
            running: Arc::new(Mutex::new(HashSet::new())),
            permits: Arc::new(Semaphore::new(1)),
            auto_attempts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Syncs queued or running.
    pub(crate) fn running_count(&self) -> usize {
        self.running.lock().map(|running| running.len()).unwrap_or(0)
    }

    fn is_running(&self, source_id: &str) -> bool {
        self.running
            .lock()
            .map(|running| running.contains(source_id))
            .unwrap_or(true)
    }

    /// Checks every repository's branch each interval and syncs the ones
    /// that moved, so the workspace follows new commits without anyone
    /// pressing Sync now. `interval` is read before every round, so a system
    /// administrator can change it (or turn polling off, `None`) at run time.
    pub(crate) fn start_polling(
        &self,
        interval: impl Fn() -> Option<Duration> + Send + 'static,
    ) -> tokio::task::JoinHandle<()> {
        let runner = self.clone();
        tokio::spawn(async move {
            loop {
                let Some(wait) = interval() else {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    continue;
                };
                tokio::time::sleep(wait).await;
                if interval().is_none() {
                    continue;
                }
                let started = runner.poll_once().await;
                if started > 0 {
                    tracing::info!(
                        count = started,
                        "Started automatic repository syncs for branches that moved"
                    );
                }
            }
        })
    }

    /// One polling round. Returns how many syncs it started.
    pub(crate) async fn poll_once(&self) -> usize {
        let source_ids = match self.core.list_all_repo_source_ids().await {
            Ok(ids) => ids,
            Err(error) => {
                tracing::warn!(error = %error, "Failed to list repositories to poll");
                return 0;
            }
        };
        let mut started = 0;
        for source_id in source_ids {
            if self.is_running(&source_id) {
                continue;
            }
            let head = match self.core.repo_head(&source_id).await {
                Ok(head) => head,
                Err(error) => {
                    tracing::debug!(repository = %source_id, error = %format!("{error:#}"), "Could not read a repository's branch");
                    continue;
                }
            };
            if !head.moved() || head.last_synced.is_none() {
                // Never synced successfully: that needs a person to look.
                continue;
            }
            let recently_tried = self
                .auto_attempts
                .lock()
                .map(|attempts| {
                    attempts
                        .get(&source_id)
                        .is_some_and(|(commit, at)| commit == &head.current && at.elapsed() < AUTO_SYNC_RETRY_AFTER)
                })
                .unwrap_or(true);
            if recently_tried {
                continue;
            }
            let Ok(source) = self.core.repo_source(&source_id).await else {
                continue;
            };
            if let Ok(mut attempts) = self.auto_attempts.lock() {
                attempts.insert(source_id.clone(), (head.current.clone(), Instant::now()));
            }
            match self.start(&source, None).await {
                Ok(Some(_)) => started += 1,
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(repository = %source.name, error = %error, "Failed to queue an automatic repository sync");
                }
            }
        }
        started
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
        if let Err(reason) =
            crate::quotas::room_for_sync(self.core.storage(), &self.runtime.current(), &source.workspace_id).await
        {
            self.forget(&source.id);
            repomemo_domain::bail_conflict!("{reason}");
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

    async fn finish(&self, source: &RepoSource, actor_user_id: Option<&str>, result: anyhow::Result<RepoSyncReport>) {
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
                        source.name,
                        report.added + report.restored,
                        report.updated,
                        report.renamed,
                        report.removed
                    ),
                )
            }
            Err(error) => {
                tracing::warn!(repository = %source.name, error = %format!("{error:#}"), "Repository sync failed");
                (
                    "repository_sync_failed",
                    format!("Syncing repository {} failed.", source.name),
                )
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
            if let Err(error) = self.start(&source, None).await {
                tracing::error!(repository = %source.name, error = %error, "Failed to queue repository sync");
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct RepositoriesResponse {
    repositories: Vec<RepoSource>,
}

/// A token as it arrives: it prints as nothing, so a request that is logged
/// or put in an error cannot show it.
#[derive(Deserialize)]
#[serde(transparent)]
pub(crate) struct SecretText(String);

impl std::fmt::Debug for SecretText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<hidden>")
    }
}

/// A repository link with the settings it would be indexed with.
#[derive(Debug, Deserialize)]
pub(crate) struct RepositoryLinkRequest {
    /// A folder on the server, anywhere inside the repository, or the
    /// `https` address of a repository on a host.
    #[serde(alias = "path")]
    link: String,
    name: Option<String>,
    branch: Option<String>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    /// A read-only token for a private remote repository.
    #[serde(default)]
    token: Option<SecretText>,
    /// The account name that goes with the token, when the host wants one.
    #[serde(default)]
    token_username: Option<String>,
    /// A deploy key (private key text) for an `ssh` address.
    #[serde(default)]
    deploy_key: Option<SecretText>,
    /// The host's public key an `ssh` link is pinned to (`ssh-keyscan` output).
    #[serde(default)]
    host_key: Option<String>,
}

impl RepositoryLinkRequest {
    fn auth(&self) -> repomemo_api::RemoteAuth {
        repomemo_api::RemoteAuth {
            token: self
                .token
                .as_ref()
                .and_then(|token| repomemo_api::RemoteToken::new(self.token_username.clone(), token.0.clone())),
            deploy_key: self.deploy_key.as_ref().and_then(|key| {
                repomemo_api::RemoteDeployKey::new(key.0.clone(), self.host_key.clone().unwrap_or_default())
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct RepositoryDeployKeyRequest {
    deploy_key: SecretText,
    /// Leave out to keep the pinned host key the link has.
    #[serde(default)]
    host_key: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RepositoryTokenRequest {
    token: SecretText,
    #[serde(default)]
    username: Option<String>,
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
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let repositories = state
        .core
        .list_repo_sources(&workspace_id)
        .await
        .map_err(map_repo_error)?;
    Ok(Json(RepositoriesResponse { repositories }))
}

/// Tells an administrator whether the server can read a repository link, and
/// what connecting it would index, without storing anything.
pub(crate) async fn check_repository(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<RepositoryLinkRequest>,
) -> Result<Json<RepoAccessCheck>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let settings = RepoSettings {
        branch: request.branch.clone(),
        include: request.include.clone(),
        exclude: request.exclude.clone(),
    };
    if repomemo_api::looks_remote(&request.link) {
        return state
            .core
            .check_remote_repo_link(&workspace_id, &request.link, settings, request.auth())
            .await
            .map(Json)
            .map_err(map_repo_error);
    }
    state
        .core
        .check_repo_link(&workspace_id, &request.link, settings)
        .await
        .map(Json)
        .map_err(map_repo_error)
}

pub(crate) async fn connect_repository(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<RepositoryLinkRequest>,
) -> Result<(StatusCode, Json<RepositorySyncResponse>), ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let settings = RepoSettings {
        branch: request.branch.clone(),
        include: request.include.clone(),
        exclude: request.exclude.clone(),
    };
    let repository = if repomemo_api::looks_remote(&request.link) {
        state
            .core
            .connect_remote_repo(
                &workspace_id,
                &request.link,
                request.name.clone(),
                settings,
                request.auth(),
                Some(&subject.user_id),
            )
            .await
            .map_err(map_repo_error)?
    } else {
        state
            .core
            .connect_local_repo(&workspace_id, &request.link, request.name.clone(), settings)
            .await
            .map_err(map_repo_error)?
    };
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
        format!(
            "Removed repository {} and its {} files.",
            repository.name, repository.file_count
        ),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// Replaces the token of a remote repository. The new one is tried against
/// the host first, and nothing about it is ever returned.
pub(crate) async fn set_repository_token(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Json(request): Json<RepositoryTokenRequest>,
) -> Result<Json<RepoSource>, ApiError> {
    let repository = load_repository(&state, &source_id).await?;
    require_workspace_admin(&state, &subject, &repository.workspace_id).await?;
    let token = repomemo_api::RemoteToken::new(request.username, request.token.0)
        .ok_or_else(|| ApiError::bad_request("Enter the token."))?;
    state
        .core
        .set_repo_token(&source_id, token, Some(&subject.user_id))
        .await
        .map_err(map_repo_error)?;
    record_workspace_activity(
        &state,
        &repository.workspace_id,
        &subject.user_id,
        "repository_token_changed",
        "repository",
        Some(&source_id),
        format!("Replaced the access token of repository {}.", repository.name),
    )
    .await;
    state
        .core
        .repo_source(&source_id)
        .await
        .map(Json)
        .map_err(map_repo_error)
}

/// Replaces the deploy key (and, when given, the pinned host key) of an
/// `ssh` repository. The new key is tried against the host first.
pub(crate) async fn set_repository_deploy_key(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Json(request): Json<RepositoryDeployKeyRequest>,
) -> Result<Json<RepoSource>, ApiError> {
    let repository = load_repository(&state, &source_id).await?;
    require_workspace_admin(&state, &subject, &repository.workspace_id).await?;
    let key = repomemo_api::RemoteDeployKey::new(request.deploy_key.0, request.host_key.unwrap_or_default())
        .ok_or_else(|| ApiError::bad_request("Enter the deploy key."))?;
    state
        .core
        .set_repo_deploy_key(&source_id, key, Some(&subject.user_id))
        .await
        .map_err(map_repo_error)?;
    record_workspace_activity(
        &state,
        &repository.workspace_id,
        &subject.user_id,
        "repository_deploy_key_changed",
        "repository",
        Some(&source_id),
        format!("Replaced the deploy key of repository {}.", repository.name),
    )
    .await;
    state
        .core
        .repo_source(&source_id)
        .await
        .map(Json)
        .map_err(map_repo_error)
}

pub(crate) async fn clear_repository_token(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let repository = load_repository(&state, &source_id).await?;
    require_workspace_admin(&state, &subject, &repository.workspace_id).await?;
    if !state.core.clear_repo_token(&source_id).await.map_err(map_repo_error)? {
        return Err(ApiError::not_found("This repository has no token."));
    }
    record_workspace_activity(
        &state,
        &repository.workspace_id,
        &subject.user_id,
        "repository_token_removed",
        "repository",
        Some(&source_id),
        format!("Removed the access token of repository {}.", repository.name),
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

#[derive(Debug, Serialize)]
pub(crate) struct RepositoryDetailResponse {
    #[serde(flatten)]
    detail: RepoDetail,
    /// The workspace has an enabled text AI provider, so a summary can be
    /// generated.
    ai_available: bool,
}

/// The repository page: its sync state, the overview of what it holds, and
/// the last AI summary.
pub(crate) async fn get_repository_detail(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<Json<RepositoryDetailResponse>, ApiError> {
    let detail = state.core.repo_detail(&source_id).await.map_err(map_repo_error)?;
    let role = require_workspace_read(&state, &subject, &detail.repository.workspace_id).await?;
    let ai_available = text_provider(&state, &detail.repository.workspace_id).await?.is_some()
        && ai_access::role_may_use_ai(&state, &detail.repository.workspace_id, &role).await?;
    Ok(Json(RepositoryDetailResponse { detail, ai_available }))
}

/// Generates and keeps an AI summary of the repository. Only the overview
/// and the opening of its key files are sent to the provider.
pub(crate) async fn summarize_repository(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<Json<RepoSummary>, ApiError> {
    let repository = load_repository(&state, &source_id).await?;
    let role = require_workspace_write(&state, &subject, &repository.workspace_id).await?;
    let provider = text_provider(&state, &repository.workspace_id)
        .await?
        .ok_or_else(|| {
            ApiError::bad_request(
                "No text AI provider is enabled for this workspace. An administrator can set one up in Settings. No content was sent.",
            )
        })?;
    ai_access::authorize_ai_use(&state, &subject, &repository.workspace_id, &role).await?;
    let summary = state
        .core
        .summarize_repo(&source_id, &provider.id)
        .await
        .map_err(map_repo_error)?;
    record_workspace_activity(
        &state,
        &repository.workspace_id,
        &subject.user_id,
        "repository_summarized",
        "repository",
        Some(&source_id),
        format!("Generated an AI summary of repository {}.", repository.name),
    )
    .await;
    Ok(Json(summary))
}

async fn text_provider(state: &AppState, workspace_id: &str) -> Result<Option<ProviderSettings>, ApiError> {
    Ok(state
        .storage
        .list_provider_settings(workspace_id)
        .await
        .map_err(ApiError::internal)?
        .into_iter()
        .find(|settings| settings.enabled && settings.purpose() == "text"))
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
pub(crate) async fn ensure_not_repository_file(state: &AppState, artifact_id: &str) -> Result<(), ApiError> {
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
/// a repository, a branch that does not exist): those carry their kind and
/// their message is shown. Anything else is the server's problem.
fn map_repo_error(error: anyhow::Error) -> ApiError {
    crate::map_storage_error(error)
}
