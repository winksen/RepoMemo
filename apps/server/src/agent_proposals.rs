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
    artifacts, map_core_error, map_storage_error,
    requests::{
        BulkArtifactsRequest, CreateMemoryCardBody, CreateTextArtifactRequest, UpdateArtifactRequest,
        UpdateMemoryCardBody,
    },
    require_workspace_write, retrieval, ApiError, AppState, AuthenticatedSubject,
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

/// Offers to keep how an answer was reached as a playbook (V0.9.55): the run behind the turn must have at least three tool steps that
/// worked. The proposal joins the turn's other proposals, waits for the person's confirmation like any change, and makes a note that
/// no run uses until a writer verifies it.
pub(crate) async fn offer_agent_playbook(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(turn_id): Path<String>,
) -> Result<Json<AgentTurn>, ApiError> {
    let (mut turn, workspace_id) = owned_turn(&state, &subject, &turn_id).await?;
    require_workspace_write(&state, &subject, &workspace_id).await?;
    if turn
        .reply
        .proposals
        .iter()
        .any(|proposal| proposal.kind == AgentProposalKind::SavePlaybook)
        || turn.reply.changeset.is_some()
    {
        return Err(ApiError::conflict(
            "This answer already has changes waiting or applied. Settle them before saving it as a playbook.",
        ));
    }
    let run = state
        .storage
        .get_agent_run_for_turn(&turn.id)
        .await
        .map_err(map_storage_error)?
        .ok_or_else(|| ApiError::not_found("CAIRN kept no record of the steps behind that answer."))?;
    let draft = repomemo_api::playbook_from_run(&turn.label, &run.id, &run.steps).ok_or_else(|| {
        ApiError::bad_request("That answer took fewer than three working steps, so there is no procedure to keep.")
    })?;
    turn.reply.proposals.push(AgentProposal {
        id: "p1".to_owned(),
        kind: AgentProposalKind::SavePlaybook,
        state: AgentProposalState::Pending,
        title: format!("Save “{}” as a playbook", draft.name),
        detail: format!(
            "A note in the Playbooks folder with the steps of this run, without what it looked at.\n\n{}",
            draft.text
        ),
        payload: serde_json::json!({ "file_name": draft.file_name, "text": draft.text, "run_id": run.id }),
        outcome: None,
        untrusted_sources: Vec::new(),
    });
    state
        .storage
        .update_agent_turn_reply(&turn.id, &turn.reply)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(turn))
}

/// What kind of offer a proposal was, when CAIRN made it unasked (V0.9.53): `decision` or `correction`.
fn proposal_offer(turn: &AgentTurn, proposal_id: &str) -> Option<String> {
    turn.reply
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .and_then(|proposal| proposal.payload.get("offer"))
        .and_then(|offer| offer.as_str())
        .map(str::to_owned)
}

