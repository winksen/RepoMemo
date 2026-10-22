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

/// The sections only owners and administrators see. A template cannot make one the landing section, because a
/// member would be sent to a page they may not open.
pub const ADMIN_ONLY_SECTIONS: [&str; 2] = ["people", "settings"];

/// The section a workspace opens on when its address names none.
pub const DEFAULT_LANDING: &str = "overview";

/// How a template arranges the sections of a workspace (V0.2.171). Every key is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SectionsBlock {
    /// The sections shown as tabs, in this order. A section named in neither list follows these, in the usual order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    /// The sections kept under a *More* menu. They are still reached by their address, and the one that is open is
    /// shown as a tab.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub more: Vec<String>,
    /// The section a workspace opens on when its address names none (V0.2.172); `overview` when left out. It may
    /// be one of the tabs or one under *More*, but not People or Settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landing: Option<String>,
}

/// The tabs and the *More* menu of a workspace, complete: every section is in one of the two lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSections {
    pub primary: Vec<String>,
    pub more: Vec<String>,
    /// The section a workspace opens on when its address names none.
    pub landing: String,
}

impl ResolvedSections {
    /// Settles the arrangement. With no block it is the usual twelve in the usual order and nothing under *More*.
    pub fn from_block(block: Option<&SectionsBlock>) -> Self {
        let Some(block) = block else {
            return Self {
                primary: WORKSPACE_SECTIONS.map(str::to_owned).to_vec(),
                more: Vec::new(),
                landing: DEFAULT_LANDING.to_owned(),
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
            landing: block.landing.clone().unwrap_or_else(|| DEFAULT_LANDING.to_owned()),
        }
    }
}

pub(crate) fn validate(block: &SectionsBlock, issues: &mut Vec<TemplateIssue>) {
    if let Some(landing) = &block.landing {
        if !WORKSPACE_SECTIONS.contains(&landing.as_str()) {
            issues.push(TemplateIssue::new(
                "sections.landing",
                format!("`{landing}` is not a workspace section"),
            ));
        } else if ADMIN_ONLY_SECTIONS.contains(&landing.as_str()) {
            issues.push(TemplateIssue::new(
                "sections.landing",
                format!("`{landing}` is only for administrators, so a member could not open it"),
            ));
        }
    }
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
