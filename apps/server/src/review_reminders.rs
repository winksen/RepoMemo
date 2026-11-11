//! Review reminders (V0.6.7): a reviewer whose item is past its review date gets one reminder per window, through the
//! channels they chose for `review_due` (the app inbox, mail, both or neither) and, for those who asked for it, the daily
//! digest, which already carries unread inbox notices.
//!
//! Dates are start-of-day UTC, so an item is due from 00:00 UTC on its date whatever the reviewer's time zone. A snoozed
//! item is skipped until its snooze ends; changing the date allows a new reminder at once; a reviewer who left, lost the
//! right to edit or was deactivated gets none. When mail is not set up, or the person chose mail only and it cannot be
//! sent, nothing is recorded as sent and the next pass tries again.

use repomemo_storage::{NewSharedNotification, ReminderCandidate};

use crate::AppState;

/// A due review is reminded about at most this often.
pub(crate) const WINDOW_DAYS: i64 = 7;

/// The words of the reminder, made from the item and the date only.
pub(crate) fn reminder_text(title: &str, next_review_at: &str) -> (String, String) {
    let day = next_review_at.get(..10).unwrap_or(next_review_at);
    (
        "A review is due".to_owned(),
        format!("{title} was due for review on {day}."),
    )
}

fn path_for(candidate: &ReminderCandidate) -> String {
    format!("/workspaces/{}/review", candidate.workspace_id)
}

/// Sends the reminders that are owed now. Returns how many people were reached.
pub(crate) async fn send_due(state: &AppState) -> anyhow::Result<usize> {
    let mut reached = 0;
    for candidate in state.storage.reminder_candidates(WINDOW_DAYS).await? {
        let (in_app, by_mail) = crate::profile::delivery(state, &candidate.user_id, "review_due").await;
        let (title, body) = reminder_text(&candidate.title, &candidate.next_review_at);
        let mut delivered = false;
        if in_app {
            let made = state
                .storage
                .create_shared_notification(NewSharedNotification {
                    user_id: candidate.user_id.clone(),
                    workspace_id: Some(candidate.workspace_id.clone()),
                    notification_type: "review_due".to_owned(),
                    title: title.clone(),
                    body: body.clone(),
                    href: path_for(&candidate),
                })
                .await;
            delivered |= made.is_ok();
        }
        if by_mail {
            if let Some(link) = state.mail.link(&path_for(&candidate)) {
                if state.mail.is_ready().await {
                    let message = format!(
                        "{body}\n\nOpen the review queue:\n\n{link}\n\nYou get at most one reminder a week for each item. Choose how reminders reach you in your profile, under Notifications.\n"
                    );
                    delivered |= state
                        .mail
                        .send_to(
                            &candidate.email,
                            &format!("{title}: {}", candidate.title),
                            &message,
                            "review_due",
                        )
                        .await
                        .is_ok();
                }
            }
        }
        if delivered {
            state.storage.record_reminder(&candidate).await?;
            reached += 1;
        }
    }
    Ok(reached)
}

#[cfg(test)]
mod tests {
    use super::reminder_text;

    #[test]
    fn a_reminder_names_the_item_and_the_day_it_was_due() {
        let (title, body) = reminder_text("Refund policy", "2026-12-01T00:00:00+00:00");
        assert_eq!(title, "A review is due");
        assert_eq!(body, "Refund policy was due for review on 2026-12-01.");
    }
}
