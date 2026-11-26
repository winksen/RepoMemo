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
    ManagingSubject(subject): ManagingSubject,
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
            if stored_key.is_some() && matches!(request.provider_type.as_str(), "openrouter" | "openai_compatible") {
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
    // Only an endpoint that leaves the network needs (and keeps) the acknowledgement, so the
    // count of cloud providers in the system report is the count of providers content leaves for.
    let cloud_content_acknowledged = cloud_content_acknowledged
        && (request.provider_type != "openai_compatible"
            || repomemo_ai::is_cloud_endpoint(&request.provider_type, request.base_url.as_deref()));
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
        metadata: provider_metadata(
            cloud_content_acknowledged,
            purpose,
            existing.as_ref().map(|settings| &settings.metadata),
            request.tool_mode.as_deref(),
            request.context_tokens,
            request.cairn_mode.as_deref(),
        )
        .map_err(ApiError::bad_request)?,
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

/// How CAIRN answers in the workspace of a provider (V0.9.47). What an administrator chose wins (Agent only for a model that takes
/// tools). With no choice, a native tool-calling model that has a passing live evaluation report on record starts in Agent mode,
/// and every other model, and every model until a report is recorded, in Jobs mode.
pub(crate) fn cairn_mode_of(provider: &ProviderSettings) -> &'static str {
    cairn_mode_with(provider, &repomemo_ai::catalog_model_passed)
}

/// [`cairn_mode_of`] with the check of which catalog models have a passing report given, so a test need not share the global one.
fn cairn_mode_with(provider: &ProviderSettings, catalog_passed: &dyn Fn(&str) -> bool) -> &'static str {
    let tool_mode = repomemo_ai::ModelTraits::from_metadata(&provider.metadata).tool_mode;
    match provider.metadata.get("cairn_mode").and_then(serde_json::Value::as_str) {
        Some("agent") if tool_mode != repomemo_ai::ToolMode::None => "agent",
        Some("agent") | Some("jobs") => "jobs",
        _ if tool_mode == repomemo_ai::ToolMode::Native
            && (repomemo_ai::agent_default_for(provider.model.as_deref())
                || provider
                    .metadata
                    .get("cairn_model_id")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(catalog_passed)) =>
        {
            "agent"
        }
        _ => "jobs",
    }
}

pub(crate) fn shared_provider_settings(provider: ProviderSettings) -> SharedAiProviderSettings {
    let purpose = provider.purpose().to_owned();
    let known = repomemo_ai::ModelTraits::is_known(&provider.metadata);
    let traits = repomemo_ai::ModelTraits::from_metadata(&provider.metadata);
    let cairn_mode = cairn_mode_of(&provider).to_owned();
    SharedAiProviderSettings {
        id: provider.id,
        provider_type: provider.provider_type,
        name: provider.name,
        base_url: provider.base_url,
        purpose,
        model: provider.model,
        enabled: provider.enabled,
        tool_mode: known.then(|| traits.tool_mode.as_str().to_owned()),
        context_tokens: known.then_some(traits.context_tokens),
        cairn_mode: Some(cairn_mode),
    }
}

/// The metadata a saved provider keeps: its purpose and acknowledgement, and the traits of its
/// model. The traits a test recorded survive a save; ones an administrator sets that differ from
/// them are kept as theirs, so a later test does not overwrite them.
fn provider_metadata(
    cloud_content_acknowledged: bool,
    purpose: &str,
    existing: Option<&serde_json::Value>,
    tool_mode: Option<&str>,
    context_tokens: Option<u32>,
    cairn_mode: Option<&str>,
) -> Result<serde_json::Value, String> {
    let mut metadata = json!({
        "cloud_content_acknowledged": cloud_content_acknowledged,
        "purpose": purpose,
    });
    let kept = existing.cloned().unwrap_or_else(|| json!({}));
    // The catalog model the provider is linked to (V0.9.78) survives a save.
    if let Some(model_id) = kept.get("cairn_model_id").and_then(serde_json::Value::as_str) {
        metadata["cairn_model_id"] = json!(model_id);
    }
    let known = repomemo_ai::ModelTraits::is_known(&kept);
    let before = repomemo_ai::ModelTraits::from_metadata(&kept);
    let mut traits = before;
    if let Some(mode) = tool_mode {
        traits.tool_mode = repomemo_ai::ToolMode::parse(mode)
            .ok_or_else(|| "Tool calling must be native, json or none.".to_owned())?;
    }
    if let Some(tokens) = context_tokens {
        if !(512..=4_000_000).contains(&tokens) {
            return Err("The context size must be between 512 and 4,000,000 tokens.".to_owned());
        }
        traits.context_tokens = tokens;
    }
    let manual = kept.get("traits_manual").and_then(serde_json::Value::as_bool) == Some(true)
        || ((tool_mode.is_some() || context_tokens.is_some()) && (!known || traits != before));
    if known || manual {
        traits.write_to(&mut metadata);
    }
    if manual {
        metadata["traits_manual"] = json!(true);
    }
    // The CAIRN mode is kept across saves; Agent mode needs a model that takes tools.
    let mode = match cairn_mode {
        Some("jobs") => "jobs",
        Some("agent") => "agent",
        Some(_) => return Err("CAIRN mode must be jobs or agent.".to_owned()),
        None if kept.get("cairn_mode").and_then(serde_json::Value::as_str) == Some("agent") => "agent",
        None => "jobs",
    };
    if mode == "jobs"
        && (cairn_mode == Some("jobs") || kept.get("cairn_mode").and_then(serde_json::Value::as_str) == Some("jobs"))
    {
        metadata["cairn_mode"] = json!("jobs");
    }
    if mode == "agent" {
        if traits.tool_mode == repomemo_ai::ToolMode::None || !(known || manual) {
            return Err("Agent mode needs a model that takes tools: test the connection first, or set Tool calling to native or JSON.".to_owned());
        }
        metadata["cairn_mode"] = json!("agent");
    }
    Ok(metadata)
}

