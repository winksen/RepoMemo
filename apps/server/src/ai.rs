use crate::*;

pub(crate) async fn generate_workspace_ai_overview(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceAiOverview>, ApiError> {
    let role = require_workspace_read(&state, &subject, &workspace_id).await?;
    let provider = state
        .storage
        .list_provider_settings(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .find(|setting| setting.enabled && setting.purpose() == "text");

    let Some(provider) = provider else {
        return Ok(Json(WorkspaceAiOverview {
            provider_configured: false,
            provider_name: None,
            summary_markdown: None,
            citations: Vec::new(),
            warnings: vec![
                "No enabled AI provider is configured for this workspace. Evidence remains available locally; configure a workspace provider before generating an AI overview.".to_owned(),
            ],
        }));
    };

    ai_access::authorize_ai_use(&state, &subject, &workspace_id, &role).await?;
    let result = state
        .core
        .summarize_workspace(workspace_id.clone(), provider.id.clone())
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "ai_overview_generated",
        "workspace",
        Some(&workspace_id),
        "Generated a citation-backed AI workspace overview.".to_owned(),
    )
    .await;
    Ok(Json(WorkspaceAiOverview {
        provider_configured: true,
        provider_name: Some(provider.name),
        summary_markdown: Some(result.summary_markdown),
        citations: result.citations,
        warnings: result.warnings,
    }))
}

pub(crate) async fn ask_workspace(
    ReadingSubject(subject): ReadingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<AskWorkspaceRequest>,
) -> Result<Json<AskAnswer>, ApiError> {
    let role = require_workspace_read(&state, &subject, &workspace_id).await?;
    let provider = state
        .storage
        .list_provider_settings(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .find(|setting| setting.enabled && setting.purpose() == "text")
        .ok_or_else(|| {
            ApiError::bad_request(
                "No enabled AI provider is configured for this workspace. An administrator can configure one in Settings.",
            )
        })?;
    ai_access::authorize_ai_use(&state, &subject, &workspace_id, &role).await?;
    let question = request.question.clone();
    let started = std::time::Instant::now();
    let answer = state
        .core
        .ask_workspace(AskRequest {
            workspace_id: workspace_id.clone(),
            question: request.question,
            provider_id: Some(provider.id),
            limit: request.limit,
        })
        .await
        .map_err(map_core_error)?;
    record_timing("ask", &workspace_id, started, answer.retrieved_context.len());
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "ai_question_answered",
        "workspace",
        Some(&workspace_id),
        format!("Asked: {}", question.trim().chars().take(240).collect::<String>()),
    )
    .await;
    Ok(Json(answer))
}

pub(crate) async fn list_workspace_ai_providers(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<SharedAiProviderSettings>>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_provider_settings(&workspace_id)
        .await
        .map(|providers| providers.into_iter().map(shared_provider_settings).collect())
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn save_workspace_ai_provider(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SaveAiProviderRequest>,
) -> Result<Json<SharedAiProviderSettings>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let provider_id = request.id.unwrap_or_default();
    let existing = if provider_id.trim().is_empty() {
        None
    } else {
        let settings = state
            .storage
            .get_provider_settings(&provider_id)
            .await
            .map_err(map_storage_error)?;
        if settings.workspace_id.as_deref() != Some(workspace_id.as_str()) {
            return Err(ApiError::forbidden());
        }
        Some(settings)
    };
    let supplied_key = request.api_key.filter(|value| !value.trim().is_empty());
    // A stored key is only ever sent to the service it was entered for. When
    // the provider type or address changes, the key must be entered again, so
    // an administrator cannot point a colleague's key at another server.
    let endpoint_changed = existing.as_ref().is_some_and(|settings| {
        provider_endpoint(&settings.provider_type, settings.base_url.as_deref())
            != provider_endpoint(&request.provider_type, request.base_url.as_deref())
    });
    let stored_key = existing.as_ref().and_then(|settings| settings.api_key.clone());
    let api_key = match supplied_key.clone() {
        Some(key) => Some(key),
        None if endpoint_changed => {
            if stored_key.is_some() && request.provider_type == "openrouter" {
                return Err(ApiError::bad_request(
                    "Enter the API key again: the provider's type or address changed, and a saved key is only sent to the address it was saved for.",
                ));
            }
            None
        }
        None => stored_key,
    };
    if supplied_key.is_some() {
        tracing::info!(target: "audit", user_id = %subject.user_id, workspace_id = %workspace_id, "AI provider API key set");
    }
    let cloud_content_acknowledged = request.cloud_content_acknowledged
        || existing
            .as_ref()
            .and_then(|settings| settings.metadata.get("cloud_content_acknowledged"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    let purpose = match request.purpose.as_deref() {
        Some("vision") => "vision",
        Some("text") => "text",
        Some("embedding") => "embedding",
        Some(_) => {
            return Err(ApiError::bad_request(
                "Provider purpose must be text, vision or embedding.",
            ))
        }
        None => existing.as_ref().map(|settings| settings.purpose()).unwrap_or("text"),
    };
    let settings = ProviderSettings {
        id: provider_id,
        workspace_id: Some(workspace_id.clone()),
        provider_type: request.provider_type,
        name: request.name,
        base_url: request.base_url,
        model: request.model,
        embedding_model: None,
        enabled: request.enabled,
        metadata: json!({
            "cloud_content_acknowledged": cloud_content_acknowledged,
            "purpose": purpose,
        }),
        api_key,
    };
    // Every validation failure is about the submitted settings.
    repomemo_ai::validate_settings(&settings).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let provider = state
        .core
        .save_provider_settings(settings)
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "ai_provider_updated",
        "ai_provider",
        Some(&provider.id),
        format!("Updated AI provider: {}.", provider.name),
    )
    .await;
    if provider.enabled && provider.purpose() == "vision" {
        // Images stuck behind a missing or broken provider are retried now.
        state.index_queue.resume_pending().await;
    }
    if provider.enabled && provider.purpose() == "embedding" {
        // Existing chunks get vectors from the (possibly new) model.
        state.embedding_queue.request(&workspace_id);
    }
    Ok(Json(shared_provider_settings(provider)))
}

pub(crate) async fn test_workspace_ai_provider(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, provider_id)): Path<(String, String)>,
) -> Result<Json<ProviderTestResult>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let provider = state
        .storage
        .get_provider_settings(&provider_id)
        .await
        .map_err(map_storage_error)?;
    if provider.workspace_id.as_deref() != Some(workspace_id.as_str()) {
        return Err(ApiError::forbidden());
    }
    let result = state
        .core
        .test_provider(provider_id.clone())
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "ai_provider_tested",
        "ai_provider",
        Some(&provider_id),
        format!("Tested AI provider: {}.", provider.name),
    )
    .await;
    Ok(Json(result))
}

/// The service a provider sends requests (and its key) to.
pub(crate) fn provider_endpoint(provider_type: &str, base_url: Option<&str>) -> (String, String) {
    (
        provider_type.trim().to_owned(),
        base_url
            .unwrap_or_default()
            .trim()
            .trim_end_matches('/')
            .to_ascii_lowercase(),
    )
}

pub(crate) fn shared_provider_settings(provider: ProviderSettings) -> SharedAiProviderSettings {
    let purpose = provider.purpose().to_owned();
    SharedAiProviderSettings {
        id: provider.id,
        provider_type: provider.provider_type,
        name: provider.name,
        base_url: provider.base_url,
        purpose,
        model: provider.model,
        enabled: provider.enabled,
    }
}
