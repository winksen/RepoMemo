//! The connections of the model catalog (V0.9.76): the servers CAIRN's models run on, kept once for the whole server. CAIRN and system
//! administrators add, change, turn off, test and delete them; app administrators read them. The key is write-only: it is never in a
//! response, in the log or in the audit trail. The same endpoint policy as a workspace's provider applies, and a connection whose
//! address leaves the network is enabled only after the administrator confirms that what CAIRN sends there leaves it.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_storage::{CairnConnection, CairnConnectionChange, CairnSaved, NewCairnConnection};
use serde::{Deserialize, Serialize};

use crate::{
    map_storage_error,
    system::{audit, require_cairn_admin, require_cairn_reader},
    ApiError, AppState, AuthenticatedSubject, ReadingSubject,
};

#[derive(Debug, Deserialize)]
pub(crate) struct ConnectionRequest {
    /// `ollama`, `openrouter` or `openai_compatible`; fixed once the connection is made.
    #[serde(default)]
    kind: String,
    name: String,
    #[serde(default)]
    base_url: Option<String>,
    /// Sent to the server at `base_url` and nowhere else; empty text removes the stored key, none keeps it.
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default = "enabled_by_default")]
    enabled: bool,
    /// The administrator confirms that what CAIRN sends to this address leaves the device and the private network.
    #[serde(default)]
    cloud_content_acknowledged: bool,
    /// The version the caller read; required to change a connection.
    #[serde(default)]
    expected_version: Option<i64>,
}

fn enabled_by_default() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub(crate) struct ConnectionTest {
    connection_id: String,
    #[serde(flatten)]
    report: repomemo_ai::ConnectionReport,
}

const CLOUD_NOTICE: &str =
    "Confirm that what CAIRN sends to this address leaves your network before turning the connection on.";

