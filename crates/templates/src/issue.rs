use std::fmt;

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
            path: path.into(),
            message: message.into(),
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
            message,
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
