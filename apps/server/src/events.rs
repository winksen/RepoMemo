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
        // A broadcast channel only delivers to receivers that already exist,
        // so a workspace nobody is watching needs no channel at all.
        let sender = self
            .channels
            .lock()
            .ok()
            .and_then(|channels| channels.get(event.workspace_id()).cloned());
        if let Some(sender) = sender {
            // Fails only when every subscriber left; `prune` drops the channel.
            let _ = sender.send(event);
        }
    }

    /// Drops the channels of workspaces nobody is subscribed to any more.
    /// Returns how many were dropped.
    pub fn prune(&self) -> usize {
        let Ok(mut channels) = self.channels.lock() else {
            return 0;
        };
        let before = channels.len();
        channels.retain(|_, sender| sender.receiver_count() > 0);
        before - channels.len()
    }

    /// Channels open, and subscribers across them.
    pub fn stats(&self) -> (usize, usize) {
        self.channels
            .lock()
            .map(|channels| {
                (
                    channels.len(),
                    channels.values().map(|sender| sender.receiver_count()).sum(),
                )
            })
            .unwrap_or((0, 0))
    }

    #[cfg(test)]
    fn channel_count(&self) -> usize {
        self.channels.lock().map(|channels| channels.len()).unwrap_or(0)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn activity(workspace_id: &str) -> WorkspaceEvent {
        WorkspaceEvent::Activity {
            event: WorkspaceActivityEvent {
                id: "event".to_owned(),
                workspace_id: workspace_id.to_owned(),
                actor: None,
                action: "test".to_owned(),
                subject_type: "workspace".to_owned(),
                subject_id: None,
                summary: "Test".to_owned(),
                created_at: "2026-01-01T00:00:00Z".to_owned(),
            },
        }
    }

    #[test]
    fn events_reach_subscribers_and_idle_channels_are_dropped() {
        let bus = WorkspaceEventBus::new();
        bus.publish(activity("unwatched"));
        assert_eq!(bus.channel_count(), 0, "publishing creates no channel");

        let mut receiver = bus.subscribe("watched");
        bus.publish(activity("watched"));
        assert!(matches!(receiver.try_recv(), Ok(WorkspaceEvent::Activity { .. })));
        assert_eq!(bus.prune(), 0);

        drop(receiver);
        assert_eq!(bus.prune(), 1);
        assert_eq!(bus.channel_count(), 0);
    }
}
