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
    capability_needs_write, history_window_start, usable_summary, TURNS_BEFORE_REFRESH,
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
    /// The models a person can pick for a chat here (V0.9.86); the one that answers without a choice is marked `default`.
    models: Vec<OfferedModel>,
    /// What the person has left of today's CAIRN budgets; none when no budget is set (V0.9.96).
    budget: Option<repomemo_storage::PersonBudget>,
    /// CAIRN's greeting and starter questions for the workspace (V0.9.116); empty where nobody wrote any.
    welcome: repomemo_storage::Welcome,
    /// What the person chose for how CAIRN works for them (V0.9.114), for the chat to apply.
    preferences: PreferencesNote,
    /// The abilities the workspace offers (V0.9.106): its organization's that are on and its own, each run with its /command.
    abilities: Vec<crate::cairn_abilities::OfferedAbility>,
}

/// The preferences the chat itself applies.
#[derive(Debug, Serialize)]
pub(crate) struct PreferencesNote {
    /// Whether "What CAIRN did" shows open.
    steps_open: bool,
}

/// A model offered in a workspace, as the chat's picker shows it.
#[derive(Debug, Serialize)]
pub(crate) struct OfferedModel {
    id: String,
    name: String,
    /// `local` or `cloud`.
    locality: String,
    /// It takes tools, so CAIRN can work in a loop with it.
    agent: bool,
    default: bool,
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
    /// Start the chat across all the person's workspaces (V0.9.60). Only a new chat takes it: a chat keeps the scope it was made with.
    #[serde(default)]
    all_workspaces: bool,
    /// The catalog model to use from now in this chat (V0.9.86); it must be offered in the workspace. Omitted keeps the chat's own.
    #[serde(default)]
    model_id: Option<String>,
}

