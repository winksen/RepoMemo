use anyhow::{bail, Result};
use repomemo_domain::{SearchRequest, SearchResult};
use repomemo_storage::StorageEngine;

#[derive(Debug, Clone)]
pub struct RetrievalService {
    storage: StorageEngine,
}

impl RetrievalService {
    pub fn new(storage: StorageEngine) -> Self {
        Self { storage }
    }

    pub async fn search(&self, request: SearchRequest) -> Result<Vec<SearchResult>> {
        if request.workspace_id.trim().is_empty() {
            bail!("Workspace id is required.");
        }

        let Some(fts_query) = prepare_fts_query(&request.query) else {
            return Ok(Vec::new());
        };

        self.storage.search_chunks(&request, &fts_query).await
    }

    pub async fn hybrid_search(
        &self,
        request: SearchRequest,
        query_embedding: Option<&[f32]>,
    ) -> Result<(Vec<SearchResult>, bool)> {
        let mut fts_results = self.search(request.clone()).await?;
        let mut used_embeddings = false;
        if let Some(query_embedding) = query_embedding {
            let semantic = self
                .storage
                .search_chunks_by_embedding(
                    &request.workspace_id,
                    query_embedding,
                    request.limit.unwrap_or(12),
                )
                .await?;
            if !semantic.is_empty() {
                used_embeddings = true;
                for (rank, result) in semantic.into_iter().enumerate() {
                    if let Some(existing) = fts_results
                        .iter_mut()
                        .find(|item| item.chunk_id == result.chunk_id)
                    {
                        existing.score += 1.0 / (rank + 1) as f64;
                    } else {
                        let mut result = result;
                        result.score = 0.5 + 1.0 / (rank + 1) as f64;
                        fts_results.push(result);
                    }
                }
            }
        }
        fts_results.sort_by(|left, right| right.score.total_cmp(&left.score));
        fts_results.truncate(request.limit.unwrap_or(12).clamp(1, 40) as usize);
        Ok((fts_results, used_embeddings))
    }
}

pub fn prepare_fts_query(query: &str) -> Option<String> {
    let tokens = query
        .split_whitespace()
        .map(|token| {
            token
                .chars()
                .filter(|character| character.is_alphanumeric() || *character == '_')
                .collect::<String>()
        })
        .filter(|token| !token.is_empty())
        .take(12)
        .map(|token| format!("\"{token}\"*"))
        .collect::<Vec<_>>();

    (!tokens.is_empty()).then(|| tokens.join(" AND "))
}

#[cfg(test)]
mod tests {
    use super::prepare_fts_query;

    #[test]
    fn creates_safe_prefix_query_for_multiple_terms() {
        assert_eq!(
            prepare_fts_query("artifact search"),
            Some("\"artifact\"* AND \"search\"*".to_owned())
        );
    }

    #[test]
    fn strips_fts_operators_and_empty_input() {
        assert_eq!(prepare_fts_query(" OR *** "), Some("\"OR\"*".to_owned()));
        assert_eq!(prepare_fts_query(" -- "), None);
    }
}
