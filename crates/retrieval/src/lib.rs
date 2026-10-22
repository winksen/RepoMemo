use std::collections::HashMap;

use anyhow::{bail, Result};
use repomemo_domain::{ArtifactType, SearchRequest, SearchResult};
use repomemo_storage::StorageEngine;

mod pattern;

pub use pattern::path_matches;

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

    async fn keyword_search(&self, request: SearchRequest, mode: KeywordMode) -> Result<Vec<SearchResult>> {
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
        profile: &RankingProfile,
    ) -> Result<(Vec<SearchResult>, bool)> {
        let limit = request.limit.unwrap_or(12).clamp(1, 100);
        let deep = SearchRequest {
            limit: Some((limit * 3).min(100)),
            ..request.clone()
        };
        let keyword = self.keyword_search(deep.clone(), mode).await?;
        let semantic = match query_embedding {
            Some((model, vector)) => self.storage.search_chunks_by_embedding(&deep, model, vector).await?,
            None => Vec::new(),
        };
        let used_embeddings = !semantic.is_empty();
        let mut fused = fuse_by_rank(keyword, semantic);
        self.rank(&request.workspace_id, profile, &mut fused).await;
        fused.truncate(limit as usize);
        Ok((fused, used_embeddings))
    }
}

impl RetrievalService {
    /// Weighs the results by the workspace's ranking profile. Evidence a person marked verified ranks a little above
    /// equally relevant passages: the boost is small enough that a clearly better match still wins.
    async fn rank(&self, workspace_id: &str, profile: &RankingProfile, results: &mut [SearchResult]) {
        if results.is_empty() || profile.is_neutral() {
            return;
        }
        let mut facts = RankingFacts::default();
        if profile.reads_lifecycle() {
            let Ok(states) = self.storage.workspace_lifecycle_states(workspace_id).await else {
                return;
            };
            facts.lifecycle = states
                .into_iter()
                .map(|(artifact_id, (status, _))| (artifact_id, status))
                .collect();
        }
        if profile.reads_ages() {
            let mut ids: Vec<String> = results.iter().map(|result| result.artifact_id.clone()).collect();
            ids.sort();
            ids.dedup();
            let Ok(times) = self.storage.artifact_updated_times(workspace_id, &ids).await else {
                return;
            };
            let now = chrono::Utc::now();
            facts.age_days = times
                .into_iter()
                .filter_map(|(artifact_id, updated_at)| {
                    let changed = chrono::DateTime::parse_from_rfc3339(&updated_at).ok()?;
                    Some((
                        artifact_id,
                        (now - changed.with_timezone(&chrono::Utc)).num_seconds().max(0) as f64 / 86_400.0,
                    ))
                })
                .collect();
        }
        apply_profile(results, profile, &facts);
    }
}

/// One rule of how search results are weighed once they are found.
#[derive(Debug, Clone, PartialEq)]
pub enum RankingRule {
    /// Passages of files in a lifecycle state (`verified`) have their score multiplied by `factor`.
    Lifecycle { status: String, factor: f64 },
    /// Passages of one kind of evidence (V0.2.179).
    ArtifactType { artifact_type: ArtifactType, factor: f64 },
    /// Passages of one kind of document, as the ledger's file types name them: `pdf`, `word`, `markdown`…
    DocumentKind { kind: String, factor: f64 },
    /// Passages of files whose path a folder pattern matches ([`path_matches`]).
    Path { pattern: String, factor: f64 },
    /// Recent evidence (V0.2.180): a file changed just now gets `max_factor`, and the preference halves every
    /// `half_life_days` until it is 1. A file whose age is not known gets 1.
    Recency { half_life_days: f64, max_factor: f64 },
}

impl RankingRule {
    /// Whether the rule can never change a score.
    fn is_neutral(&self) -> bool {
        match self {
            RankingRule::Lifecycle { factor, .. }
            | RankingRule::ArtifactType { factor, .. }
            | RankingRule::DocumentKind { factor, .. }
            | RankingRule::Path { factor, .. } => *factor == 1.0,
            RankingRule::Recency { max_factor, .. } => *max_factor == 1.0,
        }
    }

