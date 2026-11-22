//! Confirming or dismissing what the assistant proposed (V0.9.10). A proposal in a reply does nothing until its person
//! confirms it. It then runs here as that person, through the very handlers they would call themselves, so every check of the
//! workspace (role, quota, repository files, folders) applies unchanged and the activity feed records the person, not the
//! assistant. The outcome is written back into the stored reply, so a reload shows what was done and a second confirmation is
//! refused.

use axum::{
    extract::{Path, State},
    Json,
};
use repomemo_domain::{AgentProposal, AgentProposalKind, AgentProposalState, AgentTurn};

use crate::{
    map_storage_error, requests::CreateMemoryCardBody, require_workspace_write, retrieval, ApiError, AppState,
    AuthenticatedSubject,
};

/// The caller's turn with the conversation it belongs to checked: someone else's is reported as missing.
async fn owned_turn(
    state: &AppState,
    subject: &AuthenticatedSubject,
    turn_id: &str,
) -> Result<(AgentTurn, String), ApiError> {
    let turn = state
        .storage
        .get_agent_turn(turn_id)
        .await
        .map_err(|_| ApiError::not_found("That message was not found."))?;
    let conversation = state
        .storage
        .get_agent_conversation(&turn.conversation_id, &subject.user_id)
        .await
        .map_err(|_| ApiError::not_found("That message was not found."))?;
    Ok((turn, conversation.workspace_id))
}

fn pending_proposal<'a>(turn: &'a mut AgentTurn, proposal_id: &str) -> Result<&'a mut AgentProposal, ApiError> {
    let proposal = turn
        .reply
        .proposals
        .iter_mut()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| ApiError::not_found("That proposal was not found."))?;
    if proposal.state != AgentProposalState::Pending {
        return Err(ApiError::conflict("That proposal was already answered."));
    }
    Ok(proposal)
}

/// Runs a proposal's action as the person and says what happened, in a sentence.
async fn apply(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: &str,
    proposal: &AgentProposal,
) -> Result<String, ApiError> {
    match proposal.kind {
        AgentProposalKind::SaveMemory => {
            let body: CreateMemoryCardBody = serde_json::from_value(proposal.payload.clone())
                .map_err(|_| ApiError::bad_request("This proposal can no longer be applied."))?;
            let (_, Json(card)) = retrieval::create_memory_card(
                subject.clone(),
                State(state.clone()),
                Path(workspace_id.to_owned()),
                Json(body),
            )
            .await?;
            Ok(format!("Saved the memory card “{}”.", card.title))
        }
    }
}

pub(crate) async fn confirm_agent_proposal(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((turn_id, proposal_id)): Path<(String, String)>,
) -> Result<Json<AgentTurn>, ApiError> {
    let (mut turn, workspace_id) = owned_turn(&state, &subject, &turn_id).await?;
    // Whatever the action, the person must still be able to change the workspace.
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let proposal = pending_proposal(&mut turn, &proposal_id)?;
    let outcome = apply(&state, &subject, &workspace_id, proposal).await?;
    proposal.state = AgentProposalState::Applied;
    proposal.outcome = Some(outcome);
    state
        .storage
        .update_agent_turn_reply(&turn.id, &turn.reply)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(turn))
}

pub(crate) async fn dismiss_agent_proposal(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((turn_id, proposal_id)): Path<(String, String)>,
) -> Result<Json<AgentTurn>, ApiError> {
    let (mut turn, _) = owned_turn(&state, &subject, &turn_id).await?;
    let proposal = pending_proposal(&mut turn, &proposal_id)?;
    proposal.state = AgentProposalState::Dismissed;
    state
        .storage
        .update_agent_turn_reply(&turn.id, &turn.reply)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(turn))
}
