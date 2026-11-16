//! Projects (V0.7.1): workspace-scoped pieces of work with an owner, a goal, a lifecycle and a target date. A project
//! grants nothing: reading one needs workspace read access, changing one needs a role that may write content.

use crate::*;
use repomemo_domain::{Project, ProjectBrief, ProjectReference};
use repomemo_storage::NewProject;

const MAX_NAME_CHARS: usize = 120;
const MAX_GOAL_CHARS: usize = 5_000;

#[derive(Debug, Deserialize)]
pub(crate) struct ProjectFilter {
    /// `active`, `on_hold`, `done` or `archived`; without it the archived projects are left out.
    status: Option<String>,
}

/// Normalizes what a person typed into what is stored, or says what is wrong with it.
pub(crate) async fn normalize_project(
    state: &AppState,
    workspace_id: &str,
    request: SaveProjectRequest,
) -> Result<NewProject, ApiError> {
    let name = request.name.trim().to_owned();
    let goal = request.goal.trim().to_owned();
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(ApiError::bad_request(format!(
            "Project name must be between 1 and {MAX_NAME_CHARS} characters."
        )));
    }
    if goal.chars().count() > MAX_GOAL_CHARS {
        return Err(ApiError::bad_request(format!(
            "Project goal cannot exceed {MAX_GOAL_CHARS} characters."
        )));
    }
    let status = request.status.unwrap_or_else(|| "active".to_owned());
    if !matches!(status.as_str(), "active" | "on_hold" | "done") {
        return Err(ApiError::bad_request(
            "Project status must be active, on_hold or done; archive it with the archive route.",
        ));
    }
    let owner_user_id = request.owner_user_id.filter(|value| !value.trim().is_empty());
    if let Some(user_id) = owner_user_id.as_deref() {
        let member = state
            .storage
            .workspace_role_for_user(user_id, workspace_id)
            .await
            .map_err(map_storage_error)?;
        if member.is_none() {
            return Err(ApiError::bad_request("Project owner must be a workspace member."));
        }
    }
    let target_date = request
        .target_date
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    if let Some(date) = target_date.as_deref() {
        if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
            return Err(ApiError::bad_request(
                "Project target date must be a calendar date, YYYY-MM-DD.",
            ));
        }
    }
    Ok(NewProject {
        name,
        goal,
        status,
        owner_user_id,
        target_date,
    })
}

pub(crate) async fn list_projects(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(filter): Query<ProjectFilter>,
) -> Result<Json<Vec<Project>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    if let Some(status) = filter.status.as_deref() {
        if !matches!(status, "active" | "on_hold" | "done" | "archived") {
            return Err(ApiError::bad_request(
                "Project status must be active, on_hold, done or archived.",
            ));
        }
    }
    state
        .storage
        .list_projects(&workspace_id, filter.status.as_deref())
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn create_project(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SaveProjectRequest>,
) -> Result<(StatusCode, Json<Project>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let payload = normalize_project(&state, &workspace_id, request).await?;
    let project = state
        .storage
        .create_project(&workspace_id, &subject.user_id, payload)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "project_created",
        "project",
        Some(&project.id),
        format!("Created project: {}.", project.name),
    )
    .await;
    Ok((StatusCode::CREATED, Json(project)))
}

