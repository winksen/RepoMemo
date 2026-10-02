//! HTTP surface of the workspace assistant. Routing and the capabilities
//! themselves live in `RepoMemoCore`; this layer adds membership checks, picks
//! the workspace's text provider, keeps each user's conversations and records
//! AI use in the activity feed.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_api::{agent_capabilities, agent_conversation_title, agent_turn_label};
use repomemo_domain::{
    AgentCapability, AgentCapabilityInfo, AgentConversation, AgentConversationDetail,
    AgentMessage, AgentRequest, AgentTurn, ProviderSettings,
};
use serde::{Deserialize, Serialize};

use crate::{
    map_core_error, map_storage_error, record_workspace_activity, require_workspace_read,
    ApiError, AppState, AuthenticatedSubject,
};

const MAX_TITLE_CHARS: usize = 120;

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
    /// Continue this conversation; a new one is started when omitted.
    conversation_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AgentTurnResponse {
    conversation: AgentConversation,
    turn: AgentTurn,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RenameConversationRequest {
    title: String,
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
) -> Result<Json<AgentTurnResponse>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let existing = match request.conversation_id.as_deref() {
        Some(conversation_id) => {
            let conversation = owned_conversation(&state, &subject, conversation_id).await?;
            if conversation.workspace_id != workspace_id {
                return Err(conversation_not_found());
            }
            Some(conversation)
        }
        None => None,
    };
    let message = AgentMessage {
        message: request.message,
        capability: request.capability,
        artifact_id: request.artifact_id,
    };
    let provider = text_provider(&state, &workspace_id).await?;
    let reply = state
        .core
        .run_agent(AgentRequest {
            workspace_id: workspace_id.clone(),
            message: message.message.clone(),
            capability: message.capability,
            artifact_id: message.artifact_id.clone(),
            provider_id: provider.map(|provider| provider.id),
        })
        .await
        .map_err(map_core_error)?;

    // The conversation is only created once there is a reply to keep, so a
    // rejected first message leaves no empty chat behind.
    let label = agent_turn_label(&message);
    let conversation_id = match existing {
        Some(conversation) => conversation.id,
        None => {
            state
                .storage
                .create_agent_conversation(
                    &workspace_id,
                    &subject.user_id,
                    &agent_conversation_title(&label),
                )
                .await
                .map_err(map_storage_error)?
                .id
        }
    };
    let turn = state
        .storage
        .append_agent_turn(&conversation_id, &label, &message, &reply)
        .await
        .map_err(map_storage_error)?;
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
    Ok(Json(AgentTurnResponse {
        conversation: owned_conversation(&state, &subject, &conversation_id).await?,
        turn,
    }))
}

pub(crate) async fn list_agent_conversations(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<AgentConversation>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_agent_conversations(&workspace_id, &subject.user_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn get_agent_conversation(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
) -> Result<Json<AgentConversationDetail>, ApiError> {
    let conversation = owned_conversation(&state, &subject, &conversation_id).await?;
    require_workspace_read(&state, &subject, &conversation.workspace_id).await?;
    let turns = state
        .storage
        .list_agent_turns(&conversation.id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(AgentConversationDetail {
        conversation,
        turns,
    }))
}

pub(crate) async fn rename_agent_conversation(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
    Json(request): Json<RenameConversationRequest>,
) -> Result<Json<AgentConversation>, ApiError> {
    let conversation = owned_conversation(&state, &subject, &conversation_id).await?;
    require_workspace_read(&state, &subject, &conversation.workspace_id).await?;
    let title = request.title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
        return Err(ApiError::bad_request(format!(
            "Conversation titles must be between 1 and {MAX_TITLE_CHARS} characters."
        )));
    }
    state
        .storage
        .rename_agent_conversation(&conversation_id, &subject.user_id, &title)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn delete_agent_conversation(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let conversation = owned_conversation(&state, &subject, &conversation_id).await?;
    require_workspace_read(&state, &subject, &conversation.workspace_id).await?;
    state
        .storage
        .delete_agent_conversation(&conversation_id, &subject.user_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

/// The caller's own conversation. Someone else's is reported as missing so
/// its existence is not revealed.
async fn owned_conversation(
    state: &AppState,
    subject: &AuthenticatedSubject,
    conversation_id: &str,
) -> Result<AgentConversation, ApiError> {
    state
        .storage
        .get_agent_conversation(conversation_id, &subject.user_id)
        .await
        .map_err(|error| {
            if error.to_string().contains("was not found") {
                conversation_not_found()
            } else {
                map_storage_error(error)
            }
        })
}

fn conversation_not_found() -> ApiError {
    ApiError::not_found("Assistant conversation was not found.")
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
