use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::TemplateIssue;

/// The jobs the workspace assistant can do, by the id the API uses (`AgentCapability` in the domain crate; a test in
/// the api crate matches on every one, so this list cannot fall behind).
pub const CAPABILITIES: [&str; 13] = [
    "find_files",
    "find_symbol",
    "search_content",
    "summarize_file",
    "ask_question",
    "workspace_overview",
    "save_memory",
    "manage_files",
    "summarize_chat",
    "analyze_files",
    "recall_memory",
    "manage_memory",
    "create_note",
];

/// How many suggested questions a template may give, and how long each may be, in characters.
pub const SUGGESTIONS_MAX: usize = 6;
pub const SUGGESTION_MAX: usize = 120;

/// How many phrase rules a template may add to the assistant's router, and how long a phrase and the focus may be.
pub const PHRASES_MAX: usize = 12;
pub const PHRASE_MAX: usize = 60;
pub const FOCUS_MAX: usize = 160;
/// The most characters of the starting `CAIRN.md` a template ships, and of the note CAIRN reads (V0.9.49).
pub const INSTRUCTIONS_MAX: usize = 2000;

/// The most playbooks a template ships, and the limits of one (V0.9.54).
pub const PLAYBOOKS_MAX: usize = 5;
pub const PLAYBOOK_NAME_MAX: usize = 60;
pub const PLAYBOOK_DESCRIPTION_MAX: usize = 200;
pub const PLAYBOOK_WHEN_MAX: usize = 300;
pub const PLAYBOOK_STEPS_MAX: usize = 4000;

/// A procedure a template ships for CAIRN (V0.9.54). A workspace made from the template gets it as a note in its `Playbooks` folder,
/// which CAIRN offers only once a writer has verified it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaybookStarter {
    pub name: String,
    pub description: String,
    /// When CAIRN should use it.
    pub when: String,
    /// The tools it expects, by name; it grants none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    /// The steps, in Markdown.
    pub steps: String,
}

impl PlaybookStarter {
    /// The note a workspace gets: a header between two `---` lines, then the steps.
    pub fn note_text(&self) -> String {
        let tools = if self.tools.is_empty() {
            String::new()
        } else {
            format!("tools: [{}]\n", self.tools.join(", "))
        };
        format!(
            "---\nname: {}\ndescription: {}\nwhen: {}\n{tools}---\n{}\n",
            self.name,
            self.description,
            self.when,
            self.steps.trim()
        )
    }
}

/// A phrase the assistant's keyword router recognises (V0.2.185) and the job it maps onto. It is tried after the
/// router's own rules and before the AI provider is asked, so it can only add: it never takes a message from a rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phrase {
    /// Whole words, compared without regard to case: "runbook for", "meeting notes".
    pub phrase: String,
    /// One of [`CAPABILITIES`].
    pub capability: String,
}

