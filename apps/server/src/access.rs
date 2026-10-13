//! Getting people in without a mail server, and scripts in without a browser.
//!
//! - **Invitation links** let an organization or workspace administrator add
//!   someone who has no account yet. The link is bound to one email address,
//!   lasts a week, works once, and is checked again when accepted: the person
//!   who sent it must still be allowed to grant that role.
//! - **Password reset links** are issued by a system administrator (or an app
//!   administrator, except for system administrators' accounts). Using one
//!   sets the new password, ends every session and lifts sign-in lockouts.
//! - **Personal access tokens** act as their owner, read-only or with full
//!   rights. They cannot manage tokens, passwords or roles; that needs a
//!   browser session (see `token_may_call` in the extractor).
//!
//! Every secret is shown once and stored only as a hash (see the storage
//! crate's `access` module).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_domain::{Invitation, OrganizationRole, PersonalAccessToken, WorkspaceRole};
use serde::{Deserialize, Serialize};

use crate::{
    check_auth_quota, hash_password, issue_session, map_storage_error, record_workspace_activity,
    require_organization_admin, require_workspace_admin, security::ClientIp, validate_password,
    workspace_role_label, ApiError, AppState, AuthAction, AuthenticatedSubject, TokenResponse,
};

/// How long an invitation link works.
const INVITATION_TTL_DAYS: i64 = 7;
/// How long a password reset link works.
const PASSWORD_RESET_TTL_HOURS: i64 = 24;
/// The lifetimes a personal access token may be given. There are no
/// tokens that never expire.
const TOKEN_TTL_DAYS: [i64; 4] = [7, 30, 90, 365];

fn gone(message: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::GONE, "gone", message)
}

/// Accepting invitations and managing tokens are things a person does, not
/// a script.
fn require_session(subject: &AuthenticatedSubject) -> Result<(), ApiError> {
    if subject.token.is_some() {
        return Err(ApiError::forbidden_because("Sign in in the browser to do this; personal access tokens cannot."));
    }
    Ok(())
}

// ------------------------------------------------------------------ invitations

#[derive(Deserialize)]
pub(crate) struct CreateInvitationRequest {
    email: String,
    role: String,
}

#[derive(Serialize)]
pub(crate) struct CreatedInvitation {
    invitation: Invitation,
    /// The secret, shown once. The web app builds `{its origin}{path}`.
    token: String,
    path: String,
}

fn created(invitation: Invitation, token: String) -> CreatedInvitation {
    let path = format!("/invite/{token}");
    CreatedInvitation { invitation, token, path }
}

fn parse_workspace_role(value: &str) -> Result<WorkspaceRole, ApiError> {
    match value {
        "admin" => Ok(WorkspaceRole::Admin),
        "member" => Ok(WorkspaceRole::Member),
        "viewer" => Ok(WorkspaceRole::Viewer),
        _ => Err(ApiError::bad_request("Invite people as admin, member or viewer.")),
    }
}

fn parse_organization_role(value: &str) -> Result<OrganizationRole, ApiError> {
    match value {
        "admin" => Ok(OrganizationRole::Admin),
        "member" => Ok(OrganizationRole::Member),
        _ => Err(ApiError::bad_request("Invite people as admin or member.")),
    }
}

fn workspace_rank(role: &WorkspaceRole) -> u8 {
    match role {
        WorkspaceRole::Viewer => 0,
        WorkspaceRole::Member => 1,
        WorkspaceRole::Admin => 2,
        WorkspaceRole::Owner => 3,
    }
}

fn organization_rank(role: &OrganizationRole) -> u8 {
    match role {
        OrganizationRole::Member => 0,
        OrganizationRole::Admin => 1,
        OrganizationRole::Owner => 2,
    }
}

fn organization_role_label(role: &OrganizationRole) -> &'static str {
    match role {
        OrganizationRole::Owner => "owner",
        OrganizationRole::Admin => "admin",
        OrganizationRole::Member => "member",
    }
}