    /// What the rule makes of one passage: its factor if it matches, otherwise 1.
    fn weigh(&self, result: &SearchResult, facts: &RankingFacts) -> f64 {
        let (matches, factor) = match self {
            RankingRule::Lifecycle { status, factor } => {
                (facts.lifecycle.get(&result.artifact_id) == Some(status), *factor)
            }
            RankingRule::ArtifactType { artifact_type, factor } => (result.artifact_type == *artifact_type, *factor),
            RankingRule::DocumentKind { kind, factor } => {
                (document_kind(&result.artifact_type, &result.path) == kind, *factor)
            }
            RankingRule::Path { pattern, factor } => (path_matches(pattern, &result.path), *factor),
            RankingRule::Recency {
                half_life_days,
                max_factor,
            } => match facts.age_days.get(&result.artifact_id) {
                Some(age) => return recency_factor(*age, *half_life_days, *max_factor),
                None => (false, 1.0),
            },
        };
        if matches {
            factor
        } else {
            1.0
        }
    }
}

/// The preference for recent evidence: `max_factor` for a file changed just now, halving the part above 1 every
/// `half_life_days`. With a half-life of 90 days and a maximum of 1.2, a month-old file gets 1.16 and a year-old one 1.01.
pub fn recency_factor(age_days: f64, half_life_days: f64, max_factor: f64) -> f64 {
    if half_life_days <= 0.0 {
        return 1.0;
    }
    1.0 + (max_factor - 1.0) * 0.5_f64.powf(age_days.max(0.0) / half_life_days)
}

/// The product of the factors that match one passage is held between this and its inverse, so a pile of boosts
/// cannot bury a clearly better match (V0.2.179).
pub const MAX_PRODUCT: f64 = 1.5;

/// The kind of document a passage comes from, by the names the evidence ledger's file-type filter uses: a document
/// format by its extension first, then the evidence's own type, and plain text is whatever is left.
pub fn document_kind(artifact_type: &ArtifactType, path: &str) -> &'static str {
    if *artifact_type == ArtifactType::Repository {
        return "repository";
    }
    let extension = path.rsplit('.').next().map(str::to_ascii_lowercase).unwrap_or_default();
    match extension.as_str() {
        "doc" | "docx" => return "word",
        "xlsx" | "xlsm" | "xls" => return "excel",
        "pptx" | "ppt" => return "powerpoint",
        "pdf" => return "pdf",
        "one" => return "onenote",
        "eml" | "msg" => return "email",
        _ => {}
    }
    match artifact_type {
        ArtifactType::Image => "image",
        ArtifactType::CodeFile => "code",
        ArtifactType::Note => "note",
        ArtifactType::MarkdownDoc => "markdown",
        _ => "text",
    }
}

/// How a workspace weighs its search results (V0.2.178): a list of rules, each a factor for what it matches. The
/// product of the factors that match a passage multiplies its score, then scores are rescaled to at most 1. The
/// profile of a workspace on `general` is the one rule that has always been there: verified evidence ranks 10 % higher.
#[derive(Debug, Clone, PartialEq)]
pub struct RankingProfile {
    rules: Vec<RankingRule>,
}

impl RankingProfile {
    /// The ranking every workspace had before templates.
    pub fn general() -> Self {
        Self::with_verified(VERIFIED_BOOST)
    }

    /// Verified evidence by `factor`, and nothing else.
    pub fn with_verified(factor: f64) -> Self {
        Self {
            rules: vec![RankingRule::Lifecycle {
                status: "verified".to_owned(),
                factor,
            }],
        }
    }

    /// Adds a boost for one kind of evidence.
    pub fn boost_type(mut self, artifact_type: ArtifactType, factor: f64) -> Self {
        self.rules.push(RankingRule::ArtifactType { artifact_type, factor });
        self
    }

    /// Adds a boost for one kind of document.
    pub fn boost_document_kind(mut self, kind: &str, factor: f64) -> Self {
        self.rules.push(RankingRule::DocumentKind {
            kind: kind.to_owned(),
            factor,
        });
        self
    }

    /// Prefers recent evidence: see [`RankingRule::Recency`].
    pub fn prefer_recent(mut self, half_life_days: f64, max_factor: f64) -> Self {
        self.rules.push(RankingRule::Recency {
            half_life_days,
            max_factor,
        });
        self
    }

