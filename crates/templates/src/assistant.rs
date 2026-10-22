use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::TemplateIssue;

/// The jobs the workspace assistant can do, by the id the API uses (`AgentCapability` in the domain crate; a test in
/// the api crate matches on every one, so this list cannot fall behind).
pub const CAPABILITIES: [&str; 6] = [
    "find_files",
    "find_symbol",
    "search_content",
    "summarize_file",
    "ask_question",
    "workspace_overview",
];

/// How many suggested questions a template may give, and how long each may be, in characters.
pub const SUGGESTIONS_MAX: usize = 6;
pub const SUGGESTION_MAX: usize = 120;

/// What the assistant shows first and suggests (V0.2.176). Both keys are optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantBlock {
    /// Capabilities listed before the others in the assistant's buttons and the command palette, in this order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    /// Questions the assistant offers as buttons. Choosing one puts it in the message box and sends nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
}

pub(crate) fn validate(block: &AssistantBlock, issues: &mut Vec<TemplateIssue>) {
    let mut seen = HashSet::new();
    for (index, capability) in block.order.iter().enumerate() {
        let path = format!("assistant.order[{index}]");
        if !CAPABILITIES.contains(&capability.as_str()) {
            issues.push(TemplateIssue::new(
                path,
                format!("`{capability}` is not something the assistant can do"),
            ));
        } else if !seen.insert(capability.as_str()) {
            issues.push(TemplateIssue::new(path, format!("`{capability}` is listed twice")));
        }
    }
    if block.suggestions.len() > SUGGESTIONS_MAX {
        issues.push(TemplateIssue::new(
            "assistant.suggestions",
            format!(
                "there are {} suggestions; a template gives at most {SUGGESTIONS_MAX}",
                block.suggestions.len()
            ),
        ));
    }
    let mut asked = HashSet::new();
    for (index, suggestion) in block.suggestions.iter().enumerate() {
        let path = format!("assistant.suggestions[{index}]");
        let length = suggestion.chars().count();
        let problem = if suggestion.trim().is_empty() {
            Some("the suggestion is empty".to_owned())
        } else if length > SUGGESTION_MAX {
            Some(format!(
                "the suggestion is {length} characters; the most is {SUGGESTION_MAX}"
            ))
        } else if suggestion.trim() != suggestion {
            Some("the suggestion starts or ends with a space".to_owned())
        } else if suggestion.chars().any(char::is_control) {
            Some("the suggestion has a control character".to_owned())
        } else if !asked.insert(suggestion.to_lowercase()) {
            Some("the suggestion is given twice".to_owned())
        } else {
            None
        };
        if let Some(problem) = problem {
            issues.push(TemplateIssue::new(path, problem));
        }
    }
}
