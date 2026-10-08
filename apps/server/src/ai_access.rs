//! Who may use AI in a workspace, and how often.
//!
//! AI features can send workspace excerpts to a paid cloud provider, so each
//! workspace decides which roles may trigger them (its AI policy, kept in the
//! workspace settings), and every user has an hourly AI quota set by the
//! server operator. Background work (image descriptions, embeddings) is not
//! affected: it runs on behalf of the workspace, not of a request.

use axum::{
    extract::{Path, State},
    Json,
};
use repomemo_domain::WorkspaceRole;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    map_storage_error, record_workspace_activity, require_workspace_admin, require_workspace_read,
    security::{too_many_requests, wait_text},
    ApiError, AppState, AuthenticatedSubject,
};

/// Key of the policy in the workspace settings object.
const AI_POLICY_SETTING: &str = "ai_min_role";

/// The lowest workspace role allowed to use AI features.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AiMinRole {
    /// Everyone in the workspace, viewers included (the default).
    #[default]
    Viewer,
    /// Owners, administrators and members.
    Member,
    /// Owners and administrators only.
    Admin,
}

impl AiMinRole {
    pub(crate) fn allows(self, role: &WorkspaceRole) -> bool {
        match self {
            AiMinRole::Viewer => true,
            AiMinRole::Member => !matches!(role, WorkspaceRole::Viewer),
            AiMinRole::Admin => matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin),
        }
    }

    fn label(self) -> &'static str {
        match self {
            AiMinRole::Viewer => "everyone in the workspace",
            AiMinRole::Member => "members, administrators and owners",
            AiMinRole::Admin => "administrators and owners",
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct AiPolicyResponse {
    min_role: AiMinRole,
    /// AI requests each user may make per hour on this server; 0 is unlimited.
    requests_per_hour: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateAiPolicyRequest {
    min_role: AiMinRole,
}

pub(crate) async fn workspace_ai_min_role(
    state: &AppState,
    workspace_id: &str,
) -> Result<AiMinRole, ApiError> {
    let settings = state
        .storage
        .workspace_settings(workspace_id)
        .await
        .map_err(map_storage_error)?;
    Ok(settings
        .get(AI_POLICY_SETTING)
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default())
}

/// Whether `role` may use AI in the workspace, per its policy.
pub(crate) async fn role_may_use_ai(
    state: &AppState,
    workspace_id: &str,
    role: &WorkspaceRole,
) -> Result<bool, ApiError> {
    Ok(workspace_ai_min_role(state, workspace_id).await?.allows(role))
}

/// The message shown when the policy keeps AI from a role.
pub(crate) fn ai_not_allowed_message(min_role: AiMinRole) -> String {
    format!(
        "In this workspace, AI features are available to {} only. Nothing was sent to the AI provider.",
        min_role.label()
    )
}

/// Checks the workspace policy and counts one request against the user's
/// hourly AI quota. Call right before work that calls an AI provider.
pub(crate) async fn authorize_ai_use(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: &str,
    role: &WorkspaceRole,
) -> Result<(), ApiError> {
    let min_role = workspace_ai_min_role(state, workspace_id).await?;
    if !min_role.allows(role) {
        return Err(ApiError::forbidden_because(ai_not_allowed_message(min_role)));
    }
    consume_ai_quota(state, subject)
}

/// Counts one AI request against the user's hourly quota.
pub(crate) fn consume_ai_quota(state: &AppState, subject: &AuthenticatedSubject) -> Result<(), ApiError> {
    state
        .guards
        .ai
        .check(&subject.user_id, state.settings.ai_quota)
        .map_err(|wait| {
            tracing::warn!(target: "audit", user_id = %subject.user_id, "AI quota used up");
            too_many_requests(
                format!(
                    "You have reached the limit of {} AI requests per hour on this server. Try again in {}.",
                    state.settings.ai_quota.limit,
                    wait_text(wait)
                ),
                wait,
            )
        })
}

pub(crate) async fn get_ai_policy(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<AiPolicyResponse>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    Ok(Json(AiPolicyResponse {
        min_role: workspace_ai_min_role(&state, &workspace_id).await?,
        requests_per_hour: state.settings.ai_quota.limit,
    }))
}

pub(crate) async fn update_ai_policy(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<UpdateAiPolicyRequest>,
) -> Result<Json<AiPolicyResponse>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    state
        .storage
        .set_workspace_setting(&workspace_id, AI_POLICY_SETTING, json!(request.min_role))
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "ai_policy_updated",
        "workspace",
        Some(&workspace_id),
        format!("AI features are now available to {}.", request.min_role.label()),
    )
    .await;
    Ok(Json(AiPolicyResponse {
        min_role: request.min_role,
        requests_per_hour: state.settings.ai_quota.limit,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policies_admit_roles_from_the_minimum_up() {
        let roles = [
            WorkspaceRole::Owner,
            WorkspaceRole::Admin,
            WorkspaceRole::Member,
            WorkspaceRole::Viewer,
        ];
        let allowed = |policy: AiMinRole| roles.iter().filter(|role| policy.allows(role)).count();
        assert_eq!(allowed(AiMinRole::Viewer), 4);
        assert_eq!(allowed(AiMinRole::Member), 3);
        assert_eq!(allowed(AiMinRole::Admin), 2);
        assert_eq!(serde_json::to_value(AiMinRole::Member).unwrap(), json!("member"));
    }
}
