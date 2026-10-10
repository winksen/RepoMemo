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

#[derive(Clone)]
pub struct WorkspaceEventBus {
    channels: Arc<Mutex<HashMap<String, broadcast::Sender<WorkspaceEvent>>>>,
    /// Every workspace's job events, for the System jobs page.
    system_jobs: broadcast::Sender<WorkspaceEvent>,
}

impl Default for WorkspaceEventBus {
    fn default() -> Self {
        Self {
            channels: Arc::default(),
            system_jobs: broadcast::channel(CHANNEL_CAPACITY * 2).0,
        }
    }
}

impl WorkspaceEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self, workspace_id: &str) -> broadcast::Receiver<WorkspaceEvent> {
        self.sender_for(workspace_id).subscribe()
    }

    /// Job events of every workspace. Activity stays per workspace: app
    /// administrators may watch jobs without reading every workspace.
    pub fn subscribe_system_jobs(&self) -> broadcast::Receiver<WorkspaceEvent> {
        self.system_jobs.subscribe()
    }

    pub fn publish(&self, event: WorkspaceEvent) {
        if matches!(event, WorkspaceEvent::Job { .. }) {
            // Fails only when nobody watches the System jobs page.
            let _ = self.system_jobs.send(event.clone());
        }
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

    /// Channels open, and subscribers across them (System jobs watchers
    /// included).
    pub fn stats(&self) -> (usize, usize) {
        let system = self.system_jobs.receiver_count();
        self.channels
            .lock()
            .map(|channels| {
                (
                    channels.len(),
                    channels.values().map(|sender| sender.receiver_count()).sum::<usize>() + system,
                )
            })
            .unwrap_or((0, system))
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
        log_job(job);
        self.bus.publish(WorkspaceEvent::Job { job: job.clone() });
    }
}

/// Logs a job's start and outcome under the `jobs` target (progress
/// updates are not logged).
fn log_job(job: &IndexingJobStatus) {
    let kind = if job.kind.is_empty() { "indexing" } else { job.kind.as_str() };
    match job.status.as_str() {
        "running" if job.created_at == job.updated_at => {
            tracing::debug!(target: "jobs", workspace_id = %job.workspace_id, job_id = %job.id, kind, "Job started");
        }
        "completed" => {
            tracing::info!(target: "jobs", workspace_id = %job.workspace_id, job_id = %job.id, kind, items = job.progress_current, "Job completed");
        }
        "cancelled" => {
            tracing::info!(target: "jobs", workspace_id = %job.workspace_id, job_id = %job.id, kind, items = job.progress_current, "Job cancelled");
        }
        "failed" => {
            tracing::warn!(target: "jobs", workspace_id = %job.workspace_id, job_id = %job.id, kind, error = job.error_message.as_deref().unwrap_or(""), "Job failed");
        }
        _ => {}
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
        let actor = event
            .actor
            .as_ref()
            .map(|actor| actor.email.clone().unwrap_or_else(|| actor.display_name.clone()))
            .unwrap_or_else(|| "system".to_owned());
        tracing::info!(
            target: "activity",
            workspace_id = %event.workspace_id,
            action = %event.action,
            actor = %actor,
            subject = %event.subject_id.as_deref().unwrap_or(""),
            "{}",
            event.summary
        );
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

    #[test]
    fn the_system_channel_carries_jobs_of_every_workspace_but_no_activity() {
        let bus = WorkspaceEventBus::new();
        let mut system = bus.subscribe_system_jobs();
        bus.publish(activity("one"));
        bus.publish(WorkspaceEvent::Job {
            job: IndexingJobStatus {
                id: "job".to_owned(),
                workspace_id: "two".to_owned(),
                source_id: None,
                kind: "indexing".to_owned(),
                status: "running".to_owned(),
                stage: "chunking".to_owned(),
                progress_current: 0,
                progress_total: None,
                error_message: None,
                cancel_requested: false,
                created_at: "2026-01-01T00:00:00Z".to_owned(),
                updated_at: "2026-01-01T00:00:00Z".to_owned(),
            },
        });
        assert!(matches!(system.try_recv(), Ok(WorkspaceEvent::Job { .. })));
        assert!(system.try_recv().is_err(), "activity is not sent to the system channel");
        assert_eq!(bus.channel_count(), 0, "unwatched workspaces still get no channel");
    }
}
