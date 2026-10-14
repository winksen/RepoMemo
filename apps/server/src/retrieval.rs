use crate::*;

pub(crate) async fn search_workspace(
    ReadingSubject(subject): ReadingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SearchWorkspaceRequest>,
) -> Result<Json<Vec<SearchResult>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    for status in &request.lifecycle_statuses {
        if !matches!(
            status.as_str(),
            "active" | "needs_review" | "verified" | "outdated" | "superseded"
        ) {
            return Err(ApiError::bad_request(format!("{status} is not an evidence state.")));
        }
    }
    for day in [&request.modified_after, &request.modified_before]
        .into_iter()
        .flatten()
        .filter(|day| !day.is_empty())
    {
        if chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").is_err() {
            return Err(ApiError::bad_request("Dates are written YYYY-MM-DD."));
        }
    }
    let started = std::time::Instant::now();
    let result = state
        .core
        .search_workspace(SearchRequest {
            workspace_id: workspace_id.clone(),
            query: request.query,
            artifact_types: request.artifact_types,
            languages: request.languages,
            source_ids: request.source_ids,
            limit: request.limit,
            lifecycle_statuses: request.lifecycle_statuses,
            folder_id: request.folder_id,
            modified_after: request.modified_after,
            modified_before: request.modified_before,
        })
        .await
        .map_err(map_core_error)?;
    record_timing("search", &workspace_id, started, result.len());
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub(crate) struct SymbolSearchQuery {
    /// Part of a function, class or method name.
    #[serde(default)]
    q: String,
}

/// Where code symbols are defined in the workspace's indexed code: names
/// that contain `q`, exact matches first.
pub(crate) async fn search_symbols(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<SymbolSearchQuery>,
) -> Result<Json<Vec<repomemo_domain::SymbolSearchResult>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let started = std::time::Instant::now();
    let found = state
        .core
        .search_symbols(workspace_id.clone(), query.q)
        .await
        .map_err(map_core_error)?;
    record_timing("symbols", &workspace_id, started, found.len());
    Ok(Json(found))
}

pub(crate) async fn get_retrieval_facets(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<RetrievalFacetsResponse>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let artifacts = state.core.list_artifacts(workspace_id).await.map_err(map_core_error)?;
    let mut artifact_types: Vec<ArtifactType> = Vec::new();
    let mut languages: Vec<String> = Vec::new();
    let mut sources: Vec<RetrievalSourceFacet> = Vec::new();

    for artifact in artifacts.iter().filter(|artifact| artifact.indexed_at.is_some()) {
        if !artifact_types.contains(&artifact.artifact_type) {
            artifact_types.push(artifact.artifact_type.clone());
        }
        if let Some(language) = artifact
            .language
            .as_ref()
            .filter(|language| !language.trim().is_empty())
        {
            if !languages
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(language))
            {
                languages.push(language.clone());
            }
        }
        if !sources
            .iter()
            .any(|source: &RetrievalSourceFacet| source.id == artifact.source_id)
        {
            sources.push(RetrievalSourceFacet {
                id: artifact.source_id.clone(),
                name: artifact.source_name.clone(),
            });
        }
    }

    languages.sort_by_key(|language| language.to_lowercase());
    sources.sort_by_key(|source| source.name.to_lowercase());
    Ok(Json(RetrievalFacetsResponse {
        artifact_types,
        languages,
        sources,
    }))
}

pub(crate) async fn list_memory_cards(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<MemoryCardSummary>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .core
        .list_memory_cards(workspace_id)
        .await
        .map(Json)
        .map_err(map_core_error)
}

pub(crate) async fn search_memory_cards(
    ReadingSubject(subject): ReadingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SearchMemoryCardsRequest>,
) -> Result<Json<Vec<MemoryCardSummary>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .core
        .search_memory_cards(workspace_id, request.query)
        .await
        .map(Json)
        .map_err(map_core_error)
}

pub(crate) async fn create_memory_card(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<CreateMemoryCardBody>,
) -> Result<(StatusCode, Json<MemoryCard>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let card = state
        .core
        .create_memory_card(CreateMemoryCardRequest {
            workspace_id: workspace_id.clone(),
            title: request.title,
            body_markdown: request.body_markdown,
            source: request.source,
            confidence: request.confidence,
            citations: request.citations,
        })
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "memory_created",
        "memory_card",
        Some(&card.id),
        format!("Saved team memory: {}.", card.title),
    )
    .await;
    Ok((StatusCode::CREATED, Json(card)))
}

pub(crate) async fn get_memory_card(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(card_id): Path<String>,
) -> Result<Json<MemoryCardDetail>, ApiError> {
    let card = state.core.get_memory_card(card_id).await.map_err(map_core_error)?;
    require_workspace_read(&state, &subject, &card.card.workspace_id).await?;
    Ok(Json(card))
}

pub(crate) async fn update_memory_card(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(card_id): Path<String>,
    Json(request): Json<UpdateMemoryCardBody>,
) -> Result<Json<MemoryCard>, ApiError> {
    let current = state
        .core
        .get_memory_card(card_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_write(&state, &subject, &current.card.workspace_id).await?;
    let workspace_id = current.card.workspace_id;
    let card = state
        .core
        .update_memory_card(UpdateMemoryCardRequest {
            card_id,
            title: request.title,
            body_markdown: request.body_markdown,
            source: request.source,
            confidence: request.confidence,
            citations: request.citations,
        })
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "memory_updated",
        "memory_card",
        Some(&card.id),
        format!("Updated team memory: {}.", card.title),
    )
    .await;
    Ok(Json(card))
}

pub(crate) async fn delete_memory_card(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(card_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let current = state
        .core
        .get_memory_card(card_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_write(&state, &subject, &current.card.workspace_id).await?;
    let workspace_id = current.card.workspace_id;
    let title = current.card.title;
    state
        .core
        .delete_memory_card(card_id.clone())
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "memory_deleted",
        "memory_card",
        Some(&card_id),
        format!("Deleted team memory: {title}."),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn export_memory_card(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(card_id): Path<String>,
) -> Result<Response, ApiError> {
    let card = state
        .core
        .get_memory_card(card_id.clone())
        .await
        .map_err(map_core_error)?;
    require_workspace_read(&state, &subject, &card.card.workspace_id).await?;
    let markdown = state.core.export_memory_card(card_id).await.map_err(map_core_error)?;
    Ok(([(header::CONTENT_TYPE, "text/markdown; charset=utf-8")], markdown).into_response())
}
