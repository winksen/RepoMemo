use crate::*;

pub(crate) async fn require_workspace_read(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: &str,
) -> Result<WorkspaceRole, ApiError> {
    effective_workspace_role(state, &subject.user_id, subject.is_system_admin, workspace_id)
        .await?
        .ok_or_else(ApiError::forbidden)
}

/// The role a user acts with in a workspace: their membership, raised to
/// administrator for a system administrator (an owner stays owner), who also
/// acts as administrator where they are not a member.
pub(crate) async fn effective_workspace_role(
    state: &AppState,
    user_id: &str,
    is_system_admin: bool,
    workspace_id: &str,
) -> Result<Option<WorkspaceRole>, ApiError> {
    let membership = state
        .storage
        .workspace_role_for_user(user_id, workspace_id)
        .await
        .map_err(ApiError::internal)?;
    if !is_system_admin {
        return Ok(membership);
    }
    Ok(match membership {
        Some(WorkspaceRole::Owner) => Some(WorkspaceRole::Owner),
        Some(_) => Some(WorkspaceRole::Admin),
        None => state
            .storage
            .workspace_exists(workspace_id)
            .await
            .map_err(ApiError::internal)?
            .then_some(WorkspaceRole::Admin),
    })
}

pub(crate) async fn require_organization_read(
    state: &AppState,
    subject: &AuthenticatedSubject,
    organization_id: &str,
) -> Result<OrganizationRole, ApiError> {
    let membership = state
        .storage
        .organization_role_for_user(&subject.user_id, organization_id)
        .await
        .map_err(ApiError::internal)?;
    if !subject.is_system_admin {
        return membership.ok_or_else(ApiError::forbidden);
    }
    match membership {
        Some(OrganizationRole::Owner) => Ok(OrganizationRole::Owner),
        Some(_) => Ok(OrganizationRole::Admin),
        None if state
            .storage
            .organization_exists(organization_id)
            .await
            .map_err(ApiError::internal)? =>
        {
            Ok(OrganizationRole::Admin)
        }
        None => Err(ApiError::forbidden()),
    }
}

pub(crate) async fn require_organization_admin(
    state: &AppState,
    subject: &AuthenticatedSubject,
    organization_id: &str,
) -> Result<OrganizationRole, ApiError> {
    let role = require_organization_read(state, subject, organization_id).await?;
    if !matches!(role, OrganizationRole::Owner | OrganizationRole::Admin) {
        return Err(ApiError::forbidden());
    }
    Ok(role)
}

pub(crate) async fn require_organization_owner(
    state: &AppState,
    subject: &AuthenticatedSubject,
    organization_id: &str,
) -> Result<OrganizationRole, ApiError> {
    let role = require_organization_read(state, subject, organization_id).await?;
    if !matches!(role, OrganizationRole::Owner) {
        return Err(ApiError::forbidden());
    }
    Ok(role)
}

pub(crate) async fn require_workspace_write(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: &str,
) -> Result<WorkspaceRole, ApiError> {
    let role = require_workspace_read(state, subject, workspace_id).await?;
    if matches!(role, WorkspaceRole::Viewer) {
        return Err(ApiError::forbidden());
    }
    Ok(role)
}

pub(crate) async fn require_workspace_admin(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: &str,
) -> Result<WorkspaceRole, ApiError> {
    let role = require_workspace_read(state, subject, workspace_id).await?;
    if !matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin) {
        return Err(ApiError::forbidden());
    }
    Ok(role)
}

pub(crate) async fn require_workspace_owner(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: &str,
) -> Result<WorkspaceRole, ApiError> {
    let role = require_workspace_read(state, subject, workspace_id).await?;
    if !matches!(role, WorkspaceRole::Owner) {
        return Err(ApiError::forbidden());
    }
    Ok(role)
}

/// What a role may do; `may_use_ai` comes from the workspace's AI policy.
pub(crate) fn capabilities_for_role(role: WorkspaceRole, may_use_ai: bool) -> WorkspaceCapabilities {
    let can_write_content = !matches!(&role, WorkspaceRole::Viewer);
    let can_manage_members = matches!(&role, WorkspaceRole::Owner | WorkspaceRole::Admin);
    WorkspaceCapabilities {
        can_read: true,
        can_delete_content: can_write_content,
        can_write_content,
        can_assign_admin: matches!(&role, WorkspaceRole::Owner),
        can_manage_workspace: matches!(&role, WorkspaceRole::Owner),
        can_generate_ai_overview: may_use_ai,
        can_use_ai: may_use_ai,
        can_create_tasks: can_write_content,
        can_comment: can_write_content,
        can_moderate_comments: matches!(&role, WorkspaceRole::Owner | WorkspaceRole::Admin),
        can_inspect_index: can_manage_members,
        role,
        can_manage_members,
    }
}

