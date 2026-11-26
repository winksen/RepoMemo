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
    models: Vec<WorkspaceModel>,
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
    /// The version the caller read; 0 for a workspace that never saved any.
    expected_version: i64,
}

/// The models of a workspace's organization that the workspace can offer: those the organization enabled, or, when it saved no
/// settings, those that can be chosen now.
async fn organization_models(
    state: &AppState,
    workspace_id: &str,
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
        if model.purpose != "text" {
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
        organization_models(&state, &workspace_id).await?;
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
                offered: settings.version == 0 || settings.policy.offered_model_ids.contains(&id),
                answering: answering.as_deref() == Some(id.as_str()),
                model,
            }
        })
        .collect();
    Ok(Json(WorkspaceCairnView {
        settings,
        organization_default_model_id,
        organization_decided,
        models,
    }))
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
    let (allowed, _, _) = organization_models(&state, &workspace_id).await?;
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
    let policy = WorkspaceCairnPolicy {
        offered_model_ids: offered,
        default_model_id: request.default_model_id,
        model_roles,
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
