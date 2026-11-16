//! Project updates (V0.7.17): a person publishes where a project stands, which of on track, at risk or blocked they judge it to be,
//! why, and what that rests on. The product offers counts and open blockers as a starting point and never chooses the health.
//! Anyone who can read the workspace reads the updates; a role that may write content publishes one in their own name.

use crate::*;
use repomemo_domain::{ProjectUpdate, ProjectUpdateDraft, TaskQuery, TaskSummary};
use repomemo_storage::NewProjectUpdate;

const MAX_TEXT_CHARS: usize = 2_000;

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateListFilter {
    /// `retracted` to include the updates that were retracted.
    include: Option<String>,
}

pub(crate) async fn list_project_updates(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(filter): Query<UpdateListFilter>,
) -> Result<Json<Vec<ProjectUpdate>>, ApiError> {
    let project = state
        .storage
        .get_project(&project_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &project.workspace_id).await?;
    state
        .storage
        .list_project_updates(&project_id, filter.include.as_deref() == Some("retracted"), 100)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn get_project_update(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(update_id): Path<String>,
) -> Result<Json<ProjectUpdate>, ApiError> {
    let update = state
        .storage
        .get_project_update(&update_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &update.workspace_id).await?;
    Ok(Json(update))
}

/// What the product can offer someone about to write an update: the counts as they are, the milestones and the tasks that are
/// blocked, and the update before. **It never suggests a health**: that is the author's judgement.
pub(crate) async fn get_project_update_draft(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<Json<ProjectUpdateDraft>, ApiError> {
    let project = state
        .storage
        .get_project(&project_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &project.workspace_id).await?;
    let snapshot = state
        .storage
        .project_snapshot(&project_id)
        .await
        .map_err(map_storage_error)?;
    let blocked = state
        .storage
        .query_collaboration_tasks(
            &project.workspace_id,
            &TaskQuery {
                project_id: Some(project_id.clone()),
                status: vec!["blocked".to_owned()],
                ..Default::default()
            },
            &subject.user_id,
            20,
            0,
        )
        .await
        .map_err(map_storage_error)?;
    let previous = state
        .storage
        .latest_project_update(&project_id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(ProjectUpdateDraft {
        snapshot,
        open_blockers: blocked
            .tasks
            .into_iter()
            .map(|task| TaskSummary {
                id: task.id,
                key: task.key,
                title: task.title,
                status: task.status,
            })
            .collect(),
        previous,
        note: "These are counts and a starting point. The health is yours to choose, and the reason for it is what readers will see."
            .to_owned(),
    }))
}

pub(crate) async fn publish_project_update(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(request): Json<SaveProjectUpdateRequest>,
) -> Result<(StatusCode, Json<ProjectUpdate>), ApiError> {
    let project = state
        .storage
        .get_project(&project_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &project.workspace_id).await?;
    if !matches!(request.health.as_str(), "on_track" | "at_risk" | "blocked") {
        return Err(ApiError::bad_request(
            "The health must be on_track, at_risk or blocked.",
        ));
    }
    let summary = request.summary.trim().to_owned();
    let reason = request.reason.trim().to_owned();
    if summary.is_empty() || summary.chars().count() > MAX_TEXT_CHARS {
        return Err(ApiError::bad_request(format!(
            "The summary of what changed must be between 1 and {MAX_TEXT_CHARS} characters."
        )));
    }
    if reason.is_empty() || reason.chars().count() > MAX_TEXT_CHARS {
        return Err(ApiError::bad_request(format!(
            "Say why you chose this health, in 1 to {MAX_TEXT_CHARS} characters."
        )));
    }
    if project.status == "archived" {
        return Err(ApiError::conflict(
            "An archived project is restored before it is changed.",
        ));
    }
    let update = state
        .storage
        .publish_project_update(
            &project,
            &subject.user_id,
            NewProjectUpdate {
                health: request.health,
                summary,
                reason,
                milestone_ids: request.milestone_ids,
                task_ids: request.task_ids,
                evidence_ids: request.evidence_ids,
                memory_card_ids: request.memory_card_ids,
            },
        )
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &project.workspace_id,
        &subject.user_id,
        "project_update_published",
        "project",
        Some(&project.id),
        format!(
            "Published an update on {}: {}.",
            project.name,
            update.health.replace('_', " ")
        ),
    )
    .await;
    Ok((StatusCode::CREATED, Json(update)))
}

/// Retracts an update: it leaves the default list and stays on record. Its author or an administrator can.
pub(crate) async fn retract_project_update(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(update_id): Path<String>,
) -> Result<Json<ProjectUpdate>, ApiError> {
    let current = state
        .storage
        .get_project_update(&update_id)
        .await
        .map_err(map_storage_error)?;
    let role = require_workspace_write(&state, &subject, &current.workspace_id).await?;
    let is_author = current.author.as_ref().map(|author| author.id.as_str()) == Some(subject.user_id.as_str());
    if !is_author && !matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin) {
        return Err(ApiError::forbidden());
    }
    let update = state
        .storage
        .retract_project_update(&update_id, &subject.user_id)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &update.workspace_id,
        &subject.user_id,
        "project_update_retracted",
        "project",
        Some(&update.project_id),
        "Retracted a project update.".to_owned(),
    )
    .await;
    Ok(Json(update))
}