/// Dismissing an offer to save a decision stops such offers in that chat, and dismissing the card of a correction closes it.
async fn stop_offers_after(state: &AppState, conversation_id: &str, offer: Option<String>) {
    if offer.as_deref() == Some("decision") {
        let _ = state.storage.stop_decision_offers(conversation_id).await;
    }
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
        AgentProposalKind::MoveFiles | AgentProposalKind::TrashFiles | AgentProposalKind::SetStatus => {
            let body: BulkArtifactsRequest = serde_json::from_value(proposal.payload.clone())
                .map_err(|_| ApiError::bad_request("This proposal can no longer be applied."))?;
            let Json(done) = artifacts::bulk_artifacts(
                subject.clone(),
                State(state.clone()),
                Path(workspace_id.to_owned()),
                Json(body),
            )
            .await?;
            // The files are handled one by one; a file that could not be (a repository file, one already gone) is the answer.
            if let Some(failure) = done.failed.first() {
                return Err(ApiError::bad_request(failure.message.clone()));
            }
            // One file is named; several are counted.
            let title = payload_text(proposal, "title");
            let several = proposal
                .payload
                .get("count")
                .and_then(|count| count.as_u64())
                .unwrap_or(1)
                > 1;
            let subject = if several { title } else { format!("“{title}”") };
            if proposal.kind == AgentProposalKind::SetStatus {
                return Ok(format!(
                    "Marked {subject} as {}.",
                    payload_text(proposal, "status_label")
                ));
            }
            Ok(if proposal.kind == AgentProposalKind::TrashFiles {
                format!(
                    "Moved {subject} to the trash. The workspace's trash can restore {}.",
                    if several { "them" } else { "it" }
                )
            } else {
                format!("Moved {subject} to {}.", payload_text(proposal, "folder_name"))
            })
        }
        AgentProposalKind::SavePersonalMemory => {
            // Only the person's own memory changes, so no role is needed beyond reading the chat they confirm it in (V0.9.51).
            let text = payload_text(proposal, "text");
            let policy = state
                .storage
                .personal_memory_policy_for_workspace(workspace_id)
                .await
                .map_err(map_storage_error)?;
            if !policy.enabled {
                return Err(ApiError::bad_request(policy.refusal()));
            }
            state
                .storage
                .add_personal_memory(&subject.user_id, None, &text, None)
                .await
                .map_err(map_storage_error)?;
            Ok(format!("I will remember: {text}"))
        }
        AgentProposalKind::CreateTask => {
            let body: crate::requests::SaveCollaborationTaskRequest = serde_json::from_value(proposal.payload.clone())
                .map_err(|_| ApiError::bad_request("This proposal can no longer be applied."))?;
            let (_, Json(task)) = crate::collaboration::create_collaboration_task(
                subject.clone(),
                State(state.clone()),
                Path(workspace_id.to_owned()),
                Json(body),
            )
            .await?;
            Ok(format!("Created the task {} “{}”.", task.key, task.title))
        }
        AgentProposalKind::CommentTask | AgentProposalKind::SetTaskStatus => {
            let task_id = payload_text(proposal, "task_id");
            let task = state
                .storage
                .get_collaboration_task(&task_id)
                .await
                .map_err(map_storage_error)?;
            // Only against the task as CAIRN read it: a task someone changed since is left alone.
            let read_version = proposal
                .payload
                .get("expected_version")
                .and_then(|version| version.as_i64());
            if task.workspace_id != workspace_id || read_version != Some(task.version) {
                return Err(ApiError::conflict(format!(
                    "{} changed since CAIRN read it. Ask again to work from its current state.",
                    task.key
                )));
            }
            if proposal.kind == AgentProposalKind::CommentTask {
                let _ = crate::task_discussion::add_task_comment(
                    subject.clone(),
                    State(state.clone()),
                    Path(task_id),
                    Json(crate::requests::SaveTaskCommentRequest {
                        body: payload_text(proposal, "body"),
                    }),
                )
                .await?;
                Ok(format!("Commented on {}.", task.key))
            } else {
                let status = payload_text(proposal, "status");
                let request = crate::requests::SaveCollaborationTaskRequest {
                    title: task.title.clone(),
                    description: task.description.clone(),
                    status: status.clone(),
                    state_id: None,
                    priority: task.priority.clone(),
                    assignee_user_id: task.assignee.as_ref().map(|member| member.id.clone()),
                    artifact_id: task.artifact_id.clone(),
                    project_id: task.project_id.clone(),
                    milestone_id: task.milestone_id.clone(),
                    parent_task_id: task.parent_task_id.clone(),
                    expected_version: Some(task.version),
                    references: Vec::new(),
                    due_at: task.due_at.clone(),
                };
                match crate::collaboration::update_collaboration_task(
                    subject.clone(),
                    State(state.clone()),
                    Path(task_id),
                    Json(request),
                )
                .await
                {
                    Ok(_) => Ok(format!("Set {} to {}.", task.key, status.replace('_', " "))),
                    Err(crate::collaboration::TaskSaveError::Api(error)) => Err(error),
                    Err(crate::collaboration::TaskSaveError::Stale(_)) => Err(ApiError::conflict(format!(
                        "{} changed while it was being saved. Ask again.",
                        task.key
                    ))),
                }
            }
        }
        AgentProposalKind::SavePlaybook => {
            let (file_name, text) = (payload_text(proposal, "file_name"), payload_text(proposal, "text"));
            let folders = state
                .storage
                .list_folders(workspace_id)
                .await
                .map_err(map_storage_error)?;
            let folder = match folders
                .into_iter()
                .find(|folder| folder.parent_id.is_none() && folder.name.eq_ignore_ascii_case("Playbooks"))
            {
                Some(folder) => folder,
                None => state
                    .storage
                    .create_folder(workspace_id, None, "Playbooks", Some(&subject.user_id))
                    .await
                    .map_err(map_storage_error)?,
            };
            let note = state
                .core
                .import_text(
                    workspace_id.to_owned(),
                    file_name.clone(),
                    text,
                    Some("Note".to_owned()),
                )
                .await
                .map_err(map_core_error)?;
            state
                .storage
                .move_artifact_to_folder(&note.id, Some(&folder.id))
                .await
                .map_err(map_storage_error)?;
            if let Err(error) = state.core.index_artifact(note.id).await {
                tracing::warn!(error = %format!("{error:#}"), "A saved playbook could not be indexed");
            }
            Ok(format!(
                "Saved {file_name} in the Playbooks folder. CAIRN does not use it until a writer has read and verified it."
            ))
        }
        AgentProposalKind::SaveSummary | AgentProposalKind::SaveNote => {
            // The same handler as a note written by hand: its role check, its quota and the queue that indexes it.
            let body: CreateTextArtifactRequest = serde_json::from_value(proposal.payload.clone())
                .map_err(|_| ApiError::bad_request("This proposal can no longer be applied."))?;
            let (_, Json(note)) = artifacts::create_text_artifact(
                subject.clone(),
                State(state.clone()),
                Path(workspace_id.to_owned()),
                Json(body),
            )
            .await?;
            Ok(format!(
                "Saved the note “{}”. It is being indexed, so CAIRN and search can find it soon.",
                note.summary.title
            ))
        }
        AgentProposalKind::UpdateMemory | AgentProposalKind::DeleteMemory => {
            let card_id = payload_text(proposal, "card_id");
            // The card must be in this chat's workspace; the handler then checks the person's role there.
            let card = state
                .core
                .get_memory_card(card_id.clone())
                .await
                .map_err(map_core_error)?;
            if card.card.workspace_id != workspace_id {
                return Err(ApiError::not_found("That memory card was not found."));
            }
            if proposal.kind == AgentProposalKind::DeleteMemory {
                retrieval::delete_memory_card(subject.clone(), State(state.clone()), Path(card_id)).await?;
                return Ok(format!("Deleted the memory card “{}”.", card.card.title));
            }
            // The revision it started from goes along, so a card saved by someone else since is not overwritten.
            let body: UpdateMemoryCardBody = serde_json::from_value(proposal.payload.clone())
                .map_err(|_| ApiError::bad_request("This proposal can no longer be applied."))?;
            let saved =
                retrieval::update_memory_card(subject.clone(), State(state.clone()), Path(card_id), Json(body)).await?;
            Ok(format!("Changed the memory card “{}”.", saved.0.title))
        }
        AgentProposalKind::RenameFile => {
            let artifact_id = payload_text(proposal, "artifact_id");
            let title = payload_text(proposal, "title");
            let current = state
                .storage
                .get_artifact_summary(&artifact_id)
                .await
                .map_err(|_| ApiError::not_found("That file was not found."))?;
            if current.workspace_id != workspace_id {
                return Err(ApiError::not_found("That file was not found."));
            }
            let _ = artifacts::update_artifact(
                subject.clone(),
                State(state.clone()),
                Path(artifact_id),
                Json(UpdateArtifactRequest { title: title.clone() }),
            )
            .await?;
            Ok(format!("Renamed “{}” to “{title}”.", payload_text(proposal, "was")))
        }
    }
}

