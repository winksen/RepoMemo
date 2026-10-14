use crate::*;

pub(crate) async fn get_profile(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<UserProfileResponse>, ApiError> {
    let profile = state
        .storage
        .get_user_profile(&subject.user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let memberships = state
        .storage
        .workspace_memberships_for_user(&subject.user_id)
        .await
        .map_err(ApiError::internal)?;
    let activity = state
        .storage
        .user_activity_by_day(&subject.user_id, &(Utc::now() - ChronoDuration::days(364)).to_rfc3339())
        .await
        .map_err(map_storage_error)?;
    let today = Utc::now().date_naive();
    let mut activity_by_day = (0..365)
        .rev()
        .map(|offset| ((today - ChronoDuration::days(offset)).to_string(), 0))
        .collect::<BTreeMap<_, _>>();
    for (day, count) in activity {
        if let Some(value) = activity_by_day.get_mut(&day) {
            *value = count;
        }
    }
    let recent_activity_count = activity_by_day.values().sum();
    let email_verified = state
        .storage
        .email_is_verified(&subject.user_id)
        .await
        .map_err(map_storage_error)?;
    let verification_required = !email_verified && crate::verification::enforced(&state).await;
    Ok(Json(UserProfileResponse {
        user: profile.user,
        email_verified,
        verification_required,
        created_at: profile.created_at,
        updated_at: profile.updated_at,
        last_connected_at: profile.last_connected_at,
        workspace_count: memberships.len() as i64,
        recent_activity_count,
        activity_by_day: metric_timeline(activity_by_day),
    }))
}

pub(crate) async fn list_profile_tasks(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<CollaborationTask>>, ApiError> {
    state
        .storage
        .list_assigned_collaboration_tasks(&subject.user_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn list_notifications(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<SharedNotification>>, ApiError> {
    state
        .storage
        .list_shared_notifications(&subject.user_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn mark_notification_read(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(notification_id): Path<String>,
) -> Result<Json<SharedNotification>, ApiError> {
    state
        .storage
        .mark_shared_notification_read(&notification_id, &subject.user_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn mark_all_notifications_read(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .mark_all_shared_notifications_read(&subject.user_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn update_profile(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<UpdateProfileRequest>,
) -> Result<Json<SharedUser>, ApiError> {
    if request.display_name.trim().is_empty() || request.display_name.trim().len() > 120 {
        return Err(ApiError::bad_request(
            "Display name must be between 1 and 120 characters.",
        ));
    }
    state
        .storage
        .update_user_display_name(&subject.user_id, &request.display_name)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn notify_task_assignee(state: &AppState, actor_user_id: &str, task: &CollaborationTask) {
    let Some(assignee) = task.assignee.as_ref() else {
        return;
    };
    if assignee.id == actor_user_id {
        return;
    }
    create_notification(
        state,
        NewSharedNotification {
            user_id: assignee.id.clone(),
            workspace_id: Some(task.workspace_id.clone()),
            notification_type: "task_assigned".to_owned(),
            title: "Task assigned to you".to_owned(),
            body: format!("{} was assigned to you.", task.title),
            href: format!("/workspaces/{}/tasks", task.workspace_id),
        },
    )
    .await;
    crate::mail::mail_notification(
        state,
        crate::mail::NotificationMail {
            kind: "task_assigned",
            actor_id: actor_user_id,
            recipient_id: &assignee.id,
            action: "assigned you the task",
            subject_title: &task.title,
            path: &format!("/workspaces/{}/tasks", task.workspace_id),
        },
    )
    .await;
}

pub(crate) async fn notify_artifact_mentions(
    state: &AppState,
    actor_user_id: &str,
    workspace_id: &str,
    artifact_id: &str,
    artifact_title: &str,
    body: &str,
) {
    let members = match state.storage.list_workspace_members(workspace_id).await {
        Ok(members) => members,
        Err(error) => {
            tracing::error!(error = %error, workspace_id, "Failed to resolve evidence comment mentions");
            return;
        }
    };
    let mentioned_emails = body
        .split_whitespace()
        .filter_map(|token| {
            token
                .trim_matches(|character: char| {
                    matches!(
                        character,
                        ',' | '.' | ':' | ';' | '!' | '?' | ')' | ']' | '}' | '"' | '\''
                    )
                })
                .strip_prefix('@')
                .map(|email| email.to_ascii_lowercase())
        })
        .collect::<BTreeSet<_>>();
    for member in members {
        let Some(email) = member.user.email.as_deref() else {
            continue;
        };
        if member.user.id == actor_user_id || !mentioned_emails.contains(&email.to_ascii_lowercase()) {
            continue;
        }
        let path = format!("/workspaces/{workspace_id}/artifacts/{artifact_id}");
        create_notification(
            state,
            NewSharedNotification {
                user_id: member.user.id.clone(),
                workspace_id: Some(workspace_id.to_owned()),
                notification_type: "evidence_mention".to_owned(),
                title: "You were mentioned in evidence discussion".to_owned(),
                body: format!("You were mentioned on {}.", artifact_title),
                href: path.clone(),
            },
        )
        .await;
        crate::mail::mail_notification(
            state,
            crate::mail::NotificationMail {
                kind: "evidence_mention",
                actor_id: actor_user_id,
                recipient_id: &member.user.id,
                action: "mentioned you on",
                subject_title: artifact_title,
                path: &path,
            },
        )
        .await;
    }
}

pub(crate) async fn create_notification(state: &AppState, notification: NewSharedNotification) {
    if let Err(error) = state.storage.create_shared_notification(notification).await {
        tracing::error!(error = %error, "Failed to create shared notification");
    }
}
