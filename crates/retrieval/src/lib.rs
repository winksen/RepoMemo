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
        self.keyword_search(request, KeywordMode::AllTerms).await
    }

    async fn keyword_search(
        &self,
        request: SearchRequest,
        mode: KeywordMode,
    ) -> Result<Vec<SearchResult>> {
        if request.workspace_id.trim().is_empty() {
            bail!("Workspace id is required.");
        }
        let fts_query = match mode {
            KeywordMode::AllTerms => prepare_fts_query(&request.query),
            KeywordMode::AnyTerm => prepare_question_fts_query(&request.query),
        };
        let Some(fts_query) = fts_query else {
            return Ok(Vec::new());
        };
        self.storage.search_chunks(&request, &fts_query).await
    }

    /// Keyword search, merged with nearest-neighbour search when a query
    /// embedding is given. Returns whether semantic results took part.
    ///
    /// The two lists are fused by rank (reciprocal rank fusion) because bm25
    /// and cosine scores live on unrelated scales. Each list is fetched deeper
    /// than the limit so a passage found by both can climb past one found by
    /// only one. The fused score is scaled to 0–1, where 1 means first in both.
    pub async fn hybrid_search(
        &self,
        request: SearchRequest,
        mode: KeywordMode,
        query_embedding: Option<(&str, &[f32])>,
    ) -> Result<(Vec<SearchResult>, bool)> {
        let limit = request.limit.unwrap_or(12).clamp(1, 100);
        let deep = SearchRequest {
            limit: Some((limit * 3).min(100)),
            ..request.clone()
        };
        let keyword = self.keyword_search(deep.clone(), mode).await?;
        let semantic = match query_embedding {
            Some((model, vector)) => {
                self.storage
                    .search_chunks_by_embedding(&deep, model, vector)
                    .await?
            }
            None => Vec::new(),
        };
        let used_embeddings = !semantic.is_empty();
        let mut fused = fuse_by_rank(keyword, semantic);
        fused.truncate(limit as usize);
        Ok((fused, used_embeddings))
    }
}

/// Smoothing constant of reciprocal rank fusion; 60 is the usual choice and
/// keeps a single top rank from dominating agreement between the lists.
const RRF_K: f64 = 60.0;

/// Merges ranked lists into one ordered by the sum of `1 / (k + rank)`. A
/// passage found by keyword search keeps its highlighted snippet.
pub fn fuse_by_rank(keyword: Vec<SearchResult>, semantic: Vec<SearchResult>) -> Vec<SearchResult> {
    let lists = usize::from(!keyword.is_empty()) + usize::from(!semantic.is_empty());
    let best = lists.max(1) as f64 / (RRF_K + 1.0);
    let mut fused: Vec<SearchResult> = Vec::new();
    for list in [keyword, semantic] {
        for (rank, mut result) in list.into_iter().enumerate() {
            let contribution = 1.0 / (RRF_K + rank as f64 + 1.0);
            match fused.iter_mut().find(|item| item.chunk_id == result.chunk_id) {
                Some(existing) => existing.score += contribution,
                None => {
                    result.score = contribution;
                    fused.push(result);
                }
            }
        }
    }
    for result in &mut fused {
        result.score = (result.score / best).clamp(0.0, 1.0);
    }
    fused.sort_by(|left, right| right.score.total_cmp(&left.score));
    fused
}

/// How keyword search treats the words of a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeywordMode {
    /// Every word must appear: precise, for typed search terms.
    AllTerms,
    /// Any meaningful word may appear, ranked by bm25 so passages matching
    /// more and rarer words come first: for natural-language questions, whose
    /// filler words would otherwise rule out every passage.
    AnyTerm,
}

pub fn prepare_fts_query(query: &str) -> Option<String> {
    let tokens = fts_tokens(query).take(12).map(|token| format!("\"{token}\"*")).collect::<Vec<_>>();
    (!tokens.is_empty()).then(|| tokens.join(" AND "))
}

