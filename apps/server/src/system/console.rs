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
use crate::{logs::CATEGORIES, map_storage_error, ApiError, AppState, AuthenticatedSubject};

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
    CommandSpec { name: "logs", usage: "logs [--level warn] [--category security|activity|jobs|http|ai|database|server] [--grep text] [--workspace id] [--day YYYY-MM-DD] [--limit n]", summary: "Log events, newest first: recent ones, or a day's file.", changes: false, arguments: 0, flags: &["level", "category", "grep", "workspace", "day", "limit"] },
    CommandSpec { name: "users list", usage: "users list", summary: "Every account with its roles and sessions.", changes: false, arguments: 0, flags: &[] },
    CommandSpec { name: "users unlock", usage: "users unlock <email> --yes", summary: "Lifts the sign-in lockouts of an account.", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "users signout", usage: "users signout <email> --yes", summary: "Signs an account out of every device.", changes: true, arguments: 1, flags: &[] },
    CommandSpec { name: "maintenance status", usage: "maintenance status", summary: "The last maintenance run and the next one.", changes: false, arguments: 0, flags: &[] },
    CommandSpec { name: "maintenance run", usage: "maintenance run --yes", summary: "Runs a full maintenance pass now.", changes: true, arguments: 0, flags: &[] },
];

/// A group name alone runs its first command.
const GROUP_DEFAULTS: &[(&str, &str)] = &[
    ("jobs", "jobs list"),
    ("settings", "settings list"),
    ("users", "users list"),
    ("maintenance", "maintenance status"),
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
    COMMANDS.iter().find(|spec| spec.name == name).map(|spec| (spec, &words[1..]))
}

fn usage_error(spec: &CommandSpec, problem: impl std::fmt::Display) -> ApiError {
    ApiError::bad_request(format!("{problem} Usage: {}", spec.usage))
}

fn check(spec: &CommandSpec, arguments: &[String], line: &Line) -> Result<(), ApiError> {
    if spec.name != "help" && arguments.len() != spec.arguments {
        return Err(usage_error(spec, format!("{} takes {} argument(s).", spec.name, spec.arguments)));
    }
    if let Some(unknown) = line.flags.keys().find(|flag| !spec.flags.contains(&flag.as_str())) {
        return Err(usage_error(spec, format!("--{unknown} is not an option of {}.", spec.name)));
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
    ConsoleResponse { command: spec.name, summary: summary.into(), dry_run: false, view, data, applied: false }
}

fn applied(spec: &'static CommandSpec, summary: impl Into<String>, view: View, data: Value) -> ConsoleResponse {
    ConsoleResponse { applied: true, ..answer(spec, summary, view, data) }
}

fn dry_run(spec: &'static CommandSpec, summary: impl std::fmt::Display) -> ConsoleResponse {
    ConsoleResponse { dry_run: true, ..answer(spec, format!("{summary} Nothing changed: add --yes to run it."), View::Text, Value::Null) }
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
    if unit == 0 { format!("{bytes} B") } else { format!("{size:.1} {}", units[unit]) }
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
        ["Last run", status.last_finished_at.as_deref().map_or("not since the server started".to_owned(), |at| format!("{at} ({})", status.last_trigger.unwrap_or("scheduled")))],
        ["Next run", status.next_run_at.as_deref().unwrap_or("scheduled runs off")],
        ["Stored files removed", number(|report| format!("{} ({})", report.blobs_removed, bytes(&json!(report.blob_bytes_reclaimed))))],
        ["Cached previews removed", number(|report| report.previews_removed.to_string())],
        ["Old jobs removed", number(|report| report.jobs_pruned.to_string())],
        ["Expired sessions removed", number(|report| report.refresh_tokens_purged.to_string())],
        ["Failed steps", number(|report| report.failures.to_string())],
    ])
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
        role: if subject.is_system_admin { "system administrator" } else { "app administrator" },
        commands: COMMANDS,
    }))
}

