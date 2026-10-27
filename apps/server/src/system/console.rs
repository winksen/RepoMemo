//! The admin console: typed commands for technical administrators.
//!
//! `POST /v1/system/console` takes one line such as `jobs list --status failed`
//! and runs it through the same handler the System pages call, so rights,
//! validation and audit records stay the same. The console only parses, calls
//! the handler and picks how the answer is best shown. Commands that change
//! something are dry runs unless the line carries `--yes`; when they run, a
//! `console_command` audit event records the exact line.
//!
//! Design: `mindmap/technical/admin-console.md`.

use std::collections::BTreeMap;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{
    apply_setting, audit, read_setting, require_system_admin, spec, JobsQuery, LogsQuery, MaintenanceStatus,
    SettingKind, UpdateSettingsRequest,
};
use crate::{
    logs::CATEGORIES, map_storage_error, ApiError, AppState, AuthenticatedSubject, ReadingSubject, SessionSubject,
};

/// Longer lines are refused before they are parsed.
const MAX_COMMAND_LENGTH: usize = 2_000;

const LOG_LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];

/// One command of the catalog.
#[derive(Debug, Serialize)]
pub(crate) struct CommandSpec {
    name: &'static str,
    usage: &'static str,
    summary: &'static str,
    /// Changes something: a dry run unless `--yes` is given.
    changes: bool,
    /// Words that must follow the name.
    #[serde(skip)]
    arguments: usize,
    /// The `--flag value` options it accepts (`--yes` is accepted everywhere).
    #[serde(skip)]
    flags: &'static [&'static str],
}

const COMMANDS: &[CommandSpec] = &[
    CommandSpec { name: "help", usage: "help [command]", summary: "Lists the commands, or the ones starting with a word.", changes: false, arguments: 0, flags: &[] },
    CommandSpec { name: "status", usage: "status", summary: "Version, uptime, storage, background work and traffic.", changes: false, arguments: 0, flags: &[] },
    CommandSpec { name: "jobs list", usage: "jobs list [--status pending|running|completed|failed|cancelled] [--limit n]", summary: "Jobs across every workspace, newest first.", changes: false, arguments: 0, flags: &["status", "limit"] },
    CommandSpec { name: "jobs cancel", usage: "jobs cancel <job-id> --yes", summary: "Asks a pending or running job to stop at its next step.", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "settings list", usage: "settings list", summary: "Run-time settings with their value and server default.", changes: false, arguments: 0, flags: &[] },
    CommandSpec { name: "settings set", usage: "settings set <key> <value> --yes", summary: "Changes a run-time setting.", changes: true, arguments: 2, flags: &[] },
    CommandSpec { name: "settings reset", usage: "settings reset <key> --yes", summary: "Restores a setting's server default.", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "logs", usage: "logs [--level warn] [--category security|activity|jobs|http|ai|database|performance|server] [--grep text] [--workspace id] [--day YYYY-MM-DD] [--limit n] [--after sequence] [--follow]", summary: "Log events, newest first: recent ones, or a day's file.", changes: false, arguments: 0, flags: &["level", "category", "grep", "workspace", "day", "limit", "after"] },
    CommandSpec { name: "users list", usage: "users list", summary: "Every account with its roles and sessions.", changes: false, arguments: 0, flags: &[] },
    CommandSpec { name: "users reset-link", usage: "users reset-link <email> --yes", summary: "Creates a one-time link, valid for 24 hours, that lets the account choose a new password.", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "tokens list", usage: "tokens list [--user email]", summary: "Personal access tokens of your account, or of another account (system administrators).", changes: false, arguments: 0, flags: &["user"] },
    CommandSpec { name: "users unlock", usage: "users unlock <email> --yes", summary: "Lifts the sign-in lockouts of an account.", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "users signout", usage: "users signout <email> --yes", summary: "Signs an account out of every device.", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "users deactivate", usage: "users deactivate <email> --yes", summary: "Deactivates an account: it cannot sign in or use a token, its sessions, tokens and sent invitations end, and what it wrote stays. Owners transfer first (system administrators).", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "users reactivate", usage: "users reactivate <email> --yes", summary: "Lets a deactivated account sign in again with its password (system administrators).", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "users anonymize", usage: "users anonymize <email> --yes", summary: "Removes the personal data of a deactivated account for good: address, name, password, picture, memberships, notices and conversations. What it wrote stays, credited to \"Former member\" (system administrators).", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "backup list", usage: "backup list", summary: "Backups kept in the data folder, with the state of the last backup or restore.", changes: false, arguments: 0, flags: &[] },
    CommandSpec { name: "backup create", usage: "backup create --yes", summary: "Starts a backup of the database, stored files and key file; watch it with backup list.", changes: true, arguments: 0, flags: &[] },
    CommandSpec { name: "backup restore", usage: "backup restore <backup> <environment> --yes", summary: "Restores a backup into a new environment folder (it never touches the running one) and checks that it opens.", changes: true, arguments: 2, flags: &[] },
    CommandSpec { name: "backup reseal", usage: "backup reseal <backup> --yes", summary: "Seals a backup made with REPOMEMO_BACKUP_KEY_PREVIOUS again with the current REPOMEMO_BACKUP_KEY; watch it with backup list.", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "backup delete", usage: "backup delete <backup> --yes", summary: "Deletes one backup.", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "maintenance status", usage: "maintenance status", summary: "The last maintenance run and the next one.", changes: false, arguments: 0, flags: &[] },
    CommandSpec { name: "maintenance run", usage: "maintenance run --yes", summary: "Runs a full maintenance pass now.", changes: true, arguments: 0, flags: &[] },
    CommandSpec { name: "mail status", usage: "mail status", summary: "The mail server in use, whether sending is on, and what the outbox holds.", changes: false, arguments: 0, flags: &[] },
    CommandSpec { name: "mail test", usage: "mail test <address> --yes", summary: "Queues a test message to an address; its result is on the outbox entry (mail status) (system administrators).", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "workspace list", usage: "workspace list", summary: "Every workspace on the server with its id (system administrators).", changes: false, arguments: 0, flags: &[] },
    CommandSpec { name: "workspace reindex", usage: "workspace reindex <workspace> --yes", summary: "Re-indexes the files of a workspace that the current indexer has not indexed, as a background job (system administrators). The workspace is its id or its name.", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "repo list", usage: "repo list [--workspace id-or-name]", summary: "Linked repositories across the server, or in one workspace, with their sync state (system administrators).", changes: false, arguments: 0, flags: &["workspace"] },
    CommandSpec { name: "repo sync", usage: "repo sync <repository> --yes", summary: "Syncs a linked repository now, as a background job (system administrators). The repository is its id or its name.", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "templates list", usage: "templates list", summary: "The workspace templates this server offers, built in and from REPOMEMO_TEMPLATES_DIR, and the files of that folder it left out.", changes: false, arguments: 0, flags: &[] },
    CommandSpec { name: "templates show", usage: "templates show <template>", summary: "What one template arranges: its tabs, cards, assistant, search weights, checks, map and repository rules.", changes: false, arguments: 1, flags: &[] },
    CommandSpec { name: "templates validate", usage: "templates validate <file>", summary: "Checks a template file on your machine with the rules the server applies to REPOMEMO_TEMPLATES_DIR. It runs in repomemo-console, which needs no sign-in for it.", changes: false, arguments: 1, flags: &[] },
    CommandSpec { name: "embeddings rebuild", usage: "embeddings rebuild <workspace> --yes", summary: "Removes a workspace's stored vectors and embeds every passage again with its embedding provider, in the background (system administrators).", changes: true, arguments: 1, flags: &[] },
];

