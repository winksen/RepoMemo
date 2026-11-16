//! Task discussion (V0.7.4): comments, mentions and watching on a task. A comment stays with its task; a mention notifies the
//! person once (and the same person gets no second "new comment" notice for it); a person who mutes a task stops the
//! optional comment notices and still gets told when the task is assigned to them.

use crate::*;
use repomemo_domain::{TaskComment, TaskWatch};

const MAX_COMMENT_CHARS: usize = 5_000;

fn clean_comment(body: &str) -> Result<String, ApiError> {
    let body = body.trim().to_owned();
    if body.is_empty() || body.chars().count() > MAX_COMMENT_CHARS {
        return Err(ApiError::bad_request(format!(
            "A comment must be between 1 and {MAX_COMMENT_CHARS} characters."
        )));
    }
    Ok(body)
}

pub(crate) async fn list_task_comments(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Vec<TaskComment>>, ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &task.workspace_id).await?;
    state
        .storage
        .list_task_comments(&task_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn add_task_comment(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Json(request): Json<SaveTaskCommentRequest>,
) -> Result<(StatusCode, Json<TaskComment>), ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &task.workspace_id).await?;
    let body = clean_comment(&request.body)?;
    let comment = state
        .storage
        .add_task_comment(&task, &subject.user_id, &body)
        .await
        .map_err(map_storage_error)?;
    let mentioned = notify_mentions(
        &state,
        &subject.user_id,
        &task.workspace_id,
        MentionedIn::TaskComment {
            key: &task.key,
            title: &task.title,
        },
        &body,
        "",
    )
    .await;
    notify_followers(&state, &subject.user_id, &task, &mentioned).await;
    record_workspace_activity(
        &state,
        &task.workspace_id,
        &subject.user_id,
        "task_commented",
        "task",
        Some(&task.id),
        format!("Commented on {}: {}.", task.key, task.title),
    )
    .await;
    Ok((StatusCode::CREATED, Json(comment)))
}

pub(crate) async fn update_task_comment(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(comment_id): Path<String>,
    Json(request): Json<SaveTaskCommentRequest>,
) -> Result<Json<TaskComment>, ApiError> {
    let current = state
        .storage
        .get_task_comment(&comment_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    if current.author.as_ref().map(|author| author.id.as_str()) != Some(subject.user_id.as_str()) {
        return Err(ApiError::forbidden());
    }
    let body = clean_comment(&request.body)?;
    let comment = state
        .storage
        .update_task_comment(&comment_id, &body)
        .await
        .map_err(map_storage_error)?;
    let task = state
        .storage
        .get_collaboration_task(&comment.task_id)
        .await
        .map_err(map_storage_error)?;
    // Only people this edit adds are told; nobody is notified twice for the same comment.
    notify_mentions(
        &state,
        &subject.user_id,
        &task.workspace_id,
        MentionedIn::TaskComment {
            key: &task.key,
            title: &task.title,
        },
        &body,
        &current.body,
    )
    .await;
    Ok(Json(comment))
}

pub(crate) async fn delete_task_comment(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(comment_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let current = state
        .storage
        .get_task_comment(&comment_id)
        .await
        .map_err(map_storage_error)?;
    let role = require_workspace_write(&state, &subject, &current.workspace_id).await?;
    let is_author = current.author.as_ref().map(|author| author.id.as_str()) == Some(subject.user_id.as_str());
    if !is_author && !matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin) {
        return Err(ApiError::forbidden());
    }
    state
        .storage
        .delete_task_comment(&comment_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn get_task_watch(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskWatch>, ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &task.workspace_id).await?;
    let mode = state
        .storage
        .task_watch_mode(&task_id, &subject.user_id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(TaskWatch { mode }))
}

/// Follow a task (`watching`), stop the optional notices (`muted`) or go back to the default (`default`). Anyone who can read
/// the workspace can, a guest included; it changes nothing about the task.
pub(crate) async fn set_task_watch(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Json(request): Json<SetTaskWatchRequest>,
) -> Result<Json<TaskWatch>, ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &task.workspace_id).await?;
    if !matches!(request.mode.as_str(), "watching" | "muted" | "default") {
        return Err(ApiError::bad_request("The mode must be watching, muted or default."));
    }
    state
        .storage
        .set_task_watch(&task_id, &subject.user_id, &request.mode)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(TaskWatch { mode: request.mode }))
}

/// Tells the people who follow a task that it has a new comment, through their own notification choice. Not the author, not
/// anyone the comment already mentioned (they have their own notice), and only people who can still read the workspace.
async fn notify_followers(state: &AppState, actor_user_id: &str, task: &CollaborationTask, already_told: &[String]) {
    let followers = match state.storage.task_followers(task).await {
        Ok(followers) => followers,
        Err(error) => {
            tracing::error!(error = %error, "Failed to find the followers of a task");
            return;
        }
    };
    let path = format!("/workspaces/{}/tasks/{}", task.workspace_id, task.key);
    for user_id in followers {
        if user_id == actor_user_id || already_told.contains(&user_id) {
            continue;
        }
        let still_member = matches!(
            state
                .storage
                .workspace_role_for_user(&user_id, &task.workspace_id)
                .await,
            Ok(Some(_))
        );
        if !still_member {
            continue;
        }
        let (in_app, by_mail) = delivery(state, &user_id, "task_comment").await;
        if in_app {
            create_notification(
                state,
                NewSharedNotification {
                    user_id: user_id.clone(),
                    workspace_id: Some(task.workspace_id.clone()),
                    notification_type: "task_comment".to_owned(),
                    title: "New comment on a task you follow".to_owned(),
                    body: format!("{} {} has a new comment.", task.key, task.title),
                    href: path.clone(),
                },
            )
            .await;
        }
        if by_mail {
            crate::mail::mail_notification(
                state,
                crate::mail::NotificationMail {
                    kind: "task_comment",
                    actor_id: actor_user_id,
                    recipient_id: &user_id,
                    action: "commented on the task",
                    subject_title: &task.title,
                    path: &path,
                },
            )
            .await;
        }
    }
}
