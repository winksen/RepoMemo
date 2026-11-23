//! HTTP surface of the workspace assistant. Routing and the capabilities
//! themselves live in `RepoMemoCore`; this layer adds membership checks, picks
//! the workspace's text provider, keeps each user's conversations and records
//! AI use in the activity feed.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_api::{
    agent_capabilities, agent_conversation_title, agent_run_trace, agent_turn_label, attachment_reply,
    capability_needs_write, HISTORY_MAX_TURNS,
};
use repomemo_domain::{
    AgentCapability, AgentCapabilityInfo, AgentConversation, AgentConversationDetail, AgentExchange, AgentHistoryFile,
    AgentHistoryTurn, AgentMessage, AgentRequest, AgentRun, AgentTurn, ProviderSettings, WorkspaceRole,
};
use serde::{Deserialize, Serialize};

use crate::{
    ai_access, map_core_error, map_storage_error, record_workspace_activity, require_workspace_read, ApiError,
    AppState, AuthenticatedSubject,
};

const MAX_TITLE_CHARS: usize = 120;

#[derive(Debug, Serialize)]
pub(crate) struct AgentCapabilitiesResponse {
    capabilities: Vec<AgentCapabilityInfo>,
    provider_name: Option<String>,
    /// The model the workspace's text provider is set to use, when it names one.
    model: Option<String>,
    /// Whether CAIRN works in a loop in this workspace (V0.9.40), so a client can run it live (V0.9.43).
    agent_mode: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AgentMessageRequest {
    #[serde(default)]
    message: String,
    capability: Option<AgentCapability>,
    artifact_id: Option<String>,
    /// Files selected in an earlier reply that the message is about (V0.9.26).
    #[serde(default)]
    artifact_ids: Vec<String>,
    /// Continue this conversation; a new one is started when omitted.
    conversation_id: Option<String>,
}

/// A file the person uploaded in the chat, which the upload route has already stored (V0.9.12).
#[derive(Debug, Deserialize)]
pub(crate) struct AgentAttachmentRequest {
    artifact_id: String,
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
    let role = require_workspace_read(&state, &subject, &workspace_id).await?;
    let provider = text_provider(&state, &workspace_id).await?;
    // AI actions show as unavailable to roles the workspace keeps AI from.
    let provider = match provider {
        Some(provider) if ai_access::role_may_use_ai(&state, &workspace_id, &role).await? => Some(provider),
        _ => None,
    };
    let mut capabilities = agent_capabilities(provider.is_some());
    // A viewer cannot change the workspace, so what would change it is not offered (V0.9.10).
    if matches!(role, WorkspaceRole::Viewer) {
        for info in &mut capabilities {
            if capability_needs_write(info.id) {
                info.available = false;
            }
        }
    }
    let agent_mode = provider
        .as_ref()
        .is_some_and(|provider| crate::ai::cairn_mode_of(&provider.metadata) == "agent");
    let (provider_name, model) = match provider {
        Some(provider) => (
            Some(provider.name),
            provider.model.filter(|model| !model.trim().is_empty()),
        ),
        None => (None, None),
    };
    Ok(Json(AgentCapabilitiesResponse {
        capabilities,
        provider_name,
        model,
        agent_mode,
    }))
}

pub(crate) async fn send_agent_message(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<AgentMessageRequest>,
) -> Result<Json<AgentTurnResponse>, ApiError> {
    run_message(&state, &subject, workspace_id, request, LiveHooks::default())
        .await
        .map(Json)
}

/// What a live run (V0.9.43) adds to answering a message: a hook told of each step as it is taken, and the flag the person's Stop
/// sets.
#[derive(Default, Clone)]
pub(crate) struct LiveHooks {
    pub(crate) progress: Option<repomemo_api::ProgressHook>,
    pub(crate) cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

/// Answers one message and stores the exchange: the work of the synchronous route and of a live run alike.
pub(crate) async fn run_message(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: String,
    request: AgentMessageRequest,
    live: LiveHooks,
) -> Result<AgentTurnResponse, ApiError> {
    let role = require_workspace_read(state, subject, &workspace_id).await?;
    let existing = match request.conversation_id.as_deref() {
        Some(conversation_id) => {
            let conversation = owned_conversation(state, subject, conversation_id).await?;
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
        artifact_ids: request.artifact_ids,
    };
    // What goes with the question comes from the stored conversation, never
    // from the request, so a client cannot make the assistant "remember" text.
    let (history, exchanges) = match &existing {
        Some(conversation) => {
            let turns = state
                .storage
                .list_agent_turns(&conversation.id)
                .await
                .map_err(map_storage_error)?;
            (history_of(&turns), exchanges_of(&turns))
        }
        None => (Vec::new(), Vec::new()),
    };
    let provider = text_provider(state, &workspace_id).await?;
    let provider_model = provider.as_ref().map(|provider| {
        (
            provider.model.clone().filter(|model| !model.trim().is_empty()),
            repomemo_ai::ModelTraits::from_metadata(&provider.metadata)
                .tool_mode
                .as_str()
                .to_owned(),
        )
    });
    // In Agent mode (V0.9.40) a message is answered by a loop that spends one AI request for each model call, so the quota is
    // not spent up front.
    let agent_setup = provider
        .as_ref()
        .filter(|provider| crate::ai::cairn_mode_of(&provider.metadata) == "agent")
        .map(|provider| {
            let traits = repomemo_ai::ModelTraits::from_metadata(&provider.metadata);
            let runtime = state.settings();
            let (quota_state, quota_subject) = (state.clone(), subject.clone());
            repomemo_api::AgentLoopSetup {
                tool_mode: traits.tool_mode,
                context_tokens: traits.context_tokens,
                max_steps: runtime.cairn_max_steps.clamp(1, 30) as usize,
                max_seconds: runtime.cairn_max_run_seconds.clamp(10, 600) as u64,
                can_write: !matches!(role, WorkspaceRole::Viewer),
                progress: live.progress.clone(),
                cancel: live.cancel.clone(),
                consume_quota: std::sync::Arc::new(move || {
                    ai_access::consume_ai_quota(&quota_state, &quota_subject).map_err(|error| error.message)
                }),
            }
        });
    let (provider_id, ai_unavailable_reason) = match provider {
        Some(provider) => {
            let min_role = ai_access::workspace_ai_min_role(state, &workspace_id).await?;
            if min_role.allows(&role) {
                // Lookups and searches never call the provider; anything else
                // may, if only to understand the message.
                let may_call_ai = match message.capability {
                    None => agent_setup.is_none(),
                    Some(capability) => agent_capabilities(true)
                        .iter()
                        .any(|info| info.id == capability && info.requires_ai),
                };
                if may_call_ai {
                    ai_access::consume_ai_quota(state, subject)?;
                }
                (Some(provider.id), None)
            } else {
                (None, Some(ai_access::ai_not_allowed_message(min_role)))
            }
        }
        None => (None, None),
    };
    let provider_for_run = provider_id.clone();
    let started_at = chrono::Utc::now();
    let outcome = state
        .core
        .run_agent_with(
            AgentRequest {
                workspace_id: workspace_id.clone(),
                message: message.message.clone(),
                capability: message.capability,
                artifact_id: message.artifact_id.clone(),
                artifact_ids: message.artifact_ids.clone(),
                provider_id,
                ai_unavailable_reason,
                history,
                exchanges,
            },
            agent_setup,
        )
        .await
        .map_err(map_core_error)?;
    let (mut reply, loop_trace) = (outcome.reply, outcome.trace);
    // A viewer is not offered what would change the workspace (V0.9.10).
    if matches!(role, WorkspaceRole::Viewer) && !reply.proposals.is_empty() {
        reply.proposals.clear();
        reply
            .warnings
            .push("Your role in this workspace cannot change it, so I cannot offer to do that.".to_owned());
    }

    // The conversation is only created once there is a reply to keep, so a
    // rejected first message leaves no empty chat behind.
    let label = agent_turn_label(&message);
    let conversation_id = match existing {
        Some(conversation) => conversation.id,
        None => {
            state
                .storage
                .create_agent_conversation(&workspace_id, &subject.user_id, &agent_conversation_title(&label))
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
    // How the message was answered, kept for the person who asked (V0.9.39). A trace that cannot be stored never fails the answer.
    let ended_at = chrono::Utc::now();
    let trace = loop_trace
        .unwrap_or_else(|| agent_run_trace(&message, &reply, (ended_at - started_at).num_milliseconds().max(0)));
    let (model, tool_mode) = match (&provider_for_run, provider_model) {
        (Some(_), Some((model, tool_mode))) => (model, Some(tool_mode)),
        _ => (None, None),
    };
    if let Err(error) = state
        .storage
        .record_agent_run(repomemo_storage::NewAgentRun {
            turn_id: Some(turn.id.clone()),
            conversation_id: conversation_id.clone(),
            user_id: subject.user_id.clone(),
            workspace_id: workspace_id.clone(),
            origin: "chat".to_owned(),
            provider_id: provider_for_run,
            model,
            tool_mode,
            state: trace.state,
            input_tokens: trace.input_tokens,
            output_tokens: trace.output_tokens,
            started_at: started_at.to_rfc3339(),
            ended_at: Some(ended_at.to_rfc3339()),
            steps: trace.steps,
        })
        .await
    {
        tracing::warn!(error = %format!("{error:#}"), "A CAIRN run trace could not be stored");
    }
    if reply.generated {
        record_workspace_activity(
            state,
            &workspace_id,
            &subject.user_id,
            "assistant_answered",
            "workspace",
            Some(&workspace_id),
            format!(
                "Asked the assistant: {}",
                message.message.trim().chars().take(240).collect::<String>()
            ),
        )
        .await;
    }
    Ok(AgentTurnResponse {
        conversation: owned_conversation(state, subject, &conversation_id).await?,
        turn,
    })
}

/// Records an uploaded file as a turn of the chat. The upload itself (its role check, its quota, its indexing) is the upload
/// route's; this adds what the chat shows: that CAIRN has the file and when it can be asked about.
pub(crate) async fn attach_agent_file(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<AgentAttachmentRequest>,
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
    let file = state
        .storage
        .get_artifact_summary(&request.artifact_id)
        .await
        .map_err(|_| ApiError::not_found("That file was not found."))?;
    if file.workspace_id != workspace_id {
        return Err(ApiError::not_found("That file was not found."));
    }
    let label = format!("Uploaded {}", file.title);
    let message = AgentMessage {
        message: label.clone(),
        capability: None,
        artifact_id: Some(file.id.clone()),
        artifact_ids: Vec::new(),
    };
    let reply = attachment_reply(file);
    let conversation_id = match existing {
        Some(conversation) => conversation.id,
        None => {
            state
                .storage
                .create_agent_conversation(&workspace_id, &subject.user_id, &agent_conversation_title(&label))
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
    Ok(Json(AgentTurnResponse {
        conversation: owned_conversation(&state, &subject, &conversation_id).await?,
        turn,
    }))
}

/// What CAIRN did for one answer: the steps of its run. Only the person whose chat it is may read it, and an older answer, or one
/// whose trace expired, has none.
pub(crate) async fn get_agent_turn_run(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(turn_id): Path<String>,
) -> Result<Json<AgentRun>, ApiError> {
    let turn = state
        .storage
        .get_agent_turn(&turn_id)
        .await
        .map_err(|_| conversation_not_found())?;
    let conversation = owned_conversation(&state, &subject, &turn.conversation_id).await?;
    require_workspace_read(&state, &subject, &conversation.workspace_id).await?;
    state
        .storage
        .get_agent_run_for_turn(&turn.id)
        .await
        .map_err(map_storage_error)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("CAIRN kept no record of the steps behind that answer."))
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
    Ok(Json(AgentConversationDetail { conversation, turns }))
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

/// The newest turns of a conversation as the assistant is reminded of them.
/// A turn is about the files its reply listed or summarized, else those it
/// cited; the core trims the lot to its token budget.
fn history_of(turns: &[AgentTurn]) -> Vec<AgentHistoryTurn> {
    let start = turns.len().saturating_sub(HISTORY_MAX_TURNS);
    turns[start..]
        .iter()
        .map(|turn| {
            let mut files = turn
                .reply
                .files
                .iter()
                .map(|file| AgentHistoryFile {
                    id: file.id.clone(),
                    title: file.title.clone(),
                })
                .collect::<Vec<_>>();
            if files.is_empty() {
                for citation in &turn.reply.citations {
                    if !files.iter().any(|file| file.id == citation.artifact_id) {
                        files.push(AgentHistoryFile {
                            id: citation.artifact_id.clone(),
                            title: citation.title.clone(),
                        });
                    }
                }
            }
            AgentHistoryTurn {
                question: turn.label.clone(),
                answer: turn.reply.reply_markdown.clone(),
                files,
            }
        })
        .collect()
}

/// Every earlier exchange in full, for the jobs that act on an earlier answer (V0.9.10).
fn exchanges_of(turns: &[AgentTurn]) -> Vec<AgentExchange> {
    turns
        .iter()
        .map(|turn| AgentExchange {
            question: turn.label.clone(),
            answer: turn.reply.reply_markdown.clone(),
            citations: turn.reply.citations.clone(),
            file_ids: turn.reply.file_set.as_ref().map_or_else(
                || turn.reply.files.iter().map(|file| file.id.clone()).collect(),
                |set| set.ids.clone(),
            ),
        })
        .collect()
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
        .map_err(map_storage_error)
}

fn conversation_not_found() -> ApiError {
    ApiError::not_found("Assistant conversation was not found.")
}

async fn text_provider(state: &AppState, workspace_id: &str) -> Result<Option<ProviderSettings>, ApiError> {
    Ok(state
        .storage
        .list_provider_settings(workspace_id)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .find(|setting| setting.enabled && setting.purpose() == "text"))
}
