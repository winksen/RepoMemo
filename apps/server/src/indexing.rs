//! Background indexing for shared evidence.
//!
//! Notes and uploads are indexed automatically after they are stored, so no
//! one has to press an "index" button per file. The queue lives in the server
//! process; anything left unindexed by a restart is picked up again by
//! [`IndexQueue::resume_pending`].

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};

use repomemo_api::{PendingArtifactIndex, RepoMemoCore};
use repomemo_domain::{ArtifactSummary, IndexingJobStatus};
use tokio::sync::{mpsc, Semaphore};

use crate::embedding::EmbeddingQueue;

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

/// A person's re-index of one file: its job (made when it was asked for, so the
/// answer can carry it) and, until a worker picks the file up, the work itself.
/// The entry stays until the work is done, so asking again meanwhile gets the
/// same job instead of starting another.
struct ManualIndex {
    job: IndexingJobStatus,
    pending: Option<PendingArtifactIndex>,
    actor_user_id: String,
}

type ManualIndexes = Arc<tokio::sync::Mutex<HashMap<String, ManualIndex>>>;

#[derive(Clone)]
pub struct IndexQueue {
    sender: mpsc::UnboundedSender<IndexRequest>,
    queued: Arc<Mutex<HashSet<String>>>,
    manual: ManualIndexes,
    core: RepoMemoCore,
}

impl IndexQueue {
    /// Starts the queue worker on the current Tokio runtime. The worker stops
    /// once every clone of the queue has been dropped. Each indexed artifact
    /// asks `embeddings` to embed its workspace's new chunks.
    ///
    /// Indexing can call a vision provider, so `concurrency` caps how many
    /// artifacts are processed at once instead of one task per upload.
    pub fn start(core: RepoMemoCore, embeddings: EmbeddingQueue, concurrency: usize) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let queued = Arc::new(Mutex::new(HashSet::new()));
        let manual = ManualIndexes::default();
        tokio::spawn(run_worker(
            core.clone(),
            embeddings,
            queued.clone(),
            manual.clone(),
            sender.clone(),
            receiver,
            concurrency.max(1),
        ));
        Self {
            sender,
            queued,
            manual,
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
        queue_request(&self.queued, &self.sender, artifact, actor_user_id);
    }

    /// A person's request to index one file again. The file goes through this
    /// queue, behind the same cap as every other, and the answer is the job that
    /// will carry its progress. Asking again while that job is waiting or running
    /// gives the same job, so ten requests make one.
    pub async fn enqueue_manual(&self, artifact_id: &str, actor_user_id: &str) -> anyhow::Result<IndexingJobStatus> {
        // Held across the job's creation, so two requests at once make one job.
        let mut manual = self.manual.lock().await;
        if let Some(entry) = manual.get(artifact_id) {
            return Ok(entry.job.clone());
        }
        let pending = self.core.begin_index_artifact(artifact_id.to_owned()).await?;
        self.core
            .storage()
            .set_job_started_by(&pending.job.id, actor_user_id)
            .await?;
        let job = pending.job.clone();
        let summary = pending.summary().clone();
        manual.insert(
            artifact_id.to_owned(),
            ManualIndex {
                job: job.clone(),
                pending: Some(pending),
                actor_user_id: actor_user_id.to_owned(),
            },
        );
        drop(manual);
        // If the file is already waiting (or running) for an automatic pass, that
        // pass takes this work when its turn comes, or the worker queues it again.
        queue_request(&self.queued, &self.sender, &summary, Some(actor_user_id));
        Ok(job)
    }

