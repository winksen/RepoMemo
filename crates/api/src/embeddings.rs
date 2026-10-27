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
    /// This workspace used its fair turn; more chunks remain for the next pass.
    Yielded(usize),
    Cancelled(usize),
    /// The workspace used up its daily allowance of background AI calls; the
    /// rest is embedded after midnight UTC.
    BudgetSpent(usize),
}

/// What rebuilding a workspace's vectors would involve.
#[derive(Debug, Clone)]
pub struct EmbeddingRebuildPlan {
    /// The model that would embed the passages; `None` without an enabled embedding provider.
    pub model: Option<String>,
    pub passages: i64,
    /// Vectors stored now, from any model.
    pub stored_vectors: i64,
    /// Provider requests the pass makes (one per batch of passages).
    pub provider_calls: i64,
    /// The workspace's daily allowance of background AI calls (0 is unlimited).
    pub daily_limit: u32,
}

impl RepoMemoCore {
    /// The enabled provider set up for embeddings, if any.
    pub async fn embedding_provider_for_workspace(&self, workspace_id: &str) -> Result<Option<ProviderSettings>> {
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

    /// What rebuilding a workspace's vectors would cost, for a dry run.
    pub async fn embedding_rebuild_plan(&self, workspace_id: &str) -> Result<EmbeddingRebuildPlan> {
        let model = self
            .embedding_provider_for_workspace(workspace_id)
            .await?
            .map(|settings| settings.embedding_model_name().to_owned());
        let (_, passages) = self
            .storage
            .embedding_coverage(workspace_id, model.as_deref().unwrap_or(""))
            .await?;
        let stored_vectors = self.storage.embedding_count(workspace_id).await?;
        Ok(EmbeddingRebuildPlan {
            model,
            passages,
            stored_vectors,
            provider_calls: (passages + EMBED_BATCH - 1) / EMBED_BATCH,
            daily_limit: self.ai_budget.daily_limit(),
        })
    }

    /// Removes the workspace's stored vectors so the next embedding pass makes
    /// every one again. The caller asks the embedding queue to run it.
    pub async fn clear_embeddings(&self, workspace_id: &str) -> Result<u64> {
        self.storage.clear_workspace_embeddings(workspace_id).await
    }

    /// Files that the current indexer has not indexed yet (never, or by an
    /// older version): what a whole-workspace re-index would do.
    pub async fn stale_artifact_count(&self, workspace_id: &str) -> Result<usize> {
        Ok(self
            .storage
            .list_artifacts_needing_index(Some(workspace_id), repomemo_indexer::INDEXER_VERSION, true)
            .await?
            .len())
    }

    /// Embeds every chunk that has no vector from the current embedding
    /// model, as a tracked `embedding` job. Chunks whose text did not change
    /// keep their vector across re-indexing, so after the first pass only new
    /// or edited passages are sent.
    pub async fn embed_missing_chunks(&self, workspace_id: &str) -> Result<EmbeddingRun> {
        self.embed_missing_chunks_limited(workspace_id, None).await
    }

    /// Limit one pass so another workspace can use the embedding worker next.
    pub async fn embed_missing_chunks_limited(
        &self,
        workspace_id: &str,
        max_batches: Option<usize>,
    ) -> Result<EmbeddingRun> {
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
        let mut batches = 0_usize;
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
            if !self.ai_budget.try_spend(workspace_id, 1) {
                let reason = format!(
                    "The workspace reached its daily limit of {} background AI calls. Embedding continues tomorrow.",
                    self.ai_budget.daily_limit()
                );
                self.storage
                    .update_indexing_job(
                        &job.id,
                        "failed",
                        "daily_budget_reached",
                        completed as i64,
                        Some(&reason),
                    )
                    .await?;
                return Ok(EmbeddingRun::BudgetSpent(completed));
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
                    batch
                        .into_iter()
                        .map(|candidate| candidate.chunk_id)
                        .zip(vectors)
                        .collect(),
                )
                .await?;
            completed += count;
            batches += 1;
            self.storage
                .update_indexing_job(&job.id, "running", "embedding_chunks", completed as i64, None)
                .await?;
            if max_batches.is_some_and(|limit| batches >= limit.max(1)) && completed < missing as usize {
                self.storage
                    .update_indexing_job(
                        &job.id,
                        "completed",
                        "yielded_to_other_workspace",
                        completed as i64,
                        None,
                    )
                    .await?;
                return Ok(EmbeddingRun::Yielded(completed));
            }
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
