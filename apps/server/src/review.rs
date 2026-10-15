//! The review queue: what in a workspace waits for a person, in one list.
//!
//! Three things can wait, and each has its own place to be resolved: a file
//! marked *needs review* (its page), a memory card whose evidence changed
//! (its page) and an open health finding (the Health page). The list is
//! computed on each request from those places, so an item leaves it the moment
//! it is resolved there; nothing is stored here.

use axum::{
    extract::{Path, State},
    Json,
};
use repomemo_domain::{HealthDetector, ReviewItem, ReviewItemKind, ReviewQueue, ReviewResolver};

use crate::{
    consume_heavy_quota, health::cached_workspace_health, map_storage_error, record_timing, require_workspace_read,
    ApiError, AppState, AuthenticatedSubject,
};

/// Files a finding names before the rest are counted.
const FINDING_FILES_SHOWN: usize = 3;

pub(crate) async fn review_queue(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Json<ReviewQueue>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    consume_heavy_quota(&state, &subject)?;
    let started = std::time::Instant::now();
    let evidence = state
        .storage
        .artifacts_needing_review(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let cards = state
        .storage
        .memory_cards_needing_review(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let health = cached_workspace_health(&state, &workspace_id).await?;

    let mut items = Vec::new();
    let evidence_count = evidence.len() as i64;
    for (artifact_id, title, path, note, since) in evidence {
        let note = note.trim();
        items.push(ReviewItem {
            kind: ReviewItemKind::Evidence,
            id: artifact_id.clone(),
            title,
            reason: if note.is_empty() {
                "Marked as needing review.".to_owned()
            } else {
                note.to_owned()
            },
            detail: path,
            artifact_id: Some(artifact_id),
            since: Some(since),
            resolver: ReviewResolver::Writer,
        });
    }
    let memory_card_count = cards.len() as i64;
    for (card, reasons) in cards {
        let first = reasons.first().map(|reason| reason.detail.clone()).unwrap_or_default();
        let reason = match reasons.len() {
            0 | 1 => first,
            more => format!(
                "{first} And {} more change{}.",
                more - 1,
                if more == 2 { "" } else { "s" }
            ),
        };
        items.push(ReviewItem {
            kind: ReviewItemKind::MemoryCard,
            id: card.id,
            title: card.title,
            reason,
            detail: format!(
                "Cites {} piece{} of evidence",
                card.evidence_count,
                if card.evidence_count == 1 { "" } else { "s" }
            ),
            artifact_id: reasons.iter().find_map(|reason| reason.artifact_id.clone()),
            since: Some(card.updated_at),
            resolver: ReviewResolver::Writer,
        });
    }
    // A stale memory card is listed as the card above, not twice.
    let mut finding_count = 0;
    for finding in health
        .findings
        .into_iter()
        .filter(|finding| finding.detector != HealthDetector::StaleMemoryCitation)
    {
        finding_count += 1;
        let mut names = finding
            .files
            .iter()
            .take(FINDING_FILES_SHOWN)
            .map(|file| file.title.clone())
            .collect::<Vec<_>>();
        if finding.files.len() > FINDING_FILES_SHOWN {
            names.push(format!("and {} more", finding.files.len() - FINDING_FILES_SHOWN));
        }
        items.push(ReviewItem {
            kind: ReviewItemKind::HealthFinding,
            id: finding.fingerprint,
            title: finding.title,
            reason: finding.detail,
            detail: names.join(", "),
            artifact_id: finding.files.first().map(|file| file.artifact_id.clone()),
            since: None,
            resolver: ReviewResolver::Admin,
        });
    }
    // Oldest first; findings, which have no date, after what has waited.
    items.sort_by(|left, right| match (&left.since, &right.since) {
        (Some(left), Some(right)) => left.cmp(right),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    record_timing("review_queue", &workspace_id, started, items.len());
    Ok(Json(ReviewQueue {
        items,
        evidence_count,
        memory_card_count,
        finding_count,
    }))
}
