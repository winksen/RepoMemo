//! Search feedback (V0.6.25): a person tells the workspace's owners that a question went unanswered or a result did not
//! help, and an owner or administrator answers it with evidence or a memory card. Anyone who can read the workspace can
//! ask and sees their own requests; administrators see all of them. The query text is kept for a stated time only.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use repomemo_domain::{SearchFeedback, WorkspaceRole};
use repomemo_storage::{StoredSearchFeedback, CLOSED_RETENTION_DAYS, OPEN_RETENTION_DAYS};
use serde::Deserialize;

use crate::{
    map_storage_error, record_workspace_activity, require_workspace_admin, require_workspace_read, ApiError, AppState,
    AuthenticatedSubject, ManagingSubject,
};

const MAX_QUERY_CHARS: usize = 512;
const MAX_NOTE_CHARS: usize = 500;

#[derive(Debug, Deserialize)]
pub(crate) struct SubmitFeedback {
    /// `no_answer` or `unhelpful_result`.
    kind: String,
    query: String,
    /// For `unhelpful_result`: the file whose result did not help.
    #[serde(default)]
    artifact_id: Option<String>,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ResolveFeedback {
    /// `evidence` or `memory_card` (the request is answered by one) or `declined`.
    resolution: String,
    #[serde(default)]
    target_id: Option<String>,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FeedbackFilter {
    status: Option<String>,
}

fn view(stored: StoredSearchFeedback) -> SearchFeedback {
    // When the request, and the query text in it, is removed.
    let expires = if stored.status == "open" {
        DateTime::parse_from_rfc3339(&stored.created_at).map(|at| at + Duration::days(OPEN_RETENTION_DAYS))
    } else {
        DateTime::parse_from_rfc3339(stored.resolved_at.as_deref().unwrap_or(&stored.created_at))
            .map(|at| at + Duration::days(CLOSED_RETENTION_DAYS))
    }
    .map(|at| at.with_timezone(&Utc).to_rfc3339())
    .unwrap_or_default();
    SearchFeedback {
        id: stored.id,
        kind: stored.kind,
        query: stored.query,
        artifact_id: stored.artifact_id,
        artifact_title: stored.artifact_title,
        note: stored.note,
        status: stored.status,
        asked_by: stored.user,
        created_at: stored.created_at,
        resolution_kind: stored.resolution_kind,
        resolution_id: stored.resolution_id,
        resolution_title: stored.resolution_title,
        resolved_by: stored.resolved_by,
        resolved_note: stored.resolved_note,
        resolved_at: stored.resolved_at,
        expires_at: expires,
    }
}

/// Sends a question or a result to the people who can improve the evidence. The search itself is not touched, so the person
/// keeps their query and results on screen.
pub(crate) async fn submit_search_feedback(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SubmitFeedback>,
) -> Result<(StatusCode, Json<SearchFeedback>), ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    if !matches!(request.kind.as_str(), "no_answer" | "unhelpful_result" | "wrong_answer") {
        return Err(ApiError::bad_request(
            "The kind must be no_answer, unhelpful_result or wrong_answer.",
        ));
    }
    // A correction says what was wrong; without it the people who can fix the evidence have nothing to act on.
    if request.kind == "wrong_answer" && request.note.trim().chars().count() < 3 {
        return Err(ApiError::bad_request(
            "Say what was wrong with the answer, so the evidence can be corrected.",
        ));
    }
    let query = request.query.trim();
    if query.is_empty() || query.chars().count() > MAX_QUERY_CHARS {
        return Err(ApiError::bad_request(format!(
            "Say what you looked for, in at most {MAX_QUERY_CHARS} characters."
        )));
    }
    if request.note.chars().count() > MAX_NOTE_CHARS {
        return Err(ApiError::bad_request(format!(
            "The note is limited to {MAX_NOTE_CHARS} characters."
        )));
    }
    let stored = state
        .storage
        .submit_search_feedback(
            &workspace_id,
            &subject.user_id,
            &request.kind,
            query,
            request.artifact_id.as_deref(),
            request.note.trim(),
        )
        .await
        .map_err(map_storage_error)?;
    // The source an answer was built on is flagged for its reviewers when the answer is reported wrong.
    if request.kind == "wrong_answer" {
        if let Some(artifact_id) = request.artifact_id.as_deref() {
            let flagged = state
                .storage
                .flag_answer_source(
                    &workspace_id,
                    artifact_id,
                    "An answer built on this was reported as wrong.",
                )
                .await
                .map_err(map_storage_error)?;
            if flagged {
                record_workspace_activity(
                    &state,
                    &workspace_id,
                    &subject.user_id,
                    "answer_reported_wrong",
                    "artifact",
                    Some(artifact_id),
                    "Reported an answer built on a verified file as wrong; the file needs review.".to_owned(),
                )
                .await;
            }
        }
    }
    Ok((StatusCode::CREATED, Json(view(stored))))
}

/// The requests the caller may see: all of a workspace's for an owner or administrator, their own for anyone else.
pub(crate) async fn list_search_feedback(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(filter): Query<FeedbackFilter>,
) -> Result<Json<Vec<SearchFeedback>>, ApiError> {
    let role = require_workspace_read(&state, &subject, &workspace_id).await?;
    let is_admin = matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin);
    if filter
        .status
        .as_deref()
        .is_some_and(|status| !matches!(status, "open" | "resolved" | "declined"))
    {
        return Err(ApiError::bad_request("The status must be open, resolved or declined."));
    }
    let only_user = (!is_admin).then_some(subject.user_id.as_str());
    let found = state
        .storage
        .list_search_feedback(&workspace_id, only_user, filter.status.as_deref())
        .await
        .map_err(map_storage_error)?;
    Ok(Json(found.into_iter().map(view).collect()))
}

/// Answers a request with a file or a memory card of the workspace, or declines it. Only an administrator can.
pub(crate) async fn resolve_search_feedback(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path((workspace_id, feedback_id)): Path<(String, String)>,
    Json(request): Json<ResolveFeedback>,
) -> Result<Json<SearchFeedback>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let (kind, target) = match request.resolution.as_str() {
        "declined" => (None, None),
        "evidence" | "memory_card" => {
            let target = request
                .target_id
                .as_deref()
                .filter(|id| !id.trim().is_empty())
                .ok_or_else(|| ApiError::bad_request("Say which evidence or memory card answers it."))?;
            (Some(request.resolution.as_str()), Some(target))
        }
        _ => {
            return Err(ApiError::bad_request(
                "The resolution must be evidence, memory_card or declined.",
            ))
        }
    };
    if request.note.chars().count() > MAX_NOTE_CHARS {
        return Err(ApiError::bad_request(format!(
            "The note is limited to {MAX_NOTE_CHARS} characters."
        )));
    }
    let stored = state
        .storage
        .resolve_search_feedback(
            &workspace_id,
            &feedback_id,
            &subject.user_id,
            kind,
            target,
            request.note.trim(),
        )
        .await
        .map_err(map_storage_error)?;
    // The activity feed names the outcome, never the query text.
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "search_feedback_answered",
        "search_feedback",
        Some(&stored.id),
        match kind {
            Some("memory_card") => "Answered a search request with a memory card.".to_owned(),
            Some(_) => "Answered a search request with evidence.".to_owned(),
            None => "Declined a search request.".to_owned(),
        },
    )
    .await;
    Ok(Json(view(stored)))
}
