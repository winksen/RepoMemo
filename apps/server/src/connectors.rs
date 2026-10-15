//! Connectors: trackers whose issues become evidence (roadmap F17, V0.2.27).
//!
//! The reading itself lives in `RepoMemoCore`; this layer decides who may
//! connect and sync what, runs syncs in the background so a request never
//! waits on a tracker, and follows the trackers on the same interval as
//! repository polling.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_api::RepoMemoCore;
use repomemo_domain::{ConnectorAccessCheck, ConnectorSource, ConnectorSyncReport, IndexingJobStatus};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::{
    embedding::EmbeddingQueue, record_workspace_activity, require_workspace_admin, require_workspace_read,
    require_workspace_write, system::SettingsCell, ApiError, AppState, AuthenticatedSubject,
};

/// Runs connector syncs in the background, one at a time across the server,
/// and never two for the same connector.
#[derive(Clone)]
pub(crate) struct ConnectorRunner {
    core: RepoMemoCore,
    embeddings: EmbeddingQueue,
    running: Arc<Mutex<HashSet<String>>>,
    permits: Arc<Semaphore>,
    runtime: Arc<SettingsCell>,
}

impl ConnectorRunner {
    pub(crate) fn new(core: RepoMemoCore, embeddings: EmbeddingQueue, runtime: Arc<SettingsCell>) -> Self {
        Self {
            core,
            embeddings,
            running: Arc::new(Mutex::new(HashSet::new())),
            permits: Arc::new(Semaphore::new(1)),
            runtime,
        }
    }

    fn is_running(&self, source_id: &str) -> bool {
        self.running
            .lock()
            .map(|running| running.contains(source_id))
            .unwrap_or(true)
    }

    fn forget(&self, source_id: &str) {
        if let Ok(mut running) = self.running.lock() {
            running.remove(source_id);
        }
    }

    /// Queues a sync. Returns `None` when one is already running.
    pub(crate) async fn start(
        &self,
        source: &ConnectorSource,
        actor_user_id: Option<String>,
    ) -> anyhow::Result<Option<IndexingJobStatus>> {
        // A tracker that asked to wait is not asked again before it is over.
        if let Some(until) = source.paused_until.as_deref() {
            if chrono::DateTime::parse_from_rfc3339(until).is_ok_and(|until| until > chrono::Utc::now()) {
                repomemo_domain::bail_conflict!(
                    "{} is paused until {until}: {}",
                    source.name,
                    source.paused_reason.as_deref().unwrap_or("the tracker's rate limit")
                );
            }
        }
        let newly_running = self
            .running
            .lock()
            .map(|mut running| running.insert(source.id.clone()))
            .unwrap_or(false);
        if !newly_running {
            return Ok(None);
        }
        if let Err(reason) =
            crate::quotas::room_for_sync(&self.core, &self.runtime.current(), &source.workspace_id).await
        {
            self.forget(&source.id);
            repomemo_domain::bail_conflict!("{reason}");
        }
        let job = match self.core.begin_connector_sync(&source.id).await {
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
            let result = runner.core.run_connector_sync(&source.id, &job_id).await;
            drop(permit);
            runner.forget(&source.id);
            runner.finish(&source, actor_user_id.as_deref(), result).await;
        });
        Ok(Some(job))
    }

    async fn finish(
        &self,
        source: &ConnectorSource,
        actor_user_id: Option<&str>,
        result: anyhow::Result<ConnectorSyncReport>,
    ) {
        let (action, summary) = match result {
            Ok(report) => {
                if report.indexed > 0 {
                    self.embeddings.request(&source.workspace_id);
                }
                // A background sync that found nothing new is not news.
                if report.added + report.updated == 0 && actor_user_id.is_none() {
                    return;
                }
                (
                    "connector_synced",
                    format!(
                        "Synced {}: {} added, {} updated, {} unchanged.",
                        source.name, report.added, report.updated, report.unchanged
                    ),
                )
            }
            Err(error) => {
                tracing::warn!(connector = %source.name, error = %format!("{error:#}"), "Connector sync failed");
                ("connector_sync_failed", format!("Syncing {} failed.", source.name))
            }
        };
        if let Err(error) = self
            .core
            .storage()
            .record_workspace_activity(
                &source.workspace_id,
                actor_user_id,
                action,
                "connector",
                Some(&source.id),
                &summary,
            )
            .await
        {
            tracing::error!(error = %error, "Failed to record connector activity");
        }
    }

    /// Follows every connector on the repository polling interval (which a
    /// system administrator can change or turn off at run time).
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
                let Ok(ids) = runner.core.list_all_connector_ids().await else {
                    continue;
                };
                for id in ids {
                    if runner.is_running(&id) {
                        continue;
                    }
                    let Ok(source) = runner.core.connector_source(&id).await else {
                        continue;
                    };
                    // One that failed is left for a person to look at, and
                    // one that was refused waits for a new token.
                    if source.status == "error" || source.last_synced_at.is_none() {
                        continue;
                    }
                    if source.status == "paused" && source.paused_until.is_none() {
                        continue;
                    }
                    if let Err(error) = runner.start(&source, None).await {
                        tracing::debug!(connector = %source.name, error = %format!("{error:#}"), "A connector was not synced");
                    }
                }
            }
        })
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ConnectorsResponse {
    connectors: Vec<ConnectorSource>,
}

