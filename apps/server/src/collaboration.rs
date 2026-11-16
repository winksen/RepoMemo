use crate::*;
use repomemo_domain::{TaskAttempt, TaskConflict, TaskConflictError, TaskDetail};
use repomemo_storage::TaskUpdate;

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
    // Every citation is checked before the task is made, so a bad one leaves nothing behind.
    let mut citations = Vec::new();
    for reference in &request.references {
        let note = crate::task_references::check_task_reference_input(&reference.kind, &reference.note)?;
        state
            .storage
            .check_reference_target(&workspace_id, &reference.kind, reference.target_id.trim())
            .await
            .map_err(map_storage_error)?;
        citations.push((reference.kind.clone(), reference.target_id.trim().to_owned(), note));
    }
    let payload = normalize_collaboration_task(&state, &workspace_id, request, None, None).await?;
    let task = state
        .storage
        .create_collaboration_task(&workspace_id, &subject.user_id, payload)
        .await
        .map_err(map_storage_error)?;
    for (kind, target_id, note) in citations {
        // The same item twice in one request is one citation.
        match state
            .storage
            .add_task_reference(&task, &subject.user_id, &kind, &target_id, &note)
            .await
        {
            Ok(_) => {}
            Err(error)
                if matches!(
                    repomemo_domain::classify(&error),
                    Some((repomemo_domain::ErrorKind::Conflict, _))
                ) => {}
            Err(error) => return Err(map_storage_error(error)),
        }
    }
    notify_task_assignee(&state, &subject.user_id, &task).await;
    notify_mentions(
        &state,
        &subject.user_id,
        &workspace_id,
        MentionedIn::Task { title: &task.title },
        &task.description,
        "",
    )
    .await;
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

/// An error from saving a task: the usual one, or a refusal that carries the task as it is and the change that was attempted.
pub(crate) enum TaskSaveError {
    Api(ApiError),
    Stale(Box<TaskConflict>),
}

impl From<ApiError> for TaskSaveError {
    fn from(error: ApiError) -> Self {
        Self::Api(error)
    }
}

impl axum::response::IntoResponse for TaskSaveError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Api(error) => error.into_response(),
            Self::Stale(conflict) => (StatusCode::CONFLICT, Json(*conflict)).into_response(),
        }
    }
}

