//! Workspace templates for RepoMemo (roadmap F26a, track N).
//!
//! A template is a small JSON file that names modules RepoMemo already has and
//! gives them an order, an emphasis or a weight. This crate holds the format,
//! its limits and its validation. It knows nothing of the database, the
//! server or the web client: whatever reads a template receives values that
//! were checked here.
//!
//! The format grows one block at a time, and a block is added in the release
//! that also reads it, so the format never has a key without a reader. An
//! unknown key, an unknown value or a value out of its bound makes a file
//! invalid; there is no "ignore what you do not know".

mod assistant;
mod catalogue;
mod evidence;
mod folders;
mod general;
mod health;
mod issue;
mod labels;
pub mod limits;
mod map;
mod meta;
mod overrides;
mod overview;
mod ranking;
mod repositories;
mod resolve;
mod retrieval;
mod review;
mod sections;

#[cfg(test)]
mod tests;

pub use assistant::{
    AssistantBlock, Phrase, CAPABILITIES, FOCUS_MAX, PHRASES_MAX, PHRASE_MAX, SUGGESTIONS_MAX, SUGGESTION_MAX,
};
pub use catalogue::{Catalogue, SourceProblem};
pub use evidence::{EvidenceBlock, ResolvedEvidence, DEFAULT_VIEW, EVIDENCE_VIEWS, FILE_TYPES};
pub use folders::StarterFolder;
pub use general::{general, GENERAL_ID};
pub use health::{HealthBlock, DEFAULT_DETECTORS, HEALTH_DETECTORS, STALE_DAYS_MAX, STALE_DAYS_MIN};
pub use issue::TemplateIssue;
pub use labels::{LabelsBlock, ARTIFACT_TYPES};
pub use map::{MapBlock, MAP_RELATIONS};
pub use meta::{TemplateMeta, WORKSPACE_ICONS};
pub use overrides::Overrides;
pub use overview::{OverviewBlock, CARDS_MAX, DEFAULT_CARDS, PULSE_CARDS};
pub use ranking::{
    pattern_problem, Boost, RankingBlock, Recency, ResolvedRanking, BOOSTS_MAX, DEFAULT_VERIFIED, FACTOR_MAX,
    FACTOR_MIN, HALF_LIFE_MAX, HALF_LIFE_MIN, PRODUCT_MAX, RECENCY_MAX,
};
pub use repositories::{RepositoriesBlock, REPOSITORY_PATTERNS_MAX};
pub use resolve::{Origin, Resolution};
pub use retrieval::{RetrievalBlock, TYPE_CHIPS_MAX};
pub use review::{ReviewBlock, REVIEW_SOURCES};
pub use sections::{ResolvedSections, SectionsBlock, ADMIN_ONLY_SECTIONS, DEFAULT_LANDING, WORKSPACE_SECTIONS};

use serde::{Deserialize, Serialize};

/// The format number this crate reads. A file written for another number is
/// refused with a message that says which side is newer.
pub const FORMAT: u32 = 1;

/// One template, as written in a JSON file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateDefinition {
    /// The format number the file is written for ([`FORMAT`]).
    pub format: u32,
    pub meta: TemplateMeta,
    /// Folders made when a workspace is created from the template (V0.2.168); none when left out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub starter_folders: Vec<StarterFolder>,
    /// The order of the tabs and the sections kept under *More* (V0.2.171); as today when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sections: Option<SectionsBlock>,
    /// The names shown for sections and kinds of evidence (V0.2.173); the usual names when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<LabelsBlock>,
    /// The pulse cards the Overview shows, and their order (V0.2.174); the usual nine when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview: Option<OverviewBlock>,
    /// How the evidence ledger opens and which file types its filter lists first (V0.2.175); as today when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<EvidenceBlock>,
    /// The order of the assistant's buttons and the questions it suggests (V0.2.176); as today when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant: Option<AssistantBlock>,
    /// The kinds of evidence the Retrieval page suggests as chips (V0.2.176); none when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval: Option<RetrievalBlock>,
    /// How search weighs what it finds (V0.2.178); as today when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ranking: Option<RankingBlock>,
    /// The health checks that run and their order (V0.2.181); every check in the usual order when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<HealthBlock>,
    /// What the review queue lists first (V0.2.183); mixed, oldest first, when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review: Option<ReviewBlock>,
    /// What the knowledge map puts first (V0.2.184); as today when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<MapBlock>,
    /// The include and exclude patterns the repository link form is filled in with (V0.2.186); none when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repositories: Option<RepositoriesBlock>,
}

