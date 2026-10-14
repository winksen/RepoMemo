use crate::*;

pub(crate) async fn list_collaboration_tasks(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<CollaborationTask>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_collaboration_tasks(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn get_collaboration_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<CollaborationTask>, ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &task.workspace_id).await?;
    Ok(Json(task))
}

pub(crate) async fn create_collaboration_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SaveCollaborationTaskRequest>,
) -> Result<(StatusCode, Json<CollaborationTask>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let payload = normalize_collaboration_task(&state, &workspace_id, request).await?;
    let task = state
        .storage
        .create_collaboration_task(&workspace_id, &subject.user_id, payload)
        .await
        .map_err(map_storage_error)?;
    notify_task_assignee(&state, &subject.user_id, &task).await;
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
    Ok((StatusCode::CREATED, Json(task)))
}

pub(crate) async fn update_collaboration_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Json(request): Json<SaveCollaborationTaskRequest>,
) -> Result<Json<CollaborationTask>, ApiError> {
    let current = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    let payload = normalize_collaboration_task(&state, &current.workspace_id, request).await?;
    let task = state
        .storage
        .update_collaboration_task(&task_id, payload)
        .await
        .map_err(map_storage_error)?;
    if current.assignee.as_ref().map(|member| member.id.as_str())
        != task.assignee.as_ref().map(|member| member.id.as_str())
    {
        notify_task_assignee(&state, &subject.user_id, &task).await;
    }
    record_workspace_activity(
        &state,
        &task.workspace_id,
        &subject.user_id,
        "task_updated",
        "task",
        Some(&task.id),
        format!("Updated task: {} ({}).", task.title, task.status.replace('_', " ")),
    )
    .await;
    Ok(Json(task))
}