/// Common English and French words that carry no topic, dropped from
/// questions before an any-word search.
const STOPWORDS: &[&str] = &[
    "a", "about", "an", "and", "are", "as", "at", "be", "by", "can", "could", "did", "do", "does",
    "for", "from", "has", "have", "how", "i", "in", "is", "it", "its", "me", "my", "of", "on",
    "or", "our", "should", "tell", "that", "the", "their", "there", "this", "to", "was", "we",
    "were", "what", "when", "where", "which", "who", "why", "will", "with", "would", "you",
    "au", "aux", "avec", "ce", "ces", "comment", "dans", "de", "des", "du", "elle", "en", "est",
    "et", "il", "la", "le", "les", "leur", "mon", "ne", "nous", "ou", "où", "par", "pas", "pour",
    "pourquoi", "quand", "que", "quel", "quelle", "qui", "sa", "se", "son", "sont", "sur", "un",
    "une", "vous",
];

pub fn prepare_question_fts_query(query: &str) -> Option<String> {
    let mut seen = Vec::new();
    for token in fts_tokens(query) {
        let lower = token.to_lowercase();
        if STOPWORDS.contains(&lower.as_str()) || seen.contains(&lower) {
            continue;
        }
        seen.push(lower);
        if seen.len() == 16 {
            break;
        }
    }
    let tokens = seen.iter().map(|token| format!("\"{token}\"*")).collect::<Vec<_>>();
    (!tokens.is_empty()).then(|| tokens.join(" OR "))
}

fn fts_tokens(query: &str) -> impl Iterator<Item = String> + '_ {
    query
        .split_whitespace()
        .map(|token| {
            token
                .chars()
                .filter(|character| character.is_alphanumeric() || *character == '_')
                .collect::<String>()
        })
        .filter(|token| !token.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{fuse_by_rank, prepare_fts_query, prepare_question_fts_query};

    #[test]
    fn questions_search_for_any_meaningful_word() {
        assert_eq!(
            prepare_question_fts_query("How do uploads fail over 10 MiB?"),
            Some("\"uploads\"* OR \"fail\"* OR \"over\"* OR \"10\"* OR \"mib\"*".to_owned())
        );
        assert_eq!(
            prepare_question_fts_query("Pourquoi les envois échouent ?"),
            Some("\"envois\"* OR \"échouent\"*".to_owned())
        );
        assert_eq!(prepare_question_fts_query("what is it?"), None);
    }
    use repomemo_domain::{ArtifactType, SearchResult};

    fn hit(chunk_id: &str) -> SearchResult {
        SearchResult {
            artifact_id: "a".to_owned(),
            chunk_id: chunk_id.to_owned(),
            title: chunk_id.to_owned(),
            path: chunk_id.to_owned(),
            artifact_type: ArtifactType::MarkdownDoc,
            language: None,
            snippet: String::new(),
            start_line: None,
            end_line: None,
            score: 99.0,
            source_name: "s".to_owned(),
        }
    }

    #[test]
    fn fusion_prefers_passages_found_by_both_lists() {
        let fused = fuse_by_rank(
            vec![hit("keyword-only"), hit("both")],
            vec![hit("semantic-only"), hit("both")],
        );
        let order = fused.iter().map(|result| result.chunk_id.as_str()).collect::<Vec<_>>();
        assert_eq!(order[0], "both");
        assert_eq!(order.len(), 3);
        assert!(fused.iter().all(|result| (0.0..=1.0).contains(&result.score)));
    }

    #[test]
    fn fusion_of_one_list_keeps_its_order_and_scales_to_one() {
        let fused = fuse_by_rank(vec![hit("first"), hit("second")], Vec::new());
        assert_eq!(fused[0].chunk_id, "first");
        assert!((fused[0].score - 1.0).abs() < 1e-9);
    }

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
