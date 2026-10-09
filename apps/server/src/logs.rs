//! The server's logging: what is recorded, how it is printed, where it goes.
//!
//! [`init_logging`] (called by `main.rs`) installs one tracing stack:
//! - a **filter** built from per-category levels (security, workspace
//!   activity, background jobs, HTTP, AI, database, and everything else),
//!   which system administrators change at run time;
//! - the **console** output, as readable text, compact text or JSON lines,
//!   also switchable at run time;
//! - an **in-memory buffer** of recent events for System › Logs (security
//!   events kept apart so ordinary noise cannot push them out);
//! - optional **daily files** of JSON lines in `<data dir>/logs`, written by a
//!   background thread so no request waits on the disk, and deleted by
//!   maintenance once older than the retention period.
//!
//! Every sink sees the same filtered events. Nothing in this module emits
//! tracing events itself (that would feed back into the sinks); problems are
//! written to standard error instead.

use std::{
    collections::{BTreeMap, VecDeque},
    fmt::Write as _,
    fs::File,
    io::{BufWriter, Read, Seek, SeekFrom, Write as _},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Mutex, OnceLock,
    },
    time::Duration,
};

use anyhow::{bail, Context as _, Result};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tracing::{field::Field, Event, Level, Subscriber};
use tracing_subscriber::{
    fmt, layer::Context, layer::SubscriberExt, registry::LookupSpan, reload,
    util::SubscriberInitExt, EnvFilter, Layer, Registry,
};

const GENERAL_CAPACITY: usize = 2_000;
const AUDIT_CAPACITY: usize = 1_000;
const MAX_MESSAGE_CHARS: usize = 2_000;
/// Only the last part of a large day file is read for the Logs page.
const MAX_FILE_READ_BYTES: u64 = 16 * 1024 * 1024;
const FILE_PREFIX: &str = "repomemo-";
const FILE_SUFFIX: &str = ".jsonl";

// ---------------------------------------------------------------------------
// Categories and settings

/// A group of log sources an administrator can turn up or down together.
pub struct LogCategory {
    pub key: &'static str,
    pub label: &'static str,
    /// Tracing targets (module paths) in the category; a target also covers
    /// its submodules.
    pub targets: &'static [&'static str],
    pub default_level: &'static str,
}

/// Every category except "server", which is everything not listed here.
pub const CATEGORIES: &[LogCategory] = &[
    LogCategory { key: "security", label: "Security", targets: &["audit"], default_level: "info" },
    LogCategory { key: "activity", label: "Workspace activity", targets: &["activity"], default_level: "info" },
    LogCategory {
        key: "jobs",
        label: "Background jobs",
        targets: &[
            "jobs",
            "repomemo_server::indexing",
            "repomemo_server::embedding",
            "repomemo_server::repositories",
            "repomemo_server::maintenance",
            "repomemo_server::conversion",
            "repomemo_api::repo_sync",
            "repomemo_git",
        ],
        default_level: "info",
    },
    LogCategory { key: "http", label: "HTTP requests", targets: &["tower_http"], default_level: "warn" },
    LogCategory { key: "ai", label: "AI providers", targets: &["repomemo_ai", "reqwest"], default_level: "info" },
    LogCategory { key: "database", label: "Database", targets: &["repomemo_storage", "sqlx"], default_level: "warn" },
];

/// The category of everything else.
pub const SERVER_CATEGORY: &str = "server";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleFormat {
    /// Readable lines with timestamp, level, source and fields.
    Text,
    /// Shorter readable lines.
    Compact,
    /// One JSON object per line, for log collectors.
    Json,
}

impl ConsoleFormat {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "text" | "full" => Some(Self::Text),
            "compact" => Some(Self::Compact),
            "json" => Some(Self::Json),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Compact => "compact",
            Self::Json => "json",
        }
    }
}

/// What is logged and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogSettings {
    /// Level of the "server" category: everything not in another category.
    pub level: String,
    /// Level per category key.
    pub categories: BTreeMap<&'static str, String>,
    pub console_format: ConsoleFormat,
    /// Write daily JSON-lines files in `<data dir>/logs`.
    pub to_file: bool,
    /// Days log files are kept; 0 keeps them forever.
    pub retention_days: i64,
    /// A raw `RUST_LOG` filter from the environment. It applies until an
    /// administrator sets the levels here.
    pub env_filter: Option<String>,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            level: "info".to_owned(),
            categories: CATEGORIES
                .iter()
                .map(|category| (category.key, category.default_level.to_owned()))
                .collect(),
            console_format: ConsoleFormat::Text,
            to_file: true,
            retention_days: 14,
            env_filter: None,
        }
    }
}

