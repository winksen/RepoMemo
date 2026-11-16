//! Dependencies (V0.7.9): "this task blocks that one", with a plain explanation of what blocks a task. A dependency informs and
//! never acts: closing a blocker completes nothing that depends on it.

use crate::*;
use repomemo_domain::{TaskDependencies, TaskDependency};

/// What blocks a task, in words, from the blockers that are not done. `None` when nothing blocks it.
pub(crate) fn explain_blockers(blocked_by: &[TaskDependency]) -> Option<String> {
    if blocked_by.is_empty() {
        return None;
    }
    let open: Vec<&TaskDependency> = blocked_by
        .iter()
        .filter(|dependency| dependency.blocker.status != "done")
        .collect();
    if open.is_empty() {
        return Some(format!(
            "Not blocked: {} of its blockers {} done.",
            blocked_by.len(),
            if blocked_by.len() == 1 { "is" } else { "are" }
        ));
    }
    let shown: Vec<String> = open
        .iter()
        .take(3)
        .map(|dependency| {
            format!(
                "{} {} ({})",
                dependency.blocker.key,
                dependency.blocker.title,
                dependency.blocker.status.replace('_', " ")
            )
        })
        .collect();
    let more = open.len().saturating_sub(shown.len());
    Some(format!(
        "Blocked by {}{}.",
        shown.join(", "),
        if more > 0 {
            format!(" and {more} more")
        } else {
            String::new()
        }
    ))
}

pub(crate) async fn load_dependencies(state: &AppState, task_id: &str) -> Result<TaskDependencies, ApiError> {
    let (blocked_by, blocks) = state
        .storage
        .list_task_dependencies(task_id)
        .await
        .map_err(map_storage_error)?;
    let explanation = explain_blockers(&blocked_by);
    Ok(TaskDependencies {
        blocked_by,
        blocks,
        explanation,
    })
}

pub(crate) async fn list_task_dependencies(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskDependencies>, ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &task.workspace_id).await?;
    Ok(Json(load_dependencies(&state, &task_id).await?))
}

pub(crate) async fn add_task_dependency(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Json(request): Json<AddTaskDependencyRequest>,
) -> Result<(StatusCode, Json<TaskDependency>), ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &task.workspace_id).await?;
    // Exactly one side is named: the task in the path is blocked by `blocker_task_id`, or blocks `blocked_task_id`.
    let (other_id, task_is_blocked) = match (request.blocker_task_id.as_deref(), request.blocked_task_id.as_deref()) {
        (Some(blocker), None) => (blocker.trim().to_owned(), true),
        (None, Some(blocked)) => (blocked.trim().to_owned(), false),
        _ => {
            return Err(ApiError::bad_request(
                "Name exactly one of blocker_task_id and blocked_task_id.",
            ))
        }
    };
    let other = state
        .storage
        .get_collaboration_task(&other_id)
        .await
        .map_err(|_| ApiError::bad_request("Both tasks must belong to this workspace."))?;
    let (blocker, blocked) = if task_is_blocked {
        (&other, &task)
    } else {
        (&task, &other)
    };
    let dependency = state
        .storage
        .add_task_dependency(&task.workspace_id, blocker, blocked, &subject.user_id)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &task.workspace_id,
        &subject.user_id,
        "task_dependency_added",
        "task",
        Some(&blocked.id),
        format!("{} now blocks {}.", blocker.key, blocked.key),
    )
    .await;
    Ok((StatusCode::CREATED, Json(dependency)))
}

pub(crate) async fn remove_task_dependency(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(dependency_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let workspace_id = state
        .storage
        .task_dependency_workspace(&dependency_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &workspace_id).await?;
    state
        .storage
        .remove_task_dependency(&dependency_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}
