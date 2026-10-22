//! Workspace templates over HTTP (roadmap F26a, V0.2.167): the catalogue, a workspace's template and the
//! change of it. A template never reads or changes a right (decision 26): who may do what is settled before and
//! apart from it. Reading is for any member; choosing one needs a workspace owner or administrator with a
//! session or an `admin` token.

use repomemo_domain::{TemplateDetail, TemplateSummary, UpdateWorkspaceTemplateRequest, WorkspaceTemplateView};

use crate::*;

/// The templates this server offers, `general` first.
pub(crate) async fn list_templates(
    _subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Json<Vec<TemplateSummary>> {
    Json(state.core.template_summaries())
}

/// A template of the catalogue as a person looks at it before choosing it (V0.2.177): what it arranges and names.
pub(crate) async fn template_detail(
    _subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(template_id): Path<String>,
) -> Result<Json<TemplateDetail>, ApiError> {
    state
        .core
        .template_detail(&template_id)
        .map(Json)
        .ok_or_else(|| ApiError::not_found("That template was not found."))
}

/// The template the workspace uses and where it came from.
pub(crate) async fn workspace_template(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceTemplateView>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let effective = state
        .core
        .effective_template(&workspace_id)
        .await
        .map_err(map_core_error)?;
    Ok(Json(effective.view()))
}

/// Chooses the workspace's template, or returns it to the default. A change is in the activity feed.
pub(crate) async fn update_workspace_template(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<UpdateWorkspaceTemplateRequest>,
) -> Result<Json<WorkspaceTemplateView>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let change = state
        .core
        .set_workspace_template(
            &workspace_id,
            request.template_id.as_deref(),
            request.overrides.as_ref(),
        )
        .await
        .map_err(map_core_error)?;
    if change.changed || change.adjusted {
        let cleared = change
            .effective
            .reference
            .as_ref()
            .is_some_and(|reference| reference.overrides.is_null());
        let summary = match (&request.template_id, change.changed) {
            (None, _) => "Returned the workspace template to the default.".to_owned(),
            (Some(_), true) => format!(
                "Changed the workspace template to {}.",
                change.effective.template.meta.name
            ),
            (Some(_), false) if cleared => "Reset the workspace template's adjustments.".to_owned(),
            (Some(_), false) => "Adjusted the workspace template.".to_owned(),
        };
        record_workspace_activity(
            &state,
            &workspace_id,
            &subject.user_id,
            "workspace_template_changed",
            "workspace",
            Some(&workspace_id),
            summary,
        )
        .await;
    }
    Ok(Json(change.effective.view()))
}