/// A text the proposal's payload carries, or nothing.
fn payload_text(proposal: &AgentProposal, key: &str) -> String {
    proposal
        .payload
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_owned()
}

/// Refuses a confirmation that would change more than the cap allows, naming who set the cap.
fn refuse_over_cap(limits: &repomemo_storage::ChangeLimits, changes: i64) -> Result<(), ApiError> {
    if changes <= limits.max_changes {
        return Ok(());
    }
    let who = match limits.set_by {
        "organization" => "your organization allows",
        "workspace" => "this workspace allows",
        _ => "CAIRN makes",
    };
    Err(ApiError::bad_request(format!(
        "This would make {changes} changes in one confirmation; {who} at most {}. Confirm fewer at a time.",
        limits.max_changes
    )))
}

/// What is said when the number of changes must be typed to confirm.
fn type_the_number(proposal: &AgentProposal, count: i64) -> String {
    if proposal.untrusted_sources.is_empty() {
        format!("This changes {count} things at once. Type the number, {count}, to confirm.")
    } else {
        format!(
            "This was proposed after reading content from outside the workspace ({}). Type the number of files, {count}, to confirm.",
            proposal.untrusted_sources.join(", ")
        )
    }
}

/// What the person typed to confirm a large change CAIRN proposed after reading content it does not trust (V0.9.42).
#[derive(Debug, Default, serde::Deserialize)]
pub(crate) struct ConfirmProposalRequest {
    #[serde(default)]
    confirm_count: Option<i64>,
}

