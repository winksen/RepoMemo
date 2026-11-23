//! Live CAIRN runs (V0.9.43). A person's message can be answered in the background: `POST .../agent/runs` starts it and returns
//! at once, `GET /v1/agent/runs/{id}/events` streams each step as it is taken and then the stored turn, and
//! `POST /v1/agent/runs/{id}/cancel` stops it. The synchronous `POST .../agent/messages` still answers as before and waits for
//! the run. A live run belongs to the person who started it: only they can open its stream or stop it, the stream is closed when
//! they may no longer read the workspace, and a finished run is forgotten after a few minutes.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;
use tokio::sync::Notify;
use tokio_stream::wrappers::ReceiverStream;

use crate::agent::{run_message, AgentMessageRequest, LiveHooks};
use crate::*;

/// How long a finished run is kept for a stream that opens late.
const FINISHED_RUN_KEPT: Duration = Duration::from_secs(600);
/// How often an open stream checks that its person may still read the workspace.
const ACCESS_RECHECK: Duration = Duration::from_secs(30);
/// The most unfinished runs one person has at once.
const RUNS_PER_PERSON: usize = 1;

struct LiveRun {
    owner: String,
    workspace_id: String,
    /// What has been sent so far, as (event name, JSON), so a stream that opens late replays it.
    events: Mutex<Vec<(&'static str, String)>>,
    finished: AtomicBool,
    cancel: Arc<AtomicBool>,
    changed: Notify,
}

impl LiveRun {
    fn push(&self, name: &'static str, payload: serde_json::Value) {
        if let Ok(mut events) = self.events.lock() {
            events.push((name, payload.to_string()));
        }
        self.changed.notify_waiters();
    }
}

fn registry() -> &'static Mutex<HashMap<String, Arc<LiveRun>>> {
    static RUNS: OnceLock<Mutex<HashMap<String, Arc<LiveRun>>>> = OnceLock::new();
    RUNS.get_or_init(Mutex::default)
}

fn run_of(run_id: &str, subject: &AuthenticatedSubject) -> Result<Arc<LiveRun>, ApiError> {
    registry()
        .lock()
        .ok()
        .and_then(|runs| runs.get(run_id).cloned())
        // Another person's run, and one that is gone, look the same.
        .filter(|run| run.owner == subject.user_id)
        .ok_or_else(|| ApiError::not_found("That CAIRN run was not found."))
}

#[derive(Debug, Serialize)]
pub(crate) struct StartedRun {
    run_id: String,
}

/// Starts answering a message in the background.
pub(crate) async fn start_agent_run(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    Json(request): Json<AgentMessageRequest>,
) -> Result<(StatusCode, Json<StartedRun>), ApiError> {
    require_workspace_read(&state, &subject, &workspace_id).await?;
    let run_id = uuid::Uuid::new_v4().to_string();
    let run = Arc::new(LiveRun {
        owner: subject.user_id.clone(),
        workspace_id: workspace_id.clone(),
        events: Mutex::new(Vec::new()),
        finished: AtomicBool::new(false),
        cancel: Arc::new(AtomicBool::new(false)),
        changed: Notify::new(),
    });
    {
        let mut runs = registry()
            .lock()
            .map_err(|_| ApiError::internal("CAIRN runs are unavailable."))?;
        let unfinished = runs
            .values()
            .filter(|other| other.owner == subject.user_id && !other.finished.load(Ordering::Relaxed))
            .count();
        if unfinished >= RUNS_PER_PERSON {
            return Err(ApiError::conflict(
                "CAIRN is still working on your last message. Wait for it, or stop it, before you send another.",
            ));
        }
        runs.insert(run_id.clone(), run.clone());
    }
    let task_run = run.clone();
    let task_id = run_id.clone();
    tokio::spawn(async move {
        let progress_run = task_run.clone();
        let hooks = LiveHooks {
            progress: Some(Arc::new(move |step| {
                progress_run.push("step", json!(step));
            })),
            cancel: Some(task_run.cancel.clone()),
        };
        match run_message(&state, &subject, workspace_id, request, hooks).await {
            Ok(turn) => task_run.push("done", json!(turn)),
            Err(error) => task_run.push("error", json!({ "message": error.message })),
        }
        task_run.finished.store(true, Ordering::Relaxed);
        task_run.changed.notify_waiters();
        tokio::time::sleep(FINISHED_RUN_KEPT).await;
        if let Ok(mut runs) = registry().lock() {
            runs.remove(&task_id);
        }
    });
    Ok((StatusCode::ACCEPTED, Json(StartedRun { run_id })))
}

/// Streams a run: `step` events as they happen, then `done` with the stored turn (or `error`). The person is checked when the
/// stream opens and again every 30 seconds.
pub(crate) async fn agent_run_events(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(run_id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, ApiError> {
    let run = run_of(&run_id, &subject)?;
    require_workspace_read(&state, &subject, &run.workspace_id).await?;
    let (sender, receiver) = tokio::sync::mpsc::channel::<Result<SseEvent, Infallible>>(32);
    tokio::spawn(async move {
        let mut sent = 0;
        let mut checked = tokio::time::Instant::now();
        loop {
            // Take what is new, then say whether the run is over, before waiting for more.
            let batch = run
                .events
                .lock()
                .map(|events| events[sent..].to_vec())
                .unwrap_or_default();
            let finished = run.finished.load(Ordering::Relaxed);
            for (name, payload) in &batch {
                if sender
                    .send(Ok(SseEvent::default().event(*name).data(payload.clone())))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            sent += batch.len();
            if finished && batch.is_empty() {
                return;
            }
            if checked.elapsed() >= ACCESS_RECHECK {
                checked = tokio::time::Instant::now();
                if require_workspace_read(&state, &subject, &run.workspace_id)
                    .await
                    .is_err()
                {
                    return;
                }
            }
            if batch.is_empty() {
                let _ = tokio::time::timeout(Duration::from_secs(5), run.changed.notified()).await;
            }
        }
    });
    Ok(Sse::new(ReceiverStream::new(receiver)).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

/// Asks a run to stop. It ends as cancelled at its next step.
pub(crate) async fn cancel_agent_run(
    subject: AuthenticatedSubject,
    Path(run_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let run = run_of(&run_id, &subject)?;
    run.cancel.store(true, Ordering::Relaxed);
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_keeps_what_it_sent_so_a_stream_that_opens_late_can_replay_it() {
        let run = LiveRun {
            owner: "u1".to_owned(),
            workspace_id: "w1".to_owned(),
            events: Mutex::new(Vec::new()),
            finished: AtomicBool::new(false),
            cancel: Arc::new(AtomicBool::new(false)),
            changed: Notify::new(),
        };
        run.push("step", json!({ "summary": "Search evidence: retries" }));
        run.push("done", json!({ "turn": {} }));
        let events = run.events.lock().unwrap().clone();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, "step");
        assert!(events[0].1.contains("Search evidence"));
        assert_eq!(events[1].0, "done");
    }
}