pub(crate) fn workspace_role_label(role: &WorkspaceRole) -> &'static str {
    match role {
        WorkspaceRole::Owner => "owner",
        WorkspaceRole::Admin => "admin",
        WorkspaceRole::Member => "member",
        WorkspaceRole::Viewer => "viewer",
    }
}

/// The caller of a handler. A personal access token is accepted; a read-only
/// one only for GET and HEAD. Handlers mark stricter or looser needs in their
/// signature instead: [`SessionSubject`] for the account's own security
/// (tokens, password, roles, sign-out everywhere, invitations, the
/// environment), [`ReadingSubject`] for a POST that only reads (searches,
/// questions, console lookups).
impl FromRequestParts<AppState> for AuthenticatedSubject {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let subject = authenticate(parts, state).await?;
        let reading = matches!(parts.method, Method::GET | Method::HEAD);
        if subject.token.as_ref().is_some_and(|token| token.read_only) && !reading {
            return Err(access::token_refusal(true));
        }
        Ok(subject)
    }
}

/// The caller, signed in with a browser session: personal access tokens are
/// refused, whatever their scope. For what changes how the account itself is
/// reached, or the server's environment.
pub(crate) struct SessionSubject(pub(crate) AuthenticatedSubject);

impl FromRequestParts<AppState> for SessionSubject {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let subject = authenticate(parts, state).await?;
        if subject.token.is_some() {
            return Err(access::token_refusal(false));
        }
        Ok(Self(subject))
    }
}

/// The caller of a POST that only reads (its body carries a query): a
/// read-only personal access token may make it. A handler that also changes
/// something on request (the console's `--yes`) checks the token itself.
pub(crate) struct ReadingSubject(pub(crate) AuthenticatedSubject);

impl FromRequestParts<AppState> for ReadingSubject {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        authenticate(parts, state).await.map(Self)
    }
}

/// Who is calling, from a session's JWT or a personal access token, with no
/// check of what the request is.
pub(crate) async fn authenticate(parts: &Parts, state: &AppState) -> Result<AuthenticatedSubject, ApiError> {
    let header_value = parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(ApiError::unauthorized)?;
    let token = header_value
        .strip_prefix("Bearer ")
        .ok_or_else(ApiError::unauthorized)?;
    if token.starts_with(repomemo_storage::PERSONAL_TOKEN_PREFIX) {
        return authenticate_personal_token(parts, state, token).await;
    }
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[JWT_ISSUER]);
    let claims = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|_| ApiError::unauthorized())?
    .claims;
    // A token outlives neither its user nor the end of its sessions.
    let access = state
        .storage
        .user_access(&claims.sub)
        .await
        .map_err(ApiError::internal)?
        .filter(|access| access.session_version == claims.ver)
        .ok_or_else(ApiError::unauthorized)?;
    Ok(AuthenticatedSubject {
        user_id: claims.sub,
        expires_at: claims.exp as u64,
        session_version: claims.ver,
        is_system_admin: access.is_system_admin,
        is_app_admin: access.is_app_admin,
        token: None,
    })
}

/// A personal access token acts as its user, within its scope. What it may
/// reach is decided by the extractor the handler asks for (see
/// [`AuthenticatedSubject`]), not here.
pub(crate) async fn authenticate_personal_token(
    parts: &Parts,
    state: &AppState,
    token: &str,
) -> Result<AuthenticatedSubject, ApiError> {
    let grant = state
        .storage
        .authenticate_personal_token(token)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let access = state
        .storage
        .user_access(&grant.user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(ApiError::unauthorized)?;
    let read_only = !matches!(grant.scope.as_str(), "write" | "system");
    let system = grant.scope == "system";
    tracing::debug!(target: "audit", user_id = %grant.user_id, token_id = %grant.token_id, method = %parts.method, path = parts.uri.path(), "Personal access token used");
    let expires_at = chrono::DateTime::parse_from_rfc3339(&grant.expires_at)
        .map(|at| at.timestamp().max(0) as u64)
        .unwrap_or(0);
    Ok(AuthenticatedSubject {
        user_id: grant.user_id,
        expires_at,
        session_version: access.session_version,
        is_system_admin: access.is_system_admin,
        is_app_admin: access.is_app_admin,
        token: Some(TokenGrant { read_only, system }),
    })
}
