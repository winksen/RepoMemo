//! Per-workspace event bus used by the SSE endpoint.
//!
//! Storage-layer observers push job and activity changes through this bus;
//! SSE subscribers convert `broadcast::Receiver`s into HTTP streams. Every
//! workspace gets its own broadcast channel, created lazily on first
//! subscribe/publish. Slow subscribers that fall behind receive a `Lagged`
//! marker instead of blocking publishers.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use repomemo_domain::{IndexingJobStatus, WorkspaceActivityEvent};
use repomemo_storage::{ActivityObserver, JobObserver};
use serde::Serialize;
use tokio::sync::broadcast;

const CHANNEL_CAPACITY: usize = 128;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkspaceEvent {
    /// A job row was created or updated (indexing, embedding, future kinds).
    Job { job: IndexingJobStatus },
    /// A workspace activity row was recorded.
    Activity { event: WorkspaceActivityEvent },
}

impl WorkspaceEvent {
    fn workspace_id(&self) -> &str {
        match self {
            WorkspaceEvent::Job { job } => &job.workspace_id,
            WorkspaceEvent::Activity { event } => &event.workspace_id,
        }
    }

    pub fn event_name(&self) -> &'static str {
        match self {
            WorkspaceEvent::Job { .. } => "job",
            WorkspaceEvent::Activity { .. } => "activity",
        }
    }
}

#[derive(Clone, Default)]
pub struct WorkspaceEventBus {
    channels: Arc<Mutex<HashMap<String, broadcast::Sender<WorkspaceEvent>>>>,
}

impl WorkspaceEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self, workspace_id: &str) -> broadcast::Receiver<WorkspaceEvent> {
        self.sender_for(workspace_id).subscribe()
    }

    pub fn publish(&self, event: WorkspaceEvent) {
        let workspace_id = event.workspace_id().to_owned();
        let sender = self.sender_for(&workspace_id);
        // Ignore the "no active subscribers" error: it just means nobody is
        // listening right now. Broadcast keeps the last `CHANNEL_CAPACITY`
        // messages so a fresh subscriber sees them.
        let _ = sender.send(event);
    }

    fn sender_for(&self, workspace_id: &str) -> broadcast::Sender<WorkspaceEvent> {
        let mut guard = self.channels.lock().expect("event bus poisoned");
        guard
            .entry(workspace_id.to_owned())
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0)
            .clone()
    }
}

impl std::fmt::Debug for WorkspaceEventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceEventBus").finish_non_exhaustive()
    }
}

/// Bridges the storage crate's `JobObserver` trait to the event bus so the
/// api crate's job writes reach the SSE stream without knowing about it.
pub struct BusJobObserver {
    bus: WorkspaceEventBus,
}

impl BusJobObserver {
    pub fn new(bus: WorkspaceEventBus) -> Self {
        Self { bus }
    }
}

impl JobObserver for BusJobObserver {
    fn on_job_changed(&self, job: &IndexingJobStatus) {
        self.bus.publish(WorkspaceEvent::Job { job: job.clone() });
    }
}

pub struct BusActivityObserver {
    bus: WorkspaceEventBus,
}

impl BusActivityObserver {
    pub fn new(bus: WorkspaceEventBus) -> Self {
        Self { bus }
    }
}

impl ActivityObserver for BusActivityObserver {
    fn on_activity_recorded(&self, event: &WorkspaceActivityEvent) {
        self.bus.publish(WorkspaceEvent::Activity {
            event: event.clone(),
        });
    }
}
