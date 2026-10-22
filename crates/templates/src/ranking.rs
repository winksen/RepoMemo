use serde::{Deserialize, Serialize};

use crate::TemplateIssue;

/// What a ranking factor may be: a nudge, never a verdict. A factor below 1 puts a kind of evidence a little lower,
/// above 1 a little higher; none may move a result far enough to beat a clearly better match.
pub const FACTOR_MIN: f64 = 0.8;
pub const FACTOR_MAX: f64 = 1.3;

/// The factor given to verified evidence when a template says nothing: a tenth more, as it has always been.
pub const DEFAULT_VERIFIED: f64 = 1.1;

/// How search weighs what it finds (V0.2.178). Every key is optional; a missing key means "as today".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RankingBlock {
    /// The factor for passages of files a person marked verified, from 0.8 to 1.3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified: Option<f64>,
}

/// What the ranking is told: always complete.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRanking {
    pub verified: f64,
}

impl ResolvedRanking {
    pub fn from_block(block: Option<&RankingBlock>) -> Self {
        Self {
            verified: block.and_then(|block| block.verified).unwrap_or(DEFAULT_VERIFIED),
        }
    }
}

pub(crate) fn validate(block: &RankingBlock, issues: &mut Vec<TemplateIssue>) {
    if let Some(factor) = block.verified {
        issues.extend(factor_problem(factor).map(|problem| TemplateIssue::new("ranking.verified", problem)));
    }
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
