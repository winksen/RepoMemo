use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{TemplateIssue, WORKSPACE_SECTIONS};

/// The kinds of evidence a file can be, by the id the API uses (`ArtifactType` in the domain crate; a test in the
/// api crate checks that this list names every one).
pub const ARTIFACT_TYPES: [&str; 12] = [
    "file",
    "markdown_doc",
    "code_file",
    "image",
    "issue",
    "pr",
    "decision",
    "incident",
    "runbook",
    "api_spec",
    "note",
    "repository",
];

/// The names a template gives to sections and to kinds of evidence (V0.2.173). They are display text only: the
/// ids, the addresses and every rule stay as they were.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelsBlock {
    /// A section id to the name shown for it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sections: BTreeMap<String, String>,
    /// An artifact type id to the name shown for it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub artifact_types: BTreeMap<String, String>,
}

pub(crate) fn validate(block: &LabelsBlock, issues: &mut Vec<TemplateIssue>) {
    for (list, vocabulary, labels) in [
        ("sections", WORKSPACE_SECTIONS.as_slice(), &block.sections),
        ("artifact_types", ARTIFACT_TYPES.as_slice(), &block.artifact_types),
    ] {
        for (id, label) in labels {
            let path = format!("labels.{list}.{id}");
            if !vocabulary.contains(&id.as_str()) {
                issues.push(TemplateIssue::new(
                    path,
                    format!("`{id}` is not a known {}", list.trim_end_matches('s').replace('_', " ")),
                ));
            } else if let Some(problem) = label_problem(label) {
                issues.push(TemplateIssue::new(path, problem));
            }
        }
    }
}

fn label_problem(label: &str) -> Option<String> {
    let length = label.chars().count();
    if label.trim().is_empty() {
        Some("the name is empty".to_owned())
    } else if length > crate::limits::LABEL_MAX {
        Some(format!(
            "the name is {length} characters; the most is {}",
            crate::limits::LABEL_MAX
        ))
    } else if label.trim() != label {
        Some("the name starts or ends with a space".to_owned())
    } else if label.chars().any(char::is_control) {
        Some("the name has a control character".to_owned())
    } else {
        None
    }
}