    /// Adds a boost for the files a folder pattern matches.
    pub fn boost_path(mut self, pattern: &str, factor: f64) -> Self {
        self.rules.push(RankingRule::Path {
            pattern: pattern.to_owned(),
            factor,
        });
        self
    }

    pub fn rules(&self) -> &[RankingRule] {
        &self.rules
    }

    /// The factor given to verified evidence, if the profile has that rule.
    pub fn verified_factor(&self) -> Option<f64> {
        self.rules.iter().find_map(|rule| match rule {
            RankingRule::Lifecycle { status, factor } if status == "verified" => Some(*factor),
            _ => None,
        })
    }

    /// True when no rule can change a score, so the results need no second look.
    pub fn is_neutral(&self) -> bool {
        self.rules.iter().all(RankingRule::is_neutral)
    }

    fn reads_lifecycle(&self) -> bool {
        self.rules
            .iter()
            .any(|rule| matches!(rule, RankingRule::Lifecycle { .. }))
    }

    fn reads_ages(&self) -> bool {
        self.rules
            .iter()
            .any(|rule| matches!(rule, RankingRule::Recency { .. }))
    }

    /// Whether the profile has any rule at all (the usual one included).
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// What the rules make of one passage: the product of the factors of the rules it matches, held between
    /// [`MAX_PRODUCT`] and its inverse.
    fn multiplier(&self, result: &SearchResult, facts: &RankingFacts) -> f64 {
        let product: f64 = self.rules.iter().map(|rule| rule.weigh(result, facts)).product();
        product.clamp(1.0 / MAX_PRODUCT, MAX_PRODUCT)
    }
}

impl Default for RankingProfile {
    fn default() -> Self {
        Self::general()
    }
}

/// What the rules look at for the files in a result list.
#[derive(Debug, Default)]
pub struct RankingFacts {
    /// Artifact id to its lifecycle status, for files whose lifecycle was ever set.
    pub lifecycle: HashMap<String, String>,
    /// Artifact id to how many days ago the file was last changed (V0.2.180).
    pub age_days: HashMap<String, f64>,
}

/// Multiplies the score of each passage by what the profile makes of it, then rescales so no score passes 1 and the
/// order of the rest is kept. A list the profile does not touch is left as it was.
pub fn apply_profile(results: &mut [SearchResult], profile: &RankingProfile, facts: &RankingFacts) {
    let multipliers: Vec<f64> = results.iter().map(|result| profile.multiplier(result, facts)).collect();
    if multipliers.iter().all(|multiplier| *multiplier == 1.0) {
        return;
    }
    for (result, multiplier) in results.iter_mut().zip(multipliers) {
        result.score *= multiplier;
    }
    let best = results.iter().map(|result| result.score).fold(1.0_f64, f64::max);
    for result in results.iter_mut() {
        result.score /= best;
    }
    results.sort_by(|left, right| right.score.total_cmp(&left.score));
}

/// How much a verified file's passages are favoured (10 %).
pub const VERIFIED_BOOST: f64 = 1.1;

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
    let tokens = fts_tokens(query)
        .take(12)
        .map(|token| format!("\"{token}\"*"))
        .collect::<Vec<_>>();
    (!tokens.is_empty()).then(|| tokens.join(" AND "))
}

