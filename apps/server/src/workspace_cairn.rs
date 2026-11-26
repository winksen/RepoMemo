//! A workspace's CAIRN settings (V0.9.85): which of its organization's models it offers and which is its default. Its administrators see
//! only the organization's models, marked as coming from it, and never others; a change names the version it read and is an audit event
//! with its before and after. The settings travel with the workspace's archive (see `workspace_import`).

use axum::{
    extract::{Path, State},
    Json,
};
use repomemo_storage::{CairnSaved, WorkspaceCairnPolicy, WorkspaceCairnSettings, ORGANIZATION_MODELS_MAX};
use serde::{Deserialize, Serialize};

use crate::{
    cairn_models::{view, ModelView},
    map_storage_error,
    system::audit,
    ApiError, AppState, AuthenticatedSubject, ManagingSubject,
};

/// The role limits of a model list, checked (V0.9.87): only models in the list, only the roles a workspace has, and a limit that names
/// every role is no limit. Owners and admins may always use a model, so they are never left out.
pub(crate) fn checked_model_roles(
    given: std::collections::BTreeMap<String, Vec<String>>,
    models: &[String],
) -> Result<std::collections::BTreeMap<String, Vec<String>>, ApiError> {
    let mut checked = std::collections::BTreeMap::new();
    for (model_id, roles) in given {
        if !models.contains(&model_id) {
            return Err(ApiError::bad_request(
                "A role limit names a model that is not in the list.",
            ));
        }
        let mut kept: Vec<String> = Vec::new();
        for role in roles.iter().map(String::as_str).chain(["owner", "admin"]) {
            if !matches!(role, "owner" | "admin" | "member" | "viewer") {
                return Err(ApiError::bad_request("A role is owner, admin, member or viewer."));
            }
            if !kept.iter().any(|existing| existing == role) {
                kept.push(role.to_owned());
            }
        }
        if kept.len() < 4 {
            checked.insert(model_id, kept);
        }
    }
    Ok(checked)
}

