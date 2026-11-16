//! Due reminders (V0.7.15): the person a task is assigned to is told once when it falls due and once when it is overdue, through
//! the channels they chose for `task_due` (the inbox, mail, both or neither) and the daily digest, which already carries
//! unread inbox notices.
//!
//! Time is UTC. A due date without a time is due from 00:00 UTC on that day and overdue from 00:00 UTC the next day; a due time
//! is due from 24 hours before it and overdue once it has passed. A task that is done, has no assignee or whose assignee left,
//! was deactivated or lost access gets none. A reminder is claimed before it is sent, so a retry or a restart sends nothing
//! twice; moving the due date allows a new one. When nothing could be delivered the claim is given back and the next pass
//! tries again.

use chrono::{DateTime, Duration, NaiveDate, Utc};
use repomemo_storage::NewSharedNotification;

use crate::AppState;

/// What a task's due date says about it at a moment: `due`, `overdue` or nothing yet.
pub(crate) fn reminder_kind(due_at: &str, now: DateTime<Utc>) -> Option<&'static str> {
    if due_at.len() == 10 {
        let day = NaiveDate::parse_from_str(due_at, "%Y-%m-%d").ok()?;
        let start = day.and_hms_opt(0, 0, 0)?.and_utc();
        if now >= start + Duration::days(1) {
            Some("overdue")
        } else if now >= start {
            Some("due")
        } else {
            None
        }
    } else {
        let due = DateTime::parse_from_rfc3339(due_at).ok()?.with_timezone(&Utc);
        if now > due {
            Some("overdue")
        } else if now >= due - Duration::hours(24) {
            Some("due")
        } else {
            None
        }
    }
}

/// The words of the reminder, made from the task and its due date only.
pub(crate) fn reminder_text(kind: &str, key: &str, title: &str, due_at: &str) -> (String, String) {
    let when = if due_at.len() == 10 {
        due_at.to_owned()
    } else {
        format!("{} UTC", due_at.get(..16).unwrap_or(due_at).replace('T', " "))
    };
    if kind == "overdue" {
        (
            "A task is overdue".to_owned(),
            format!("{key} {title} was due on {when} and is not done."),
        )
    } else if due_at.len() == 10 {
        (
            "A task is due today".to_owned(),
            format!("{key} {title} is due today ({when})."),
        )
    } else {
        (
            "A task is due soon".to_owned(),
            format!("{key} {title} is due on {when}."),
        )
    }
}

/// Sends the reminders that are owed now. Returns how many people were reached.
pub(crate) async fn send_due(state: &AppState) -> anyhow::Result<usize> {
    let now = Utc::now();
    let mut reached = 0;
    for candidate in state.storage.task_reminder_candidates().await? {
        let Some(kind) = reminder_kind(&candidate.due_at, now) else {
            continue;
        };
        // Claimed first: a second pass, a retry or a restart finds it taken and sends nothing.
        if !state.storage.claim_task_reminder(&candidate, kind).await? {
            continue;
        }
        let (in_app, by_mail) = crate::profile::delivery(state, &candidate.user_id, "task_due").await;
        if !in_app && !by_mail {
            // The person chose not to be told; the claim stays so they are not asked again every pass.
            continue;
        }
        let (title, body) = reminder_text(kind, &candidate.key, &candidate.title, &candidate.due_at);
        let path = format!("/workspaces/{}/tasks/{}", candidate.workspace_id, candidate.key);
        let mut delivered = false;
        if in_app {
            delivered |= state
                .storage
                .create_shared_notification(NewSharedNotification {
                    user_id: candidate.user_id.clone(),
                    workspace_id: Some(candidate.workspace_id.clone()),
                    notification_type: if kind == "overdue" { "task_overdue" } else { "task_due" }.to_owned(),
                    title: title.clone(),
                    body: body.clone(),
                    href: path.clone(),
                })
                .await
                .is_ok();
        }
        if by_mail {
            if let (Some(link), Some(address)) = (state.mail.link(&path), candidate.email.as_deref()) {
                if state.mail.is_ready().await {
                    let message = format!(
                        "{body}\n\nOpen the task:\n\n{link}\n\nYou get at most one reminder when a task falls due and one when it is overdue. Choose how reminders reach you in your profile, under Notifications.\n"
                    );
                    delivered |= state
                        .mail
                        .send_to(address, &format!("{title}: {}", candidate.title), &message, "task_due")
                        .await
                        .is_ok();
                }
            }
        }
        if delivered {
            reached += 1;
        } else {
            state.storage.release_task_reminder(&candidate, kind).await?;
        }
    }
    Ok(reached)
}

#[cfg(test)]
mod tests {
    use super::{reminder_kind, reminder_text};
    use chrono::{TimeZone, Utc};

    #[test]
    fn a_date_is_due_from_its_first_moment_and_overdue_the_next_day() {
        let at = |day: u32, hour: u32, minute: u32| Utc.with_ymd_and_hms(2026, 12, day, hour, minute, 0).unwrap();
        assert_eq!(reminder_kind("2026-12-10", at(9, 23, 59)), None);
        assert_eq!(reminder_kind("2026-12-10", at(10, 0, 0)), Some("due"));
        assert_eq!(reminder_kind("2026-12-10", at(10, 23, 59)), Some("due"));
        assert_eq!(reminder_kind("2026-12-10", at(11, 0, 0)), Some("overdue"));
    }

    #[test]
    fn a_time_is_due_a_day_ahead_and_overdue_once_it_has_passed() {
        let at = |day: u32, hour: u32| Utc.with_ymd_and_hms(2026, 12, day, hour, 0, 0).unwrap();
        let due = "2026-12-10T15:00:00+00:00";
        assert_eq!(reminder_kind(due, at(9, 14)), None);
        assert_eq!(reminder_kind(due, at(9, 15)), Some("due"));
        assert_eq!(reminder_kind(due, at(10, 15)), Some("due"));
        assert_eq!(reminder_kind(due, at(10, 16)), Some("overdue"));
        assert_eq!(reminder_kind("not a date", at(10, 16)), None);
    }

    #[test]
    fn a_reminder_names_the_task_and_the_day() {
        assert_eq!(
            reminder_text("overdue", "T-4", "Backfill", "2026-12-10"),
            (
                "A task is overdue".to_owned(),
                "T-4 Backfill was due on 2026-12-10 and is not done.".to_owned()
            )
        );
        assert_eq!(
            reminder_text("due", "T-4", "Backfill", "2026-12-10T15:00:00+00:00").1,
            "T-4 Backfill is due on 2026-12-10 15:00 UTC."
        );
    }
}
