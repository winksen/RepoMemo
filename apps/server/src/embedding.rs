//! Background embedding for semantic search.
//!
//! After evidence is indexed, or when a workspace's embedding provider is
//! saved, the workspace is queued here and every chunk without a vector from
//! the current model is embedded. Requests for a workspace that is already
//! waiting or running are coalesced, so a burst of uploads costs one pass.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};

use repomemo_api::{EmbeddingRun, RepoMemoCore};
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct EmbeddingQueue {
    sender: mpsc::UnboundedSender<String>,
    waiting: Arc<Mutex<HashMap<String, Instant>>>,
    core: RepoMemoCore,
}

impl EmbeddingQueue {
    /// Starts the worker on the current Tokio runtime. Up to `concurrency`
    /// workspaces are embedded at once (one by default, so a local embedding
    /// model is not overloaded).
    pub fn start(core: RepoMemoCore, concurrency: usize) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let waiting = Arc::new(Mutex::new(HashMap::new()));
        tokio::spawn(run_worker(
            core.clone(),
            waiting.clone(),
            sender.clone(),
            receiver,
            concurrency.max(1),
        ));
        Self { sender, waiting, core }
    }

    /// Queues a workspace unless it is already waiting. A workspace without an
    /// embedding provider is a cheap no-op when its turn comes.
    pub fn request(&self, workspace_id: &str) {
        let newly_waiting = self
            .waiting
            .lock()
            .map(|mut waiting| {
                if waiting.contains_key(workspace_id) {
                    false
                } else {
                    waiting.insert(workspace_id.to_owned(), Instant::now());
                    true
                }
            })
            .unwrap_or(false);
        if newly_waiting && self.sender.send(workspace_id.to_owned()).is_err() {
            tracing::error!(workspace_id, "Embedding queue is closed");
        }
    }

    /// Workspaces waiting for an embedding pass.
    pub fn waiting_count(&self) -> usize {
        self.waiting.lock().map(|waiting| waiting.len()).unwrap_or(0)
    }

    pub fn oldest_waiting_seconds(&self) -> Option<u64> {
        self.waiting
            .lock()
            .ok()?
            .values()
            .map(|queued| queued.elapsed().as_secs())
            .max()
    }

    /// Queues every workspace, so chunks indexed while the server was down or
    /// the provider unreachable are embedded after a restart.
    pub async fn resume_pending(&self) {
        match self.core.list_workspaces().await {
            Ok(workspaces) => {
                for workspace in workspaces {
                    self.request(&workspace.id);
                }
            }
            Err(error) => tracing::error!(error = %error, "Failed to list workspaces for embedding"),
        }
    }
}

async fn run_worker(
    core: RepoMemoCore,
    waiting: Arc<Mutex<HashMap<String, Instant>>>,
    sender: mpsc::UnboundedSender<String>,
    mut receiver: mpsc::UnboundedReceiver<String>,
    concurrency: usize,
) {
    let permits = Arc::new(tokio::sync::Semaphore::new(concurrency));
    while let Some(workspace_id) = receiver.recv().await {
        let Ok(permit) = permits.clone().acquire_owned().await else {
            break;
        };
        // Leave the waiting set before running, so chunks indexed during this
        // pass queue another one instead of being dropped.
        if let Ok(mut waiting) = waiting.lock() {
            waiting.remove(&workspace_id);
        }
        let core = core.clone();
        let waiting = waiting.clone();
        let sender = sender.clone();
        tokio::spawn(async move {
            if embed_workspace(&core, &workspace_id).await {
                let newly_waiting = waiting
                    .lock()
                    .map(|mut waiting| {
                        if waiting.contains_key(&workspace_id) {
                            false
                        } else {
                            waiting.insert(workspace_id.clone(), Instant::now());
                            true
                        }
                    })
                    .unwrap_or(false);
                if newly_waiting {
                    let _ = sender.send(workspace_id);
                }
            }
            drop(permit);
        });
    }
}

/// A pass processes at most 64 chunks before yielding the worker to another workspace.
async fn embed_workspace(core: &RepoMemoCore, workspace_id: &str) -> bool {
    match core.embed_missing_chunks_limited(workspace_id, Some(2)).await {
        Ok(EmbeddingRun::Embedded(count)) => {
            tracing::info!(workspace_id = %workspace_id, count, "Embedded chunks for semantic search");
        }
        Ok(EmbeddingRun::Yielded(count)) => {
            tracing::info!(workspace_id = %workspace_id, count, "Embedding pass yielded to another workspace");
            return true;
        }
        Ok(EmbeddingRun::Cancelled(count)) => {
            tracing::info!(workspace_id = %workspace_id, count, "Embedding cancelled");
        }
        Ok(EmbeddingRun::BudgetSpent(count)) => {
            tracing::warn!(target: "audit", workspace_id = %workspace_id, count, "Embedding paused: the workspace used its daily allowance of background AI calls");
        }
        Ok(EmbeddingRun::NotConfigured | EmbeddingRun::UpToDate) => {}
        Err(error) => {
            // The job records the reason; the next indexed file, provider
            // change or restart tries again.
            tracing::warn!(workspace_id = %workspace_id, error = %format!("{error:#}"), "Embedding failed");
        }
    }
    false
}
