//! The review queue: what in a workspace waits for a person, in one list.
//!
//! Three things can wait, and each has its own place to be resolved: a file
//! marked *needs review* (its page), a memory card whose evidence changed
//! (its page) and an open health finding (the Health page). The list is
//! computed on each request from those places, so an item leaves it the moment
//! it is resolved there; nothing is stored here.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use repomemo_domain::{HealthDetector, ReviewItem, ReviewItemKind, ReviewQueue, ReviewResolver};
use serde::Deserialize;

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

/// Takes the items a person snoozed out of their list. `snoozes` are `(kind, id, until)` rows that have not
/// expired; the snoozed items keep their order of coming back, soonest first, each with its `snoozed_until`.
pub(crate) fn split_snoozed(
    items: Vec<ReviewItem>,
    snoozes: &[(String, String, String)],
) -> (Vec<ReviewItem>, Vec<ReviewItem>) {
    let mut active = Vec::with_capacity(items.len());
    let mut snoozed = Vec::new();
    for mut item in items {
        let kind = source_of(item.kind);
        match snoozes
            .iter()
            .find(|(snoozed_kind, snoozed_id, _)| snoozed_kind == kind && *snoozed_id == item.id)
        {
            Some((_, _, until)) => {
                item.snoozed_until = Some(until.clone());
                snoozed.push(item);
            }
            None => active.push(item),
        }
    }
    snoozed.sort_by(|left, right| left.snoozed_until.cmp(&right.snoozed_until));
    (active, snoozed)
}

/// A snooze lasts a year at most.
const SNOOZE_MAX_DAYS: i64 = 366;
/// Characters of an item's id that are accepted.
const SNOOZE_ID_CHARS: usize = 200;

/// Reads the date an item comes back (`2026-11-01` is its first moment in UTC, or a full RFC 3339 time) and
/// answers it in RFC 3339, refusing a past time and one more than a year ahead.
pub(crate) fn parse_snooze_until(text: &str, now: DateTime<Utc>) -> Result<String, &'static str> {
    let text = text.trim();
    let until = DateTime::parse_from_rfc3339(text)
        .map(|time| time.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(text, "%Y-%m-%d")
                .ok()
                .and_then(|day| day.and_hms_opt(0, 0, 0))
                .map(|time| time.and_utc())
        })
        .ok_or("Give the date the item comes back, such as 2026-11-01.")?;
    if until <= now {
        return Err("Choose a time in the future.");
    }
    if until > now + Duration::days(SNOOZE_MAX_DAYS) {
        return Err("An item can be snoozed for a year at most.");
    }
    Ok(until.to_rfc3339())
}

fn known_kind(kind: &str) -> bool {
    matches!(kind, "evidence" | "memory_card" | "health_finding")
}

fn check_item(kind: &str, id: &str) -> Result<(), ApiError> {
    if !known_kind(kind) {
        return Err(ApiError::bad_request(
            "An item is a file (evidence), a memory card (memory_card) or a health finding (health_finding).",
        ));
    }
    if id.is_empty() || id.chars().count() > SNOOZE_ID_CHARS || id.chars().any(char::is_control) {
        return Err(ApiError::bad_request("That item cannot be named."));
    }
    Ok(())
}

#[derive(Deserialize)]
pub(crate) struct SnoozeRequest {
    kind: String,
    id: String,
    /// When the item comes back: a date (`2026-11-01`) or an RFC 3339 time.
    until: String,
}

/// Hides an item from the caller's review list until a date (V0.3.10). It is the caller's own list: anyone who
/// can read the workspace can snooze, and nobody else's queue changes.
pub(crate) async fn snooze_review_item(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<SnoozeRequest>,
) -> Result<StatusCode, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    check_item(&request.kind, &request.id)?;
    let until = parse_snooze_until(&request.until, Utc::now()).map_err(ApiError::bad_request)?;
    state
        .storage
        .snooze_review_item(&subject.user_id, &workspace_id, &request.kind, &request.id, &until)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub(crate) struct BringBackQuery {
    kind: String,
    id: String,
}

