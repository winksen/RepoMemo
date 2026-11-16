//! Task hierarchy (V0.7.8): a task can be a sub-task of another task of the same workspace. A sub-task is a task of its own;
//! a cycle and a tree deeper than `MAX_TASK_DEPTH` are refused; completing a parent changes nothing about its sub-tasks.

use crate::*;
use repomemo_storage::MAX_TASK_DEPTH;

/// Checks that `parent_id` can be the parent of `task_id` (a task about to be made when `None`).
pub(crate) async fn check_task_parent(
    state: &AppState,
    workspace_id: &str,
    task_id: Option<&str>,
    parent_id: &str,
) -> Result<(), ApiError> {
    let parent = state
        .storage
        .get_collaboration_task(parent_id)
        .await
        .map_err(|_| ApiError::bad_request("Parent task must belong to this workspace."))?;
    if parent.workspace_id != workspace_id {
        return Err(ApiError::bad_request("Parent task must belong to this workspace."));
    }
    let ancestors = state
        .storage
        .task_ancestor_ids(parent_id)
        .await
        .map_err(map_storage_error)?;
    let mut below = 0;
    if let Some(task_id) = task_id {
        if task_id == parent_id {
            return Err(ApiError::bad_request("A task cannot be its own parent."));
        }
        if ancestors.iter().any(|ancestor| ancestor == task_id) {
            return Err(ApiError::bad_request(
                "A task cannot be placed under one of its own sub-tasks: that would make a cycle.",
            ));
        }
        below = state
            .storage
            .task_subtree_height(task_id)
            .await
            .map_err(map_storage_error)?;
    }
    // The parent's level, this task's level, and the levels already under this task.
    if ancestors.len() + 2 + below > MAX_TASK_DEPTH {
        return Err(ApiError::bad_request(format!(
            "Tasks can be nested at most {MAX_TASK_DEPTH} levels deep."
        )));
    }
    Ok(())
}

/// The direct sub-tasks of a task, open ones first.
pub(crate) async fn list_child_tasks(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Vec<CollaborationTask>>, ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &task.workspace_id).await?;
    state
        .storage
        .list_child_tasks(&task_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}