impl LogSettings {
    /// Defaults from the environment: `RUST_LOG`, `REPOMEMO_LOG_FORMAT`,
    /// `REPOMEMO_LOG_TO_FILE` and `REPOMEMO_LOG_RETENTION_DAYS`.
    pub fn from_env() -> Result<Self> {
        let text = |name: &str| std::env::var(name).ok().map(|value| value.trim().to_owned()).filter(|value| !value.is_empty());
        let mut settings = Self { env_filter: text("RUST_LOG"), ..Self::default() };
        if let Some(format) = text("REPOMEMO_LOG_FORMAT") {
            settings.console_format = ConsoleFormat::parse(&format)
                .with_context(|| format!("REPOMEMO_LOG_FORMAT must be text, compact or json, not {format}"))?;
        }
        if let Some(flag) = text("REPOMEMO_LOG_TO_FILE") {
            settings.to_file = match flag.to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => true,
                "0" | "false" | "no" | "off" => false,
                _ => bail!("REPOMEMO_LOG_TO_FILE must be true or false, not {flag}"),
            };
        }
        if let Some(days) = text("REPOMEMO_LOG_RETENTION_DAYS") {
            settings.retention_days = days
                .parse::<i64>()
                .ok()
                .filter(|days| (0..=3650).contains(days))
                .with_context(|| format!("REPOMEMO_LOG_RETENTION_DAYS must be a number of days from 0 to 3650, not {days}"))?;
        }
        Ok(settings)
    }

    /// The filter in tracing's directive syntax.
    pub fn directives(&self) -> String {
        if let Some(raw) = &self.env_filter {
            return raw.clone();
        }
        let mut directives = self.level.clone();
        for category in CATEGORIES {
            let level = self
                .categories
                .get(category.key)
                .map(String::as_str)
                .unwrap_or(category.default_level);
            for target in category.targets {
                let _ = write!(directives, ",{target}={level}");
            }
        }
        directives
    }
}

/// The category key of a tracing target.
pub fn category_of(target: &str) -> &'static str {
    CATEGORIES
        .iter()
        .find(|category| {
            category.targets.iter().any(|prefix| {
                target == *prefix || target.strip_prefix(prefix).is_some_and(|rest| rest.starts_with("::"))
            })
        })
        .map_or(SERVER_CATEGORY, |category| category.key)
}

// ---------------------------------------------------------------------------
// The installed stack

type ReloadFn<T> = Box<dyn Fn(T) -> std::result::Result<(), String> + Send + Sync>;

struct LogControl {
    set_filter: ReloadFn<EnvFilter>,
    set_format: ReloadFn<ConsoleFormat>,
    applied: Mutex<Option<(String, ConsoleFormat)>>,
}

static CONTROL: OnceLock<LogControl> = OnceLock::new();
static FILE_SINK: OnceLock<FileSink> = OnceLock::new();

/// Installs the server's tracing stack with the environment's settings.
/// Call once, before anything logs. [`apply`] changes it later.
pub fn init_logging() -> Result<()> {
    let settings = LogSettings::from_env()?;
    let filter = EnvFilter::try_new(settings.directives())
        .with_context(|| format!("RUST_LOG is not a valid log filter: {}", settings.directives()))?;
    let (console, console_handle) = reload::Layer::new(console_layer::<Registry>(settings.console_format));
    let (filter, filter_handle) = reload::Layer::new(filter);
    tracing_subscriber::registry()
        .with(console)
        .with(filter)
        .with(log_capture_layer())
        .try_init()
        .context("logging was already initialised")?;
    let control = LogControl {
        set_filter: Box::new(move |filter| filter_handle.reload(filter).map_err(|error| error.to_string())),
        set_format: Box::new(move |format| {
            console_handle
                .reload(console_layer::<Registry>(format))
                .map_err(|error| error.to_string())
        }),
        applied: Mutex::new(Some((settings.directives(), settings.console_format))),
    };
    let _ = CONTROL.set(control);
    Ok(())
}