/// A file the person uploaded in the chat, which the upload route has already stored (V0.9.12).
#[derive(Debug, Deserialize)]
pub(crate) struct AgentAttachmentRequest {
    artifact_id: String,
    conversation_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AgentTurnResponse {
    pub(crate) conversation: AgentConversation,
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
    let provider = text_provider_for_role(&state, &workspace_id, &role).await?;
    let models = offered_models(&state, &workspace_id, provider.as_ref(), &role).await?;
    let budget = state
        .storage
        .person_budget(
            &workspace_id,
            &subject.user_id,
            subject.is_system_admin || matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin),
        )
        .await
        .ok()
        .filter(|budget| !budget.exempt);
    // AI actions show as unavailable to roles the workspace keeps AI from.
    let provider = match provider {
        Some(provider) if ai_access::role_may_use_ai(&state, &workspace_id, &role).await? => Some(provider),
        _ => None,
    };
    let mut capabilities = agent_capabilities(provider.is_some());
    // A job that was turned off is shown as unavailable, with who turned it off (V0.9.91).
    let off = state
        .storage
        .tools_off_for_workspace(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    for info in &mut capabilities {
        if let Some((_, by)) = off
            .iter()
            .find(|(name, _)| *name == crate::cairn_tools::job_id(info.id))
        {
            info.available = false;
            info.switched_off_by = Some((*by).to_owned());
        }
    }
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
        .is_some_and(|provider| crate::ai::cairn_mode_of(provider) == "agent");
    let (provider_name, model) = match provider {
        Some(provider) => (
            Some(provider.name),
            provider.model.filter(|model| !model.trim().is_empty()),
        ),
        None => (None, None),
    };
    let abilities = crate::cairn_abilities::offered(&state, &workspace_id, agent_mode).await?;
    let person = state
        .storage
        .effective_cairn_preferences(&subject.user_id, &workspace_id)
        .await
        .unwrap_or_default();
    // The picker shows the person's preferred model as the one that answers, while it is offered here.
    let mut models = models;
    if let Some(preferred) = person
        .models
        .get(&workspace_id)
        .filter(|id| models.iter().any(|model| &model.id == *id))
    {
        for model in &mut models {
            model.default = &model.id == preferred;
        }
    }
    let welcome = state
        .storage
        .welcome_for_workspace(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(AgentCapabilitiesResponse {
        welcome,
        preferences: PreferencesNote {
            steps_open: person.steps_open.unwrap_or(false),
        },
        abilities,
        capabilities,
        provider_name,
        model,
        agent_mode,
        models,
        budget: budget.filter(|budget| budget.runs_limit.is_some() || budget.tokens_limit.is_some()),
    }))
}

/// The models a person can pick for a chat in a workspace, with where each runs and whether it takes tools.
async fn offered_models(
    state: &AppState,
    workspace_id: &str,
    answering: Option<&ProviderSettings>,
    role: &WorkspaceRole,
) -> Result<Vec<OfferedModel>, ApiError> {
    let answering = answering
        .and_then(|provider| provider.metadata.get("cairn_model_id"))
        .and_then(serde_json::Value::as_str);
    let mut models = Vec::new();
    for id in state
        .storage
        .offered_text_model_ids_for(workspace_id, Some(role_name(role)))
        .await
        .map_err(map_storage_error)?
    {
        let model = state.storage.get_cairn_model(&id).await.map_err(map_storage_error)?;
        let connection = state
            .storage
            .get_cairn_connection(&model.connection_id)
            .await
            .map_err(map_storage_error)?;
        models.push(OfferedModel {
            default: answering == Some(id.as_str()),
            agent: model.tool_mode != "none",
            locality: connection.locality,
            name: model.display_name,
            id,
        });
    }
    Ok(models)
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
    /// A run nobody is watching, such as a routine's (V0.9.63): it has read tools only and proposes nothing.
    pub(crate) read_only: bool,
}

/// Runs a routine's prompt as its owner into the routine's chat, with read tools only (V0.9.63).
pub(crate) async fn run_routine_message(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: String,
    prompt: String,
    conversation_id: Option<String>,
) -> Result<AgentTurnResponse, ApiError> {
    let request = AgentMessageRequest {
        message: prompt,
        capability: None,
        artifact_id: None,
        artifact_ids: Vec::new(),
        conversation_id,
        all_workspaces: false,
        model_id: None,
    };
    run_message(
        state,
        subject,
        workspace_id,
        request,
        LiveHooks {
            read_only: true,
            ..LiveHooks::default()
        },
    )
    .await
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
    let (turns, history, exchanges) = match &existing {
        Some(conversation) => {
            let turns = state
                .storage
                .list_agent_turns(&conversation.id)
                .await
                .map_err(map_storage_error)?;
            let (history, exchanges) = (history_of(&turns), exchanges_of(&turns));
            (turns, history, exchanges)
        }
        None => (Vec::new(), Vec::new(), Vec::new()),
    };
    // The model of the chat: the one the person picks now, else the one the chat has. A model that is not offered here is refused,
    // and one the chat has that is no longer offered is replaced by what the workspace resolves, with a note (V0.9.86).
    // The person's own preferences (V0.9.114): settings they chose, read into their own runs only.
    let preferences = state
        .storage
        .effective_cairn_preferences(&subject.user_id, &workspace_id)
        .await
        .unwrap_or_default();
    let requested_model = request.model_id.clone().filter(|id| !id.trim().is_empty());
    if let Some(model_id) = &requested_model {
        let offered = state
            .storage
            .offered_text_model_ids_for(&workspace_id, Some(role_name(&role)))
            .await
            .map_err(map_storage_error)?;
        if !offered.contains(model_id) {
            return Err(ApiError::bad_request("That model is not offered in this workspace."));
        }
    }
    let mut chosen_model = requested_model
        .clone()
        .or_else(|| existing.as_ref().and_then(|conversation| conversation.model_id.clone()));
    // With no choice for this chat, the person's preferred model of the workspace, while it is still offered there.
    if chosen_model.is_none() {
        if let Some(preferred) = preferences.models.get(&workspace_id) {
            let offered = state
                .storage
                .offered_text_model_ids_for(&workspace_id, Some(role_name(&role)))
                .await
                .map_err(map_storage_error)?;
            if offered.contains(preferred) {
                chosen_model = Some(preferred.clone());
            }
        }
    }
    let resolution = state
        .storage
        .resolve_text_provider_for(&workspace_id, chosen_model.as_deref(), Some(role_name(&role)))
        .await
        .map_err(map_storage_error)?;
    // Said in the answer when the workspace's own model is not the one that answers.
    let model_notice = resolution
        .resolved
        .as_ref()
        .and_then(|resolved| resolved.notice.clone())
        .or_else(|| resolution.notice.clone());
    let provider = resolution.resolved.map(|resolved| resolved.provider);
    let context_tokens = provider
        .as_ref()
        .map(|provider| repomemo_ai::ModelTraits::from_metadata(&provider.metadata).context_tokens);
    let provider_model = provider.as_ref().map(|provider| {
        (
            provider.model.clone().filter(|model| !model.trim().is_empty()),
            repomemo_ai::ModelTraits::from_metadata(&provider.metadata)
                .tool_mode
                .as_str()
                .to_owned(),
        )
    });
    // What was turned off for this workspace: tools the loop is not given, and jobs that are refused (V0.9.91).
    let tools_off = state
        .storage
        .tools_off_for_workspace(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    // Personal memory off in this workspace: CAIRN is not given the tool that proposes it, and reads none (V0.9.111).
    let personal_memory_on = state
        .storage
        .personal_memory_policy_for_workspace(&workspace_id)
        .await
        .map_err(map_storage_error)?
        .enabled;
    // The organization's instructions for every run in its workspaces (V0.9.109).
    let organization_instructions = state
        .storage
        .organization_instructions_for_workspace(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    // The organization's knowledge workspace, only for a person who can read it (V0.9.110).
    let knowledge_workspace = match state
        .storage
        .knowledge_workspace_for(&workspace_id)
        .await
        .map_err(map_storage_error)?
    {
        Some(knowledge) if subject.token_reaches(&knowledge) => {
            crate::effective_workspace_role(state, &subject.user_id, subject.is_system_admin, &knowledge)
                .await?
                .map(|_| knowledge)
        }
        _ => None,
    };
    let instructions_version = organization_instructions.as_ref().map(|(_, version)| *version);
    // The model's fallback, when it has one and may be used (V0.9.80).
    let fallback = match provider.as_ref() {
        Some(provider) if crate::ai::cairn_mode_of(provider) == "agent" => {
            crate::cairn_models::fallback_settings(state, provider).await
        }
        _ => None,
    };
    // In Agent mode (V0.9.40) a message is answered by a loop that spends one AI request for each model call, so the quota is
    // not spent up front.
    let mut agent_setup = provider
        .as_ref()
        .filter(|provider| crate::ai::cairn_mode_of(provider) == "agent")
        // A person who prefers Jobs gets Jobs, where both are possible (V0.9.114).
        .filter(|_| preferences.mode.as_deref() != Some("jobs"))
        .map(|provider| {
            let traits = repomemo_ai::ModelTraits::from_metadata(&provider.metadata);
            let runtime = state.settings();
            let (quota_state, quota_subject) = (state.clone(), subject.clone());
            repomemo_api::AgentLoopSetup {
                tool_mode: traits.tool_mode,
                context_tokens: traits.context_tokens,
                max_steps: runtime.cairn_max_steps.clamp(1, 30) as usize,
                max_seconds: runtime.cairn_max_run_seconds.clamp(10, 600) as u64,
                max_run_tokens: runtime.cairn_max_run_tokens.max(0) as u64,
                disabled_tools: tools_off
                    .iter()
                    .map(|(name, _)| name.clone())
                    .chain((!personal_memory_on).then(|| "propose_personal_memory".to_owned()))
                    .collect(),
                ability: None,
                style: preferences.style_note(),
                organization_instructions: organization_instructions.clone(),
                knowledge_workspace,
                fallback,
                can_write: !matches!(role, WorkspaceRole::Viewer) && !live.read_only,
                user_id: subject.user_id.clone(),
                background_max_files: runtime.cairn_background_max_files.clamp(8, 1000) as usize,
                progress: live.progress.clone(),
                cancel: live.cancel.clone(),
                consume_quota: std::sync::Arc::new(move || {
                    ai_access::consume_ai_quota(&quota_state, &quota_subject).map_err(|error| error.message)
                }),
            }
        });
    // An ability run with its /command (V0.9.107): it needs Agent mode, asks for its fields first, and follows its instructions with
    // only its tools, of the tools the run has.
    let mut ability_refusal: Option<repomemo_domain::AgentReply> = None;
    let mut ran_ability: Option<(String, i64)> = None;
    if message.capability.is_none() && !request.all_workspaces {
        if let Some((command, rest)) = split_command(&message.message) {
            let abilities = state
                .storage
                .abilities_for_workspace(&workspace_id)
                .await
                .map_err(map_storage_error)?;
            if let Some(ability) = abilities.into_iter().find(|ability| ability.command == command) {
                let missing = missing_inputs(rest, &ability.inputs);
                if agent_setup.is_none() {
                    ability_refusal = Some(repomemo_api::lookup_reply(format!(
                        "{} ({}) needs Agent mode. It is not on for you here: this workspace's model may not run in it, or you chose Jobs in your preferences.",
                        ability.name, ability.command
                    )));
                } else if !missing.is_empty() {
                    let lines: Vec<String> = missing.iter().map(|label| format!("{label}: ")).collect();
                    ability_refusal = Some(repomemo_api::lookup_reply(format!(
                        "{} needs a little more first. Send {} again with:

{}",
                        ability.name,
                        ability.command,
                        lines.join(
                            "
"
                        )
                    )));
                } else if let Some(setup) = agent_setup.as_mut() {
                    ran_ability = Some((ability.name.clone(), ability.version));
                    setup.ability = Some(repomemo_api::AgentAbility {
                        name: ability.name,
                        command: ability.command,
                        version: ability.version,
                        instructions: ability.instructions,
                        tools: ability.tools,
                        output_template: ability.output_template,
                    });
                }
            }
        }
    }
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
                // The person's daily budgets of runs and tokens (V0.9.96): a run that would call the model is refused once used up.
                if (may_call_ai || agent_setup.is_some()) && !live.read_only {
                    let budget = state
                        .storage
                        .person_budget(
                            &workspace_id,
                            &subject.user_id,
                            subject.is_system_admin || matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin),
                        )
                        .await
                        .map_err(map_storage_error)?;
                    if !budget.allows_a_run() {
                        return Err(ApiError::new(
                            axum::http::StatusCode::TOO_MANY_REQUESTS,
                            "budget_used",
                            budget.refusal(),
                        ));
                    }
                }
                // The organization's monthly token budget (V0.9.97): administrators are told at 80 and 100 percent, once; at the cap CAIRN
                // stops, or keeps to models on the network.
                if (may_call_ai || agent_setup.is_some()) && !live.read_only {
                    if let Some(budget) = state
                        .storage
                        .organization_budget(&workspace_id)
                        .await
                        .map_err(map_storage_error)?
                    {
                        for threshold in [80, 100] {
                            if budget.percent() >= threshold
                                && state
                                    .storage
                                    .claim_budget_notice(&budget.organization_id, &budget.month, threshold)
                                    .await
                                    .map_err(map_storage_error)?
                            {
                                notify_budget(state, &budget, threshold).await;
                            }
                        }
                        if budget.used >= budget.limit {
                            let cloud =
                                repomemo_ai::is_cloud_connection(&provider.provider_type, provider.base_url.as_deref());
                            if budget.at_budget != "local_only" || cloud {
                                return Err(ApiError::new(
                                    axum::http::StatusCode::TOO_MANY_REQUESTS,
                                    "organization_budget_used",
                                    if budget.at_budget == "local_only" {
                                        "Your organization used its monthly CAIRN token budget: only models on its own network answer until next month."
                                    } else {
                                        "Your organization used its monthly CAIRN token budget: CAIRN answers again next month."
                                    },
                                ));
                            }
                        }
                    }
                }
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
    // What the person told CAIRN was wrong earlier, and whether they stopped being offered to save decisions here (V0.9.53).
    let corrections = state
        .storage
        .open_agent_corrections(&workspace_id, &subject.user_id, 5)
        .await
        .unwrap_or_default();
    let decision_offers_off = match &existing {
        Some(conversation) => state
            .storage
            .decision_offers_off(&conversation.id)
            .await
            .unwrap_or(false),
        None => false,
    };
    // What CAIRN remembers about this person goes into their own run and nobody else's (V0.9.51).
    let personal_memory = state
        .storage
        .personal_memory_for_run(&subject.user_id, &workspace_id)
        .await
        .unwrap_or_default();
    // What goes with the message from the chat's older turns: as many as the model's window has room for, and a summary of the rest.
    let (history, summary, history_tokens) = conversation_memory(
        state,
        subject,
        &workspace_id,
        existing.as_ref().map(|conversation| conversation.id.as_str()),
        &turns,
        history,
        provider_id.as_deref(),
        context_tokens,
    )
    .await;
    let started_at = chrono::Utc::now();
    let all_workspaces = existing
        .as_ref()
        .map_or(request.all_workspaces, |conversation| conversation.all_workspaces);
    // A job asked for from the + menu that was turned off is refused before it runs.
    let refused_job = message
        .capability
        .and_then(|capability| switched_off_reply(&tools_off, capability))
        .or(ability_refusal);
    let outcome = if let Some(reply) = refused_job {
        repomemo_api::AgentOutcome {
            reply,
            correction: None,
            trace: None,
        }
    } else if all_workspaces {
        // Across workspaces the chat only looks things up, with no AI: each result is labelled with its own workspace (V0.9.60).
        repomemo_api::AgentOutcome {
            reply: all_workspaces_reply(
                state,
                subject,
                &workspace_id,
                &role,
                &message.message,
                chosen_model.as_deref(),
            )
            .await?,
            correction: None,
            trace: None,
        }
    } else {
        state
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
                    summary,
                    personal_memory,
                    corrections,
                    decision_offers_off,
                    history_tokens,
                    exchanges,
                },
                agent_setup,
            )
            .await
            .map_err(map_core_error)?
    };
    let (mut reply, loop_trace, correction) = (outcome.reply, outcome.trace, outcome.correction);
    // The answer says which ability, and which version, it followed.
    if let Some((name, version)) = &ran_ability {
        reply.reply_markdown = format!(
            "{}

_Ability: {name}, version {version}_",
            reply.reply_markdown.trim_end()
        );
    }
    // Strict citations (V0.9.98): an answer about the workspace's content that cites nothing it read is not shown, and says what was read.
    if reply.generated
        && reply.citations.is_empty()
        && reply.proposals.is_empty()
        && state
            .storage
            .strict_citations_for_workspace(&workspace_id)
            .await
            .map_err(map_storage_error)?
    {
        let read: Vec<String> = reply
            .files
            .iter()
            .map(|file| file.title.clone())
            .chain(reply.matches.iter().map(|hit| hit.title.clone()))
            .take(8)
            .collect();
        reply.reply_markdown = format!(
            "I could not ground this answer in what I read, so I am not showing it.{}",
            if read.is_empty() {
                " I read no file.".to_owned()
            } else {
                format!(" What I read: {}.", read.join(", "))
            }
        );
        reply.generated = false;
    }
    // A job the router picked from the words that was turned off: what it made is dropped, and the person is told who turned it off.
    if let Some(reply_job) = reply
        .capability
        .and_then(|capability| switched_off_reply(&tools_off, capability))
    {
        reply = reply_job;
    }
    if let Some(notice) = model_notice {
        reply.warnings.push(notice);
    }
    // A routine proposes nothing: nobody is there to confirm it (V0.9.63).
    if live.read_only {
        reply.proposals.clear();
        reply.changeset = None;
    }
    // A viewer is not offered what would change the workspace (V0.9.10).
    // Being remembered is not a change to the workspace, so a viewer is still offered that (V0.9.51).
    if matches!(role, WorkspaceRole::Viewer) && !reply.proposals.is_empty() {
        let before = reply.proposals.len();
        reply
            .proposals
            .retain(|proposal| proposal.kind == repomemo_domain::AgentProposalKind::SavePersonalMemory);
        if reply.proposals.len() < before {
            reply
                .warnings
                .push("Your role in this workspace cannot change it, so I cannot offer to do that.".to_owned());
        }
        if reply.proposals.is_empty() {
            reply.changeset = None;
        }
    }

