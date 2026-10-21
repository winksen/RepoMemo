use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value;

use crate::events::WorkspaceEvent;
use crate::*;

/// How often an open event stream re-checks that its user may still read the
/// workspace.
#[cfg(not(test))]
pub(crate) const EVENT_STREAM_RECHECK: Duration = Duration::from_secs(30);

#[cfg(test)]
pub(crate) const EVENT_STREAM_RECHECK: Duration = Duration::from_millis(50);

pub(crate) enum EventStreamItem {
    Event(SseEvent),
    Close,
}

/// Server-sent-events stream of job progress and activity for a workspace.
/// Emits `job` events, and `activity` events to owners and administrators,
/// who may read the activity feed; every other reader gets a `changed` event
/// in its place, which names the kind of change and nothing about who made it,
/// what it said or where from (E6). `resync` is sent when the client fell
/// behind and missed some (it should reload what it shows). `KeepAlive`
/// comments keep proxies from closing the connection during quiet stretches.
///
/// The stream ends when the access token expires, when the user's sessions
/// are ended, when a personal access token that opened it is revoked or
/// expires, or when the user leaves the workspace; the client reconnects with
/// a fresh token.
pub(crate) async fn workspace_events(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, ApiError> {
    let role = require_workspace_read(&state, &subject, &workspace_id).await?;
    let open = OpenEventStream::new(
        state.telemetry.clone(),
        &subject.user_id,
        state.settings().max_event_streams_per_user,
    )?;
    let receiver = state.event_bus.subscribe(&workspace_id);
    // Read when the stream opens and again by the watcher, so a demotion takes effect on a stream already open.
    let sees_activity = Arc::new(AtomicBool::new(sees_activity_feed(&role)));
    let reader_sees_activity = sees_activity.clone();
    let events = BroadcastStream::new(receiver).filter_map(move |item| match item {
        Ok(event) => Some(EventStreamItem::Event(workspace_event_message(
            &event,
            reader_sees_activity.load(Ordering::Relaxed),
        )?)),
        Err(BroadcastStreamRecvError::Lagged(missed)) => Some(EventStreamItem::Event(
            SseEvent::default()
                .event("resync")
                .data(json!({ "type": "resync", "missed": missed }).to_string()),
        )),
    });
    let (closer, closed) = tokio::sync::mpsc::channel::<()>(1);
    tokio::spawn(watch_event_stream_access(
        state.clone(),
        subject,
        workspace_id,
        sees_activity,
        closer,
    ));
    let stream = events
        .merge(ReceiverStream::new(closed).map(|_| EventStreamItem::Close))
        .take_while(|item| !matches!(item, EventStreamItem::Close))
        .filter_map(move |item| {
            let _counted = &open;
            match item {
                EventStreamItem::Event(event) => Some(Ok(event)),
                EventStreamItem::Close => None,
            }
        });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

/// Server-sent-events stream of job changes in every workspace, for the
/// System jobs page. Same `job` and `resync` events as a workspace stream, no
/// activity. Ends when the token expires, the sessions are ended, or the user
/// stops being a system or app administrator.
pub(crate) async fn system_events(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, ApiError> {
    system::require_system_admin(&subject)?;
    let open = OpenEventStream::new(
        state.telemetry.clone(),
        &subject.user_id,
        state.settings().max_event_streams_per_user,
    )?;
    let receiver = state.event_bus.subscribe_system_jobs();
    let events = BroadcastStream::new(receiver).filter_map(|item| match item {
        Ok(event) => {
            let payload = serde_json::to_string(&event).ok()?;
            Some(EventStreamItem::Event(
                SseEvent::default().event(event.event_name()).data(payload),
            ))
        }
        Err(BroadcastStreamRecvError::Lagged(missed)) => Some(EventStreamItem::Event(
            SseEvent::default()
                .event("resync")
                .data(json!({ "type": "resync", "missed": missed }).to_string()),
        )),
    });
    let (closer, closed) = tokio::sync::mpsc::channel::<()>(1);
    tokio::spawn(watch_system_event_stream_access(state.clone(), subject, closer));
    let stream = events
        .merge(ReceiverStream::new(closed).map(|_| EventStreamItem::Close))
        .take_while(|item| !matches!(item, EventStreamItem::Close))
        .filter_map(move |item| {
            let _counted = &open;
            match item {
                EventStreamItem::Event(event) => Some(Ok(event)),
                EventStreamItem::Close => None,
            }
        });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

/// Closes the System event stream once its token expires or its user loses
/// the System role. Stops by itself when the client disconnects.
pub(crate) async fn watch_system_event_stream_access(
    state: AppState,
    subject: AuthenticatedSubject,
    closer: tokio::sync::mpsc::Sender<()>,
) {
    loop {
        let remaining = subject.expires_at.saturating_sub(jsonwebtoken::get_current_timestamp());
        tokio::select! {
            _ = closer.closed() => return,
            _ = tokio::time::sleep(EVENT_STREAM_RECHECK.min(Duration::from_secs(remaining))) => {}
        }
        let expired = jsonwebtoken::get_current_timestamp() >= subject.expires_at;
        // A failed lookup keeps the stream; a missing user or a changed
        // session or role ends it.
        let allowed = match state.storage.user_access(&subject.user_id).await {
            Ok(access) => access.is_some_and(|access| {
                access.session_version == subject.session_version && (access.is_system_admin || access.is_app_admin)
            }),
            Err(_) => true,
        };
        let token_active = token_is_active(&state, &subject).await;
        let closing = state.closing.load(std::sync::atomic::Ordering::SeqCst);
        if expired || !allowed || !token_active || closing {
            let _ = closer.send(()).await;
            return;
        }
    }
}

/// Counts an open event stream for the System page while it lives, and for
/// its person: one account cannot hold the server's connections open.
pub(crate) struct OpenEventStream {
    pub(crate) telemetry: Arc<Telemetry>,
    pub(crate) user_id: String,
}

impl OpenEventStream {
    pub(crate) fn new(telemetry: Arc<Telemetry>, user_id: &str, limit: usize) -> Result<Self, ApiError> {
        if !telemetry.claim_event_stream(user_id, limit) {
            return Err(too_many_requests(
                format!("This account already has {limit} live streams open. Close a tab and try again."),
                Duration::from_secs(5),
            ));
        }
        Ok(Self {
            telemetry,
            user_id: user_id.to_owned(),
        })
    }
}

impl Drop for OpenEventStream {
    fn drop(&mut self) {
        self.telemetry.release_event_stream(&self.user_id);
    }
}

/// Who may read the activity feed (`GET …/activity`) and so be sent activity events: owners and administrators.
pub(crate) fn sees_activity_feed(role: &WorkspaceRole) -> bool {
    matches!(role, WorkspaceRole::Owner | WorkspaceRole::Admin)
}

/// Subjects whose ids a reader can already open, so the `changed` notice may carry the id (the page showing
/// that file or card refreshes itself). The ids of members, invitations, providers and the like stay out.
const READABLE_SUBJECTS: &[&str] = &[
    "artifact",
    "memory_card",
    "task",
    "folder",
    "repository",
    "connector",
    "saved_search",
];

/// What a reader who may not see the activity feed is told when an activity event happens: the action and the
/// kind of subject (and the id of a readable one), never the actor, the summary or the time.
pub(crate) fn changed_notice(event: &repomemo_domain::WorkspaceActivityEvent) -> Value {
    let subject_id = READABLE_SUBJECTS
        .contains(&event.subject_type.as_str())
        .then(|| event.subject_id.clone())
        .flatten();
    json!({
        "type": "changed",
        "action": event.action,
        "subject_type": event.subject_type,
        "subject_id": subject_id,
    })
}

/// The server-sent message for a workspace event, as the reader is allowed to see it.
pub(crate) fn workspace_event_message(event: &WorkspaceEvent, sees_activity: bool) -> Option<SseEvent> {
    match event {
        WorkspaceEvent::Activity { event } if !sees_activity => Some(
            SseEvent::default()
                .event("changed")
                .data(changed_notice(event).to_string()),
        ),
        _ => {
            let payload = serde_json::to_string(event).ok()?;
            Some(SseEvent::default().event(event.event_name()).data(payload))
        }
    }
}

/// Whether the personal access token a stream was opened with is still there (a session has none).
/// A failed lookup keeps the stream, as for the other checks.
async fn token_is_active(state: &AppState, subject: &AuthenticatedSubject) -> bool {
    match &subject.token {
        Some(token) => state
            .storage
            .personal_token_active(&token.token_id)
            .await
            .unwrap_or(true),
        None => true,
    }
}

/// Closes an event stream once its token expires or its user may no longer
/// read the workspace. Stops by itself when the client disconnects.
pub(crate) async fn watch_event_stream_access(
    state: AppState,
    subject: AuthenticatedSubject,
    workspace_id: String,
    sees_activity: Arc<AtomicBool>,
    closer: tokio::sync::mpsc::Sender<()>,
) {
    loop {
        let remaining = subject.expires_at.saturating_sub(jsonwebtoken::get_current_timestamp());
        tokio::select! {
            _ = closer.closed() => return,
            _ = tokio::time::sleep(EVENT_STREAM_RECHECK.min(Duration::from_secs(remaining))) => {}
        }
        let expired = jsonwebtoken::get_current_timestamp() >= subject.expires_at;
        let access = state.storage.user_access(&subject.user_id).await.ok();
        let session_valid = access.map_or(true, |access| {
            access.is_some_and(|access| access.session_version == subject.session_version)
        });
        let is_system_admin = access.flatten().is_some_and(|access| access.is_system_admin);
        let role = effective_workspace_role(&state, &subject.user_id, is_system_admin, &workspace_id).await;
        // A failed lookup keeps the stream as it was; a missing membership ends it.
        let member = role.as_ref().map(|role| role.is_some()).unwrap_or(true);
        if let Ok(Some(role)) = &role {
            sees_activity.store(sees_activity_feed(role), Ordering::Relaxed);
        }
        let token_active = token_is_active(&state, &subject).await;
        let closing = state.closing.load(std::sync::atomic::Ordering::SeqCst);
        if expired || !session_valid || !member || !token_active || closing {
            let _ = closer.send(()).await;
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity(subject_type: &str, subject_id: Option<&str>) -> repomemo_domain::WorkspaceActivityEvent {
        repomemo_domain::WorkspaceActivityEvent {
            id: "event-1".to_owned(),
            workspace_id: "workspace-1".to_owned(),
            actor: None,
            action: "member_removed".to_owned(),
            subject_type: subject_type.to_owned(),
            subject_id: subject_id.map(str::to_owned),
            summary: "Removed ada@example.com from the workspace.".to_owned(),
            created_at: "2026-10-21T10:00:00Z".to_owned(),
        }
    }

    #[test]
    fn only_owners_and_administrators_see_the_activity_feed() {
        assert!(sees_activity_feed(&WorkspaceRole::Owner));
        assert!(sees_activity_feed(&WorkspaceRole::Admin));
        assert!(!sees_activity_feed(&WorkspaceRole::Member));
        assert!(!sees_activity_feed(&WorkspaceRole::Viewer));
    }

    #[test]
    fn a_changed_notice_names_the_kind_and_nothing_about_who_or_what_was_said() {
        let notice = changed_notice(&activity("artifact", Some("artifact-1")));
        assert_eq!(notice["type"], "changed");
        assert_eq!(notice["action"], "member_removed");
        assert_eq!(notice["subject_type"], "artifact");
        assert_eq!(notice["subject_id"], "artifact-1", "a readable file's id may travel");
        let text = notice.to_string();
        for private in [
            "ada@example.com",
            "Removed",
            "actor",
            "summary",
            "created_at",
            "2026-10-21",
        ] {
            assert!(!text.contains(private), "{private} must not be in {text}");
        }
        // The id of a member, an invitation or a provider is not a thing a reader opens.
        for hidden in ["user", "invitation", "ai_provider", "workspace"] {
            assert!(
                changed_notice(&activity(hidden, Some("secret-id")))["subject_id"].is_null(),
                "{hidden}"
            );
        }
    }

    #[test]
    fn activity_goes_out_in_full_only_to_readers_who_may_read_the_feed() {
        let event = WorkspaceEvent::Activity {
            event: activity("user", Some("user-1")),
        };
        let full = format!("{:?}", workspace_event_message(&event, true).unwrap());
        assert!(full.contains("activity") && full.contains("ada@example.com"), "{full}");
        let notice = format!("{:?}", workspace_event_message(&event, false).unwrap());
        assert!(
            notice.contains("changed") && !notice.contains("ada@example.com"),
            "{notice}"
        );
        assert!(!notice.contains("user-1"), "{notice}");
    }

    #[test]
    fn one_account_cannot_hold_more_live_streams_than_the_limit() {
        let telemetry = Arc::new(Telemetry::default());
        let first = OpenEventStream::new(telemetry.clone(), "user-a", 2).unwrap();
        let _second = OpenEventStream::new(telemetry.clone(), "user-a", 2).unwrap();
        assert!(
            OpenEventStream::new(telemetry.clone(), "user-a", 2).is_err(),
            "the third is refused"
        );
        let _other = OpenEventStream::new(telemetry.clone(), "user-b", 2).unwrap();
        assert_eq!(
            telemetry.open_event_streams.load(std::sync::atomic::Ordering::Relaxed),
            3
        );
        drop(first);
        assert!(
            OpenEventStream::new(telemetry.clone(), "user-a", 2).is_ok(),
            "a closed stream frees its slot"
        );
    }
}