fn console_layer<S>(format: ConsoleFormat) -> Box<dyn Layer<S> + Send + Sync>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    match format {
        ConsoleFormat::Text => Box::new(fmt::layer()),
        ConsoleFormat::Compact => Box::new(fmt::layer().compact()),
        ConsoleFormat::Json => Box::new(JsonConsoleLayer),
    }
}

/// Applies settings to the installed stack: the filter, the console format
/// and file output. Does nothing when the stack was not installed (tests).
pub fn apply(settings: &LogSettings, data_dir: &Path) -> Result<()> {
    let Some(control) = CONTROL.get() else {
        return Ok(());
    };
    let directives = settings.directives();
    let wanted = (directives.clone(), settings.console_format);
    let mut applied = control.applied.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if applied.as_ref() != Some(&wanted) {
        let filter = EnvFilter::try_new(&directives).with_context(|| format!("invalid log filter {directives}"))?;
        (control.set_filter)(filter).map_err(anyhow::Error::msg)?;
        if applied.as_ref().map(|(_, format)| *format) != Some(settings.console_format) {
            (control.set_format)(settings.console_format).map_err(anyhow::Error::msg)?;
        }
        *applied = Some(wanted);
    }
    file_sink().configure(settings.to_file.then(|| log_dir(data_dir)));
    Ok(())
}

/// The filter in force, when the stack is installed.
pub fn effective_filter() -> Option<String> {
    CONTROL
        .get()
        .and_then(|control| control.applied.lock().ok().and_then(|applied| applied.as_ref().map(|(filter, _)| filter.clone())))
}

/// Whether events are being written to files now.
pub fn writing_files() -> bool {
    FILE_SINK.get().is_some_and(FileSink::is_active)
}

// ---------------------------------------------------------------------------
// Records

#[derive(Debug, Clone, Serialize)]
pub struct LogRecord {
    /// Increases with every captured event (or, for a file, every line); use
    /// it to fetch only newer ones.
    pub sequence: u64,
    pub timestamp: String,
    pub level: &'static str,
    pub target: String,
    pub category: &'static str,
    pub message: String,
    /// The event's other fields as `name=value` pairs.
    pub fields: String,
}

/// A line of a log file.
#[derive(Debug, Serialize, Deserialize)]
struct FileLine {
    timestamp: String,
    level: String,
    target: String,
    message: String,
    #[serde(default)]
    fields: String,
}

fn record_of(event: &Event<'_>, sequence: u64) -> LogRecord {
    let mut visitor = FieldVisitor::default();
    event.record(&mut visitor);
    let metadata = event.metadata();
    LogRecord {
        sequence,
        timestamp: Utc::now().to_rfc3339(),
        level: level_name(metadata.level()),
        target: metadata.target().to_owned(),
        category: category_of(metadata.target()),
        message: truncate(visitor.message),
        fields: truncate(visitor.fields),
    }
}

fn file_line(record: &LogRecord) -> String {
    serde_json::to_string(&FileLine {
        timestamp: record.timestamp.clone(),
        level: record.level.to_owned(),
        target: record.target.clone(),
        message: record.message.clone(),
        fields: record.fields.clone(),
    })
    .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// In-memory buffer (and the file sink, fed from the same layer)

pub struct LogBuffer {
    sequence: AtomicU64,
    general: Mutex<VecDeque<LogRecord>>,
    audit: Mutex<VecDeque<LogRecord>>,
}

static BUFFER: OnceLock<LogBuffer> = OnceLock::new();

/// The process-wide buffer, when log capture was installed.
pub fn buffer() -> Option<&'static LogBuffer> {
    BUFFER.get()
}

/// A tracing layer that keeps events in the in-memory buffer and hands them
/// to the file writer when file output is on.
pub fn log_capture_layer() -> LogCaptureLayer {
    BUFFER.get_or_init(|| LogBuffer {
        sequence: AtomicU64::new(0),
        general: Mutex::new(VecDeque::with_capacity(GENERAL_CAPACITY)),
        audit: Mutex::new(VecDeque::with_capacity(AUDIT_CAPACITY)),
    });
    LogCaptureLayer { _private: () }
}

