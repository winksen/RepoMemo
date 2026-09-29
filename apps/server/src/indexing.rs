//! Background indexing for shared evidence.
//!
//! Notes and uploads are indexed automatically after they are stored, so no
//! one has to press an "index" button per file. The queue lives in the server
//! process; anything left unindexed by a restart is picked up again by
//! [`IndexQueue::resume_pending`].

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use repomemo_api::RepoMemoCore;
use repomemo_domain::ArtifactSummary;
use tokio::sync::{mpsc, Semaphore};

/// Indexing can call a vision provider, so keep a small ceiling on how many
/// artifacts are processed at once instead of one task per upload.
const MAX_CONCURRENT_INDEXING: usize = 2;

#[derive(Debug)]
struct IndexRequest {
    artifact_id: String,
    workspace_id: String,
    title: String,
    actor_user_id: Option<String>,
}

#[derive(Clone)]
pub struct IndexQueue {
    sender: mpsc::UnboundedSender<IndexRequest>,
    queued: Arc<Mutex<HashSet<String>>>,
    core: RepoMemoCore,
}

impl IndexQueue {
    /// Starts the queue worker on the current Tokio runtime. The worker stops
    /// once every clone of the queue has been dropped.
    pub fn start(core: RepoMemoCore) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let queued = Arc::new(Mutex::new(HashSet::new()));
        tokio::spawn(run_worker(core.clone(), queued.clone(), receiver));
        Self {
            sender,
            queued,
            core,
        }
    }

    /// Schedules an artifact for indexing unless it is already indexed or
    /// already waiting in the queue.
    pub fn enqueue(&self, artifact: &ArtifactSummary, actor_user_id: Option<&str>) {
        if artifact.indexed_at.is_some() {
            return;
        }
        let newly_queued = self
            .queued
            .lock()
            .map(|mut queued| queued.insert(artifact.id.clone()))
            .unwrap_or(false);
        if !newly_queued {
            return;
        }
        let request = IndexRequest {
            artifact_id: artifact.id.clone(),
            workspace_id: artifact.workspace_id.clone(),
            title: artifact.title.clone(),
            actor_user_id: actor_user_id.map(str::to_owned),
        };
        if self.sender.send(request).is_err() {
            tracing::error!(artifact_id = %artifact.id, "Indexing queue is closed");
            forget(&self.queued, &artifact.id);
        }
    }

    /// Queues every artifact that was stored but never indexed, for example
    /// because the server stopped before the queue drained.
    pub async fn resume_pending(&self) {
        match self.core.storage().list_unindexed_artifacts().await {
            Ok(pending) => {
                if !pending.is_empty() {
                    tracing::info!(count = pending.len(), "Resuming pending evidence indexing");
                }
                for artifact in &pending {
                    self.enqueue(artifact, None);
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
    queued: Arc<Mutex<HashSet<String>>>,
    mut receiver: mpsc::UnboundedReceiver<IndexRequest>,
) {
    let permits = Arc::new(Semaphore::new(MAX_CONCURRENT_INDEXING));
    while let Some(request) = receiver.recv().await {
        let Ok(permit) = permits.clone().acquire_owned().await else {
            break;
        };
        let core = core.clone();
        let queued = queued.clone();
        tokio::spawn(async move {
            index_one(&core, &request).await;
            forget(&queued, &request.artifact_id);
            drop(permit);
        });
    }
}

async fn index_one(core: &RepoMemoCore, request: &IndexRequest) {
    match core.index_artifact(request.artifact_id.clone()).await {
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
        }
        // The core has already marked the job as failed. The artifact stays
        // unindexed and is retried on the next server start.
        Err(error) => tracing::warn!(
            error = %error,
            artifact_id = %request.artifact_id,
            "Automatic indexing failed"
        ),
    }
}

fn forget(queued: &Mutex<HashSet<String>>, artifact_id: &str) {
    if let Ok(mut queued) = queued.lock() {
        queued.remove(artifact_id);
    }
}
