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
    let mut tasks = state
        .storage
        .list_assigned_collaboration_tasks(&subject.user_id)
        .await
        .map_err(map_storage_error)?;
    tasks.retain(|task| subject.token_reaches(&task.workspace_id));
    Ok(Json(tasks))
}

pub(crate) async fn list_notifications(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<SharedNotification>>, ApiError> {
    // Notices name things in any workspace, so a token limited to some workspaces is not given them.
    if subject.token.as_ref().is_some_and(|grant| grant.workspaces.is_some()) {
        return Ok(Json(Vec::new()));
    }
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

/// The kinds of notice a person chooses a channel for. Security notices
/// (password resets) always reach the app and the mail: they are not here.
const CHOOSABLE_KINDS: [(&str, &str); 3] = [
    ("task_assigned", "A task is assigned to you"),
    (
        "evidence_mention",
        "You are mentioned in a comment, a task or a memory card",
    ),
    (
        "daily_digest",
        "A summary by mail of what you have not read, once a day",
    ),
];

/// The digest is off until asked for; everything else starts as both.
fn default_channel(kind: &str) -> &'static str {
    if kind == "daily_digest" {
        "none"
    } else {
        "both"
    }
}

/// Where a notice of `kind` goes for this person: `(in the app, by mail)`.
async fn delivery(state: &AppState, user_id: &str, kind: &str) -> (bool, bool) {
    if !CHOOSABLE_KINDS.iter().any(|(known, _)| *known == kind) || kind == "daily_digest" {
        return (true, true);
    }
    match state.storage.notification_channel(user_id, kind).await.as_deref() {
        Ok("app") => (true, false),
        Ok("mail") => (false, true),
        Ok("none") => (false, false),
        _ => (true, true),
    }
}

#[derive(serde::Serialize)]
pub(crate) struct NotificationPreference {
    kind: &'static str,
    label: &'static str,
    /// `both`, `app`, `mail` or `none`.
    channel: String,
}

/// What a person chose for each kind of notice (the default is both).
pub(crate) async fn get_notification_preferences(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<NotificationPreference>>, ApiError> {
    let chosen = state
        .storage
        .notification_preferences(&subject.user_id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(
        CHOOSABLE_KINDS
            .iter()
            .map(|(kind, label)| NotificationPreference {
                kind,
                label,
                channel: chosen
                    .get(*kind)
                    .cloned()
                    .unwrap_or_else(|| default_channel(kind).to_owned()),
            })
            .collect(),
    ))
}

#[derive(serde::Deserialize)]
pub(crate) struct SetNotificationPreference {
    kind: String,
    channel: String,
}

pub(crate) async fn set_notification_preference(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<SetNotificationPreference>,
) -> Result<Json<Vec<NotificationPreference>>, ApiError> {
    if !CHOOSABLE_KINDS.iter().any(|(kind, _)| *kind == request.kind) {
        return Err(ApiError::bad_request("That kind of notice cannot be chosen."));
    }
    if request.kind == "daily_digest" && !matches!(request.channel.as_str(), "mail" | "none") {
        return Err(ApiError::bad_request(
            "The daily summary is sent by mail or not at all.",
        ));
    }
    state
        .storage
        .set_notification_preference(&subject.user_id, &request.kind, &request.channel)
        .await
        .map_err(map_storage_error)?;
    // A digest asked for now should not wait for the worker's next look.
    state.mail.nudge();
    get_notification_preferences(subject, State(state)).await
}

pub(crate) async fn notify_task_assignee(state: &AppState, actor_user_id: &str, task: &CollaborationTask) {
    let Some(assignee) = task.assignee.as_ref() else {
        return;
    };
    if assignee.id == actor_user_id {
        return;
    }
    let (in_app, by_mail) = delivery(state, &assignee.id, "task_assigned").await;
    if in_app {
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
    }
    if by_mail {
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
}

/// Where a mention was written. It decides the words of the notice, its type and where it leads (V0.3.9).
pub(crate) enum MentionedIn<'a> {
    /// A comment on a file.
    Artifact { id: &'a str, title: &'a str },
    /// The description of a task.
    Task { title: &'a str },
    /// The text of a memory card.
    MemoryCard { id: &'a str, title: &'a str },
}

/// The addresses written after an `@` in a text, in lower case.
pub(crate) fn mentioned_addresses(text: &str) -> BTreeSet<String> {
    text.split_whitespace()
        .filter_map(|token| {
            token
                .trim_matches(|character: char| {
                    matches!(
                        character,
                        ',' | '.' | ':' | ';' | '!' | '?' | ')' | ']' | '}' | '"' | '\''
                    )
                })
                .strip_prefix('@')
                .filter(|address| !address.is_empty())
                .map(|address| address.to_ascii_lowercase())
        })
        .collect()
}

pub(crate) async fn notify_artifact_mentions(
    state: &AppState,
    actor_user_id: &str,
    workspace_id: &str,
    artifact_id: &str,
    artifact_title: &str,
    body: &str,
) {
    notify_mentions(
        state,
        actor_user_id,
        workspace_id,
        MentionedIn::Artifact {
            id: artifact_id,
            title: artifact_title,
        },
        body,
        "",
    )
    .await;
}

/// Tells the members of a workspace that a text mentions them. Only direct members of the workspace
/// are told (an organization member who is not in the workspace is neither suggested nor notified), never
/// the person who wrote it, and, when `earlier` is the text as it was before this save, nobody it already
/// mentioned. One preference ("You are mentioned") covers every place.
pub(crate) async fn notify_mentions(
    state: &AppState,
    actor_user_id: &str,
    workspace_id: &str,
    place: MentionedIn<'_>,
    text: &str,
    earlier: &str,
) {
    let mentioned_emails = mentioned_addresses(text);
    if mentioned_emails.is_empty() {
        return;
    }
    let already = mentioned_addresses(earlier);
    let members = match state.storage.list_workspace_members(workspace_id).await {
        Ok(members) => members,
        Err(error) => {
            tracing::error!(error = %error, workspace_id, "Failed to resolve mentions");
            return;
        }
    };
    let (notification_type, title, subject_title, action, path) = match place {
        MentionedIn::Artifact { id, title } => (
            "evidence_mention",
            "You were mentioned in evidence discussion",
            title,
            "mentioned you on",
            format!("/workspaces/{workspace_id}/artifacts/{id}"),
        ),
        MentionedIn::Task { title } => (
            "task_mention",
            "You were mentioned in a task",
            title,
            "mentioned you in the task",
            format!("/workspaces/{workspace_id}/tasks"),
        ),
        MentionedIn::MemoryCard { id, title } => (
            "memory_mention",
            "You were mentioned in team memory",
            title,
            "mentioned you in the memory card",
            format!("/workspaces/{workspace_id}/memory-cards/{id}"),
        ),
    };
    for member in members {
        let Some(email) = member.user.email.as_deref() else {
            continue;
        };
        let email = email.to_ascii_lowercase();
        if member.user.id == actor_user_id || !mentioned_emails.contains(&email) || already.contains(&email) {
            continue;
        }
        let (in_app, by_mail) = delivery(state, &member.user.id, "evidence_mention").await;
        if in_app {
            create_notification(
                state,
                NewSharedNotification {
                    user_id: member.user.id.clone(),
                    workspace_id: Some(workspace_id.to_owned()),
                    notification_type: notification_type.to_owned(),
                    title: title.to_owned(),
                    body: format!("You were mentioned on {subject_title}."),
                    href: path.clone(),
                },
            )
            .await;
        }
        if by_mail {
            crate::mail::mail_notification(
                state,
                crate::mail::NotificationMail {
                    kind: notification_type,
                    actor_id: actor_user_id,
                    recipient_id: &member.user.id,
                    action,
                    subject_title,
                    path: &path,
                },
            )
            .await;
        }
    }
}

pub(crate) async fn create_notification(state: &AppState, notification: NewSharedNotification) {
    if let Err(error) = state.storage.create_shared_notification(notification).await {
        tracing::error!(error = %error, "Failed to create shared notification");
    }
}

#[cfg(test)]
mod tests {
    use super::mentioned_addresses;

    #[test]
    fn addresses_are_read_after_an_at_sign_and_trimmed_of_punctuation() {
        let found =
            mentioned_addresses("Ask @Ada@Example.com, then @grace@example.com). Not a@b.com, nor @ alone, nor @.");
        assert_eq!(
            found.into_iter().collect::<Vec<_>>(),
            ["ada@example.com", "grace@example.com"]
        );
        assert!(mentioned_addresses("").is_empty());
    }
}
