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

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: ReviewItemKind, id: &str, since: Option<&str>) -> ReviewItem {
        ReviewItem {
            kind,
            id: id.to_owned(),
            title: id.to_owned(),
            reason: String::new(),
            detail: String::new(),
            artifact_id: None,
            since: since.map(str::to_owned),
            resolver: ReviewResolver::Writer,
        }
    }

    fn list() -> Vec<ReviewItem> {
        vec![
            item(ReviewItemKind::HealthFinding, "finding-a", None),
            item(ReviewItemKind::MemoryCard, "card-late", Some("2026-03-01")),
            item(ReviewItemKind::Evidence, "file-late", Some("2026-02-01")),
            item(ReviewItemKind::HealthFinding, "finding-b", None),
            item(ReviewItemKind::Evidence, "file-early", Some("2026-01-01")),
            item(ReviewItemKind::MemoryCard, "card-early", Some("2026-01-15")),
        ]
    }

    fn ids(items: &[ReviewItem]) -> Vec<&str> {
        items.iter().map(|item| item.id.as_str()).collect()
    }

    fn groups(list: &[&str]) -> Vec<String> {
        list.iter().map(|group| (*group).to_owned()).collect()
    }

    #[test]
    fn without_a_choice_the_items_are_mixed_oldest_first_with_the_findings_after() {
        let mut items = list();
        order_items(&mut items, &[]);
        assert_eq!(
            ids(&items),
            [
                "file-early",
                "card-early",
                "file-late",
                "card-late",
                "finding-a",
                "finding-b"
            ]
        );
    }

    #[test]
    fn a_template_lists_its_kinds_in_groups_and_inside_a_group_the_oldest_first() {
        let mut operations = list();
        order_items(&mut operations, &groups(&["health_finding", "evidence", "memory_card"]));
        assert_eq!(
            ids(&operations),
            [
                "finding-a",
                "finding-b",
                "file-early",
                "file-late",
                "card-early",
                "card-late"
            ]
        );
        let mut cards_first = list();
        order_items(
            &mut cards_first,
            &groups(&["memory_card", "evidence", "health_finding"]),
        );
        assert_eq!(
            ids(&cards_first),
            [
                "card-early",
                "card-late",
                "file-early",
                "file-late",
                "finding-a",
                "finding-b"
            ]
        );
    }

    #[test]
    fn a_kind_the_template_does_not_name_follows_the_named_ones_and_the_findings_keep_healths_order() {
        let mut items = list();
        order_items(&mut items, &groups(&["evidence"]));
        assert_eq!(ids(&items)[..2], ["file-early", "file-late"]);
        // The others follow, mixed by the same rule: oldest first, findings last, in the order they came.
        assert_eq!(ids(&items)[2..], ["card-early", "card-late", "finding-a", "finding-b"]);
        let mut again = list();
        order_items(&mut again, &groups(&["health_finding"]));
        assert_eq!(ids(&again)[..2], ["finding-a", "finding-b"]);
    }

    #[test]
    fn the_kinds_a_template_can_name_are_the_review_item_kinds() {
        let every = [
            ReviewItemKind::Evidence,
            ReviewItemKind::MemoryCard,
            ReviewItemKind::HealthFinding,
        ];
        let named: Vec<&str> = every.iter().map(|kind| source_of(*kind)).collect();
        assert_eq!(named, repomemo_templates::REVIEW_SOURCES);
        for kind in every {
            assert_eq!(serde_json::to_value(kind).unwrap(), serde_json::json!(source_of(kind)));
        }
        // Adding a kind stops this from compiling until it is listed.
        let _ = |kind: ReviewItemKind| match kind {
            ReviewItemKind::Evidence | ReviewItemKind::MemoryCard | ReviewItemKind::HealthFinding => (),
        };
    }
}

use crate::{
    consume_heavy_quota, health::cached_workspace_health, map_storage_error, record_timing, require_workspace_read,
    ApiError, AppState, AuthenticatedSubject,
};

/// Files a finding names before the rest are counted.
const FINDING_FILES_SHOWN: usize = 3;

/// The id the template uses for a kind of item.
fn source_of(kind: ReviewItemKind) -> &'static str {
    match kind {
        ReviewItemKind::Evidence => "evidence",
        ReviewItemKind::MemoryCard => "memory_card",
        ReviewItemKind::HealthFinding => "health_finding",
    }
}

/// Oldest first, findings (which have no date) after what has waited. With `groups` (a template's choice) each kind
/// is a group, the groups in that order, and inside a group the same rule: the oldest first, and findings in the
/// order Health gave them.
pub(crate) fn order_items(items: &mut [ReviewItem], groups: &[String]) {
    let rank = |item: &ReviewItem| {
        groups
            .iter()
            .position(|group| group == source_of(item.kind))
            .unwrap_or(groups.len())
    };
    items.sort_by(|left, right| {
        let by_group = if groups.is_empty() {
            std::cmp::Ordering::Equal
        } else {
            rank(left).cmp(&rank(right))
        };
        by_group.then_with(|| match (&left.since, &right.since) {
            (Some(left), Some(right)) => left.cmp(right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        })
    });
}

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
    // The template's groups come first (V0.2.183); a workspace without a choice keeps the mixed list.
    let groups = match state.core.effective_template(&workspace_id).await {
        Ok(effective) => effective.template.review_order(),
        Err(_) => Vec::new(),
    };
    order_items(&mut items, &groups);
    record_timing("review_queue", &workspace_id, started, items.len());
    Ok(Json(ReviewQueue {
        items,
        evidence_count,
        memory_card_count,
        finding_count,
    }))
}