/// A group name alone runs its first command.
const GROUP_DEFAULTS: &[(&str, &str)] = &[
    ("jobs", "jobs list"),
    ("settings", "settings list"),
    ("users", "users list"),
    ("tokens", "tokens list"),
    ("maintenance", "maintenance status"),
    ("backup", "backup list"),
    ("mail", "mail status"),
    ("workspace", "workspace list"),
    ("repo", "repo list"),
    ("templates", "templates list"),
];

// ---------------------------------------------------------------------------
// Parsing

/// A parsed line: words, `--flag value` options and the `--yes` confirmation.
#[derive(Debug, Default, PartialEq)]
struct Line {
    words: Vec<String>,
    flags: BTreeMap<String, String>,
    yes: bool,
}

impl Line {
    fn flag(&self, name: &str) -> Option<String> {
        self.flags.get(name).cloned()
    }

    fn limit(&self, default: i64) -> Result<i64, ApiError> {
        match self.flags.get("limit") {
            None => Ok(default),
            Some(value) => value
                .parse::<i64>()
                .ok()
                .filter(|limit| (1..=1000).contains(limit))
                .ok_or_else(|| ApiError::bad_request("--limit must be a number from 1 to 1000.")),
        }
    }
}

/// Splits a line on whitespace, keeping `"double"` and `'single'` quoted
/// parts together.
fn tokenize(line: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quote = None;
    for character in line.chars() {
        match quote {
            Some(open) if character == open => quote = None,
            Some(_) => current.push(character),
            None if character == '"' || character == '\'' => {
                quote = Some(character);
                in_token = true;
            }
            None if character.is_whitespace() => {
                if in_token {
                    tokens.push(std::mem::take(&mut current));
                    in_token = false;
                }
            }
            None => {
                current.push(character);
                in_token = true;
            }
        }
    }
    if quote.is_some() {
        return Err("A quote is not closed.".to_owned());
    }
    if in_token {
        tokens.push(current);
    }
    Ok(tokens)
}

fn parse(line: &str) -> Result<Line, String> {
    let mut parsed = Line::default();
    let mut tokens = tokenize(line)?.into_iter();
    while let Some(token) = tokens.next() {
        if token == "--yes" || token == "-y" {
            parsed.yes = true;
        } else if let Some(option) = token.strip_prefix("--") {
            let (name, value) = match option.split_once('=') {
                Some((name, value)) => (name.to_owned(), value.to_owned()),
                None => {
                    let value = tokens.next().ok_or_else(|| format!("--{option} needs a value."))?;
                    (option.to_owned(), value)
                }
            };
            if name.is_empty() {
                return Err("An option has no name.".to_owned());
            }
            parsed.flags.insert(name.to_lowercase(), value);
        } else {
            parsed.words.push(token);
        }
    }
    Ok(parsed)
}

/// The command a line names, and the words after its name.
fn resolve(words: &[String]) -> Option<(&'static CommandSpec, &[String])> {
    let first = words.first()?.to_lowercase();
    if let Some(second) = words.get(1) {
        let name = format!("{first} {}", second.to_lowercase());
        if let Some(spec) = COMMANDS.iter().find(|spec| spec.name == name) {
            return Some((spec, &words[2..]));
        }
    }
    let name = GROUP_DEFAULTS
        .iter()
        .find(|(group, _)| *group == first && words.len() == 1)
        .map_or(first.as_str(), |(_, name)| *name);
    COMMANDS
        .iter()
        .find(|spec| spec.name == name)
        .map(|spec| (spec, &words[1..]))
}

fn usage_error(spec: &CommandSpec, problem: impl std::fmt::Display) -> ApiError {
    ApiError::bad_request(format!("{problem} Usage: {}", spec.usage))
}