/// The most files a proposal made after untrusted reading may move or trash without the person typing how many.
const UNTRUSTED_BULK_LIMIT: i64 = 10;

/// Whether a proposal must be confirmed by typing its count: it moves or trashes more than ten files and was made after CAIRN
/// read content it does not trust. Returns that count.
#[cfg(test)]
fn needs_typed_count(proposal: &AgentProposal) -> Option<i64> {
    needs_typed_count_with(proposal, None)
}

/// How many changes a proposal makes: the files it moves, trashes or marks, and one for anything else (V0.9.92).
fn change_count(proposal: &AgentProposal) -> i64 {
    match proposal.kind {
        AgentProposalKind::MoveFiles | AgentProposalKind::TrashFiles | AgentProposalKind::SetStatus => proposal
            .payload
            .get("count")
            .and_then(|count| count.as_i64())
            .unwrap_or(1)
            .max(1),
        _ => 1,
    }
}

/// [`needs_typed_count`] with the count from which an organization or workspace asks the number to be typed for any proposal
/// (V0.9.92), besides the rule for what CAIRN read from outside the workspace.
fn needs_typed_count_with(proposal: &AgentProposal, typed_from: Option<i64>) -> Option<i64> {
    if let Some(from) = typed_from {
        let count = change_count(proposal);
        if count > from {
            return Some(count);
        }
    }
    if proposal.untrusted_sources.is_empty()
        || !matches!(
            proposal.kind,
            AgentProposalKind::MoveFiles | AgentProposalKind::TrashFiles
        )
    {
        return None;
    }
    let count = proposal.payload.get("count").and_then(|count| count.as_i64())?;
    (count > UNTRUSTED_BULK_LIMIT).then_some(count)
}

