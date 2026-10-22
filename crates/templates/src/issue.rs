use std::fmt;

/// The most characters of one message or path kept.
const TEXT_MAX: usize = 300;

/// A text that came from a template file or its name, made safe to show (V0.2.192): a control character or a
/// character that reorders text is written as an escape (`\n`, `\u{1b}`), and a long text is cut. Messages quote the
/// file (an unknown key, a word outside a list), so without this a key holding a line break or a terminal escape would
/// reach the log, System › Overview and an administrator's terminal as it is.
pub(crate) fn clean_text(text: &str) -> String {
    let mut cleaned = String::new();
    for (count, character) in text.chars().enumerate() {
        if count >= TEXT_MAX {
            cleaned.push('…');
            break;
        }
        let reorders = matches!(character, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{feff}');
        if character.is_control() || reorders {
            cleaned.extend(character.escape_default());
        } else {
            cleaned.push(character);
        }
    }
    cleaned
}

/// One problem found in a template: where it is and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateIssue {
    /// The key it concerns, as `meta.id`; empty for the file as a whole.
    pub path: String,
    pub message: String,
    /// Set when the JSON reader found it.
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl TemplateIssue {
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: clean_text(&path.into()),
            message: clean_text(&message.into()),
            line: None,
            column: None,
        }
    }

    pub(crate) fn from_json(error: &serde_json::Error) -> Self {
        let full = error.to_string();
        // The reader appends " at line L column C"; the position is kept in its own fields.
        let message = match full.rfind(" at line ") {
            Some(at) => full[..at].to_owned(),
            None => full,
        };
        let (line, column) = (error.line(), error.column());
        Self {
            path: String::new(),
            message: clean_text(&message),
            line: (line > 0).then_some(line),
            column: (column > 0).then_some(column),
        }
    }
}

impl fmt::Display for TemplateIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.path.is_empty() {
            write!(formatter, "{}: ", self.path)?;
        }
        write!(formatter, "{}", self.message)?;
        match (self.line, self.column) {
            (Some(line), Some(column)) => write!(formatter, " (line {line}, column {column})"),
            (Some(line), None) => write!(formatter, " (line {line})"),
            _ => Ok(()),
        }
    }
}