/// What the assistant shows first and suggests (V0.2.176). Every key is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssistantBlock {
    /// Capabilities listed before the others in the assistant's buttons and the command palette, in this order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    /// Questions the assistant offers as buttons. Choosing one puts it in the message box and sends nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<String>,
    /// Phrases the keyword router recognises after its own rules (V0.2.185), at most 12.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phrases: Vec<Phrase>,
    /// One line about what the workspace is for, added to the Ask prompt as context (V0.2.185), at most 160 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus: Option<String>,
    /// The text of the `CAIRN.md` note a workspace made from the template starts with (V0.9.49): how the team wants CAIRN to work,
    /// at most 2,000 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Playbooks a workspace made from the template starts with (V0.9.54), at most 5.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub playbooks: Vec<PlaybookStarter>,
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
    if block.phrases.len() > PHRASES_MAX {
        issues.push(TemplateIssue::new(
            "assistant.phrases",
            format!(
                "there are {} phrases; a template gives at most {PHRASES_MAX}",
                block.phrases.len()
            ),
        ));
    }
    let mut said = HashSet::new();
    for (index, rule) in block.phrases.iter().enumerate() {
        let path = format!("assistant.phrases[{index}]");
        let length = rule.phrase.chars().count();
        let problem = if rule.phrase.trim().is_empty() {
            Some("the phrase is empty".to_owned())
        } else if length > PHRASE_MAX {
            Some(format!("the phrase is {length} characters; the most is {PHRASE_MAX}"))
        } else if rule.phrase.trim() != rule.phrase {
            Some("the phrase starts or ends with a space".to_owned())
        } else if rule
            .phrase
            .chars()
            .any(|c| c.is_control() || !(c.is_alphanumeric() || " -'".contains(c)))
        {
            Some("a phrase is words: letters, digits, spaces, hyphens and apostrophes".to_owned())
        } else if !said.insert(rule.phrase.to_lowercase()) {
            Some("the phrase is given twice".to_owned())
        } else {
            None
        };
        if let Some(problem) = problem {
            issues.push(TemplateIssue::new(format!("{path}.phrase"), problem));
        }
        if !CAPABILITIES.contains(&rule.capability.as_str()) {
            issues.push(TemplateIssue::new(
                format!("{path}.capability"),
                format!("`{}` is not something the assistant can do", rule.capability),
            ));
        }
    }
    if let Some(focus) = &block.focus {
        let length = focus.chars().count();
        let problem = if focus.trim().is_empty() {
            Some("the focus is empty".to_owned())
        } else if length > FOCUS_MAX {
            Some(format!("the focus is {length} characters; the most is {FOCUS_MAX}"))
        } else if focus.trim() != focus {
            Some("the focus starts or ends with a space".to_owned())
        } else if focus.chars().any(char::is_control) {
            Some("the focus has a control character".to_owned())
        } else {
            None
        };
        issues.extend(problem.map(|problem| TemplateIssue::new("assistant.focus", problem)));
    }
    if block.playbooks.len() > PLAYBOOKS_MAX {
        issues.push(TemplateIssue::new(
            "assistant.playbooks",
            format!(
                "there are {} playbooks; a template gives at most {PLAYBOOKS_MAX}",
                block.playbooks.len()
            ),
        ));
    }
    let mut named = HashSet::new();
    for (index, playbook) in block.playbooks.iter().enumerate() {
        let path = format!("assistant.playbooks[{index}]");
        let problem = if playbook.name.trim().is_empty()
            || playbook.name.chars().count() > PLAYBOOK_NAME_MAX
            || playbook.name.trim() != playbook.name
            || !playbook
                .name
                .chars()
                .all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == '_')
        {
            Some(format!(
                "the name is at most {PLAYBOOK_NAME_MAX} letters, digits, spaces, - or _"
            ))
        } else if !named.insert(playbook.name.to_lowercase()) {
            Some("the name is given twice".to_owned())
        } else if playbook.description.trim().is_empty()
            || playbook.description.chars().count() > PLAYBOOK_DESCRIPTION_MAX
            || playbook.description.contains('\n')
        {
            Some(format!(
                "the description is one line of at most {PLAYBOOK_DESCRIPTION_MAX} characters"
            ))
        } else if playbook.when.trim().is_empty()
            || playbook.when.chars().count() > PLAYBOOK_WHEN_MAX
            || playbook.when.contains('\n')
        {
            Some(format!("`when` is one line of at most {PLAYBOOK_WHEN_MAX} characters"))
        } else if playbook.steps.trim().is_empty() || playbook.steps.chars().count() > PLAYBOOK_STEPS_MAX {
            Some(format!(
                "the steps are not empty and at most {PLAYBOOK_STEPS_MAX} characters"
            ))
        } else if playbook
            .tools
            .iter()
            .any(|tool| tool.is_empty() || !tool.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        {
            Some("a tool is named in lower case letters and _".to_owned())
        } else {
            None
        };
        if let Some(problem) = problem {
            issues.push(TemplateIssue::new(path, problem));
        }
    }
    if let Some(instructions) = &block.instructions {
        let length = instructions.chars().count();
        let problem = if instructions.trim().is_empty() {
            Some("the instructions are empty".to_owned())
        } else if length > INSTRUCTIONS_MAX {
            Some(format!(
                "the instructions are {length} characters; the most is {INSTRUCTIONS_MAX}"
            ))
        } else {
            None
        };
        issues.extend(problem.map(|problem| TemplateIssue::new("assistant.instructions", problem)));
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