    // The conversation is only created once there is a reply to keep, so a
    // rejected first message leaves no empty chat behind.
    let label = agent_turn_label(&message);
    let conversation_id = match existing {
        Some(conversation) => conversation.id,
        None => {
            let created = state
                .storage
                .create_agent_conversation(&workspace_id, &subject.user_id, &agent_conversation_title(&label))
                .await
                .map_err(map_storage_error)?;
            if all_workspaces {
                state
                    .storage
                    .set_agent_conversation_all_workspaces(&created.id)
                    .await
                    .map_err(map_storage_error)?;
            }
            created.id
        }
    };
    // The chat keeps the model the person picked, so it answers with it from one message to the next.
    if let Some(model_id) = &requested_model {
        state
            .storage
            .set_agent_conversation_model(&conversation_id, Some(model_id))
            .await
            .map_err(map_storage_error)?;
    }
    let turn = state
        .storage
        .append_agent_turn(&conversation_id, &label, &message, &reply)
        .await
        .map_err(map_storage_error)?;
    // What the person said was wrong is kept against the turn that holds the run's answer (V0.9.53).
    if let Some(statement) = correction {
        if let Err(error) = state
            .storage
            .add_agent_correction(
                &workspace_id,
                &subject.user_id,
                Some(&conversation_id),
                Some(&turn.id),
                &statement,
            )
            .await
        {
            tracing::warn!(error = %format!("{error:#}"), "A correction could not be stored");
        }
    }
    // How the message was answered, kept for the person who asked (V0.9.39). A trace that cannot be stored never fails the answer.
    let ended_at = chrono::Utc::now();
    let ran_loop = loop_trace.is_some();
    let mut trace = loop_trace
        .unwrap_or_else(|| agent_run_trace(&message, &reply, (ended_at - started_at).num_milliseconds().max(0)));
    // What steered the run is the first thing its trace says: the organization's instructions, then the ability.
    let mut notes: Vec<String> = Vec::new();
    if let (Some(version), true) = (instructions_version, ran_loop) {
        notes.push(format!("Organization instructions: version {version}"));
    }
    if let Some((name, version)) = &ran_ability {
        notes.push(format!("Ability: {name}, version {version}"));
    }
    for (index, summary) in notes.into_iter().enumerate() {
        trace.steps.insert(
            index,
            repomemo_domain::AgentStep {
                position: 0,
                kind: "note".to_owned(),
                tool: None,
                summary,
                evidence: Vec::new(),
                duration_ms: 0,
                error: None,
            },
        );
    }
    for (index, step) in trace.steps.iter_mut().enumerate() {
        step.position = index as i64 + 1;
    }
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
/// What a chat across all the person's workspaces answers (V0.9.60): the files and memory cards the search over every workspace they
/// can read finds, grouped by workspace, with no AI. It is the search of `POST /v1/search` run as the person, so each workspace's
/// rights and the token's workspaces apply exactly as there.
async fn all_workspaces_reply(
    state: &AppState,
    subject: &AuthenticatedSubject,
    home_workspace_id: &str,
    home_role: &WorkspaceRole,
    message: &str,
    chosen_model: Option<&str>,
) -> Result<repomemo_domain::AgentReply, ApiError> {
    let query: String = message
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(512)
        .collect();
    let request: crate::requests::SearchAllRequest =
        serde_json::from_value(serde_json::json!({ "query": query, "limit": 12 }))
            .map_err(|_| ApiError::bad_request("That could not be searched."))?;
    let (_, Json(mut found)) = crate::retrieval::search_all_workspaces(
        crate::access_control::ReadingSubject(subject.clone()),
        State(state.clone()),
        Json(request),
    )
    .await?;
    // What a workspace administrator closed to CAIRN is never returned in a chat across workspaces (V0.9.93).
    let mut closed: Vec<(String, std::collections::HashSet<String>)> = Vec::new();
    for hit in &found.results {
        if !closed.iter().any(|(workspace, _)| *workspace == hit.workspace_id) {
            let ids = state
                .storage
                .closed_artifact_ids(&hit.workspace_id)
                .await
                .map_err(map_storage_error)?;
            closed.push((hit.workspace_id.clone(), ids));
        }
    }
    found.results.retain(|hit| {
        !closed
            .iter()
            .any(|(workspace, ids)| *workspace == hit.workspace_id && ids.contains(&hit.result.artifact_id))
    });
    let mut names: Vec<(String, String)> = Vec::new();
    for hit in &found.results {
        if !names.iter().any(|(id, _)| *id == hit.workspace_id) {
            names.push((hit.workspace_id.clone(), hit.workspace_name.clone()));
        }
    }
    for card in &found.memory_cards {
        if !names.iter().any(|(id, _)| *id == card.workspace_id) {
            names.push((card.workspace_id.clone(), card.workspace_name.clone()));
        }
    }
    if names.is_empty() {
        return Ok(repomemo_api::lookup_reply(format!(
            "Nothing matched “{query}” in any of your {} workspaces.",
            found.workspaces.len()
        )));
    }
    // Writing an answer (V0.9.61): a passage goes to the provider of this chat only when its own workspace lets this person use AI
    // and is set up with that same provider. The rest is listed as found and not sent, and none of it is in the request.
    let mut answer: Option<String> = None;
    let mut held_back = String::new();
    let home_min = ai_access::workspace_ai_min_role(state, home_workspace_id).await?;
    // The model of the chat, as its home workspace gives it to this person (V0.9.88).
    let home_provider = state
        .storage
        .resolve_text_provider_for(home_workspace_id, chosen_model, Some(role_name(home_role)))
        .await
        .map_err(map_storage_error)?
        .resolved
        .map(|resolved| resolved.provider);
    if let (Some(home_provider), true) = (home_provider, home_min.allows(home_role)) {
        let home_model_id = home_provider
            .metadata
            .get("cairn_model_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let home_model_name = home_provider
            .model
            .clone()
            .unwrap_or_else(|| home_provider.name.clone());
        let mut allowed = std::collections::HashSet::new();
        let mut refusals = std::collections::BTreeMap::new();
        for (workspace_id, _) in &names {
            let reason = match crate::access_control::effective_workspace_role(
                state,
                &subject.user_id,
                subject.is_system_admin,
                workspace_id,
            )
            .await?
            {
                None => Some("you cannot read it".to_owned()),
                Some(role) => {
                    let min = ai_access::workspace_ai_min_role(state, workspace_id).await?;
                    if !min.allows(&role) {
                        Some("your role there may not use AI".to_owned())
                    } else {
                        // A workspace's passages go to the model only when that same model is offered there to this person's role
                        // (V0.9.88): its organization and the workspace offer it, and their limits by role allow this role.
                        let offers_model = match &home_model_id {
                            Some(model_id) => state
                                .storage
                                .offered_text_model_ids_for(workspace_id, Some(role_name(&role)))
                                .await
                                .map_err(map_storage_error)?
                                .contains(model_id),
                            None => false,
                        };
                        match text_provider_for_role(state, workspace_id, &role).await? {
                            Some(own) if offers_model || repomemo_api::same_provider(&own, &home_provider) => {
                                // Each workspace that contributes spends from the person's AI allowance.
                                ai_access::consume_ai_quota(state, subject)
                                    .err()
                                    .map(|_| "your AI allowance is used up".to_owned())
                            }
                            Some(_) if home_model_id.is_some() => {
                                Some(format!("it does not offer {home_model_name} to your role"))
                            }
                            Some(_) => Some("it uses a different AI provider".to_owned()),
                            None => Some("it has no AI provider".to_owned()),
                        }
                    }
                }
            };
            match reason {
                None => {
                    allowed.insert(workspace_id.clone());
                }
                Some(reason) => {
                    refusals.insert(workspace_id.clone(), reason);
                }
            }
        }
        let mut passages: Vec<repomemo_api::CrossPassage> = found
            .results
            .iter()
            .map(|hit| repomemo_api::CrossPassage {
                workspace_id: hit.workspace_id.clone(),
                workspace_name: hit.workspace_name.clone(),
                title: hit.result.title.clone(),
                location: hit.result.path.clone(),
                text: hit.result.snippet.clone(),
            })
            .collect();
        passages.extend(found.memory_cards.iter().map(|card| repomemo_api::CrossPassage {
            workspace_id: card.workspace_id.clone(),
            workspace_name: card.workspace_name.clone(),
            title: format!("Memory card: {}", card.card.title),
            location: "memory card".to_owned(),
            text: card.card.body_excerpt.clone(),
        }));
        let (sent, held) = repomemo_api::split_by_boundary(&passages, &allowed, &refusals);
        held_back = repomemo_api::held_back_lines(&held);
        if !sent.is_empty() {
            match repomemo_ai::provider_from_settings(home_provider) {
                Ok(provider) => match repomemo_api::answer_from(&provider, &query, &sent).await {
                    Ok(text) if !text.is_empty() => answer = Some(text),
                    Ok(_) => {}
                    Err(error) => {
                        tracing::warn!(error = %format!("{error:#}"), "An answer across workspaces could not be written")
                    }
                },
                Err(error) => {
                    tracing::warn!(error = %format!("{error:#}"), "The provider of a chat across workspaces could not be used")
                }
            }
        }
    }
    let mut text = match &answer {
        Some(answer) => format!("{answer}\n\n"),
        None => String::new(),
    };
    if !held_back.is_empty() {
        text.push_str(&format!("{held_back}\n\n"));
    }
    text.push_str(&format!(
        "Found in {} of your workspaces{}:\n",
        names.len(),
        if answer.is_some() { "" } else { ", with no AI" }
    ));
    for (workspace_id, name) in &names {
        text.push_str(&format!("\n**[{name}](/spaces/{workspace_id})**\n"));
        for hit in found
            .results
            .iter()
            .filter(|hit| hit.workspace_id == *workspace_id)
            .take(4)
        {
            let snippet = hit.result.snippet.split_whitespace().collect::<Vec<_>>().join(" ");
            let snippet: String = snippet.chars().take(160).collect();
            text.push_str(&format!("- {} (`{}`): {snippet}\n", hit.result.title, hit.result.path));
        }
        for card in found
            .memory_cards
            .iter()
            .filter(|card| card.workspace_id == *workspace_id)
            .take(3)
        {
            text.push_str(&format!(
                "- Memory card **{}**: {}\n",
                card.card.title, card.card.body_excerpt
            ));
        }
    }
    text.push_str(
        "\nThis chat looks things up across your workspaces. To change something, open a chat in that workspace.",
    );
    let mut reply = repomemo_api::lookup_reply(text);
    reply.generated = answer.is_some();
    Ok(reply)
}

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
/// The most stored turns of a chat read to build its history; the core keeps the newest that fit the model's window.
const MAX_TURNS_READ: usize = 200;

/// What goes with a message from the chat's older turns (V0.9.48). With a provider, the newest turns that fit 40% of the model's
/// window and a stored summary of the older ones, rewritten with one AI call when more than a few turns have fallen out since it
/// was written (the call uses one of the person's AI requests, and is skipped when none is left). A summary that claims turns the
/// chat no longer has is dropped and rebuilt. Without a provider the router's own small window applies and older turns are dropped.
#[allow(clippy::too_many_arguments)]
async fn conversation_memory(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: &str,
    conversation_id: Option<&str>,
    turns: &[AgentTurn],
    history: Vec<AgentHistoryTurn>,
    provider_id: Option<&str>,
    context_tokens: Option<u32>,
) -> (
    Vec<AgentHistoryTurn>,
    Option<repomemo_domain::AgentSummary>,
    Option<usize>,
) {
    let (Some(conversation_id), Some(provider_id), Some(context_tokens)) =
        (conversation_id, provider_id, context_tokens)
    else {
        return (history, None, None);
    };
    let budget = (context_tokens as usize * 40 / 100).max(500);
    let start = history_window_start(&history, budget);
    let stored = state.storage.get_agent_summary(conversation_id).await.ok().flatten();
    let last_position = turns.last().map_or(0, |turn| turn.position);
    let mut summary = usable_summary(stored.clone(), last_position);
    if stored.is_some() && summary.is_none() {
        let _ = state.storage.set_agent_summary(conversation_id, None).await;
    }
    let covered = summary.as_ref().map_or(0, |summary| summary.covers_up_to);
    let offset = turns.len().saturating_sub(history.len());
    let fallen: Vec<(i64, AgentHistoryTurn)> = history[..start]
        .iter()
        .enumerate()
        .filter_map(|(index, turn)| {
            let position = turns.get(offset + index)?.position;
            (position > covered).then(|| (position, turn.clone()))
        })
        .collect();
    if fallen.len() > TURNS_BEFORE_REFRESH && ai_access::consume_ai_quota(state, subject).is_ok() {
        match state
            .core
            .summarize_conversation(provider_id, workspace_id, summary.as_ref(), &fallen)
            .await
        {
            Ok(fresh) => {
                if let Err(error) = state.storage.set_agent_summary(conversation_id, Some(&fresh)).await {
                    tracing::warn!(error = %format!("{error:#}"), "A CAIRN chat summary could not be stored");
                }
                summary = Some(fresh);
            }
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "A CAIRN chat summary could not be written");
            }
        }
    }
    let kept = history[start..].to_vec();
    (kept, summary.filter(|_| start > 0), Some(budget))
}