pub(crate) async fn confirm_agent_proposal(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((turn_id, proposal_id)): Path<(String, String)>,
    body: Option<Json<ConfirmProposalRequest>>,
) -> Result<Json<AgentTurn>, ApiError> {
    let (mut turn, workspace_id) = owned_turn(&state, &subject, &turn_id).await?;
    if turn.reply.changeset.is_some() {
        return Err(ApiError::bad_request(
            "These changes are confirmed together: use the set of changes.",
        ));
    }
    // Whatever the action, the person must still be able to change the workspace, except to be remembered (V0.9.51).
    if turn
        .reply
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .is_none_or(|proposal| proposal.kind != AgentProposalKind::SavePersonalMemory)
    {
        require_workspace_write(&state, &subject, &workspace_id).await?;
    }
    let limits = state
        .storage
        .change_limits_for_workspace(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let proposal = pending_proposal(&mut turn, &proposal_id)?;
    refuse_over_cap(&limits, change_count(proposal))?;
    if let Some(count) = needs_typed_count_with(proposal, limits.type_to_confirm_from) {
        let typed = body.and_then(|Json(request)| request.confirm_count);
        if typed != Some(count) {
            return Err(ApiError::bad_request(type_the_number(proposal, count)));
        }
    }
    let settles_correction = proposal.payload.get("offer").and_then(|offer| offer.as_str()) == Some("correction");
    let outcome = apply(&state, &subject, &workspace_id, proposal).await?;
    proposal.state = AgentProposalState::Applied;
    proposal.outcome = Some(outcome);
    // A correction whose fact became a memory card is reviewed (V0.9.53).
    if settles_correction {
        let _ = state.storage.settle_agent_corrections(&turn.id, "reviewed").await;
    }
    state
        .storage
        .update_agent_turn_reply(&turn.id, &turn.reply)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(turn))
}

/// The ticked actions of a changeset (V0.9.44), and the count the person typed for each large change proposed after reading
/// content from outside the workspace.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct ConfirmChangesetRequest {
    proposal_ids: Vec<String>,
    #[serde(default)]
    confirm_counts: std::collections::HashMap<String, i64>,
}

fn changeset_pending(turn: &AgentTurn) -> Result<(), ApiError> {
    match turn.reply.changeset.as_ref() {
        None => Err(ApiError::not_found("That message has no set of changes.")),
        Some(set) if set.state != "pending" => Err(ApiError::conflict("That set of changes was already answered.")),
        Some(_) => Ok(()),
    }
}

/// Applies the ticked actions of a changeset in the order CAIRN proposed them, as the person, and stops at the first one that
/// fails. The set is answered once: a second confirmation is refused. Actions not ticked, or after a failure, are not applied.
pub(crate) async fn confirm_agent_changeset(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(turn_id): Path<String>,
    Json(request): Json<ConfirmChangesetRequest>,
) -> Result<Json<AgentTurn>, ApiError> {
    let (mut turn, workspace_id) = owned_turn(&state, &subject, &turn_id).await?;
    // A set of changes to oneself needs no write role; any change to the workspace does.
    let only_personal = turn
        .reply
        .proposals
        .iter()
        .filter(|proposal| request.proposal_ids.contains(&proposal.id))
        .all(|proposal| proposal.kind == AgentProposalKind::SavePersonalMemory);
    if !only_personal || request.proposal_ids.is_empty() {
        require_workspace_write(&state, &subject, &workspace_id).await?;
    }
    changeset_pending(&turn)?;
    if request.proposal_ids.is_empty() {
        return Err(ApiError::bad_request("Tick at least one change to apply."));
    }
    for id in &request.proposal_ids {
        if !turn.reply.proposals.iter().any(|proposal| &proposal.id == id) {
            return Err(ApiError::bad_request(
                "One of the ticked changes is not part of this set.",
            ));
        }
    }
    // The whole set may not change more than one confirmation is allowed to (V0.9.92), and nothing is applied until every large
    // change has its count typed.
    let limits = state
        .storage
        .change_limits_for_workspace(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let ticked = turn
        .reply
        .proposals
        .iter()
        .filter(|proposal| request.proposal_ids.contains(&proposal.id));
    refuse_over_cap(&limits, ticked.clone().map(change_count).sum())?;
    for proposal in ticked {
        if let Some(count) = needs_typed_count_with(proposal, limits.type_to_confirm_from) {
            if request.confirm_counts.get(&proposal.id) != Some(&count) {
                return Err(ApiError::bad_request(format!(
                    "\"{}\": {}",
                    proposal.title,
                    type_the_number(proposal, count)
                )));
            }
        }
    }
    let mut applied = Vec::new();
    let mut failure: Option<(String, String)> = None;
    for index in 0..turn.reply.proposals.len() {
        let ticked = request.proposal_ids.contains(&turn.reply.proposals[index].id);
        if !ticked {
            turn.reply.proposals[index].state = AgentProposalState::Dismissed;
            continue;
        }
        if failure.is_some() {
            turn.reply.proposals[index].state = AgentProposalState::Dismissed;
            turn.reply.proposals[index].outcome = Some("Not applied: an earlier action failed.".to_owned());
            continue;
        }
        let result = apply(&state, &subject, &workspace_id, &turn.reply.proposals[index]).await;
        let proposal = &mut turn.reply.proposals[index];
        match result {
            Ok(outcome) => {
                proposal.state = AgentProposalState::Applied;
                applied.push(outcome.clone());
                proposal.outcome = Some(outcome);
            }
            Err(error) => {
                proposal.state = AgentProposalState::Dismissed;
                proposal.outcome = Some(format!("Not applied: {}", error.message));
                failure = Some((proposal.title.clone(), error.message));
            }
        }
    }
    let outcome = match &failure {
        None => format!("Applied {} of {} changes.", applied.len(), turn.reply.proposals.len()),
        Some((title, message)) => format!(
            "Applied {}. Stopped at \"{title}\": {message}",
            if applied.is_empty() {
                "none of the changes".to_owned()
            } else {
                format!(
                    "{} {}",
                    applied.len(),
                    if applied.len() == 1 { "change" } else { "changes" }
                )
            }
        ),
    };
    if let Some(set) = turn.reply.changeset.as_mut() {
        set.state = if failure.is_none() { "applied" } else { "partial" }.to_owned();
        set.outcome = Some(outcome);
    }
    state
        .storage
        .update_agent_turn_reply(&turn.id, &turn.reply)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(turn))
}

