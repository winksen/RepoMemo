//! Tracker context (V0.8.8): a mirrored issue or pull request and the local work that follows it up.
//!
//! Ownership stays apart. The tracker owns the item and its state (`remote`, read-only here); a task that follows it up is a
//! separate local record with its own status, assignee and project, linked by a citation, so a local edit is never read as a
//! remote update and a remote update never changes the task. Links are readable in both directions.

use crate::*;
use repomemo_domain::{FollowUpProject, ItemFollowUps};

#[derive(Debug, Default, Deserialize)]
pub(crate) struct FollowUpRequest {
    /// The task's title; by default "Follow up:" and the title of the item.
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    assignee_user_id: Option<String>,
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    due_at: Option<String>,
    /// Make another task although one is already open for the item.
    #[serde(default)]
    allow_another: bool,
}

async fn load_mirrored_item(
    state: &AppState,
    artifact_id: &str,
) -> Result<(repomemo_domain::ArtifactSummary, repomemo_domain::RemoteItemInfo), ApiError> {
    let artifact = state
        .storage
        .get_artifact_summary(artifact_id)
        .await
        .map_err(map_storage_error)?;
    // An item held back by a quarantine is not there for anyone.
    let remote = state
        .storage
        .remote_item_info(artifact_id)
        .await
        .map_err(map_storage_error)?;
    match remote {
        Some(remote) => Ok((artifact, remote)),
        None => {
            if state
                .storage
                .artifact_is_quarantined(artifact_id)
                .await
                .map_err(map_storage_error)?
            {
                Err(ApiError::not_found(
                    "This item is held back while its source's access is checked.",
                ))
            } else {
                Err(ApiError::bad_request(
                    "Only an issue or pull request mirrored from a tracker has a tracker state to follow up.",
                ))
            }
        }
    }
}

/// The tracker's state of a mirrored item, and every local task and project that cites it.
pub(crate) async fn list_follow_ups(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<ItemFollowUps>, ApiError> {
    let artifact = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &artifact.workspace_id).await?;
    let (_, remote) = load_mirrored_item(&state, &artifact_id).await?;
    let tasks = state
        .storage
        .tasks_following(&artifact.workspace_id, &artifact_id)
        .await
        .map_err(map_storage_error)?;
    let projects = state
        .storage
        .list_projects_citing(&artifact.workspace_id, "evidence", &artifact_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .map(|project| FollowUpProject {
            project_id: project.id,
            name: project.name,
            status: project.status,
        })
        .collect();
    Ok(Json(ItemFollowUps {
        artifact_id,
        remote,
        tasks,
        projects,
    }))
}

/// Makes a local task that follows a mirrored item up. The task cites the item and is otherwise its own: its own owner, status
/// and project. One open follow-up per item unless another is asked for.
pub(crate) async fn create_follow_up(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
    body: Option<Json<FollowUpRequest>>,
) -> Result<(StatusCode, Json<CollaborationTask>), ApiError> {
    let request = body.map(|Json(request)| request).unwrap_or_default();
    let artifact = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &artifact.workspace_id).await?;
    load_mirrored_item(&state, &artifact_id).await?;
    if !request.allow_another {
        let open = state
            .storage
            .tasks_following(&artifact.workspace_id, &artifact_id)
            .await
            .map_err(map_storage_error)?
            .into_iter()
            .filter(|task| task.status != "done")
            .map(|task| task.key)
            .collect::<Vec<_>>();
        if !open.is_empty() {
            return Err(ApiError::conflict(format!(
                "{} already follows this item up. Send allow_another to make one more.",
                open.join(", ")
            )));
        }
    }
    let title = request
        .title
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| format!("Follow up: {}", artifact.title));
    let title: String = title.chars().take(180).collect();
    let task = SaveCollaborationTaskRequest {
        title,
        description: request.description.unwrap_or_else(|| {
            "A local follow-up of an item mirrored from a tracker. The item's state belongs to the tracker; this task's status is yours."
                .to_owned()
        }),
        status: "open".to_owned(),
        state_id: None,
        priority: request.priority.unwrap_or_else(|| "medium".to_owned()),
        assignee_user_id: request.assignee_user_id,
        artifact_id: None,
        project_id: request.project_id,
        milestone_id: None,
        parent_task_id: None,
        expected_version: None,
        references: vec![TaskReferenceInput {
            kind: "evidence".to_owned(),
            target_id: artifact_id,
            note: "Follow-up of a mirrored item".to_owned(),
        }],
        due_at: request.due_at,
    };
    crate::collaboration::create_collaboration_task(subject, State(state), Path(artifact.workspace_id), Json(task))
        .await
}
