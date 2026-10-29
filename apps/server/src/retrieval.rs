use crate::*;

/// Workspaces one search across workspaces will read; the rest are counted
/// in `not_searched`.
const CROSS_SEARCH_MAX_WORKSPACES: usize = 30;
/// Workspaces searched at the same time.
const CROSS_SEARCH_PARALLEL: usize = 4;
const CROSS_SEARCH_DEFAULT_LIMIT: i64 = 20;
const CROSS_SEARCH_MAX_LIMIT: i64 = 50;
/// Memory cards in one search across workspaces.
const CROSS_SEARCH_MEMORY_LIMIT: usize = 10;
/// The HTTP search budget includes embedding, keyword, semantic and ranking work.
const SEARCH_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);
const CROSS_SEARCH_BUDGET: std::time::Duration = std::time::Duration::from_secs(10);
const SEARCH_MAX_QUERY_CHARS: usize = 512;

/// A day filter written `YYYY-MM-DD`, or empty.
fn check_day(day: &Option<String>) -> Result<(), ApiError> {
    match day.as_deref().filter(|day| !day.is_empty()) {
        Some(day) if chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").is_err() => {
            Err(ApiError::bad_request("Dates are written YYYY-MM-DD."))
        }
        _ => Ok(()),
    }
}

/// Whether a card last written at `updated_at` (RFC 3339) is inside the days asked for.
pub(crate) fn card_in_days(updated_at: &str, after: Option<&str>, before: Option<&str>) -> bool {
    let day = updated_at.get(..10).unwrap_or(updated_at);
    after.filter(|after| !after.is_empty()).is_none_or(|after| day >= after)
        && before
            .filter(|before| !before.is_empty())
            .is_none_or(|before| day <= before)
}

fn check_lifecycle_statuses(statuses: &[String]) -> Result<(), ApiError> {
    for status in statuses {
        if !matches!(
            status.as_str(),
            "active" | "needs_review" | "verified" | "outdated" | "superseded"
        ) {
            return Err(ApiError::bad_request(format!("{status} is not an evidence state.")));
        }
    }
    Ok(())
}

