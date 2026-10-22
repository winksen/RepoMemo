use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::TemplateIssue;

/// The health checks, by the id the API uses (`HealthDetector` in the domain crate; a test in the api crate matches
/// on every one), in the order Workspace Health has always listed them. `stale_evidence` (V0.2.182) is the one a
/// workspace does not run unless its template names it.
pub const HEALTH_DETECTORS: [&str; 8] = [
    "older_version_active",
    "duplicate_content",
    "removed_symbol_mentioned",
    "outdated_evidence_referenced",
    "stale_memory_citation",
    "index_failed",
    "unconnected",
    "stale_evidence",
];

/// The checks a workspace runs when its template does not choose: the seven Health has always had.
pub const DEFAULT_DETECTORS: [&str; 7] = [
    "older_version_active",
    "duplicate_content",
    "removed_symbol_mentioned",
    "outdated_evidence_referenced",
    "stale_memory_citation",
    "index_failed",
    "unconnected",
];

/// How many days without a change (or a verification) make evidence stale: the bounds a template may choose from.
pub const STALE_DAYS_MIN: u32 = 7;
pub const STALE_DAYS_MAX: u32 = 3650;

/// Which health checks run in a workspace, and in what order their findings are listed (V0.2.181).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthBlock {
    /// The checks that run, in the order their findings are listed. A check left out is not run (what was decided
    /// about its earlier findings stays recorded).
    #[serde(default)]
    pub detectors: Vec<String>,
    /// How many days without a change or a verification make evidence stale, for the `stale_evidence` check (7 to
    /// 3650). Required when that check is listed and meaningless otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_after_days: Option<u32>,
}

impl HealthBlock {
    /// The checks that run: the template's list, or the seven that have always run without a block.
    pub fn resolved(block: Option<&HealthBlock>) -> Vec<String> {
        match block {
            Some(block) => block.detectors.clone(),
            None => DEFAULT_DETECTORS.map(str::to_owned).to_vec(),
        }
    }
}

pub(crate) fn validate(block: &HealthBlock, issues: &mut Vec<TemplateIssue>) {
    if block.detectors.is_empty() {
        issues.push(TemplateIssue::new(
            "health.detectors",
            "list at least one check, or leave the block out",
        ));
    }
    let stale_listed = block.detectors.iter().any(|detector| detector == "stale_evidence");
    match (stale_listed, block.stale_after_days) {
        (true, None) => issues.push(TemplateIssue::new(
            "health.stale_after_days",
            "stale_evidence needs the number of days after which evidence is stale",
        )),
        (false, Some(_)) => issues.push(TemplateIssue::new(
            "health.stale_after_days",
            "the days are for the stale_evidence check, which is not listed",
        )),
        (true, Some(days)) if !(STALE_DAYS_MIN..=STALE_DAYS_MAX).contains(&days) => issues.push(TemplateIssue::new(
            "health.stale_after_days",
            format!("the days are {days}; they are {STALE_DAYS_MIN} to {STALE_DAYS_MAX}"),
        )),
        _ => {}
    }
    let mut seen = HashSet::new();
    for (index, detector) in block.detectors.iter().enumerate() {
        let path = format!("health.detectors[{index}]");
        if !HEALTH_DETECTORS.contains(&detector.as_str()) {
            issues.push(TemplateIssue::new(path, format!("`{detector}` is not a health check")));
        } else if !seen.insert(detector.as_str()) {
            issues.push(TemplateIssue::new(path, format!("`{detector}` is listed twice")));
        }
    }
}
