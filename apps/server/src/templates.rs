//! Workspace templates over HTTP (roadmap F26a, V0.2.167): the catalogue, a workspace's template and the
//! change of it. A template never reads or changes a right (decision 26): who may do what is settled before and
//! apart from it. Reading is for any member; choosing one needs a workspace owner or administrator with a
//! session or an `admin` token.

use repomemo_domain::{TemplateDetail, TemplateSummary, UpdateWorkspaceTemplateRequest, WorkspaceTemplateView};
use repomemo_templates::{read_folder, Catalogue};

use crate::*;

/// What the operator's folder of templates gave (V0.2.189), kept from the start of the server: shown in System ›
/// Overview and in the server check.
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct TemplateFiles {
    /// The folder (`REPOMEMO_TEMPLATES_DIR`), when one is set.
    pub(crate) folder: Option<String>,
    /// Every template this server offers, `general` first, and where each came from (V0.2.190).
    pub(crate) templates: Vec<TemplateListed>,
    /// The templates it added.
    pub(crate) loaded: Vec<TemplateFileLoaded>,
    /// The files it left out, and why.
    pub(crate) problems: Vec<TemplateFileProblem>,
}

/// One template of the catalogue, as System › Overview and the console list it.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct TemplateListed {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) version: u32,
    /// `code` for general, `built-in` for the server's own files, `custom` for the operator's folder.
    pub(crate) source: &'static str,
    /// The file it came from, for a custom template.
    pub(crate) file: Option<String>,
}

fn listed(catalogue: &Catalogue) -> Vec<TemplateListed> {
    catalogue
        .list()
        .iter()
        .map(|template| {
            let custom = catalogue.is_custom(template.id());
            TemplateListed {
                id: template.id().to_owned(),
                name: template.meta.name.clone(),
                version: template.meta.version,
                source: if custom {
                    "custom"
                } else if template.id() == repomemo_templates::GENERAL_ID {
                    "code"
                } else {
                    "built-in"
                },
                file: custom.then(|| format!("{}.json", template.id())),
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TemplateFileLoaded {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) file: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TemplateFileProblem {
    pub(crate) file: String,
    pub(crate) issues: Vec<TemplateFileIssue>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TemplateFileIssue {
    /// Where in the template the problem is (`sections.order[1]`); empty for the file as a whole.
    pub(crate) path: String,
    pub(crate) message: String,
    /// Where the JSON reader found it, when it did.
    pub(crate) line: Option<usize>,
    pub(crate) column: Option<usize>,
}

/// The templates this server offers: its own, and the valid files of the operator's folder. A file that is not valid
/// is left out and named; nothing here can stop the server.
pub(crate) fn load_catalogue(folder: Option<&std::path::Path>) -> (Catalogue, TemplateFiles) {
    let builtin = Catalogue::builtin();
    let Some(folder) = folder else {
        return (
            builtin.clone(),
            TemplateFiles {
                templates: listed(builtin),
                ..TemplateFiles::default()
            },
        );
    };
    let read = read_folder(folder);
    let (catalogue, refused) = builtin.extended(read.sources.iter().map(|(name, text)| (name.as_str(), text.as_str())));
    let loaded: Vec<TemplateFileLoaded> = catalogue
        .list()
        .iter()
        .filter(|template| catalogue.is_custom(template.id()))
        .map(|template| TemplateFileLoaded {
            id: template.id().to_owned(),
            name: template.meta.name.clone(),
            file: format!("{}.json", template.id()),
        })
        .collect();
    let problems: Vec<TemplateFileProblem> = read
        .problems
        .into_iter()
        .chain(refused)
        .map(|problem| TemplateFileProblem {
            file: problem.source,
            issues: problem
                .issues
                .into_iter()
                .map(|issue| TemplateFileIssue {
                    path: issue.path.clone(),
                    message: issue.message.clone(),
                    line: issue.line,
                    column: issue.column,
                })
                .collect(),
        })
        .collect();
    tracing::info!(folder = %folder.display(), added = loaded.len(), left_out = problems.len(), "Template files read");
    for problem in &problems {
        for issue in &problem.issues {
            let place = if issue.path.is_empty() {
                String::new()
            } else {
                format!(" ({})", issue.path)
            };
            tracing::warn!(file = %problem.file, "A template file was left out: {}{place}", issue.message);
        }
    }
    let templates = listed(&catalogue);
    (
        catalogue,
        TemplateFiles {
            folder: Some(folder.display().to_string()),
            templates,
            loaded,
            problems,
        },
    )
}

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
