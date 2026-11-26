//! CAIRN abilities (V0.9.104): procedures an organization's or a workspace's administrators teach CAIRN, each with a `/command`, its
//! instructions and the tools it may use. Creating and publishing need the same rights as the CAIRN settings of that scope; a stale
//! publish names the current version. An ability adds no tool and no right: the tools are checked against the registry. Running an
//! ability comes later. Every change is an audit event, so it shows in the administration history.

use axum::{
    extract::{Path, State},
    Json,
};
use repomemo_storage::{AbilityDraft, CairnAbility};
use serde::{Deserialize, Serialize};

use crate::{
    cairn_tools::checked_names, map_storage_error, organization_cairn::require_access, system::audit, ApiError,
    AppState, AuthenticatedSubject, ManagingSubject,
};

#[derive(Debug, Deserialize)]
pub(crate) struct AbilityRequest {
    #[serde(flatten)]
    draft: AbilityDraft,
    /// The version the caller read when publishing; ignored when making an ability.
    #[serde(default)]
    expected_version: i64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EnabledRequest {
    enabled: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct AbilityVersion {
    version: i64,
    at: String,
    by: Option<String>,
}

/// Whether the caller reads or changes the abilities of the scope an ability belongs to.
async fn require_ability_access(
    state: &AppState,
    subject: &AuthenticatedSubject,
    ability: &CairnAbility,
    change: bool,
) -> Result<(), ApiError> {
    match (&ability.organization_id, &ability.workspace_id) {
        (Some(organization_id), _) => require_access(state, subject, organization_id, change).await,
        (_, Some(workspace_id)) => crate::require_workspace_admin(state, subject, workspace_id)
            .await
            .map(drop),
        _ => Err(ApiError::forbidden()),
    }
}

/// The tools of a draft, checked against the registry.
fn checked(mut request: AbilityRequest) -> Result<AbilityRequest, ApiError> {
    request.draft.tools = checked_names(std::mem::take(&mut request.draft.tools))?;
    Ok(request)
}

async fn scope_label(state: &AppState, subject: &AuthenticatedSubject, ability: &CairnAbility) -> String {
    if let Some(organization_id) = &ability.organization_id {
        return state
            .storage
            .list_organizations_for_system_admin(&subject.user_id)
            .await
            .ok()
            .and_then(|all| all.into_iter().find(|organization| &organization.id == organization_id))
            .map_or_else(|| organization_id.clone(), |organization| organization.name);
    }
    ability.workspace_id.clone().unwrap_or_default()
}

async fn record(state: &AppState, subject: &AuthenticatedSubject, action: &str, ability: &CairnAbility, what: &str) {
    let scope = scope_label(state, subject, ability).await;
    audit(
        state,
        subject,
        action,
        format!(
            "{what} the ability {} ({}) of {scope}, now at version {}.",
            ability.name, ability.command, ability.version
        ),
    )
    .await;
}

pub(crate) async fn list_organization(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
) -> Result<Json<Vec<CairnAbility>>, ApiError> {
    require_access(&state, &subject, &organization_id, false).await?;
    Ok(Json(
        state
            .storage
            .list_cairn_abilities(Some(&organization_id), None)
            .await
            .map_err(map_storage_error)?,
    ))
}

pub(crate) async fn list_workspace(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<CairnAbility>>, ApiError> {
    crate::require_workspace_admin(&state, &subject, &workspace_id).await?;
    Ok(Json(
        state
            .storage
            .list_cairn_abilities(None, Some(&workspace_id))
            .await
            .map_err(map_storage_error)?,
    ))
}

pub(crate) async fn create_organization(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
    Json(request): Json<AbilityRequest>,
) -> Result<Json<CairnAbility>, ApiError> {
    require_access(&state, &subject, &organization_id, true).await?;
    let request = checked(request)?;
    let ability = state
        .storage
        .create_cairn_ability(Some(&organization_id), None, &request.draft, &subject.user_id)
        .await
        .map_err(map_storage_error)?;
    record(&state, &subject, "cairn_ability_created", &ability, "Created").await;
    Ok(Json(ability))
}

pub(crate) async fn create_workspace(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<AbilityRequest>,
) -> Result<Json<CairnAbility>, ApiError> {
    crate::require_workspace_admin(&state, &subject, &workspace_id).await?;
    let request = checked(request)?;
    let ability = state
        .storage
        .create_cairn_ability(None, Some(&workspace_id), &request.draft, &subject.user_id)
        .await
        .map_err(map_storage_error)?;
    record(&state, &subject, "cairn_ability_created", &ability, "Created").await;
    Ok(Json(ability))
}

/// `PUT /v1/cairn/abilities/{id}`: publishes a new version of an ability that was read at `expected_version`.
pub(crate) async fn publish(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(ability_id): Path<String>,
    Json(request): Json<AbilityRequest>,
) -> Result<Json<CairnAbility>, ApiError> {
    let existing = state
        .storage
        .get_cairn_ability(&ability_id)
        .await
        .map_err(map_storage_error)?;
    require_ability_access(&state, &subject, &existing, true).await?;
    let request = checked(request)?;
    match state
        .storage
        .publish_cairn_ability(&ability_id, request.expected_version, &request.draft, &subject.user_id)
        .await
        .map_err(map_storage_error)?
    {
        Ok(ability) => {
            record(
                &state,
                &subject,
                "cairn_ability_published",
                &ability,
                "Published a new version of",
            )
            .await;
            Ok(Json(ability))
        }
        Err(current) => Err(ApiError::conflict(format!(
            "The ability was changed by someone else (now at version {}). Reload it and change it again.",
            current.version
        ))),
    }
}

pub(crate) async fn set_enabled(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(ability_id): Path<String>,
    Json(request): Json<EnabledRequest>,
) -> Result<Json<CairnAbility>, ApiError> {
    let existing = state
        .storage
        .get_cairn_ability(&ability_id)
        .await
        .map_err(map_storage_error)?;
    require_ability_access(&state, &subject, &existing, true).await?;
    let ability = state
        .storage
        .set_cairn_ability_enabled(&ability_id, request.enabled)
        .await
        .map_err(map_storage_error)?;
    record(
        &state,
        &subject,
        "cairn_ability_switched",
        &ability,
        if request.enabled { "Turned on" } else { "Turned off" },
    )
    .await;
    Ok(Json(ability))
}

pub(crate) async fn remove(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(ability_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let existing = state
        .storage
        .get_cairn_ability(&ability_id)
        .await
        .map_err(map_storage_error)?;
    require_ability_access(&state, &subject, &existing, true).await?;
    state
        .storage
        .delete_cairn_ability(&ability_id)
        .await
        .map_err(map_storage_error)?;
    record(&state, &subject, "cairn_ability_deleted", &existing, "Deleted").await;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub(crate) async fn versions(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(ability_id): Path<String>,
) -> Result<Json<Vec<AbilityVersion>>, ApiError> {
    let existing = state
        .storage
        .get_cairn_ability(&ability_id)
        .await
        .map_err(map_storage_error)?;
    require_ability_access(&state, &subject, &existing, false).await?;
    Ok(Json(
        state
            .storage
            .cairn_ability_versions(&ability_id)
            .await
            .map_err(map_storage_error)?
            .into_iter()
            .map(|(version, at, by)| AbilityVersion { version, at, by })
            .collect(),
    ))
}

#[derive(Debug, Serialize)]
pub(crate) struct OrganizationAbilityView {
    ability: CairnAbility,
    /// Whether this workspace has it on.
    on: bool,
}

/// `GET /v1/workspaces/{id}/cairn/abilities/organization`: the organization's abilities, each with whether the workspace has it on.
pub(crate) async fn list_for_workspace(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<OrganizationAbilityView>>, ApiError> {
    crate::require_workspace_admin(&state, &subject, &workspace_id).await?;
    Ok(Json(
        state
            .storage
            .organization_abilities_for_workspace(&workspace_id)
            .await
            .map_err(map_storage_error)?
            .into_iter()
            .map(|(ability, on)| OrganizationAbilityView { ability, on })
            .collect(),
    ))
}

/// `PUT /v1/workspaces/{id}/cairn/abilities/organization/{ability_id}`: turns one of the organization's abilities on or off here.
pub(crate) async fn set_for_workspace(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path((workspace_id, ability_id)): Path<(String, String)>,
    Json(request): Json<EnabledRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::require_workspace_admin(&state, &subject, &workspace_id).await?;
    state
        .storage
        .set_workspace_ability(&workspace_id, &ability_id, request.enabled, &subject.user_id)
        .await
        .map_err(map_storage_error)?;
    let ability = state
        .storage
        .get_cairn_ability(&ability_id)
        .await
        .map_err(map_storage_error)?;
    audit(
        &state,
        &subject,
        "workspace_cairn_ability_switched",
        format!(
            "{} the organization's ability {} ({}) in workspace {workspace_id}.",
            if request.enabled { "Turned on" } else { "Turned off" },
            ability.name,
            ability.command
        ),
    )
    .await;
    Ok(Json(serde_json::json!({ "enabled": request.enabled })))
}

/// What the chat's + menu and the `/commands` need to know about an ability.
#[derive(Debug, Serialize)]
pub(crate) struct OfferedAbility {
    id: String,
    name: String,
    command: String,
    description: String,
    /// `organization` or `workspace`.
    scope: String,
    version: i64,
    inputs: Vec<repomemo_storage::AbilityInput>,
    /// The workspace's model does not run in Agent mode, so the ability cannot run here.
    needs_agent_mode: bool,
}

/// The abilities a workspace offers its people, for the capabilities response.
pub(crate) async fn offered(
    state: &AppState,
    workspace_id: &str,
    agent_mode: bool,
) -> Result<Vec<OfferedAbility>, ApiError> {
    Ok(state
        .storage
        .abilities_for_workspace(workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .map(|ability| OfferedAbility {
            id: ability.id,
            name: ability.name,
            command: ability.command,
            description: ability.description,
            scope: ability.scope,
            version: ability.version,
            inputs: ability.inputs,
            needs_agent_mode: !agent_mode,
        })
        .collect())
}
