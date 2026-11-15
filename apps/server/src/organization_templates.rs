//! Organization templates (V0.6.23): the administrators of an organization save templates that its workspaces can preview
//! and adopt. A template is held to the rules of every template file, so it can arrange and name what a workspace shows
//! and can never grant, widen or hide a right, run anything or move content. Each save is the next version; earlier
//! versions stay readable. A saved template is offered at once, with no restart.

use axum::{
    extract::{Path, State},
    Json,
};
use repomemo_domain::SharedUser;
use serde::{Deserialize, Serialize};

use crate::{
    map_core_error, map_storage_error, require_organization_admin, require_organization_read, ApiError, AppState,
    AuthenticatedSubject, ManagingSubject,
};

/// One organization template as the settings list it: the newest version and who saved it.
#[derive(Debug, Serialize)]
pub(crate) struct OrganizationTemplateView {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) summary: String,
    pub(crate) version: i64,
    pub(crate) saved_by: Option<SharedUser>,
    pub(crate) saved_at: String,
    pub(crate) note: String,
    /// How many versions are saved, the newest included.
    pub(crate) versions: usize,
}

/// A saved version, with the file when the caller asks for one.
#[derive(Debug, Serialize)]
pub(crate) struct OrganizationTemplateVersion {
    pub(crate) version: i64,
    pub(crate) saved_by: Option<SharedUser>,
    pub(crate) saved_at: String,
    pub(crate) note: String,
    pub(crate) definition: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveOrganizationTemplate {
    /// The template file, as the JSON text of a template (the one a workspace's setup is exported as).
    pub(crate) definition: String,
    /// Why this version was saved, for the history.
    #[serde(default)]
    pub(crate) note: String,
}

/// The organization's templates, newest version of each. Anyone in the organization can read them.
pub(crate) async fn list_organization_templates(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
) -> Result<Json<Vec<OrganizationTemplateView>>, ApiError> {
    require_organization_read(&state, &subject, &organization_id).await?;
    let latest = state
        .storage
        .latest_organization_templates(Some(&organization_id))
        .await
        .map_err(map_storage_error)?;
    let mut views = Vec::with_capacity(latest.len());
    for template in latest {
        let versions = state
            .storage
            .organization_template_versions(&organization_id, &template.template_id)
            .await
            .map_err(map_storage_error)?
            .len();
        views.push(OrganizationTemplateView {
            id: template.template_id,
            name: template.name,
            summary: template.summary,
            version: template.version,
            saved_by: template.created_by,
            saved_at: template.created_at,
            note: template.note,
            versions,
        });
    }
    Ok(Json(views))
}

/// Every saved version of one template, newest first, with its file. For the organization's administrators, who edit it.
pub(crate) async fn organization_template_versions(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((organization_id, template_id)): Path<(String, String)>,
) -> Result<Json<Vec<OrganizationTemplateVersion>>, ApiError> {
    require_organization_admin(&state, &subject, &organization_id).await?;
    let versions = state
        .storage
        .organization_template_versions(&organization_id, &template_id)
        .await
        .map_err(map_storage_error)?;
    if versions.is_empty() {
        return Err(ApiError::not_found("That template was not found."));
    }
    Ok(Json(
        versions
            .into_iter()
            .map(|saved| OrganizationTemplateVersion {
                version: saved.version,
                saved_by: saved.created_by,
                saved_at: saved.created_at,
                note: saved.note,
                definition: saved.definition,
            })
            .collect(),
    ))
}

/// Saves a template file as the next version of its id. The file is checked first against the rules of every template
/// file; its `meta.version` must be one more than the newest saved (1 for a new template), so two administrators who
/// edit from the same version cannot overwrite each other.
pub(crate) async fn save_organization_template(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(organization_id): Path<String>,
    Json(request): Json<SaveOrganizationTemplate>,
) -> Result<Json<OrganizationTemplateView>, ApiError> {
    require_organization_admin(&state, &subject, &organization_id).await?;
    if request.definition.len() > repomemo_templates::limits::FILE_BYTES {
        return Err(ApiError::bad_request("That template file is too large."));
    }
    if request.note.chars().count() > 200 {
        return Err(ApiError::bad_request("The note is limited to 200 characters."));
    }
    let template = state
        .core
        .check_organization_template(&organization_id, &request.definition)
        .map_err(|issues| ApiError::bad_request(issues.join("; ")))?;
    state
        .storage
        .save_organization_template(
            &organization_id,
            template.id(),
            i64::from(template.meta.version),
            &template.meta.name,
            &template.meta.summary,
            &request.definition,
            request.note.trim(),
            &subject.user_id,
        )
        .await
        .map_err(map_storage_error)?;
    // Offered at once: the catalogue is rebuilt from what is saved.
    let problems = state
        .core
        .reload_organization_templates()
        .await
        .map_err(map_core_error)?;
    for problem in problems {
        tracing::warn!(source = %problem.source, "An organization template was left out of the catalogue");
    }
    tracing::info!(target: "audit", user_id = %subject.user_id, organization_id = %organization_id, template = %template.id(), version = template.meta.version, "Organization template saved");
    Ok(Json(OrganizationTemplateView {
        id: template.meta.id.clone(),
        name: template.meta.name.clone(),
        summary: template.meta.summary.clone(),
        version: i64::from(template.meta.version),
        saved_by: None,
        saved_at: chrono::Utc::now().to_rfc3339(),
        note: request.note.trim().to_owned(),
        versions: template.meta.version as usize,
    }))
}
