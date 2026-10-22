use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{TemplateIssue, ARTIFACT_TYPES};

/// How many type chips a template may suggest on the Retrieval page.
pub const TYPE_CHIPS_MAX: usize = 6;

/// What the Retrieval page suggests (V0.2.176): kinds of evidence as chips above the search. A chip filters the
/// search to that kind when it is chosen; nothing is applied until then.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetrievalBlock {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub type_chips: Vec<String>,
}

pub(crate) fn validate(block: &RetrievalBlock, issues: &mut Vec<TemplateIssue>) {
    if block.type_chips.len() > TYPE_CHIPS_MAX {
        issues.push(TemplateIssue::new(
            "retrieval.type_chips",
            format!(
                "there are {} chips; a template suggests at most {TYPE_CHIPS_MAX}",
                block.type_chips.len()
            ),
        ));
    }
    let mut seen = HashSet::new();
    for (index, kind) in block.type_chips.iter().enumerate() {
        let path = format!("retrieval.type_chips[{index}]");
        if !ARTIFACT_TYPES.contains(&kind.as_str()) {
            issues.push(TemplateIssue::new(path, format!("`{kind}` is not a kind of evidence")));
        } else if !seen.insert(kind.as_str()) {
            issues.push(TemplateIssue::new(path, format!("`{kind}` is listed twice")));
        }
    }
}
