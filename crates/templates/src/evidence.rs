use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::TemplateIssue;

/// How the evidence ledger can show its files, by id.
pub const EVIDENCE_VIEWS: [&str; 2] = ["grid", "list"];

/// The view a ledger opens in when its template does not choose.
pub const DEFAULT_VIEW: &str = "grid";

/// The file types the ledger's *File types* filter offers, in the order it has always listed them
/// (`FILE_CATEGORIES` in `web/constants.ts`; a test compares the two lists).
pub const FILE_TYPES: [&str; 12] = [
    "markdown",
    "note",
    "text",
    "code",
    "word",
    "excel",
    "powerpoint",
    "pdf",
    "onenote",
    "email",
    "image",
    "repository",
];

/// How the evidence ledger opens and which file types its filter lists first (V0.2.175). Both keys are optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceBlock {
    /// `grid` or `list`. A person who chose a view for themselves keeps it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_view: Option<String>,
    /// File types listed before the others in the ledger's filter, in this order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types_first: Vec<String>,
}

/// What the ledger is told: always complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEvidence {
    pub default_view: String,
    pub types_first: Vec<String>,
}

impl ResolvedEvidence {
    pub fn from_block(block: Option<&EvidenceBlock>) -> Self {
        Self {
            default_view: block
                .and_then(|block| block.default_view.clone())
                .unwrap_or_else(|| DEFAULT_VIEW.to_owned()),
            types_first: block.map(|block| block.types_first.clone()).unwrap_or_default(),
        }
    }
}

pub(crate) fn validate(block: &EvidenceBlock, issues: &mut Vec<TemplateIssue>) {
    if let Some(view) = &block.default_view {
        if !EVIDENCE_VIEWS.contains(&view.as_str()) {
            issues.push(TemplateIssue::new(
                "evidence.default_view",
                format!("`{view}` is not a view; use grid or list"),
            ));
        }
    }
    let mut seen = HashSet::new();
    for (index, file_type) in block.types_first.iter().enumerate() {
        let path = format!("evidence.types_first[{index}]");
        if !FILE_TYPES.contains(&file_type.as_str()) {
            issues.push(TemplateIssue::new(
                path,
                format!("`{file_type}` is not a file type of the ledger"),
            ));
        } else if !seen.insert(file_type.as_str()) {
            issues.push(TemplateIssue::new(path, format!("`{file_type}` is listed twice")));
        }
    }
}
