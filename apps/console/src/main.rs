//! `repomemo-console`: the RepoMemo admin console in a terminal.
//!
//! Signs in to a RepoMemo server and runs the same commands as System › Console
//! in the web app, through the same endpoint (`/v1/system/console`), so both
//! always offer the same commands with the same rights and audit records.
//! Without a command it opens an interactive console (logo, history, Tab
//! completion, usage hints); with one it runs it and exits, for scripts.
//!
//! Design: `mindmap/technical/admin-console.md`.

mod client;
mod prompt;
mod render;

use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
    process::ExitCode,
};

use rustyline::error::ReadlineError;

use client::{Client, Failure};
use render::Paint;

const DEFAULT_SERVER: &str = "http://127.0.0.1:3020";

const USAGE: &str = "\
Usage: repomemo-console [options] [command...]

Opens the RepoMemo admin console. With a command, runs it once and exits.

Options:
  --server <url>   The server (default: REPOMEMO_SERVER_URL, else http://127.0.0.1:3020)
  --email <email>  Sign in as this account (default: REPOMEMO_EMAIL); the password is asked for
  -h, --help       Show this help
  -V, --version    Show the version

Environment:
  REPOMEMO_TOKEN   An access token to use instead of signing in, for scripts

Exit codes: 0 done, 1 the server refused the command, 2 sign-in or connection problem.

Examples:
  repomemo-console
  repomemo-console status
  repomemo-console jobs list --status failed --json
  repomemo-console settings set log_http_level debug --yes
";

/// The server refused the command (usage, rights, conflict).
const EXIT_REFUSED: u8 = 1;
/// Sign-in, connection or server trouble.
const EXIT_TROUBLE: u8 = 2;

struct Options {
    server: String,
    email: Option<String>,
    /// The words of a single command to run, or empty for the interactive console.
    command: Vec<String>,
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.trim().is_empty())
}