/// Runs one command line.
pub(crate) async fn run(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<ConsoleRequest>,
) -> Result<Json<ConsoleResponse>, ApiError> {
    require_system_admin(&subject)?;
    let text = request.command.trim();
    if text.chars().count() > MAX_COMMAND_LENGTH {
        return Err(ApiError::bad_request(format!("A command is at most {MAX_COMMAND_LENGTH} characters.")));
    }
    let line = parse(text).map_err(ApiError::bad_request)?;
    if line.words.is_empty() {
        return Err(ApiError::bad_request("Type a command, or help to list them."));
    }
    let (spec, arguments) = resolve(&line.words).ok_or_else(|| {
        ApiError::bad_request(format!("{} is not a command. Type help to list them.", line.words.join(" ")))
    })?;
    check(spec, arguments, &line)?;
    tracing::debug!(user_id = %subject.user_id, command = spec.name, "Console command");
    let response = execute(&state, &subject, spec, arguments, &line).await?;
    if response.applied {
        audit(&state, &subject, "console_command", format!("Ran `{text}` from the console.")).await;
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
                .filter(|command| wanted.is_empty() || command.name == wanted || command.name.starts_with(&format!("{wanted} ")))
                .map(|command| json!({"command": command.usage, "summary": command.summary, "changes": command.changes}))
                .collect::<Vec<_>>();
            if rows.is_empty() {
                return Err(ApiError::bad_request(format!("{wanted} is not a command. Type help to list them.")));
            }
            Ok(answer(spec, "Commands that change something are dry runs until you add --yes.", View::Table { columns: &["command", "summary", "changes"] }, Value::Array(rows)))
        }

        "status" => {
            let overview = to_json(super::overview(who, state_).await?.0)?;
            let (instance, statistics, storage) = (&overview["instance"], &overview["statistics"], &overview["storage"]);
            let (background, traffic) = (&overview["background"], &overview["traffic"]);
            let maintenance = if background["maintenance"]["running"] == true {
                "running".to_owned()
            } else {
                shown(&background["maintenance"]["next_run_at"])
            };
            let facts = json!([
                ["Version", instance["version"]],
                ["Host", format!("{} ({}, {})", shown(&instance["host_name"]), shown(&instance["operating_system"]), shown(&instance["architecture"]))],
                ["Address", instance["bind_address"]],
                ["Data folder", instance["data_dir"]],
                ["Uptime", duration(instance["uptime_seconds"].as_u64().unwrap_or(0))],
                ["People · active sessions", format!("{} · {}", statistics["user_count"], statistics["active_session_count"])],
                ["Organizations · workspaces", format!("{} · {}", statistics["organization_count"], statistics["workspace_count"])],
                ["Database (+ write-ahead log)", format!("{} (+ {})", bytes(&storage["database_bytes"]), bytes(&storage["write_ahead_log_bytes"]))],
                ["Stored files", bytes(&storage["blob_bytes"])],
                ["Indexing queued", background["indexing_queued"]],
                ["Embedding workspaces waiting", background["embedding_workspaces_waiting"]],
                ["Repository syncs running", background["repository_syncs_running"]],
                ["Open event streams", background["open_event_streams"]],
                ["Next maintenance", maintenance],
                ["Requests since start (last hour)", format!("{} ({})", traffic["requests"], traffic["requests_last_hour"])],
                ["Server errors · rate limited", format!("{} · {}", traffic["server_errors"], traffic["rate_limited"])],
            ]);
            Ok(answer(spec, format!("RepoMemo {} up for {}.", shown(&instance["version"]), duration(instance["uptime_seconds"].as_u64().unwrap_or(0))), View::Facts, facts))
        }

        "jobs list" => {
            let status = line.flag("status").map(|status| status.to_lowercase());
            let jobs = super::jobs(who, state_, Query(JobsQuery { status: status.clone(), limit: Some(line.limit(50)?) })).await?.0;
            let summary = format!("{} {}job(s), newest first.", jobs.len(), status.map(|status| format!("{status} ")).unwrap_or_default());
            Ok(answer(spec, summary, View::Table { columns: &["id", "kind", "workspace_name", "status", "stage", "progress_current", "progress_total", "updated_at", "error_message"] }, to_json(jobs)?))
        }

        "jobs cancel" => {
            let job = state
                .storage
                .get_job(&arguments[0])
                .await
                .map_err(map_storage_error)?
                .ok_or_else(|| ApiError::not_found("The job was not found."))?;
            if !matches!(job.status.as_str(), "pending" | "running") {
                return Err(ApiError::conflict(format!("That job is {}; only pending and running jobs can be stopped.", job.status)));
            }
            if job.cancel_requested {
                return Err(ApiError::conflict("That job is already stopping."));
            }
            crate::require_workspace_write(state, subject, &job.workspace_id).await?;
            let what = format!("the {} job {} ({}, {})", job.kind.replace('_', " "), job.id, job.status, job.stage.replace('_', " "));
            if !line.yes {
                return Ok(dry_run(spec, format!("Would stop {what}.")));
            }
            let job = crate::cancel_job(who, state_, Path(job.id)).await?.0;
            Ok(applied(spec, format!("Asked {what} to stop; it stops at its next step."), View::Text, to_json(job)?))
        }

        "settings list" => {
            let settings = to_json(super::get_settings(who, state_).await?.0)?;
            let rows = settings["settings"].as_array().cloned().unwrap_or_default();
            let changed = rows.iter().filter(|row| row["overridden"] == true).count();
            Ok(answer(spec, format!("{} settings, {changed} changed from the server default.", rows.len()), View::Table { columns: &["key", "value", "default_value", "unit", "overridden", "group"] }, Value::Array(rows)))
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
                SettingKind::Integer => typed
                    .parse::<i64>()
                    .map(|number| json!(number))
                    .map_err(|_| ApiError::bad_request(format!("{key} is a whole number from {} to {}.", setting.min, setting.max)))?,
                SettingKind::Choice => json!(typed.to_lowercase()),
            };
            let current = read_setting(&state.settings(), key);
            apply_setting(&mut (*state.settings()).clone(), key, &value).map_err(ApiError::bad_request)?;
            let unit = if setting.unit.is_empty() { String::new() } else { format!(" {}", setting.unit) };
            if current == value {
                return Ok(answer(spec, format!("{} is already {}{unit}.", setting.label, shown(&value)), View::Text, Value::Null));
            }
            if !line.yes {
                return Ok(dry_run(spec, format!("Would change {} from {}{unit} to {}{unit}.", setting.label, shown(&current), shown(&value))));
            }
            let values = BTreeMap::from([(key.to_owned(), value.clone())]);
            let _ = super::update_settings(who, state_, Json(UpdateSettingsRequest { values })).await?;
            Ok(applied(spec, format!("{} is now {}{unit}.", setting.label, shown(&value)), View::Text, Value::Null))
        }

        "settings reset" => {
            let key = arguments[0].as_str();
            let setting = spec_of(key)?;
            let (current, default) = (read_setting(&state.settings(), key), read_setting(&state.runtime.defaults, key));
            if !line.yes {
                return Ok(dry_run(spec, format!("Would restore the default of {}: {} → {}.", setting.label, shown(&current), shown(&default))));
            }
            let _ = super::reset_setting(who, state_, Path(key.to_owned())).await?;
            Ok(applied(spec, format!("{} is back to its default, {}.", setting.label, shown(&default)), View::Text, Value::Null))
        }

        "logs" => {
            let level = line.flag("level").map(|level| level.to_lowercase());
            if let Some(level) = level.as_deref().filter(|level| !LOG_LEVELS.contains(level)) {
                return Err(usage_error(spec, format!("{level} is not a level; use one of {}.", LOG_LEVELS.join(", "))));
            }
            let category = line.flag("category").map(|category| category.to_lowercase());
            if let Some(category) = category.as_deref().filter(|key| !CATEGORIES.iter().any(|known| known.key == *key)) {
                let known = CATEGORIES.iter().map(|known| known.key).collect::<Vec<_>>().join(", ");
                return Err(usage_error(spec, format!("{category} is not a kind of log; use one of {known}.")));
            }
            let query = LogsQuery {
                level,
                audit: false,
                category,
                workspace: line.flag("workspace"),
                q: line.flag("grep"),
                after: None,
                limit: Some(line.limit(50)? as usize),
                day: line.flag("day"),
            };
            let logs = to_json(super::logs(who, state_, Query(query)).await?.0)?;
            let records = logs["records"].as_array().cloned().unwrap_or_default();
            let summary = if logs["capturing"] == false {
                "This server is not capturing logs in memory; read them from its console or log collector.".to_owned()
            } else {
                let source = if logs["source"] == "file" { "that day's file" } else { "memory" };
                format!("{} event(s) from {source}, newest first.", records.len())
            };
            Ok(answer(spec, summary, View::Table { columns: &["timestamp", "level", "category", "target", "message", "fields"] }, Value::Array(records)))
        }

        "users list" => {
            let users = super::list_users(who, state_).await?.0;
            let summary = format!("{} account(s).", users.len());
            Ok(answer(spec, summary, View::Table { columns: &["email", "display_name", "is_system_admin", "is_app_admin", "active_sessions", "last_connected_at", "id"] }, to_json(users)?))
        }

        "users unlock" => {
            let user = find_account(state, &arguments[0]).await?;
            let name = user.email.clone().unwrap_or_else(|| user.display_name.clone());
            if !line.yes {
                return Ok(dry_run(spec, format!("Would lift every sign-in lockout of {name}.")));
            }
            let cleared = super::unlock_user(who, state_, Path(user.id)).await?.0;
            Ok(applied(spec, format!("Lifted {} lockout(s) of {name}.", cleared["cleared"]), View::Text, cleared))
        }

        "users signout" => {
            let user = find_account(state, &arguments[0]).await?;
            let name = user.email.clone().unwrap_or_else(|| user.display_name.clone());
            let yourself = if user.id == subject.user_id { " This includes you, on this device too." } else { "" };
            if !line.yes {
                return Ok(dry_run(spec, format!("Would sign {name} out of every device.{yourself}")));
            }
            super::end_user_sessions(who, state_, Path(user.id)).await?;
            Ok(applied(spec, format!("Signed {name} out of every device."), View::Text, Value::Null))
        }

        "maintenance status" => {
            let status = super::get_maintenance(who, state_).await?.0;
            let summary = if status.running { "Maintenance is running." } else { "Maintenance is idle." };
            Ok(answer(spec, summary, View::Facts, maintenance_facts(&status)))
        }

        "maintenance run" => {
            if !line.yes {
                return Ok(dry_run(spec, "Would run a full maintenance pass now: expired sessions, old jobs, unreferenced files, failed indexing retried, database checkpoint."));
            }
            let status = super::run_maintenance(who, state_).await?.0;
            let failures = status.last_report.as_ref().map_or(0, |report| report.failures);
            let summary = if failures > 0 { format!("Maintenance finished with {failures} failed step(s); see logs --category jobs.") } else { "Maintenance finished.".to_owned() };
            Ok(applied(spec, summary, View::Facts, maintenance_facts(&status)))
        }

        other => Err(ApiError::internal(format!("The console command {other} has no implementation"))),
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
        assert_eq!(name("Jobs"), Some(("jobs list", 0)), "a group alone runs its first command");
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
        assert!(checked("jobs list --stauts failed").unwrap_err().contains("--stauts is not an option"));
        assert!(checked("jobs cancel").unwrap_err().contains("Usage: jobs cancel <job-id>"));
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
            assert!(COMMANDS.iter().any(|command| command.name == *name && name.starts_with(group)));
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
