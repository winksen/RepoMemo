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
//!   browser session (handlers that need one take `SessionSubject`).
//!
//! Every secret is shown once and stored only as a hash (see the storage
//! crate's `access` module).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_domain::{Invitation, OrganizationRole, PersonalAccessToken, WorkspaceRole};
use repomemo_storage::NewSharedNotification;
use serde::{Deserialize, Serialize};

use crate::{
    check_auth_quota, create_notification, hash_password, issue_session, map_storage_error, record_workspace_activity,
    require_organization_admin, require_workspace_admin, security::ClientIp, validate_password, workspace_role_label,
    ApiError, AppState, AuthAction, AuthenticatedSubject, ManagingSubject, SessionSubject, TokenResponse,
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
    /// `/invite#<token>`: the secret is in the fragment, which browsers never
    /// send to a server, so it stays out of access logs and `Referer`.
    path: String,
    /// What became of the mail with the link: `queued`, `not_set_up`,
    /// `no_public_address` or `failed`. The link is returned either way.
    mail: &'static str,
}

fn created(invitation: Invitation, token: String, mail: &'static str) -> CreatedInvitation {
    let path = format!("/invite#{token}");
    CreatedInvitation {
        invitation,
        token,
        path,
        mail,
    }
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
        return Err(ApiError::forbidden_because(
            "Only the workspace owner can invite administrators.",
        ));
    }
    Ok(())
}

fn check_organization_grant(caller: &OrganizationRole, role: &OrganizationRole) -> Result<(), ApiError> {
    if matches!(role, OrganizationRole::Admin) && !matches!(caller, OrganizationRole::Owner) {
        return Err(ApiError::forbidden_because(
            "Only the organization owner can invite administrators.",
        ));
    }
    Ok(())
}