impl TemplateDefinition {
    /// Reads and checks one file. Every problem found is returned, so a person
    /// fixing a file sees them all at once; the ones the JSON reader finds
    /// carry the line and column.
    pub fn parse(text: &str) -> Result<Self, Vec<TemplateIssue>> {
        if text.len() > limits::FILE_BYTES {
            return Err(vec![TemplateIssue::new(
                "",
                format!(
                    "the file is {} bytes; a template is at most {}",
                    text.len(),
                    limits::FILE_BYTES
                ),
            )]);
        }
        let depth = nesting_depth(text);
        if depth > limits::NESTING {
            return Err(vec![TemplateIssue::new(
                "",
                format!(
                    "the file is nested {depth} levels deep; a template is at most {}",
                    limits::NESTING
                ),
            )]);
        }
        let definition: Self = serde_json::from_str(text).map_err(|error| vec![TemplateIssue::from_json(&error)])?;
        let issues = definition.validate();
        if issues.is_empty() {
            Ok(definition)
        } else {
            Err(issues)
        }
    }

    /// The problems of a template that was read, built in code or edited: an
    /// empty list means it is valid.
    pub fn validate(&self) -> Vec<TemplateIssue> {
        let mut issues = Vec::new();
        if self.format != FORMAT {
            let side = if self.format > FORMAT { "newer" } else { "older" };
            issues.push(TemplateIssue::new(
                "format",
                format!(
                    "the file is written for format {}, which is {side} than the format {FORMAT} this server reads",
                    self.format
                ),
            ));
        }
        self.meta.validate(&mut issues);
        folders::validate(&self.starter_folders, &mut issues);
        if let Some(sections) = &self.sections {
            sections::validate(sections, &mut issues);
        }
        if let Some(labels) = &self.labels {
            labels::validate(labels, &mut issues);
        }
        if let Some(overview) = &self.overview {
            overview::validate(overview, &mut issues);
        }
        if let Some(evidence) = &self.evidence {
            evidence::validate(evidence, &mut issues);
        }
        if let Some(assistant) = &self.assistant {
            assistant::validate(assistant, &mut issues);
        }
        if let Some(retrieval) = &self.retrieval {
            retrieval::validate(retrieval, &mut issues);
        }
        if let Some(ranking) = &self.ranking {
            ranking::validate(ranking, &mut issues);
        }
        if let Some(health) = &self.health {
            health::validate(health, &mut issues);
        }
        if let Some(review) = &self.review {
            review::validate(review, &mut issues);
        }
        if let Some(map) = &self.map {
            map::validate(map, &mut issues);
        }
        if let Some(repositories) = &self.repositories {
            repositories::validate(repositories, &mut issues);
        }
        issues
    }

    pub fn id(&self) -> &str {
        &self.meta.id
    }

    /// The names this template gives to sections and kinds of evidence; empty for the usual names.
    pub fn labels(&self) -> LabelsBlock {
        self.labels.clone().unwrap_or_default()
    }

    /// The health checks that run, in the order their findings are listed.
    pub fn health_detectors(&self) -> Vec<String> {
        HealthBlock::resolved(self.health.as_ref())
    }

    /// What the knowledge map puts first; nothing for the usual map.
    pub fn map_emphasis(&self) -> MapBlock {
        self.map.clone().unwrap_or_default()
    }

    /// The patterns the repository link form is filled in with; none for the usual empty form.
    pub fn repository_rules(&self) -> RepositoriesBlock {
        self.repositories.clone().unwrap_or_default()
    }

    /// The groups the review queue lists, in order; empty for the usual mixed list.
    pub fn review_order(&self) -> Vec<String> {
        ReviewBlock::resolved(self.review.as_ref())
    }

    /// After how many days without a change or a verification evidence is stale, when the template checks for it.
    pub fn stale_after_days(&self) -> Option<u32> {
        self.health.as_ref().and_then(|health| health.stale_after_days)
    }

    /// How search weighs what it finds, complete.
    pub fn ranking(&self) -> ResolvedRanking {
        ResolvedRanking::from_block(self.ranking.as_ref())
    }

    /// What the assistant shows first and suggests; empty for the usual buttons and no suggestion.
    pub fn assistant(&self) -> AssistantBlock {
        self.assistant.clone().unwrap_or_default()
    }

    /// The kinds of evidence the Retrieval page suggests as chips; none for the usual page.
    pub fn retrieval(&self) -> RetrievalBlock {
        self.retrieval.clone().unwrap_or_default()
    }

    /// How the evidence ledger opens and which file types it lists first, complete.
    pub fn evidence(&self) -> ResolvedEvidence {
        ResolvedEvidence::from_block(self.evidence.as_ref())
    }

    /// The pulse cards the Overview shows, in order.
    pub fn overview_cards(&self) -> Vec<String> {
        OverviewBlock::resolved(self.overview.as_ref())
    }

    /// The tabs and the *More* menu this template asks for, complete.
    pub fn sections(&self) -> ResolvedSections {
        ResolvedSections::from_block(self.sections.as_ref())
    }
}

/// How deeply `{` and `[` nest in a JSON text, ignoring brackets inside strings.
fn nesting_depth(text: &str) -> usize {
    let (mut depth, mut deepest) = (0usize, 0usize);
    let (mut in_string, mut escaped) = (false, false);
    for character in text.chars() {
        if in_string {
            match (escaped, character) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' | '[' => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            '}' | ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    deepest
}