fn history_of(turns: &[AgentTurn]) -> Vec<AgentHistoryTurn> {
    let start = turns.len().saturating_sub(MAX_TURNS_READ);
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
            // What the person confirmed is part of the conversation, so "now summarize what you moved" can follow from it (V0.9.44).
            let mut answer = turn.reply.reply_markdown.clone();
            for proposal in &turn.reply.proposals {
                if proposal.state == repomemo_domain::AgentProposalState::Applied {
                    if let Some(outcome) = &proposal.outcome {
                        answer.push_str(&format!("\n[Done: {outcome}]"));
                    }
                }
            }
            AgentHistoryTurn {
                question: turn.label.clone(),
                answer,
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

/// The text model that serves a workspace (V0.9.84): its own provider while the organization offers it, else the organization's
/// default, else none. See `StorageEngine::resolve_text_provider`.
pub(crate) async fn text_resolution(
    state: &AppState,
    workspace_id: &str,
) -> Result<repomemo_storage::TextResolution, ApiError> {
    state
        .storage
        .resolve_text_provider(workspace_id)
        .await
        .map_err(map_storage_error)
}

/// Tells an organization's owners and admins that its monthly CAIRN token budget reached a threshold (V0.9.97).
async fn notify_budget(state: &AppState, budget: &repomemo_storage::OrganizationBudget, threshold: i64) {
    let Ok(members) = state.storage.list_organization_members(&budget.organization_id).await else {
        return;
    };
    for member in members.into_iter().filter(|member| {
        matches!(
            member.role,
            repomemo_domain::OrganizationRole::Owner | repomemo_domain::OrganizationRole::Admin
        )
    }) {
        let _ = state
            .storage
            .create_shared_notification(repomemo_storage::NewSharedNotification {
                user_id: member.user.id,
                workspace_id: None,
                notification_type: "cairn_budget".to_owned(),
                title: format!("CAIRN has used {threshold}% of this month's token budget"),
                body: format!(
                    "{} of {} tokens used. At the cap CAIRN {}.",
                    budget.used,
                    budget.limit,
                    if budget.at_budget == "local_only" {
                        "keeps to models on your network"
                    } else {
                        "stops until next month"
                    }
                ),
                href: "/cairn/admin/organizations".to_owned(),
            })
            .await;
    }
}

/// A message that starts with a `/command`: the command, lower case with its slash, and what follows it.
fn split_command(message: &str) -> Option<(String, &str)> {
    let trimmed = message.trim_start();
    if !trimmed.starts_with('/') {
        return None;
    }
    let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    Some((trimmed[..end].to_ascii_lowercase(), trimmed[end..].trim()))
}

/// The fields an ability asks that the message leaves empty. A line `Label: value` (or `name: value`) fills a field; one field also
/// takes the words after the command as they are.
fn missing_inputs(rest: &str, inputs: &[repomemo_storage::AbilityInput]) -> Vec<String> {
    if inputs.len() == 1 && !rest.trim().is_empty() {
        return Vec::new();
    }
    inputs
        .iter()
        .filter(|input| {
            !rest.lines().any(|line| {
                let Some((key, value)) = line.split_once(':') else {
                    return false;
                };
                let key = key.trim();
                (key.eq_ignore_ascii_case(&input.label) || key.eq_ignore_ascii_case(&input.name))
                    && !value.trim().is_empty()
            })
        })
        .map(|input| input.label.clone())
        .collect()
}

/// The reply to a job that was turned off, naming who turned it off (V0.9.91); none when it is on.
fn switched_off_reply(
    tools_off: &[(String, &'static str)],
    capability: AgentCapability,
) -> Option<repomemo_domain::AgentReply> {
    let id = crate::cairn_tools::job_id(capability);
    let (_, by) = tools_off.iter().find(|(name, _)| *name == id)?;
    let who = if *by == "organization" {
        "Your organization turned off"
    } else {
        "This workspace's administrators turned off"
    };
    let mut reply = repomemo_api::lookup_reply(format!(
        "{who} “{}”, so I cannot do that here.",
        crate::cairn_tools::label_of(&id)
    ));
    reply.capability = Some(capability);
    Some(reply)
}

/// The workspace role as the model limits name it (V0.9.87).
pub(crate) fn role_name(role: &WorkspaceRole) -> &'static str {
    match role {
        WorkspaceRole::Owner => "owner",
        WorkspaceRole::Admin => "admin",
        WorkspaceRole::Member => "member",
        WorkspaceRole::Viewer => "viewer",
    }
}

/// The text model for a person with a role: one limited to other roles is not used.
pub(crate) async fn text_provider_for_role(
    state: &AppState,
    workspace_id: &str,
    role: &WorkspaceRole,
) -> Result<Option<ProviderSettings>, ApiError> {
    Ok(state
        .storage
        .resolve_text_provider_as(workspace_id, Some(role_name(role)))
        .await
        .map_err(map_storage_error)?
        .resolved
        .map(|resolved| resolved.provider))
}

pub(crate) async fn text_provider(state: &AppState, workspace_id: &str) -> Result<Option<ProviderSettings>, ApiError> {
    Ok(text_resolution(state, workspace_id)
        .await?
        .resolved
        .map(|resolved| resolved.provider))
}

#[cfg(test)]
mod ability_tests {
    use super::*;
    use repomemo_storage::AbilityInput;

    fn field(name: &str, label: &str) -> AbilityInput {
        AbilityInput {
            name: name.to_owned(),
            label: label.to_owned(),
        }
    }

    #[test]
    fn a_message_that_starts_with_a_slash_is_a_command_and_its_rest() {
        assert_eq!(
            split_command("  /Weekly-Status\nTeam: Blue"),
            Some(("/weekly-status".to_owned(), "Team: Blue"))
        );
        assert_eq!(split_command("/status"), Some(("/status".to_owned(), "")));
        assert_eq!(split_command("what is /status"), None);
    }

    #[test]
    fn a_field_is_filled_by_its_label_or_name_and_a_single_field_takes_the_words_after_the_command() {
        let team = [field("team", "Team"), field("week", "Week")];
        assert_eq!(missing_inputs("Team: Blue\nweek: 12", &team), Vec::<String>::new());
        assert_eq!(missing_inputs("Team: Blue\nWeek: ", &team), vec!["Week".to_owned()]);
        assert_eq!(missing_inputs("", &team), vec!["Team".to_owned(), "Week".to_owned()]);
        assert!(missing_inputs("the blue team", &[field("team", "Team")]).is_empty());
        assert!(missing_inputs("", &[]).is_empty());
    }
}