pub(crate) async fn get_project(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Project>, ApiError> {
    let project = state
        .storage
        .get_project(&project_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &project.workspace_id).await?;
    Ok(Json(project))
}

pub(crate) async fn update_project(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(request): Json<SaveProjectRequest>,
) -> Result<Json<Project>, ApiError> {
    let current = state
        .storage
        .get_project(&project_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    let payload = normalize_project(&state, &current.workspace_id, request).await?;
    let project = state
        .storage
        .update_project(&project_id, payload)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &project.workspace_id,
        &subject.user_id,
        "project_updated",
        "project",
        Some(&project.id),
        format!(
            "Updated project: {} ({}).",
            project.name,
            project.status.replace('_', " ")
        ),
    )
    .await;
    Ok(Json(project))
}

pub(crate) async fn archive_project(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Project>, ApiError> {
    set_archived(subject, state, project_id, true).await
}

pub(crate) async fn restore_project(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Project>, ApiError> {
    set_archived(subject, state, project_id, false).await
}

async fn set_archived(
    subject: AuthenticatedSubject,
    state: AppState,
    project_id: String,
    archived: bool,
) -> Result<Json<Project>, ApiError> {
    let current = state
        .storage
        .get_project(&project_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    let project = state
        .storage
        .set_project_archived(&project_id, archived)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &project.workspace_id,
        &subject.user_id,
        if archived {
            "project_archived"
        } else {
            "project_restored"
        },
        "project",
        Some(&project.id),
        format!(
            "{} project: {}.",
            if archived { "Archived" } else { "Restored" },
            project.name
        ),
    )
    .await;
    Ok(Json(project))
}

/// Checks that a task's project belongs to its workspace and can take new work.
pub(crate) async fn check_task_project(
    state: &AppState,
    workspace_id: &str,
    project_id: &str,
    already_in: Option<&str>,
) -> Result<(), ApiError> {
    let project = state
        .storage
        .get_project(project_id)
        .await
        .map_err(|_| ApiError::bad_request("Task project must belong to this workspace."))?;
    if project.workspace_id != workspace_id {
        return Err(ApiError::bad_request("Task project must belong to this workspace."));
    }
    // An archived project keeps the tasks it has; nothing new is added to it.
    if project.status == "archived" && already_in != Some(project_id) {
        return Err(ApiError::conflict(
            "An archived project cannot take new tasks; restore it first.",
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub(crate) struct CitingFilter {
    kind: String,
    target_id: String,
}

fn check_reference_kind(kind: &str) -> Result<(), ApiError> {
    if matches!(kind, "memory_card" | "evidence") {
        Ok(())
    } else {
        Err(ApiError::bad_request("The kind must be memory_card or evidence."))
    }
}

/// The project with what its brief cites. Titles are read from the targets now, so a deleted target is shown as unavailable.
pub(crate) async fn get_project_brief(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<ProjectBrief>, ApiError> {
    let project = state
        .storage
        .get_project(&project_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &project.workspace_id).await?;
    let references = state
        .storage
        .list_project_references(&project_id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(ProjectBrief { project, references }))
}

pub(crate) async fn add_project_reference(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(request): Json<AddProjectReferenceRequest>,
) -> Result<(StatusCode, Json<ProjectReference>), ApiError> {
    let project = state
        .storage
        .get_project(&project_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &project.workspace_id).await?;
    check_reference_kind(&request.kind)?;
    let note = request.note.trim().to_owned();
    if note.chars().count() > 500 {
        return Err(ApiError::bad_request("A note cannot exceed 500 characters."));
    }
    if project.status == "archived" {
        return Err(ApiError::conflict(
            "An archived project is restored before it is changed.",
        ));
    }
    let reference = state
        .storage
        .add_project_reference(
            &project,
            &subject.user_id,
            &request.kind,
            request.target_id.trim(),
            &note,
        )
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &project.workspace_id,
        &subject.user_id,
        "project_reference_added",
        "project",
        Some(&project.id),
        format!(
            "Cited {} in project: {}.",
            reference.title.as_deref().unwrap_or("an item"),
            project.name
        ),
    )
    .await;
    Ok((StatusCode::CREATED, Json(reference)))
}

pub(crate) async fn remove_project_reference(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((project_id, reference_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let project = state
        .storage
        .get_project(&project_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &project.workspace_id).await?;
    if project.status == "archived" {
        return Err(ApiError::conflict(
            "An archived project is restored before it is changed.",
        ));
    }
    state
        .storage
        .remove_project_reference(&project_id, &reference_id)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &project.workspace_id,
        &subject.user_id,
        "project_reference_removed",
        "project",
        Some(&project.id),
        format!("Removed a citation from project: {}.", project.name),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// The backlinks of a card or a file: the projects whose brief cites it. Anyone who can read the workspace can ask.
pub(crate) async fn list_projects_citing(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(filter): Query<CitingFilter>,
) -> Result<Json<Vec<Project>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    check_reference_kind(&filter.kind)?;
    state
        .storage
        .list_projects_citing(&workspace_id, &filter.kind, &filter.target_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}
