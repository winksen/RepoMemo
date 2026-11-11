//! Memory history (V0.6.3): list, compare and restore a card's attributed revisions. Reading needs read access to the
//! card's workspace; restoring needs write access, and the restore is itself a new revision.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use repomemo_domain::{
    MemoryChange, MemoryChanges, MemoryRestoreResult, MemoryRevision, MemoryRevisionCitation, MemoryRevisionComparison,
};
use repomemo_storage::{RevisionCitation, StoredMemoryRevision};
use serde::Deserialize;

use crate::{
    map_core_error, map_storage_error, record_workspace_activity, require_workspace_read, require_workspace_write,
    ApiError, AppState, AuthenticatedSubject,
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

/// What changed behind a card's "needs review" flag (V0.6.9): for each reason, the earlier and current text of the cited
/// file when an earlier version is kept, otherwise a plain statement of why it cannot be shown.
pub(crate) async fn card_changes(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(card_id): Path<String>,
) -> Result<Json<MemoryChanges>, ApiError> {
    card_workspace(&state, &subject, &card_id, false).await?;
    let detail = state
        .storage
        .get_memory_card(&card_id)
        .await
        .map_err(map_storage_error)?;
    let written = detail.card.updated_at.clone();
    let missing = state
        .storage
        .missing_cited_targets(&card_id)
        .await
        .map_err(map_storage_error)?;
    let mut changes = Vec::new();
    for reason in detail.review {
        let title = reason.title.clone().unwrap_or_default();
        let mut change = MemoryChange {
            kind: reason.kind.clone(),
            artifact_id: reason.artifact_id.clone(),
            title: title.clone(),
            summary: reason.detail.clone(),
            comparison: None,
            unavailable_reason: None,
        };
        match (reason.kind.as_str(), reason.artifact_id.as_deref()) {
            ("missing", _) => {
                let gone = missing
                    .iter()
                    .find(|citation| !citation.title.is_empty() || !citation.path.is_empty());
                if let Some(gone) = gone {
                    change.title = if gone.title.is_empty() {
                        gone.path.clone()
                    } else {
                        gone.title.clone()
                    };
                }
                change.unavailable_reason = Some(
                    "The cited passage or file no longer exists, and RepoMemo does not keep the text of removed passages. The citation can be removed from the card."
                        .to_owned(),
                );
            }
            (_, Some(artifact_id)) => {
                let versions = state
                    .storage
                    .list_artifact_versions(artifact_id)
                    .await
                    .map_err(map_storage_error)?;
                let current = versions
                    .iter()
                    .find(|version| version.is_current)
                    .map(|version| version.version);
                // The version that was current when the card was last written.
                let earlier = versions.iter().find(|version| {
                    version.created_at <= written
                        && version.replaced_at.as_deref().is_none_or(|end| end > written.as_str())
                });
                match (earlier, current) {
                    (Some(earlier), Some(current)) if earlier.version != current => {
                        let comparison = state
                            .core
                            .compare_artifact_versions(artifact_id, earlier.version, current)
                            .await
                            .map_err(map_core_error)?;
                        change.summary = format!(
                            "{} Version {} was current when the card was written; version {current} is now.",
                            change.summary, earlier.version
                        );
                        change.comparison = Some(comparison);
                    }
                    _ if reason.kind == "retired" => {
                        change.unavailable_reason =
                            Some("The file's text did not change; it was marked outdated or superseded.".to_owned());
                    }
                    _ => {
                        change.unavailable_reason = Some(
                            "No earlier version of this file is kept (repository files follow their commits), so the previous text cannot be shown. Open the file and compare it with the card."
                                .to_owned(),
                        );
                    }
                }
            }
            _ => {
                change.unavailable_reason = Some("The cited source cannot be identified.".to_owned());
            }
        }
        changes.push(change);
    }
    Ok(Json(MemoryChanges {
        revision: detail.revision,
        changes,
    }))
}
