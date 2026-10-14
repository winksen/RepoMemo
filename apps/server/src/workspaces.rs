use crate::*;

pub(crate) async fn create_workspace(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<CreateWorkspaceRequest>,
) -> Result<(StatusCode, Json<SharedWorkspace>), ApiError> {
    quotas::check_new_workspace(&state, &subject).await?;
    let workspace = state
        .storage
        .create_shared_workspace(&subject.user_id, &request.organization_id, &request.name)
        .await
        .map_err(map_storage_error)?;
    Ok((StatusCode::CREATED, Json(workspace)))
}

/// The caller's workspaces; every workspace for a system administrator.
pub(crate) async fn list_workspaces(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<SharedWorkspace>>, ApiError> {
    let workspaces = if subject.is_system_admin {
        state
            .storage
            .list_shared_workspaces_for_system_admin(&subject.user_id)
            .await
    } else {
        state.storage.list_shared_workspaces_for_user(&subject.user_id).await
    };
    workspaces.map(Json).map_err(ApiError::internal)
}

/// The figures the dashboard shows for every workspace the caller can open,
/// in one request (the page used to ask for each workspace's metrics).
pub(crate) async fn dashboard(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<DashboardResponse>, ApiError> {
    consume_heavy_quota(&state, &subject)?;
    let started = std::time::Instant::now();
    let workspaces = if subject.is_system_admin {
        state
            .storage
            .list_shared_workspaces_for_system_admin(&subject.user_id)
            .await
    } else {
        state.storage.list_shared_workspaces_for_user(&subject.user_id).await
    }
    .map_err(ApiError::internal)?;
    let ids = workspaces
        .iter()
        .map(|entry| entry.workspace.id.clone())
        .collect::<Vec<_>>();
    let counts = state.storage.dashboard_counts(&ids).await.map_err(map_storage_error)?;
    record_timing("dashboard", "all", started, ids.len());
    Ok(Json(DashboardResponse {
        workspaces: counts
            .into_iter()
            .map(|counts| DashboardWorkspace {
                workspace_id: counts.workspace_id,
                artifact_count: counts.artifacts,
                active_task_count: counts.active_tasks,
                memory_card_count: counts.memory_cards,
            })
            .collect(),
    }))
}

pub(crate) async fn update_workspace(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<UpdateWorkspaceRequest>,
) -> Result<Json<Workspace>, ApiError> {
    require_workspace_owner(&state, &subject, &workspace_id).await?;
    let workspace = state
        .core
        .update_workspace_name(workspace_id.clone(), request.name)
        .await
        .map_err(map_core_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "workspace_updated",
        "workspace",
        Some(&workspace_id),
        format!("Renamed the workspace to {}.", workspace.name),
    )
    .await;
    Ok(Json(workspace))
}

pub(crate) async fn delete_workspace(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_workspace_owner(&state, &subject, &workspace_id).await?;
    state
        .core
        .delete_workspace(workspace_id)
        .await
        .map_err(map_core_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn workspace_overview(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceOverview>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .core
        .workspace_overview(workspace_id)
        .await
        .map(Json)
        .map_err(map_core_error)
}

pub(crate) async fn workspace_metrics(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceMetricsResponse>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    consume_heavy_quota(&state, &subject)?;
    let started = std::time::Instant::now();
    let overview = state
        .core
        .workspace_overview(workspace_id.clone())
        .await
        .map_err(map_core_error)?;
    let freshness_threshold = (Utc::now() - ChronoDuration::days(7)).to_rfc3339();
    let stats = state
        .storage
        .workspace_artifact_stats(&workspace_id, &freshness_threshold)
        .await
        .map_err(map_storage_error)?;
    let activity = state
        .storage
        .list_workspace_activity(&workspace_id, 100)
        .await
        .map_err(map_storage_error)?;
    let members = state
        .storage
        .list_workspace_members(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let collaboration = state
        .storage
        .workspace_collaboration_counts(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let mut artifact_types = BTreeMap::<String, i64>::new();
    let mut artifact_bytes_by_type = BTreeMap::<String, i64>::new();
    let mut languages = BTreeMap::<String, i64>::new();
    let mut activity_actions = BTreeMap::<String, i64>::new();
    let mut member_roles = BTreeMap::<String, i64>::new();
    let today = Utc::now().date_naive();
    let mut activity_by_day = (0..14)
        .rev()
        .map(|offset| ((today - ChronoDuration::days(offset)).to_string(), 0))
        .collect::<BTreeMap<_, _>>();
    for (artifact_type, files, bytes) in &stats.by_type {
        let label = artifact_type_label(artifact_type).to_owned();
        *artifact_types.entry(label.clone()).or_default() += files;
        *artifact_bytes_by_type.entry(label).or_default() += bytes;
    }
    for (language, files) in &stats.by_language {
        *languages.entry(language.clone()).or_default() += files;
    }
    let indexed_artifact_count = stats.indexed;
    let total_artifact_bytes = stats.total_bytes;
    let indexed_artifact_bytes = stats.indexed_bytes;
    let artifacts_created_last_7_days = stats.created_since;
    let artifacts_updated_last_7_days = stats.updated_since;
    for event in &activity {
        *activity_actions.entry(event.action.replace('_', " ")).or_default() += 1;
        if let Some(day) = event.created_at.get(..10) {
            if let Some(count) = activity_by_day.get_mut(day) {
                *count += 1;
            }
        }
    }
    for member in &members {
        *member_roles
            .entry(workspace_role_label(&member.role).to_owned())
            .or_default() += 1;
    }

    let embedded_chunk_count = state
        .core
        .embedding_coverage(&workspace_id)
        .await
        .map_err(map_core_error)?
        .map(|(embedded, _)| embedded);

    record_timing(
        "metrics",
        &workspace_id,
        started,
        overview.artifact_count.max(0) as usize,
    );
    Ok(Json(WorkspaceMetricsResponse {
        workspace_id,
        generated_at: Utc::now().to_rfc3339(),
        embedded_chunk_count,
        source_count: overview.source_count,
        member_count: members.len() as i64,
        artifact_count: overview.artifact_count,
        indexed_artifact_count,
        pending_artifact_count: overview.artifact_count - indexed_artifact_count,
        total_artifact_bytes,
        indexed_artifact_bytes,
        pending_artifact_bytes: total_artifact_bytes - indexed_artifact_bytes,
        chunk_count: overview.chunk_count,
        symbol_count: overview.symbol_count,
        memory_card_count: overview.memory_card_count,
        open_task_count: collaboration.open_tasks,
        in_progress_task_count: collaboration.in_progress_tasks,
        blocked_task_count: collaboration.blocked_tasks,
        completed_task_count: collaboration.completed_tasks,
        overdue_task_count: collaboration.overdue_tasks,
        comment_count: collaboration.comments,
        recent_activity_count: activity.len() as i64,
        artifacts_created_last_7_days,
        artifacts_updated_last_7_days,
        activity_actions: metric_breakdown(activity_actions),
        activity_by_day: metric_timeline(activity_by_day),
        member_roles: metric_breakdown(member_roles),
        artifact_types: metric_breakdown(artifact_types),
        artifact_bytes_by_type: metric_breakdown(artifact_bytes_by_type),
        languages: metric_breakdown(languages),
    }))
}

pub(crate) fn artifact_type_label(artifact_type: &ArtifactType) -> &'static str {
    match artifact_type {
        ArtifactType::File => "File",
        ArtifactType::MarkdownDoc => "Markdown",
        ArtifactType::CodeFile => "Code",
        ArtifactType::Image => "Image",
        ArtifactType::Issue => "Issue",
        ArtifactType::Pr => "Pull request",
        ArtifactType::Decision => "Decision",
        ArtifactType::Incident => "Incident",
        ArtifactType::Runbook => "Runbook",
        ArtifactType::ApiSpec => "API specification",
        ArtifactType::Note => "Note",
        ArtifactType::Repository => "Repository",
    }
}

pub(crate) fn metric_breakdown(values: BTreeMap<String, i64>) -> Vec<WorkspaceMetricBreakdown> {
    let mut values = values
        .into_iter()
        .map(|(label, value)| WorkspaceMetricBreakdown { label, value })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| right.value.cmp(&left.value).then_with(|| left.label.cmp(&right.label)));
    values
}

pub(crate) fn metric_timeline(values: BTreeMap<String, i64>) -> Vec<WorkspaceMetricBreakdown> {
    values
        .into_iter()
        .map(|(label, value)| WorkspaceMetricBreakdown { label, value })
        .collect()
}

pub(crate) async fn workspace_capabilities(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceCapabilities>, ApiError> {
    let role = require_workspace_read(&state, &subject, &workspace_id).await?;
    let may_use_ai = ai_access::role_may_use_ai(&state, &workspace_id, &role).await?;
    Ok(Json(capabilities_for_role(role, may_use_ai)))
}

pub(crate) async fn workspace_knowledge_map(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<KnowledgeMap>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    consume_heavy_quota(&state, &subject)?;
    let started = std::time::Instant::now();
    let version = state
        .storage
        .workspace_content_version(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    if let Some(map) = state.derived.knowledge_map(&workspace_id, &version) {
        record_timing("knowledge_map_cached", &workspace_id, started, map.nodes.len());
        return Ok(Json(map));
    }
    let map = state.core.knowledge_map(&workspace_id).await.map_err(map_core_error)?;
    state.derived.keep_knowledge_map(&workspace_id, version, map.clone());
    record_timing("knowledge_map", &workspace_id, started, map.nodes.len());
    Ok(Json(map))
}

pub(crate) async fn list_workspace_activity(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<WorkspaceActivityEvent>>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_workspace_activity(&workspace_id, 100)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn workspace_activity_calendar(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<WorkspaceActivityCalendarResponse>, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let today = Utc::now().date_naive();
    let mut activity_by_day = (0..365)
        .rev()
        .map(|offset| ((today - ChronoDuration::days(offset)).to_string(), 0))
        .collect::<BTreeMap<_, _>>();
    let rows = state
        .storage
        .workspace_activity_by_day(&workspace_id, &(Utc::now() - ChronoDuration::days(364)).to_rfc3339())
        .await
        .map_err(map_storage_error)?;
    for (day, count) in rows {
        if let Some(value) = activity_by_day.get_mut(&day) {
            *value = count;
        }
    }
    let total_activity_count = activity_by_day.values().sum();
    Ok(Json(WorkspaceActivityCalendarResponse {
        total_activity_count,
        activity_by_day: metric_timeline(activity_by_day),
    }))
}

pub(crate) async fn record_workspace_activity(
    state: &AppState,
    workspace_id: &str,
    actor_user_id: &str,
    action: &str,
    subject_type: &str,
    subject_id: Option<&str>,
    summary: String,
) {
    if let Err(error) = state
        .storage
        .record_workspace_activity(
            workspace_id,
            Some(actor_user_id),
            action,
            subject_type,
            subject_id,
            &summary,
        )
        .await
    {
        tracing::error!(error = %error, workspace_id, action, "Failed to record workspace activity");
    }
}