pub struct LogCaptureLayer {
    _private: (),
}

impl<S: Subscriber> Layer<S> for LogCaptureLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let Some(buffer) = BUFFER.get() else { return };
        let record = record_of(event, buffer.sequence.fetch_add(1, Ordering::Relaxed) + 1);
        if let Some(sink) = FILE_SINK.get() {
            sink.write(&record);
        }
        let (queue, capacity) = if record.target == "audit" {
            (&buffer.audit, AUDIT_CAPACITY)
        } else {
            (&buffer.general, GENERAL_CAPACITY)
        };
        if let Ok(mut queue) = queue.lock() {
            if queue.len() == capacity {
                queue.pop_front();
            }
            queue.push_back(record);
        }
    }
}

/// Which events to return.
#[derive(Debug, Clone, Default)]
pub struct LogQuery {
    /// Only this level and more severe ones (`error`, `warn`, `info`, `debug`, `trace`).
    pub min_level: Option<String>,
    /// Only events of this category (see [`CATEGORIES`] and [`SERVER_CATEGORY`]).
    pub category: Option<String>,
    /// Only events about this workspace (a `workspace_id` field).
    pub workspace_id: Option<String>,
    /// Text that must appear in the target, message or fields.
    pub contains: Option<String>,
    /// Only events after this sequence number.
    pub after: Option<u64>,
    pub limit: usize,
}

impl LogQuery {
    fn matches(&self, record: &LogRecord) -> bool {
        let min_rank = self.min_level.as_deref().map(level_rank).unwrap_or(u8::MAX);
        let needle = self.contains.as_deref().map(str::to_lowercase).filter(|text| !text.is_empty());
        let workspace = self.workspace_id.as_deref().filter(|id| !id.is_empty()).map(|id| format!("workspace_id={id}"));
        level_rank(record.level) <= min_rank
            && self.after.map_or(true, |after| record.sequence > after)
            && self.category.as_deref().filter(|category| !category.is_empty()).map_or(true, |category| record.category == category)
            && workspace.map_or(true, |workspace| record.fields.split(' ').any(|field| field == workspace))
            && needle.map_or(true, |needle| {
                record.message.to_lowercase().contains(&needle)
                    || record.target.to_lowercase().contains(&needle)
                    || record.fields.to_lowercase().contains(&needle)
            })
    }

    fn limit(&self) -> usize {
        self.limit.clamp(1, GENERAL_CAPACITY + AUDIT_CAPACITY)
    }
}

impl LogBuffer {
    /// Matching events, newest first.
    pub fn query(&self, query: &LogQuery) -> Vec<LogRecord> {
        let collect = |queue: &Mutex<VecDeque<LogRecord>>| -> Vec<LogRecord> {
            queue
                .lock()
                .map(|queue| queue.iter().filter(|record| query.matches(record)).cloned().collect())
                .unwrap_or_default()
        };
        let mut records = collect(&self.audit);
        records.extend(collect(&self.general));
        records.sort_by(|left, right| right.sequence.cmp(&left.sequence));
        records.truncate(query.limit());
        records
    }

    pub fn len(&self) -> usize {
        let size = |queue: &Mutex<VecDeque<LogRecord>>| queue.lock().map(|queue| queue.len()).unwrap_or(0);
        size(&self.general) + size(&self.audit)
    }
}

// ---------------------------------------------------------------------------
// JSON console output

struct JsonConsoleLayer;

impl<S: Subscriber> Layer<S> for JsonConsoleLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let line = file_line(&record_of(event, 0));
        let mut stdout = std::io::stdout().lock();
        let _ = writeln!(stdout, "{line}");
    }
}

// ---------------------------------------------------------------------------
// Daily files

pub fn log_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("logs")
}

fn file_for_day(dir: &Path, day: NaiveDate) -> PathBuf {
    dir.join(format!("{FILE_PREFIX}{day}{FILE_SUFFIX}"))
}

fn day_of_file(name: &str) -> Option<NaiveDate> {
    name.strip_prefix(FILE_PREFIX)?
        .strip_suffix(FILE_SUFFIX)
        .and_then(|day| NaiveDate::parse_from_str(day, "%Y-%m-%d").ok())
}