fn check(spec: &CommandSpec, arguments: &[String], line: &Line) -> Result<(), ApiError> {
    if spec.name != "help" && arguments.len() != spec.arguments {
        return Err(usage_error(
            spec,
            format!("{} takes {} argument(s).", spec.name, spec.arguments),
        ));
    }
    if let Some(unknown) = line.flags.keys().find(|flag| !spec.flags.contains(&flag.as_str())) {
        return Err(usage_error(
            spec,
            format!("--{unknown} is not an option of {}.", spec.name),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Answers

/// How the client should show `data`. Scripts can ignore it.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum View {
    /// `data` is an array of objects; these fields are worth a column.
    Table { columns: &'static [&'static str] },
    /// `data` is an array of `[label, value]` pairs, in order.
    Facts,
    /// The summary says it all; `data` is there for `--json`.
    Text,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConsoleResponse {
    command: &'static str,
    summary: String,
    /// Nothing changed because the line had no `--yes`.
    dry_run: bool,
    view: View,
    data: Value,
    /// Something changed: the line goes to the audit trail.
    #[serde(skip)]
    applied: bool,
}

fn answer(spec: &'static CommandSpec, summary: impl Into<String>, view: View, data: Value) -> ConsoleResponse {
    ConsoleResponse {
        command: spec.name,
        summary: summary.into(),
        dry_run: false,
        view,
        data,
        applied: false,
    }
}

fn applied(spec: &'static CommandSpec, summary: impl Into<String>, view: View, data: Value) -> ConsoleResponse {
    ConsoleResponse {
        applied: true,
        ..answer(spec, summary, view, data)
    }
}

fn dry_run(spec: &'static CommandSpec, summary: impl std::fmt::Display) -> ConsoleResponse {
    ConsoleResponse {
        dry_run: true,
        ..answer(
            spec,
            format!("{summary} Nothing changed: add --yes to run it."),
            View::Text,
            Value::Null,
        )
    }
}

fn to_json(value: impl Serialize) -> Result<Value, ApiError> {
    serde_json::to_value(value).map_err(ApiError::internal)
}

/// A value as a person would type it: strings without their quotes.
fn shown(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "none".to_owned(),
        other => other.to_string(),
    }
}

fn bytes(value: &Value) -> String {
    let bytes = value.as_f64().unwrap_or(0.0);
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes;
    let mut unit = 0;
    while size >= 1024.0 && unit < units.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", units[unit])
    }
}

fn duration(seconds: u64) -> String {
    let (days, hours, minutes) = (seconds / 86_400, seconds % 86_400 / 3_600, seconds % 3_600 / 60);
    match (days, hours) {
        (0, 0) => format!("{minutes} min"),
        (0, _) => format!("{hours} h {minutes} min"),
        _ => format!("{days} d {hours} h"),
    }
}

fn maintenance_facts(status: &MaintenanceStatus) -> Value {
    let report = status.last_report.as_ref();
    let number = |pick: fn(&super::MaintenanceReport) -> String| report.map_or_else(|| "—".to_owned(), pick);
    json!([
        ["Running", status.running],
        [
            "Last run",
            status
                .last_finished_at
                .as_deref()
                .map_or("not since the server started".to_owned(), |at| format!(
                    "{at} ({})",
                    status.last_trigger.unwrap_or("scheduled")
                ))
        ],
        [
            "Next run",
            status.next_run_at.as_deref().unwrap_or("scheduled runs off")
        ],
        [
            "Stored files removed",
            number(|report| format!(
                "{} ({})",
                report.blobs_removed,
                bytes(&json!(report.blob_bytes_reclaimed))
            ))
        ],
        [
            "Cached previews removed",
            number(|report| report.previews_removed.to_string())
        ],
        ["Old jobs removed", number(|report| report.jobs_pruned.to_string())],
        [
            "Expired sessions removed",
            number(|report| report.refresh_tokens_purged.to_string())
        ],
        ["Failed steps", number(|report| report.failures.to_string())],
    ])
}

/// Where a template came from, in words.
fn template_source(template: &crate::templates::TemplateListed) -> String {
    match (template.source, &template.file) {
        ("custom", Some(file)) => format!("custom ({file})"),
        ("code", _) => "built into the server (code)".to_owned(),
        _ => "built into the server".to_owned(),
    }
}

/// A problem of a template file, with the place in the file and in the template when they are known.
fn template_issue_text(issue: &crate::templates::TemplateFileIssue) -> String {
    let place = match (issue.line, issue.column) {
        (Some(line), Some(column)) => format!(" (line {line}, column {column})"),
        (Some(line), None) => format!(" (line {line})"),
        _ => String::new(),
    };
    if issue.path.is_empty() {
        format!("{}{place}", issue.message)
    } else {
        format!("{}: {}{place}", issue.path, issue.message)
    }
}

/// Finds an account by email, or by id.
async fn find_account(state: &AppState, who: &str) -> Result<repomemo_domain::SharedUser, ApiError> {
    let by_email = state
        .storage
        .find_user_by_email(&who.to_lowercase())
        .await
        .map_err(map_storage_error)?;
    let found = match by_email {
        Some(user) => Some(user),
        None => state.storage.find_user(who).await.map_err(map_storage_error)?,
    };
    found.ok_or_else(|| ApiError::not_found(format!("No account has the email or id {who}.")))
}

// ---------------------------------------------------------------------------
// Handlers

#[derive(Debug, Deserialize)]
pub(crate) struct ConsoleRequest {
    command: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct WelcomeResponse {
    /// The logo and lines every console shows when it opens.
    banner: &'static str,
    tagline: &'static str,
    tips: &'static str,
    version: &'static str,
    /// Who is signed in, as the console greets them.
    user: String,
    role: &'static str,
    /// The catalog, for help, completion and usage hints.
    commands: &'static [CommandSpec],
}

/// What a console shows when it opens: the same for the terminal client and
/// the web page.
pub(crate) async fn welcome(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<WelcomeResponse>, ApiError> {
    require_system_admin(&subject)?;
    let user = state
        .storage
        .find_user(&subject.user_id)
        .await
        .map_err(map_storage_error)?
        .map(|user| match user.email {
            Some(email) => format!("{} <{email}>", user.display_name),
            None => user.display_name,
        })
        .unwrap_or_else(|| subject.user_id.clone());
    Ok(Json(WelcomeResponse {
        banner: repomemo_domain::console::CONSOLE_BANNER,
        tagline: repomemo_domain::console::CONSOLE_TAGLINE,
        tips: repomemo_domain::console::CONSOLE_TIPS,
        version: state.instance.version,
        user,
        role: if subject.is_system_admin {
            "system administrator"
        } else {
            "app administrator"
        },
        commands: COMMANDS,
    }))
}

/// The console's way into a handler that needs a signed-in session (see
/// `SessionSubject`): only when the console itself was opened with one, by
/// password in `repomemo-console` or in the web app, never with a token.
fn as_session(subject: AuthenticatedSubject) -> Result<SessionSubject, ApiError> {
    if subject.token.is_some() {
        return Err(ApiError::forbidden_because(
            "Changing run-time settings needs a signed-in session (the web app, or repomemo-console with a password); personal access tokens cannot.",
        ));
    }
    Ok(SessionSubject(subject))
}

/// Runs one command line.
pub(crate) async fn run(
    ReadingSubject(subject): ReadingSubject,
    State(state): State<AppState>,
    Json(request): Json<ConsoleRequest>,
) -> Result<Json<ConsoleResponse>, ApiError> {
    require_system_admin(&subject)?;
    let text = request.command.trim();
    if text.chars().count() > MAX_COMMAND_LENGTH {
        return Err(ApiError::bad_request(format!(
            "A command is at most {MAX_COMMAND_LENGTH} characters."
        )));
    }
    let line = parse(text).map_err(ApiError::bad_request)?;
    if line.words.is_empty() {
        return Err(ApiError::bad_request("Type a command, or help to list them."));
    }
    let (spec, arguments) = resolve(&line.words).ok_or_else(|| {
        ApiError::bad_request(format!(
            "{} is not a command. Type help to list them.",
            line.words.join(" ")
        ))
    })?;
    check(spec, arguments, &line)?;
    if spec.changes && line.yes && subject.token.as_ref().is_some_and(|token| token.read_only) {
        return Err(ApiError::forbidden_because(
            "This personal access token is read-only: it can run dry runs and lookups, not changes.",
        ));
    }
    if spec.changes && line.yes {
        super::require_system_change(&subject)?;
    }
    tracing::debug!(user_id = %subject.user_id, command = spec.name, "Console command");
    let response = execute(&state, &subject, spec, arguments, &line).await?;
    if response.applied {
        audit(
            &state,
            &subject,
            "console_command",
            format!("Ran `{text}` from the console."),
        )
        .await;
    }
    Ok(Json(response))
}

async fn execute(
    state: &AppState,
    subject: &AuthenticatedSubject,
    spec: &'static CommandSpec,
    arguments: &[String],
    line: &Line,
) -> Result<ConsoleResponse, ApiError> {
    let (who, state_) = (subject.clone(), State(state.clone()));
    match spec.name {
        "help" => {
            let wanted = arguments.join(" ").to_lowercase();
            let rows = COMMANDS
                .iter()
                .filter(|command| {
                    wanted.is_empty() || command.name == wanted || command.name.starts_with(&format!("{wanted} "))
                })
                .map(
                    |command| json!({"command": command.usage, "summary": command.summary, "changes": command.changes}),
                )
                .collect::<Vec<_>>();
            if rows.is_empty() {
                return Err(ApiError::bad_request(format!(
                    "{wanted} is not a command. Type help to list them."
                )));
            }
            Ok(answer(
                spec,
                "Commands that change something are dry runs until you add --yes.",
                View::Table {
                    columns: &["command", "summary", "changes"],
                },
                Value::Array(rows),
            ))
        }

        "templates list" => {
            let files = &state.template_files;
            let mut rows: Vec<Value> = files
                .templates
                .iter()
                .map(|template| {
                    json!({
                        "id": template.id,
                        "name": template.name,
                        "version": template.version,
                        "source": template_source(template),
                        "state": "available",
                    })
                })
                .collect();
            for problem in &files.problems {
                for issue in &problem.issues {
                    rows.push(json!({
                        "id": "-",
                        "name": problem.file,
                        "version": "-",
                        "source": "custom file",
                        "state": format!("left out: {}", template_issue_text(issue)),
                    }));
                }
            }
            let left_out = files.problems.len();
            let summary = format!(
                "{} template(s) available{}{}.",
                files.templates.len(),
                match &files.folder {
                    Some(folder) => format!(", {} added from {folder}", files.loaded.len()),
                    None => ", no REPOMEMO_TEMPLATES_DIR set".to_owned(),
                },
                if left_out > 0 {
                    format!("; {left_out} file(s) left out")
                } else {
                    String::new()
                }
            );
            Ok(answer(
                spec,
                summary,
                View::Table {
                    columns: &["id", "name", "version", "source", "state"],
                },
                Value::Array(rows),
            ))
        }

        "templates show" => {
            let id = arguments[0].to_lowercase();
            let detail = state.core.template_detail(&id).ok_or_else(|| {
                ApiError::not_found(format!(
                    "No template has the id {}. templates list shows them.",
                    arguments[0]
                ))
            })?;
            let listed = state.template_files.templates.iter().find(|template| template.id == id);
            let source = listed.map_or_else(|| "unknown".to_owned(), template_source);
            let list = |items: &[String]| {
                if items.is_empty() {
                    "none".to_owned()
                } else {
                    items.join(", ")
                }
            };
            let renamed: Vec<String> = detail
                .labels
                .sections
                .iter()
                .chain(&detail.labels.artifact_types)
                .map(|(from, to)| format!("{from} → {to}"))
                .collect();
            let boosts = detail.ranking.boosts.len();
            let facts = json!([
                ["Name", detail.template.name],
                ["Id · version", format!("{} · {}", detail.template.id, detail.template.version)],
                ["Source", source],
                ["Summary", detail.template.summary],
                ["Tabs", list(&detail.sections.primary)],
                ["Under More", list(&detail.sections.more)],
                ["Opens on", detail.sections.landing],
                ["Renamed", list(&renamed)],
                ["Overview cards", list(&detail.overview.cards)],
                ["Dashboard headline", detail.overview.headline.clone().unwrap_or_else(|| "none".to_owned())],
                [
                    "Evidence ledger",
                    format!("{} view; first: {}", detail.evidence.default_view, list(&detail.evidence.types_first))
                ],
                [
                    "Assistant",
                    format!(
                        "first: {}; {} suggestion(s); {} phrase(s); {}",
                        list(&detail.assistant.order),
                        detail.assistant.suggestions.len(),
                        detail.assistant.phrases.len(),
                        detail.assistant.focus.as_deref().map_or("no focus line".to_owned(), |focus| format!("focus: {focus}"))
                    )
                ],
                ["Retrieval chips", list(&detail.retrieval.type_chips)],
                [
                    "Search",
                    format!(
                        "verified x{}; {boosts} boost(s){}",
                        detail.ranking.verified,
                        detail.ranking.recency.as_ref().map_or(String::new(), |recency| format!(
                            "; recent evidence up to x{} (half-life {} days)",
                            recency.max_factor, recency.half_life_days
                        ))
                    )
                ],
                [
                    "Health checks",
                    format!(
                        "{}{}",
                        list(&detail.health.detectors),
                        detail.health.stale_after_days.map_or(String::new(), |days| format!(" (stale after {days} days)"))
                    )
                ],
                [
                    "Review queue",
                    if detail.review.order.is_empty() {
                        "mixed, oldest first".to_owned()
                    } else {
                        list(&detail.review.order)
                    }
                ],
                [
                    "Map first",
                    format!(
                        "files: {}; links: {}",
                        list(&detail.map.coverage_first),
                        list(&detail.map.relations_first)
                    )
                ],
                [
                    "Repository links",
                    format!(
                        "{} include, {} exclude pattern(s)",
                        detail.repositories.include.len(),
                        detail.repositories.exclude.len()
                    )
                ],
            ]);
            Ok(answer(
                spec,
                format!("{} ({}).", detail.template.name, detail.template.id),
                View::Facts,
                facts,
            ))
        }

        "templates validate" => Err(ApiError::bad_request(
            "templates validate checks a file on your machine, so it runs in repomemo-console (it needs no sign-in): repomemo-console templates validate <file>.",
        )),

        "status" => {
            let overview = to_json(super::overview(who, state_).await?.0)?;
            let (instance, statistics, storage) =
                (&overview["instance"], &overview["statistics"], &overview["storage"]);
            let (background, traffic) = (&overview["background"], &overview["traffic"]);
            let maintenance = if background["maintenance"]["running"] == true {
                "running".to_owned()
            } else {
                shown(&background["maintenance"]["next_run_at"])
            };
            let facts = json!([
                ["Version", instance["version"]],
                [
                    "Host",
                    format!(
                        "{} ({}, {})",
                        shown(&instance["host_name"]),
                        shown(&instance["operating_system"]),
                        shown(&instance["architecture"])
                    )
                ],
                ["Address", instance["bind_address"]],
                ["Data folder", instance["data_dir"]],
                ["Uptime", duration(instance["uptime_seconds"].as_u64().unwrap_or(0))],
                [
                    "People · active sessions",
                    format!("{} · {}", statistics["user_count"], statistics["active_session_count"])
                ],
                [
                    "Organizations · workspaces",
                    format!(
                        "{} · {}",
                        statistics["organization_count"], statistics["workspace_count"]
                    )
                ],
                [
                    "Database (+ write-ahead log)",
                    format!(
                        "{} (+ {})",
                        bytes(&storage["database_bytes"]),
                        bytes(&storage["write_ahead_log_bytes"])
                    )
                ],
                ["Stored files", bytes(&storage["blob_bytes"])],
                ["Indexing queued", background["indexing_queued"]],
                [
                    "Embedding workspaces waiting",
                    background["embedding_workspaces_waiting"]
                ],
                ["Repository syncs running", background["repository_syncs_running"]],
                ["Open event streams", background["open_event_streams"]],
                ["Next maintenance", maintenance],
                [
                    "Requests since start (last hour)",
                    format!("{} ({})", traffic["requests"], traffic["requests_last_hour"])
                ],
                [
                    "Server errors · rate limited",
                    format!("{} · {}", traffic["server_errors"], traffic["rate_limited"])
                ],
            ]);
            Ok(answer(
                spec,
                format!(
                    "RepoMemo {} up for {}.",
                    shown(&instance["version"]),
                    duration(instance["uptime_seconds"].as_u64().unwrap_or(0))
                ),
                View::Facts,
                facts,
            ))
        }

        "jobs list" => {
            let status = line.flag("status").map(|status| status.to_lowercase());
            let jobs = super::jobs(
                who,
                state_,
                Query(JobsQuery {
                    status: status.clone(),
                    limit: Some(line.limit(50)?),
                    cursor: None,
                }),
            )
            .await?
            .1.0;
            let summary = format!(
                "{} {}job(s), newest first.",
                jobs.len(),
                status.map(|status| format!("{status} ")).unwrap_or_default()
            );
            Ok(answer(
                spec,
                summary,
                View::Table {
                    columns: &[
                        "id",
                        "kind",
                        "workspace_name",
                        "status",
                        "stage",
                        "progress_current",
                        "progress_total",
                        "updated_at",
                        "error_message",
                    ],
                },
                to_json(jobs)?,
            ))
        }

        "jobs cancel" => {
            let job = state
                .storage
                .get_job(&arguments[0])
                .await
                .map_err(map_storage_error)?
                .ok_or_else(|| ApiError::not_found("The job was not found."))?;
            if !matches!(job.status.as_str(), "pending" | "running") {
                return Err(ApiError::conflict(format!(
                    "That job is {}; only pending and running jobs can be stopped.",
                    job.status
                )));
            }
            if job.cancel_requested {
                return Err(ApiError::conflict("That job is already stopping."));
            }
            crate::require_workspace_write(state, subject, &job.workspace_id).await?;
            let what = format!(
                "the {} job {} ({}, {})",
                job.kind.replace('_', " "),
                job.id,
                job.status,
                job.stage.replace('_', " ")
            );
            if !line.yes {
                return Ok(dry_run(spec, format!("Would stop {what}.")));
            }
            let job = crate::cancel_job(who, state_, Path(job.id)).await?.0;
            Ok(applied(
                spec,
                format!("Asked {what} to stop; it stops at its next step."),
                View::Text,
                to_json(job)?,
            ))
        }

        "settings list" => {
            let settings = to_json(super::get_settings(who, state_).await?.0)?;
            let rows = settings["settings"].as_array().cloned().unwrap_or_default();
            let changed = rows.iter().filter(|row| row["overridden"] == true).count();
            Ok(answer(
                spec,
                format!("{} settings, {changed} changed from the server default.", rows.len()),
                View::Table {
                    columns: &["key", "value", "default_value", "unit", "overridden", "group"],
                },
                Value::Array(rows),
            ))
        }

        "settings set" => {
            let (key, typed) = (arguments[0].as_str(), arguments[1].as_str());
            let setting = spec_of(key)?;
            let value = match setting.kind {
                SettingKind::Boolean => match typed.to_lowercase().as_str() {
                    "true" | "on" | "yes" => json!(true),
                    "false" | "off" | "no" => json!(false),
                    _ => return Err(ApiError::bad_request(format!("{key} is on or off."))),
                },
                SettingKind::Integer => typed.parse::<i64>().map(|number| json!(number)).map_err(|_| {
                    ApiError::bad_request(format!(
                        "{key} is a whole number from {} to {}.",
                        setting.min, setting.max
                    ))
                })?,
                SettingKind::Choice => json!(typed.to_lowercase()),
            };
            let current = read_setting(&state.settings(), key);
            apply_setting(&mut (*state.settings()).clone(), key, &value).map_err(ApiError::bad_request)?;
            let unit = if setting.unit.is_empty() {
                String::new()
            } else {
                format!(" {}", setting.unit)
            };
            if current == value {
                return Ok(answer(
                    spec,
                    format!("{} is already {}{unit}.", setting.label, shown(&value)),
                    View::Text,
                    Value::Null,
                ));
            }
            if !line.yes {
                return Ok(dry_run(
                    spec,
                    format!(
                        "Would change {} from {}{unit} to {}{unit}.",
                        setting.label,
                        shown(&current),
                        shown(&value)
                    ),
                ));
            }
            let values = BTreeMap::from([(key.to_owned(), value.clone())]);
            let _ = super::update_settings(as_session(who)?, state_, Json(UpdateSettingsRequest { values })).await?;
            Ok(applied(
                spec,
                format!("{} is now {}{unit}.", setting.label, shown(&value)),
                View::Text,
                Value::Null,
            ))
        }

        "settings reset" => {
            let key = arguments[0].as_str();
            let setting = spec_of(key)?;
            let (current, default) = (
                read_setting(&state.settings(), key),
                read_setting(&state.runtime.defaults, key),
            );
            if !line.yes {
                return Ok(dry_run(
                    spec,
                    format!(
                        "Would restore the default of {}: {} → {}.",
                        setting.label,
                        shown(&current),
                        shown(&default)
                    ),
                ));
            }
            let _ = super::reset_setting(as_session(who)?, state_, Path(key.to_owned())).await?;
            Ok(applied(
                spec,
                format!("{} is back to its default, {}.", setting.label, shown(&default)),
                View::Text,
                Value::Null,
            ))
        }

        "logs" => {
            let level = line.flag("level").map(|level| level.to_lowercase());
            if let Some(level) = level.as_deref().filter(|level| !LOG_LEVELS.contains(level)) {
                return Err(usage_error(
                    spec,
                    format!("{level} is not a level; use one of {}.", LOG_LEVELS.join(", ")),
                ));
            }
            let category = line.flag("category").map(|category| category.to_lowercase());
            if let Some(category) = category
                .as_deref()
                .filter(|key| !CATEGORIES.iter().any(|known| known.key == *key))
            {
                let known = CATEGORIES.iter().map(|known| known.key).collect::<Vec<_>>().join(", ");
                return Err(usage_error(
                    spec,
                    format!("{category} is not a kind of log; use one of {known}."),
                ));
            }
            let query = LogsQuery {
                level,
                audit: false,
                category,
                workspace: line.flag("workspace"),
                q: line.flag("grep"),
                after: match line.flag("after") {
                    Some(value) => Some(value.parse::<u64>().map_err(|_| {
                        usage_error(
                            spec,
                            "--after is the sequence number of the last event you have.".to_owned(),
                        )
                    })?),
                    None => None,
                },
                limit: Some(line.limit(50)? as usize),
                day: line.flag("day"),
            };
            let logs = to_json(super::logs(who, state_, Query(query)).await?.0)?;
            let records = logs["records"].as_array().cloned().unwrap_or_default();
            let summary = if logs["capturing"] == false {
                "This server is not capturing logs in memory; read them from its console or log collector.".to_owned()
            } else {
                let source = if logs["source"] == "file" {
                    "that day's file"
                } else {
                    "memory"
                };
                format!("{} event(s) from {source}, newest first.", records.len())
            };
            Ok(answer(
                spec,
                summary,
                View::Table {
                    columns: &[
                        "sequence",
                        "timestamp",
                        "level",
                        "category",
                        "target",
                        "message",
                        "fields",
                    ],
                },
                Value::Array(records),
            ))
        }

        "users list" => {
            let users = super::list_users(who, state_).await?.0;
            let summary = format!("{} account(s).", users.len());
            Ok(answer(
                spec,
                summary,
                View::Table {
                    columns: &[
                        "email",
                        "display_name",
                        "is_system_admin",
                        "is_app_admin",
                        "active_sessions",
                        "deactivated_at",
                        "last_connected_at",
                        "id",
                    ],
                },
                to_json(users)?,
            ))
        }

        "users reset-link" => {
            let user = find_account(state, &arguments[0]).await?;
            let name = user.email.clone().unwrap_or_else(|| user.display_name.clone());
            if !line.yes {
                return Ok(dry_run(
                    spec,
                    format!("Would create a one-time link, valid for 24 hours, for {name} to choose a new password."),
                ));
            }
            let issued =
                crate::access::issue_password_reset(as_session(subject.clone())?, state_, Path(user.id)).await?;
            let issued = to_json(issued.1 .0)?;
            Ok(applied(
                spec,
                format!(
                    "Reset link for {name}, valid until {}: {}  (add your web address in front of it, and pass it on yourself).",
                    issued["expires_at"].as_str().unwrap_or("-"),
                    issued["path"].as_str().unwrap_or("-")
                ),
                View::Text,
                issued,
            ))
        }

        "tokens list" => {
            let owner = match line.flag("user") {
                Some(email) => {
                    super::require_full_system_admin(subject)?;
                    find_account(state, &email).await?.id
                }
                None => subject.user_id.clone(),
            };
            let tokens = state
                .storage
                .list_personal_tokens(&owner)
                .await
                .map_err(map_storage_error)?;
            let summary = format!("{} personal access token(s).", tokens.len());
            Ok(answer(
                spec,
                summary,
                View::Table {
                    columns: &["name", "scope", "prefix", "created_at", "expires_at", "last_used_at"],
                },
                to_json(tokens)?,
            ))
        }

        "users unlock" => {
            let user = find_account(state, &arguments[0]).await?;
            let name = user.email.clone().unwrap_or_else(|| user.display_name.clone());
            if !line.yes {
                return Ok(dry_run(spec, format!("Would lift every sign-in lockout of {name}.")));
            }
            let cleared = super::unlock_user(who, state_, Path(user.id)).await?.0;
            Ok(applied(
                spec,
                format!("Lifted {} lockout(s) of {name}.", cleared["cleared"]),
                View::Text,
                cleared,
            ))
        }

        "users signout" => {
            let user = find_account(state, &arguments[0]).await?;
            let name = user.email.clone().unwrap_or_else(|| user.display_name.clone());
            let yourself = if user.id == subject.user_id {
                " This includes you, on this device too."
            } else {
                ""
            };
            if !line.yes {
                return Ok(dry_run(
                    spec,
                    format!("Would sign {name} out of every device.{yourself}"),
                ));
            }
            super::end_user_sessions(who, state_, Path(user.id)).await?;
            Ok(applied(
                spec,
                format!("Signed {name} out of every device."),
                View::Text,
                Value::Null,
            ))
        }

        "users deactivate" => {
            let user = find_account(state, &arguments[0]).await?;
            let name = user.email.clone().unwrap_or_else(|| user.display_name.clone());
            if !line.yes {
                return Ok(dry_run(
                    spec,
                    format!(
                        "Would deactivate {name}: they could no longer sign in or use a token, their sessions and tokens would end and the invitations they sent would be withdrawn. What they wrote stays."
                    ),
                ));
            }
            let ended = super::deactivate_user(who, state_, Path(user.id)).await?.0;
            Ok(applied(
                spec,
                format!(
                    "Deactivated {name}: revoked {} personal access token(s) and withdrew {} invitation(s).",
                    ended["personal_tokens_revoked"], ended["invitations_withdrawn"]
                ),
                View::Text,
                ended,
            ))
        }

        "users reactivate" => {
            let user = find_account(state, &arguments[0]).await?;
            let name = user.email.clone().unwrap_or_else(|| user.display_name.clone());
            if !line.yes {
                return Ok(dry_run(
                    spec,
                    format!("Would let {name} sign in again with their password."),
                ));
            }
            super::reactivate_user(who, state_, Path(user.id)).await?;
            Ok(applied(
                spec,
                format!("Reactivated {name}. They sign in again with their password."),
                View::Text,
                Value::Null,
            ))
        }

        "users anonymize" => {
            let user = find_account(state, &arguments[0]).await?;
            let name = user.email.clone().unwrap_or_else(|| user.display_name.clone());
            if !line.yes {
                return Ok(dry_run(
                    spec,
                    format!(
                        "Would remove the address, name, password, picture, memberships, notices and conversations of {name} for good. What they wrote would stay, credited to \"Former member\". This cannot be undone."
                    ),
                ));
            }
            let removed = super::anonymize_user(
                who,
                state_,
                Path(user.id),
                Json(super::AnonymizeRequest { confirm: name.clone() }),
            )
            .await?
            .0;
            Ok(applied(
                spec,
                format!(
                    "Anonymized {name}: removed {} membership(s), {} notice(s) and {} conversation(s).",
                    removed["memberships_removed"], removed["notices_removed"], removed["conversations_removed"]
                ),
                View::Text,
                removed,
            ))
        }

        "backup list" => {
            let overview = crate::backup::overview_with_keys(state).await;
            let status = &overview.status;
            let summary = match (status.running, status.message.as_deref()) {
                (true, _) => format!("A {} is running.", status.operation.unwrap_or("backup")),
                (false, Some(message)) => message.to_owned(),
                (false, None) => format!("{} backup(s) kept in {}.", overview.backups.len(), overview.directory),
            };
            Ok(answer(
                spec,
                summary,
                View::Table {
                    columns: &[
                        "name",
                        "created_at",
                        "version",
                        "trigger",
                        "database_bytes",
                        "blob_count",
                        "blob_bytes",
                        "includes_key",
                        "sealed_with",
                    ],
                },
                to_json(overview.backups)?,
            ))
        }

        "backup create" => {
            if !line.yes {
                return Ok(dry_run(
                    spec,
                    "Would back up the database, the stored files and the key file into the backups folder of the data folder.",
                ));
            }
            crate::system::require_full_system_admin(subject)?;
            crate::backup::start_backup(state, "manual")?;
            Ok(applied(
                spec,
                "The backup started. Check it with backup list.",
                View::Text,
                Value::Null,
            ))
        }

        "backup restore" => {
            let (name, environment) = (&arguments[0], arguments[1].trim());
            if !line.yes {
                return Ok(dry_run(
                    spec,
                    format!("Would restore {name} into a new environment folder named {environment}."),
                ));
            }
            crate::system::require_full_system_admin(subject)?;
            crate::backup::start_restore(state, name, environment)?;
            Ok(applied(
                spec,
                format!("Restoring {name} into {environment}. Check it with backup list; attach it from the environment menu when it is done."),
                View::Text,
                Value::Null,
            ))
        }

        "backup reseal" => {
            let name = &arguments[0];
            if !line.yes {
                return Ok(dry_run(
                    spec,
                    format!("Would seal the backup {name} again with the current REPOMEMO_BACKUP_KEY."),
                ));
            }
            crate::system::require_full_system_admin(subject)?;
            crate::backup::start_reseal(state, name)?;
            Ok(applied(
                spec,
                format!("Sealing {name} again with the current key. Check it with backup list."),
                View::Text,
                Value::Null,
            ))
        }

        "backup delete" => {
            let name = &arguments[0];
            if !line.yes {
                return Ok(dry_run(spec, format!("Would delete the backup {name}.")));
            }
            crate::system::require_full_system_admin(subject)?;
            crate::backup::delete_backup(state, name).await?;
            Ok(applied(
                spec,
                format!("Deleted the backup {name}."),
                View::Text,
                Value::Null,
            ))
        }

        "maintenance status" => {
            let status = super::get_maintenance(who, state_).await?.0;
            let summary = if status.running {
                "Maintenance is running."
            } else {
                "Maintenance is idle."
            };
            Ok(answer(spec, summary, View::Facts, maintenance_facts(&status)))
        }

        "maintenance run" => {
            if !line.yes {
                return Ok(dry_run(spec, "Would run a full maintenance pass now: expired sessions, old jobs, unreferenced files, failed indexing retried, database checkpoint."));
            }
            let status = super::run_maintenance(who, state_).await?.0;
            let failures = status.last_report.as_ref().map_or(0, |report| report.failures);
            let summary = if failures > 0 {
                format!("Maintenance finished with {failures} failed step(s); see logs --category jobs.")
            } else {
                "Maintenance finished.".to_owned()
            };
            Ok(applied(spec, summary, View::Facts, maintenance_facts(&status)))
        }

        "mail status" => {
            let overview = crate::mail::overview(state, 10).await?;
            let settings = &overview.settings;
            let summary = if !settings.configured {
                "Mail is not set up.".to_owned()
            } else if settings.enabled {
                format!("Mail is on, through {}:{}.", settings.host, settings.port)
            } else {
                "Mail is saved but switched off.".to_owned()
            };
            Ok(answer(
                spec,
                summary,
                View::Facts,
                json!([
                    [
                        "Server",
                        format!("{}:{} ({})", settings.host, settings.port, settings.security)
                    ],
                    ["Sender", settings.from_address],
                    [
                        "Signs in as",
                        if settings.username.is_empty() {
                            "no sign-in".to_owned()
                        } else {
                            settings.username.clone()
                        }
                    ],
                    ["Password saved", settings.password_set],
                    ["Waiting in the outbox", overview.queued_count],
                    ["Sent in the last day", overview.sent_last_day],
                    ["Failed in the last day", overview.failed_last_day],
                ]),
            ))
        }

        "mail test" => {
            super::require_full_system_admin(subject)?;
            let address = arguments[0].trim().to_owned();
            if !repomemo_mail::valid_address(&address) {
                return Err(ApiError::bad_request("That is not a valid email address."));
            }
            if !line.yes {
                return Ok(dry_run(spec, format!("Would queue a test message to {address}.")));
            }
            let entry = crate::mail::send_test(state, subject, &address).await?;
            Ok(applied(
                spec,
                format!("Queued a test message to {address}. See its result with mail status or in System › Settings › Mail."),
                View::Text,
                to_json(entry)?,
            ))
        }

        "workspace list" => {
            super::require_full_system_admin(subject)?;
            let workspaces = state.core.list_workspaces().await.map_err(map_storage_error)?;
            let mut rows = Vec::with_capacity(workspaces.len());
            for workspace in &workspaces {
                let overview = state
                    .core
                    .workspace_overview(workspace.id.clone())
                    .await
                    .map_err(map_storage_error)?;
                rows.push(json!({
                    "id": workspace.id,
                    "name": workspace.name,
                    "files": overview.artifact_count,
                    "memory_cards": overview.memory_card_count,
                    "created_at": workspace.created_at,
                }));
            }
            Ok(answer(
                spec,
                format!("{} workspace(s).", rows.len()),
                View::Table {
                    columns: &["id", "name", "files", "memory_cards", "created_at"],
                },
                Value::Array(rows),
            ))
        }

        "workspace reindex" => {
            super::require_full_system_admin(subject)?;
            let workspace = find_workspace(state, &arguments[0]).await?;
            let stale = state
                .core
                .stale_artifact_count(&workspace.id)
                .await
                .map_err(map_storage_error)?;
            if !line.yes {
                return Ok(dry_run(
                    spec,
                    format!(
                        "Would re-index {stale} file(s) of {} that the current indexer has not indexed, as a background job.",
                        workspace.name
                    ),
                ));
            }
            let (_, job) = crate::index_workspace(who, state_, Path(workspace.id.clone())).await?;
            Ok(applied(
                spec,
                format!(
                    "Re-indexing {stale} file(s) of {} as job {}. Follow it with jobs list.",
                    workspace.name, job.0.id
                ),
                View::Text,
                to_json(job.0)?,
            ))
        }

        "repo list" => {
            super::require_full_system_admin(subject)?;
            let workspaces = match line.flag("workspace") {
                Some(which) => vec![find_workspace(state, &which).await?],
                None => state.core.list_workspaces().await.map_err(map_storage_error)?,
            };
            let mut rows = Vec::new();
            for workspace in &workspaces {
                for source in state
                    .core
                    .list_repo_sources(&workspace.id)
                    .await
                    .map_err(map_storage_error)?
                {
                    rows.push(json!({
                        "id": source.id,
                        "name": source.name,
                        "workspace": workspace.name,
                        "status": source.status,
                        "last_synced_at": source.last_synced_at,
                        "files": source.file_count,
                        "last_error": source.last_error,
                    }));
                }
            }
            Ok(answer(
                spec,
                format!("{} repositor(ies).", rows.len()),
                View::Table {
                    columns: &[
                        "id",
                        "name",
                        "workspace",
                        "status",
                        "last_synced_at",
                        "files",
                        "last_error",
                    ],
                },
                Value::Array(rows),
            ))
        }

        "repo sync" => {
            super::require_full_system_admin(subject)?;
            let (repository, workspace_name) = find_repository(state, &arguments[0]).await?;
            if repository.active_job.is_some() {
                return Err(ApiError::conflict(format!(
                    "{} is already syncing; follow it with jobs list.",
                    repository.name
                )));
            }
            if !line.yes {
                return Ok(dry_run(
                    spec,
                    format!(
                        "Would sync the repository {} of {workspace_name} ({}, {} files, last synced {}).",
                        repository.name,
                        repository.status,
                        repository.file_count,
                        repository.last_synced_at.as_deref().unwrap_or("never")
                    ),
                ));
            }
            let job = state
                .repo_sync
                .start(&repository, Some(subject.user_id.clone()))
                .await
                .map_err(map_storage_error)?
                .ok_or_else(|| ApiError::conflict(format!("{} is already syncing.", repository.name)))?;
            Ok(applied(
                spec,
                format!(
                    "Syncing {} of {workspace_name} as job {}. Follow it with jobs list.",
                    repository.name, job.id
                ),
                View::Text,
                to_json(job)?,
            ))
        }

        "embeddings rebuild" => {
            super::require_full_system_admin(subject)?;
            let workspace = find_workspace(state, &arguments[0]).await?;
            let plan = state
                .core
                .embedding_rebuild_plan(&workspace.id)
                .await
                .map_err(map_storage_error)?;
            let Some(model) = plan.model.as_deref() else {
                return Err(ApiError::conflict(format!(
                    "{} has no enabled embedding provider, so there is nothing to rebuild the vectors with. Set one up in the workspace settings first.",
                    workspace.name
                )));
            };
            let allowance = if plan.daily_limit == 0 {
                String::new()
            } else {
                let days = (plan.provider_calls + i64::from(plan.daily_limit) - 1) / i64::from(plan.daily_limit);
                if days > 1 {
                    format!(
                        " That is more than the workspace's daily allowance of {} background AI calls: it finishes over {days} days.",
                        plan.daily_limit
                    )
                } else {
                    String::new()
                }
            };
            if !line.yes {
                return Ok(dry_run(
                    spec,
                    format!(
                        "Would remove {} stored vector(s) of {} and embed its {} passage(s) again with {model}, in about {} provider call(s). Search by meaning is partial until it finishes; keyword search is not affected.{allowance}",
                        plan.stored_vectors, workspace.name, plan.passages, plan.provider_calls
                    ),
                ));
            }
            let running = state
                .storage
                .list_workspace_jobs(&workspace.id, Some("embedding"), Some("running"), 5)
                .await
                .map_err(map_storage_error)?;
            if !running.is_empty() {
                return Err(ApiError::conflict(format!(
                    "An embedding pass of {} is running (job {}). Let it finish, or stop it with jobs cancel, then rebuild.",
                    workspace.name, running[0].id
                )));
            }
            let removed = state
                .core
                .clear_embeddings(&workspace.id)
                .await
                .map_err(map_storage_error)?;
            state.embedding_queue.request(&workspace.id);
            Ok(applied(
                spec,
                format!(
                    "Removed {removed} vector(s) of {}; its {} passage(s) are being embedded again with {model}. Follow it with jobs list.{allowance}",
                    workspace.name, plan.passages
                ),
                View::Text,
                json!({ "workspace_id": workspace.id, "removed": removed, "passages": plan.passages, "model": model }),
            ))
        }

        other => Err(ApiError::internal(format!(
            "The console command {other} has no implementation"
        ))),
    }
}

/// A workspace by id, or by name when exactly one has it.
async fn find_workspace(state: &AppState, who: &str) -> Result<repomemo_domain::Workspace, ApiError> {
    let all = state.core.list_workspaces().await.map_err(map_storage_error)?;
    if let Some(found) = all.iter().find(|workspace| workspace.id == who) {
        return Ok(found.clone());
    }
    let named = all
        .iter()
        .filter(|workspace| workspace.name.eq_ignore_ascii_case(who))
        .collect::<Vec<_>>();
    match named.as_slice() {
        [only] => Ok((*only).clone()),
        [] => Err(ApiError::not_found(format!(
            "No workspace has the id or name {who}. Type workspace list to see them."
        ))),
        several => Err(ApiError::conflict(format!(
            "{} workspaces are named {who}; use the id: {}.",
            several.len(),
            several
                .iter()
                .map(|workspace| workspace.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// A linked repository by id, or by name when exactly one has it, with the
/// name of its workspace.
async fn find_repository(state: &AppState, who: &str) -> Result<(repomemo_domain::RepoSource, String), ApiError> {
    let mut found = Vec::new();
    for workspace in state.core.list_workspaces().await.map_err(map_storage_error)? {
        for source in state
            .core
            .list_repo_sources(&workspace.id)
            .await
            .map_err(map_storage_error)?
        {
            if source.id == who || source.name.eq_ignore_ascii_case(who) {
                found.push((source, workspace.name.clone()));
            }
        }
    }
    if let Some(index) = found.iter().position(|(source, _)| source.id == who) {
        return Ok(found.swap_remove(index));
    }
    match found.len() {
        0 => Err(ApiError::not_found(format!(
            "No repository has the id or name {who}. Type repo list to see them."
        ))),
        1 => Ok(found.remove(0)),
        count => Err(ApiError::conflict(format!(
            "{count} repositories are named {who}; use the id: {}.",
            found
                .iter()
                .map(|(source, workspace)| format!("{} (in {workspace})", source.id))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn spec_of(key: &str) -> Result<&'static super::SettingSpec, ApiError> {
    spec(key).ok_or_else(|| ApiError::not_found(format!("{key} is not a setting. Type settings list to see them.")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        parse(line).unwrap().words
    }

    #[test]
    fn lines_are_split_into_words_options_and_confirmation() {
        let line = parse(r#"logs --level warn --grep "sign in failed" --category=security -y"#).unwrap();
        assert_eq!(line.words, ["logs"]);
        assert_eq!(line.flag("level").as_deref(), Some("warn"));
        assert_eq!(line.flag("grep").as_deref(), Some("sign in failed"));
        assert_eq!(line.flag("category").as_deref(), Some("security"));
        assert!(line.yes);
        assert_eq!(words("settings set 'a b'  c"), ["settings", "set", "a b", "c"]);
        assert_eq!(words(r#"say "it's""#), ["say", "it's"]);
        assert_eq!(words(r#"empty """#), ["empty", ""]);
        assert!(parse(r#"logs --grep "open"#).is_err(), "an unclosed quote");
        assert!(parse("logs --level").is_err(), "an option without its value");
    }

    #[test]
    fn commands_are_found_by_one_or_two_words() {
        let name = |line: &str| resolve(&words(line)).map(|(spec, arguments)| (spec.name, arguments.len()));
        assert_eq!(name("status"), Some(("status", 0)));
        assert_eq!(
            name("Jobs"),
            Some(("jobs list", 0)),
            "a group alone runs its first command"
        );
        assert_eq!(name("jobs cancel abc"), Some(("jobs cancel", 1)));
        assert_eq!(name("settings SET log_level debug"), Some(("settings set", 2)));
        assert_eq!(name("help jobs"), Some(("help", 1)));
        assert_eq!(name("jobs explode"), None);
        assert_eq!(name("rm -rf"), None);
    }

    #[test]
    fn arguments_and_options_are_checked_against_the_usage() {
        let checked = |text: &str| {
            let line = parse(text).unwrap();
            let (spec, arguments) = resolve(&line.words).unwrap();
            check(spec, arguments, &line).map_err(|error| error.message)
        };
        assert!(checked("jobs list --status failed --limit 5").is_ok());
        assert!(checked("jobs list --stauts failed")
            .unwrap_err()
            .contains("--stauts is not an option"));
        assert!(checked("jobs cancel")
            .unwrap_err()
            .contains("Usage: jobs cancel <job-id>"));
        assert!(checked("status now").is_err());
        assert!(checked("maintenance run --yes").is_ok(), "--yes is accepted everywhere");
        assert!(checked("help settings set").is_ok(), "help takes any words");
    }

    #[test]
    fn every_command_has_an_implementation_and_changes_ask_for_yes() {
        for command in COMMANDS {
            assert!(command.usage.starts_with(command.name), "{}", command.name);
            assert_eq!(command.changes, command.usage.ends_with("--yes"), "{}", command.name);
        }
        for (group, name) in GROUP_DEFAULTS {
            assert!(COMMANDS
                .iter()
                .any(|command| command.name == *name && name.starts_with(group)));
        }
    }

    #[test]
    fn sizes_and_durations_read_well() {
        assert_eq!(bytes(&json!(512)), "512 B");
        assert_eq!(bytes(&json!(1536)), "1.5 KiB");
        assert_eq!(duration(59), "0 min");
        assert_eq!(duration(3_660), "1 h 1 min");
        assert_eq!(duration(90_000), "1 d 1 h");
    }
}
