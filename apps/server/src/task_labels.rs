//! Labels (V0.7.10): workspace-managed names for kinds of work, with an accessible colour, applied to tasks and used to filter.
//! A label organizes only: no route reads one to decide who may see or change a task.

use crate::*;
use repomemo_domain::{TaskLabel, TaskLabelTag, LABEL_COLOR_NAMES};

const MAX_NAME_CHARS: usize = 40;

fn clean_label(request: SaveTaskLabelRequest) -> Result<(String, String), ApiError> {
    let name = request.name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS || name.chars().any(char::is_control) {
        return Err(ApiError::bad_request(format!(
            "A label name must be between 1 and {MAX_NAME_CHARS} characters."
        )));
    }
    let color = request.color.unwrap_or_else(|| "gray".to_owned());
    if !LABEL_COLOR_NAMES.contains(&color.as_str()) {
        return Err(ApiError::bad_request(format!(
            "The colour must be one of: {}.",
            LABEL_COLOR_NAMES.join(", ")
        )));
    }
    Ok((name, color))
}

pub(crate) async fn list_task_labels(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<TaskLabel>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_task_labels(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn create_task_label(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SaveTaskLabelRequest>,
) -> Result<(StatusCode, Json<TaskLabel>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let (name, color) = clean_label(request)?;
    let label = state
        .storage
        .create_task_label(&workspace_id, &name, &color)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "task_label_created",
        "task_label",
        Some(&label.id),
        format!("Created label {}.", label.name),
    )
    .await;
    Ok((StatusCode::CREATED, Json(label)))
}

pub(crate) async fn update_task_label(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(label_id): Path<String>,
    Json(request): Json<SaveTaskLabelRequest>,
) -> Result<Json<TaskLabel>, ApiError> {
    let current = state
        .storage
        .get_task_label(&label_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    let (name, color) = clean_label(request)?;
    let label = state
        .storage
        .update_task_label(&label_id, &name, &color)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &label.workspace_id,
        &subject.user_id,
        "task_label_updated",
        "task_label",
        Some(&label.id),
        format!("Changed label {} to {}.", current.name, label.name),
    )
    .await;
    Ok(Json(label))
}

pub(crate) async fn delete_task_label(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(label_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let current = state
        .storage
        .get_task_label(&label_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_admin(&state, &subject, &current.workspace_id).await?;
    state
        .storage
        .delete_task_label(&label_id)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &current.workspace_id,
        &subject.user_id,
        "task_label_deleted",
        "task_label",
        Some(&current.id),
        format!("Deleted label {} from {} tasks.", current.name, current.task_count),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// Moves every task of one label to another and removes the first. Owners and administrators only: it changes many tasks.
pub(crate) async fn merge_task_labels(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(label_id): Path<String>,
    Json(request): Json<MergeTaskLabelRequest>,
) -> Result<Json<TaskLabel>, ApiError> {
    let from = state
        .storage
        .get_task_label(&label_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_admin(&state, &subject, &from.workspace_id).await?;
    if request.into_label_id == from.id {
        return Err(ApiError::bad_request("A label cannot be merged into itself."));
    }
    let into = state
        .storage
        .get_task_label(&request.into_label_id)
        .await
        .map_err(|_| ApiError::bad_request("Both labels must belong to this workspace."))?;
    if into.workspace_id != from.workspace_id {
        return Err(ApiError::bad_request("Both labels must belong to this workspace."));
    }
    let merged = state
        .storage
        .merge_task_labels(&from.id, &into.id)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &merged.workspace_id,
        &subject.user_id,
        "task_labels_merged",
        "task_label",
        Some(&merged.id),
        format!("Merged label {} into {}.", from.name, merged.name),
    )
    .await;
    Ok(Json(merged))
}

/// Replaces the labels on a task (at most ten). Anyone who can edit the task can; the labels must be the workspace's.
pub(crate) async fn set_task_labels(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Json(request): Json<SetTaskLabelsRequest>,
) -> Result<Json<Vec<TaskLabelTag>>, ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &task.workspace_id).await?;
    let labels = state
        .storage
        .set_task_labels(&task, &request.label_ids)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(labels))
}