/// Searches every workspace the caller can read and merges what they find.
///
/// Each workspace is searched on its own, with its own role check and its own
/// AI policy: where the policy keeps AI from the caller's role the query is
/// matched by keywords only and never sent to that workspace's embedding
/// provider. A workspace's scores are relative to its own best match, so every
/// workspace's best result ties at the top and the rest follow by score, then
/// by rank in their workspace.
pub(crate) async fn search_all_workspaces(
    ReadingSubject(subject): ReadingSubject,
    State(state): State<AppState>,
    Json(request): Json<SearchAllRequest>,
) -> Result<(HeaderMap, Json<CrossSearchResponse>), ApiError> {
    consume_heavy_quota(&state, &subject)?;
    if request.query.chars().count() > SEARCH_MAX_QUERY_CHARS {
        return Err(ApiError::bad_request(
            "Search queries may contain at most 512 characters.",
        ));
    }
    check_lifecycle_statuses(&request.lifecycle_statuses)?;
    check_day(&request.modified_after)?;
    check_day(&request.modified_before)?;
    let mut response = CrossSearchResponse {
        results: Vec::new(),
        memory_cards: Vec::new(),
        workspaces: Vec::new(),
        not_searched: 0,
    };
    // Cards are not files: a type or a language leaves them out.
    let with_cards =
        request.memory_cards.unwrap_or(true) && request.artifact_types.is_empty() && request.languages.is_empty();
    let query = request.query.trim().to_owned();
    if query.is_empty() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-repomemo-query-state"),
            HeaderValue::from_static("complete"),
        );
        return Ok((headers, Json(response)));
    }
    let limit = request
        .limit
        .unwrap_or(CROSS_SEARCH_DEFAULT_LIMIT)
        .clamp(1, CROSS_SEARCH_MAX_LIMIT);
    let listing = if subject.is_system_admin {
        state
            .storage
            .list_shared_workspaces_for_system_admin(&subject.user_id)
            .await
    } else {
        state.storage.list_shared_workspaces_for_user(&subject.user_id).await
    }
    .map_err(ApiError::internal)?;

    let started = std::time::Instant::now();
    // The caller's rights and each workspace's AI policy are settled for every workspace first; the
    // searches then run a few at a time, with the query embedded once for each provider and model.
    let mut chosen = Vec::new();
    let mut inputs = Vec::new();
    for entry in listing {
        let workspace = entry.workspace;
        if !request.workspace_ids.is_empty() && !request.workspace_ids.contains(&workspace.id) {
            continue;
        }
        if !subject.token_reaches(&workspace.id) {
            continue;
        }
        let Some(role) =
            effective_workspace_role(&state, &subject.user_id, subject.is_system_admin, &workspace.id).await?
        else {
            continue;
        };
        if chosen.len() >= CROSS_SEARCH_MAX_WORKSPACES {
            response.not_searched += 1;
            continue;
        }
        let min_role = ai_access::workspace_ai_min_role(&state, &workspace.id).await?;
        let by_meaning_allowed = min_role.allows(&role);
        inputs.push((
            SearchRequest {
                workspace_id: workspace.id.clone(),
                query: query.clone(),
                limit: Some(limit),
                lifecycle_statuses: request.lifecycle_statuses.clone(),
                artifact_types: request.artifact_types.clone(),
                languages: request.languages.clone(),
                modified_after: request.modified_after.clone().filter(|day| !day.is_empty()),
                modified_before: request.modified_before.clone().filter(|day| !day.is_empty()),
                ..Default::default()
            },
            by_meaning_allowed,
        ));
        chosen.push((workspace, by_meaning_allowed));
    }
    let outcomes = match tokio::time::timeout(
        CROSS_SEARCH_BUDGET,
        state.core.search_workspaces_detailed(inputs, CROSS_SEARCH_PARALLEL),
    )
    .await
    {
        Ok(outcomes) => outcomes,
        Err(_) => {
            response.not_searched += chosen.len() as i64;
            let mut headers = HeaderMap::new();
            headers.insert(
                HeaderName::from_static("x-repomemo-query-state"),
                HeaderValue::from_static("timed_out"),
            );
            return Ok((headers, Json(response)));
        }
    };
    // (score, rank in its workspace, workspace name) order the merged list.
    let mut merged: Vec<(f64, usize, String, CrossSearchResult)> = Vec::new();
    for ((workspace, by_meaning_allowed), searched) in chosen.into_iter().zip(outcomes) {
        let (results, by_meaning, note) = match searched {
            Ok((results, by_meaning)) => {
                let note = (!by_meaning_allowed).then(|| {
                    "Searched by keywords only: in this workspace AI features are kept to other roles, so the query was not sent to its AI provider.".to_owned()
                });
                (results, by_meaning, note)
            }
            Err(error) => {
                tracing::warn!(workspace_id = %workspace.id, "search across workspaces skipped one: {error:#}");
                (
                    Vec::new(),
                    false,
                    Some("This workspace could not be searched just now.".to_owned()),
                )
            }
        };
        // Memory cards need the same right as the files of the workspace: the caller can read it, which was
        // settled above. A card is found by keywords only, so the workspace's AI policy has nothing to keep back.
        if with_cards && response.memory_cards.len() < CROSS_SEARCH_MEMORY_LIMIT {
            match state
                .core
                .search_memory_cards(workspace.id.clone(), query.clone())
                .await
            {
                Ok(cards) => {
                    for card in cards {
                        if response.memory_cards.len() >= CROSS_SEARCH_MEMORY_LIMIT {
                            break;
                        }
                        if card_in_days(
                            &card.updated_at,
                            request.modified_after.as_deref(),
                            request.modified_before.as_deref(),
                        ) {
                            response.memory_cards.push(CrossMemoryCard {
                                workspace_id: workspace.id.clone(),
                                workspace_name: workspace.name.clone(),
                                card,
                            });
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(workspace_id = %workspace.id, "search across workspaces skipped memory cards: {error:#}")
                }
            }
        }
        response.workspaces.push(CrossSearchWorkspace {
            workspace_id: workspace.id.clone(),
            name: workspace.name.clone(),
            result_count: results.len() as i64,
            by_meaning,
            note,
        });
        for (rank, result) in results.into_iter().enumerate() {
            merged.push((
                result.score,
                rank,
                workspace.name.to_lowercase(),
                CrossSearchResult {
                    workspace_id: workspace.id.clone(),
                    workspace_name: workspace.name.clone(),
                    result,
                },
            ));
        }
    }
    merged.sort_by(|left, right| {
        right
            .0
            .total_cmp(&left.0)
            .then_with(|| left.1.cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    response.results = merged
        .into_iter()
        .take(limit as usize)
        .map(|(_, _, _, result)| result)
        .collect();
    record_timing("search_all", "all", started, response.results.len());
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-repomemo-query-state"),
        HeaderValue::from_static("complete"),
    );
    Ok((headers, Json(response)))
}

pub(crate) async fn search_workspace(
    ReadingSubject(subject): ReadingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SearchWorkspaceRequest>,
) -> Result<(HeaderMap, Json<Vec<SearchResult>>), ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    consume_heavy_quota(&state, &subject)?;
    if request.query.chars().count() > SEARCH_MAX_QUERY_CHARS {
        return Err(ApiError::bad_request(
            "Search queries may contain at most 512 characters.",
        ));
    }
    check_lifecycle_statuses(&request.lifecycle_statuses)?;
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
    let searched = tokio::time::timeout(
        SEARCH_BUDGET,
        state.core.search_workspace_detailed(
            SearchRequest {
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
            },
            true,
        ),
    )
    .await;
    let (result, semantic_available, timed_out) = match searched {
        Ok(Ok((result, semantic_available))) => (result, semantic_available, false),
        Ok(Err(error)) => return Err(map_core_error(error)),
        Err(_) => (Vec::new(), false, true),
    };
    record_timing("search", &workspace_id, started, result.len());
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-repomemo-semantic-search"),
        HeaderValue::from_static(if semantic_available { "available" } else { "unavailable" }),
    );
    headers.insert(
        HeaderName::from_static("x-repomemo-query-state"),
        HeaderValue::from_static(if timed_out { "timed_out" } else { "complete" }),
    );
    Ok((headers, Json(result)))
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
    consume_heavy_quota(&state, &subject)?;
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
    crate::notify_mentions(
        &state,
        &subject.user_id,
        &workspace_id,
        crate::MentionedIn::MemoryCard {
            id: &card.id,
            title: &card.title,
        },
        &card.body_markdown,
        "",
    )
    .await;
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
    let earlier_body = current.card.body_markdown;
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
    crate::notify_mentions(
        &state,
        &subject.user_id,
        &workspace_id,
        crate::MentionedIn::MemoryCard {
            id: &card.id,
            title: &card.title,
        },
        &card.body_markdown,
        &earlier_body,
    )
    .await;
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
