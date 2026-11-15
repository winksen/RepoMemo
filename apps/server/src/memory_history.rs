//! Memory history (V0.6.3): list, compare and restore a card's attributed revisions. Reading needs read access to the
//! card's workspace; restoring needs write access, and the restore is itself a new revision.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderName, HeaderValue},
    Json,
};
use repomemo_domain::{
    CitedSource, MemoryChange, MemoryChanges, MemoryRestoreResult, MemoryRevision, MemoryRevisionCitation,
    MemoryRevisionComparison,
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
        artifact_id: Some(stored.artifact_id.clone()).filter(|id| !id.is_empty()),
        content_hash: Some(stored.content_hash.clone()).filter(|hash| !hash.is_empty()),
        currency: String::new(),
        missing_at_save: stored.missing_at_save,
    }
}

/// The content hash a file has now: `None` when the file no longer exists, an error when that could not be read, so a
/// database failure is never reported as a missing source.
async fn current_hash(state: &AppState, artifact_id: &str) -> Result<Option<String>, ApiError> {
    match state.storage.get_artifact_summary(artifact_id).await {
        Ok(summary) => Ok(Some(summary.content_hash)),
        Err(error) => match repomemo_domain::classify(&error) {
            Some((repomemo_domain::ErrorKind::NotFound, _)) => Ok(None),
            _ => Err(map_storage_error(error)),
        },
    }
}

/// Says where each cited file stands now against the content the revision recorded.
async fn with_currency(state: &AppState, revisions: &mut [MemoryRevision]) -> Result<(), ApiError> {
    let mut hashes: std::collections::HashMap<String, Option<String>> = std::collections::HashMap::new();
    for revision in revisions.iter() {
        for citation in &revision.citations {
            if let Some(id) = &citation.artifact_id {
                if !hashes.contains_key(id) {
                    hashes.insert(id.clone(), current_hash(state, id).await?);
                }
            }
        }
    }
    for revision in revisions.iter_mut() {
        for citation in &mut revision.citations {
            citation.currency = match (&citation.artifact_id, &citation.content_hash) {
                _ if citation.missing_at_save => "gone",
                (Some(id), Some(recorded)) => match hashes.get(id).and_then(|now| now.as_ref()) {
                    None => "gone",
                    Some(now) if now == recorded => "current",
                    Some(_) => "changed",
                },
                (Some(id), None) => match hashes.get(id).and_then(|now| now.as_ref()) {
                    None => "gone",
                    Some(_) => "unbound",
                },
                _ => "unbound",
            }
            .to_owned();
        }
    }
    Ok(())
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
        author_label: stored.author_label,
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
    Query(page): Query<RevisionPage>,
) -> Result<(HeaderMap, Json<Vec<MemoryRevision>>), ApiError> {
    card_workspace(&state, &subject, &card_id, false).await?;
    let (revisions, next, total) = state
        .storage
        .memory_revisions_page(
            &card_id,
            page.before,
            page.limit.unwrap_or(repomemo_storage::DEFAULT_REVISION_PAGE),
        )
        .await
        .map_err(map_storage_error)?;
    let mut revisions: Vec<MemoryRevision> = revisions.into_iter().map(revision).collect();
    with_currency(&state, &mut revisions).await?;
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-total-count"),
        HeaderValue::from(total.max(0) as u64),
    );
    if let Some(cursor) = next {
        headers.insert(
            HeaderName::from_static("x-next-cursor"),
            HeaderValue::from(cursor.max(0) as u64),
        );
    }
    Ok((headers, Json(revisions)))
}

/// Paging of a card's history (V0.6.19): `limit` (default 50, at most 200) and `before`, the revision number to continue
/// below, which is the `X-Next-Cursor` of the previous page.
#[derive(Debug, Deserialize)]
pub(crate) struct RevisionPage {
    limit: Option<i64>,
    before: Option<i64>,
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
        .restore_memory_revision_by(&card_id, number, Some(&subject.user_id))
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

#[derive(Debug, Deserialize)]
pub(crate) struct SourceQuery {
    index: usize,
}

/// The text a revision's citation pointed at (V0.6.10): the file as cited when it is unchanged or an earlier version with
/// the recorded content is kept, otherwise the exact reason it cannot be shown. A person without access to the workspace
/// is refused before any of this is read.
pub(crate) async fn cited_source(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path((card_id, number)): Path<(String, i64)>,
    Query(query): Query<SourceQuery>,
) -> Result<Json<CitedSource>, ApiError> {
    card_workspace(&state, &subject, &card_id, false).await?;
    let stored = state
        .storage
        .memory_revision(&card_id, number)
        .await
        .map_err(map_storage_error)?;
    let citation = stored
        .citations
        .get(query.index)
        .ok_or_else(|| ApiError::not_found("That citation was not found in this revision."))?;
    let source = |state: &str, text: Option<String>, truncated: bool, note: &str| CitedSource {
        state: state.to_owned(),
        title: citation.title.clone(),
        content_hash: Some(citation.content_hash.clone()).filter(|hash| !hash.is_empty()),
        text,
        truncated,
        note: note.to_owned(),
    };
    if citation.missing_at_save {
        return Ok(Json(source(
            "gone",
            None,
            false,
            "The cited passage or file was already gone when this revision was saved, so there was nothing to record.",
        )));
    }
    if citation.artifact_id.is_empty() || citation.content_hash.is_empty() {
        return Ok(Json(source(
            "unbound",
            None,
            false,
            "This revision was saved before citations recorded the content they pointed at, so what the source said then is not known.",
        )));
    }
    let Some(current_hash) = current_hash(&state, &citation.artifact_id).await? else {
        return Ok(Json(source(
            "gone",
            None,
            false,
            "The cited file no longer exists, and the text it had is not kept.",
        )));
    };
    let versions = state
        .storage
        .list_artifact_versions(&citation.artifact_id)
        .await
        .map_err(map_storage_error)?;
    let Some(found) = versions
        .iter()
        .find(|version| version.content_hash == citation.content_hash)
    else {
        return Ok(Json(source(
            "unavailable",
            None,
            false,
            "The file changed after this revision was saved and the text it had then was not kept (repository files follow their commits).",
        )));
    };
    let text = state
        .core
        .artifact_version_text(&citation.artifact_id, found.version)
        .await
        .map_err(map_core_error)?;
    let state_name = if current_hash == citation.content_hash {
        "current"
    } else {
        "kept_version"
    };
    let note = if state_name == "current" {
        "The file is as it was when this revision was saved."
    } else {
        "The file has changed since; this is the version this revision cited."
    };
    let note = text.note.clone().unwrap_or_else(|| note.to_owned());
    Ok(Json(source(state_name, text.text, text.truncated, &note)))
}