pub(crate) async fn create_workspace_invitation(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<CreateInvitationRequest>,
) -> Result<(StatusCode, Json<CreatedInvitation>), ApiError> {
    let caller = require_workspace_admin(&state, &subject, &workspace_id).await?;
    let role = parse_workspace_role(request.role.trim())?;
    check_workspace_grant(&caller, &role)?;
    if let Some(user) = state
        .storage
        .find_user_by_email(&request.email)
        .await
        .map_err(map_storage_error)?
    {
        if state
            .storage
            .workspace_role_for_user(&user.id, &workspace_id)
            .await
            .map_err(map_storage_error)?
            .is_some()
        {
            return Err(ApiError::conflict(
                "This person is already a member of this workspace. Change their role in the member list.",
            ));
        }
    }
    let (invitation, token) = state
        .storage
        .create_invitation(
            &request.email,
            "workspace",
            &workspace_id,
            workspace_role_label(&role),
            &subject.user_id,
            INVITATION_TTL_DAYS,
        )
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
    let workspace_name = state
        .core
        .list_workspaces()
        .await
        .ok()
        .and_then(|all| all.into_iter().find(|workspace| workspace.id == workspace_id))
        .map_or_else(|| "a workspace".to_owned(), |workspace| workspace.name);
    let path = format!("/invite#{token}");
    let mail = crate::mail::mail_invitation(&state, &subject.user_id, &invitation, &workspace_name, &path).await;
    Ok((StatusCode::CREATED, Json(created(invitation, token, mail))))
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
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
    Json(request): Json<CreateInvitationRequest>,
) -> Result<(StatusCode, Json<CreatedInvitation>), ApiError> {
    let caller = require_organization_admin(&state, &subject, &organization_id).await?;
    let role = parse_organization_role(request.role.trim())?;
    check_organization_grant(&caller, &role)?;
    if let Some(user) = state
        .storage
        .find_user_by_email(&request.email)
        .await
        .map_err(map_storage_error)?
    {
        if state
            .storage
            .organization_role_for_user(&user.id, &organization_id)
            .await
            .map_err(map_storage_error)?
            .is_some()
        {
            return Err(ApiError::conflict(
                "This person is already a member of this organization. Change their role in the member list.",
            ));
        }
    }
    let (invitation, token) = state
        .storage
        .create_invitation(
            &request.email,
            "organization",
            &organization_id,
            organization_role_label(&role),
            &subject.user_id,
            INVITATION_TTL_DAYS,
        )
        .await
        .map_err(map_storage_error)?;
    tracing::info!(target: "audit", user_id = %subject.user_id, organization_id = %organization_id, invitation_id = %invitation.id, "Organization invitation created");
    let organization_name = state
        .storage
        .list_organizations_for_user(&subject.user_id)
        .await
        .ok()
        .and_then(|all| all.into_iter().find(|organization| organization.id == organization_id))
        .map_or_else(|| "an organization".to_owned(), |organization| organization.name);
    let path = format!("/invite#{token}");
    let mail = crate::mail::mail_invitation(&state, &subject.user_id, &invitation, &organization_name, &path).await;
    Ok((StatusCode::CREATED, Json(created(invitation, token, mail))))
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
    ManagingSubject(subject): ManagingSubject,
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
    if !state
        .storage
        .revoke_invitation(&invitation_id)
        .await
        .map_err(map_storage_error)?
    {
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
        .ok_or_else(|| {
            ApiError::not_found("This invitation link is not valid. Check that it was copied completely.")
        })?;
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
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    Json(request): Json<TokenBody>,
) -> Result<Json<AcceptedInvitation>, ApiError> {
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
        return Err(gone(
            "The person who invited you can no longer add members with this role. Ask for a new invitation.",
        ));
    }
    if !state
        .storage
        .mark_invitation_accepted(&invitation.id, &user.id)
        .await
        .map_err(map_storage_error)?
    {
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
    // The invitation was sent to this address and its owner used it.
    let _ = state.storage.mark_email_verified(&user.id).await;
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
        let current = state
            .storage
            .workspace_role_for_user(user_id, &invitation.target_id)
            .await
            .map_err(map_storage_error)?;
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
        let current = state
            .storage
            .organization_role_for_user(user_id, &invitation.target_id)
            .await
            .map_err(map_storage_error)?;
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
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<(StatusCode, Json<IssuedPasswordReset>), ApiError> {
    crate::system::require_full_system_admin(&subject)?;
    if state
        .storage
        .is_deactivated(&user_id)
        .await
        .map_err(ApiError::internal)?
    {
        return Err(ApiError::conflict(
            "This account is deactivated. Reactivate it before creating a password reset link.",
        ));
    }
    let target = state
        .storage
        .user_access(&user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("User was not found."))?;
    // Otherwise an app administrator could take over a system administrator's account.
    if target.is_system_admin && !subject.is_system_admin {
        return Err(ApiError::forbidden_because(
            "Only system administrators can reset a system administrator's password.",
        ));
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
    // The account hears about it, in case the request did not come from them (review L4).
    create_notification(
        &state,
        NewSharedNotification {
            user_id: user_id.clone(),
            workspace_id: None,
            notification_type: "password_reset_issued".to_owned(),
            title: "A password reset link was created for your account".to_owned(),
            body: format!(
                "An administrator created a link that sets your password, valid until {expires_at}. If you did not ask for one, tell an administrator."
            ),
            href: "/profile".to_owned(),
        },
    )
    .await;
    let who = user.email.unwrap_or(user.display_name);
    crate::system::audit(
        &state,
        &subject,
        "password_reset_issued",
        format!("Issued a password reset link for {who}."),
    )
    .await;
    // In the fragment, like invitation links: never sent to a server.
    let path = format!("/reset-password#{token}");
    Ok((
        StatusCode::CREATED,
        Json(IssuedPasswordReset {
            token,
            path,
            expires_at,
        }),
    ))
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
    Ok(Json(PasswordResetPreview {
        email: reset.user.email,
        display_name: reset.user.display_name,
        expires_at: reset.expires_at,
    }))
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
    let reset_email = state
        .storage
        .find_password_reset(&request.token)
        .await
        .map_err(map_storage_error)?
        .and_then(|reset| reset.user.email);
    validate_password(&request.password, reset_email.as_deref())?;
    // Hash first: a failure here must not use up the link.
    let password_hash = hash_password(&request.password).await?;
    if let Some(user) = state
        .storage
        .find_password_reset(&request.token)
        .await
        .map_err(map_storage_error)?
    {
        crate::identity::password_sign_in_allowed(&state, &user.user.id).await?;
    }
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
    state
        .storage
        .update_user_password(&user_id, &password_hash)
        .await
        .map_err(map_storage_error)?;
    state
        .storage
        .end_user_sessions(&user_id)
        .await
        .map_err(map_storage_error)?;
    // A reset often follows a lost or taken-over account: its scripts' tokens
    // go too, and the owner creates new ones once signed in.
    let tokens = state
        .storage
        .revoke_user_personal_tokens(&user_id)
        .await
        .map_err(map_storage_error)?;
    let email = user.email.clone().unwrap_or_default().to_lowercase();
    state.guards.logins.clear_account(&email);
    tracing::info!(target: "audit", user_id = %user_id, client = %client.0, tokens_revoked = tokens, "Password reset with a link");
    create_notification(
        &state,
        NewSharedNotification {
            user_id: user_id.clone(),
            workspace_id: None,
            notification_type: "password_reset_used".to_owned(),
            title: "Your password was changed with a reset link".to_owned(),
            body: "Every device was signed out and your personal access tokens were revoked. If this was not you, tell an administrator now.".to_owned(),
            href: "/profile".to_owned(),
        },
    )
    .await;
    if let Err(error) = state
        .storage
        .record_system_event(
            Some(&user_id),
            "password_reset_used",
            &format!("{email} set a new password with a reset link."),
        )
        .await
    {
        tracing::error!(error = %error, "Failed to record a system audit event");
    }
    Ok(Json(issue_session(&state, user).await?))
}

// ------------------------------------------------------- personal access tokens

pub(crate) async fn list_tokens(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<PersonalAccessToken>>, ApiError> {
    state
        .storage
        .list_personal_tokens(&subject.user_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

#[derive(Deserialize)]
pub(crate) struct CreateTokenRequest {
    name: String,
    scope: String,
    expires_in_days: i64,
    /// Limit the token to these workspaces; leave out for every workspace its owner can open.
    #[serde(default)]
    workspace_ids: Option<Vec<String>>,
}

#[derive(Serialize)]
pub(crate) struct CreatedToken {
    token: PersonalAccessToken,
    /// The token itself, shown once.
    secret: String,
}

pub(crate) async fn create_token(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    Json(request): Json<CreateTokenRequest>,
) -> Result<(StatusCode, Json<CreatedToken>), ApiError> {
    if request.scope.trim() == "system" && !(subject.is_system_admin || subject.is_app_admin) {
        return Err(ApiError::forbidden_because(
            "Only system and app administrators can create tokens with the system scope.",
        ));
    }
    if !TOKEN_TTL_DAYS.contains(&request.expires_in_days) {
        return Err(ApiError::bad_request("A token lasts 7, 30, 90 or 365 days."));
    }
    let workspaces = match &request.workspace_ids {
        Some(ids) => {
            if request.scope.trim() == "system" {
                return Err(ApiError::bad_request(
                    "A token with the system scope is not limited to workspaces. Choose another scope.",
                ));
            }
            Some(checked_token_workspaces(&state, &subject, ids).await?)
        }
        None => None,
    };
    let (token, secret) = state
        .storage
        .create_personal_token_for(
            &subject.user_id,
            &request.name,
            request.scope.trim(),
            request.expires_in_days,
            workspaces.as_deref(),
        )
        .await
        .map_err(map_storage_error)?;
    tracing::info!(target: "audit", user_id = %subject.user_id, token_id = %token.id, scope = %token.scope, "Personal access token created");
    Ok((StatusCode::CREATED, Json(CreatedToken { token, secret })))
}

/// The workspaces a token may be limited to: between one and fifty, each one its owner can open now.
async fn checked_token_workspaces(
    state: &AppState,
    subject: &crate::AuthenticatedSubject,
    ids: &[String],
) -> Result<Vec<String>, ApiError> {
    let mut chosen: Vec<String> = Vec::new();
    for id in ids {
        if !chosen.contains(id) {
            chosen.push(id.clone());
        }
    }
    if chosen.is_empty() || chosen.len() > 50 {
        return Err(ApiError::bad_request(
            "Choose between one and fifty workspaces for the token.",
        ));
    }
    for id in &chosen {
        let role =
            crate::access_control::effective_workspace_role(state, &subject.user_id, subject.is_system_admin, id)
                .await?;
        if role.is_none() {
            return Err(ApiError::bad_request(
                "A token can only be limited to workspaces you can open.",
            ));
        }
    }
    Ok(chosen)
}

#[derive(Deserialize)]
pub(crate) struct TokenWorkspacesRequest {
    workspace_ids: Vec<String>,
}

/// Limits a token you own to chosen workspaces, replacing an earlier choice. It is how a token made before V0.5.15,
/// which reaches everything its owner can open, is narrowed. A limited token never becomes unlimited again: make a
/// new token for that.
pub(crate) async fn set_token_workspaces(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    Path(token_id): Path<String>,
    Json(request): Json<TokenWorkspacesRequest>,
) -> Result<StatusCode, ApiError> {
    let workspaces = checked_token_workspaces(&state, &subject, &request.workspace_ids).await?;
    if !state
        .storage
        .set_token_workspaces(&subject.user_id, &token_id, &workspaces)
        .await
        .map_err(map_storage_error)?
    {
        return Err(ApiError::not_found("Token was not found."));
    }
    tracing::info!(target: "audit", user_id = %subject.user_id, token_id = %token_id, workspaces = workspaces.len(), "Personal access token limited to workspaces");
    Ok(StatusCode::NO_CONTENT)
}

/// Every live token on the server with its owner, scope, expiry, last use and workspaces: the access inventory.
pub(crate) async fn token_inventory(
    subject: crate::AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::system::require_full_system_admin(&subject)?;
    let tokens = state.storage.token_inventory().await.map_err(map_storage_error)?;
    Ok(Json(serde_json::json!({"tokens":tokens})))
}

/// An administrator ends any token; the owner is told nothing is wrong with their account, the audit trail records it.
pub(crate) async fn admin_revoke_token(
    subject: crate::AuthenticatedSubject,
    State(state): State<AppState>,
    Path(token_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    crate::system::require_full_system_admin(&subject)?;
    crate::system::require_system_change(&subject)?;
    let Some(owner) = state
        .storage
        .revoke_token_by_id(&token_id)
        .await
        .map_err(map_storage_error)?
    else {
        return Err(ApiError::not_found("Token was not found."));
    };
    crate::system::audit(
        &state,
        &subject,
        "token_revoked_by_administrator",
        format!("Revoked token {token_id} of user {owner}."),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn revoke_token(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    Path(token_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    if !state
        .storage
        .revoke_personal_token(&subject.user_id, &token_id)
        .await
        .map_err(map_storage_error)?
    {
        return Err(ApiError::not_found("Token was not found."));
    }
    tracing::info!(target: "audit", user_id = %subject.user_id, token_id = %token_id, "Personal access token revoked");
    Ok(StatusCode::NO_CONTENT)
}

/// What a script sees when it asks for something only the `admin` scope or a session may do.
pub(crate) fn admin_scope_refusal() -> ApiError {
    ApiError::forbidden_because(
        "This route changes who has access or how the workspace is set up. Sign in in the browser, or use a personal access token with the admin scope.",
    )
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
    fn only_owners_grant_administrators() {
        assert!(check_workspace_grant(&WorkspaceRole::Admin, &WorkspaceRole::Admin).is_err());
        assert!(check_workspace_grant(&WorkspaceRole::Owner, &WorkspaceRole::Admin).is_ok());
        assert!(check_workspace_grant(&WorkspaceRole::Admin, &WorkspaceRole::Viewer).is_ok());
        assert!(check_organization_grant(&OrganizationRole::Admin, &OrganizationRole::Admin).is_err());
        assert!(parse_workspace_role("owner").is_err());
        assert!(parse_organization_role("viewer").is_err());
    }
}

/// How long a link asked for by email works: the owner of the address is the
/// one who asked, so it is much shorter than an administrator's 24 hours.
const SELF_SERVICE_RESET_TTL_HOURS: i64 = 1;
/// Requests for one address per hour. Past it nothing more is sent, and the
/// answer does not change.
const FORGOT_PER_ADDRESS_PER_HOUR: u32 = 3;

#[derive(Deserialize)]
pub(crate) struct ForgotPasswordRequest {
    email: String,
}

#[derive(Serialize)]
pub(crate) struct ForgotPasswordAnswer {
    message: &'static str,
}

/// Public: "Forgot your password?". The answer is the same for every address
/// (known or not, mail set up or not, limit reached or not) and comes back
/// before anything is looked up, so neither its words nor its timing say who
/// has an account. Only the sign-in limits of the client address can refuse
/// (`429`). When the address belongs to an account and mail is set up, a link
/// valid for one hour (which ends every session when used) goes to it through
/// the outbox.
pub(crate) async fn forgot_password(
    State(state): State<AppState>,
    client: ClientIp,
    Json(request): Json<ForgotPasswordRequest>,
) -> Result<(StatusCode, Json<ForgotPasswordAnswer>), ApiError> {
    check_auth_quota(&state, &client, AuthAction::SignIn)?;
    let email = request.email.trim().to_lowercase();
    let within_limit = state
        .guards
        .auth
        .check(
            &format!("forgot:{email}"),
            crate::Quota::per_hour(FORGOT_PER_ADDRESS_PER_HOUR),
        )
        .is_ok();
    if within_limit && repomemo_mail::valid_address(&email) {
        let worker = state.clone();
        tokio::spawn(async move { send_reset_link(&worker, &email).await });
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(ForgotPasswordAnswer {
            message: "If an account uses that address and this server sends mail, a link to choose a new password is on its way. It works once, for one hour.",
        }),
    ))
}

async fn send_reset_link(state: &AppState, email: &str) {
    if !state.mail.is_ready().await {
        return;
    }
    let Ok(Some(user)) = state.storage.find_user_by_email(email).await else {
        return;
    };
    if state.storage.is_deactivated(&user.id).await.unwrap_or(true) {
        return;
    }
    let Ok((token, expires_at)) = state
        .storage
        .create_password_reset(&user.id, &user.id, SELF_SERVICE_RESET_TTL_HOURS)
        .await
    else {
        return;
    };
    let Some(link) = state.mail.link(&format!("/reset-password#{token}")) else {
        tracing::warn!(
            "A password reset was asked for by mail, but REPOMEMO_PUBLIC_URL is not set, so the link cannot be sent"
        );
        return;
    };
    let body = format!(
        "Someone asked to choose a new password for the RepoMemo account of this address.\n\n\
         Open this link to choose one. It works once and for one hour (until {expires_at}); using it signs the account out everywhere:\n\n\
         {link}\n\n\
         If you did not ask for this, ignore this message: your password has not changed.\n"
    );
    if state
        .mail
        .send_to(email, "Choose a new RepoMemo password", &body, "password_reset")
        .await
        .is_ok()
    {
        tracing::info!(target: "audit", user_id = %user.id, "A password reset link was sent by mail on request");
        create_notification(
            state,
            NewSharedNotification {
                user_id: user.id.clone(),
                workspace_id: None,
                notification_type: "password_reset_issued".to_owned(),
                title: "A password reset link was sent to your email".to_owned(),
                body: format!(
                    "A link that sets your password was sent to your address, valid until {expires_at}. If you did not ask for it, tell an administrator."
                ),
                href: "/profile".to_owned(),
            },
        )
        .await;
    }
}