pub(crate) async fn update_collaboration_task(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Json(request): Json<SaveCollaborationTaskRequest>,
) -> Result<Json<CollaborationTask>, TaskSaveError> {
    let current = state
        .storage
        .get_collaboration_task(&task_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &current.workspace_id).await?;
    let attempted = TaskAttempt {
        title: request.title.clone(),
        description: request.description.clone(),
        status: request.status.clone(),
        priority: request.priority.clone(),
        assignee_user_id: request.assignee_user_id.clone(),
        artifact_id: request.artifact_id.clone(),
        project_id: request.project_id.clone(),
        due_at: request.due_at.clone(),
    };
    let payload = normalize_collaboration_task(
        &state,
        &current.workspace_id,
        request,
        current.project_id.as_deref(),
        current.milestone_id.as_deref(),
    )
    .await?;
    let task = match state
        .storage
        .update_collaboration_task(&task_id, payload)
        .await
        .map_err(map_storage_error)?
    {
        TaskUpdate::Saved(task) => *task,
        TaskUpdate::Stale(now) => {
            return Err(TaskSaveError::Stale(Box::new(TaskConflict {
                error: TaskConflictError {
                    code: "conflict".to_owned(),
                    message: format!(
                        "{} was changed by someone else (now at version {}). Your change is returned with it so nothing is lost.",
                        now.key, now.version
                    ),
                },
                current: *now,
                attempted,
            })));
        }
    };
    if current.assignee.as_ref().map(|member| member.id.as_str())
        != task.assignee.as_ref().map(|member| member.id.as_str())
    {
        notify_task_assignee(&state, &subject.user_id, &task).await;
    }
    // Only people this save adds are told, not everyone the description already named.
    notify_mentions(
        &state,
        &subject.user_id,
        &task.workspace_id,
        MentionedIn::Task { title: &task.title },
        &task.description,
        &current.description,
    )
    .await;
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
        .list_visible_saved_searches(&workspace_id, &subject.user_id)
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

pub(crate) async fn set_saved_search_sharing(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(search_id): Path<String>,
    Json(request): Json<ShareSavedSearchRequest>,
) -> Result<Json<SavedSearch>, ApiError> {
    let saved = state
        .storage
        .get_saved_search(&search_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_owner(&state, &subject, &saved.workspace_id).await?;
    state
        .storage
        .set_saved_search_sharing(&search_id, request.shared)
        .await
        .map(Json)
        .map_err(map_storage_error)
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
    current_project_id: Option<&str>,
    current_milestone_id: Option<&str>,
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
            .get_artifact_summary(artifact_id)
            .await
            .map_err(map_storage_error)?;
        if artifact.workspace_id != workspace_id {
            return Err(ApiError::bad_request("Task evidence must belong to this workspace."));
        }
    }
    let due_at = request.due_at.filter(|value| !value.trim().is_empty());
    if due_at.as_ref().is_some_and(|value| value.len() > 64) {
        return Err(ApiError::bad_request("Task due date is invalid."));
    }
    // Left out keeps the project; an empty value removes it; anything else must be a project of this workspace.
    let project_id = match request.project_id.as_deref().map(str::trim) {
        None => None,
        Some("") => Some(None),
        Some(id) => {
            crate::projects::check_task_project(state, workspace_id, id, current_project_id).await?;
            Some(Some(id.to_owned()))
        }
    };
    // The task's milestone belongs to the project it ends up in; moving it to another project drops the milestone.
    let ends_in: Option<String> = match &project_id {
        None => current_project_id.map(str::to_owned),
        Some(project) => project.clone(),
    };
    let milestone_id = match request.milestone_id.as_deref().map(str::trim) {
        Some("") => Some(None),
        Some(id) => {
            crate::milestones::check_task_milestone(state, workspace_id, id, ends_in.as_deref()).await?;
            Some(Some(id.to_owned()))
        }
        None if project_id.is_some() && current_milestone_id.is_some() && ends_in.as_deref() != current_project_id => {
            Some(None)
        }
        None => None,
    };
    Ok(NewCollaborationTask {
        title,
        description,
        status: request.status,
        priority: request.priority,
        assignee_user_id: request.assignee_user_id,
        artifact_id: request.artifact_id,
        project_id,
        milestone_id,
        due_at,
        expected_version: request.expected_version,
    })
}

pub(crate) async fn list_artifact_comments(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(artifact_id): Path<String>,
) -> Result<Json<Vec<ArtifactComment>>, ApiError> {
    let artifact = state
        .storage
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &artifact.workspace_id).await?;
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
        .get_artifact_summary(&artifact_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_write(&state, &subject, &artifact.workspace_id).await?;
    let body = validate_comment_body(&request.body)?;
    let comment = state
        .storage
        .create_artifact_comment(&artifact.workspace_id, &artifact_id, &subject.user_id, &body)
        .await
        .map_err(map_storage_error)?;
    notify_artifact_mentions(
        &state,
        &subject.user_id,
        &artifact.workspace_id,
        &artifact_id,
        &artifact.title,
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
        format!("Commented on evidence: {}.", artifact.title),
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

/// People suggested while typing `@` in a comment.
const MENTION_SUGGESTION_LIMIT: usize = 8;

#[derive(Debug, Deserialize)]
pub(crate) struct MentionSuggestionQuery {
    /// What was typed after the `@`.
    #[serde(default)]
    q: String,
}

/// Members of the workspace a comment can mention, best match first:
/// whose name or address starts with what was typed, then whose name has a
/// word that does, then whose name or address contains it. The person asking
/// is left out, and so is anyone without an address (an anonymized account).
pub(crate) fn rank_mention_candidates(
    members: &[WorkspaceMember],
    typed: &str,
    asking_user_id: &str,
) -> Vec<SharedUser> {
    let typed = typed.trim().trim_start_matches('@').to_lowercase();
    let mut ranked = members
        .iter()
        .filter(|member| member.user.id != asking_user_id)
        .filter_map(|member| {
            let email = member
                .user
                .email
                .as_deref()
                .filter(|email| !email.is_empty())?
                .to_lowercase();
            let name = member.user.display_name.to_lowercase();
            let local = email.split('@').next().unwrap_or_default().to_owned();
            let score = if typed.is_empty() {
                2
            } else if name.starts_with(&typed) || local.starts_with(&typed) || email.starts_with(&typed) {
                0
            } else if name.split_whitespace().any(|word| word.starts_with(&typed)) {
                1
            } else if name.contains(&typed) || email.contains(&typed) {
                2
            } else {
                return None;
            };
            Some((score, name, member.user.clone()))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    ranked.into_iter().map(|(_, _, user)| user).collect()
}

/// Suggestions for an `@mention` in a comment: members of this workspace
/// only, for the people who can comment, a few at a time.
pub(crate) async fn suggest_mentions(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<MentionSuggestionQuery>,
) -> Result<Json<Vec<SharedUser>>, ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let members = state
        .storage
        .list_workspace_members(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let mut suggestions = Vec::new();
    for user in rank_mention_candidates(&members, &query.q, &subject.user_id) {
        // Someone who is deactivated would never see the mention.
        if state.storage.is_deactivated(&user.id).await.unwrap_or(true) {
            continue;
        }
        suggestions.push(user);
        if suggestions.len() == MENTION_SUGGESTION_LIMIT {
            break;
        }
    }
    Ok(Json(suggestions))
}

/// A task by its identifier: `T-12`, `12` or its id. A guest gets the authorized view; a task that was deleted says so, and
/// one that never existed says that, so an old link is never a bare error.
pub(crate) async fn get_task_detail(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, task_ref)): Path<(String, String)>,
) -> Result<Json<TaskDetail>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let reference = task_ref.trim();
    let number = reference
        .strip_prefix("T-")
        .or_else(|| reference.strip_prefix("t-"))
        .unwrap_or(reference)
        .parse::<i64>()
        .ok()
        .filter(|number| *number > 0);
    let task = match number {
        Some(number) => {
            let found = state
                .storage
                .find_collaboration_task_by_number(&workspace_id, number)
                .await
                .map_err(map_storage_error)?;
            match found {
                Some(task) => task,
                None => {
                    let deleted = state
                        .storage
                        .task_number_was_deleted(&workspace_id, number)
                        .await
                        .map_err(map_storage_error)?;
                    return Err(if deleted {
                        ApiError::gone(format!("Task T-{number} was deleted."))
                    } else {
                        ApiError::not_found(format!("There is no task T-{number} in this workspace."))
                    });
                }
            }
        }
        None => {
            let task = state
                .storage
                .get_collaboration_task(reference)
                .await
                .map_err(|_| ApiError::not_found("Task was not found."))?;
            if task.workspace_id != workspace_id {
                return Err(ApiError::not_found("Task was not found."));
            }
            task
        }
    };
    let checklist = state
        .storage
        .list_task_checklist_items(&task.id)
        .await
        .map_err(map_storage_error)?;
    let path = format!("/workspaces/{}/tasks/{}", task.workspace_id, task.key);
    let watch = state
        .storage
        .task_watch_mode(&task.id, &subject.user_id)
        .await
        .map_err(map_storage_error)?;
    let comment_count = state
        .storage
        .count_task_comments(&task.id)
        .await
        .map_err(map_storage_error)?;
    let references = state
        .storage
        .list_task_references(&task.id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(TaskDetail {
        task,
        checklist,
        path,
        watch,
        comment_count,
        references,
    }))
}

#[cfg(test)]
mod mention_tests {
    use super::*;
    use repomemo_domain::WorkspaceRole;

    fn member(id: &str, name: &str, email: Option<&str>) -> WorkspaceMember {
        WorkspaceMember {
            user: SharedUser {
                id: id.to_owned(),
                display_name: name.to_owned(),
                email: email.map(str::to_owned),
            },
            role: WorkspaceRole::Member,
            joined_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn names(users: &[SharedUser]) -> Vec<&str> {
        users.iter().map(|user| user.display_name.as_str()).collect()
    }

    #[test]
    fn what_is_typed_ranks_names_then_words_then_anywhere() {
        let members = [
            member("1", "Grace Hopper", Some("grace@example.com")),
            member("2", "Ada Lovelace", Some("ada@example.com")),
            member("3", "Margaret Hamilton", Some("mh@example.com")),
            member("4", "Gracie Allen", Some("allen@example.com")),
        ];
        // "gra": two names start with it, nobody else matches.
        assert_eq!(
            names(&rank_mention_candidates(&members, "gra", "")),
            ["Grace Hopper", "Gracie Allen"]
        );
        // A later word of the name, and the address.
        assert_eq!(
            names(&rank_mention_candidates(&members, "ham", "")),
            ["Margaret Hamilton"]
        );
        assert_eq!(
            names(&rank_mention_candidates(&members, "@MH", "")),
            ["Margaret Hamilton"]
        );
        // Anywhere in the name or address, last.
        assert_eq!(
            names(&rank_mention_candidates(&members, "ace", "")),
            ["Ada Lovelace", "Grace Hopper"]
        );
        // Nothing typed yet: everyone, by name.
        assert_eq!(
            names(&rank_mention_candidates(&members, "", "")),
            ["Ada Lovelace", "Grace Hopper", "Gracie Allen", "Margaret Hamilton"]
        );
    }

    #[test]
    fn a_name_that_starts_with_it_comes_before_a_later_word_and_before_an_inner_match() {
        let members = [
            member("1", "Anna Karamazov", Some("anna@example.com")),
            member("2", "Thomas Mann", Some("tmann@example.com")),
            member("3", "Max Planck", Some("max@example.com")),
            member("4", "Samantha Reed", Some("sreed@example.com")),
        ];
        assert_eq!(
            names(&rank_mention_candidates(&members, "ma", "")),
            ["Max Planck", "Thomas Mann", "Anna Karamazov", "Samantha Reed"]
        );
    }

    #[test]
    fn the_asker_and_people_without_an_address_are_never_suggested() {
        let members = [
            member("1", "Ada Lovelace", Some("ada@example.com")),
            member("2", "Former member", None),
            member("3", "Adam Smith", Some("adam@example.com")),
        ];
        assert_eq!(names(&rank_mention_candidates(&members, "ad", "1")), ["Adam Smith"]);
        assert!(names(&rank_mention_candidates(&members, "former", "")).is_empty());
    }
}