/// Dismisses every action of a changeset.
pub(crate) async fn dismiss_agent_changeset(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(turn_id): Path<String>,
) -> Result<Json<AgentTurn>, ApiError> {
    let (mut turn, _) = owned_turn(&state, &subject, &turn_id).await?;
    changeset_pending(&turn)?;
    for proposal in &mut turn.reply.proposals {
        if proposal.state == AgentProposalState::Pending {
            proposal.state = AgentProposalState::Dismissed;
        }
    }
    let dismissed_offer = turn
        .reply
        .proposals
        .iter()
        .any(|proposal| {
            proposal.state == AgentProposalState::Dismissed
                && proposal.payload.get("offer").and_then(|offer| offer.as_str()) == Some("decision")
        })
        .then(|| "decision".to_owned());
    stop_offers_after(&state, &turn.conversation_id, dismissed_offer).await;
    if let Some(set) = turn.reply.changeset.as_mut() {
        set.state = "dismissed".to_owned();
        set.outcome = Some("Dismissed. Nothing was changed.".to_owned());
    }
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
    stop_offers_after(&state, &turn.conversation_id, proposal_offer(&turn, &proposal_id)).await;
    state
        .storage
        .update_agent_turn_reply(&turn.id, &turn.reply)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(turn))
}

#[cfg(test)]
mod tests {
    use repomemo_domain::{AgentChangeset, AgentReply, AgentRouting, AgentTurn};
    use serde_json::json;

    use super::*;

    fn proposal(id: &str, kind: AgentProposalKind, count: i64, untrusted: &[&str]) -> AgentProposal {
        AgentProposal {
            id: id.to_owned(),
            kind,
            state: AgentProposalState::Pending,
            title: format!("Change {id}"),
            detail: String::new(),
            payload: json!({ "count": count }),
            outcome: None,
            untrusted_sources: untrusted.iter().map(|source| (*source).to_owned()).collect(),
        }
    }

