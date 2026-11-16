//! Task context (V0.7.5): a task cites several pieces of evidence and memory cards, each checked against the task's own
//! workspace when it is added. The title is read from the target each time, so a deleted one shows as unavailable.

use crate::*;
use repomemo_domain::TaskReference;

pub(crate) fn check_task_reference_input(kind: &str, note: &str) -> Result<String, ApiError> {
    if !matches!(kind, "evidence" | "memory_card") {
        return Err(ApiError::bad_request("The kind must be evidence or memory_card."));
    }
    let note = note.trim().to_owned();
    if note.chars().count() > 500 {
        return Err(ApiError::bad_request("A note cannot exceed 500 characters."));
    }
    Ok(note)
}

pub(crate) async fn list_task_references(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Vec<TaskReference>>, ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &task.workspace_id).await?;
    state
        .storage
        .list_task_references(&task_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn add_task_reference(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Json(request): Json<TaskReferenceInput>,
) -> Result<(StatusCode, Json<TaskReference>), ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &task.workspace_id).await?;
    let note = check_task_reference_input(&request.kind, &request.note)?;
    let reference = state
        .storage
        .add_task_reference(&task, &subject.user_id, &request.kind, request.target_id.trim(), &note)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &task.workspace_id,
        &subject.user_id,
        "task_reference_added",
        "task",
        Some(&task.id),
        format!(
            "Linked {} to {}: {}.",
            reference.title.as_deref().unwrap_or("an item"),
            task.key,
            task.title
        ),
    )
    .await;
    Ok((StatusCode::CREATED, Json(reference)))
}

pub(crate) async fn remove_task_reference(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((task_id, reference_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &task.workspace_id).await?;
    state
        .storage
        .remove_task_reference(&task_id, &reference_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}
