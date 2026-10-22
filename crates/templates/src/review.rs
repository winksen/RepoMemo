use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::TemplateIssue;

/// The three things that can wait in a workspace's review queue, by the id the API uses for the kind of item
/// (`ReviewItemKind` in the domain crate; a test in the server matches on every one), in the order they have
/// always been mixed.
pub const REVIEW_SOURCES: [&str; 3] = ["evidence", "memory_card", "health_finding"];

/// What the review queue lists first (V0.2.183).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewBlock {
    /// Kinds of item listed first, in this order, each as a group: the oldest first inside a group (open health
    /// findings keep Health's order). A kind not named follows, after the named ones.
    #[serde(default)]
    pub order: Vec<String>,
}

impl ReviewBlock {
    /// The groups the queue lists, complete: the template's order and then the kinds it does not name, in the usual
    /// order. Without a block this is empty, which means "as today": the items are mixed, oldest first, with the
    /// health findings (which have no date) after what has waited.
    pub fn resolved(block: Option<&ReviewBlock>) -> Vec<String> {
        let Some(block) = block else {
            return Vec::new();
        };
        let mut order = block.order.clone();
        order.extend(
            REVIEW_SOURCES
                .iter()
                .filter(|source| !block.order.iter().any(|named| named == *source))
                .map(|source| (*source).to_owned()),
        );
        order
    }
}

pub(crate) fn validate(block: &ReviewBlock, issues: &mut Vec<TemplateIssue>) {
    if block.order.is_empty() {
        issues.push(TemplateIssue::new(
            "review.order",
            "list at least one kind of item, or leave the block out",
        ));
    }
    let mut seen = HashSet::new();
    for (index, source) in block.order.iter().enumerate() {
        let path = format!("review.order[{index}]");
        if !REVIEW_SOURCES.contains(&source.as_str()) {
            issues.push(TemplateIssue::new(
                path,
                format!("`{source}` is not something the review queue lists"),
            ));
        } else if !seen.insert(source.as_str()) {
            issues.push(TemplateIssue::new(path, format!("`{source}` is listed twice")));
        }
    }
}