/// Sends lines to a writer thread, which appends them to the day's file.
struct FileSink {
    sender: Mutex<Option<(PathBuf, mpsc::Sender<String>)>>,
}

fn file_sink() -> &'static FileSink {
    FILE_SINK.get_or_init(|| FileSink { sender: Mutex::new(None) })
}

impl FileSink {
    /// Starts writing to `dir`, or stops when `None`.
    fn configure(&self, dir: Option<PathBuf>) {
        let mut sender = self.sender.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if sender.as_ref().map(|(current, _)| current) == dir.as_ref() {
            return;
        }
        // Dropping the old sender ends its thread after it flushes.
        *sender = dir.map(|dir| {
            let (lines, receiver) = mpsc::channel::<String>();
            let thread_dir = dir.clone();
            let _ = std::thread::Builder::new()
                .name("repomemo-log-writer".to_owned())
                .spawn(move || run_file_writer(&thread_dir, receiver));
            (dir, lines)
        });
    }

    fn is_active(&self) -> bool {
        self.sender.lock().map(|sender| sender.is_some()).unwrap_or(false)
    }

    fn write(&self, record: &LogRecord) {
        if let Ok(sender) = self.sender.lock() {
            if let Some((_, lines)) = sender.as_ref() {
                let _ = lines.send(file_line(record));
            }
        }
    }
}

