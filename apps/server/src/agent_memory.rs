//! What CAIRN remembers about a person (V0.9.51): the profile's list of their entries, to read, add, edit, delete and switch off.
//! It is personal: every route works on the caller's own entries and nothing else, and the entries are read into the caller's own
//! runs only (see `agent.rs`). CAIRN adds entries by proposing them in a chat, which the person confirms.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_domain::{PersonalMemory, PersonalMemoryEntry};
use serde::Deserialize;

use crate::{map_storage_error, ApiError, AppState, AuthenticatedSubject, ManagingSubject};

#[derive(Debug, Deserialize)]
pub(crate) struct EntryRequest {
    text: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EnabledRequest {
    enabled: bool,
}

pub(crate) async fn get_personal_memory(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<PersonalMemory>, ApiError> {
    state
        .storage
        .personal_memory(&subject.user_id)
        .await
        .map(Json)
        .map_err(map_storage_error)
}

/// `GET /v1/agent/personal-memory/export`: everything CAIRN remembers about the caller, to keep.
pub(crate) async fn export_personal_memory(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let memory = state
        .storage
        .personal_memory(&subject.user_id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(serde_json::json!({
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "enabled": memory.enabled,
        "entries": memory.entries,
    })))
}

/// `DELETE /v1/agent/personal-memory`: deletes everything CAIRN remembers about the caller.
pub(crate) async fn delete_all_personal_memory(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = state
        .storage
        .delete_all_personal_memory(&subject.user_id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

pub(crate) async fn add_personal_memory(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Json(request): Json<EntryRequest>,
) -> Result<(StatusCode, Json<PersonalMemoryEntry>), ApiError> {
    state
        .storage
        .add_personal_memory(&subject.user_id, None, &request.text, None)
        .await
        .map(|entry| (StatusCode::CREATED, Json(entry)))
        .map_err(map_storage_error)
}

pub(crate) async fn update_personal_memory(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(entry_id): Path<String>,
    Json(request): Json<EntryRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .update_personal_memory(&subject.user_id, &entry_id, &request.text)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn delete_personal_memory(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(entry_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .delete_personal_memory(&subject.user_id, &entry_id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn set_personal_memory_enabled(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Json(request): Json<EnabledRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .set_personal_memory_enabled(&subject.user_id, request.enabled)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}
