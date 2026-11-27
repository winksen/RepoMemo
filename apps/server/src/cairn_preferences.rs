//! A person's CAIRN preferences (V0.9.114): read and saved by the person alone. The server applies them in the person's own runs
//! (see `agent.rs`); nothing here is visible to anyone else.

use axum::{extract::State, Json};
use repomemo_storage::CairnPreferences;
use serde::Serialize;

use crate::{map_storage_error, ApiError, AppState, AuthenticatedSubject, ManagingSubject};

#[derive(Debug, Serialize)]
pub(crate) struct PreferencesView {
    #[serde(flatten)]
    preferences: CairnPreferences,
    /// When the person last saved them; none when they never did.
    updated_at: Option<String>,
    /// The defaults their organizations set for people who have not chosen (V0.9.115), for the organizations that set any.
    organization_defaults: Vec<OrganizationDefaults>,
}

#[derive(Debug, Serialize)]
pub(crate) struct OrganizationDefaults {
    organization_id: String,
    organization_name: String,
    language: Option<String>,
    length: Option<String>,
    steps_open: Option<bool>,
    mode: Option<String>,
}

/// `GET /v1/profile/cairn`: the caller's preferences.
pub(crate) async fn get_preferences(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<PreferencesView>, ApiError> {
    let (preferences, updated_at) = state
        .storage
        .get_cairn_preferences(&subject.user_id)
        .await
        .map_err(map_storage_error)?;
    let mut organization_defaults = Vec::new();
    for organization in state
        .storage
        .list_organizations_for_user(&subject.user_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .filter(|organization| !organization.guest)
    {
        let defaults = state
            .storage
            .get_organization_cairn_settings(&organization.id)
            .await
            .map_err(map_storage_error)?
            .policy
            .preference_defaults;
        if defaults.language.is_some()
            || defaults.length.is_some()
            || defaults.steps_open.is_some()
            || defaults.mode.is_some()
        {
            organization_defaults.push(OrganizationDefaults {
                organization_id: organization.id,
                organization_name: organization.name,
                language: defaults.language,
                length: defaults.length,
                steps_open: defaults.steps_open,
                mode: defaults.mode,
            });
        }
    }
    Ok(Json(PreferencesView {
        preferences,
        updated_at,
        organization_defaults,
    }))
}

/// `PUT /v1/profile/cairn`: saves the caller's preferences as given; a field left out is no preference.
pub(crate) async fn save_preferences(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Json(mut preferences): Json<CairnPreferences>,
) -> Result<Json<PreferencesView>, ApiError> {
    preferences.language = preferences
        .language
        .map(|language| language.trim().to_owned())
        .filter(|language| !language.is_empty());
    preferences
        .models
        .retain(|workspace, model| !workspace.trim().is_empty() && !model.trim().is_empty());
    state
        .storage
        .save_cairn_preferences(&subject.user_id, &preferences)
        .await
        .map_err(map_storage_error)?;
    get_preferences(subject, State(state)).await
}
