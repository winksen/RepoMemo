//! An organization's CAIRN settings (V0.9.82): which catalog models it uses, its default and its Agent-mode default. Members read them;
//! the organization's owners and admins, and CAIRN and system administrators, change them. A change names the version it read, so two
//! administrators cannot overwrite each other unseen, and every change is an audit event with its before and after.

use axum::{
    extract::{Path, State},
    Json,
};
use repomemo_domain::OrganizationRole;
use repomemo_storage::{
    CairnSaved, OrganizationCairnPolicy, OrganizationCairnSettings, AGENT_MODES, ORGANIZATION_MODELS_MAX,
};
use serde::Deserialize;

use crate::{
    map_storage_error,
    system::{audit, require_cairn_admin, require_cairn_reader},
    ApiError, AppState, AuthenticatedSubject, ManagingSubject,
};

#[derive(Debug, Deserialize)]
pub(crate) struct SettingsRequest {
    #[serde(default)]
    enabled_model_ids: Vec<String>,
    #[serde(default)]
    default_model_id: Option<String>,
    #[serde(default)]
    agent_mode: Option<String>,
    /// The roles that may use a model, by model id (V0.9.87); a model with no entry is open to everyone who may use AI.
    #[serde(default)]
    model_roles: std::collections::BTreeMap<String, Vec<String>>,
    /// What CAIRN may do in the organization's workspaces (V0.9.91 on).
    #[serde(default)]
    guardrails: repomemo_storage::Guardrails,
    /// The version the caller read; 0 for an organization that never saved any.
    expected_version: i64,
}

/// Whether the caller reads or changes an organization's CAIRN settings. CAIRN and system administrators may do both for every
/// organization; app administrators read; members read their own; owners and admins change their own.
async fn require_access(
    state: &AppState,
    subject: &AuthenticatedSubject,
    organization_id: &str,
    change: bool,
) -> Result<(), ApiError> {
    if !state
        .storage
        .organization_exists(organization_id)
        .await
        .map_err(ApiError::internal)?
    {
        return Err(ApiError::not_found("The organization was not found."));
    }
    if change && require_cairn_admin(subject).is_ok() {
        return Ok(());
    }
    if !change && require_cairn_reader(subject).is_ok() {
        return Ok(());
    }
    let role = state
        .storage
        .organization_role_for_user(&subject.user_id, organization_id)
        .await
        .map_err(ApiError::internal)?;
    match role {
        Some(OrganizationRole::Owner | OrganizationRole::Admin) => Ok(()),
        Some(OrganizationRole::Member) if !change => Ok(()),
        _ => Err(ApiError::forbidden()),
    }
}

/// The models an organization's administrators choose from (V0.9.83): those that can be chosen now, and any the organization already
/// uses, with where they run, what they take and how they did in the live evaluation. For the people who may change the settings.
pub(crate) async fn list_models(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
) -> Result<Json<Vec<crate::cairn_models::ModelView>>, ApiError> {
    require_access(&state, &subject, &organization_id, true).await?;
    let used = state
        .storage
        .get_organization_cairn_settings(&organization_id)
        .await
        .map_err(map_storage_error)?
        .policy
        .enabled_model_ids;
    let models = state.storage.list_cairn_models(None).await.map_err(map_storage_error)?;
    let mut views = Vec::new();
    for model in models {
        let chosen = used.contains(&model.id);
        let view = crate::cairn_models::view(&state, model).await?;
        if view.selectable() || chosen {
            views.push(view);
        }
    }
    Ok(Json(views))
}

/// The guardrails of a request, checked (V0.9.91): only names of jobs and tools that exist.
pub(crate) fn checked_guardrails(
    given: repomemo_storage::Guardrails,
) -> Result<repomemo_storage::Guardrails, ApiError> {
    let in_range = |value: Option<i64>, what: &str| match value {
        Some(value) if !(1..=repomemo_storage::DEFAULT_MAX_CHANGES).contains(&value) => {
            Err(ApiError::bad_request(format!(
                "{what} is a number from 1 to {}.",
                repomemo_storage::DEFAULT_MAX_CHANGES
            )))
        }
        _ => Ok(value),
    };
    Ok(repomemo_storage::Guardrails {
        tools_off: crate::cairn_tools::checked_names(given.tools_off)?,
        max_changes: in_range(given.max_changes, "The most changes in one confirmation")?,
        type_to_confirm_from: in_range(given.type_to_confirm_from, "The count from which the number is typed")?,
    })
}