/// The same rule as adding a member: only owners grant the admin role.
fn check_workspace_grant(caller: &WorkspaceRole, role: &WorkspaceRole) -> Result<(), ApiError> {
    if matches!(role, WorkspaceRole::Admin) && !matches!(caller, WorkspaceRole::Owner) {
        return Err(ApiError::forbidden_because("Only the workspace owner can invite administrators."));
    }
    Ok(())
}

fn check_organization_grant(caller: &OrganizationRole, role: &OrganizationRole) -> Result<(), ApiError> {
    if matches!(role, OrganizationRole::Admin) && !matches!(caller, OrganizationRole::Owner) {
        return Err(ApiError::forbidden_because("Only the organization owner can invite administrators."));
    }
    Ok(())
}

pub(crate) async fn create_workspace_invitation(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<CreateInvitationRequest>,
) -> Result<(StatusCode, Json<CreatedInvitation>), ApiError> {
    let caller = require_workspace_admin(&state, &subject, &workspace_id).await?;
    let role = parse_workspace_role(request.role.trim())?;
    check_workspace_grant(&caller, &role)?;
    if let Some(user) = state.storage.find_user_by_email(&request.email).await.map_err(map_storage_error)? {
        if state.storage.workspace_role_for_user(&user.id, &workspace_id).await.map_err(map_storage_error)?.is_some() {
            return Err(ApiError::conflict("This person is already a member of this workspace. Change their role in the member list."));
        }
    }
    let (invitation, token) = state
        .storage
        .create_invitation(&request.email, "workspace", &workspace_id, workspace_role_label(&role), &subject.user_id, INVITATION_TTL_DAYS)
        .await
        .map_err(map_storage_error)?;
    tracing::info!(target: "audit", user_id = %subject.user_id, workspace_id = %workspace_id, invitation_id = %invitation.id, "Workspace invitation created");
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "member_invited",
        "invitation",
        Some(&invitation.id),
        format!("Invited {} as {}.", invitation.email, invitation.role),
    )
    .await;
    Ok((StatusCode::CREATED, Json(created(invitation, token))))
}

