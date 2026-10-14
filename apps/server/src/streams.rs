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
/// Emits `job` and `activity` events, and `resync` when the client fell
/// behind and missed some (it should reload what it shows). `KeepAlive`
/// comments keep proxies from closing the connection during quiet stretches.
///
/// The stream ends when the access token expires, when the user's sessions
/// are ended, or when the user leaves the workspace; the client reconnects
/// with a fresh token.
pub(crate) async fn workspace_events(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let open = OpenEventStream::new(
        state.telemetry.clone(),
        &subject.user_id,
        state.settings().max_event_streams_per_user,
    )?;
    let receiver = state.event_bus.subscribe(&workspace_id);
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
    tokio::spawn(watch_event_stream_access(state.clone(), subject, workspace_id, closer));
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
        let closing = state.closing.load(std::sync::atomic::Ordering::SeqCst);
        if expired || !allowed || closing {
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

/// Closes an event stream once its token expires or its user may no longer
/// read the workspace. Stops by itself when the client disconnects.
pub(crate) async fn watch_event_stream_access(
    state: AppState,
    subject: AuthenticatedSubject,
    workspace_id: String,
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
        let member = effective_workspace_role(&state, &subject.user_id, is_system_admin, &workspace_id)
            .await
            .map(|role| role.is_some())
            .unwrap_or(true);
        let closing = state.closing.load(std::sync::atomic::Ordering::SeqCst);
        if expired || !session_valid || !member || closing {
            let _ = closer.send(()).await;
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
