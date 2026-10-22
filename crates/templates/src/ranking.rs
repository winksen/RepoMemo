use serde::{Deserialize, Serialize};

use crate::{TemplateIssue, ARTIFACT_TYPES, FILE_TYPES};

/// What a ranking factor may be: a nudge, never a verdict. A factor below 1 puts a kind of evidence a little lower,
/// above 1 a little higher; none may move a result far enough to beat a clearly better match.
pub const FACTOR_MIN: f64 = 0.8;
pub const FACTOR_MAX: f64 = 1.3;

/// The factor given to verified evidence when a template says nothing: a tenth more, as it has always been.
pub const DEFAULT_VERIFIED: f64 = 1.1;

/// How many boosts a template may list.
pub const BOOSTS_MAX: usize = 16;

/// The most a pattern may be, in characters, and in folders.
pub const PATTERN_MAX: usize = 120;
pub const PATTERN_SEGMENTS_MAX: usize = 12;

/// Passages a boost matches are weighed together: the product of all the factors that match one passage is held to
/// this much more (and to its inverse as the least), so no pile of boosts can bury a clearly better match.
pub const PRODUCT_MAX: f64 = 1.5;

/// How long the preference for recent evidence takes to halve, in days.
pub const HALF_LIFE_MIN: u32 = 7;
pub const HALF_LIFE_MAX: u32 = 3650;

/// The most a file changed just now may be favoured by (V0.2.180).
pub const RECENCY_MAX: f64 = 1.2;

/// A preference for recent evidence (V0.2.180): a file changed just now gets `max_factor`, and the nudge halves every
/// `half_life_days` until it is 1. Off when the block is left out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recency {
    /// From 7 to 3650.
    pub half_life_days: u32,
    /// Above 1 and at most 1.2.
    pub max_factor: f64,
}

/// One boost (V0.2.179): a factor for the passages of one kind of evidence, one kind of document or one folder. Exactly
/// one of the three selectors is given.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boost {
    /// A kind of evidence (`runbook`, `decision`…), as `ARTIFACT_TYPES` lists them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_type: Option<String>,
    /// A kind of document as the ledger's file types name them (`pdf`, `word`, `markdown`…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_kind: Option<String>,
    /// A folder or file pattern: `/` between folders, `*` inside a name, `**` for any folders. Never a regular expression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// From 0.8 to 1.3.
    pub factor: f64,
}

/// How search weighs what it finds (V0.2.178). Every key is optional; a missing key means "as today".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RankingBlock {
    /// The factor for passages of files a person marked verified, from 0.8 to 1.3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified: Option<f64>,
    /// Boosts by kind of evidence, kind of document or folder (V0.2.179), at most 16.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boosts: Vec<Boost>,
    /// A preference for recent evidence (V0.2.180); none when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recency: Option<Recency>,
}

/// What the ranking is told: always complete.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRanking {
    pub verified: f64,
    pub boosts: Vec<Boost>,
    pub recency: Option<Recency>,
}

impl ResolvedRanking {
    pub fn from_block(block: Option<&RankingBlock>) -> Self {
        Self {
            verified: block.and_then(|block| block.verified).unwrap_or(DEFAULT_VERIFIED),
            boosts: block.map(|block| block.boosts.clone()).unwrap_or_default(),
            recency: block.and_then(|block| block.recency.clone()),
        }
    }
}

