//! Review ownership (V0.6.6): who is accountable for reviewing a file or a memory card, and when it is next due.
//! The reviewer is a workspace member who can write. Anyone who can write may assign an item nobody reviews yet;
//! changing an existing reviewer needs that reviewer or a workspace administrator, so a review cannot be taken away
//! quietly.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use repomemo_domain::{ReviewAssignmentView, WorkspaceRole};
use repomemo_storage::ReviewAssignment;
use serde::Deserialize;

use crate::{
    map_storage_error, record_workspace_activity, require_workspace_read, require_workspace_write, ApiError, AppState,
    AuthenticatedSubject,
};

#[derive(Debug, Deserialize)]
pub(crate) struct ItemQuery {
    kind: String,
    id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AssignRequest {
    kind: String,
    id: String,
    #[serde(default)]
    reviewer_user_id: Option<String>,
    /// A date (`2026-12-01`) or an instant; `None` leaves the item without a due date.
    #[serde(default)]
    next_review_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CompleteRequest {
    kind: String,
    id: String,
    #[serde(default)]
    next_review_at: Option<String>,
}

fn view(kind: &str, id: &str, found: Option<ReviewAssignment>) -> ReviewAssignmentView {
    let now = Utc::now().to_rfc3339();
    match found {
        Some(found) => ReviewAssignmentView {
            kind: kind.to_owned(),
            item_id: id.to_owned(),
            overdue: found.next_review_at.as_deref().is_some_and(|due| due <= now.as_str()),
            reviewer: found.reviewer,
            reviewer_departed: found.reviewer_departed,
            reviewer_label: found.reviewer_label,
            next_review_at: found.next_review_at,
            last_reviewed_at: found.last_reviewed_at,
        },
        None => ReviewAssignmentView {
            kind: kind.to_owned(),
            item_id: id.to_owned(),
            reviewer: None,
            reviewer_departed: false,
            reviewer_label: None,
            next_review_at: None,
            last_reviewed_at: None,
            overdue: false,
        },
    }
}

/// A date or an instant as the instant stored (start of the day, UTC, for a date).
fn normalize_due(value: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if let Ok(day) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        let start = day.and_hms_opt(0, 0, 0).expect("midnight exists").and_utc();
        return Ok(Some(start.to_rfc3339()));
    }
    DateTime::parse_from_rfc3339(value)
        .map(|instant| Some(instant.with_timezone(&Utc).to_rfc3339()))
        .map_err(|_| ApiError::bad_request("The review date must be a date such as 2026-12-01."))
}

/// The workspace an item is in, or the same refusal for an item that is missing and one in another workspace.
async fn item_in_workspace(state: &AppState, workspace_id: &str, kind: &str, id: &str) -> Result<(), ApiError> {
    let inside = match kind {
        "evidence" => state
            .storage
            .get_artifact_summary(id)
            .await
            .map(|summary| summary.workspace_id == workspace_id),
        "memory_card" => state
            .storage
            .get_memory_card(id)
            .await
            .map(|detail| detail.card.workspace_id == workspace_id),
        _ => return Err(ApiError::bad_request("The kind is evidence or memory_card.")),
    };
    match inside {
        Ok(true) => Ok(()),
        _ => Err(ApiError::not_found("That item was not found in this workspace.")),
    }
}

pub(crate) async fn get_assignment(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<ItemQuery>,
) -> Result<Json<ReviewAssignmentView>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    item_in_workspace(&state, &workspace_id, &query.kind, &query.id).await?;
    let found = state
        .storage
        .review_assignment(&query.kind, &query.id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(view(&query.kind, &query.id, found)))
}

pub(crate) async fn assign(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<AssignRequest>,
) -> Result<Json<ReviewAssignmentView>, ApiError> {
    let role = require_workspace_write(&state, &subject, &workspace_id).await?;
    item_in_workspace(&state, &workspace_id, &request.kind, &request.id).await?;
    let reviewer = request
        .reviewer_user_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    if let Some(reviewer) = reviewer {
        let reviewer_role = state
            .storage
            .workspace_role_for_user(reviewer, &workspace_id)
            .await
            .map_err(map_storage_error)?;
        if matches!(reviewer_role, None | Some(WorkspaceRole::Viewer)) {
            return Err(ApiError::bad_request(
                "A reviewer must be a workspace member who can edit.",
            ));
        }
    }
    let current = state
        .storage
        .review_assignment(&request.kind, &request.id)
        .await
        .map_err(map_storage_error)?;
    if let Some(existing) = current.as_ref().and_then(|found| found.reviewer.as_ref()) {
        let may_change = existing.id == subject.user_id
            || matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin)
            || current.as_ref().is_some_and(|found| found.reviewer_departed)
            || reviewer == Some(existing.id.as_str());
        if !may_change {
            return Err(ApiError::forbidden());
        }
    }
    let due = normalize_due(request.next_review_at.as_deref())?;
    state
        .storage
        .set_review_assignment(
            &workspace_id,
            &request.kind,
            &request.id,
            reviewer,
            due.as_deref(),
            &subject.user_id,
        )
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "review_assigned",
        &request.kind,
        Some(&request.id),
        "Set who reviews this and when.".to_owned(),
    )
    .await;
    let found = state
        .storage
        .review_assignment(&request.kind, &request.id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(view(&request.kind, &request.id, found)))
}

/// Records that the item was reviewed now. Only its reviewer, or an administrator, may say so.
pub(crate) async fn complete(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<CompleteRequest>,
) -> Result<Json<ReviewAssignmentView>, ApiError> {
    let role = require_workspace_write(&state, &subject, &workspace_id).await?;
    item_in_workspace(&state, &workspace_id, &request.kind, &request.id).await?;
    let current = state
        .storage
        .review_assignment(&request.kind, &request.id)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(|| ApiError::not_found("Nobody is assigned to review that yet."))?;
    let is_reviewer = current.reviewer.as_ref().is_some_and(|who| who.id == subject.user_id);
    if !is_reviewer && !matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin) {
        return Err(ApiError::forbidden());
    }
    let due = normalize_due(request.next_review_at.as_deref())?;
    state
        .storage
        .complete_review(&request.kind, &request.id, due.as_deref())
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "review_completed",
        &request.kind,
        Some(&request.id),
        "Marked this as reviewed.".to_owned(),
    )
    .await;
    let found = state
        .storage
        .review_assignment(&request.kind, &request.id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(view(&request.kind, &request.id, found)))
}
