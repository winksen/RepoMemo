//! Workspace templates for RepoMemo (roadmap F26a, track N).
//!
//! A template is a small JSON file that names modules RepoMemo already has and
//! gives them an order, an emphasis or a weight. This crate holds the format,
//! its limits and its validation. It knows nothing of the database, the
//! server or the web client: whatever reads a template receives values that
//! were checked here.
//!
//! The format grows one block at a time, and a block is added in the release
//! that also reads it, so the format never has a key without a reader. An
//! unknown key, an unknown value or a value out of its bound makes a file
//! invalid; there is no "ignore what you do not know".

mod catalogue;
mod evidence;
mod folders;
mod general;
mod issue;
mod labels;
pub mod limits;
mod meta;
mod overview;
mod resolve;
mod sections;

#[cfg(test)]
mod tests;

pub use catalogue::{Catalogue, SourceProblem};
pub use evidence::{EvidenceBlock, ResolvedEvidence, DEFAULT_VIEW, EVIDENCE_VIEWS, FILE_TYPES};
pub use folders::StarterFolder;
pub use general::{general, GENERAL_ID};
pub use issue::TemplateIssue;
pub use labels::{LabelsBlock, ARTIFACT_TYPES};
pub use meta::{TemplateMeta, WORKSPACE_ICONS};
pub use overview::{OverviewBlock, CARDS_MAX, DEFAULT_CARDS, PULSE_CARDS};
pub use resolve::{Origin, Resolution};
pub use sections::{ResolvedSections, SectionsBlock, ADMIN_ONLY_SECTIONS, DEFAULT_LANDING, WORKSPACE_SECTIONS};

use serde::{Deserialize, Serialize};

/// The format number this crate reads. A file written for another number is
/// refused with a message that says which side is newer.
pub const FORMAT: u32 = 1;

/// One template, as written in a JSON file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateDefinition {
    /// The format number the file is written for ([`FORMAT`]).
    pub format: u32,
    pub meta: TemplateMeta,
    /// Folders made when a workspace is created from the template (V0.2.168); none when left out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub starter_folders: Vec<StarterFolder>,
    /// The order of the tabs and the sections kept under *More* (V0.2.171); as today when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sections: Option<SectionsBlock>,
    /// The names shown for sections and kinds of evidence (V0.2.173); the usual names when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<LabelsBlock>,
    /// The pulse cards the Overview shows, and their order (V0.2.174); the usual nine when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview: Option<OverviewBlock>,
    /// How the evidence ledger opens and which file types its filter lists first (V0.2.175); as today when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<EvidenceBlock>,
}

impl TemplateDefinition {
    /// Reads and checks one file. Every problem found is returned, so a person
    /// fixing a file sees them all at once; the ones the JSON reader finds
    /// carry the line and column.
    pub fn parse(text: &str) -> Result<Self, Vec<TemplateIssue>> {
        if text.len() > limits::FILE_BYTES {
            return Err(vec![TemplateIssue::new(
                "",
                format!(
                    "the file is {} bytes; a template is at most {}",
                    text.len(),
                    limits::FILE_BYTES
                ),
            )]);
        }
        let depth = nesting_depth(text);
        if depth > limits::NESTING {
            return Err(vec![TemplateIssue::new(
                "",
                format!(
                    "the file is nested {depth} levels deep; a template is at most {}",
                    limits::NESTING
                ),
            )]);
        }
        let definition: Self = serde_json::from_str(text).map_err(|error| vec![TemplateIssue::from_json(&error)])?;
        let issues = definition.validate();
        if issues.is_empty() {
            Ok(definition)
        } else {
            Err(issues)
        }
    }

    /// The problems of a template that was read, built in code or edited: an
    /// empty list means it is valid.
    pub fn validate(&self) -> Vec<TemplateIssue> {
        let mut issues = Vec::new();
        if self.format != FORMAT {
            let side = if self.format > FORMAT { "newer" } else { "older" };
            issues.push(TemplateIssue::new(
                "format",
                format!(
                    "the file is written for format {}, which is {side} than the format {FORMAT} this server reads",
                    self.format
                ),
            ));
        }
        self.meta.validate(&mut issues);
        folders::validate(&self.starter_folders, &mut issues);
        if let Some(sections) = &self.sections {
            sections::validate(sections, &mut issues);
        }
        if let Some(labels) = &self.labels {
            labels::validate(labels, &mut issues);
        }
        if let Some(overview) = &self.overview {
            overview::validate(overview, &mut issues);
        }
        if let Some(evidence) = &self.evidence {
            evidence::validate(evidence, &mut issues);
        }
        issues
    }

    pub fn id(&self) -> &str {
        &self.meta.id
    }

    /// The names this template gives to sections and kinds of evidence; empty for the usual names.
    pub fn labels(&self) -> LabelsBlock {
        self.labels.clone().unwrap_or_default()
    }

    /// How the evidence ledger opens and which file types it lists first, complete.
    pub fn evidence(&self) -> ResolvedEvidence {
        ResolvedEvidence::from_block(self.evidence.as_ref())
    }

    /// The pulse cards the Overview shows, in order.
    pub fn overview_cards(&self) -> Vec<String> {
        OverviewBlock::resolved(self.overview.as_ref())
    }

    /// The tabs and the *More* menu this template asks for, complete.
    pub fn sections(&self) -> ResolvedSections {
        ResolvedSections::from_block(self.sections.as_ref())
    }
}

/// How deeply `{` and `[` nest in a JSON text, ignoring brackets inside strings.
fn nesting_depth(text: &str) -> usize {
    let (mut depth, mut deepest) = (0usize, 0usize);
    let (mut in_string, mut escaped) = (false, false);
    for character in text.chars() {
        if in_string {
            match (escaped, character) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' | '[' => {
                depth += 1;
                deepest = deepest.max(depth);
            }
            '}' | ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    deepest
}
