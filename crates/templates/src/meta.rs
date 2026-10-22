use serde::{Deserialize, Serialize};

use crate::limits;
use crate::TemplateIssue;

/// The icons a workspace can take, by the key the server stores. The web
/// client owns the pictures (`web/workspaceIcons.tsx`); a test compares the
/// two lists, so a template can only name an icon the client can draw.
pub const WORKSPACE_ICONS: [&str; 32] = [
    "stack",
    "rocket",
    "code",
    "api",
    "server",
    "database",
    "cloud",
    "cpu",
    "mobile",
    "world",
    "lock",
    "bolt",
    "brain",
    "flask",
    "bulb",
    "puzzle",
    "tools",
    "chart",
    "bank",
    "card",
    "cart",
    "store",
    "truck",
    "briefcase",
    "users",
    "messages",
    "book",
    "school",
    "heartbeat",
    "leaf",
    "palette",
    "compass",
];

/// What identifies a template and how the picker shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateMeta {
    pub id: String,
    /// Raised when a template file changes, so a workspace can tell that the
    /// server it was set on is newer than this one.
    pub version: u32,
    pub name: String,
    pub summary: String,
    /// One of [`WORKSPACE_ICONS`].
    pub icon: String,
}

impl TemplateMeta {
    pub(crate) fn validate(&self, issues: &mut Vec<TemplateIssue>) {
        if let Some(problem) = id_problem(&self.id) {
            issues.push(TemplateIssue::new("meta.id", problem));
        }
        if self.version == 0 || self.version > limits::VERSION_MAX {
            issues.push(TemplateIssue::new(
                "meta.version",
                format!(
                    "the version is {}; it is a whole number from 1 to {}",
                    self.version,
                    limits::VERSION_MAX
                ),
            ));
        }
        if let Some(problem) = text_problem(&self.name, limits::NAME_MAX) {
            issues.push(TemplateIssue::new("meta.name", problem));
        }
        if let Some(problem) = text_problem(&self.summary, limits::SUMMARY_MAX) {
            issues.push(TemplateIssue::new("meta.summary", problem));
        }
        if !WORKSPACE_ICONS.contains(&self.icon.as_str()) {
            issues.push(TemplateIssue::new(
                "meta.icon",
                format!("`{}` is not a workspace icon", self.icon),
            ));
        }
    }
}

fn id_problem(id: &str) -> Option<String> {
    let length = id.chars().count();
    if !(limits::ID_MIN..=limits::ID_MAX).contains(&length) {
        return Some(format!(
            "the id is {length} characters; it is {} to {}",
            limits::ID_MIN,
            limits::ID_MAX
        ));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Some("the id uses only a-z, 0-9 and -".to_owned());
    }
    if id.starts_with('-') || id.ends_with('-') || id.contains("--") {
        return Some("the id does not start or end with -, and has no --".to_owned());
    }
    None
}

/// A line of display text: not empty, no longer than `max` characters, no control characters.
fn text_problem(text: &str, max: usize) -> Option<String> {
    let length = text.chars().count();
    if text.trim().is_empty() {
        return Some("it is empty".to_owned());
    }
    if length > max {
        return Some(format!("it is {length} characters; the most is {max}"));
    }
    if text.chars().any(char::is_control) {
        return Some("it has a control character".to_owned());
    }
    if text.trim() != text {
        return Some("it starts or ends with a space".to_owned());
    }
    None
}
