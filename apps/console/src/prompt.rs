//! The interactive prompt: history, Tab completion of command names and usage
//! hints as you type, like the prompt of System › Console in the web app.

use std::borrow::Cow;

use rustyline::{
    completion::Completer,
    highlight::Highlighter,
    hint::{Hint, Hinter},
    history::DefaultHistory,
    validate::Validator,
    CompletionType, Config, Context, Editor, Helper,
};

use crate::client::CommandInfo;

pub const PROMPT: &str = "repomemo › ";

pub struct ConsoleHelper {
    commands: Vec<CommandInfo>,
    color: bool,
}

impl ConsoleHelper {
    pub fn new(commands: &[CommandInfo], color: bool) -> Self {
        let mut commands = commands.to_vec();
        // Handled by the terminal itself, never sent to the server.
        for (name, usage) in [("clear", "clear (empties the screen)"), ("exit", "exit (leaves the console)")] {
            commands.push(CommandInfo { name: name.to_owned(), usage: usage.to_owned() });
        }
        Self { commands, color }
    }
}

/// The words typed so far, lower case and single-spaced.
fn typed(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

pub fn editor(helper: ConsoleHelper) -> rustyline::Result<Editor<ConsoleHelper, DefaultHistory>> {
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .auto_add_history(false)
        .build();
    let mut editor = Editor::with_config(config)?;
    editor.set_helper(Some(helper));
    Ok(editor)
}

/// A hint shown in grey after the cursor. Only the rest of a command name can
/// be accepted (Right arrow); a usage line is there to read.
pub struct ConsoleHint {
    display: String,
    completion: Option<String>,
}

impl Hint for ConsoleHint {
    fn display(&self) -> &str {
        &self.display
    }

    fn completion(&self) -> Option<&str> {
        self.completion.as_deref()
    }
}

impl Hinter for ConsoleHelper {
    type Hint = ConsoleHint;

    fn hint(&self, line: &str, pos: usize, _context: &Context<'_>) -> Option<ConsoleHint> {
        let words = typed(line);
        if pos < line.len() || words.is_empty() {
            return None;
        }
        // A command typed in full: how to use it.
        let named = self
            .commands
            .iter()
            .filter(|command| words == command.name || words.starts_with(&format!("{} ", command.name)))
            .max_by_key(|command| command.name.len());
        if let Some(command) = named {
            return Some(ConsoleHint { display: format!("   {}", command.usage), completion: None });
        }
        // Part of a name: the rest when one command fits, else the choices.
        let candidates = self.commands.iter().filter(|command| command.name.starts_with(&words)).collect::<Vec<_>>();
        match candidates.as_slice() {
            [] => None,
            [only] if !line.ends_with(char::is_whitespace) => {
                let rest = only.name[words.len()..].to_owned();
                Some(ConsoleHint { display: rest.clone(), completion: Some(rest) })
            }
            many => Some(ConsoleHint {
                display: format!("   {}", many.iter().map(|command| command.name.as_str()).collect::<Vec<_>>().join(" · ")),
                completion: None,
            }),
        }
    }
}

impl Completer for ConsoleHelper {
    type Candidate = String;

    fn complete(&self, line: &str, pos: usize, _context: &Context<'_>) -> rustyline::Result<(usize, Vec<String>)> {
        let before = &line[..pos];
        let start = before.len() - before.trim_start().len();
        let words = before[start..].to_lowercase();
        let candidates = self
            .commands
            .iter()
            .filter(|command| command.name.starts_with(&words) && command.name != words.trim_end())
            .map(|command| format!("{} ", command.name))
            .collect();
        Ok((start, candidates))
    }
}

impl Highlighter for ConsoleHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(&'s self, prompt: &'p str, _default: bool) -> Cow<'b, str> {
        if self.color { Cow::Owned(format!("\x1b[1;38;5;36m{prompt}\x1b[0m")) } else { Cow::Borrowed(prompt) }
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        if self.color { Cow::Owned(format!("\x1b[90m{hint}\x1b[0m")) } else { Cow::Borrowed(hint) }
    }
}

impl Validator for ConsoleHelper {}

impl Helper for ConsoleHelper {}

#[cfg(test)]
mod tests {
    use rustyline::history::DefaultHistory;

    use super::*;

    fn helper() -> ConsoleHelper {
        let command = |name: &str, usage: &str| CommandInfo { name: name.to_owned(), usage: usage.to_owned() };
        ConsoleHelper::new(&[command("status", "status"), command("jobs list", "jobs list [--status s]"), command("jobs cancel", "jobs cancel <job-id> --yes")], false)
    }

    #[test]
    fn hints_complete_names_and_show_usage() {
        let helper = helper();
        let history = DefaultHistory::new();
        let context = Context::new(&history);
        let hint = |line: &str| helper.hint(line, line.len(), &context).map(|hint| (hint.display, hint.completion));
        assert_eq!(hint("sta"), Some(("tus".to_owned(), Some("tus".to_owned()))));
        assert_eq!(hint("jobs"), Some(("   jobs list · jobs cancel".to_owned(), None)));
        assert_eq!(hint("JOBS  cancel abc"), Some(("   jobs cancel <job-id> --yes".to_owned(), None)));
        assert_eq!(hint("nope"), None);
    }

    #[test]
    fn tab_completes_command_names() {
        let helper = helper();
        let history = DefaultHistory::new();
        let context = Context::new(&history);
        assert_eq!(helper.complete("  jobs c", 8, &context).unwrap(), (2, vec!["jobs cancel ".to_owned()]));
        assert_eq!(helper.complete("st", 2, &context).unwrap(), (0, vec!["status ".to_owned()]));
    }
}