pub(crate) async fn delete_collaboration_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    // Authors who are now viewers no longer change content, their own included.
    let role = require_workspace_write(&state, &subject, &task.workspace_id).await?;
    if task.created_by.id != subject.user_id && !matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin) {
        return Err(ApiError::forbidden());
    }
    state
        .storage
        .delete_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &task.workspace_id,
        &subject.user_id,
        "task_deleted",
        "task",
        Some(&task.id),
        format!("Deleted task: {}.", task.title),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn list_saved_searches(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<SavedSearch>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_saved_searches(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn create_saved_search(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SaveSearchRequest>,
) -> Result<(StatusCode, Json<SavedSearch>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let name = request.name.trim().to_owned();
    let query = request.query.trim().to_owned();
    let result_limit = request.result_limit.unwrap_or(20);
    if name.is_empty()
        || name.len() > 120
        || query.is_empty()
        || query.len() > 500
        || !(1..=100).contains(&result_limit)
    {
        return Err(ApiError::bad_request(
            "Saved search needs a name, query, and a result limit between 1 and 100.",
        ));
    }
    let saved = state
        .storage
        .create_saved_search(
            &workspace_id,
            &subject.user_id,
            NewSavedSearch {
                name,
                query,
                artifact_types: request.artifact_types,
                languages: request.languages,
                source_ids: request.source_ids,
                result_limit,
            },
        )
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "saved_search_created",
        "saved_search",
        Some(&saved.id),
        format!("Saved retrieval workflow: {}.", saved.name),
    )
    .await;
    Ok((StatusCode::CREATED, Json(saved)))
}

pub(crate) async fn delete_saved_search(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(search_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let saved = state
        .storage
        .get_saved_search(&search_id)
        .await
        .map_err(map_storage_error)?;
    // Saved searches are shared; only their author or an administrator removes one.
    let role = require_workspace_write(&state, &subject, &saved.workspace_id).await?;
    if saved.created_by.id != subject.user_id && !matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin) {
        return Err(ApiError::forbidden());
    }
    state
        .storage
        .delete_saved_search(&search_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn list_task_checklist(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Vec<TaskChecklistItem>>, ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &task.workspace_id).await?;
    state
        .storage
        .list_task_checklist_items(&task_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn create_task_checklist_item(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Json(request): Json<CreateChecklistItemRequest>,
) -> Result<(StatusCode, Json<TaskChecklistItem>), ApiError> {
    let task = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &task.workspace_id).await?;
    let body = request.body.trim().to_owned();
    if body.is_empty() || body.len() > 500 {
        return Err(ApiError::bad_request(
            "Checklist item must be between 1 and 500 characters.",
        ));
    }
    let item = state
        .storage
        .create_task_checklist_item(
            &task_id,
            &task.workspace_id,
            &subject.user_id,
            NewTaskChecklistItem { body },
        )
        .await
        .map_err(map_storage_error)?;
    Ok((StatusCode::CREATED, Json(item)))
}

pub(crate) async fn toggle_task_checklist_item(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(item_id): Path<String>,
    Json(request): Json<ToggleChecklistItemRequest>,
) -> Result<Json<TaskChecklistItem>, ApiError> {
    let current = state
        .storage
        .get_task_checklist_item(&item_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    state
        .storage
        .toggle_task_checklist_item(&item_id, &subject.user_id, request.completed)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn delete_task_checklist_item(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(item_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let current = state
        .storage
        .get_task_checklist_item(&item_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    state
        .storage
        .delete_task_checklist_item(&item_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn normalize_collaboration_task(
    state: &AppState,
    workspace_id: &str,
    request: SaveCollaborationTaskRequest,
) -> Result<NewCollaborationTask, ApiError> {
    let title = request.title.trim().to_owned();
    let description = request.description.trim().to_owned();
    if title.is_empty() || title.len() > 180 {
        return Err(ApiError::bad_request(
            "Task title must be between 1 and 180 characters.",
        ));
    }
    if description.len() > 5_000 {
        return Err(ApiError::bad_request(
            "Task description cannot exceed 5,000 characters.",
        ));
    }
    if !matches!(request.status.as_str(), "open" | "in_progress" | "blocked" | "done") {
        return Err(ApiError::bad_request(
            "Task status must be open, in_progress, blocked, or done.",
        ));
    }
    if !matches!(request.priority.as_str(), "low" | "medium" | "high" | "urgent") {
        return Err(ApiError::bad_request(
            "Task priority must be low, medium, high, or urgent.",
        ));
    }
    if let Some(user_id) = request.assignee_user_id.as_deref() {
        if state
            .storage
            .workspace_role_for_user(user_id, workspace_id)
            .await
            .map_err(map_storage_error)?
            .is_none()
        {
            return Err(ApiError::bad_request("Task assignee must be a workspace member."));
        }
    }
    if let Some(artifact_id) = request.artifact_id.as_deref() {
        let artifact = state
            .storage
            .get_artifact(artifact_id)
            .await
            .map_err(map_storage_error)?;
        if artifact.summary.workspace_id != workspace_id {
            return Err(ApiError::bad_request("Task evidence must belong to this workspace."));
        }
    }
    let due_at = request.due_at.filter(|value| !value.trim().is_empty());
    if due_at.as_ref().is_some_and(|value| value.len() > 64) {
        return Err(ApiError::bad_request("Task due date is invalid."));
    }
    Ok(NewCollaborationTask {
        title,
        description,
        status: request.status,
        priority: request.priority,
        assignee_user_id: request.assignee_user_id,
        artifact_id: request.artifact_id,
        due_at,
    })
}

pub(crate) async fn list_artifact_comments(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<Vec<ArtifactComment>>, ApiError> {
    let artifact = state
        .storage
        .get_artifact(&artifact_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &artifact.summary.workspace_id).await?;
    state
        .storage
        .list_artifact_comments(&artifact_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn create_artifact_comment(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
    Json(request): Json<SaveArtifactCommentRequest>,
) -> Result<(StatusCode, Json<ArtifactComment>), ApiError> {
    let artifact = state
        .storage
        .get_artifact(&artifact_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &artifact.summary.workspace_id).await?;
    let body = validate_comment_body(&request.body)?;
    let comment = state
        .storage
        .create_artifact_comment(&artifact.summary.workspace_id, &artifact_id, &subject.user_id, &body)
        .await
        .map_err(map_storage_error)?;
    notify_artifact_mentions(
        &state,
        &subject.user_id,
        &artifact.summary.workspace_id,
        &artifact_id,
        &artifact.summary.title,
        &body,
    )
    .await;
    record_workspace_activity(
        &state,
        &comment.workspace_id,
        &subject.user_id,
        "comment_added",
        "artifact",
        Some(&artifact_id),
        format!("Commented on evidence: {}.", artifact.summary.title),
    )
    .await;
    Ok((StatusCode::CREATED, Json(comment)))
}

pub(crate) async fn update_artifact_comment(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(comment_id): Path<String>,
    Json(request): Json<SaveArtifactCommentRequest>,
) -> Result<Json<ArtifactComment>, ApiError> {
    let current = state
        .storage
        .get_artifact_comment(&comment_id)
        .await
        .map_err(map_storage_error)?;
    // Authors who are now viewers no longer change content, their own included.
    let role = require_workspace_write(&state, &subject, &current.workspace_id).await?;
    if current.author.id != subject.user_id && !matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin) {
        return Err(ApiError::forbidden());
    }
    let body = validate_comment_body(&request.body)?;
    let updated = state
        .storage
        .update_artifact_comment(&comment_id, &body)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &updated.workspace_id,
        &subject.user_id,
        "comment_updated",
        "artifact",
        Some(&updated.artifact_id),
        "Updated an evidence comment.".to_owned(),
    )
    .await;
    Ok(Json(updated))
}

pub(crate) async fn delete_artifact_comment(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(comment_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let comment = state
        .storage
        .get_artifact_comment(&comment_id)
        .await
        .map_err(map_storage_error)?;
    // Authors who are now viewers no longer change content, their own included.
    let role = require_workspace_write(&state, &subject, &comment.workspace_id).await?;
    if comment.author.id != subject.user_id && !matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin) {
        return Err(ApiError::forbidden());
    }
    state
        .storage
        .delete_artifact_comment(&comment_id)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &comment.workspace_id,
        &subject.user_id,
        "comment_deleted",
        "artifact",
        Some(&comment.artifact_id),
        "Removed an evidence comment.".to_owned(),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) fn validate_comment_body(body: &str) -> Result<String, ApiError> {
    let body = body.trim().to_owned();
    if body.is_empty() || body.len() > 5_000 {
        return Err(ApiError::bad_request("Comment must be between 1 and 5,000 characters."));
    }
    Ok(body)
}
