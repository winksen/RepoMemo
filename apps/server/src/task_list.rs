//! The task list (V0.7.12): a page of a workspace's tasks for a query (project, milestone, assignee, priority, status, label,
//! due date, title), and saved views of a query. A view holds only filters and is evaluated against the reader's current
//! rights on every use; the same query model serves the board.

use crate::*;
use repomemo_domain::{TaskPage, TaskQuery, TaskView};
use repomemo_storage::{DEFAULT_TASK_PAGE, MAX_TASK_PAGE};

#[derive(Debug, Default, Deserialize)]
pub(crate) struct TaskListParams {
    /// A saved view whose filters apply; any other parameter replaces the view's own.
    view: Option<String>,
    project_id: Option<String>,
    milestone_id: Option<String>,
    /// A user id, `me` (the reader) or `none`.
    assignee: Option<String>,
    /// One or more priorities, separated by commas.
    priority: Option<String>,
    /// One or more of the four statuses, separated by commas.
    status: Option<String>,
    label: Option<String>,
    due_before: Option<String>,
    due_after: Option<String>,
    overdue: Option<bool>,
    q: Option<String>,
    sort: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

fn words(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|part| part.trim().to_lowercase())
        .filter(|part| !part.is_empty())
        .collect()
}

/// Checks a query and tidies it; says what is wrong in words.
pub(crate) fn validate_query(query: &mut TaskQuery) -> Result<(), ApiError> {
    for priority in &query.priority {
        if !matches!(priority.as_str(), "low" | "medium" | "high" | "urgent") {
            return Err(ApiError::bad_request("Priority must be low, medium, high or urgent."));
        }
    }
    for status in &query.status {
        if !matches!(status.as_str(), "open" | "in_progress" | "blocked" | "done") {
            return Err(ApiError::bad_request(
                "Status must be open, in_progress, blocked or done.",
            ));
        }
    }
    for date in [&query.due_before, &query.due_after].into_iter().flatten() {
        if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
            return Err(ApiError::bad_request(
                "A due date filter must be a calendar date, YYYY-MM-DD.",
            ));
        }
    }
    if let Some(sort) = query.sort.as_deref() {
        if !matches!(sort, "priority" | "due" | "updated" | "number") {
            return Err(ApiError::bad_request("Sort must be priority, due, updated or number."));
        }
    }
    query.q = query
        .q
        .take()
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty());
    if query.q.as_ref().is_some_and(|text| text.chars().count() > 120) {
        return Err(ApiError::bad_request("A title search cannot exceed 120 characters."));
    }
    Ok(())
}

pub(crate) async fn list_task_page(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(params): Query<TaskListParams>,
) -> Result<Json<TaskPage>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let mut query = TaskQuery::default();
    if let Some(view_id) = params.view.as_deref() {
        let view = state.storage.get_task_view(view_id).await.map_err(map_storage_error)?;
        // A view of another workspace, or someone else's private one, does not exist for this reader.
        if view.workspace_id != workspace_id || (!view.shared && view.owner.id != subject.user_id) {
            return Err(ApiError::not_found("View was not found."));
        }
        query = view.query;
    }
    if let Some(value) = params.project_id {
        query.project_id = Some(value);
    }
    if let Some(value) = params.milestone_id {
        query.milestone_id = Some(value);
    }
    if let Some(value) = params.assignee {
        query.assignee = Some(value);
    }
    if let Some(value) = params.priority {
        query.priority = words(&value);
    }
    if let Some(value) = params.status {
        query.status = words(&value);
    }
    if let Some(value) = params.label {
        query.label_id = Some(value);
    }
    if let Some(value) = params.due_before {
        query.due_before = Some(value);
    }
    if let Some(value) = params.due_after {
        query.due_after = Some(value);
    }
    if let Some(value) = params.overdue {
        query.overdue = value;
    }
    if let Some(value) = params.q {
        query.q = Some(value);
    }
    if let Some(value) = params.sort {
        query.sort = Some(value);
    }
    validate_query(&mut query)?;
    let limit = params.limit.unwrap_or(DEFAULT_TASK_PAGE);
    if !(1..=MAX_TASK_PAGE).contains(&limit) {
        return Err(ApiError::bad_request(format!(
            "A page holds between 1 and {MAX_TASK_PAGE} tasks."
        )));
    }
    let offset = params.offset.unwrap_or(0);
    if !(0..=1_000_000).contains(&offset) {
        return Err(ApiError::bad_request("The offset must be between 0 and 1,000,000."));
    }
    state
        .storage
        .query_collaboration_tasks(&workspace_id, &query, &subject.user_id, limit, offset)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

fn clean_view_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::bad_request(
            "A view name must be between 1 and 80 characters.",
        ));
    }
    Ok(name)
}

pub(crate) async fn list_task_views(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<TaskView>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_task_views(&workspace_id, &subject.user_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

/// Saves a view. Anyone who can read the workspace can keep their own views, a reader included: a view changes no task.
pub(crate) async fn create_task_view(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SaveTaskViewRequest>,
) -> Result<(StatusCode, Json<TaskView>), ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let name = clean_view_name(&request.name)?;
    let mut query = request.query;
    validate_query(&mut query)?;
    let view = state
        .storage
        .create_task_view(&workspace_id, &subject.user_id, &name, &query)
        .await
        .map_err(map_storage_error)?;
    Ok((StatusCode::CREATED, Json(view)))
}

async fn own_view(state: &AppState, subject: &AuthenticatedSubject, view_id: &str) -> Result<TaskView, ApiError> {
    let view = state.storage.get_task_view(view_id).await.map_err(map_storage_error)?;
    require_workspace_read(state, subject, &view.workspace_id).await?;
    if view.owner.id != subject.user_id {
        // Someone else's private view is not there for this reader; a shared one is there but is not theirs to change.
        return Err(if view.shared {
            ApiError::forbidden()
        } else {
            ApiError::not_found("View was not found.")
        });
    }
    Ok(view)
}

pub(crate) async fn update_task_view(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(view_id): Path<String>,
    Json(request): Json<SaveTaskViewRequest>,
) -> Result<Json<TaskView>, ApiError> {
    own_view(&state, &subject, &view_id).await?;
    let name = clean_view_name(&request.name)?;
    let mut query = request.query;
    validate_query(&mut query)?;
    state
        .storage
        .update_task_view(&view_id, &name, &query)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn set_task_view_sharing(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(view_id): Path<String>,
    Json(request): Json<SetViewSharingRequest>,
) -> Result<Json<TaskView>, ApiError> {
    own_view(&state, &subject, &view_id).await?;
    state
        .storage
        .set_task_view_sharing(&view_id, request.shared)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn delete_task_view(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(view_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    own_view(&state, &subject, &view_id).await?;
    state
        .storage
        .delete_task_view(&view_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}
