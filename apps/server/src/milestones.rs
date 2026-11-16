//! Milestones (V0.7.7): named, dated groups of a project's tasks. A date is a statement: changing it moves no task. Reading
//! needs workspace access; changing needs a role that may write content, and an archived project is restored first.

use crate::*;
use repomemo_domain::{Milestone, Project};
use repomemo_storage::NewMilestone;

const MAX_NAME_CHARS: usize = 120;
const MAX_DESCRIPTION_CHARS: usize = 2_000;

fn normalize_milestone(request: SaveMilestoneRequest) -> Result<NewMilestone, ApiError> {
    let name = request.name.trim().to_owned();
    let description = request.description.trim().to_owned();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(ApiError::bad_request(format!(
            "Milestone name must be between 1 and {MAX_NAME_CHARS} characters."
        )));
    }
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(ApiError::bad_request(format!(
            "Milestone description cannot exceed {MAX_DESCRIPTION_CHARS} characters."
        )));
    }
    let completion_rule = request.completion_rule.unwrap_or_else(|| "all_tasks".to_owned());
    if !matches!(completion_rule.as_str(), "all_tasks" | "manual") {
        return Err(ApiError::bad_request(
            "The completion rule must be all_tasks or manual.",
        ));
    }
    let due_date = request
        .due_date
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(date) = due_date.as_deref() {
        if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
            return Err(ApiError::bad_request(
                "Milestone date must be a calendar date, YYYY-MM-DD.",
            ));
        }
    }
    Ok(NewMilestone {
        name,
        description,
        due_date,
        completion_rule,
    })
}

async fn open_project(state: &AppState, project_id: &str) -> Result<Project, ApiError> {
    state.storage.get_project(project_id).await.map_err(map_storage_error)
}

pub(crate) async fn list_milestones(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Vec<Milestone>>, ApiError> {
    let project = open_project(&state, &project_id).await?;
    require_workspace_read(&state, &subject, &project.workspace_id).await?;
    state
        .storage
        .list_milestones(&project_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn create_milestone(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(request): Json<SaveMilestoneRequest>,
) -> Result<(StatusCode, Json<Milestone>), ApiError> {
    let project = open_project(&state, &project_id).await?;
    require_workspace_write(&state, &subject, &project.workspace_id).await?;
    let payload = normalize_milestone(request)?;
    if project.status == "archived" {
        return Err(ApiError::conflict(
            "An archived project is restored before it is changed.",
        ));
    }
    let milestone = state
        .storage
        .create_milestone(&project, &subject.user_id, payload)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &project.workspace_id,
        &subject.user_id,
        "milestone_created",
        "project",
        Some(&project.id),
        format!("Planned milestone {} in {}.", milestone.name, project.name),
    )
    .await;
    Ok((StatusCode::CREATED, Json(milestone)))
}

pub(crate) async fn get_milestone(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(milestone_id): Path<String>,
) -> Result<Json<Milestone>, ApiError> {
    let milestone = state
        .storage
        .get_milestone(&milestone_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &milestone.workspace_id).await?;
    Ok(Json(milestone))
}

pub(crate) async fn update_milestone(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(milestone_id): Path<String>,
    Json(request): Json<SaveMilestoneRequest>,
) -> Result<Json<Milestone>, ApiError> {
    let current = state
        .storage
        .get_milestone(&milestone_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    let payload = normalize_milestone(request)?;
    let milestone = state
        .storage
        .update_milestone(&milestone_id, payload)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &milestone.workspace_id,
        &subject.user_id,
        "milestone_updated",
        "project",
        Some(&milestone.project_id),
        format!("Updated milestone {}.", milestone.name),
    )
    .await;
    Ok(Json(milestone))
}

pub(crate) async fn delete_milestone(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(milestone_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let current = state
        .storage
        .get_milestone(&milestone_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    state
        .storage
        .delete_milestone(&milestone_id)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &current.workspace_id,
        &subject.user_id,
        "milestone_deleted",
        "project",
        Some(&current.project_id),
        format!("Removed milestone {}; its tasks stay in the project.", current.name),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn complete_milestone(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(milestone_id): Path<String>,
) -> Result<Json<Milestone>, ApiError> {
    set_completed(subject, state, milestone_id, true).await
}

pub(crate) async fn reopen_milestone(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(milestone_id): Path<String>,
) -> Result<Json<Milestone>, ApiError> {
    set_completed(subject, state, milestone_id, false).await
}

async fn set_completed(
    subject: AuthenticatedSubject,
    state: AppState,
    milestone_id: String,
    completed: bool,
) -> Result<Json<Milestone>, ApiError> {
    let current = state
        .storage
        .get_milestone(&milestone_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    let milestone = state
        .storage
        .set_milestone_completed(&milestone_id, completed)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &milestone.workspace_id,
        &subject.user_id,
        if completed {
            "milestone_completed"
        } else {
            "milestone_reopened"
        },
        "project",
        Some(&milestone.project_id),
        format!(
            "{} milestone {}.",
            if completed { "Completed" } else { "Reopened" },
            milestone.name
        ),
    )
    .await;
    Ok(Json(milestone))
}

/// Checks that a task's milestone belongs to the project the task will be in.
pub(crate) async fn check_task_milestone(
    state: &AppState,
    workspace_id: &str,
    milestone_id: &str,
    task_project_id: Option<&str>,
) -> Result<(), ApiError> {
    let milestone = state
        .storage
        .get_milestone(milestone_id)
        .await
        .map_err(|_| ApiError::bad_request("Task milestone must belong to this workspace."))?;
    if milestone.workspace_id != workspace_id {
        return Err(ApiError::bad_request("Task milestone must belong to this workspace."));
    }
    if task_project_id != Some(milestone.project_id.as_str()) {
        return Err(ApiError::bad_request(
            "A milestone belongs to the project the task is in; put the task in that project first.",
        ));
    }
    Ok(())
}