#[derive(Debug, Serialize)]
pub(crate) struct WorkspaceModel {
    #[serde(flatten)]
    model: ModelView,
    /// Always true: a workspace is shown only what its organization offers.
    from_organization: bool,
    /// The workspace offers it to its people.
    offered: bool,
    /// It is the model that answers in the workspace now.
    answering: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct WorkspaceCairnView {
    #[serde(flatten)]
    settings: WorkspaceCairnSettings,
    /// The organization's default, which serves the workspace when its own default is not offered.
    organization_default_model_id: Option<String>,
    /// The organization saved its settings: only the models it enabled are shown.
    organization_decided: bool,
    /// What the organization turned off, which the workspace cannot turn on (V0.9.91).
    organization_tools_off: Vec<String>,
    /// What the organization set for one confirmation, which the workspace can only tighten (V0.9.92).
    organization_limits: OrganizationLimits,
    /// The workspace's folders, each marked when it is closed to CAIRN (V0.9.93).
    folders: Vec<repomemo_domain::Folder>,
    models: Vec<WorkspaceModel>,
    /// The embedding and vision models of the catalog the organization enabled, and the one the workspace uses now (V0.9.89).
    embedding: PurposeChoice,
    vision: PurposeChoice,
}

#[derive(Debug, Serialize)]
pub(crate) struct OrganizationLimits {
    max_changes: Option<i64>,
    type_to_confirm_from: Option<i64>,
    secrets_for_cloud: Option<String>,
    mask_personal_data: bool,
    strict_citations: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct PurposeChoice {
    /// The catalog model the workspace's provider of this purpose is linked to, when it has one on.
    current_model_id: Option<String>,
    /// The name of the model the workspace's provider uses when it is not one of the catalog's.
    current_name: Option<String>,
    candidates: Vec<ModelView>,
}

async fn purpose_choice(state: &AppState, workspace_id: &str, purpose: &str) -> Result<PurposeChoice, ApiError> {
    let (candidates, _, _) = organization_models(state, workspace_id, purpose).await?;
    let provider = state
        .storage
        .list_provider_settings(workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .find(|setting| setting.enabled && setting.purpose() == purpose);
    Ok(PurposeChoice {
        current_model_id: provider
            .as_ref()
            .and_then(|provider| provider.metadata.get("cairn_model_id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        current_name: provider.map(|provider| {
            if purpose == "embedding" {
                provider.embedding_model_name().to_owned()
            } else {
                provider.model.unwrap_or(provider.name)
            }
        }),
        candidates,
    })
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkspaceCairnRequest {
    #[serde(default)]
    offered_model_ids: Vec<String>,
    #[serde(default)]
    default_model_id: Option<String>,
    /// The roles that may use a model in this workspace, by model id (V0.9.87), within what the organization allows.
    #[serde(default)]
    model_roles: std::collections::BTreeMap<String, Vec<String>>,
    /// What the workspace turns off or limits further than its organization (V0.9.91 on).
    #[serde(default)]
    guardrails: repomemo_storage::Guardrails,
    /// The version the caller read; 0 for a workspace that never saved any.
    expected_version: i64,
}

/// The models of a workspace's organization that the workspace can offer: those the organization enabled, or, when it saved no
/// settings, those that can be chosen now.
async fn organization_models(
    state: &AppState,
    workspace_id: &str,
    purpose: &str,
) -> Result<(Vec<ModelView>, bool, Option<String>), ApiError> {
    let organization = match state
        .storage
        .workspace_organization(workspace_id)
        .await
        .map_err(map_storage_error)?
    {
        Some(id) => Some(
            state
                .storage
                .get_organization_cairn_settings(&id)
                .await
                .map_err(map_storage_error)?,
        ),
        None => None,
    };
    let decided = organization.as_ref().is_some_and(|settings| settings.version > 0);
    let default = organization
        .as_ref()
        .and_then(|settings| settings.policy.default_model_id.clone());
    let mut views = Vec::new();
    for model in state.storage.list_cairn_models(None).await.map_err(map_storage_error)? {
        if model.purpose != purpose {
            continue;
        }
        let enabled = organization
            .as_ref()
            .filter(|settings| settings.version > 0)
            .is_none_or(|settings| settings.policy.enabled_model_ids.contains(&model.id));
        if !enabled {
            continue;
        }
        let view = view(state, model).await?;
        // Without a decision of the organization, only what can be chosen now is shown.
        if decided || view.selectable() {
            views.push(view);
        }
    }
    Ok((views, decided, default))
}

pub(crate) async fn get_settings(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceCairnView>, ApiError> {
    crate::require_workspace_admin(&state, &subject, &workspace_id).await?;
    let settings = state
        .storage
        .get_workspace_cairn_settings(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let (models, organization_decided, organization_default_model_id) =
        organization_models(&state, &workspace_id, "text").await?;
    let organization_tools_off = state
        .storage
        .tools_off_for_workspace(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .filter(|(_, by)| *by == "organization")
        .map(|(name, _)| name)
        .collect();
    let mut folders = state
        .storage
        .list_folders(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let closed = state
        .storage
        .closed_folder_ids(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    for folder in &mut folders {
        folder.closed_to_cairn = closed.contains(&folder.id);
    }
    let organization_limits = match state
        .storage
        .workspace_organization(&workspace_id)
        .await
        .map_err(map_storage_error)?
    {
        Some(id) => {
            let guardrails = state
                .storage
                .get_organization_cairn_settings(&id)
                .await
                .map_err(map_storage_error)?
                .policy
                .guardrails;
            OrganizationLimits {
                max_changes: guardrails.max_changes,
                type_to_confirm_from: guardrails.type_to_confirm_from,
                secrets_for_cloud: guardrails.secrets_for_cloud,
                mask_personal_data: guardrails.mask_personal_data,
                strict_citations: guardrails.strict_citations,
            }
        }
        None => OrganizationLimits {
            max_changes: None,
            type_to_confirm_from: None,
            secrets_for_cloud: None,
            mask_personal_data: false,
            strict_citations: false,
        },
    };
    let answering = state
        .storage
        .resolve_text_provider(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .resolved
        .and_then(|resolved| {
            resolved
                .provider
                .metadata
                .get("cairn_model_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        });
    let models = models
        .into_iter()
        .map(|model| {
            let id = model.id().to_owned();
            WorkspaceModel {
                from_organization: true,
                offered: settings.version == 0
                    || settings.policy.offered_model_ids.is_empty()
                    || settings.policy.offered_model_ids.contains(&id),
                answering: answering.as_deref() == Some(id.as_str()),
                model,
            }
        })
        .collect();
    Ok(Json(WorkspaceCairnView {
        settings,
        organization_default_model_id,
        organization_decided,
        organization_tools_off,
        organization_limits,
        folders,
        models,
        embedding: purpose_choice(&state, &workspace_id, "embedding").await?,
        vision: purpose_choice(&state, &workspace_id, "vision").await?,
    }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChooseModelRequest {
    /// The catalog model to use, or none to turn the workspace's provider of this purpose off.
    #[serde(default)]
    model_id: Option<String>,
    /// The administrator confirms that changing the embedding model removes the stored vectors and embeds every passage again.
    #[serde(default)]
    confirm_rebuild: bool,
}

/// Chooses a workspace's embedding or vision model from the catalog (V0.9.89): one the organization enabled, of that purpose, that can
/// be chosen now. It becomes the workspace's provider of that purpose, so indexing and search read it as before. Changing the embedding
/// model asks first, because every stored vector is removed and every passage embedded again in the background.
pub(crate) async fn choose_model(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path((workspace_id, purpose)): Path<(String, String)>,
    Json(request): Json<ChooseModelRequest>,
) -> Result<Json<WorkspaceCairnView>, ApiError> {
    crate::require_workspace_admin(&state, &subject, &workspace_id).await?;
    if !matches!(purpose.as_str(), "embedding" | "vision") {
        return Err(ApiError::bad_request(
            "A workspace chooses an embedding or a vision model here.",
        ));
    }
    let existing = state
        .storage
        .list_provider_settings(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .find(|setting| setting.enabled && setting.purpose() == purpose);
    let Some(model_id) = request.model_id else {
        // Turning it off leaves the vectors and the descriptions that were made.
        if let Some(mut provider) = existing {
            provider.enabled = false;
            state
                .core
                .save_provider_settings(provider)
                .await
                .map_err(crate::map_core_error)?;
        }
        return get_settings(subject, State(state), Path(workspace_id)).await;
    };
    let (allowed, _, _) = organization_models(&state, &workspace_id, &purpose).await?;
    let Some(model) = allowed.iter().find(|model| model.id() == model_id.as_str()) else {
        return Err(ApiError::bad_request(
            "A workspace chooses only among the models its organization offers for this purpose.",
        ));
    };
    if !model.selectable() {
        return Err(ApiError::bad_request(format!(
            "{} cannot be chosen: it, or its connection, is off, or the connection leaves your network and is not confirmed.",
            model.name()
        )));
    }
    let catalog = state
        .storage
        .get_cairn_model(&model_id)
        .await
        .map_err(map_storage_error)?;
    let connection = state
        .storage
        .get_cairn_connection(&catalog.connection_id)
        .await
        .map_err(map_storage_error)?;
    let changes_embedding = purpose == "embedding"
        && existing
            .as_ref()
            .is_none_or(|provider| provider.embedding_model_name() != catalog.model_id);
    let stored = if changes_embedding {
        state
            .storage
            .embedding_count(&workspace_id)
            .await
            .map_err(map_storage_error)?
    } else {
        0
    };
    if stored > 0 && !request.confirm_rebuild {
        return Err(ApiError::conflict(format!(
            "Changing the embedding model to {} removes the {stored} stored vector(s) of this workspace and embeds every passage again in the background. Search by meaning is partial until it finishes; keyword search is not affected. Confirm to continue.",
            catalog.display_name
        )));
    }
    let key = state
        .storage
        .cairn_connection_key(&connection.id)
        .await
        .map_err(map_storage_error)?;
    let settings = repomemo_domain::ProviderSettings {
        id: existing
            .as_ref()
            .map(|provider| provider.id.clone())
            .unwrap_or_default(),
        workspace_id: Some(workspace_id.clone()),
        provider_type: connection.kind.clone(),
        name: catalog.display_name.clone(),
        base_url: Some(connection.base_url.clone()),
        model: Some(catalog.model_id.clone()),
        embedding_model: (purpose == "embedding").then(|| catalog.model_id.clone()),
        enabled: true,
        metadata: serde_json::json!({
            "purpose": purpose,
            "cloud_content_acknowledged": connection.cloud_acknowledged_at.is_some(),
            "cairn_model_id": catalog.id,
        }),
        api_key: key,
    };
    repomemo_ai::validate_settings(&settings).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let saved = state
        .core
        .save_provider_settings(settings)
        .await
        .map_err(crate::map_core_error)?;
    if purpose == "embedding" {
        if changes_embedding && stored > 0 {
            state
                .storage
                .clear_workspace_embeddings(&workspace_id)
                .await
                .map_err(map_storage_error)?;
        }
        state.embedding_queue.request(&workspace_id);
    } else {
        // Images that were waiting for a provider are described now.
        state.index_queue.resume_pending().await;
    }
    let detail = format!(
        "Chose the {purpose} model {} ({}) for the workspace {workspace_id}.",
        catalog.display_name, saved.name
    );
    crate::record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "cairn_model_chosen",
        "workspace",
        Some(&workspace_id),
        detail.clone(),
    )
    .await;
    audit(&state, &subject, "workspace_cairn_model_chosen", detail).await;
    get_settings(subject, State(state), Path(workspace_id)).await
}

pub(crate) async fn update_settings(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<WorkspaceCairnRequest>,
) -> Result<Json<WorkspaceCairnSettings>, ApiError> {
    crate::require_workspace_admin(&state, &subject, &workspace_id).await?;
    let mut offered = Vec::new();
    for id in request.offered_model_ids {
        if !offered.contains(&id) {
            offered.push(id);
        }
    }
    if offered.len() > ORGANIZATION_MODELS_MAX {
        return Err(ApiError::bad_request(format!(
            "A workspace offers at most {ORGANIZATION_MODELS_MAX} models."
        )));
    }
    let before = state
        .storage
        .get_workspace_cairn_settings(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    // Only what the organization offers, and what can be chosen now unless the workspace already offers it.
    let (allowed, _, _) = organization_models(&state, &workspace_id, "text").await?;
    for id in &offered {
        let Some(model) = allowed.iter().find(|model| model.id() == id.as_str()) else {
            return Err(ApiError::bad_request(
                "A workspace offers only the models its organization offers.",
            ));
        };
        if !(model.selectable() || before.policy.offered_model_ids.contains(id)) {
            return Err(ApiError::bad_request(format!(
                "{} cannot be offered: it, or its connection, is off, or the connection leaves your network and is not confirmed.",
                model.name()
            )));
        }
    }
    if let Some(default) = &request.default_model_id {
        if !offered.contains(default) {
            return Err(ApiError::bad_request(
                "The default model is one of the models the workspace offers.",
            ));
        }
    }
    let model_roles = checked_model_roles(request.model_roles, &offered)?;
    let mut guardrails = crate::organization_cairn::checked_guardrails(request.guardrails)?;
    // Only folders of this workspace can be closed (V0.9.93).
    let folders = state
        .storage
        .list_folders(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    if guardrails
        .closed_folder_ids
        .iter()
        .any(|id| !folders.iter().any(|folder| folder.id == *id))
    {
        return Err(ApiError::bad_request(
            "A folder closed to CAIRN is a folder of this workspace.",
        ));
    }
    guardrails.closed_folder_ids.sort();
    guardrails.closed_folder_ids.dedup();
    // A workspace can be stricter than its organization, never looser.
    if let Some(organization_id) = state
        .storage
        .workspace_organization(&workspace_id)
        .await
        .map_err(map_storage_error)?
    {
        let organization = state
            .storage
            .get_organization_cairn_settings(&organization_id)
            .await
            .map_err(map_storage_error)?
            .policy
            .guardrails;
        if organization.strict_citations && !guardrails.strict_citations {
            return Err(ApiError::bad_request(
                "Your organization asks for strict citations; a workspace cannot turn that off.",
            ));
        }
        if guardrails
            .routines_per_person
            .zip(organization.routines_per_person)
            .is_some_and(|(own, theirs)| own > theirs)
            || guardrails
                .routines_per_workspace
                .zip(organization.routines_per_workspace)
                .is_some_and(|(own, theirs)| own > theirs)
            || (organization.event_routines == Some(false) && guardrails.event_routines != Some(false))
        {
            return Err(ApiError::bad_request(
                "A workspace's routine limits are never looser than its organization's.",
            ));
        }
        if guardrails.monthly_token_budget.is_some() || guardrails.at_budget.is_some() {
            return Err(ApiError::bad_request(
                "The monthly token budget is the organization's, not a workspace's.",
            ));
        }
        let loosens =
            |own: Option<i64>, theirs: Option<i64>| matches!((own, theirs), (Some(own), Some(theirs)) if own > theirs);
        if loosens(guardrails.daily_runs_per_person, organization.daily_runs_per_person)
            || loosens(guardrails.daily_tokens_per_person, organization.daily_tokens_per_person)
        {
            return Err(ApiError::bad_request(
                "A workspace's daily budget is never higher than its organization's.",
            ));
        }
        if organization.mask_personal_data && !guardrails.mask_personal_data {
            return Err(ApiError::bad_request(
                "Your organization masks email addresses and phone numbers for cloud models; a workspace cannot turn that off.",
            ));
        }
        if organization.secrets_for_cloud.as_deref() == Some("refuse")
            && guardrails.secrets_for_cloud.as_deref() == Some("mask")
        {
            return Err(ApiError::bad_request(
                "A workspace cannot send secrets to a cloud model, masked, when its organization refuses to.",
            ));
        }
        let ceiling = organization
            .max_changes
            .unwrap_or(repomemo_storage::DEFAULT_MAX_CHANGES);
        if guardrails.max_changes.is_some_and(|own| own > ceiling) {
            return Err(ApiError::bad_request(format!(
                "A workspace allows at most {ceiling} changes in one confirmation, as its organization does."
            )));
        }
        if let (Some(own), Some(theirs)) = (guardrails.type_to_confirm_from, organization.type_to_confirm_from) {
            if own > theirs {
                return Err(ApiError::bad_request(format!(
                    "A workspace types the number from {theirs} changes at the latest, as its organization asks."
                )));
            }
        }
    }
    let policy = WorkspaceCairnPolicy {
        offered_model_ids: offered,
        default_model_id: request.default_model_id,
        model_roles,
        guardrails,
    };
    let saved = state
        .storage
        .save_workspace_cairn_settings(&workspace_id, request.expected_version, &policy, Some(&subject.user_id))
        .await
        .map_err(map_storage_error)?;
    let saved = match saved {
        CairnSaved::Saved(settings) => settings,
        CairnSaved::Stale(current) => {
            return Err(ApiError::conflict(format!(
                "The workspace's CAIRN settings were changed by someone else (now at version {}). Reload them and change them again.",
                current.version
            )))
        }
    };
    let name_of = |ids: &[String]| {
        let names = ids
            .iter()
            .map(|id| {
                allowed
                    .iter()
                    .find(|model| model.id() == id.as_str())
                    .map_or_else(|| id.clone(), |model| model.name().to_owned())
            })
            .collect::<Vec<_>>();
        if names.is_empty() {
            "none".to_owned()
        } else {
            names.join(", ")
        }
    };
    let detail = format!(
        "Changed the CAIRN settings of the workspace {workspace_id}: models offered from {} to {}; default from {} to {}.",
        if before.version == 0 { "all of the organization's".to_owned() } else { name_of(&before.policy.offered_model_ids) },
        name_of(&saved.policy.offered_model_ids),
        before.policy.default_model_id.as_deref().map_or_else(|| "none".to_owned(), |id| name_of(&[id.to_owned()])),
        saved.policy.default_model_id.as_deref().map_or_else(|| "none".to_owned(), |id| name_of(&[id.to_owned()])),
    );
    // Routines the new limits no longer allow are paused with the reason, not deleted (V0.9.99).
    let _ = state.storage.enforce_routine_limits(&workspace_id).await;
    crate::record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "cairn_settings_updated",
        "workspace",
        Some(&workspace_id),
        detail.clone(),
    )
    .await;
    audit(&state, &subject, "workspace_cairn_settings_changed", detail).await;
    Ok(Json(saved))
}
