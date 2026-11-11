//! Memory history (V0.6.3): list, compare and restore a card's attributed revisions. Reading needs read access to the
//! card's workspace; restoring needs write access, and the restore is itself a new revision.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use repomemo_domain::{MemoryRestoreResult, MemoryRevision, MemoryRevisionCitation, MemoryRevisionComparison};
use repomemo_storage::{RevisionCitation, StoredMemoryRevision};
use serde::Deserialize;

use crate::{
    map_storage_error, record_workspace_activity, require_workspace_read, require_workspace_write, ApiError, AppState,
    AuthenticatedSubject,
};

fn citation(stored: &RevisionCitation) -> MemoryRevisionCitation {
    MemoryRevisionCitation {
        target_type: stored.target_type.clone(),
        target_id: stored.target_id.clone(),
        title: stored.title.clone(),
        path: stored.path.clone(),
        confidence: stored.confidence,
    }
}

fn revision(stored: StoredMemoryRevision) -> MemoryRevision {
    MemoryRevision {
        revision: stored.revision,
        title: stored.title,
        body_markdown: stored.body_markdown,
        source: stored.source,
        confidence: stored.confidence,
        citations: stored.citations.iter().map(citation).collect(),
        author: stored.author,
        created_at: stored.created_at,
        restored_from: stored.restored_from,
    }
}

/// The workspace a card is in, after the caller has been checked against it. A card the caller cannot read looks
/// the same as one that does not exist.
async fn card_workspace(
    state: &AppState,
    subject: &AuthenticatedSubject,
    card_id: &str,
    write: bool,
) -> Result<String, ApiError> {
    let card = state
        .storage
        .get_memory_card(card_id)
        .await
        .map_err(|_| ApiError::not_found("Memory card was not found."))?;
    let workspace_id = card.card.workspace_id;
    if write {
        require_workspace_write(state, subject, &workspace_id).await?;
    } else {
        require_workspace_read(state, subject, &workspace_id).await?;
    }
    Ok(workspace_id)
}

pub(crate) async fn list_revisions(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(card_id): Path<String>,
) -> Result<Json<Vec<MemoryRevision>>, ApiError> {
    card_workspace(&state, &subject, &card_id, false).await?;
    let revisions = state
        .storage
        .memory_revisions(&card_id)
        .await
        .map_err(map_storage_error)?;
    Ok(Json(revisions.into_iter().map(revision).collect()))
}

#[derive(Debug, Deserialize)]
pub(crate) struct CompareQuery {
    from: i64,
    to: i64,
}

pub(crate) async fn compare_revisions(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(card_id): Path<String>,
    Query(query): Query<CompareQuery>,
) -> Result<Json<MemoryRevisionComparison>, ApiError> {
    card_workspace(&state, &subject, &card_id, false).await?;
    let (older, newer) = if query.from <= query.to {
        (query.from, query.to)
    } else {
        (query.to, query.from)
    };
    let from = state
        .storage
        .memory_revision(&card_id, older)
        .await
        .map_err(map_storage_error)?;
    let to = state
        .storage
        .memory_revision(&card_id, newer)
        .await
        .map_err(map_storage_error)?;
    let key = |stored: &RevisionCitation| (stored.target_type.clone(), stored.target_id.clone());
    let added = to
        .citations
        .iter()
        .filter(|stored| !from.citations.iter().any(|other| key(other) == key(stored)))
        .map(citation)
        .collect();
    let removed = from
        .citations
        .iter()
        .filter(|stored| !to.citations.iter().any(|other| key(other) == key(stored)))
        .map(citation)
        .collect();
    let mut body = repomemo_api::compare_texts(&from.body_markdown, &to.body_markdown);
    body.from_version = older;
    body.to_version = newer;
    Ok(Json(MemoryRevisionComparison {
        from_revision: older,
        to_revision: newer,
        from_title: from.title,
        to_title: to.title,
        body,
        citations_added: added,
        citations_removed: removed,
    }))
}

pub(crate) async fn restore_revision(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((card_id, number)): Path<(String, i64)>,
) -> Result<Json<MemoryRestoreResult>, ApiError> {
    let workspace_id = card_workspace(&state, &subject, &card_id, true).await?;
    let restored = state
        .storage
        .restore_memory_revision(&card_id, number)
        .await
        .map_err(map_storage_error)?;
    state
        .storage
        .attribute_memory_revision(&card_id, &subject.user_id)
        .await
        .map_err(map_storage_error)?;
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "memory_restored",
        "memory_card",
        Some(&card_id),
        format!("Restored revision {number} of team memory: {}.", restored.card.title),
    )
    .await;
    Ok(Json(MemoryRestoreResult {
        card: restored.card,
        revision: restored.revision,
        skipped_citations: restored.skipped.iter().map(citation).collect(),
    }))
}