/// Brings a snoozed item back to the caller's list now.
pub(crate) async fn bring_back_review_item(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<BringBackQuery>,
) -> Result<StatusCode, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    check_item(&query.kind, &query.id)?;
    state
        .storage
        .unsnooze_review_item(&subject.user_id, &workspace_id, &query.kind, &query.id)
        .await
        .map_err(map_storage_error)?;
    Ok(StatusCode::NO_CONTENT)
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
            snoozed_until: None,
        });
    }
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
            snoozed_until: None,
        });
    }
    // A stale memory card is listed as the card above, not twice.
    for finding in health
        .findings
        .into_iter()
        .filter(|finding| finding.detector != HealthDetector::StaleMemoryCitation)
    {
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
            snoozed_until: None,
        });
    }
    // The template's groups come first (V0.2.183); a workspace without a choice keeps the mixed list.
    let groups = match state.core.effective_template(&workspace_id).await {
        Ok(effective) => effective.template.review_order(),
        Err(_) => Vec::new(),
    };
    order_items(&mut items, &groups);
    // What this person snoozed is kept out of their list, and out of the counts, until it comes back.
    let snoozes = state
        .storage
        .review_snoozes(&subject.user_id, &workspace_id)
        .await
        .map_err(map_storage_error)?;
    let (items, snoozed) = split_snoozed(items, &snoozes);
    let waiting = |kind: ReviewItemKind| items.iter().filter(|item| item.kind == kind).count() as i64;
    let (evidence_count, memory_card_count, finding_count) = (
        waiting(ReviewItemKind::Evidence),
        waiting(ReviewItemKind::MemoryCard),
        waiting(ReviewItemKind::HealthFinding),
    );
    record_timing("review_queue", &workspace_id, started, items.len());
    Ok(Json(ReviewQueue {
        items,
        evidence_count,
        memory_card_count,
        finding_count,
        snoozed,
    }))
}

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
            snoozed_until: None,
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

    #[test]
    fn snoozed_items_leave_the_list_by_kind_and_id_and_come_back_soonest_first() {
        let snoozes = vec![
            (
                "memory_card".to_owned(),
                "card-late".to_owned(),
                "2026-12-01T00:00:00+00:00".to_owned(),
            ),
            (
                "evidence".to_owned(),
                "file-early".to_owned(),
                "2026-11-01T00:00:00+00:00".to_owned(),
            ),
            // The same id under another kind is another item.
            (
                "health_finding".to_owned(),
                "file-late".to_owned(),
                "2026-11-15T00:00:00+00:00".to_owned(),
            ),
        ];
        let (waiting, snoozed) = split_snoozed(list(), &snoozes);
        assert_eq!(ids(&waiting), ["finding-a", "file-late", "finding-b", "card-early"]);
        assert_eq!(ids(&snoozed), ["file-early", "card-late"]);
        assert_eq!(snoozed[0].snoozed_until.as_deref(), Some("2026-11-01T00:00:00+00:00"));
        assert!(waiting.iter().all(|item| item.snoozed_until.is_none()));
        // Nothing snoozed: nothing moves.
        let (all, none) = split_snoozed(list(), &[]);
        assert_eq!((all.len(), none.len()), (6, 0));
    }

    #[test]
    fn a_snooze_date_is_a_day_or_a_time_in_the_next_year() {
        let now = "2026-10-24T12:00:00Z".parse::<DateTime<Utc>>().unwrap();
        assert_eq!(
            parse_snooze_until("2026-11-01", now).unwrap(),
            "2026-11-01T00:00:00+00:00"
        );
        assert_eq!(
            parse_snooze_until(" 2026-10-25T09:30:00+02:00 ", now).unwrap(),
            "2026-10-25T07:30:00+00:00"
        );
        assert!(parse_snooze_until("2026-10-24", now).is_err(), "earlier today is past");
        assert!(parse_snooze_until("2026-10-24T11:59:59Z", now).is_err());
        assert!(parse_snooze_until("2027-10-25", now).is_ok());
        assert!(parse_snooze_until("2027-11-30", now).is_err(), "more than a year");
        for text in ["", "soon", "2026-13-01", "01/11/2026"] {
            assert!(parse_snooze_until(text, now).is_err(), "{text:?}");
        }
    }
}
