//! HTTP surface of Workspace Health. The detectors live in `RepoMemoCore`;
//! this layer checks membership, lets administrators act on a finding, and
//! records what they did. An action re-runs the detectors and applies only to
//! the files of the finding as it stands now, so a stale page can never act
//! on files the finding no longer covers.

use axum::{
    extract::{Path, State},
    Json,
};
use repomemo_domain::{
    HealthAction, HealthActionRequest, HealthActionResult, HealthFinding, WorkspaceHealth,
};
use repomemo_storage::{NewCollaborationTask, SaveArtifactLifecycle};

use crate::{
    map_core_error, map_storage_error, record_workspace_activity, require_workspace_admin,
    require_workspace_read, ApiError, AppState, AuthenticatedSubject,
};

const MAX_TASK_TITLE_CHARS: usize = 180;
const MAX_TASK_DESCRIPTION_CHARS: usize = 5_000;

pub(crate) async fn workspace_health(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceHealth>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .core
        .workspace_health(&workspace_id)
        .await
        .map(Json)
        .map_err(map_core_error)
}

pub(crate) async fn apply_health_action(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<HealthActionRequest>,
) -> Result<Json<HealthActionResult>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let finding = state
        .core
        .workspace_health(&workspace_id)
        .await
        .map_err(map_core_error)?
        .findings
        .into_iter()
        .find(|finding| finding.fingerprint == request.fingerprint)
        .ok_or_else(|| {
            ApiError::conflict("This finding changed or was already handled. Refresh to see the current list.")
        })?;
    if !finding.actions.contains(&request.action) {
        return Err(ApiError::bad_request("This action is not available for this finding."));
    }

    let mut updated_artifact_ids = Vec::new();
    let mut task_id = None;
    match request.action {
        HealthAction::Supersede => {
            let keep = request
                .keep_artifact_id
                .or_else(|| finding.keep_artifact_id.clone())
                .filter(|keep| finding.files.iter().any(|file| &file.artifact_id == keep))
                .ok_or_else(|| ApiError::bad_request("Choose one of this finding's files to keep."))?;
            for file in finding.files.iter().filter(|file| file.artifact_id != keep) {
                set_lifecycle(&state, &subject, &file.artifact_id, "superseded", Some(keep.clone()), &finding).await?;
                updated_artifact_ids.push(file.artifact_id.clone());
            }
        }
        HealthAction::NeedsReview | HealthAction::MarkOutdated => {
            let status = if request.action == HealthAction::NeedsReview { "needs_review" } else { "outdated" };
            for file in &finding.files {
                set_lifecycle(&state, &subject, &file.artifact_id, status, None, &finding).await?;
                updated_artifact_ids.push(file.artifact_id.clone());
            }
        }
        HealthAction::CreateTask => {
            let task = state
                .storage
                .create_collaboration_task(
                    &workspace_id,
                    &subject.user_id,
                    NewCollaborationTask {
                        title: clip(&finding.title, MAX_TASK_TITLE_CHARS),
                        description: clip(&task_description(&finding), MAX_TASK_DESCRIPTION_CHARS),
                        status: "open".to_owned(),
                        priority: "medium".to_owned(),
                        assignee_user_id: None,
                        artifact_id: finding.files.first().map(|file| file.artifact_id.clone()),
                        due_at: None,
                    },
                )
                .await
                .map_err(map_storage_error)?;
            record_workspace_activity(
                &state,
                &workspace_id,
                &subject.user_id,
                "task_created",
                "task",
                Some(&task.id),
                format!("Created task: {}.", task.title),
            )
            .await;
            task_id = Some(task.id);
        }
        HealthAction::Dismiss => {}
    }

    state
        .storage
        .record_health_action(
            &workspace_id,
            &finding.fingerprint,
            finding.detector.as_str(),
            request.action.as_str(),
            Some(&subject.user_id),
        )
        .await
        .map_err(map_storage_error)?;
    let verb = match request.action {
        HealthAction::Supersede => "Marked superseded",
        HealthAction::NeedsReview => "Flagged for review",
        HealthAction::MarkOutdated => "Marked outdated",
        HealthAction::CreateTask => "Opened a task for",
        HealthAction::Dismiss => "Dismissed",
    };
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "workspace_health_action",
        "health_finding",
        None,
        format!("{verb} a health finding: {}.", finding.title),
    )
    .await;

    Ok(Json(HealthActionResult {
        action: request.action,
        updated_artifact_ids,
        task_id,
    }))
}

/// Changes a file's lifecycle status, keeping its owner and any note a
/// person already wrote.
async fn set_lifecycle(
    state: &AppState,
    subject: &AuthenticatedSubject,
    artifact_id: &str,
    status: &str,
    superseded_by_artifact_id: Option<String>,
    finding: &HealthFinding,
) -> Result<(), ApiError> {
    let current = state
        .storage
        .get_artifact_lifecycle(artifact_id)
        .await
        .map_err(map_storage_error)?;
    let review_note = if current.review_note.trim().is_empty() {
        format!("Workspace Health: {}", finding.title)
    } else {
        current.review_note
    };
    state
        .storage
        .save_artifact_lifecycle(
            artifact_id,
            &subject.user_id,
            SaveArtifactLifecycle {
                status: status.to_owned(),
                owner_user_id: current.owner.map(|owner| owner.id),
                review_note,
                superseded_by_artifact_id,
            },
        )
        .await
        .map_err(map_storage_error)?;
    Ok(())
}

fn task_description(finding: &HealthFinding) -> String {
    let mut description = format!("{}\n\nFiles:\n", finding.detail);
    for file in &finding.files {
        description.push_str(&format!("- {} ({})\n", file.title, file.path));
    }
    if !finding.evidence.is_empty() {
        description.push_str("\nEvidence:\n");
        for evidence in &finding.evidence {
            let lines = match (evidence.start_line, evidence.end_line) {
                (Some(start), Some(end)) if end > start => format!(", lines {start}-{end}"),
                (Some(start), _) => format!(", line {start}"),
                _ => String::new(),
            };
            description.push_str(&format!("- {}{lines}: \"{}\"\n", evidence.title, evidence.excerpt));
        }
    }
    description.push_str("\nRaised by Workspace Health.");
    description
}

/// At most `max` bytes, cut on a character boundary.
fn clip(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max.saturating_sub('…'.len_utf8());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}