impl Options {
    /// Options come first; the first other word starts the command, and
    /// everything after it belongs to the command (`jobs list --status failed`).
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut options = Options {
            server: non_empty_env("REPOMEMO_SERVER_URL").unwrap_or_else(|| DEFAULT_SERVER.to_owned()),
            email: non_empty_env("REPOMEMO_EMAIL"),
            command: Vec::new(),
        };
        while let Some(arg) = args.next() {
            if let Some(value) = arg.strip_prefix("--server=") {
                options.server = value.to_owned();
                continue;
            }
            if let Some(value) = arg.strip_prefix("--email=") {
                options.email = Some(value.to_owned());
                continue;
            }
            match arg.as_str() {
                "-h" | "--help" => {
                    print!("{USAGE}");
                    std::process::exit(0);
                }
                "-V" | "--version" => {
                    println!("repomemo-console {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                "--server" => options.server = args.next().ok_or_else(|| "--server needs a URL.".to_owned())?,
                "--email" => options.email = Some(args.next().ok_or_else(|| "--email needs an address.".to_owned())?),
                "--" => {
                    options.command.extend(args.by_ref());
                    break;
                }
                _ => {
                    options.command.push(arg);
                    options.command.extend(args.by_ref());
                    break;
                }
            }
        }
        Ok(options)
    }
}

/// Rebuilds one line from the shell's words, quoting those the shell unquoted.
fn join_words(words: &[String]) -> String {
    words
        .iter()
        .map(|word| match (word.is_empty() || word.chars().any(char::is_whitespace), word.contains('"')) {
            (false, _) => word.clone(),
            (true, false) => format!("\"{word}\""),
            (true, true) => format!("'{word}'"),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `--json` is handled here, as in the web console: the server never sees it.
fn strip_json(line: &str) -> (String, bool) {
    let raw = line.split_whitespace().any(|word| word == "--json");
    if !raw {
        return (line.to_owned(), false);
    }
    (line.split_whitespace().filter(|word| *word != "--json").collect::<Vec<_>>().join(" "), true)
}

fn describe(failure: &Failure, client: &Client) -> String {
    match failure {
        Failure::Refused(message) | Failure::Server(message) => message.clone(),
        Failure::Expired => "Your session has ended. Sign in again.".to_owned(),
        Failure::Unreachable(detail) => format!(
            "Cannot reach the RepoMemo server at {} ({detail}). Is it running? Use --server or REPOMEMO_SERVER_URL for another address.",
            client.base()
        ),
    }
}

fn history_path() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(home).join(".repomemo").join("console-history.txt"))
}

fn ask(label: &str) -> Result<String, ExitCode> {
    print!("{label}");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    match io::stdin().read_line(&mut answer) {
        Ok(0) | Err(_) => Err(ExitCode::from(EXIT_TROUBLE)),
        Ok(_) => Ok(answer.trim().to_owned()),
    }
}

/// Asks for the email (unless given) and the password, hidden. Three tries.
async fn sign_in(client: &mut Client, options: &Options, paint: &Paint) -> Result<(), ExitCode> {
    if !io::stdin().is_terminal() {
        eprintln!("{}", paint.error("Signing in needs a terminal. For scripts, set REPOMEMO_TOKEN to an access token."));
        return Err(ExitCode::from(EXIT_TROUBLE));
    }
    println!("{}", paint.muted(&format!("Sign in to {} with a system or app administrator account.", client.base())));
    for _ in 0..3 {
        let email = match &options.email {
            Some(email) => {
                println!("Email: {email}");
                email.clone()
            }
            None => ask("Email: ")?,
        };
        let password = rpassword::prompt_password("Password: ").map_err(|_| ExitCode::from(EXIT_TROUBLE))?;
        match client.sign_in(&email, &password).await {
            Ok(()) => return Ok(()),
            Err(failure @ Failure::Unreachable(_)) => {
                eprintln!("{}", paint.error(&describe(&failure, client)));
                return Err(ExitCode::from(EXIT_TROUBLE));
            }
            Err(failure) => eprintln!("{}", paint.error(&describe(&failure, client))),
        }
    }
    Err(ExitCode::from(EXIT_TROUBLE))
}

enum Outcome {
    Done,
    Refused,
    Expired,
    Trouble,
}

/// Runs one line and prints its answer.
async fn execute(client: &mut Client, paint: &Paint, line: &str) -> Outcome {
    let (command, raw) = strip_json(line);
    match client.run(&command).await {
        Ok(response) => {
            if raw {
                println!("{}", serde_json::to_string_pretty(&response).unwrap_or_default());
            } else {
                print!("{}", render::answer(paint, &response));
            }
            Outcome::Done
        }
        Err(Failure::Refused(message)) => {
            eprintln!("{}", paint.error(&message));
            Outcome::Refused
        }
        Err(Failure::Expired) => Outcome::Expired,
        Err(failure) => {
            eprintln!("{}", paint.error(&describe(&failure, client)));
            Outcome::Trouble
        }
    }
}

async fn interactive(client: &mut Client, paint: &Paint, options: &Options, welcome: &client::Welcome) -> ExitCode {
    let mut editor = match prompt::editor(prompt::ConsoleHelper::new(&welcome.commands, paint.enabled())) {
        Ok(editor) => editor,
        Err(error) => {
            eprintln!("{}", paint.error(&format!("The console could not open: {error}")));
            return ExitCode::from(EXIT_TROUBLE);
        }
    };
    let history = history_path();
    if let Some(path) = &history {
        let _ = editor.load_history(path);
    }
    loop {
        match editor.readline(prompt::PROMPT) {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let _ = editor.add_history_entry(line);
                match line {
                    "exit" | "quit" => break,
                    "clear" | "cls" => {
                        let _ = editor.clear_screen();
                        continue;
                    }
                    _ => {}
                }
                if let Outcome::Expired = execute(client, paint, line).await {
                    println!("{}", paint.warning("Your session has ended."));
                    if sign_in(client, options, paint).await.is_err() {
                        break;
                    }
                    execute(client, paint, line).await;
                }
                println!();
            }
            Err(ReadlineError::Interrupted) => println!("{}", paint.muted("Type exit or press Ctrl+D to leave.")),
            Err(ReadlineError::Eof) => break,
            Err(error) => {
                eprintln!("{}", paint.error(&error.to_string()));
                break;
            }
        }
    }
    if let Some(path) = &history {
        if let Some(folder) = path.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        let _ = editor.save_history(path);
    }
    println!("{}", paint.muted("Bye."));
    ExitCode::SUCCESS
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let options = match Options::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            return ExitCode::from(EXIT_TROUBLE);
        }
    };
    let paint = Paint::detect();
    let single = !options.command.is_empty();
    let mut client = Client::new(&options.server, non_empty_env("REPOMEMO_TOKEN"));
    if !single {
        println!("\n{}", render::banner(&paint));
    }
    if !client.is_signed_in() {
        if let Err(code) = sign_in(&mut client, &options, &paint).await {
            return code;
        }
    }
    let welcome = match client.welcome().await {
        Ok(welcome) => welcome,
        Err(failure) => {
            eprintln!("{}", paint.error(&describe(&failure, &client)));
            return ExitCode::from(if matches!(failure, Failure::Refused(_)) { EXIT_REFUSED } else { EXIT_TROUBLE });
        }
    };
    if single {
        return match execute(&mut client, &paint, &join_words(&options.command)).await {
            Outcome::Done => ExitCode::SUCCESS,
            Outcome::Refused => ExitCode::from(EXIT_REFUSED),
            Outcome::Expired => {
                eprintln!("{}", paint.error(&describe(&Failure::Expired, &client)));
                ExitCode::from(EXIT_TROUBLE)
            }
            Outcome::Trouble => ExitCode::from(EXIT_TROUBLE),
        };
    }
    println!("{}", render::greeting(&paint, &welcome, client.base()));
    interactive(&mut client, &paint, &options, &welcome).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn options_come_before_the_command() {
        let options = Options::parse(words(&["--server", "http://host:3020", "jobs", "list", "--status", "failed"]).into_iter()).unwrap();
        assert_eq!(options.server, "http://host:3020");
        assert_eq!(options.command, words(&["jobs", "list", "--status", "failed"]));
        let options = Options::parse(words(&["--email=a@b.c", "--", "--json", "status"]).into_iter()).unwrap();
        assert_eq!((options.email.as_deref(), options.command), (Some("a@b.c"), words(&["--json", "status"])));
        assert!(Options::parse(words(&["--server"]).into_iter()).is_err());
    }

    #[test]
    fn shell_words_become_one_line_again() {
        assert_eq!(join_words(&words(&["logs", "--grep", "sign in failed"])), r#"logs --grep "sign in failed""#);
        assert_eq!(join_words(&words(&["logs", "--grep", r#"say "hi" now"#])), r#"logs --grep 'say "hi" now'"#);
        assert_eq!(strip_json("jobs list --json"), ("jobs list".to_owned(), true));
        assert_eq!(strip_json("status"), ("status".to_owned(), false));
    }
}
