//! CAIRN's weekly memory upkeep (V0.9.56): the switch an administrator turns on, resolving what the pass found, and the maintenance step
//! that runs it. Off for every workspace until turned on. The pass reads and records; it never edits a file or a card.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_domain::UpkeepSettings;
use serde::Deserialize;

use crate::{
    map_storage_error, require_workspace_admin, require_workspace_read, require_workspace_write, ApiError, AppState,
    AuthenticatedSubject,
};

/// How long a workspace waits between two passes.
const PASS_EVERY_DAYS: i64 = 7;

#[derive(Debug, Deserialize)]
pub(crate) struct UpkeepRequest {
    enabled: bool,
}

pub(crate) async fn get_upkeep(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<UpkeepSettings>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .upkeep_settings(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn set_upkeep(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<UpkeepRequest>,
) -> Result<Json<UpkeepSettings>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    state
        .storage
        .set_upkeep_enabled(&workspace_id, request.enabled)
        .await
        .map_err(map_storage_error)?;
    state
        .storage
        .upkeep_settings(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn resolve_upkeep_finding(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, finding_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    state
        .storage
        .resolve_upkeep_finding(&workspace_id, &finding_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Runs the pass for each workspace that turned it on and has not been run for a week. Returns how many findings were added.
pub(crate) async fn run_due(state: &AppState) -> anyhow::Result<usize> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(PASS_EVERY_DAYS)).to_rfc3339();
    let mut added = 0;
    for workspace_id in state.storage.workspaces_due_for_upkeep(&cutoff).await? {
        match state.core.run_upkeep(&workspace_id).await {
            Ok(count) => added += count,
            Err(error) => {
                tracing::warn!(workspace = %workspace_id, error = %format!("{error:#}"), "The memory upkeep of a workspace failed");
            }
        }
    }
    Ok(added)
}