#[cfg(test)]
mod provider_metadata_tests {
    use super::provider_metadata;
    use serde_json::json;

    #[test]
    fn a_new_provider_has_no_traits_until_tested_or_set() {
        let metadata = provider_metadata(false, "text", None, None, None, None).unwrap();
        assert_eq!(
            metadata,
            json!({ "cloud_content_acknowledged": false, "purpose": "text" })
        );
    }

    #[test]
    fn tested_traits_survive_a_save_without_becoming_manual() {
        let existing = json!({ "purpose": "text", "tool_mode": "native", "context_tokens": 32000 });
        let metadata = provider_metadata(true, "text", Some(&existing), Some("native"), Some(32000), None).unwrap();
        assert_eq!(metadata["tool_mode"], "native");
        assert_eq!(metadata["context_tokens"], 32000);
        assert!(metadata.get("traits_manual").is_none());
    }

    #[test]
    fn a_correction_is_kept_as_the_administrators() {
        let existing = json!({ "purpose": "text", "tool_mode": "none", "context_tokens": 8192 });
        let metadata = provider_metadata(true, "text", Some(&existing), Some("json"), Some(16000), None).unwrap();
        assert_eq!(metadata["tool_mode"], "json");
        assert_eq!(metadata["context_tokens"], 16000);
        assert_eq!(metadata["traits_manual"], true);
        let again = provider_metadata(true, "text", Some(&metadata), None, None, None).unwrap();
        assert_eq!(again["traits_manual"], true);
        assert_eq!(again["tool_mode"], "json");
    }

    #[test]
    fn nonsense_is_refused() {
        assert!(provider_metadata(false, "text", None, Some("magic"), None, None).is_err());
        assert!(provider_metadata(false, "text", None, None, Some(10), None).is_err());
    }
}

#[cfg(test)]
mod catalog_default_tests {
    use super::cairn_mode_with;
    use repomemo_domain::ProviderSettings;
    use serde_json::json;

    fn provider(metadata: serde_json::Value) -> ProviderSettings {
        ProviderSettings {
            id: "p".to_owned(),
            workspace_id: Some("w".to_owned()),
            provider_type: "ollama".to_owned(),
            name: "Local".to_owned(),
            base_url: None,
            model: Some("a-model-nobody-lists".to_owned()),
            embedding_model: None,
            enabled: true,
            metadata,
            api_key: None,
        }
    }

    #[test]
    fn a_native_model_of_the_catalog_with_a_passing_report_starts_in_agent_mode() {
        let linked = |tool_mode: &str, id: &str| {
            provider(json!({"purpose": "text", "tool_mode": tool_mode, "context_tokens": 32000, "cairn_model_id": id}))
        };
        let nothing = |_: &str| false;
        let passed = |id: &str| id == "unit-test-passed";
        // Nothing is on record: every workspace starts in Jobs mode.
        assert_eq!(cairn_mode_with(&linked("native", "unit-test-passed"), &nothing), "jobs");
        assert_eq!(cairn_mode_with(&linked("native", "unit-test-passed"), &passed), "agent");
        // Only a model that takes tools natively, and only the one that passed.
        assert_eq!(cairn_mode_with(&linked("json", "unit-test-passed"), &passed), "jobs");
        assert_eq!(cairn_mode_with(&linked("native", "unit-test-failed"), &passed), "jobs");
        // What an administrator chose wins.
        let chosen = provider(
            json!({"purpose": "text", "tool_mode": "native", "cairn_model_id": "unit-test-passed", "cairn_mode": "jobs"}),
        );
        assert_eq!(cairn_mode_with(&chosen, &passed), "jobs");
    }
}
