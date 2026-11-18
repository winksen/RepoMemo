//! AI administration (V0.8.13): what a workspace's AI work has used, what it may use, and what happens when it cannot.
//!
//! The calls the server makes on its own (embedding batches, image descriptions) are counted in the database, so a limit stops
//! them the same way before and after a restart. A workspace can set its own daily limit; without one the server's default
//! applies, and 0 means no limit. Keyword search never calls an AI provider, so it keeps working when AI is off, not configured,
//! over its limit or failing.

use crate::*;

#[derive(Debug, Deserialize)]
pub(crate) struct UsageQuery {
    days: Option<i64>,
}

#[derive(Debug, Serialize)]
struct KindUsage {
    kind: String,
    calls: i64,
    refused: i64,
}

#[derive(Debug, Serialize)]
struct DayUsage {
    day: String,
    calls: i64,
    refused: i64,
    kinds: Vec<KindUsage>,
}

#[derive(Debug, Serialize)]
struct ProviderState {
    name: String,
    /// `text`, `vision` or `embedding`.
    purpose: String,
    enabled: bool,
}

#[derive(Debug, Serialize)]
struct StoredCounts {
    files: i64,
    passages: i64,
    vectors: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct AiUsage {
    /// Calls a day the server makes on its own for this workspace; 0 is no limit.
    limit: u32,
    /// `workspace`, `server` or `none`.
    limit_source: &'static str,
    spent_today: i64,
    refused_today: i64,
    /// What is left today, or none when there is no limit.
    remaining: Option<i64>,
    days: Vec<DayUsage>,
    stored: StoredCounts,
    providers: Vec<ProviderState>,
    /// Whether any provider is on, for each purpose.
    text_available: bool,
    embedding_available: bool,
    vision_available: bool,
    /// What still works and what stops when AI is off, over its limit or failing.
    when_unavailable: &'static str,
}

const WHEN_UNAVAILABLE: &str = "Keyword search never calls an AI provider and keeps working when AI is off, not configured, over its limit or failing. Embedding and image description stop at the limit and resume the next day; Ask and summaries say why they cannot answer. Search finds a file whether or not it was embedded.";

pub(crate) async fn get_ai_usage(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<AiUsage>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let storage = &state.storage;
    let own = storage
        .workspace_ai_limit(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let default = state.core.background_ai_budget().daily_limit();
    let (limit, limit_source) = match own {
        Some(limit) => (limit, "workspace"),
        None if default > 0 => (default, "server"),
        None => (0, "none"),
    };
    let rows = storage
        .ai_usage_days(&workspace_id, query.days.unwrap_or(14))
        .await
        .map_err(map_storage_error)?;
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let mut days: Vec<DayUsage> = Vec::new();
    for row in rows {
        if days.last().map(|day| day.day.as_str()) != Some(row.day.as_str()) {
            days.push(DayUsage {
                day: row.day.clone(),
                calls: 0,
                refused: 0,
                kinds: Vec::new(),
            });
        }
        if let Some(day) = days.last_mut() {
            day.calls += row.calls;
            day.refused += row.refused;
            day.kinds.push(KindUsage {
                kind: row.kind,
                calls: row.calls,
                refused: row.refused,
            });
        }
    }
    let (spent_today, refused_today) = days
        .iter()
        .find(|day| day.day == today)
        .map(|day| (day.calls, day.refused))
        .unwrap_or((0, 0));
    let (files, passages, vectors) = storage
        .ai_stored_counts(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let providers: Vec<ProviderState> = storage
        .list_provider_settings(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .map(|provider| ProviderState {
            purpose: provider.purpose().to_owned(),
            name: provider.name,
            enabled: provider.enabled,
        })
        .collect();
    let on = |purpose: &str| {
        providers
            .iter()
            .any(|provider| provider.enabled && provider.purpose == purpose)
    };
    Ok(Json(AiUsage {
        limit,
        limit_source,
        spent_today,
        refused_today,
        remaining: (limit > 0).then(|| (i64::from(limit) - spent_today).max(0)),
        days,
        stored: StoredCounts {
            files,
            passages,
            vectors,
        },
        text_available: on("text"),
        embedding_available: on("embedding"),
        vision_available: on("vision"),
        providers,
        when_unavailable: WHEN_UNAVAILABLE,
    }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct LimitRequest {
    /// Calls a day; 0 is no limit; left out or null, the workspace follows the server's default again.
    #[serde(default)]
    daily_calls: Option<u32>,
}

pub(crate) async fn set_ai_limit(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<LimitRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    if request.daily_calls.is_some_and(|calls| calls > 1_000_000) {
        return Err(ApiError::bad_request("A daily limit is at most 1,000,000 calls."));
    }
    state
        .storage
        .set_workspace_ai_limit(&workspace_id, request.daily_calls, Some(&subject.user_id))
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "ai_limit_changed",
        "workspace",
        Some(&workspace_id),
        match request.daily_calls {
            Some(0) => "Removed the workspace's daily AI call limit.".to_owned(),
            Some(calls) => format!("Set the workspace's daily AI call limit to {calls}."),
            None => "Returned the workspace to the server's daily AI call limit.".to_owned(),
        },
    )
    .await;
    Ok(Json(serde_json::json!({ "daily_calls": request.daily_calls })))
}