pub(crate) async fn get_settings(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
) -> Result<Json<OrganizationCairnSettings>, ApiError> {
    require_access(&state, &subject, &organization_id, false).await?;
    state
        .storage
        .get_organization_cairn_settings(&organization_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

/// The names of catalog models, for the audit trail: the id when a model has gone.
async fn names_of(state: &AppState, ids: &[String]) -> String {
    let mut names = Vec::new();
    for id in ids {
        names.push(
            state
                .storage
                .get_cairn_model(id)
                .await
                .map(|model| model.display_name)
                .unwrap_or_else(|_| id.clone()),
        );
    }
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(", ")
    }
}

pub(crate) async fn update_settings(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
    Json(request): Json<SettingsRequest>,
) -> Result<Json<OrganizationCairnSettings>, ApiError> {
    require_access(&state, &subject, &organization_id, true).await?;
    let agent_mode = request.agent_mode.unwrap_or_else(|| "auto".to_owned());
    if !AGENT_MODES.contains(&agent_mode.as_str()) {
        return Err(ApiError::bad_request("Agent mode is auto, agent or jobs."));
    }
    let mut enabled = Vec::new();
    for id in request.enabled_model_ids {
        if !enabled.contains(&id) {
            enabled.push(id);
        }
    }
    if enabled.len() > ORGANIZATION_MODELS_MAX {
        return Err(ApiError::bad_request(format!(
            "An organization uses at most {ORGANIZATION_MODELS_MAX} models."
        )));
    }
    // A model can be chosen only while it, and its connection, are on and a connection that leaves the network is confirmed.
    let before = state
        .storage
        .get_organization_cairn_settings(&organization_id)
        .await
        .map_err(map_storage_error)?;
    for id in &enabled {
        let model = state
            .storage
            .get_cairn_model(id)
            .await
            .map_err(|_| ApiError::bad_request("A chosen model is not in the catalog."))?;
        let connection = state
            .storage
            .get_cairn_connection(&model.connection_id)
            .await
            .map_err(map_storage_error)?;
        // A model the organization already uses stays, so its settings can be saved while its connection is off.
        let kept = before.policy.enabled_model_ids.contains(id);
        let usable = model.enabled && connection.is_usable();
        if !(kept || usable) {
            return Err(ApiError::bad_request(format!(
                "{} cannot be chosen: it, or its connection, is off, or the connection leaves your network and is not confirmed.",
                model.display_name
            )));
        }
    }
    if let Some(default) = &request.default_model_id {
        if !enabled.contains(default) {
            return Err(ApiError::bad_request(
                "The default model is one of the models the organization uses.",
            ));
        }
        let default_model = state
            .storage
            .get_cairn_model(default)
            .await
            .map_err(map_storage_error)?;
        if default_model.purpose != "text" {
            return Err(ApiError::bad_request("The default model is a text model."));
        }
    }
    let model_roles = crate::workspace_cairn::checked_model_roles(request.model_roles, &enabled)?;
    let guardrails = checked_guardrails(request.guardrails)?;
    let policy = OrganizationCairnPolicy {
        enabled_model_ids: enabled,
        default_model_id: request.default_model_id,
        agent_mode,
        model_roles,
        guardrails,
    };
    let saved = state
        .storage
        .save_organization_cairn_settings(&organization_id, request.expected_version, &policy, &subject.user_id)
        .await
        .map_err(map_storage_error)?;
    let saved = match saved {
        CairnSaved::Saved(settings) => settings,
        CairnSaved::Stale(current) => {
            return Err(ApiError::conflict(format!(
                "The organization's CAIRN settings were changed by someone else (now at version {}). Reload them and change them again.",
                current.version
            )))
        }
    };
    let organization_name = state
        .storage
        .list_organizations_for_system_admin(&subject.user_id)
        .await
        .ok()
        .and_then(|all| all.into_iter().find(|organization| organization.id == organization_id))
        .map_or_else(|| organization_id.clone(), |organization| organization.name);
    audit(
        &state,
        &subject,
        "organization_cairn_settings_changed",
        format!(
            "Changed the CAIRN settings of {organization_name}: models from {} to {}; default from {} to {}; Agent mode from {} to {}.",
            names_of(&state, &before.policy.enabled_model_ids).await,
            names_of(&state, &saved.policy.enabled_model_ids).await,
            match &before.policy.default_model_id {
                Some(id) => names_of(&state, std::slice::from_ref(id)).await,
                None => "none".to_owned(),
            },
            match &saved.policy.default_model_id {
                Some(id) => names_of(&state, std::slice::from_ref(id)).await,
                None => "none".to_owned(),
            },
            before.policy.agent_mode,
            saved.policy.agent_mode
        ),
    )
    .await;
    Ok(Json(saved))
}
