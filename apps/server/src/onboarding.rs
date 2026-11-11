//! The first-value checklist (V0.6.1): four steps derived from what exists in the workspace, a per-person dismissal and
//! nothing that needs an AI provider or a mail server.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_domain::{OnboardingStep, WorkspaceOnboarding};
use repomemo_storage::OnboardingFacts;
use serde::Deserialize;

use crate::{map_storage_error, require_workspace_read, ApiError, AppState, AuthenticatedSubject};

/// The steps for a workspace as it stands. `noun` is the template's word for evidence, so the text reads like the
/// workspace; the ids and the rule for "done" never change.
pub(crate) fn onboarding_steps(facts: &OnboardingFacts, noun: &str) -> Vec<OnboardingStep> {
    let step = |id: &str, title: &str, detail: String, done: bool| OnboardingStep {
        id: id.to_owned(),
        title: title.to_owned(),
        detail,
        done,
    };
    vec![
        step(
            "add_evidence",
            &format!("Add your first {noun}"),
            "Upload a file, write a note or link a repository. Nothing here needs an AI provider.".to_owned(),
            facts.artifacts > 0,
        ),
        step(
            "searchable",
            "Make it searchable",
            format!(
                "{} of {} indexed. Search works on indexed text, with or without AI.",
                facts.indexed, facts.artifacts
            ),
            facts.indexed > 0,
        ),
        step(
            "cited_memory",
            "Save a memory card that cites it",
            "Keep a conclusion together with the passage it rests on.".to_owned(),
            facts.cited_cards > 0,
        ),
        step(
            "invite",
            "Bring in a teammate",
            "Optional: invite someone so the knowledge is shared.".to_owned(),
            facts.members > 1,
        ),
    ]
}

pub(crate) async fn get_onboarding(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceOnboarding>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let facts = state
        .storage
        .onboarding_facts(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let dismissed = state
        .storage
        .onboarding_dismissed(&subject.user_id, &workspace_id)
        .await
        .map_err(map_storage_error)?;
    let (template_name, noun) = match state.core.effective_template(&workspace_id).await {
        Ok(effective) => {
            let view = effective.view();
            (view.template.name, "piece of evidence".to_owned())
        }
        Err(_) => ("General".to_owned(), "piece of evidence".to_owned()),
    };
    let steps = onboarding_steps(&facts, &noun);
    let done_count = steps.iter().filter(|step| step.done).count() as i64;
    Ok(Json(WorkspaceOnboarding {
        total_count: steps.len() as i64,
        done_count,
        steps,
        dismissed,
        template_name,
    }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct DismissRequest {
    dismissed: bool,
}

/// Hides the checklist for the caller, or brings it back. It changes nothing for anyone else.
pub(crate) async fn set_onboarding_dismissed(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<DismissRequest>,
) -> Result<StatusCode, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .set_onboarding_dismissed(&subject.user_id, &workspace_id, request.dismissed)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}