pub(crate) fn validate(block: &RankingBlock, issues: &mut Vec<TemplateIssue>) {
    if let Some(factor) = block.verified {
        issues.extend(factor_problem(factor).map(|problem| TemplateIssue::new("ranking.verified", problem)));
    }
    if let Some(recency) = &block.recency {
        if !(HALF_LIFE_MIN..=HALF_LIFE_MAX).contains(&recency.half_life_days) {
            issues.push(TemplateIssue::new(
                "ranking.recency.half_life_days",
                format!(
                    "the half-life is {} days; it is {HALF_LIFE_MIN} to {HALF_LIFE_MAX}",
                    recency.half_life_days
                ),
            ));
        }
        if !recency.max_factor.is_finite() || recency.max_factor <= 1.0 || recency.max_factor > RECENCY_MAX {
            issues.push(TemplateIssue::new(
                "ranking.recency.max_factor",
                format!(
                    "the factor is {}; it is above 1 and at most {RECENCY_MAX}",
                    recency.max_factor
                ),
            ));
        }
    }
    if block.boosts.len() > BOOSTS_MAX {
        issues.push(TemplateIssue::new(
            "ranking.boosts",
            format!(
                "there are {} boosts; a template lists at most {BOOSTS_MAX}",
                block.boosts.len()
            ),
        ));
    }
    for (index, boost) in block.boosts.iter().enumerate() {
        let path = format!("ranking.boosts[{index}]");
        let selectors = [
            boost.artifact_type.is_some(),
            boost.document_kind.is_some(),
            boost.path.is_some(),
        ];
        if selectors.iter().filter(|given| **given).count() != 1 {
            issues.push(TemplateIssue::new(
                path.clone(),
                "a boost names exactly one of artifact_type, document_kind and path",
            ));
        }
        if let Some(kind) = &boost.artifact_type {
            if !ARTIFACT_TYPES.contains(&kind.as_str()) {
                issues.push(TemplateIssue::new(
                    format!("{path}.artifact_type"),
                    format!("`{kind}` is not a kind of evidence"),
                ));
            }
        }
        if let Some(kind) = &boost.document_kind {
            if !FILE_TYPES.contains(&kind.as_str()) {
                issues.push(TemplateIssue::new(
                    format!("{path}.document_kind"),
                    format!("`{kind}` is not a kind of document"),
                ));
            }
        }
        if let Some(pattern) = &boost.path {
            issues.extend(pattern_problem(pattern).map(|problem| TemplateIssue::new(format!("{path}.path"), problem)));
        }
        issues
            .extend(factor_problem(boost.factor).map(|problem| TemplateIssue::new(format!("{path}.factor"), problem)));
    }
}

/// Why a folder pattern is not allowed, if it is not. The grammar is small on purpose: names, `*` inside a name and
/// `**` for any number of folders; nothing that looks like a regular expression.
pub fn pattern_problem(pattern: &str) -> Option<String> {
    let length = pattern.chars().count();
    if pattern.is_empty() || length > PATTERN_MAX {
        return Some(format!("the pattern is {length} characters; it is 1 to {PATTERN_MAX}"));
    }
    let segments: Vec<&str> = pattern.split('/').collect();
    if segments.len() > PATTERN_SEGMENTS_MAX {
        return Some(format!(
            "the pattern has {} folders; the most is {PATTERN_SEGMENTS_MAX}",
            segments.len()
        ));
    }
    for segment in segments {
        if segment.is_empty() {
            return Some("the pattern has an empty folder name (a leading, trailing or doubled /)".to_owned());
        }
        if segment == ".." || segment == "." {
            return Some("the pattern cannot name . or ..".to_owned());
        }
        if segment != "**" && segment.contains("**") {
            return Some("** stands for whole folders: write it between slashes".to_owned());
        }
        if let Some(bad) = segment.chars().find(|c| {
            !(c.is_alphanumeric() || matches!(c, '*' | '-' | '_' | '.' | ' ' | '@' | '+' | ',' | '=' | '~' | '#' | '%'))
        }) {
            return Some(format!("`{bad}` is not allowed in a pattern; only names, * and **"));
        }
    }
    None
}

/// Why a factor is not allowed, if it is not.
pub(crate) fn factor_problem(factor: f64) -> Option<String> {
    if !factor.is_finite() || !(FACTOR_MIN..=FACTOR_MAX).contains(&factor) {
        Some(format!(
            "the factor is {factor}; it is from {FACTOR_MIN} to {FACTOR_MAX}"
        ))
    } else {
        None
    }
}
