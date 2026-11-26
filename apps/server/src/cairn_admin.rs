//! CAIRN's administration page (V0.9.72): who may open it and what it shows. Server administrators (system, CAIRN and app
//! administrators) see every section and, on Overview, how CAIRN is set up today. Owners and admins of an organization see only the
//! sections that concern their organizations. Everyone else is refused. App administrators read and change nothing.

use std::collections::BTreeMap;

use axum::{extract::State, Json};
use repomemo_domain::OrganizationRole;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    map_storage_error,
    system::{cairn_budgets, change_cairn_budgets, require_cairn_admin, require_cairn_reader, CairnBudget},
    ApiError, AppState, AuthenticatedSubject, ReadingSubject,
};

/// Every section of the page, in the order it is shown.
const SERVER_SECTIONS: &[&str] = &[
    "overview",
    "models",
    "organizations",
    "guardrails",
    "abilities",
    "memory",
    "usage",
    "history",
];

/// What an organization's owners and admins reach: only what concerns their organizations.
const ORGANIZATION_SECTIONS: &[&str] = &["organizations", "guardrails", "abilities", "memory", "usage"];

#[derive(Debug, Serialize)]
pub(crate) struct AdminOrganization {
    id: String,
    name: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct AdminWorkspaceSetup {
    workspace_id: String,
    workspace_name: String,
    organization_id: String,
    organization_name: String,
    /// The workspace's first enabled text provider, which CAIRN uses today; none when it has none.
    provider_name: Option<String>,
    provider_type: Option<String>,
    model: Option<String>,
    /// `agent` or `jobs`, as CAIRN answers in the workspace.
    mode: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AdminBudgets {
    /// What CAIRN administrators edit: steps, run seconds, run tokens, background files and trace days.
    settings: Vec<CairnBudget>,
    background_ai_per_day: u32,
    ai_requests_per_hour: u32,
    /// Models the server treats as having passed the live evaluation, from the environment.
    eval_passed_models: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AdminOverview {
    workspaces: Vec<AdminWorkspaceSetup>,
    budgets: AdminBudgets,
}

#[derive(Debug, Serialize)]
pub(crate) struct CairnAdminView {
    /// `server` for system, CAIRN and app administrators; `organization` for owners and admins of organizations.
    scope: &'static str,
    /// Whether the caller may change what the page shows. App administrators may not.
    can_change: bool,
    sections: &'static [&'static str],
    /// The organizations the page concerns: all of them for `server`, the caller's own for `organization`.
    organizations: Vec<AdminOrganization>,
    /// Today's setup, read only; present for `server` only.
    overview: Option<AdminOverview>,
}

/// Who the page is for, and what it shows today.
pub(crate) async fn admin_view(
    ReadingSubject(subject): ReadingSubject,
    State(state): State<AppState>,
) -> Result<Json<CairnAdminView>, ApiError> {
    if require_cairn_reader(&subject).is_ok() {
        let organizations = state
            .storage
            .list_organizations_for_system_admin(&subject.user_id)
            .await
            .map_err(map_storage_error)?;
        let workspaces = state
            .storage
            .list_shared_workspaces_for_system_admin(&subject.user_id)
            .await
            .map_err(map_storage_error)?;
        let mut setups = Vec::with_capacity(workspaces.len());
        for shared in workspaces {
            let provider = state
                .storage
                .list_provider_settings(&shared.workspace.id)
                .await
                .map_err(map_storage_error)?
                .into_iter()
                .find(|setting| setting.enabled && setting.purpose() == "text");
            let organization_name = organizations
                .iter()
                .find(|organization| organization.id == shared.organization_id)
                .map(|organization| organization.name.clone())
                .unwrap_or_default();
            setups.push(AdminWorkspaceSetup {
                workspace_id: shared.workspace.id,
                workspace_name: shared.workspace.name,
                organization_id: shared.organization_id,
                organization_name,
                mode: provider.as_ref().map(crate::ai::cairn_mode_of),
                provider_name: provider.as_ref().map(|provider| provider.name.clone()),
                provider_type: provider.as_ref().map(|provider| provider.provider_type.clone()),
                model: provider
                    .and_then(|provider| provider.model)
                    .filter(|model| !model.trim().is_empty()),
            });
        }
        setups.sort_by(|left, right| {
            (&left.organization_name, &left.workspace_name).cmp(&(&right.organization_name, &right.workspace_name))
        });
        let settings = state.settings();
        return Ok(Json(CairnAdminView {
            scope: "server",
            can_change: require_cairn_admin(&subject).is_ok(),
            sections: SERVER_SECTIONS,
            organizations: organizations
                .into_iter()
                .map(|organization| AdminOrganization {
                    id: organization.id,
                    name: organization.name,
                })
                .collect(),
            overview: Some(AdminOverview {
                workspaces: setups,
                budgets: AdminBudgets {
                    settings: cairn_budgets(&state).await?,
                    background_ai_per_day: settings.background_ai_per_day,
                    ai_requests_per_hour: settings.ai_quota.limit,
                    eval_passed_models: repomemo_ai::agent_default_models(),
                },
            }),
        }));
    }

    let organizations: Vec<AdminOrganization> = state
        .storage
        .list_organizations_for_user(&subject.user_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .filter(|organization| {
            !organization.guest && matches!(organization.role, OrganizationRole::Owner | OrganizationRole::Admin)
        })
        .map(|organization| AdminOrganization {
            id: organization.id,
            name: organization.name,
        })
        .collect();
    if organizations.is_empty() {
        return Err(ApiError::forbidden_because(
            "Only CAIRN, system and app administrators, and owners and admins of an organization, can open the CAIRN administration.",
        ));
    }
    Ok(Json(CairnAdminView {
        scope: "organization",
        // What an organization's admins may change arrives with the rows that add those settings.
        can_change: true,
        sections: ORGANIZATION_SECTIONS,
        organizations,
        overview: None,
    }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct BudgetsRequest {
    /// Budgets to set, by key.
    #[serde(default)]
    values: BTreeMap<String, Value>,
    /// Budgets to restore to the environment's value, by key.
    #[serde(default)]
    reset: Vec<String>,
}

/// Changes the CAIRN run budgets (V0.9.74): for CAIRN and system administrators, with a session or a `system`-scope token. A change
/// applies to the next run and is audited with each budget's value before and after.
pub(crate) async fn update_budgets(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<BudgetsRequest>,
) -> Result<Json<Vec<CairnBudget>>, ApiError> {
    require_cairn_admin(&subject)?;
    change_cairn_budgets(&state, &subject, &request.values, &request.reset).await?;
    cairn_budgets(&state).await.map(Json)
}
