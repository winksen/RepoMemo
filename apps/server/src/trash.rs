//! Trash for deleted memory cards and uploaded files (V0.6.15): list, restore and purge.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use repomemo_domain::{TrashItem, TrashRestoreResult};

use crate::{
    map_storage_error, record_workspace_activity, require_workspace_admin, require_workspace_write, ApiError, AppState,
    AuthenticatedSubject,
};

/// What can be put back. Anyone who can edit the workspace sees it; it holds only what they could have deleted.
pub(crate) async fn list_trash(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<Vec<TrashItem>>, ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let items = state
        .storage
        .list_trash(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(
        items
            .into_iter()
            .map(|item| TrashItem {
                id: item.id,
                kind: item.kind,
                title: item.title,
                deleted_by: item.deleted_by,
                deleted_at: item.deleted_at,
                expires_at: item.expires_at,
            })
            .collect(),
    ))
}

/// Puts an item back under its own id. A restored file is indexed again, and the citations that pointed at it are placed
/// as passages when they can be found and as the file when not.
pub(crate) async fn restore_trash_item(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, trash_id)): Path<(String, String)>,
) -> Result<Json<TrashRestoreResult>, ApiError> {
    require_workspace_write(&state, &subject, &workspace_id).await?;
    let restored = state
        .storage
        .restore_from_trash(&workspace_id, &trash_id)
        .await
        .map_err(map_storage_error)?;
    let mut widened = 0_u64;
    if restored.kind == "artifact" {
        let chunks = match state.core.index_artifact(restored.item_id.clone()).await {
            Ok(_) => state
                .storage
                .list_chunks_for_artifact(&restored.item_id)
                .await
                .unwrap_or_default(),
            Err(error) => {
                tracing::warn!(error = %error, "A restored file could not be indexed straight away");
                Vec::new()
            }
        };
        for citation in &restored.citations {
            let found = citation.chunk_hash.as_ref().and_then(|hash| {
                chunks.iter().find(|chunk| &chunk.content_hash == hash).or_else(|| {
                    chunks
                        .iter()
                        .find(|chunk| Some(chunk.chunk_index) == citation.chunk_index)
                })
            });
            if citation.chunk_hash.is_some() && found.is_none() {
                widened += 1;
            }
            if let Err(error) = state
                .storage
                .relink_restored_citation(
                    &workspace_id,
                    &citation.card_id,
                    &restored.item_id,
                    found.map(|chunk| chunk.id.as_str()),
                    citation.confidence,
                )
                .await
            {
                tracing::error!(error = %error, "A citation could not be placed after a restore");
            }
        }
        state.index_queue.resume_pending().await;
    }
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "trash_restored",
        &restored.kind,
        Some(&restored.item_id),
        format!("Restored from the trash: {}.", restored.title),
    )
    .await;
    Ok(Json(TrashRestoreResult {
        kind: restored.kind,
        item_id: restored.item_id,
        title: restored.title,
        skipped_citations: restored.skipped_citations,
        widened_citations: widened,
    }))
}

/// Removes an item from trash for good. Only an administrator can, and the workspace's activity records it.
pub(crate) async fn purge_trash_item(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((workspace_id, trash_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    let item = state
        .storage
        .trash_item(&workspace_id, &trash_id)
        .await
        .map_err(map_storage_error)?;
    state
        .storage
        .purge_trash_item(&workspace_id, &trash_id)
        .await
        .map_err(map_storage_error)?;
    tracing::info!(target: "audit", user_id = %subject.user_id, workspace_id = %workspace_id, kind = %item.kind, "Trash item purged");
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "trash_purged",
        &item.kind,
        Some(&item.item_id),
        format!("Permanently removed from the trash: {}.", item.title),
    )
    .await;
    Ok(StatusCode::NO_CONTENT)
}
