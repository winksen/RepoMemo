use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{pattern_problem, TemplateIssue};

/// How many include patterns, and how many exclude patterns, a template may offer.
pub const REPOSITORY_PATTERNS_MAX: usize = 16;

/// The include and exclude patterns the repository link form is filled in with (V0.2.186). They are an offer: the form
/// shows them, the person keeps, edits or clears them, and the link records what was kept. A repository already linked
/// is never touched, and nothing is applied to a sync by the template itself.
///
/// The grammar is the folder pattern of the ranking block (names, `*` and `**`, never a regular expression).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoriesBlock {
    /// A file must match one of these to be indexed; none means every file.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    /// Files left out, on top of the defaults every repository has.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
}

pub(crate) fn validate(block: &RepositoriesBlock, issues: &mut Vec<TemplateIssue>) {
    for (key, list) in [("include", &block.include), ("exclude", &block.exclude)] {
        if list.len() > REPOSITORY_PATTERNS_MAX {
            issues.push(TemplateIssue::new(
                format!("repositories.{key}"),
                format!(
                    "there are {} patterns; a template offers at most {REPOSITORY_PATTERNS_MAX}",
                    list.len()
                ),
            ));
        }
        let mut seen = HashSet::new();
        for (index, pattern) in list.iter().enumerate() {
            let path = format!("repositories.{key}[{index}]");
            if let Some(problem) = pattern_problem(pattern) {
                issues.push(TemplateIssue::new(path, problem));
            } else if !seen.insert(pattern.as_str()) {
                issues.push(TemplateIssue::new(path, format!("`{pattern}` is listed twice")));
            }
        }
    }
}
