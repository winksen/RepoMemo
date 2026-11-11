//! Approval of a memory card revision (V0.6.8). An approval belongs to one revision, so any edit (which makes a new
//! revision) leaves the card unapproved. When the workspace asks for independent review, the person who made a revision
//! cannot approve it, administrators included. If the card has an accountable reviewer, only that reviewer or a workspace
//! administrator decides; otherwise anyone who can edit may.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_domain::{MemoryApprovalDecision, MemoryApprovalStatus, WorkspaceRole};
use serde::{Deserialize, Serialize};

use crate::{
    map_storage_error, record_workspace_activity, require_workspace_admin, require_workspace_read,
    require_workspace_write, ApiError, AppState, AuthenticatedSubject,
};

const POLICY_SETTING: &str = "independent_review";

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Policy {
    independent_review: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApproveRequest {
    /// The revision the reviewer read; the approval is refused if the card has moved on.
    revision: i64,
    #[serde(default)]
    note: String,
}

async fn independent(state: &AppState, workspace_id: &str) -> Result<bool, ApiError> {
    let settings = state
        .storage
        .workspace_settings(workspace_id)
        .await
        .map_err(map_storage_error)?;
    Ok(settings
        .get(POLICY_SETTING)
        .and_then(|value| value.as_bool())
        .unwrap_or(false))
}

async fn card_workspace(state: &AppState, card_id: &str) -> Result<String, ApiError> {
    state
        .storage
        .get_memory_card(card_id)
        .await
        .map(|detail| detail.card.workspace_id)
        .map_err(|_| ApiError::not_found("Memory card was not found."))
}

/// The status as the caller sees it, with whether and why they may approve.
async fn build_status(
    state: &AppState,
    subject: &AuthenticatedSubject,
    card_id: &str,
    workspace_id: &str,
    role: &WorkspaceRole,
) -> Result<MemoryApprovalStatus, ApiError> {
    let history = state
        .storage
        .memory_approval_history(card_id)
        .await
        .map_err(map_storage_error)?;
    let revisions = state
        .storage
        .memory_revisions(card_id)
        .await
        .map_err(map_storage_error)?;
    let newest = revisions.first();
    let revision = newest.map_or(0, |found| found.revision);
    let current = history.iter().find(|decision| decision.revision == revision);
    let approved = current.is_some_and(|decision| decision.decision == "approved");
    let independent_review = independent(state, workspace_id).await?;
    let is_admin = matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin);
    let assignment = state
        .storage
        .review_assignment("memory_card", card_id)
        .await
        .map_err(map_storage_error)?;
    let reviewer = assignment
        .as_ref()
        .filter(|found| !found.reviewer_departed)
        .and_then(|found| found.reviewer.as_ref());
    let reason = if matches!(role, WorkspaceRole::Viewer) {
        Some("You can read this card but not decide on it.")
    } else if reviewer.is_some_and(|who| who.id != subject.user_id) && !is_admin {
        Some("Only the card's reviewer or a workspace administrator can approve it.")
    } else if independent_review
        && newest
            .and_then(|found| found.author.as_ref())
            .is_some_and(|author| author.id == subject.user_id)
    {
        Some("This workspace asks for independent review: you made this revision, so someone else must approve it.")
    } else if approved {
        Some("This revision is already approved.")
    } else {
        None
    };
    Ok(MemoryApprovalStatus {
        revision,
        approved,
        approved_by: current.filter(|_| approved).and_then(|decision| decision.user.clone()),
        approved_at: current.filter(|_| approved).map(|decision| decision.created_at.clone()),
        independent_review,
        can_approve: reason.is_none(),
        cannot_approve_reason: reason.map(str::to_owned),
        history: history
            .into_iter()
            .map(|decision| MemoryApprovalDecision {
                revision: decision.revision,
                decision: decision.decision,
                user: decision.user,
                note: decision.note,
                created_at: decision.created_at,
            })
            .collect(),
    })
}

pub(crate) async fn status(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(card_id): Path<String>,
) -> Result<Json<MemoryApprovalStatus>, ApiError> {
    let workspace_id = card_workspace(&state, &card_id).await?;
    let role = require_workspace_read(&state, &subject, &workspace_id).await?;
    Ok(Json(
        build_status(&state, &subject, &card_id, &workspace_id, &role).await?,
    ))
}

pub(crate) async fn approve(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(card_id): Path<String>,
    Json(request): Json<ApproveRequest>,
) -> Result<Json<MemoryApprovalStatus>, ApiError> {
    let workspace_id = card_workspace(&state, &card_id).await?;
    let role = require_workspace_write(&state, &subject, &workspace_id).await?;
    let before = build_status(&state, &subject, &card_id, &workspace_id, &role).await?;
    if before.revision != request.revision {
        return Err(ApiError::conflict(format!(
            "This card is now at revision {}; you read revision {}. Read the newer text first.",
            before.revision, request.revision
        )));
    }
    if let Some(reason) = before.cannot_approve_reason {
        return Err(if before.approved {
            ApiError::conflict(reason)
        } else {
            ApiError::new(StatusCode::FORBIDDEN, "forbidden", reason)
        });
    }
    if note_too_long(&request.note) {
        return Err(ApiError::bad_request("The note is too long."));
    }
    state
        .storage
        .record_memory_approval(&card_id, request.revision, "approved", &subject.user_id, &request.note)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "memory_approved",
        "memory_card",
        Some(&card_id),
        format!("Approved revision {} of a memory card.", request.revision),
    )
    .await;
    Ok(Json(
        build_status(&state, &subject, &card_id, &workspace_id, &role).await?,
    ))
}

fn note_too_long(note: &str) -> bool {
    note.chars().count() > 1_000
}

/// Withdraws the approval of the newest revision. The person who approved, or an administrator, may.
pub(crate) async fn withdraw(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(card_id): Path<String>,
) -> Result<Json<MemoryApprovalStatus>, ApiError> {
    let workspace_id = card_workspace(&state, &card_id).await?;
    let role = require_workspace_write(&state, &subject, &workspace_id).await?;
    let before = build_status(&state, &subject, &card_id, &workspace_id, &role).await?;
    if !before.approved {
        return Err(ApiError::conflict("The newest revision is not approved."));
    }
    let is_admin = matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin);
    let by_me = before.approved_by.as_ref().is_some_and(|who| who.id == subject.user_id);
    if !by_me && !is_admin {
        return Err(ApiError::forbidden());
    }
    state
        .storage
        .record_memory_approval(&card_id, before.revision, "withdrawn", &subject.user_id, "")
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "memory_approval_withdrawn",
        "memory_card",
        Some(&card_id),
        format!("Withdrew the approval of revision {}.", before.revision),
    )
    .await;
    Ok(Json(
        build_status(&state, &subject, &card_id, &workspace_id, &role).await?,
    ))
}

pub(crate) async fn get_policy(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Policy>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    Ok(Json(Policy {
        independent_review: independent(&state, &workspace_id).await?,
    }))
}

/// Turns independent review on or off. Existing approvals are not changed; the rule applies to later decisions.
pub(crate) async fn set_policy(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(policy): Json<Policy>,
) -> Result<Json<Policy>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    state
        .storage
        .set_workspace_setting(
            &workspace_id,
            POLICY_SETTING,
            serde_json::Value::Bool(policy.independent_review),
        )
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "approval_policy_changed",
        "workspace",
        Some(&workspace_id),
        if policy.independent_review {
            "Required independent review for approvals.".to_owned()
        } else {
            "Stopped requiring independent review for approvals.".to_owned()
        },
    )
    .await;
    Ok(Json(policy))
}