/// A token as it arrives: it prints as nothing.
#[derive(Deserialize)]
#[serde(transparent)]
pub(crate) struct SecretText(String);

impl std::fmt::Debug for SecretText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<hidden>")
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConnectorLinkRequest {
    /// The tracker's address, such as `https://github.com/acme/app` or
    /// `https://gitlab.com/group/project`.
    link: String,
    /// `github` or `gitlab`; without it the address tells.
    #[serde(default)]
    provider: Option<String>,
    name: Option<String>,
    /// A read-only token, for a private repository or a higher rate limit.
    #[serde(default)]
    token: Option<SecretText>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConnectorTokenRequest {
    token: SecretText,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConnectorSyncResponse {
    connector: ConnectorSource,
    job: Option<IndexingJobStatus>,
}

fn map_error(error: anyhow::Error) -> ApiError {
    crate::map_storage_error(error)
}

async fn load(state: &AppState, source_id: &str) -> Result<ConnectorSource, ApiError> {
    state.core.connector_source(source_id).await.map_err(map_error)
}

pub(crate) async fn list_connectors(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<ConnectorsResponse>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let connectors = state.core.list_connectors(&workspace_id).await.map_err(map_error)?;
    Ok(Json(ConnectorsResponse { connectors }))
}

pub(crate) async fn check_connector(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<ConnectorLinkRequest>,
) -> Result<Json<ConnectorAccessCheck>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    state
        .core
        .check_tracker_link(
            &workspace_id,
            &request.link,
            request.provider.as_deref(),
            request.token.as_ref().map(|token| token.0.as_str()),
        )
        .await
        .map(Json)
        .map_err(map_error)
}

pub(crate) async fn connect_connector(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<ConnectorLinkRequest>,
) -> Result<(StatusCode, Json<ConnectorSyncResponse>), ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let connector = state
        .core
        .connect_tracker(
            &workspace_id,
            &request.link,
            request.provider.as_deref(),
            request.name,
            request.token.map(|token| token.0),
            Some(&subject.user_id),
        )
        .await
        .map_err(map_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "connector_connected",
        "connector",
        Some(&connector.id),
        format!("Connected {}.", connector.name),
    )
    .await;
    let job = state
        .connector_sync
        .start(&connector, Some(subject.user_id.clone()))
        .await
        .map_err(map_error)?;
    let connector = load(&state, &connector.id).await?;
    Ok((StatusCode::CREATED, Json(ConnectorSyncResponse { connector, job })))
}

pub(crate) async fn get_connector(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<Json<ConnectorSource>, ApiError> {
    let connector = load(&state, &source_id).await?;
    require_workspace_read(&state, &subject, &connector.workspace_id).await?;
    Ok(Json(connector))
}

pub(crate) async fn sync_connector(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<Json<ConnectorSyncResponse>, ApiError> {
    let connector = load(&state, &source_id).await?;
    require_workspace_write(&state, &subject, &connector.workspace_id).await?;
    let job = state
        .connector_sync
        .start(&connector, Some(subject.user_id.clone()))
        .await
        .map_err(map_error)?
        .ok_or_else(|| ApiError::conflict("This connector is already syncing."))?;
    let connector = load(&state, &source_id).await?;
    Ok(Json(ConnectorSyncResponse {
        connector,
        job: Some(job),
    }))
}

/// Replaces the token. The new one is tried against the tracker first.
pub(crate) async fn set_connector_token(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
    Json(request): Json<ConnectorTokenRequest>,
) -> Result<Json<ConnectorSource>, ApiError> {
    let connector = load(&state, &source_id).await?;
    require_workspace_admin(&state, &subject, &connector.workspace_id).await?;
    if request.token.0.trim().is_empty() {
        return Err(ApiError::bad_request("Enter the token."));
    }
    state
        .core
        .set_connector_token(&source_id, &request.token.0, Some(&subject.user_id))
        .await
        .map_err(map_error)?;
    record_workspace_activity(
        &state,
        &connector.workspace_id,
        &subject.user_id,
        "connector_token_changed",
        "connector",
        Some(&source_id),
        format!("Replaced the access token of {}.", connector.name),
    )
    .await;
    load(&state, &source_id).await.map(Json)
}

pub(crate) async fn delete_connector(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(source_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let connector = load(&state, &source_id).await?;
    require_workspace_admin(&state, &subject, &connector.workspace_id).await?;
    if connector.active_job.is_some() || state.connector_sync.is_running(&source_id) {
        return Err(ApiError::conflict(
            "This connector is syncing. Wait for it to finish before removing it.",
        ));
    }
    state.core.remove_connector(&source_id).await.map_err(map_error)?;
    record_workspace_activity(
        &state,
        &connector.workspace_id,
        &subject.user_id,
        "connector_removed",
        "connector",
        Some(&source_id),
        format!(
            "Removed {} and its {} items.",
            connector.name,
            connector.open_items + connector.closed_items
        ),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}
