use crate::*;

pub(crate) async fn create_organization(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<CreateOrganizationRequest>,
) -> Result<(StatusCode, Json<Organization>), ApiError> {
    let organization = state
        .storage
        .create_organization(&subject.user_id, &request.name)
        .await
        .map_err(map_storage_error)?;
    Ok((StatusCode::CREATED, Json(organization)))
}

/// The caller's organizations; every organization for a system administrator.
pub(crate) async fn list_organizations(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<Organization>>, ApiError> {
    let organizations = if subject.is_system_admin {
        state
            .storage
            .list_organizations_for_system_admin(&subject.user_id)
            .await
    } else {
        state.storage.list_organizations_for_user(&subject.user_id).await
    };
    organizations.map(Json).map_err(ApiError::internal)
}

pub(crate) async fn update_organization(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
    Json(request): Json<UpdateOrganizationRequest>,
) -> Result<Json<Organization>, ApiError> {
    require_organization_owner(&state, &subject, &organization_id).await?;
    state
        .storage
        .update_organization(&organization_id, &subject.user_id, &request.name)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn list_organization_members(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
) -> Result<Json<Vec<OrganizationMember>>, ApiError> {
    require_organization_read(&state, &subject, &organization_id).await?;
    state
        .storage
        .list_organization_members(&organization_id)
        .await
        .map(Json)
        .map_err(ApiError::internal)
}

pub(crate) async fn upsert_organization_member(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
    Json(request): Json<UpsertOrganizationMemberRequest>,
) -> Result<Json<OrganizationMember>, ApiError> {
    let caller_role = require_organization_admin(&state, &subject, &organization_id).await?;
    if matches!(request.role, OrganizationRole::Owner)
        || (matches!(caller_role, OrganizationRole::Admin) && matches!(request.role, OrganizationRole::Admin))
    {
        return Err(ApiError::forbidden());
    }
    let member = match state
        .storage
        .find_user_by_email(&request.email)
        .await
        .map_err(map_storage_error)?
    {
        Some(user) => state
            .storage
            .organization_role_for_user(&user.id, &organization_id)
            .await
            .map_err(map_storage_error)?
            .is_some(),
        None => false,
    };
    if !member {
        return Err(invitation_required());
    }
    if matches!(caller_role, OrganizationRole::Admin) {
        if let Some(target) = state
            .storage
            .find_user_by_email(&request.email)
            .await
            .map_err(ApiError::internal)?
        {
            let target_role = state
                .storage
                .organization_role_for_user(&target.id, &organization_id)
                .await
                .map_err(ApiError::internal)?;
            if matches!(target_role, Some(OrganizationRole::Owner | OrganizationRole::Admin)) {
                return Err(ApiError::forbidden());
            }
        }
    }
    state
        .storage
        .upsert_organization_member(&organization_id, &request.email, request.role)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn remove_organization_member(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((organization_id, user_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let caller_role = require_organization_admin(&state, &subject, &organization_id).await?;
    let target_role = state
        .storage
        .organization_role_for_user(&user_id, &organization_id)
        .await
        .map_err(ApiError::internal)?;
    if matches!(target_role, Some(OrganizationRole::Owner))
        || (matches!(caller_role, OrganizationRole::Admin) && matches!(target_role, Some(OrganizationRole::Admin)))
    {
        return Err(ApiError::forbidden());
    }
    state
        .storage
        .remove_organization_member(&organization_id, &user_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

/// The answer to adding someone who is not in the organization yet. It is the
/// same whether or not the address has an account, so a member form cannot
/// tell anyone who has one (review M4); and nobody is added to an
/// organization or workspace without accepting.
pub(crate) fn invitation_required() -> ApiError {
    ApiError::new(
        StatusCode::CONFLICT,
        "invitation_required",
        "Only people already in this organization can be added directly. Create an invitation link for this address: they join when they accept it.",
    )
}

pub(crate) async fn list_workspace_members(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<WorkspaceMember>>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_workspace_members(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn upsert_workspace_member(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<UpsertWorkspaceMemberRequest>,
) -> Result<Json<WorkspaceMember>, ApiError> {
    let caller_role = require_workspace_admin(&state, &subject, &workspace_id).await?;
    let target = state
        .storage
        .find_user_by_email(&request.email)
        .await
        .map_err(map_storage_error)?;
    let reachable = match &target {
        Some(user) => {
            state
                .storage
                .workspace_role_for_user(&user.id, &workspace_id)
                .await
                .map_err(map_storage_error)?
                .is_some()
                || state
                    .storage
                    .user_in_workspace_organization(&user.id, &workspace_id)
                    .await
                    .map_err(map_storage_error)?
        }
        None => false,
    };
    if !reachable {
        return Err(invitation_required());
    }
    if matches!(&caller_role, WorkspaceRole::Admin) {
        if matches!(request.role, WorkspaceRole::Admin) {
            return Err(ApiError::forbidden());
        }
        if let Some(user) = state
            .storage
            .find_user_by_email(&request.email)
            .await
            .map_err(map_storage_error)?
        {
            let target_role = state
                .storage
                .workspace_role_for_user(&user.id, &workspace_id)
                .await
                .map_err(map_storage_error)?;
            if matches!(target_role, Some(WorkspaceRole::Owner | WorkspaceRole::Admin)) {
                return Err(ApiError::forbidden());
            }
        }
    }
    let member = state
        .storage
        .upsert_workspace_member(&workspace_id, &request.email, request.role)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "member_updated",
        "user",
        Some(&member.user.id),
        format!(
            "Set {} to {}.",
            member.user.display_name,
            workspace_role_label(&member.role)
        ),
    )
    .await;
    Ok(Json(member))
}

pub(crate) async fn remove_workspace_member(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, user_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let caller_role = require_workspace_admin(&state, &subject, &workspace_id).await?;
    let member = state
        .storage
        .list_workspace_members(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .find(|member| member.user.id == user_id);
    if matches!(&caller_role, WorkspaceRole::Admin)
        && matches!(
            member.as_ref().map(|entry| &entry.role),
            Some(WorkspaceRole::Owner | WorkspaceRole::Admin)
        )
    {
        return Err(ApiError::forbidden());
    }
    let member_name = member
        .map(|entry| entry.user.display_name)
        .unwrap_or_else(|| "a workspace member".to_owned());
    state
        .storage
        .remove_workspace_member(&workspace_id, &user_id)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "member_removed",
        "user",
        Some(&user_id),
        format!("Removed {member_name} from the workspace."),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// Hands a workspace to another member. Owners only, from a signed-in
/// session; the previous owner stays an administrator. Recorded in the
/// workspace activity and in the System audit.
pub(crate) async fn transfer_workspace_ownership(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<TransferOwnershipRequest>,
) -> Result<StatusCode, ApiError> {
    require_workspace_owner(&state, &subject, &workspace_id).await?;
    if request.user_id == subject.user_id {
        return Err(ApiError::bad_request("Choose someone else to hand the workspace to."));
    }
    let new_owner = state
        .storage
        .transfer_workspace_ownership(&workspace_id, &request.user_id)
        .await
        .map_err(map_storage_error)?;
    let who = new_owner.email.clone().unwrap_or(new_owner.display_name.clone());
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "ownership_transferred",
        "workspace",
        Some(&workspace_id),
        format!("Handed the workspace to {who}."),
    )
    .await;
    crate::system::audit(
        &state,
        &subject,
        "workspace_ownership_transferred",
        format!("Workspace {workspace_id} was handed to {who}."),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// Hands an organization to another member. Owners only, from a signed-in session.
pub(crate) async fn transfer_organization_ownership(
    SessionSubject(subject): SessionSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
    Json(request): Json<TransferOwnershipRequest>,
) -> Result<StatusCode, ApiError> {
    require_organization_owner(&state, &subject, &organization_id).await?;
    if request.user_id == subject.user_id {
        return Err(ApiError::bad_request(
            "Choose someone else to hand the organization to.",
        ));
    }
    let new_owner = state
        .storage
        .transfer_organization_ownership(&organization_id, &request.user_id)
        .await
        .map_err(map_storage_error)?;
    let who = new_owner.email.clone().unwrap_or(new_owner.display_name.clone());
    crate::system::audit(
        &state,
        &subject,
        "organization_ownership_transferred",
        format!("Organization {organization_id} was handed to {who}."),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}