    fn turn_with(changeset: Option<&str>) -> AgentTurn {
        AgentTurn {
            id: "t1".to_owned(),
            conversation_id: "c1".to_owned(),
            position: 1,
            label: "Tidy up".to_owned(),
            request: Default::default(),
            reply: AgentReply {
                capability: None,
                routing: AgentRouting::Agent,
                subject: String::new(),
                reply_markdown: String::new(),
                generated: true,
                files: Vec::new(),
                matches: Vec::new(),
                citations: Vec::new(),
                warnings: Vec::new(),
                context_turns: 0,
                proposals: Vec::new(),
                file_set: None,
                memory_cards: Vec::new(),
                changeset: changeset.map(|state| AgentChangeset {
                    state: state.to_owned(),
                    outcome: None,
                }),
                trust: None,
                ai_unavailable_reason: None,
            },
            created_at: String::new(),
        }
    }

    #[test]
    fn a_large_move_or_trash_after_untrusted_reading_needs_its_count_typed() {
        let issue = ["Login fails (an issue)"];
        assert_eq!(
            needs_typed_count(&proposal("p1", AgentProposalKind::TrashFiles, 11, &issue)),
            Some(11)
        );
        assert_eq!(
            needs_typed_count(&proposal("p1", AgentProposalKind::MoveFiles, 40, &issue)),
            Some(40)
        );
        assert_eq!(
            needs_typed_count(&proposal("p1", AgentProposalKind::TrashFiles, 10, &issue)),
            None
        );
        assert_eq!(
            needs_typed_count(&proposal("p1", AgentProposalKind::TrashFiles, 50, &[])),
            None
        );
        assert_eq!(
            needs_typed_count(&proposal("p1", AgentProposalKind::SaveMemory, 50, &issue)),
            None
        );
    }

    #[test]
    fn a_set_of_changes_is_answered_once() {
        assert!(changeset_pending(&turn_with(Some("pending"))).is_ok());
        for answered in ["applied", "partial", "dismissed"] {
            let error = changeset_pending(&turn_with(Some(answered))).unwrap_err();
            assert_eq!(error.status, axum::http::StatusCode::CONFLICT, "{answered}");
        }
        let none = changeset_pending(&turn_with(None)).unwrap_err();
        assert_eq!(none.status, axum::http::StatusCode::NOT_FOUND);
    }

    #[test]
    fn a_confirmation_over_the_cap_is_refused_with_who_set_it_and_the_number_is_typed_from_the_set_count() {
        let limits = |max_changes, set_by| repomemo_storage::ChangeLimits {
            max_changes,
            type_to_confirm_from: None,
            set_by,
        };
        assert!(refuse_over_cap(&limits(200, "default"), 200).is_ok());
        let by_organization = refuse_over_cap(&limits(50, "organization"), 51).unwrap_err().message;
        assert!(
            by_organization.contains("your organization allows at most 50"),
            "{by_organization}"
        );
        let by_workspace = refuse_over_cap(&limits(20, "workspace"), 21).unwrap_err().message;
        assert!(
            by_workspace.contains("this workspace allows at most 20"),
            "{by_workspace}"
        );

        // From the set count the number is typed for any proposal, trusted or not; below it, only the old rule applies.
        let trusted = proposal("p1", AgentProposalKind::TrashFiles, 8, &[]);
        assert_eq!(needs_typed_count_with(&trusted, Some(5)), Some(8));
        assert_eq!(
            needs_typed_count_with(&trusted, Some(8)),
            None,
            "at the count, not above it"
        );
        assert_eq!(needs_typed_count_with(&trusted, None), None);
        let outside = proposal("p2", AgentProposalKind::TrashFiles, 11, &["an issue"]);
        assert_eq!(
            needs_typed_count_with(&outside, Some(50)),
            Some(11),
            "the rule for outside content still applies"
        );
        // A change that is not about files counts as one.
        assert_eq!(change_count(&proposal("p3", AgentProposalKind::SaveMemory, 40, &[])), 1);
        assert!(type_the_number(&trusted, 8).contains("Type the number, 8"));
    }
}
