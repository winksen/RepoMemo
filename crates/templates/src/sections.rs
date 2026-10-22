use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::TemplateIssue;

/// The sections of a workspace, in the order they have always had. The web client owns the pages
/// (`web/constants.ts`); a test compares the two lists, so a template can only name a section that exists.
pub const WORKSPACE_SECTIONS: [&str; 12] = [
    "overview",
    "evidence",
    "documents",
    "repositories",
    "retrieval",
    "map",
    "health",
    "review",
    "memory",
    "tasks",
    "people",
    "settings",
];

/// How a template arranges the sections of a workspace (V0.2.171). Both lists are optional.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionsBlock {
    /// The sections shown as tabs, in this order. A section named in neither list follows these, in the usual order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    /// The sections kept under a *More* menu. They are still reached by their address, and the one that is open is
    /// shown as a tab.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub more: Vec<String>,
}

/// The tabs and the *More* menu of a workspace, complete: every section is in one of the two lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSections {
    pub primary: Vec<String>,
    pub more: Vec<String>,
}

impl ResolvedSections {
    /// Settles the arrangement. With no block it is the usual twelve in the usual order and nothing under *More*.
    pub fn from_block(block: Option<&SectionsBlock>) -> Self {
        let Some(block) = block else {
            return Self {
                primary: WORKSPACE_SECTIONS.map(str::to_owned).to_vec(),
                more: Vec::new(),
            };
        };
        let mut primary = block.order.clone();
        primary.extend(
            WORKSPACE_SECTIONS
                .iter()
                .filter(|section| !block.order.iter().chain(&block.more).any(|named| named == *section))
                .map(|section| (*section).to_owned()),
        );
        Self {
            primary,
            more: block.more.clone(),
        }
    }
}

pub(crate) fn validate(block: &SectionsBlock, issues: &mut Vec<TemplateIssue>) {
    let mut named = HashSet::new();
    for (list, sections) in [("order", &block.order), ("more", &block.more)] {
        for (index, section) in sections.iter().enumerate() {
            let path = format!("sections.{list}[{index}]");
            if !WORKSPACE_SECTIONS.contains(&section.as_str()) {
                issues.push(TemplateIssue::new(
                    path,
                    format!("`{section}` is not a workspace section"),
                ));
            } else if !named.insert(section.as_str()) {
                issues.push(TemplateIssue::new(
                    path,
                    format!("`{section}` is already named in `order` or `more`"),
                ));
            }
        }
    }
}