    /// Artifacts waiting for indexing or for a retry.
    pub fn queued_count(&self) -> usize {
        self.queued.lock().map(|queued| queued.len()).unwrap_or(0)
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

/// Puts a file in the queue unless it is already waiting or running there.
fn queue_request(
    queued: &Mutex<HashSet<String>>,
    sender: &mpsc::UnboundedSender<IndexRequest>,
    artifact: &ArtifactSummary,
    actor_user_id: Option<&str>,
) {
    let newly_queued = queued
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
    if sender.send(request).is_err() {
        tracing::error!(artifact_id = %artifact.id, "Indexing queue is closed");
        forget(queued, &artifact.id);
    }
}

async fn run_worker(
    core: RepoMemoCore,
    embeddings: EmbeddingQueue,
    queued: Arc<Mutex<HashSet<String>>>,
    manual: ManualIndexes,
    sender: mpsc::UnboundedSender<IndexRequest>,
    mut receiver: mpsc::UnboundedReceiver<IndexRequest>,
    concurrency: usize,
) {
    let permits = Arc::new(Semaphore::new(concurrency));
    while let Some(request) = receiver.recv().await {
        let Ok(permit) = permits.clone().acquire_owned().await else {
            break;
        };
        let core = core.clone();
        let embeddings = embeddings.clone();
        let queued = queued.clone();
        let manual = manual.clone();
        let sender = sender.clone();
        tokio::spawn(async move {
            let retry_pending = index_one(&core, &embeddings, &request, &sender, &manual).await;
            // A request waiting for its retry stays "queued" so duplicates are
            // not added; the permit is released so other files keep moving.
            if !retry_pending {
                forget(&queued, &request.artifact_id);
                // A person's request that arrived after this file was picked up
                // has not been taken by anyone: queue the file again for it.
                let waiting = {
                    let mut manual = manual.lock().await;
                    match manual.get(&request.artifact_id) {
                        Some(entry) => match entry.pending.as_ref() {
                            Some(pending) => Some((pending.summary().clone(), entry.actor_user_id.clone())),
                            None => {
                                manual.remove(&request.artifact_id);
                                None
                            }
                        },
                        None => None,
                    }
                };
                if let Some((summary, actor)) = waiting {
                    queue_request(&queued, &sender, &summary, Some(&actor));
                }
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
    manual: &ManualIndexes,
) -> bool {
    // A person asked for this file and has its job already: do that work, with
    // no retry (they ask again if it fails) and an activity entry in their name.
    let claimed = manual.lock().await.get_mut(&request.artifact_id).and_then(|entry| {
        entry
            .pending
            .take()
            .map(|pending| (pending, entry.actor_user_id.clone()))
    });
    if let Some((pending, actor)) = claimed {
        match core.finish_index_artifact(pending).await {
            Ok(_) => {
                embeddings.request(&request.workspace_id);
                if let Err(error) = core
                    .storage()
                    .record_workspace_activity(
                        &request.workspace_id,
                        Some(&actor),
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
            // The job already carries the reason; the log is for the operator.
            Err(error) => tracing::warn!(error = %error, artifact_id = %request.artifact_id, "Manual indexing failed"),
        }
        manual.lock().await.remove(&request.artifact_id);
        return false;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// V0.2.151: a person's re-index of one file is one job however often it is
    /// asked for while the file waits, and the job remembers who started it.
    #[tokio::test]
    async fn ten_requests_for_one_file_make_one_job() {
        let data_dir = std::env::temp_dir().join(format!("repomemo-index-queue-{}", uuid::Uuid::new_v4()));
        let core = RepoMemoCore::boot(data_dir.clone()).await.unwrap();
        let workspace = core.create_workspace("Queue".to_owned()).await.unwrap();
        let artifact = core
            .import_text(
                workspace.id.clone(),
                "Queued".to_owned(),
                "One passage.".to_owned(),
                Some("Markdown".to_owned()),
            )
            .await
            .unwrap();
        let other = core
            .import_text(
                workspace.id.clone(),
                "Other".to_owned(),
                "Another passage.".to_owned(),
                Some("Markdown".to_owned()),
            )
            .await
            .unwrap();

        // A queue whose worker has not started: the requests stay where they were put.
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let queue = IndexQueue {
            sender,
            queued: Arc::default(),
            manual: ManualIndexes::default(),
            core: core.clone(),
        };
        let first = queue.enqueue_manual(&artifact.id, "user-1").await.unwrap();
        for _ in 0..9 {
            let again = queue.enqueue_manual(&artifact.id, "user-2").await.unwrap();
            assert_eq!(again.id, first.id, "the same job comes back while the file waits");
        }
        assert_eq!(receiver.try_recv().unwrap().artifact_id, artifact.id);
        assert!(receiver.try_recv().is_err(), "the file is in the queue once");
        let jobs = core
            .storage()
            .list_workspace_jobs(&workspace.id, Some("indexing"), None, 50)
            .await
            .unwrap();
        assert_eq!(jobs.len(), 1, "one job for ten requests");
        assert_eq!(
            core.storage().job_started_by(&first.id).await.unwrap().as_deref(),
            Some("user-1")
        );

        // Another file has a job of its own.
        let second = queue.enqueue_manual(&other.id, "user-2").await.unwrap();
        assert_ne!(second.id, first.id);
        assert_eq!(
            core.storage().job_started_by(&second.id).await.unwrap().as_deref(),
            Some("user-2")
        );

        drop(queue);
        drop(core);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