fn run_file_writer(dir: &Path, receiver: mpsc::Receiver<String>) {
    if let Err(error) = std::fs::create_dir_all(dir) {
        eprintln!("RepoMemo could not create the log folder {}: {error}", dir.display());
        return;
    }
    let mut current: Option<(NaiveDate, BufWriter<File>)> = None;
    loop {
        match receiver.recv_timeout(Duration::from_secs(1)) {
            Ok(line) => {
                let today = Utc::now().date_naive();
                if current.as_ref().map(|(day, _)| *day) != Some(today) {
                    if let Some((_, mut writer)) = current.take() {
                        let _ = writer.flush();
                    }
                    match std::fs::OpenOptions::new().create(true).append(true).open(file_for_day(dir, today)) {
                        Ok(file) => current = Some((today, BufWriter::new(file))),
                        Err(error) => {
                            eprintln!("RepoMemo could not open its log file in {}: {error}", dir.display());
                            continue;
                        }
                    }
                }
                if let Some((_, writer)) = current.as_mut() {
                    let _ = writeln!(writer, "{line}");
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some((_, writer)) = current.as_mut() {
                    let _ = writer.flush();
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    if let Some((_, mut writer)) = current {
        let _ = writer.flush();
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LogFileInfo {
    /// `YYYY-MM-DD`, in UTC.
    pub day: String,
    pub bytes: u64,
}

/// The log files in the data directory, newest first.
pub fn list_log_files(data_dir: &Path) -> Vec<LogFileInfo> {
    let mut files = std::fs::read_dir(log_dir(data_dir))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| {
                    let day = day_of_file(entry.file_name().to_str()?)?;
                    Some(LogFileInfo { day: day.to_string(), bytes: entry.metadata().ok()?.len() })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    files.sort_by(|left, right| right.day.cmp(&left.day));
    files
}

/// The path of a day's log file; `day` must be `YYYY-MM-DD`.
pub fn log_file_path(data_dir: &Path, day: &str) -> Result<PathBuf> {
    let day = NaiveDate::parse_from_str(day, "%Y-%m-%d").context("the day must be written YYYY-MM-DD")?;
    Ok(file_for_day(&log_dir(data_dir), day))
}

/// Matching events of a day's file, newest first. Only the last 16 MiB of a
/// very large file are read.
pub fn read_log_file(data_dir: &Path, day: &str, query: &LogQuery) -> Result<Vec<LogRecord>> {
    let path = log_file_path(data_dir, day)?;
    let mut file = File::open(&path).with_context(|| format!("there is no log file for {day}"))?;
    let length = file.metadata()?.len();
    let start = length.saturating_sub(MAX_FILE_READ_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_READ_BYTES).read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    // When reading from the middle, the first line is a fragment.
    let lines = text.lines().skip(usize::from(start > 0));
    let mut records = lines
        .enumerate()
        .filter_map(|(index, line)| {
            let line = serde_json::from_str::<FileLine>(line).ok()?;
            Some(LogRecord {
                sequence: index as u64 + 1,
                timestamp: line.timestamp,
                level: level_static(&line.level),
                category: category_of(&line.target),
                target: line.target,
                message: line.message,
                fields: line.fields,
            })
        })
        .filter(|record| query.matches(record))
        .collect::<Vec<_>>();
    records.reverse();
    records.truncate(query.limit());
    Ok(records)
}

/// Deletes log files older than `retention_days` (0 keeps everything).
/// Returns how many were deleted.
pub fn prune_log_files(data_dir: &Path, retention_days: i64) -> usize {
    if retention_days <= 0 {
        return 0;
    }
    let oldest_kept = Utc::now().date_naive() - chrono::Duration::days(retention_days - 1);
    let dir = log_dir(data_dir);
    list_log_files(data_dir)
        .into_iter()
        .filter_map(|file| NaiveDate::parse_from_str(&file.day, "%Y-%m-%d").ok())
        .filter(|day| *day < oldest_kept)
        .filter(|day| std::fs::remove_file(file_for_day(&dir, *day)).is_ok())
        .count()
}

// ---------------------------------------------------------------------------
// Field and level helpers

#[derive(Default)]
struct FieldVisitor {
    message: String,
    fields: String,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            self.push(field, value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            self.push(field, &format!("{value:?}"));
        }
    }
}

impl FieldVisitor {
    fn push(&mut self, field: &Field, value: &str) {
        if !self.fields.is_empty() {
            self.fields.push(' ');
        }
        let _ = write!(self.fields, "{}={value}", field.name());
    }
}

fn truncate(text: String) -> String {
    if text.chars().count() <= MAX_MESSAGE_CHARS {
        return text;
    }
    let mut cut = text.chars().take(MAX_MESSAGE_CHARS).collect::<String>();
    cut.push('…');
    cut
}

fn level_name(level: &Level) -> &'static str {
    match *level {
        Level::ERROR => "error",
        Level::WARN => "warn",
        Level::INFO => "info",
        Level::DEBUG => "debug",
        Level::TRACE => "trace",
    }
}

fn level_static(level: &str) -> &'static str {
    match level.to_ascii_lowercase().as_str() {
        "error" => "error",
        "warn" | "warning" => "warn",
        "info" => "info",
        "debug" => "debug",
        _ => "trace",
    }
}

fn level_rank(level: &str) -> u8 {
    match level.to_ascii_lowercase().as_str() {
        "error" => 0,
        "warn" | "warning" => 1,
        "info" => 2,
        "debug" => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_captured_and_filtered_by_level_category_workspace_and_text() {
        let subscriber = tracing_subscriber::registry().with(log_capture_layer());
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "repomemo_server", count = 3, "Indexed evidence");
            tracing::warn!(target: "audit", user_id = "u1", "Sign-in failed");
            tracing::info!(target: "activity", workspace_id = "ws-1", action = "evidence_stored", "Stored evidence");
            tracing::info!(target: "repomemo_server::indexing", workspace_id = "ws-2", "Indexing");
        });
        let buffer = buffer().unwrap();
        let all = buffer.query(&LogQuery { limit: 100, ..Default::default() });
        let indexed = all.iter().find(|record| record.message == "Indexed evidence").unwrap();
        assert_eq!((indexed.level, indexed.category, indexed.fields.as_str()), ("info", "server", "count=3"));

        let security = buffer.query(&LogQuery { category: Some("security".to_owned()), limit: 100, ..Default::default() });
        assert!(!security.is_empty() && security.iter().all(|record| record.target == "audit"));
        let jobs = buffer.query(&LogQuery { category: Some("jobs".to_owned()), limit: 100, ..Default::default() });
        assert!(jobs.iter().any(|record| record.message == "Indexing"));
        let workspace = buffer.query(&LogQuery { workspace_id: Some("ws-1".to_owned()), limit: 100, ..Default::default() });
        assert!(!workspace.is_empty() && workspace.iter().all(|record| record.fields.contains("workspace_id=ws-1")));

        let warnings = buffer.query(&LogQuery { min_level: Some("warn".to_owned()), limit: 100, ..Default::default() });
        assert!(warnings.iter().all(|record| record.level == "warn" || record.level == "error"));
        assert!(!buffer.query(&LogQuery { contains: Some("INDEXED".to_owned()), limit: 100, ..Default::default() }).is_empty());
        let newest = all.first().unwrap().sequence;
        assert!(buffer.query(&LogQuery { after: Some(newest), limit: 100, ..Default::default() }).is_empty());
    }

    #[test]
    fn categories_compile_to_a_filter_and_classify_targets() {
        let mut settings = LogSettings::default();
        settings.categories.insert("http", "debug".to_owned());
        settings.categories.insert("database", "off".to_owned());
        let directives = settings.directives();
        assert!(directives.starts_with("info,"));
        assert!(directives.contains("tower_http=debug"));
        assert!(directives.contains("sqlx=off"));
        assert!(EnvFilter::try_new(&directives).is_ok());
        settings.env_filter = Some("warn,repomemo_server=debug".to_owned());
        assert_eq!(settings.directives(), "warn,repomemo_server=debug");

        assert_eq!(category_of("audit"), "security");
        assert_eq!(category_of("repomemo_server::indexing"), "jobs");
        assert_eq!(category_of("tower_http::trace::on_response"), "http");
        assert_eq!(category_of("repomemo_server"), "server");
        assert_eq!(category_of("auditor"), "server", "a prefix must end at a module boundary");
    }

    #[test]
    fn day_files_are_read_filtered_and_pruned() {
        let data_dir = std::env::temp_dir().join(format!("repomemo-logs-{}", uuid::Uuid::new_v4()));
        let dir = log_dir(&data_dir);
        std::fs::create_dir_all(&dir).unwrap();
        let today = Utc::now().date_naive();
        let line = |level: &str, target: &str, message: &str| {
            serde_json::to_string(&FileLine { timestamp: Utc::now().to_rfc3339(), level: level.to_owned(), target: target.to_owned(), message: message.to_owned(), fields: String::new() }).unwrap()
        };
        std::fs::write(
            file_for_day(&dir, today),
            [line("info", "repomemo_server", "first"), line("warn", "audit", "second"), "not json".to_owned()].join("\n"),
        )
        .unwrap();
        std::fs::write(file_for_day(&dir, today - chrono::Duration::days(30)), line("info", "x", "old")).unwrap();
        std::fs::write(dir.join("unrelated.txt"), "ignored").unwrap();

        let files = list_log_files(&data_dir);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].day, today.to_string());
        let records = read_log_file(&data_dir, &today.to_string(), &LogQuery { limit: 10, ..Default::default() }).unwrap();
        assert_eq!(records.iter().map(|record| record.message.as_str()).collect::<Vec<_>>(), vec!["second", "first"]);
        let security = read_log_file(&data_dir, &today.to_string(), &LogQuery { category: Some("security".to_owned()), limit: 10, ..Default::default() }).unwrap();
        assert_eq!(security.len(), 1);
        assert!(log_file_path(&data_dir, "../../etc").is_err());
        assert!(read_log_file(&data_dir, "2001-01-01", &LogQuery::default()).is_err());

        assert_eq!(prune_log_files(&data_dir, 0), 0, "0 keeps everything");
        assert_eq!(prune_log_files(&data_dir, 14), 1);
        assert_eq!(list_log_files(&data_dir).len(), 1);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn the_file_sink_writes_lines_to_the_day_file() {
        let data_dir = std::env::temp_dir().join(format!("repomemo-sink-{}", uuid::Uuid::new_v4()));
        let sink = FileSink { sender: Mutex::new(None) };
        sink.configure(Some(log_dir(&data_dir)));
        assert!(sink.is_active());
        let record = LogRecord { sequence: 1, timestamp: Utc::now().to_rfc3339(), level: "info", target: "activity".to_owned(), category: "activity", message: "Stored".to_owned(), fields: "workspace_id=w".to_owned() };
        sink.write(&record);
        sink.configure(None);
        assert!(!sink.is_active());
        let path = file_for_day(&log_dir(&data_dir), Utc::now().date_naive());
        let mut written = String::new();
        for _ in 0..50 {
            written = std::fs::read_to_string(&path).unwrap_or_default();
            if !written.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(written.contains("\"message\":\"Stored\""), "{written}");
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
