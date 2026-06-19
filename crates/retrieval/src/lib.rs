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
