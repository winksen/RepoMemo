//! Workflow states (V0.7.11): named task states mapped to the four stable categories. Reading the workflow needs workspace
//! access; changing it is an administrator's job, because it can move many tasks. A task's `status` stays its category.

use crate::*;
use repomemo_domain::{WorkflowState, BUILTIN_STATE_CATEGORIES};

const MAX_NAME_CHARS: usize = 40;

#[derive(Debug, Deserialize)]
pub(crate) struct RemoveStateFilter {
    /// Where the tasks in the removed state go: a built-in category name or another state's id.
    move_to: Option<String>,
}

fn clean_state(request: SaveWorkflowStateRequest) -> Result<(String, String), ApiError> {
    let name = request.name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS || name.chars().any(char::is_control) {
        return Err(ApiError::bad_request(format!(
            "A state name must be between 1 and {MAX_NAME_CHARS} characters."
        )));
    }
    if !BUILTIN_STATE_CATEGORIES.contains(&request.category.as_str()) {
        return Err(ApiError::bad_request(
            "The category must be open, in_progress, blocked or done.",
        ));
    }
    Ok((name, request.category))
}

pub(crate) async fn get_workflow(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<WorkflowState>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_workflow_states(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn create_workflow_state(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SaveWorkflowStateRequest>,
) -> Result<(StatusCode, Json<WorkflowState>), ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let (name, category) = clean_state(request)?;
    let created = state
        .storage
        .create_workflow_state(&workspace_id, &name, &category)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "workflow_state_created",
        "workflow_state",
        Some(&created.id),
        format!(
            "Added the task state {} ({}).",
            created.name,
            created.category.replace('_', " ")
        ),
    )
    .await;
    Ok((StatusCode::CREATED, Json(created)))
}

pub(crate) async fn update_workflow_state(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(state_id): Path<String>,
    Json(request): Json<SaveWorkflowStateRequest>,
) -> Result<Json<WorkflowState>, ApiError> {
    if BUILTIN_STATE_CATEGORIES.contains(&state_id.as_str()) {
        return Err(ApiError::bad_request("A built-in state cannot be changed."));
    }
    let current = state
        .storage
        .get_workflow_state(&state_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_admin(&state, &subject, &current.workspace_id).await?;
    let (name, category) = clean_state(request)?;
    let updated = state
        .storage
        .update_workflow_state(&state_id, &name, &category)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &updated.workspace_id,
        &subject.user_id,
        "workflow_state_updated",
        "workflow_state",
        Some(&updated.id),
        format!(
            "Changed the task state {} to {} ({}); {} task(s) follow it.",
            current.name,
            updated.name,
            updated.category.replace('_', " "),
            updated.task_count
        ),
    )
    .await;
    Ok(Json(updated))
}

pub(crate) async fn delete_workflow_state(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(state_id): Path<String>,
    Query(filter): Query<RemoveStateFilter>,
) -> Result<StatusCode, ApiError> {
    if BUILTIN_STATE_CATEGORIES.contains(&state_id.as_str()) {
        return Err(ApiError::bad_request("A built-in state cannot be removed."));
    }
    let current = state
        .storage
        .get_workflow_state(&state_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_admin(&state, &subject, &current.workspace_id).await?;
    let moved = state
        .storage
        .delete_workflow_state(&state_id, filter.move_to.as_deref())
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &current.workspace_id,
        &subject.user_id,
        "workflow_state_deleted",
        "workflow_state",
        Some(&current.id),
        format!("Removed the task state {}; {moved} task(s) were moved.", current.name),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// Works out a task's status and state from what a save names. A built-in state or a status alone is the plain case; a named
/// state gives its category, and a status that disagrees with it is refused so the mapping is never guessed.
pub(crate) async fn resolve_task_state(
    state: &AppState,
    workspace_id: &str,
    status: &str,
    state_id: Option<&str>,
) -> Result<(String, Option<Option<String>>), ApiError> {
    let requested = state_id.map(str::trim).filter(|value| !value.is_empty());
    let Some(id) = requested else {
        return Ok((status.to_owned(), None));
    };
    if BUILTIN_STATE_CATEGORIES.contains(&id) {
        if !status.is_empty() && status != id {
            return Err(ApiError::bad_request(format!(
                "The state {id} has the category {id}; the status sent was {status}."
            )));
        }
        return Ok((id.to_owned(), Some(None)));
    }
    let named = state
        .storage
        .get_workflow_state(id)
        .await
        .map_err(|_| ApiError::bad_request("Task state must belong to this workspace."))?;
    if named.workspace_id != workspace_id {
        return Err(ApiError::bad_request("Task state must belong to this workspace."));
    }
    if !status.is_empty() && status != named.category {
        return Err(ApiError::bad_request(format!(
            "The state {} has the category {}; the status sent was {status}.",
            named.name, named.category
        )));
    }
    Ok((named.category, Some(Some(named.id))))
}