/// Common English and French words that carry no topic, dropped from
/// questions before an any-word search.
const STOPWORDS: &[&str] = &[
    "a", "about", "an", "and", "are", "as", "at", "be", "by", "can", "could", "did", "do", "does", "for", "from",
    "has", "have", "how", "i", "in", "is", "it", "its", "me", "my", "of", "on", "or", "our", "should", "tell", "that",
    "the", "their", "there", "this", "to", "was", "we", "were", "what", "when", "where", "which", "who", "why", "will",
    "with", "would", "you", "au", "aux", "avec", "ce", "ces", "comment", "dans", "de", "des", "du", "elle", "en",
    "est", "et", "il", "la", "le", "les", "leur", "mon", "ne", "nous", "ou", "où", "par", "pas", "pour", "pourquoi",
    "quand", "que", "quel", "quelle", "qui", "sa", "se", "son", "sont", "sur", "un", "une", "vous",
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
            commit: None,
            last_author: None,
            last_changed_at: None,
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

    fn result(artifact: &str, chunk: &str, score: f64) -> SearchResult {
        SearchResult {
            artifact_id: artifact.to_owned(),
            score,
            ..hit(chunk)
        }
    }

    /// The facts of a workspace whose listed files are verified.
    fn verified(artifacts: &[&str]) -> super::RankingFacts {
        super::RankingFacts {
            lifecycle: artifacts
                .iter()
                .map(|artifact| ((*artifact).to_owned(), "verified".to_owned()))
                .collect(),
            ..Default::default()
        }
    }

    fn scores(results: &[SearchResult]) -> Vec<(String, f64)> {
        results.iter().map(|r| (r.chunk_id.clone(), r.score)).collect()
    }

    #[test]
    fn verified_passages_get_a_small_boost_that_keeps_scores_within_one() {
        use super::{apply_profile, RankingProfile};
        let general = RankingProfile::general();
        // Close scores: the verified file moves ahead.
        let mut close = vec![result("plain", "p", 0.80), result("checked", "v", 0.76)];
        apply_profile(&mut close, &general, &verified(&["checked"]));
        assert_eq!(close[0].chunk_id, "v");
        assert!(close.iter().all(|r| r.score <= 1.0));
        // A clearly better unverified match still wins.
        let mut far = vec![result("plain", "p", 1.0), result("checked", "v", 0.5)];
        apply_profile(&mut far, &general, &verified(&["checked"]));
        assert_eq!(far[0].chunk_id, "p");
        // Nothing verified: untouched.
        let mut none = vec![result("plain", "p", 0.4)];
        apply_profile(&mut none, &general, &verified(&[]));
        assert_eq!(none[0].score, 0.4);
    }

    /// V0.2.178: a workspace on General ranks exactly as V0.2.162 did. The expected figures below are what
    /// V0.2.162's `apply_verified_boost` (verified x 1.1, rescale to at most 1, sort) made of the same lists.
    #[test]
    fn general_ranks_exactly_as_before() {
        use super::{apply_profile, RankingProfile};
        let general = RankingProfile::general();
        assert_eq!(general.verified_factor(), Some(super::VERIFIED_BOOST));
        assert_eq!(super::VERIFIED_BOOST, 1.1);
        let close = [("plain", "p", 0.80), ("checked", "v", 0.76), ("other", "o", 0.40)];
        let mut results: Vec<SearchResult> = close.iter().map(|(a, c, s)| result(a, c, *s)).collect();
        apply_profile(&mut results, &general, &verified(&["checked"]));
        let expected = [("v", 0.76 * 1.1), ("p", 0.80), ("o", 0.40)];
        for ((chunk, score), (want_chunk, want)) in scores(&results).iter().zip(expected) {
            assert_eq!(chunk, want_chunk);
            assert!((score - want).abs() < 1e-12, "{chunk}: {score} against {want}");
        }
        // A list led by a perfect score is rescaled by what the boost made of it.
        let mut led = vec![result("checked", "v", 1.0), result("plain", "p", 0.9)];
        apply_profile(&mut led, &general, &verified(&["checked"]));
        assert_eq!(led[0].chunk_id, "v");
        assert!((led[0].score - 1.0).abs() < 1e-12 && (led[1].score - 0.9 / 1.1).abs() < 1e-12);
        // The same list with the rule left out is untouched, and so is an empty one.
        let mut same = vec![result("plain", "p", 0.9), result("checked", "v", 0.8)];
        apply_profile(&mut same, &RankingProfile::with_verified(1.0), &verified(&["checked"]));
        assert_eq!(scores(&same), vec![("p".to_owned(), 0.9), ("v".to_owned(), 0.8)]);
        apply_profile(&mut [], &general, &verified(&["checked"]));
    }

    #[test]
    fn a_template_with_another_verified_factor_changes_the_order_and_the_scores() {
        use super::{apply_profile, RankingProfile};
        let list = || vec![result("plain", "p", 0.80), result("checked", "v", 0.72)];
        // At the usual 1.1 the verified passage is still behind (0.72 x 1.1 = 0.792 against 0.80).
        let mut usual = list();
        apply_profile(&mut usual, &RankingProfile::general(), &verified(&["checked"]));
        assert_eq!(usual[0].chunk_id, "p");
        // At 1.15 (Operations, Knowledge base) it moves ahead (0.828).
        let mut operations = list();
        apply_profile(
            &mut operations,
            &RankingProfile::with_verified(1.15),
            &verified(&["checked"]),
        );
        assert_eq!(operations[0].chunk_id, "v");
        assert!((operations[0].score - 0.72 * 1.15).abs() < 1e-12);
        // Below 1 it ranks verified evidence a little lower.
        let mut lower = vec![result("checked", "v", 0.80), result("plain", "p", 0.78)];
        apply_profile(&mut lower, &RankingProfile::with_verified(0.9), &verified(&["checked"]));
        assert_eq!(lower[0].chunk_id, "p");
        // The bounds a template may use (0.8 to 1.3) never let a clearly better match lose.
        let mut clear = vec![result("plain", "p", 1.0), result("checked", "v", 0.6)];
        apply_profile(&mut clear, &RankingProfile::with_verified(1.3), &verified(&["checked"]));
        assert_eq!(clear[0].chunk_id, "p");
    }

    fn typed(artifact: &str, chunk: &str, score: f64, artifact_type: ArtifactType, path: &str) -> SearchResult {
        SearchResult {
            artifact_id: artifact.to_owned(),
            score,
            artifact_type,
            path: path.to_owned(),
            ..hit(chunk)
        }
    }

    #[test]
    fn evidence_of_the_kinds_a_template_cares_about_ranks_a_little_higher() {
        use super::{apply_profile, RankingFacts, RankingProfile};
        // Operations: a runbook outranks an equally good match from a note.
        let operations = RankingProfile::with_verified(1.15)
            .boost_type(ArtifactType::Runbook, 1.2)
            .boost_type(ArtifactType::Incident, 1.2);
        let mut equal = vec![
            typed("n", "note", 0.90, ArtifactType::Note, "notes/today.md"),
            typed("r", "runbook", 0.90, ArtifactType::Runbook, "runbooks/restart.md"),
        ];
        apply_profile(&mut equal, &operations, &RankingFacts::default());
        assert_eq!(equal[0].chunk_id, "runbook");
        assert!((equal[0].score - 1.0).abs() < 1e-12, "rescaled so the best is 1");
        // A far better note still wins.
        let mut far = vec![
            typed("r", "runbook", 0.60, ArtifactType::Runbook, "runbooks/restart.md"),
            typed("n", "note", 1.00, ArtifactType::Note, "notes/today.md"),
        ];
        apply_profile(&mut far, &operations, &RankingFacts::default());
        assert_eq!(far[0].chunk_id, "note");
    }

    #[test]
    fn a_document_kind_and_a_folder_pattern_boost_what_they_match() {
        use super::{apply_profile, RankingFacts, RankingProfile};
        let profile = RankingProfile::with_verified(1.0)
            .boost_document_kind("pdf", 1.2)
            .boost_path("docs/adr/**", 1.2);
        let mut results = vec![
            typed("a", "plain", 0.80, ArtifactType::File, "misc/plain.txt"),
            typed("b", "pdf", 0.70, ArtifactType::File, "contracts/Lease.PDF"),
            typed("c", "adr", 0.69, ArtifactType::MarkdownDoc, "payments/docs/adr/0003.md"),
        ];
        apply_profile(&mut results, &profile, &RankingFacts::default());
        assert_eq!(
            results.iter().map(|r| r.chunk_id.as_str()).collect::<Vec<_>>(),
            ["pdf", "adr", "plain"]
        );
        assert!((results[0].score - 0.84).abs() < 1e-12 && (results[1].score - 0.828).abs() < 1e-12);
    }

    #[test]
    fn the_product_of_all_the_factors_that_match_is_held_to_one_and_a_half() {
        use super::{apply_profile, RankingFacts, RankingProfile, MAX_PRODUCT};
        assert_eq!(MAX_PRODUCT, 1.5);
        // Sixteen rules that all match one passage, each 1.3: 1.3^16 is far above 1.5.
        let mut profile = RankingProfile::with_verified(1.0);
        for _ in 0..16 {
            profile = profile.boost_type(ArtifactType::Runbook, 1.3);
        }
        assert_eq!(profile.rules().len(), 17);
        let mut results = vec![
            typed("a", "boosted", 0.50, ArtifactType::Runbook, "r.md"),
            typed("b", "other", 0.74, ArtifactType::Note, "n.md"),
        ];
        apply_profile(&mut results, &profile, &RankingFacts::default());
        // 0.50 x 1.5 = 0.75 beats 0.74, and no more than that: the cap, not the pile, decides.
        assert_eq!(results[0].chunk_id, "boosted");
        assert!((results[0].score - 0.75).abs() < 1e-12);
        let mut clear = vec![
            typed("a", "boosted", 0.50, ArtifactType::Runbook, "r.md"),
            typed("b", "other", 0.80, ArtifactType::Note, "n.md"),
        ];
        apply_profile(&mut clear, &profile, &RankingFacts::default());
        assert_eq!(clear[0].chunk_id, "other", "a clearly better match still wins");
        // The same holds downwards: sixteen 0.8s do not bury a passage below two thirds.
        let mut lowered = RankingProfile::with_verified(1.0);
        for _ in 0..16 {
            lowered = lowered.boost_type(ArtifactType::Note, 0.8);
        }
        let mut sunk = vec![
            typed("a", "note", 0.90, ArtifactType::Note, "n.md"),
            typed("b", "other", 0.58, ArtifactType::File, "o.txt"),
        ];
        apply_profile(&mut sunk, &lowered, &RankingFacts::default());
        assert!((sunk.iter().find(|r| r.chunk_id == "note").unwrap().score - 0.60).abs() < 1e-12);
    }

    #[test]
    fn rules_that_stack_multiply_and_the_general_profile_adds_nothing() {
        use super::{apply_profile, RankingFacts, RankingProfile};
        let profile = RankingProfile::with_verified(1.1).boost_type(ArtifactType::Runbook, 1.2);
        let mut facts = RankingFacts::default();
        facts.lifecycle.insert("r".to_owned(), "verified".to_owned());
        let mut results = vec![
            typed("r", "both", 0.60, ArtifactType::Runbook, "r.md"),
            typed("p", "plain", 0.70, ArtifactType::File, "p.txt"),
        ];
        apply_profile(&mut results, &profile, &facts);
        assert_eq!(results[0].chunk_id, "both");
        assert!((results[0].score - 0.60 * 1.1 * 1.2).abs() < 1e-12);
        assert!(RankingProfile::general().rules().len() == 1 && !RankingProfile::general().is_empty());
    }

    #[test]
    fn the_kind_of_document_follows_the_ledgers_own_rules() {
        use super::document_kind;
        assert_eq!(document_kind(&ArtifactType::File, "contracts/Lease.PDF"), "pdf");
        assert_eq!(document_kind(&ArtifactType::File, "a/budget.xlsx"), "excel");
        assert_eq!(document_kind(&ArtifactType::File, "a/deck.pptx"), "powerpoint");
        assert_eq!(document_kind(&ArtifactType::File, "a/memo.docx"), "word");
        assert_eq!(document_kind(&ArtifactType::File, "a/mail.eml"), "email");
        assert_eq!(document_kind(&ArtifactType::File, "a/notes.one"), "onenote");
        assert_eq!(document_kind(&ArtifactType::MarkdownDoc, "docs/guide.md"), "markdown");
        assert_eq!(document_kind(&ArtifactType::Note, "notes/idea"), "note");
        assert_eq!(document_kind(&ArtifactType::CodeFile, "src/lib.rs"), "code");
        assert_eq!(document_kind(&ArtifactType::Image, "img/logo.png"), "image");
        assert_eq!(document_kind(&ArtifactType::Repository, "payments"), "repository");
        assert_eq!(document_kind(&ArtifactType::File, "notes.txt"), "text");
        assert_eq!(document_kind(&ArtifactType::Runbook, "runbooks/restart.md"), "text");
        // A document format wins over the evidence's own type, as in the ledger.
        assert_eq!(document_kind(&ArtifactType::Decision, "decisions/0001.docx"), "word");
    }

    fn aged(days: &[(&str, f64)]) -> super::RankingFacts {
        super::RankingFacts {
            age_days: days
                .iter()
                .map(|(artifact, age)| ((*artifact).to_owned(), *age))
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn the_preference_for_recent_evidence_halves_with_the_half_life_and_never_goes_below_one() {
        use super::recency_factor;
        assert!(
            (recency_factor(0.0, 90.0, 1.2) - 1.2).abs() < 1e-12,
            "changed just now gets the maximum"
        );
        assert!(
            (recency_factor(90.0, 90.0, 1.2) - 1.1).abs() < 1e-12,
            "one half-life: half of the nudge"
        );
        assert!((recency_factor(180.0, 90.0, 1.2) - 1.05).abs() < 1e-12);
        let month = recency_factor(30.0, 90.0, 1.2);
        let year = recency_factor(365.0, 90.0, 1.2);
        assert!(
            (month - 1.159).abs() < 0.001 && (year - 1.012).abs() < 0.001,
            "{month} {year}"
        );
        assert!(year > 1.0 && recency_factor(100_000.0, 90.0, 1.2) >= 1.0);
        assert_eq!(
            recency_factor(-5.0, 90.0, 1.2),
            1.2,
            "a clock in the past is a file changed just now"
        );
        assert_eq!(recency_factor(10.0, 0.0, 1.2), 1.0, "no half-life, no preference");
    }

    #[test]
    fn a_month_old_runbook_edges_out_a_year_old_one_of_equal_relevance_and_nothing_changes_with_the_rule_off() {
        use super::{apply_profile, RankingProfile};
        let list = || {
            vec![
                typed("old", "year", 0.90, ArtifactType::Runbook, "runbooks/a.md"),
                typed("new", "month", 0.90, ArtifactType::Runbook, "runbooks/b.md"),
            ]
        };
        let facts = aged(&[("old", 365.0), ("new", 30.0)]);
        let mut preferred = list();
        apply_profile(
            &mut preferred,
            &RankingProfile::with_verified(1.0).prefer_recent(90.0, 1.2),
            &facts,
        );
        assert_eq!(preferred[0].chunk_id, "month");
        assert!(preferred[0].score > preferred[1].score);
        // The rule off: untouched, the order they came in.
        let mut off = list();
        apply_profile(&mut off, &RankingProfile::with_verified(1.0), &facts);
        assert_eq!(
            off.iter().map(|r| r.chunk_id.as_str()).collect::<Vec<_>>(),
            ["year", "month"]
        );
        assert!(RankingProfile::with_verified(1.0).prefer_recent(90.0, 1.0).is_neutral());
        // A far better old match still wins, and a file whose age is not known is not touched.
        let mut far = vec![
            typed("old", "year", 1.0, ArtifactType::Runbook, "a.md"),
            typed("new", "month", 0.7, ArtifactType::Runbook, "b.md"),
        ];
        apply_profile(
            &mut far,
            &RankingProfile::with_verified(1.0).prefer_recent(90.0, 1.2),
            &facts,
        );
        assert_eq!(far[0].chunk_id, "year");
        let mut unknown = vec![typed("x", "unknown", 0.5, ArtifactType::Note, "x.md")];
        apply_profile(
            &mut unknown,
            &RankingProfile::with_verified(1.0).prefer_recent(90.0, 1.2),
            &aged(&[]),
        );
        assert_eq!(unknown[0].score, 0.5);
    }

    #[test]
    fn a_profile_says_whether_it_changes_anything() {
        use super::{RankingProfile, RankingRule};
        assert!(!RankingProfile::general().is_neutral());
        assert!(RankingProfile::with_verified(1.0).is_neutral());
        assert_eq!(RankingProfile::default(), RankingProfile::general());
        assert_eq!(RankingProfile::general().rules().len(), 1);
        assert!(matches!(
            RankingProfile::general().rules()[0],
            RankingRule::Lifecycle { .. }
        ));
        assert_eq!(RankingProfile::with_verified(1.15).verified_factor(), Some(1.15));
    }
}
