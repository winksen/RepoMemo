//! Recurring work (V0.7.16): tasks made again on an explicit schedule. Anyone who can read the workspace sees the schedules and
//! their next dates; a role that may write content makes, changes, pauses, resumes and stops them. The maintenance pass makes
//! the tasks that are owed, once for each occurrence date.

use crate::*;
use repomemo_domain::{RecurringDetail, RecurringTask};
use repomemo_storage::{check_schedule, NewRecurringTask};

/// Makes the tasks that are owed today, tells their assignees and writes them to the activity feed. Returns how many were made.
pub(crate) async fn make_due(state: &AppState) -> anyhow::Result<usize> {
    let tasks = state
        .storage
        .make_due_recurring_tasks(chrono::Utc::now().date_naive())
        .await?;
    for task in &tasks {
        notify_task_assignee(state, &task.created_by.id, task).await;
        record_workspace_activity(
            state,
            &task.workspace_id,
            &task.created_by.id,
            "task_created",
            "task",
            Some(&task.id),
            format!("A recurring task made {}: {}.", task.key, task.title),
        )
        .await;
    }
    Ok(tasks.len())
}

async fn normalize_recurring(
    state: &AppState,
    workspace_id: &str,
    request: SaveRecurringTaskRequest,
) -> Result<NewRecurringTask, ApiError> {
    let title = request.title.trim().to_owned();
    let description = request.description.trim().to_owned();
    if title.is_empty() || title.chars().count() > 180 {
        return Err(ApiError::bad_request(
            "Task title must be between 1 and 180 characters.",
        ));
    }
    if description.chars().count() > 5_000 {
        return Err(ApiError::bad_request(
            "Task description cannot exceed 5,000 characters.",
        ));
    }
    let priority = request.priority.unwrap_or_else(|| "medium".to_owned());
    if !matches!(priority.as_str(), "low" | "medium" | "high" | "urgent") {
        return Err(ApiError::bad_request(
            "Task priority must be low, medium, high, or urgent.",
        ));
    }
    let missed_policy = request.missed_policy.unwrap_or_else(|| "latest".to_owned());
    if !matches!(missed_policy.as_str(), "latest" | "skip") {
        return Err(ApiError::bad_request("The missed-run policy must be latest or skip."));
    }
    if request.due_after_days.is_some_and(|days| !(0..=365).contains(&days)) {
        return Err(ApiError::bad_request("Days until due must be between 0 and 365."));
    }
    check_schedule(&request.schedule).map_err(ApiError::bad_request)?;
    let assignee_user_id = request.assignee_user_id.filter(|value| !value.trim().is_empty());
    if let Some(user_id) = assignee_user_id.as_deref() {
        let member = state
            .storage
            .workspace_role_for_user(user_id, workspace_id)
            .await
            .map_err(map_storage_error)?;
        if member.is_none() {
            return Err(ApiError::bad_request("Task assignee must be a workspace member."));
        }
    }
    let project_id = request.project_id.filter(|value| !value.trim().is_empty());
    if let Some(project_id) = project_id.as_deref() {
        crate::projects::check_task_project(state, workspace_id, project_id, None).await?;
    }
    Ok(NewRecurringTask {
        title,
        description,
        priority,
        assignee_user_id,
        project_id,
        due_after_days: request.due_after_days,
        schedule: Some(request.schedule),
        missed_policy,
        paused: request.paused,
    })
}

pub(crate) async fn list_recurring_tasks(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<RecurringTask>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_recurring_tasks(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn create_recurring_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SaveRecurringTaskRequest>,
) -> Result<(StatusCode, Json<RecurringTask>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let payload = normalize_recurring(&state, &workspace_id, request).await?;
    let recurring = state
        .storage
        .create_recurring_task(&workspace_id, &subject.user_id, payload)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "recurring_task_created",
        "recurring_task",
        Some(&recurring.id),
        format!("Scheduled the recurring task {}.", recurring.title),
    )
    .await;
    Ok((StatusCode::CREATED, Json(recurring)))
}

pub(crate) async fn get_recurring_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(recurring_id): Path<String>,
) -> Result<Json<RecurringDetail>, ApiError> {
    let recurring = state
        .storage
        .get_recurring_task(&recurring_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &recurring.workspace_id).await?;
    let occurrences = state
        .storage
        .list_recurring_occurrences(&recurring_id, 60)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(RecurringDetail { recurring, occurrences }))
}

pub(crate) async fn update_recurring_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(recurring_id): Path<String>,
    Json(request): Json<SaveRecurringTaskRequest>,
) -> Result<Json<RecurringTask>, ApiError> {
    let current = state
        .storage
        .get_recurring_task(&recurring_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    let payload = normalize_recurring(&state, &current.workspace_id, request).await?;
    let recurring = state
        .storage
        .update_recurring_task(&recurring_id, payload)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &recurring.workspace_id,
        &subject.user_id,
        "recurring_task_updated",
        "recurring_task",
        Some(&recurring.id),
        format!("Changed the recurring task {}.", recurring.title),
    )
    .await;
    Ok(Json(recurring))
}

async fn set_status(
    subject: AuthenticatedSubject,
    state: AppState,
    recurring_id: String,
    status: &str,
) -> Result<Json<RecurringTask>, ApiError> {
    let current = state
        .storage
        .get_recurring_task(&recurring_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    let recurring = state
        .storage
        .set_recurring_status(&recurring_id, status)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &recurring.workspace_id,
        &subject.user_id,
        match status {
            "paused" => "recurring_task_paused",
            "stopped" => "recurring_task_stopped",
            _ => "recurring_task_resumed",
        },
        "recurring_task",
        Some(&recurring.id),
        format!("The recurring task {} is now {status}.", recurring.title),
    )
    .await;
    Ok(Json(recurring))
}

pub(crate) async fn pause_recurring_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(recurring_id): Path<String>,
) -> Result<Json<RecurringTask>, ApiError> {
    set_status(subject, state, recurring_id, "paused").await
}

pub(crate) async fn resume_recurring_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(recurring_id): Path<String>,
) -> Result<Json<RecurringTask>, ApiError> {
    set_status(subject, state, recurring_id, "active").await
}

pub(crate) async fn stop_recurring_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(recurring_id): Path<String>,
) -> Result<Json<RecurringTask>, ApiError> {
    set_status(subject, state, recurring_id, "stopped").await
}
