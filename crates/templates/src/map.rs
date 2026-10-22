use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{TemplateIssue, FILE_TYPES};

/// The kinds of link the knowledge map draws, by the id the API uses (`KnowledgeEdgeKind` in the domain crate; a test in
/// the api crate matches on every one).
pub const MAP_RELATIONS: [&str; 2] = ["similar", "cites"];

/// What the knowledge map puts first (V0.2.184). It is presentation only: nothing is left out of the map's data.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapBlock {
    /// File types whose rows of the coverage table come first, in this order. They are the ledger's file types
    /// (`FILE_TYPES`); the rows of the others follow in the usual order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coverage_first: Vec<String>,
    /// Kinds of link listed first and drawn on top of the others: `similar` (files that mean alike) and `cites` (a
    /// memory card citing a file).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations_first: Vec<String>,
}

pub(crate) fn validate(block: &MapBlock, issues: &mut Vec<TemplateIssue>) {
    for (key, list, vocabulary, noun) in [
        (
            "coverage_first",
            &block.coverage_first,
            FILE_TYPES.as_slice(),
            "a file type",
        ),
        (
            "relations_first",
            &block.relations_first,
            MAP_RELATIONS.as_slice(),
            "a kind of link",
        ),
    ] {
        let mut seen = HashSet::new();
        for (index, id) in list.iter().enumerate() {
            let path = format!("map.{key}[{index}]");
            if !vocabulary.contains(&id.as_str()) {
                issues.push(TemplateIssue::new(path, format!("`{id}` is not {noun}")));
            } else if !seen.insert(id.as_str()) {
                issues.push(TemplateIssue::new(path, format!("`{id}` is listed twice")));
            }
        }
    }
}
