//! Background embedding for semantic search.
//!
//! After evidence is indexed, or when a workspace's embedding provider is
//! saved, the workspace is queued here and every chunk without a vector from
//! the current model is embedded. Requests for a workspace that is already
//! waiting or running are coalesced, so a burst of uploads costs one pass.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use repomemo_api::{EmbeddingRun, RepoMemoCore};
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct EmbeddingQueue {
    sender: mpsc::UnboundedSender<String>,
    waiting: Arc<Mutex<HashSet<String>>>,
    core: RepoMemoCore,
}

impl EmbeddingQueue {
    /// Starts the worker on the current Tokio runtime. Workspaces are
    /// embedded one at a time so a local embedding model is not overloaded.
    pub fn start(core: RepoMemoCore) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let waiting = Arc::new(Mutex::new(HashSet::new()));
        tokio::spawn(run_worker(core.clone(), waiting.clone(), receiver));
        Self {
            sender,
            waiting,
            core,
        }
    }

    /// Queues a workspace unless it is already waiting. A workspace without an
    /// embedding provider is a cheap no-op when its turn comes.
    pub fn request(&self, workspace_id: &str) {
        let newly_waiting = self
            .waiting
            .lock()
            .map(|mut waiting| waiting.insert(workspace_id.to_owned()))
            .unwrap_or(false);
        if newly_waiting && self.sender.send(workspace_id.to_owned()).is_err() {
            tracing::error!(workspace_id, "Embedding queue is closed");
        }
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
    waiting: Arc<Mutex<HashSet<String>>>,
    mut receiver: mpsc::UnboundedReceiver<String>,
) {
    while let Some(workspace_id) = receiver.recv().await {
        // Leave the waiting set before running, so chunks indexed during this
        // pass queue another one instead of being dropped.
        if let Ok(mut waiting) = waiting.lock() {
            waiting.remove(&workspace_id);
        }
        match core.embed_missing_chunks(&workspace_id).await {
            Ok(EmbeddingRun::Embedded(count)) => {
                tracing::info!(workspace_id = %workspace_id, count, "Embedded chunks for semantic search");
            }
            Ok(EmbeddingRun::Cancelled(count)) => {
                tracing::info!(workspace_id = %workspace_id, count, "Embedding cancelled");
            }
            Ok(EmbeddingRun::NotConfigured | EmbeddingRun::UpToDate) => {}
            Err(error) => {
                // The job records the reason; the next indexed file, provider
                // change or restart tries again.
                tracing::warn!(workspace_id = %workspace_id, error = %format!("{error:#}"), "Embedding failed");
            }
        }
    }
}
