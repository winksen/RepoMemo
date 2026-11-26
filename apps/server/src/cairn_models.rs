//! The models of the catalog's connections (V0.9.77): finding what a server offers, adding chosen models with a name and a purpose,
//! testing each for the tools it takes and its context size, and correcting what the test found. CAIRN and system administrators
//! change them; app administrators read them. A model in use cannot be deleted.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use repomemo_storage::{CairnModel, CairnModelChange, CairnSaved, NewCairnModel};
use serde::{Deserialize, Serialize};

use crate::{
    map_storage_error,
    system::{audit, require_cairn_admin, require_cairn_reader},
    ApiError, AppState, AuthenticatedSubject, ReadingSubject,
};

#[derive(Debug, Serialize)]
pub(crate) struct ModelView {
    #[serde(flatten)]
    model: CairnModel,
    /// In how many places the model is used; a model in use cannot be deleted.
    uses: i64,
    /// `local` or `cloud`, from the connection it runs on: shown wherever a model is listed or chosen (V0.9.81).
    locality: String,
    /// The model and its connection are on, and a connection that leaves the network has its confirmation: only then can an
    /// organization choose the model.
    selectable: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ModelsQuery {
    #[serde(default)]
    connection_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddModelRequest {
    connection_id: String,
    /// The name the model server knows the model by.
    model_id: String,
    #[serde(default)]
    display_name: Option<String>,
    purpose: String,
    #[serde(default)]
    tool_mode: Option<String>,
    #[serde(default)]
    context_tokens: Option<i64>,
    #[serde(default)]
    vision: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChangeModelRequest {
    display_name: String,
    tool_mode: String,
    context_tokens: i64,
    vision: bool,
    enabled: bool,
    /// Another model of the catalog that answers when this one times out, answers with a server error or is rate limited.
    #[serde(default)]
    fallback_model_id: Option<String>,
    expected_version: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct AvailableModel {
    model_id: String,
    /// The purposes the model is already in the catalog for, on this connection.
    added_for: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ModelTest {
    #[serde(flatten)]
    report: repomemo_ai::ModelReport,
    /// The model with the traits the test found, when it succeeded; as it was when it did not.
    model: ModelView,
}

/// The catalog models that take tools natively and have a passing report: Agent mode is offered by default to a workspace whose
/// provider is linked to one. Read again whenever a report is attached or a model changes.
pub(crate) async fn refresh_agent_defaults(storage: &repomemo_storage::StorageEngine) {
    match storage.list_cairn_models(None).await {
        Ok(models) => repomemo_ai::set_catalog_passed_models(
            models
                .into_iter()
                .filter(|model| {
                    model.enabled
                        && model.purpose == "text"
                        && model.tool_mode == "native"
                        && model
                            .evaluation
                            .as_ref()
                            .and_then(|report| report.get("passed"))
                            .and_then(serde_json::Value::as_bool)
                            == Some(true)
                })
                .map(|model| model.id)
                .collect(),
        ),
        Err(error) => tracing::error!(error = %error, "The models with a passing evaluation could not be read"),
    }
}

/// The fallback of the catalog model a provider is linked to, as provider settings for the run (V0.9.80), or none: the model has no
/// fallback, the fallback is off or on a connection that is off, it takes tools differently from the model (the loop speaks one
/// protocol), or it would send what a local model keeps on the network to a cloud server. The rules are checked again here because
/// a connection's address can change after the fallback was saved.
pub(crate) async fn fallback_settings(
    state: &AppState,
    provider: &repomemo_domain::ProviderSettings,
) -> Option<repomemo_domain::ProviderSettings> {
    let model_id = provider.metadata.get("cairn_model_id")?.as_str()?;
    let model = state.storage.get_cairn_model(model_id).await.ok()?;
    let fallback = state
        .storage
        .get_cairn_model(model.fallback_model_id.as_deref()?)
        .await
        .ok()?;
    let own = state.storage.get_cairn_connection(&model.connection_id).await.ok()?;
    let theirs = state.storage.get_cairn_connection(&fallback.connection_id).await.ok()?;
    let usable = fallback.enabled
        && fallback.purpose == "text"
        && theirs.enabled
        && fallback.tool_mode == model.tool_mode
        && !(own.locality == "local" && theirs.locality != "local");
    if !usable {
        tracing::warn!(model = %model.display_name, fallback = %fallback.display_name, "A model's fallback cannot be used");
        return None;
    }
    let key = state.storage.cairn_connection_key(&theirs.id).await.ok()?;
    Some(repomemo_domain::ProviderSettings {
        id: String::new(),
        workspace_id: None,
        provider_type: theirs.kind.clone(),
        name: format!("{} (fallback)", fallback.display_name),
        base_url: Some(theirs.base_url.clone()),
        model: Some(fallback.model_id.clone()),
        embedding_model: None,
        enabled: true,
        metadata: serde_json::json!({
            "purpose": "text",
            "cloud_content_acknowledged": theirs.cloud_acknowledged_at.is_some(),
            "tool_mode": fallback.tool_mode,
            "context_tokens": fallback.context_tokens,
        }),
        api_key: key,
    })
}

/// A live evaluation report, as `npm run cairn:eval` measures it (V0.9.79).
#[derive(Debug, Deserialize)]
pub(crate) struct EvaluationRequest {
    passed: bool,
    /// The day the run was made, `YYYY-MM-DD`.
    run_on: String,
    citation_precision: f64,
    tool_accuracy: f64,
    forbidden_actions: i64,
    cases_run: i64,
    /// The RepoMemo version the report was made at.
    #[serde(default)]
    version: Option<String>,
}

async fn view(state: &AppState, model: CairnModel) -> Result<ModelView, ApiError> {
    let uses = state
        .storage
        .cairn_model_uses(&model.id)
        .await
        .map_err(map_storage_error)?;
    let connection = state
        .storage
        .get_cairn_connection(&model.connection_id)
        .await
        .map_err(map_storage_error)?;
    Ok(ModelView {
        uses,
        locality: connection.locality.clone(),
        selectable: model.enabled && connection.is_usable(),
        model,
    })
}

pub(crate) async fn list_models(
    ReadingSubject(subject): ReadingSubject,
    State(state): State<AppState>,
    Query(query): Query<ModelsQuery>,
) -> Result<Json<Vec<ModelView>>, ApiError> {
    require_cairn_reader(&subject)?;
    let models = state
        .storage
        .list_cairn_models(query.connection_id.as_deref())
        .await
        .map_err(map_storage_error)?;
    let mut views = Vec::with_capacity(models.len());
    for model in models {
        views.push(view(&state, model).await?);
    }
    Ok(Json(views))
}

/// What a connection's server offers, so the administrator can choose. It reaches the server with the connection's key.
pub(crate) async fn available_models(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(connection_id): Path<String>,
) -> Result<Json<Vec<AvailableModel>>, ApiError> {
    require_cairn_admin(&subject)?;
    let connection = state
        .storage
        .get_cairn_connection(&connection_id)
        .await
        .map_err(map_storage_error)?;
    let key = state
        .storage
        .cairn_connection_key(&connection_id)
        .await
        .map_err(map_storage_error)?;
    let ids = repomemo_ai::list_connection_models(&connection.kind, Some(&connection.base_url), key.as_deref())
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let added = state
        .storage
        .list_cairn_models(Some(&connection_id))
        .await
        .map_err(map_storage_error)?;
    Ok(Json(
        ids.into_iter()
            .map(|model_id| AvailableModel {
                added_for: added
                    .iter()
                    .filter(|model| model.model_id == model_id)
                    .map(|model| model.purpose.clone())
                    .collect(),
                model_id,
            })
            .collect(),
    ))
}

pub(crate) async fn add_model(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<AddModelRequest>,
) -> Result<(StatusCode, Json<ModelView>), ApiError> {
    require_cairn_admin(&subject)?;
    let display_name = request
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(request.model_id.trim())
        .to_owned();
    let created = state
        .storage
        .create_cairn_model(
            NewCairnModel {
                connection_id: request.connection_id,
                model_id: request.model_id,
                display_name,
                vision: request.vision.unwrap_or(request.purpose == "vision"),
                purpose: request.purpose,
                tool_mode: request.tool_mode.unwrap_or_else(|| "none".to_owned()),
                context_tokens: request.context_tokens.unwrap_or(0),
                evaluation: None,
                fallback_model_id: None,
                roles: None,
                enabled: true,
            },
            &subject.user_id,
        )
        .await
        .map_err(map_storage_error)?;
    audit(
        &state,
        &subject,
        "cairn_model_added",
        format!(
            "Added the {} model {} ({}).",
            created.purpose, created.display_name, created.model_id
        ),
    )
    .await;
    Ok((StatusCode::CREATED, Json(view(&state, created).await?)))
}

pub(crate) async fn change_model(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<ChangeModelRequest>,
) -> Result<Json<ModelView>, ApiError> {
    require_cairn_admin(&subject)?;
    let existing = state.storage.get_cairn_model(&id).await.map_err(map_storage_error)?;
    let saved = state
        .storage
        .update_cairn_model(
            &id,
            request.expected_version,
            CairnModelChange {
                display_name: request.display_name,
                tool_mode: request.tool_mode,
                context_tokens: request.context_tokens,
                vision: request.vision,
                evaluation: existing.evaluation.clone(),
                fallback_model_id: request.fallback_model_id,
                roles: existing.roles.clone(),
                enabled: request.enabled,
            },
            &subject.user_id,
        )
        .await
        .map_err(map_storage_error)?;
    let saved = match saved {
        CairnSaved::Saved(model) => model,
        CairnSaved::Stale(current) => {
            return Err(ApiError::conflict(format!(
                "The model {} was changed by someone else (now at version {}). Reload it and change it again.",
                current.display_name, current.version
            )))
        }
    };
    let mut changes = Vec::new();
    if existing.display_name != saved.display_name {
        changes.push(format!("name from {} to {}", existing.display_name, saved.display_name));
    }
    if existing.tool_mode != saved.tool_mode {
        changes.push(format!("tools from {} to {}", existing.tool_mode, saved.tool_mode));
    }
    if existing.context_tokens != saved.context_tokens {
        changes.push(format!(
            "context from {} to {}",
            existing.context_tokens, saved.context_tokens
        ));
    }
    if existing.vision != saved.vision {
        changes.push(format!("vision {}", if saved.vision { "on" } else { "off" }));
    }
    if existing.enabled != saved.enabled {
        changes.push(format!("turned {}", if saved.enabled { "on" } else { "off" }));
    }
    if existing.fallback_model_id != saved.fallback_model_id {
        changes.push(match &saved.fallback_model_id {
            Some(id) => format!(
                "fallback {}",
                state
                    .storage
                    .get_cairn_model(id)
                    .await
                    .map(|model| model.display_name)
                    .unwrap_or_else(|_| id.clone())
            ),
            None => "fallback removed".to_owned(),
        });
    }
    refresh_agent_defaults(&state.storage).await;
    audit(
        &state,
        &subject,
        "cairn_model_changed",
        format!(
            "Changed the model {}: {}.",
            saved.display_name,
            if changes.is_empty() {
                "nothing visible".to_owned()
            } else {
                changes.join("; ")
            }
        ),
    )
    .await;
    Ok(Json(view(&state, saved).await?))
}

/// Attaches a live evaluation report to a model: the Models table shows passed, failed or not evaluated with the date, and a text
/// model that takes tools natively and passed is offered Agent mode by default. A newer report replaces the older one.
pub(crate) async fn attach_evaluation(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<EvaluationRequest>,
) -> Result<Json<ModelView>, ApiError> {
    require_cairn_admin(&subject)?;
    if chrono::NaiveDate::parse_from_str(&request.run_on, "%Y-%m-%d").is_err() {
        return Err(ApiError::bad_request("run_on is a date like 2026-11-26."));
    }
    if !(0.0..=1.0).contains(&request.citation_precision) || !(0.0..=1.0).contains(&request.tool_accuracy) {
        return Err(ApiError::bad_request(
            "Citation precision and tool accuracy are between 0 and 1.",
        ));
    }
    if request.forbidden_actions < 0 || request.cases_run < 0 {
        return Err(ApiError::bad_request("The counts cannot be negative."));
    }
    let existing = state.storage.get_cairn_model(&id).await.map_err(map_storage_error)?;
    let report = serde_json::json!({
        "passed": request.passed,
        "run_on": request.run_on,
        "citation_precision": request.citation_precision,
        "tool_accuracy": request.tool_accuracy,
        "forbidden_actions": request.forbidden_actions,
        "cases_run": request.cases_run,
        "version": request.version,
        "attached_at": chrono::Utc::now().to_rfc3339(),
    });
    let saved = state
        .storage
        .update_cairn_model(
            &id,
            existing.version,
            CairnModelChange {
                display_name: existing.display_name.clone(),
                tool_mode: existing.tool_mode.clone(),
                context_tokens: existing.context_tokens,
                vision: existing.vision,
                evaluation: Some(report),
                fallback_model_id: existing.fallback_model_id.clone(),
                roles: existing.roles.clone(),
                enabled: existing.enabled,
            },
            &subject.user_id,
        )
        .await
        .map_err(map_storage_error)?;
    let saved = match saved {
        CairnSaved::Saved(model) => model,
        CairnSaved::Stale(_) => {
            return Err(ApiError::conflict(
                "The model was changed meanwhile. Attach the report again.",
            ))
        }
    };
    refresh_agent_defaults(&state.storage).await;
    audit(
        &state,
        &subject,
        "cairn_model_evaluated",
        format!(
            "Attached a {} evaluation report to the model {} (run on {}, citation precision {:.2}, tool accuracy {:.2}, {} forbidden actions).",
            if request.passed { "passing" } else { "failing" },
            saved.display_name,
            request.run_on,
            request.citation_precision,
            request.tool_accuracy,
            request.forbidden_actions
        ),
    )
    .await;
    Ok(Json(view(&state, saved).await?))
}

pub(crate) async fn delete_model(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_cairn_admin(&subject)?;
    let existing = state.storage.get_cairn_model(&id).await.map_err(map_storage_error)?;
    state.storage.delete_cairn_model(&id).await.map_err(map_storage_error)?;
    refresh_agent_defaults(&state.storage).await;
    audit(
        &state,
        &subject,
        "cairn_model_deleted",
        format!("Deleted the model {} ({}).", existing.display_name, existing.model_id),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// Tries a model, and fills the tool mode, the context size and, for a vision model, the vision trait with what the test found. The
/// connection must be on: a model of a connection that is off, or that leaves the network without the confirmation, is not called.
pub(crate) async fn test_model(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ModelTest>, ApiError> {
    require_cairn_admin(&subject)?;
    let model = state.storage.get_cairn_model(&id).await.map_err(map_storage_error)?;
    let connection = state
        .storage
        .get_cairn_connection(&model.connection_id)
        .await
        .map_err(map_storage_error)?;
    if !connection.enabled {
        return Err(ApiError::bad_request(format!(
            "The connection {} is off; turn it on to test its models.",
            connection.name
        )));
    }
    let key = state
        .storage
        .cairn_connection_key(&connection.id)
        .await
        .map_err(map_storage_error)?;
    let report = repomemo_ai::test_catalog_model(
        &connection.kind,
        Some(&connection.base_url),
        key.as_deref(),
        &model.model_id,
        &model.purpose,
    )
    .await
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let model = if report.success && (report.tool_mode.is_some() || report.context_tokens.is_some()) {
        let saved = state
            .storage
            .update_cairn_model(
                &id,
                model.version,
                CairnModelChange {
                    display_name: model.display_name.clone(),
                    tool_mode: report.tool_mode.clone().unwrap_or_else(|| model.tool_mode.clone()),
                    context_tokens: report.context_tokens.map_or(model.context_tokens, i64::from),
                    vision: model.vision,
                    evaluation: model.evaluation.clone(),
                    fallback_model_id: model.fallback_model_id.clone(),
                    roles: model.roles.clone(),
                    enabled: model.enabled,
                },
                &subject.user_id,
            )
            .await
            .map_err(map_storage_error)?;
        match saved {
            CairnSaved::Saved(saved) | CairnSaved::Stale(saved) => saved,
        }
    } else {
        model
    };
    refresh_agent_defaults(&state.storage).await;
    audit(
        &state,
        &subject,
        "cairn_model_tested",
        format!(
            "Tested the model {}: {}.",
            model.display_name,
            if report.success {
                "it answered"
            } else {
                "it did not answer"
            }
        ),
    )
    .await;
    Ok(Json(ModelTest {
        report,
        model: view(&state, model).await?,
    }))
}
