//! Background indexing for shared evidence.
//!
//! Notes and uploads are indexed automatically after they are stored, so no
//! one has to press an "index" button per file. The queue lives in the server
//! process; anything left unindexed by a restart is picked up again by
//! [`IndexQueue::resume_pending`].

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use repomemo_api::RepoMemoCore;
use repomemo_domain::ArtifactSummary;
use tokio::sync::{mpsc, Semaphore};

use crate::embedding::EmbeddingQueue;

/// Indexing can call a vision provider, so keep a small ceiling on how many
/// artifacts are processed at once instead of one task per upload.
const MAX_CONCURRENT_INDEXING: usize = 2;

/// A failed attempt is retried after each of these delays, then given up on so
/// an unreachable provider is not hammered forever. Giving up records the
/// reason for the shared client; the next server start or a provider change
/// tries again.
#[cfg(not(test))]
const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(30),
    Duration::from_secs(120),
    Duration::from_secs(600),
];
#[cfg(test)]
const RETRY_DELAYS: [Duration; 3] = [Duration::from_millis(20); 3];

#[derive(Debug, Clone)]
struct IndexRequest {
    /// Zero for the first try; counts retries already used.
    attempt: usize,
    artifact_id: String,
    workspace_id: String,
    title: String,
    actor_user_id: Option<String>,
    /// First-time indexing is worth an activity entry; a background refresh of
    /// an already indexed artifact is not.
    record_activity: bool,
}

#[derive(Clone)]
pub struct IndexQueue {
    sender: mpsc::UnboundedSender<IndexRequest>,
    queued: Arc<Mutex<HashSet<String>>>,
    core: RepoMemoCore,
}

impl IndexQueue {
    /// Starts the queue worker on the current Tokio runtime. The worker stops
    /// once every clone of the queue has been dropped. Each indexed artifact
    /// asks `embeddings` to embed its workspace's new chunks.
    pub fn start(core: RepoMemoCore, embeddings: EmbeddingQueue) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let queued = Arc::new(Mutex::new(HashSet::new()));
        tokio::spawn(run_worker(
            core.clone(),
            embeddings,
            queued.clone(),
            sender.clone(),
            receiver,
        ));
        Self {
            sender,
            queued,
            core,
        }
    }

    /// Schedules a newly stored artifact for indexing unless it is already
    /// indexed (a duplicate upload) or already waiting in the queue.
    pub fn enqueue(&self, artifact: &ArtifactSummary, actor_user_id: Option<&str>) {
        if artifact.indexed_at.is_some() {
            return;
        }
        self.push(artifact, actor_user_id);
    }

    fn push(&self, artifact: &ArtifactSummary, actor_user_id: Option<&str>) {
        let newly_queued = self
            .queued
            .lock()
            .map(|mut queued| queued.insert(artifact.id.clone()))
            .unwrap_or(false);
        if !newly_queued {
            return;
        }
        let request = IndexRequest {
            attempt: 0,
            artifact_id: artifact.id.clone(),
            workspace_id: artifact.workspace_id.clone(),
            title: artifact.title.clone(),
            actor_user_id: actor_user_id.map(str::to_owned),
            record_activity: artifact.indexed_at.is_none(),
        };
        if self.sender.send(request).is_err() {
            tracing::error!(artifact_id = %artifact.id, "Indexing queue is closed");
            forget(&self.queued, &artifact.id);
        }
    }

    /// Queues every artifact that was stored but never indexed (for example
    /// because the server stopped before the queue drained) and every artifact
    /// indexed by an older indexer version, so improvements to chunking reach
    /// existing evidence without anyone re-indexing by hand.
    pub async fn resume_pending(&self) {
        match self.core.artifacts_needing_index().await {
            Ok(pending) => {
                if !pending.is_empty() {
                    tracing::info!(count = pending.len(), "Resuming pending evidence indexing");
                }
                for artifact in &pending {
                    self.push(artifact, None);
                }
            }
            Err(error) => {
                tracing::error!(error = %error, "Failed to list artifacts waiting for indexing");
            }
        }
    }
}

async fn run_worker(
    core: RepoMemoCore,
    embeddings: EmbeddingQueue,
    queued: Arc<Mutex<HashSet<String>>>,
    sender: mpsc::UnboundedSender<IndexRequest>,
    mut receiver: mpsc::UnboundedReceiver<IndexRequest>,
) {
    let permits = Arc::new(Semaphore::new(MAX_CONCURRENT_INDEXING));
    while let Some(request) = receiver.recv().await {
        let Ok(permit) = permits.clone().acquire_owned().await else {
            break;
        };
        let core = core.clone();
        let embeddings = embeddings.clone();
        let queued = queued.clone();
        let sender = sender.clone();
        tokio::spawn(async move {
            let retry_pending = index_one(&core, &embeddings, &request, &sender).await;
            // A request waiting for its retry stays "queued" so duplicates are
            // not added; the permit is released so other files keep moving.
            if !retry_pending {
                forget(&queued, &request.artifact_id);
            }
            drop(permit);
        });
    }
}

/// Returns true when a retry was scheduled.
async fn index_one(
    core: &RepoMemoCore,
    embeddings: &EmbeddingQueue,
    request: &IndexRequest,
    sender: &mpsc::UnboundedSender<IndexRequest>,
) -> bool {
    let result = core.index_artifact(request.artifact_id.clone()).await;
    if result.is_ok() {
        embeddings.request(&request.workspace_id);
    }
    match result {
        Ok(_) if !request.record_activity => false,
        Ok(_) => {
            if let Err(error) = core
                .storage()
                .record_workspace_activity(
                    &request.workspace_id,
                    request.actor_user_id.as_deref(),
                    "artifact_indexed",
                    "artifact",
                    Some(&request.artifact_id),
                    &format!("Indexed evidence: {}.", request.title),
                )
                .await
            {
                tracing::error!(error = %error, "Failed to record indexing activity");
            }
            false
        }
        Err(error) => {
            let reason = format!("{error:#}");
            if let Some(delay) = RETRY_DELAYS.get(request.attempt).copied() {
                tracing::warn!(
                    error = %reason,
                    artifact_id = %request.artifact_id,
                    retry_in_secs = delay.as_secs_f32(),
                    "Automatic indexing failed; will retry"
                );
                let next = IndexRequest {
                    attempt: request.attempt + 1,
                    ..request.clone()
                };
                let sender = sender.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(delay).await;
                    let _ = sender.send(next);
                });
                return true;
            }
            tracing::warn!(
                error = %reason,
                artifact_id = %request.artifact_id,
                attempts = request.attempt + 1,
                "Automatic indexing failed; giving up until the next restart or provider change"
            );
            if let Err(store_error) = core
                .storage()
                .record_index_failure(&request.artifact_id, &reason, (request.attempt + 1) as i64)
                .await
            {
                tracing::error!(error = %store_error, "Failed to record indexing failure");
            }
            false
        }
    }
}

fn forget(queued: &Mutex<HashSet<String>>, artifact_id: &str) {
    if let Ok(mut queued) = queued.lock() {
        queued.remove(artifact_id);
    }
}