pub(crate) async fn list_workspace_invitations(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<Invitation>>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_pending_invitations("workspace", &workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn create_organization_invitation(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
    Json(request): Json<CreateInvitationRequest>,
) -> Result<(StatusCode, Json<CreatedInvitation>), ApiError> {
    let caller = require_organization_admin(&state, &subject, &organization_id).await?;
    let role = parse_organization_role(request.role.trim())?;
    check_organization_grant(&caller, &role)?;
    if let Some(user) = state.storage.find_user_by_email(&request.email).await.map_err(map_storage_error)? {
        if state.storage.organization_role_for_user(&user.id, &organization_id).await.map_err(map_storage_error)?.is_some() {
            return Err(ApiError::conflict("This person is already a member of this organization. Change their role in the member list."));
        }
    }
    let (invitation, token) = state
        .storage
        .create_invitation(&request.email, "organization", &organization_id, organization_role_label(&role), &subject.user_id, INVITATION_TTL_DAYS)
        .await
        .map_err(map_storage_error)?;
    tracing::info!(target: "audit", user_id = %subject.user_id, organization_id = %organization_id, invitation_id = %invitation.id, "Organization invitation created");
    Ok((StatusCode::CREATED, Json(created(invitation, token))))
}

pub(crate) async fn list_organization_invitations(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
) -> Result<Json<Vec<Invitation>>, ApiError> {
    require_organization_admin(&state, &subject, &organization_id).await?;
    state
        .storage
        .list_pending_invitations("organization", &organization_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

/// Withdraws a pending invitation; administrators of its target only.
pub(crate) async fn revoke_invitation(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(invitation_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let stored = state
        .storage
        .find_invitation(&invitation_id)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(|| ApiError::not_found("Invitation was not found."))?;
    let invitation = &stored.invitation;
    match invitation.target_type.as_str() {
        "workspace" => {
            require_workspace_admin(&state, &subject, &invitation.target_id).await?;
        }
        _ => {
            require_organization_admin(&state, &subject, &invitation.target_id).await?;
        }
    }
    if !state.storage.revoke_invitation(&invitation_id).await.map_err(map_storage_error)? {
        return Err(ApiError::conflict("This invitation was already used or withdrawn."));
    }
    tracing::info!(target: "audit", user_id = %subject.user_id, invitation_id = %invitation_id, "Invitation withdrawn");
    if invitation.target_type == "workspace" {
        record_workspace_activity(
            &state,
            &invitation.target_id,
            &subject.user_id,
            "invitation_revoked",
            "invitation",
            Some(&invitation_id),
            format!("Withdrew the invitation of {}.", invitation.email),
        )
        .await;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub(crate) struct TokenBody {
    token: String,
}

#[derive(Serialize)]
pub(crate) struct InvitationPreview {
    email: String,
    target_type: String,
    target_id: String,
    target_name: String,
    organization_name: String,
    role: String,
    invited_by: Option<String>,
    expires_at: String,
    /// Whether an account exists for the invited address: sign in to accept,
    /// or create the account first.
    account_exists: bool,
}

/// Loads a usable invitation by its secret, or explains why it cannot be used.
async fn usable_invitation(state: &AppState, token: &str) -> Result<repomemo_storage::StoredInvitation, ApiError> {
    let stored = state
        .storage
        .find_invitation_by_token(token)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(|| ApiError::not_found("This invitation link is not valid. Check that it was copied completely."))?;
    if let Some(reason) = stored.unusable_reason() {
        return Err(gone(reason));
    }
    Ok(stored)
}

/// Public: what an invitation link is for, so the invitee knows what they
/// join before creating an account or signing in.
pub(crate) async fn preview_invitation(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<TokenBody>,
) -> Result<Json<InvitationPreview>, ApiError> {
    check_auth_quota(&state, &client, AuthAction::SignIn)?;
    let stored = usable_invitation(&state, &request.token).await?;
    let invitation = stored.invitation;
    let (target_name, organization_name, _) = state
        .storage
        .invitation_target(&invitation.target_type, &invitation.target_id)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(|| gone("The organization or workspace of this invitation no longer exists."))?;
    let account_exists = state
        .storage
        .find_user_by_email(&invitation.email)
        .await
        .map_err(map_storage_error)?
        .is_some();
    Ok(Json(InvitationPreview {
        email: invitation.email,
        target_type: invitation.target_type,
        target_id: invitation.target_id,
        target_name,
        organization_name,
        role: invitation.role,
        invited_by: invitation.invited_by.map(|user| user.display_name),
        expires_at: invitation.expires_at,
        account_exists,
    }))
}

/// The inviter as they are now, to check they may still grant the role.
async fn inviter_subject(state: &AppState, user_id: &str) -> Result<Option<AuthenticatedSubject>, ApiError> {
    let access = state.storage.user_access(user_id).await.map_err(ApiError::internal)?;
    Ok(access.map(|access| AuthenticatedSubject {
        user_id: user_id.to_owned(),
        expires_at: 0,
        session_version: access.session_version,
        is_system_admin: access.is_system_admin,
        is_app_admin: access.is_app_admin,
        token: None,
    }))
}

/// Whether the person who sent the invitation may still grant its role.
async fn inviter_may_still_grant(state: &AppState, invitation: &Invitation) -> Result<bool, ApiError> {
    let Some(inviter_id) = invitation.invited_by.as_ref().map(|user| user.id.clone()) else {
        return Ok(false);
    };
    let Some(inviter) = inviter_subject(state, &inviter_id).await? else {
        return Ok(false);
    };
    Ok(match invitation.target_type.as_str() {
        "workspace" => {
            let role = parse_workspace_role(&invitation.role)?;
            match require_workspace_admin(state, &inviter, &invitation.target_id).await {
                Ok(caller) => check_workspace_grant(&caller, &role).is_ok(),
                Err(_) => false,
            }
        }
        _ => {
            let role = parse_organization_role(&invitation.role)?;
            match require_organization_admin(state, &inviter, &invitation.target_id).await {
                Ok(caller) => check_organization_grant(&caller, &role).is_ok(),
                Err(_) => false,
            }
        }
    })
}

#[derive(Serialize)]
pub(crate) struct AcceptedInvitation {
    target_type: String,
    target_id: String,
    organization_id: String,
    /// The role held after joining: the invited one, or a higher one the
    /// person already had (accepting never lowers a role).
    role: String,
}

/// Joins the invitation's organization or workspace as the signed-in user,
/// whose email must be the invited one.
pub(crate) async fn accept_invitation(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<TokenBody>,
) -> Result<Json<AcceptedInvitation>, ApiError> {
    require_session(&subject)?;
    let stored = usable_invitation(&state, &request.token).await?;
    let invitation = stored.invitation;
    let user = state
        .storage
        .find_user(&subject.user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let email = user.email.clone().unwrap_or_default().to_lowercase();
    if email != invitation.email {
        return Err(ApiError::forbidden_because(format!(
            "This invitation is for {}. Sign in with that account to accept it.",
            invitation.email
        )));
    }
    let (_, _, organization_id) = state
        .storage
        .invitation_target(&invitation.target_type, &invitation.target_id)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(|| gone("The organization or workspace of this invitation no longer exists."))?;
    if !inviter_may_still_grant(&state, &invitation).await? {
        return Err(gone("The person who invited you can no longer add members with this role. Ask for a new invitation."));
    }
    if !state.storage.mark_invitation_accepted(&invitation.id, &user.id).await.map_err(map_storage_error)? {
        return Err(gone("This invitation has already been used."));
    }
    let joined = join(&state, &invitation, &user.id, &email).await;
    let role = match joined {
        Ok(role) => role,
        Err(error) => {
            // Give the link back so it can be tried again.
            let _ = state.storage.unmark_invitation_accepted(&invitation.id).await;
            return Err(error);
        }
    };
    tracing::info!(target: "audit", user_id = %user.id, invitation_id = %invitation.id, target_type = %invitation.target_type, target_id = %invitation.target_id, "Invitation accepted");
    if invitation.target_type == "workspace" {
        record_workspace_activity(
            &state,
            &invitation.target_id,
            &user.id,
            "member_joined",
            "user",
            Some(&user.id),
            format!("{} joined from an invitation as {role}.", user.display_name),
        )
        .await;
    }
    Ok(Json(AcceptedInvitation {
        target_type: invitation.target_type,
        target_id: invitation.target_id,
        organization_id,
        role,
    }))
}

/// Applies the membership with the usual rules, never lowering a role the
/// person already has. Returns the role held afterwards.
async fn join(state: &AppState, invitation: &Invitation, user_id: &str, email: &str) -> Result<String, ApiError> {
    if invitation.target_type == "workspace" {
        let role = parse_workspace_role(&invitation.role)?;
        let current = state.storage.workspace_role_for_user(user_id, &invitation.target_id).await.map_err(map_storage_error)?;
        if let Some(current) = current.filter(|current| workspace_rank(current) >= workspace_rank(&role)) {
            return Ok(workspace_role_label(&current).to_owned());
        }
        let member = state
            .storage
            .upsert_workspace_member(&invitation.target_id, email, role)
            .await
            .map_err(map_storage_error)?;
        Ok(workspace_role_label(&member.role).to_owned())
    } else {
        let role = parse_organization_role(&invitation.role)?;
        let current = state.storage.organization_role_for_user(user_id, &invitation.target_id).await.map_err(map_storage_error)?;
        if let Some(current) = current.filter(|current| organization_rank(current) >= organization_rank(&role)) {
            return Ok(organization_role_label(&current).to_owned());
        }
        let member = state
            .storage
            .upsert_organization_member(&invitation.target_id, email, role)
            .await
            .map_err(map_storage_error)?;
        Ok(organization_role_label(&member.role).to_owned())
    }
}

/// For registration: whether `token` is a usable invitation for `email`.
/// Lets an invited person create an account even when registration is closed.
pub(crate) async fn invitation_allows_registration(state: &AppState, token: &str, email: &str) -> Result<(), ApiError> {
    let stored = usable_invitation(state, token).await?;
    if stored.invitation.email != email.trim().to_lowercase() {
        return Err(ApiError::bad_request(format!(
            "This invitation is for {}. Create the account with that address.",
            stored.invitation.email
        )));
    }
    Ok(())
}

// -------------------------------------------------------------- password resets

#[derive(Serialize)]
pub(crate) struct IssuedPasswordReset {
    token: String,
    path: String,
    expires_at: String,
}

/// System › Users: a one-time link that lets the user choose a new password.
pub(crate) async fn issue_password_reset(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<(StatusCode, Json<IssuedPasswordReset>), ApiError> {
    crate::system::require_system_admin(&subject)?;
    let target = state
        .storage
        .user_access(&user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("User was not found."))?;
    // Otherwise an app administrator could take over a system administrator's account.
    if target.is_system_admin && !subject.is_system_admin {
        return Err(ApiError::forbidden_because("Only system administrators can reset a system administrator's password."));
    }
    let user = state
        .storage
        .find_user(&user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("User was not found."))?;
    let (token, expires_at) = state
        .storage
        .create_password_reset(&user_id, &subject.user_id, PASSWORD_RESET_TTL_HOURS)
        .await
        .map_err(map_storage_error)?;
    let who = user.email.unwrap_or(user.display_name);
    crate::system::audit(&state, &subject, "password_reset_issued", format!("Issued a password reset link for {who}.")).await;
    let path = format!("/reset-password/{token}");
    Ok((StatusCode::CREATED, Json(IssuedPasswordReset { token, path, expires_at })))
}

#[derive(Serialize)]
pub(crate) struct PasswordResetPreview {
    email: Option<String>,
    display_name: String,
    expires_at: String,
}

fn unusable_reset() -> ApiError {
    gone("This password reset link is not valid any more: it was used, replaced by a newer one, or it expired. Ask a system administrator for a new one.")
}

/// Public: whose password a reset link changes.
pub(crate) async fn preview_password_reset(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<TokenBody>,
) -> Result<Json<PasswordResetPreview>, ApiError> {
    check_auth_quota(&state, &client, AuthAction::SignIn)?;
    let reset = state
        .storage
        .find_password_reset(&request.token)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(unusable_reset)?;
    if reset.used || reset.expires_at < chrono::Utc::now().to_rfc3339() {
        return Err(unusable_reset());
    }
    Ok(Json(PasswordResetPreview { email: reset.user.email, display_name: reset.user.display_name, expires_at: reset.expires_at }))
}

#[derive(Deserialize)]
pub(crate) struct CompletePasswordReset {
    token: String,
    password: String,
}

/// Public: sets the new password, ends every session of the account, lifts
/// its sign-in lockouts and signs this browser in.
pub(crate) async fn complete_password_reset(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<CompletePasswordReset>,
) -> Result<Json<TokenResponse>, ApiError> {
    check_auth_quota(&state, &client, AuthAction::SignIn)?;
    validate_password(&request.password)?;
    // Hash first: a failure here must not use up the link.
    let password_hash = hash_password(&request.password).await?;
    let user_id = state
        .storage
        .consume_password_reset(&request.token)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(unusable_reset)?;
    let user = state
        .storage
        .find_user(&user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(unusable_reset)?;
    state.storage.update_user_password(&user_id, &password_hash).await.map_err(map_storage_error)?;
    state.storage.end_user_sessions(&user_id).await.map_err(map_storage_error)?;
    let email = user.email.clone().unwrap_or_default().to_lowercase();
    state.guards.logins.clear_account(&email);
    tracing::info!(target: "audit", user_id = %user_id, client = %client.0, "Password reset with a link");
    if let Err(error) = state
        .storage
        .record_system_event(Some(&user_id), "password_reset_used", &format!("{email} set a new password with a reset link."))
        .await
    {
        tracing::error!(error = %error, "Failed to record a system audit event");
    }
    Ok(Json(issue_session(&state, user).await?))
}

// ------------------------------------------------------- personal access tokens

pub(crate) async fn list_tokens(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<PersonalAccessToken>>, ApiError> {
    require_session(&subject)?;
    state.storage.list_personal_tokens(&subject.user_id).await.map(Json).map_err(map_storage_error)
}

#[derive(Deserialize)]
pub(crate) struct CreateTokenRequest {
    name: String,
    scope: String,
    expires_in_days: i64,
}

#[derive(Serialize)]
pub(crate) struct CreatedToken {
    token: PersonalAccessToken,
    /// The token itself, shown once.
    secret: String,
}

pub(crate) async fn create_token(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<CreateTokenRequest>,
) -> Result<(StatusCode, Json<CreatedToken>), ApiError> {
    require_session(&subject)?;
    if !TOKEN_TTL_DAYS.contains(&request.expires_in_days) {
        return Err(ApiError::bad_request("A token lasts 7, 30, 90 or 365 days."));
    }
    let (token, secret) = state
        .storage
        .create_personal_token(&subject.user_id, &request.name, request.scope.trim(), request.expires_in_days)
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    tracing::info!(target: "audit", user_id = %subject.user_id, token_id = %token.id, scope = %token.scope, "Personal access token created");
    Ok((StatusCode::CREATED, Json(CreatedToken { token, secret })))
}

pub(crate) async fn revoke_token(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(token_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_session(&subject)?;
    if !state.storage.revoke_personal_token(&subject.user_id, &token_id).await.map_err(map_storage_error)? {
        return Err(ApiError::not_found("Token was not found."));
    }
    tracing::info!(target: "audit", user_id = %subject.user_id, token_id = %token_id, "Personal access token revoked");
    Ok(StatusCode::NO_CONTENT)
}

/// Requests a personal access token may never make, whatever its scope: they
/// change how the account itself is reached, or the server's environment.
pub(crate) fn token_forbidden_path(path: &str) -> bool {
    path.starts_with("/v1/profile/tokens")
        || path == "/v1/profile/password"
        || path == "/v1/auth/logout-all"
        || path == "/v1/invitations/accept"
        || path == "/v1/system/environment/detach"
        || (path.starts_with("/v1/system/users/")
            && (path.ends_with("/system-admin") || path.ends_with("/app-admin") || path.ends_with("/password-reset")))
}

/// Requests other than GET that a read-only token may make: searches and
/// questions, which read but use POST for their body. The console is checked
/// again in its handler, which refuses changes from a read-only token.
pub(crate) fn read_token_may_post(path: &str) -> bool {
    path.ends_with("/search")
        || path.ends_with("/artifacts/query")
        || path.ends_with("/ask")
        || path == "/v1/system/console"
}

/// What a script sees when it asks for something its token cannot do.
pub(crate) fn token_refusal(read_only: bool) -> ApiError {
    if read_only {
        ApiError::forbidden_because("This personal access token is read-only.")
    } else {
        ApiError::forbidden_because("Personal access tokens cannot do this; sign in in the browser.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_never_reach_account_security() {
        for path in ["/v1/profile/tokens", "/v1/profile/tokens/abc", "/v1/profile/password", "/v1/auth/logout-all", "/v1/system/users/u/system-admin", "/v1/system/users/u/password-reset", "/v1/system/environment/detach", "/v1/invitations/accept"] {
            assert!(token_forbidden_path(path), "{path}");
        }
        for path in ["/v1/workspaces", "/v1/system/jobs", "/v1/system/users/u/sessions/end", "/v1/profile"] {
            assert!(!token_forbidden_path(path), "{path}");
        }
        assert!(read_token_may_post("/v1/workspaces/w/search"));
        assert!(read_token_may_post("/v1/workspaces/w/ask"));
        assert!(!read_token_may_post("/v1/workspaces/w/artifacts/text"));
    }

    #[test]
    fn only_owners_grant_administrators() {
        assert!(check_workspace_grant(&WorkspaceRole::Admin, &WorkspaceRole::Admin).is_err());
        assert!(check_workspace_grant(&WorkspaceRole::Owner, &WorkspaceRole::Admin).is_ok());
        assert!(check_workspace_grant(&WorkspaceRole::Admin, &WorkspaceRole::Viewer).is_ok());
        assert!(check_organization_grant(&OrganizationRole::Admin, &OrganizationRole::Admin).is_err());
        assert!(parse_workspace_role("owner").is_err());
        assert!(parse_organization_role("viewer").is_err());
    }
}
