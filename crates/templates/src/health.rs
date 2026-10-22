use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::TemplateIssue;

/// The health checks, by the id the API uses (`HealthDetector` in the domain crate; a test in the api crate matches
/// on every one), in the order Workspace Health has always listed them.
pub const HEALTH_DETECTORS: [&str; 7] = [
    "older_version_active",
    "duplicate_content",
    "removed_symbol_mentioned",
    "outdated_evidence_referenced",
    "stale_memory_citation",
    "index_failed",
    "unconnected",
];

/// Which health checks run in a workspace, and in what order their findings are listed (V0.2.181).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthBlock {
    /// The checks that run, in the order their findings are listed. A check left out is not run (what was decided
    /// about its earlier findings stays recorded).
    #[serde(default)]
    pub detectors: Vec<String>,
}

impl HealthBlock {
    /// The checks that run: the template's list, or every check in the usual order without a block.
    pub fn resolved(block: Option<&HealthBlock>) -> Vec<String> {
        match block {
            Some(block) => block.detectors.clone(),
            None => HEALTH_DETECTORS.map(str::to_owned).to_vec(),
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
