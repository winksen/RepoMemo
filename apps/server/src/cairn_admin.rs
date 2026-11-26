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

/// The smallest group of people whose figures are shown (V0.9.102): below it only the group's name and size are.
const SMALLEST_GROUP: i64 = 5;

#[derive(Debug, Deserialize)]
pub(crate) struct UsageQuery {
    /// 7 or 30.
    #[serde(default)]
    days: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UsageRow {
    organization: String,
    workspace: String,
    model: String,
    people: i64,
    /// True when the group is smaller than five people, so its figures are withheld.
    withheld: bool,
    runs: Option<i64>,
    tokens: Option<i64>,
    failures: Option<i64>,
    fallbacks: Option<i64>,
    average_steps: Option<f64>,
    average_seconds: Option<f64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UsageView {
    days: i64,
    rows: Vec<UsageRow>,
    /// The people and figures of every group together, withheld when fewer than five people made the runs.
    total: UsageRow,
}

fn row_of(group: repomemo_storage::UsageGroup) -> UsageRow {
    let withheld = group.people < SMALLEST_GROUP;
    UsageRow {
        organization: group.organization_name,
        workspace: group.workspace_name,
        model: group.model,
        people: group.people,
        withheld,
        runs: (!withheld).then_some(group.runs),
        tokens: (!withheld).then_some(group.tokens),
        failures: (!withheld).then_some(group.failures),
        fallbacks: (!withheld).then_some(group.fallbacks),
        average_steps: (!withheld).then_some(group.average_steps),
        average_seconds: (!withheld).then_some(group.average_seconds),
    }
}

/// CAIRN's usage and health (V0.9.102) over 7 or 30 days: runs, tokens, failures, fallbacks, steps and time for each organization,
/// workspace and model. CAIRN, system and app administrators see every organization, owners and admins their own. Aggregates only: no
/// message, no chat name, no person, and a group of fewer than five people shows its size and nothing else.
pub(crate) async fn usage(
    ReadingSubject(subject): ReadingSubject,
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<UsageQuery>,
) -> Result<Json<UsageView>, ApiError> {
    let days = match query.days.unwrap_or(7) {
        7 => 7,
        30 => 30,
        _ => return Err(ApiError::bad_request("Usage is shown for 7 or 30 days.")),
    };
    let organizations: Option<Vec<String>> = if require_cairn_reader(&subject).is_ok() {
        None
    } else {
        let own: Vec<String> = state
            .storage
            .list_organizations_for_user(&subject.user_id)
            .await
            .map_err(map_storage_error)?
            .into_iter()
            .filter(|organization| {
                !organization.guest && matches!(organization.role, OrganizationRole::Owner | OrganizationRole::Admin)
            })
            .map(|organization| organization.id)
            .collect();
        if own.is_empty() {
            return Err(ApiError::forbidden());
        }
        Some(own)
    };
    let groups = state
        .storage
        .cairn_usage(days, organizations.as_deref())
        .await
        .map_err(map_storage_error)?;
    let total_people = {
        // The people who made any run, counted again over all groups, so a person in two workspaces is one person.
        let mut sum = repomemo_storage::UsageGroup {
            organization_id: None,
            organization_name: "All".to_owned(),
            workspace_id: String::new(),
            workspace_name: "All".to_owned(),
            model: String::new(),
            runs: groups.iter().map(|group| group.runs).sum(),
            tokens: groups.iter().map(|group| group.tokens).sum(),
            failures: groups.iter().map(|group| group.failures).sum(),
            fallbacks: groups.iter().map(|group| group.fallbacks).sum(),
            average_steps: 0.0,
            average_seconds: 0.0,
            people: groups.iter().map(|group| group.people).max().unwrap_or(0),
        };
        let runs = sum.runs.max(1) as f64;
        sum.average_steps = groups
            .iter()
            .map(|group| group.average_steps * group.runs as f64)
            .sum::<f64>()
            / runs;
        sum.average_seconds = groups
            .iter()
            .map(|group| group.average_seconds * group.runs as f64)
            .sum::<f64>()
            / runs;
        sum
    };
    Ok(Json(UsageView {
        days,
        rows: groups.into_iter().map(row_of).collect(),
        total: row_of(total_people),
    }))
}

#[derive(Debug, Serialize)]
pub(crate) struct HistoryEntry {
    id: String,
    at: String,
    who: String,
    action: String,
    /// What changed, with the values before and after. It never holds a key.
    detail: String,
}

/// The administration history (V0.9.103): every change made on the CAIRN administration page and in the organization and workspace
/// CAIRN settings, newest first, with who made it and the values before and after. CAIRN, system and app administrators see all of it;
/// owners and admins of an organization see the changes that name their organizations. It is the System audit trail, filtered.
pub(crate) async fn history(
    ReadingSubject(subject): ReadingSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<HistoryEntry>>, ApiError> {
    let names: Option<Vec<String>> = if require_cairn_reader(&subject).is_ok() {
        None
    } else {
        let own: Vec<String> = state
            .storage
            .list_organizations_for_user(&subject.user_id)
            .await
            .map_err(map_storage_error)?
            .into_iter()
            .filter(|organization| {
                !organization.guest && matches!(organization.role, OrganizationRole::Owner | OrganizationRole::Admin)
            })
            .map(|organization| organization.name)
            .collect();
        if own.is_empty() {
            return Err(ApiError::forbidden());
        }
        Some(own)
    };
    let events = state
        .storage
        .list_system_events(1000)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(
        events
            .into_iter()
            .filter(|event| {
                event.action.starts_with("cairn_")
                    || event.action.starts_with("organization_cairn_")
                    || event.action.starts_with("workspace_cairn_")
            })
            .filter(|event| {
                names
                    .as_ref()
                    .is_none_or(|names| names.iter().any(|name| event.detail.contains(name.as_str())))
            })
            .map(|event| HistoryEntry {
                id: event.id,
                at: event.created_at,
                who: event
                    .actor
                    .map_or_else(|| "The server".to_owned(), |actor| actor.display_name),
                action: event.action,
                detail: event.detail,
            })
            .collect(),
    ))
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
            // What the workspace answers with now: its own model, or the organization's default (V0.9.84).
            let provider = crate::agent::text_provider(&state, &shared.workspace.id).await?;
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
