//! HTTP surface of the workspace assistant. Routing and the capabilities
//! themselves live in `RepoMemoCore`; this layer adds membership checks, picks
//! the workspace's text provider and records AI use in the activity feed.

use axum::{
    extract::{Path, State},
    Json,
};
use repomemo_api::agent_capabilities;
use repomemo_domain::{
    AgentCapability, AgentCapabilityInfo, AgentReply, AgentRequest, ProviderSettings,
};
use serde::{Deserialize, Serialize};

use crate::{
    map_core_error, map_storage_error, record_workspace_activity, require_workspace_read,
    ApiError, AppState, AuthenticatedSubject,
};

#[derive(Debug, Serialize)]
pub(crate) struct AgentCapabilitiesResponse {
    capabilities: Vec<AgentCapabilityInfo>,
    provider_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AgentMessageRequest {
    #[serde(default)]
    message: String,
    capability: Option<AgentCapability>,
    artifact_id: Option<String>,
}

pub(crate) async fn list_agent_capabilities(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<AgentCapabilitiesResponse>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let provider = text_provider(&state, &workspace_id).await?;
    Ok(Json(AgentCapabilitiesResponse {
        capabilities: agent_capabilities(provider.is_some()),
        provider_name: provider.map(|provider| provider.name),
    }))
}

pub(crate) async fn send_agent_message(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<AgentMessageRequest>,
) -> Result<Json<AgentReply>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let provider = text_provider(&state, &workspace_id).await?;
    let reply = state
        .core
        .run_agent(AgentRequest {
            workspace_id: workspace_id.clone(),
            message: request.message,
            capability: request.capability,
            artifact_id: request.artifact_id,
            provider_id: provider.map(|provider| provider.id),
        })
        .await
        .map_err(map_core_error)?;
    if reply.generated {
        record_workspace_activity(
            &state,
            &workspace_id,
            &subject.user_id,
            "assistant_answered",
            "workspace",
            Some(&workspace_id),
            "Used the workspace assistant to generate a citation-backed reply.".to_owned(),
        )
        .await;
    }
    Ok(Json(reply))
}

async fn text_provider(
    state: &AppState,
    workspace_id: &str,
) -> Result<Option<ProviderSettings>, ApiError> {
    Ok(state
        .storage
        .list_provider_settings(workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .find(|setting| setting.enabled && setting.purpose() == "text"))
}
