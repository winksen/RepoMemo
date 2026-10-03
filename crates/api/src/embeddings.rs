//! Semantic search vectors. A workspace with an enabled `embedding` provider
//! gets every chunk embedded in the background; search and Ask then combine
//! keyword and nearest-neighbour results. Without one, search stays keyword
//! only and nothing leaves the server.

use anyhow::{bail, Result};
use repomemo_ai::{provider_from_settings, AiProvider};
use repomemo_domain::ProviderSettings;
use repomemo_storage::EmbeddingCandidate;
use serde_json::json;

use crate::RepoMemoCore;

/// Chunks sent to the provider per request.
const EMBED_BATCH: i64 = 32;
/// Longest text embedded per chunk. Embedding models truncate long input on
/// their own, often far earlier; this keeps requests a sane size.
const EMBED_TEXT_CHARS: usize = 6_000;

/// Result of a background embedding pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingRun {
    /// No enabled embedding provider; nothing to do.
    NotConfigured,
    /// Every chunk already had a vector from the current model.
    UpToDate,
    Embedded(usize),
    Cancelled(usize),
}

impl RepoMemoCore {
    /// The enabled provider set up for embeddings, if any.
    pub async fn embedding_provider_for_workspace(
        &self,
        workspace_id: &str,
    ) -> Result<Option<ProviderSettings>> {
        Ok(self
            .storage
            .list_provider_settings(workspace_id)
            .await?
            .into_iter()
            .find(|settings| settings.enabled && settings.purpose() == "embedding"))
    }

    /// `(embedded, total)` chunk counts for the current embedding model, or
    /// `None` when the workspace has no embedding provider.
    pub async fn embedding_coverage(&self, workspace_id: &str) -> Result<Option<(i64, i64)>> {
        match self.embedding_provider_for_workspace(workspace_id).await? {
            Some(settings) => Ok(Some(
                self.storage
                    .embedding_coverage(workspace_id, settings.embedding_model_name())
                    .await?,
            )),
            None => Ok(None),
        }
    }

    /// Embeds every chunk that has no vector from the current embedding
    /// model, as a tracked `embedding` job. Chunks whose text did not change
    /// keep their vector across re-indexing, so after the first pass only new
    /// or edited passages are sent.
    pub async fn embed_missing_chunks(&self, workspace_id: &str) -> Result<EmbeddingRun> {
        let Some(settings) = self.embedding_provider_for_workspace(workspace_id).await? else {
            return Ok(EmbeddingRun::NotConfigured);
        };
        let model = settings.embedding_model_name().to_owned();
        let (embedded, total) = self.storage.embedding_coverage(workspace_id, &model).await?;
        let missing = total - embedded;
        if missing <= 0 {
            return Ok(EmbeddingRun::UpToDate);
        }
        let provider = provider_from_settings(settings)?;
        let job = self
            .storage
            .create_job(workspace_id, None, "embedding", "embedding_chunks", Some(missing))
            .await?;
        let mut completed = 0_usize;
        loop {
            if self.storage.is_job_cancel_requested(&job.id).await? {
                self.storage
                    .update_indexing_job(&job.id, "cancelled", "cancelled", completed as i64, None)
                    .await?;
                return Ok(EmbeddingRun::Cancelled(completed));
            }
            let batch = self
                .storage
                .chunks_missing_embeddings(workspace_id, &model, EMBED_BATCH)
                .await?;
            if batch.is_empty() {
                break;
            }
            let texts = batch.iter().map(embedding_text).collect::<Vec<_>>();
            let vectors = match provider.embed(texts, json!({})).await {
                Ok(vectors) => vectors,
                Err(error) => {
                    let reason = format!("{error:#}");
                    self.storage
                        .update_indexing_job(&job.id, "failed", "embedding_failed", completed as i64, Some(&reason))
                        .await?;
                    bail!("{reason}");
                }
            };
            let count = batch.len();
            self.storage
                .upsert_embeddings(
                    workspace_id,
                    &model,
                    batch.into_iter().map(|candidate| candidate.chunk_id).zip(vectors).collect(),
                )
                .await?;
            completed += count;
            self.storage
                .update_indexing_job(&job.id, "running", "embedding_chunks", completed as i64, None)
                .await?;
        }
        self.storage
            .update_indexing_job(&job.id, "completed", "embedded_chunks", completed as i64, None)
            .await?;
        Ok(EmbeddingRun::Embedded(completed))
    }

    /// The vector of a search query and the model that produced it. Uses the
    /// workspace's embedding provider, or else `fallback_provider_id` when it
    /// is a provider saved without a purpose (the single-provider desktop
    /// setup, which embeds with its text provider). `Ok(None)` means semantic
    /// search is not set up; `Err` is a provider failure.
    pub(crate) async fn query_embedding(
        &self,
        workspace_id: &str,
        fallback_provider_id: Option<&str>,
        query: &str,
    ) -> Result<Option<(String, Vec<f32>)>> {
        let settings = match self.embedding_provider_for_workspace(workspace_id).await? {
            Some(settings) => settings,
            None => match fallback_provider_id {
                Some(provider_id) => {
                    let settings = self.storage.get_provider_settings(provider_id).await?;
                    if settings.has_explicit_purpose() || !settings.enabled {
                        return Ok(None);
                    }
                    settings
                }
                None => return Ok(None),
            },
        };
        let model = settings.embedding_model_name().to_owned();
        let provider = provider_from_settings(settings)?;
        let vector = provider
            .embed(vec![query.to_owned()], json!({}))
            .await?
            .into_iter()
            .next()
            .unwrap_or_default();
        Ok((!vector.is_empty()).then_some((model, vector)))
    }
}

/// The text embedded for a chunk: its file title and heading first, so a
/// passage is found by what it is about and not only by the words in it.
fn embedding_text(candidate: &EmbeddingCandidate) -> String {
    let mut text = candidate.title.clone();
    if let Some(heading) = candidate.heading_path.as_deref().filter(|heading| !heading.is_empty()) {
        text.push_str(" > ");
        text.push_str(heading);
    }
    text.push_str("\n\n");
    text.push_str(&candidate.text);
    text.chars().take(EMBED_TEXT_CHARS).collect()
}
