//! CAIRN chats shared with the workspace (V0.9.57). The owner shares and unshares; anyone who can read the workspace lists the shared
//! chats, opens one read-only and can continue it in a chat of their own. Confirming what a chat proposed stays with its owner
//! (`agent_proposals.rs` only ever looks up the caller's own turns), and unsharing ends access at once.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_domain::{AgentConversation, AgentConversationDetail, SharedConversation};

use crate::{map_storage_error, require_workspace_read, ApiError, AppState, AuthenticatedSubject};

pub(crate) async fn share_agent_conversation(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let conversation = state
        .storage
        .get_agent_conversation(&conversation_id, &subject.user_id)
        .await
        .map_err(map_storage_error)?;
    require_workspace_read(&state, &subject, &conversation.workspace_id).await?;
    state
        .storage
        .share_agent_conversation(&conversation_id, &subject.user_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn unshare_agent_conversation(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .unshare_agent_conversation(&conversation_id, &subject.user_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn list_shared_agent_conversations(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<SharedConversation>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_shared_agent_conversations(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn get_shared_agent_conversation(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, conversation_id)): Path<(String, String)>,
) -> Result<Json<AgentConversationDetail>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let conversation = state
        .storage
        .get_shared_agent_conversation(&conversation_id, &workspace_id)
        .await
        .map_err(map_storage_error)?;
    let turns = state
        .storage
        .list_agent_turns(&conversation.id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(AgentConversationDetail { conversation, turns }))
}

pub(crate) async fn continue_shared_agent_conversation(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, conversation_id)): Path<(String, String)>,
) -> Result<(StatusCode, Json<AgentConversation>), ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .copy_shared_agent_conversation(&conversation_id, &workspace_id, &subject.user_id)
        .await
        .map(|copy| (StatusCode::CREATED, Json(copy)))
        .map_err(map_storage_error)
}
