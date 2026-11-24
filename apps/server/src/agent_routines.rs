//! CAIRN routines (V0.9.63): a prompt saved to run on a schedule, as its owner, with read tools only. The owner makes, pauses, resumes
//! and deletes it (a workspace administrator may too); the maintenance pass runs the ones that are due. A run is posted into one chat
//! named after the routine and tells the owner. A routine pauses when its owner can no longer change the workspace, when the
//! workspace has no AI provider, and after three failed runs in a row.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_domain::{AgentRoutine, RecurringSchedule, WorkspaceRole};
use repomemo_storage::{NewRoutine, NewSharedNotification};
use serde::Deserialize;

use crate::{
    map_storage_error, profile::create_notification, require_workspace_admin, require_workspace_read,
    require_workspace_write, ApiError, AppState, AuthenticatedSubject,
};

#[derive(Debug, Deserialize)]
pub(crate) struct RoutineRequest {
    title: String,
    prompt: String,
    schedule: RecurringSchedule,
}

pub(crate) async fn list_routines(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<AgentRoutine>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    state
        .storage
        .list_agent_routines(&workspace_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn create_routine(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<RoutineRequest>,
) -> Result<(StatusCode, Json<AgentRoutine>), ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    state
        .storage
        .create_agent_routine(NewRoutine {
            workspace_id,
            owner_id: subject.user_id.clone(),
            title: request.title,
            prompt: request.prompt,
            schedule: request.schedule,
        })
        .await
        .map(|routine| (StatusCode::CREATED, Json(routine)))
        .map_err(map_storage_error)
}

/// The routine, when the caller owns it or administers its workspace.
async fn managed_routine(
    state: &AppState,
    subject: &AuthenticatedSubject,
    workspace_id: &str,
    routine_id: &str,
) -> Result<AgentRoutine, ApiError> {
    let routine = state
        .storage
        .get_agent_routine(routine_id)
        .await
        .map_err(map_storage_error)?;
    if routine.workspace_id != workspace_id {
        return Err(ApiError::not_found("That routine was not found."));
    }
    if routine.owner_id != subject.user_id {
        require_workspace_admin(state, subject, workspace_id).await?;
    } else {
        require_workspace_read(state, subject, workspace_id).await?;
    }
    Ok(routine)
}

pub(crate) async fn delete_routine(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, routine_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    managed_routine(&state, &subject, &workspace_id, &routine_id).await?;
    state
        .storage
        .delete_agent_routine(&routine_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn pause_routine(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, routine_id)): Path<(String, String)>,
) -> Result<Json<AgentRoutine>, ApiError> {
    managed_routine(&state, &subject, &workspace_id, &routine_id).await?;
    state
        .storage
        .pause_agent_routine(&routine_id, "Paused by a person.")
        .await
        .map_err(map_storage_error)?;
    state
        .storage
        .get_agent_routine(&routine_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn resume_routine(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, routine_id)): Path<(String, String)>,
) -> Result<Json<AgentRoutine>, ApiError> {
    managed_routine(&state, &subject, &workspace_id, &routine_id).await?;
    state
        .storage
        .resume_agent_routine(&routine_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

/// Runs a routine now, for the owner, without waiting for its date.
pub(crate) async fn run_routine_now(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, routine_id)): Path<(String, String)>,
) -> Result<Json<AgentRoutine>, ApiError> {
    let routine = managed_routine(&state, &subject, &workspace_id, &routine_id).await?;
    if routine.owner_id != subject.user_id {
        return Err(ApiError::forbidden_because(
            "Only the owner of a routine can run it now.",
        ));
    }
    Ok(Json(run_routine(&state, &routine).await))
}

/// Why a routine may not run: its owner can no longer change the workspace, or the workspace has no AI provider.
async fn reason_not_to_run(state: &AppState, routine: &AgentRoutine) -> Option<String> {
    match state
        .storage
        .workspace_role_for_user(&routine.owner_id, &routine.workspace_id)
        .await
    {
        Ok(Some(role)) if !matches!(role, WorkspaceRole::Viewer) => {}
        _ => return Some("Its owner can no longer change this workspace.".to_owned()),
    }
    match state.storage.list_provider_settings(&routine.workspace_id).await {
        Ok(providers)
            if providers
                .iter()
                .any(|setting| setting.enabled && setting.purpose() == "text") =>
        {
            None
        }
        _ => Some("The workspace has no AI provider turned on.".to_owned()),
    }
}

/// Runs one routine as its owner, with read tools only, into its own chat, and tells the owner. Returns the routine as it is after.
pub(crate) async fn run_routine(state: &AppState, routine: &AgentRoutine) -> AgentRoutine {
    if let Some(reason) = reason_not_to_run(state, routine).await {
        let _ = state.storage.pause_agent_routine(&routine.id, &reason).await;
        notify(
            state,
            routine,
            "A CAIRN routine was paused",
            &format!("{}: {reason}", routine.title),
            None,
        )
        .await;
        return state
            .storage
            .get_agent_routine(&routine.id)
            .await
            .unwrap_or_else(|_| routine.clone());
    }
    let subject = AuthenticatedSubject {
        user_id: routine.owner_id.clone(),
        expires_at: u64::try_from(chrono::Utc::now().timestamp()).unwrap_or(0) + 600,
        session_version: 0,
        is_system_admin: false,
        is_app_admin: false,
        token: None,
    };
    let result = crate::agent::run_routine_message(
        state,
        &subject,
        routine.workspace_id.clone(),
        routine.prompt.clone(),
        routine.conversation_id.clone(),
    )
    .await;
    match result {
        Ok(turn) => {
            let conversation_id = turn.conversation.id.clone();
            if routine.conversation_id.is_none() {
                let _ = state
                    .storage
                    .rename_agent_conversation(&conversation_id, &routine.owner_id, &routine.title)
                    .await;
            }
            let after = state
                .storage
                .record_agent_routine_run(&routine.id, true, Some(&conversation_id), "")
                .await
                .unwrap_or_else(|_| routine.clone());
            notify(
                state,
                routine,
                &routine.title,
                "CAIRN posted the answer of your routine.",
                Some(format!("/cairn/{conversation_id}")),
            )
            .await;
            after
        }
        Err(error) => {
            let after = state
                .storage
                .record_agent_routine_run(&routine.id, false, None, &error.message)
                .await
                .unwrap_or_else(|_| routine.clone());
            if after.state == "paused" {
                notify(
                    state,
                    routine,
                    "A CAIRN routine was paused",
                    &format!("{}: {}", routine.title, after.pause_reason.clone().unwrap_or_default()),
                    None,
                )
                .await;
            }
            after
        }
    }
}

async fn notify(state: &AppState, routine: &AgentRoutine, title: &str, body: &str, href: Option<String>) {
    create_notification(
        state,
        NewSharedNotification {
            user_id: routine.owner_id.clone(),
            workspace_id: Some(routine.workspace_id.clone()),
            notification_type: "cairn_routine".to_owned(),
            title: title.to_owned(),
            body: body.to_owned(),
            href: href.unwrap_or_else(|| "/cairn".to_owned()),
        },
    )
    .await;
}

/// Runs the routines whose date has come. Returns how many ran.
pub(crate) async fn run_due(state: &AppState) -> anyhow::Result<usize> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let due = state.storage.due_agent_routines(&today).await?;
    for routine in &due {
        run_routine(state, routine).await;
    }
    Ok(due.len())
}