pub(crate) async fn list_connections(
    ReadingSubject(subject): ReadingSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<CairnConnection>>, ApiError> {
    require_cairn_reader(&subject)?;
    state
        .storage
        .list_cairn_connections()
        .await
        .map(Json)
        .map_err(map_storage_error)
}

pub(crate) async fn create_connection(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<ConnectionRequest>,
) -> Result<(StatusCode, Json<CairnConnection>), ApiError> {
    require_cairn_admin(&subject)?;
    let base_url = repomemo_ai::connection_base_url(&request.kind, request.base_url.as_deref())
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let cloud = repomemo_ai::is_cloud_connection(&request.kind, Some(&base_url));
    let key = request.api_key.as_deref().map(str::trim).filter(|key| !key.is_empty());
    if request.kind == "openrouter" && request.enabled && key.is_none() {
        return Err(ApiError::bad_request("An OpenRouter API key is required."));
    }
    if cloud && request.enabled && !request.cloud_content_acknowledged {
        return Err(ApiError::bad_request(CLOUD_NOTICE));
    }
    let created = state
        .storage
        .create_cairn_connection(
            NewCairnConnection {
                kind: request.kind.clone(),
                name: request.name.clone(),
                base_url: base_url.clone(),
                api_key: key.map(str::to_owned),
                locality: locality(cloud).to_owned(),
                enabled: request.enabled,
            },
            &subject.user_id,
        )
        .await
        .map_err(map_storage_error)?;
    let created = if cloud && request.cloud_content_acknowledged {
        state
            .storage
            .acknowledge_cairn_connection(&created.id, &subject.user_id)
            .await
            .map_err(map_storage_error)?
    } else {
        created
    };
    audit(
        &state,
        &subject,
        "cairn_connection_added",
        format!(
            "Added the {} connection {} at {} ({}, {}{}).",
            created.kind,
            created.name,
            created.base_url,
            created.locality,
            if created.enabled { "on" } else { "off" },
            if created.has_key { ", with a key" } else { "" }
        ),
    )
    .await;
    Ok((StatusCode::CREATED, Json(created)))
}

pub(crate) async fn update_connection(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<ConnectionRequest>,
) -> Result<Json<CairnConnection>, ApiError> {
    require_cairn_admin(&subject)?;
    let expected_version = request
        .expected_version
        .ok_or_else(|| ApiError::bad_request("Say which version you are changing (expected_version)."))?;
    let existing = state
        .storage
        .get_cairn_connection(&id)
        .await
        .map_err(map_storage_error)?;
    let base_url = repomemo_ai::connection_base_url(
        &existing.kind,
        request.base_url.as_deref().or(Some(existing.base_url.as_str())),
    )
    .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let moved = base_url != existing.base_url;
    let cloud = repomemo_ai::is_cloud_connection(&existing.kind, Some(&base_url));
    let supplied_key = request.api_key.as_deref().map(str::trim);
    // A stored key is only ever sent to the address it was entered for.
    if moved && existing.has_key && supplied_key.is_none() {
        return Err(ApiError::bad_request(
            "Enter the API key again: the address changed, and a saved key is only sent to the address it was saved for.",
        ));
    }
    if existing.kind == "openrouter" && request.enabled && supplied_key.is_none() && !existing.has_key {
        return Err(ApiError::bad_request("An OpenRouter API key is required."));
    }
    let acknowledged = request.cloud_content_acknowledged || (existing.cloud_acknowledged_at.is_some() && !moved);
    if cloud && request.enabled && !acknowledged {
        return Err(ApiError::bad_request(CLOUD_NOTICE));
    }
    let saved = state
        .storage
        .update_cairn_connection(
            &id,
            expected_version,
            CairnConnectionChange {
                name: request.name.clone(),
                base_url: base_url.clone(),
                api_key: request.api_key.clone(),
                locality: locality(cloud).to_owned(),
                enabled: request.enabled,
            },
            &subject.user_id,
        )
        .await
        .map_err(map_storage_error)?;
    let saved = match saved {
        CairnSaved::Saved(connection) => connection,
        CairnSaved::Stale(current) => {
            return Err(ApiError::conflict(format!(
                "The connection {} was changed by someone else (now at version {}). Reload it and change it again.",
                current.name, current.version
            )))
        }
    };
    let saved = if cloud && request.cloud_content_acknowledged {
        state
            .storage
            .acknowledge_cairn_connection(&id, &subject.user_id)
            .await
            .map_err(map_storage_error)?
    } else {
        saved
    };
    let mut changes = Vec::new();
    if existing.name != saved.name {
        changes.push(format!("name from {} to {}", existing.name, saved.name));
    }
    if existing.base_url != saved.base_url {
        changes.push(format!("address from {} to {}", existing.base_url, saved.base_url));
    }
    if existing.enabled != saved.enabled {
        changes.push(format!("turned {}", if saved.enabled { "on" } else { "off" }));
    }
    if existing.locality != saved.locality {
        changes.push(format!("locality from {} to {}", existing.locality, saved.locality));
    }
    if supplied_key.is_some() {
        changes.push(
            if supplied_key == Some("") {
                "key removed"
            } else {
                "key replaced"
            }
            .to_owned(),
        );
    }
    audit(
        &state,
        &subject,
        "cairn_connection_changed",
        format!(
            "Changed the connection {}: {}.",
            saved.name,
            if changes.is_empty() {
                "nothing visible".to_owned()
            } else {
                changes.join("; ")
            }
        ),
    )
    .await;
    Ok(Json(saved))
}

pub(crate) async fn delete_connection(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_cairn_admin(&subject)?;
    let existing = state
        .storage
        .get_cairn_connection(&id)
        .await
        .map_err(map_storage_error)?;
    state
        .storage
        .delete_cairn_connection(&id)
        .await
        .map_err(map_storage_error)?;
    audit(
        &state,
        &subject,
        "cairn_connection_deleted",
        format!("Deleted the connection {} at {}.", existing.name, existing.base_url),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}

/// Reaches the connection's server and, with a key, tries it. The report says whether the server answered and took the key; it
/// never holds the key or the server's own words.
pub(crate) async fn test_connection(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ConnectionTest>, ApiError> {
    require_cairn_admin(&subject)?;
    let connection = state
        .storage
        .get_cairn_connection(&id)
        .await
        .map_err(map_storage_error)?;
    let key = state
        .storage
        .cairn_connection_key(&id)
        .await
        .map_err(map_storage_error)?;
    let report = repomemo_ai::probe_connection(&connection.kind, Some(&connection.base_url), key.as_deref())
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    audit(
        &state,
        &subject,
        "cairn_connection_tested",
        format!(
            "Tested the connection {}: {}.",
            connection.name,
            if report.reachable {
                "the server answered"
            } else {
                "no answer"
            }
        ),
    )
    .await;
    Ok(Json(ConnectionTest {
        connection_id: id,
        report,
    }))
}

fn locality(cloud: bool) -> &'static str {
    if cloud {
        "cloud"
    } else {
        "local"
    }
}
