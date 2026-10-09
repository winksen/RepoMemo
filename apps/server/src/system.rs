//! System administration: the `/v1/system/*` API, settings that system
//! administrators change at run time, and what the server observes about
//! itself (its instance, the clients calling it, its traffic).
//!
//! Every route here requires a system administrator. Changes are recorded in
//! the durable system audit trail and logged under the `audit` target.

use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Path, Query, Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use chrono::Utc;
use repomemo_domain::{
    CountByLabel, IndexingJobStatus, SystemAuditEvent, SystemStatistics, SystemUser, WorkspaceUsage,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    logs::{self, ConsoleFormat, LogFileInfo, LogQuery, LogRecord, CATEGORIES},
    maintenance::{self, MaintenanceReport},
    map_storage_error,
    security::client_ip,
    ApiError, AppState, AuthenticatedSubject, Quota, RuntimeSettings,
};

// ---------------------------------------------------------------------------
// Run-time settings

/// The run-time settings in force, swapped as a whole when a system
/// administrator changes one, plus the defaults the environment gave.
pub(crate) struct SettingsCell {
    defaults: RuntimeSettings,
    current: RwLock<Arc<RuntimeSettings>>,
    /// Where log files go when file logging is on.
    data_dir: PathBuf,
}

impl SettingsCell {
    pub(crate) fn new(defaults: RuntimeSettings, data_dir: PathBuf) -> Self {
        Self {
            current: RwLock::new(Arc::new(defaults.clone())),
            defaults,
            data_dir,
        }
    }

    pub(crate) fn current(&self) -> Arc<RuntimeSettings> {
        self.current
            .read()
            .map(|settings| settings.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone())
    }

    /// Swaps in new settings and applies the logging part to the running
    /// log stack.
    fn replace(&self, settings: RuntimeSettings) {
        if let Err(error) = logs::apply(&settings.logging, &self.data_dir) {
            tracing::warn!(error = %format!("{error:#}"), "The logging settings could not be applied");
        }
        let mut current = self
            .current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *current = Arc::new(settings);
    }

    /// Applies the stored overrides on top of the defaults. Invalid stored
    /// values (from an older version, or edited by hand) are logged and
    /// skipped.
    pub(crate) fn load_overrides(&self, overrides: &[(String, Value)]) {
        let mut settings = self.defaults.clone();
        for (key, value) in overrides {
            if let Err(message) = apply_setting(&mut settings, key, value) {
                tracing::warn!(setting = %key, error = %message, "Ignoring an invalid stored system setting");
            }
        }
        self.replace(settings);
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum SettingKind {
    Boolean,
    Integer,
    /// One of `options`.
    Choice,
}

struct SettingSpec {
    key: &'static str,
    group: &'static str,
    label: &'static str,
    description: &'static str,
    kind: SettingKind,
    min: i64,
    max: i64,
    unit: &'static str,
    /// The allowed values of a choice, with their labels.
    options: &'static [(&'static str, &'static str)],
}

const LEVEL_OPTIONS: &[(&str, &str)] = &[
    ("off", "Off"),
    ("error", "Errors only"),
    ("warn", "Warnings and errors"),
    ("info", "Information and up"),
    ("debug", "Debug and up"),
    ("trace", "Everything (trace)"),
];

const FORMAT_OPTIONS: &[(&str, &str)] = &[
    ("text", "Readable text"),
    ("compact", "Compact text"),
    ("json", "JSON lines"),
];

/// The setting holding a log category's level.
fn category_setting(category: &str) -> String {
    format!("log_{category}_level")
}

const SETTINGS: &[SettingSpec] = &[
    SettingSpec { key: "allow_registration", group: "Accounts", label: "Open registration", description: "Anyone who can reach the server can create an account. When off, people are added after an administrator creates their account elsewhere; the first account can always register.", kind: SettingKind::Boolean, min: 0, max: 1, unit: "", options: &[] },
    SettingSpec { key: "access_token_ttl_minutes", group: "Accounts", label: "Access token lifetime", description: "How long a sign-in token is valid before the browser renews it silently.", kind: SettingKind::Integer, min: 5, max: 1440, unit: "minutes", options: &[] },
    SettingSpec { key: "refresh_token_ttl_days", group: "Accounts", label: "Session lifetime", description: "How long a session lasts without use before people must sign in again.", kind: SettingKind::Integer, min: 1, max: 365, unit: "days", options: &[] },
    SettingSpec { key: "auth_requests_per_minute", group: "Protection", label: "Sign-in requests per address", description: "Sign-ins and registrations allowed per client address each minute (session renewals get four times as many). 0 turns the limit off.", kind: SettingKind::Integer, min: 0, max: 10_000, unit: "per minute", options: &[] },
    SettingSpec { key: "login_max_failures", group: "Protection", label: "Failed sign-ins before lockout", description: "Consecutive failures for one account from one address before it is locked; after five times as many from all addresses the account is locked everywhere. 0 turns lockout off.", kind: SettingKind::Integer, min: 0, max: 1_000, unit: "failures", options: &[] },
    SettingSpec { key: "login_lockout_minutes", group: "Protection", label: "Lockout length", description: "How long a locked account waits before sign-in is allowed again.", kind: SettingKind::Integer, min: 1, max: 1440, unit: "minutes", options: &[] },
    SettingSpec { key: "ai_requests_per_hour", group: "AI", label: "AI requests per person", description: "Answers, overviews, summaries and assistant AI actions each person may start per hour. 0 is unlimited.", kind: SettingKind::Integer, min: 0, max: 100_000, unit: "per hour", options: &[] },
    SettingSpec { key: "repo_poll_seconds", group: "Background", label: "Repository check interval", description: "How often linked repositories are checked for new commits and synced. 0 turns automatic syncing off; otherwise at least 30 seconds.", kind: SettingKind::Integer, min: 0, max: 86_400, unit: "seconds", options: &[] },
    SettingSpec { key: "maintenance_interval_minutes", group: "Background", label: "Maintenance interval", description: "How often background maintenance runs (cleanup, garbage collection, retries, database checkpoint). 0 turns it off.", kind: SettingKind::Integer, min: 0, max: 10_080, unit: "minutes", options: &[] },
    SettingSpec { key: "blob_gc", group: "Background", label: "Delete unused stored files", description: "Maintenance deletes stored files that no evidence references any more, and their cached previews.", kind: SettingKind::Boolean, min: 0, max: 1, unit: "", options: &[] },
    SettingSpec { key: "job_retention_days", group: "Background", label: "Job history", description: "Finished jobs are kept this long. 0 keeps them forever.", kind: SettingKind::Integer, min: 0, max: 3_650, unit: "days", options: &[] },
    SettingSpec { key: "index_retry_hours", group: "Background", label: "Failed indexing retry", description: "How often indexing that failed for good is tried again.", kind: SettingKind::Integer, min: 1, max: 720, unit: "hours", options: &[] },
    SettingSpec { key: "log_level", group: "Logging", label: "Default level", description: "Applies to the server itself and to every source not listed below. Debug and trace are verbose and can include request details: use them briefly. Changing any level here replaces a RUST_LOG filter set in the environment.", kind: SettingKind::Choice, min: 0, max: 0, unit: "", options: LEVEL_OPTIONS },
    SettingSpec { key: "log_security_level", group: "Logging", label: "Security events", description: "Sign-ins, failed sign-ins and lockouts, role changes, settings changes, provider keys.", kind: SettingKind::Choice, min: 0, max: 0, unit: "", options: LEVEL_OPTIONS },
    SettingSpec { key: "log_activity_level", group: "Logging", label: "Workspace activity", description: "Every action recorded in a workspace's activity feed, with its workspace, so the Logs page can filter by workspace.", kind: SettingKind::Choice, min: 0, max: 0, unit: "", options: LEVEL_OPTIONS },
    SettingSpec { key: "log_jobs_level", group: "Logging", label: "Background jobs", description: "Indexing, search vectors, repository syncs, previews and maintenance; finished and failed jobs.", kind: SettingKind::Choice, min: 0, max: 0, unit: "", options: LEVEL_OPTIONS },
    SettingSpec { key: "log_http_level", group: "Logging", label: "HTTP requests", description: "Requests the API serves. Debug lists every request with its status and duration.", kind: SettingKind::Choice, min: 0, max: 0, unit: "", options: LEVEL_OPTIONS },
    SettingSpec { key: "log_ai_level", group: "Logging", label: "AI providers", description: "Calls to the configured AI providers.", kind: SettingKind::Choice, min: 0, max: 0, unit: "", options: LEVEL_OPTIONS },
    SettingSpec { key: "log_database_level", group: "Logging", label: "Database", description: "Storage and SQL. Debug lists every query.", kind: SettingKind::Choice, min: 0, max: 0, unit: "", options: LEVEL_OPTIONS },
    SettingSpec { key: "log_console_format", group: "Logging", label: "Console format", description: "How the server prints logs to its console (what a service manager or container collects). Log files are always JSON lines so they can be searched here.", kind: SettingKind::Choice, min: 0, max: 0, unit: "", options: FORMAT_OPTIONS },
    SettingSpec { key: "log_to_file", group: "Logging", label: "Write log files", description: "Keeps logs in one file per day (JSON lines) in the logs folder of the data folder. Past days can be browsed and downloaded under Logs.", kind: SettingKind::Boolean, min: 0, max: 1, unit: "", options: &[] },
    SettingSpec { key: "log_retention_days", group: "Logging", label: "Log file retention", description: "Daily log files older than this are deleted by maintenance. 0 keeps them forever.", kind: SettingKind::Integer, min: 0, max: 3_650, unit: "days", options: &[] },
    SettingSpec { key: "system_audit_retention_days", group: "Background", label: "System audit history", description: "System administration events are kept this long. 0 keeps them forever.", kind: SettingKind::Integer, min: 0, max: 3_650, unit: "days", options: &[] },
];

fn spec(key: &str) -> Option<&'static SettingSpec> {
    SETTINGS.iter().find(|spec| spec.key == key)
}

fn read_setting(settings: &RuntimeSettings, key: &str) -> Value {
    let maintenance = &settings.maintenance;
    match key {
        "allow_registration" => json!(settings.allow_registration),
        "access_token_ttl_minutes" => json!(settings.access_token_ttl_minutes),
        "refresh_token_ttl_days" => json!(settings.refresh_token_ttl_days),
        "auth_requests_per_minute" => json!(settings.auth_quota.limit),
        "login_max_failures" => json!(settings.login_max_failures),
        "login_lockout_minutes" => json!(settings.login_lockout.as_secs() / 60),
        "ai_requests_per_hour" => json!(settings.ai_quota.limit),
        "repo_poll_seconds" => json!(settings.repo_poll_interval.map_or(0, |interval| interval.as_secs())),
        "maintenance_interval_minutes" => json!(maintenance.interval.as_secs() / 60),
        "blob_gc" => json!(maintenance.blob_gc),
        "job_retention_days" => json!(maintenance.job_retention_days),
        "index_retry_hours" => json!(maintenance.index_retry_interval.as_secs() / 3600),
        "system_audit_retention_days" => json!(settings.system_audit_retention_days),
        "log_level" => json!(settings.logging.level),
        "log_console_format" => json!(settings.logging.console_format.as_str()),
        "log_to_file" => json!(settings.logging.to_file),
        "log_retention_days" => json!(settings.logging.retention_days),
        _ => CATEGORIES
            .iter()
            .find(|category| category_setting(category.key) == key)
            .map(|category| {
                json!(settings
                    .logging
                    .categories
                    .get(category.key)
                    .map_or(category.default_level, String::as_str))
            })
            .unwrap_or(Value::Null),
    }
}

/// Validates `value` against the setting's spec and writes it into
/// `settings`.
fn apply_setting(settings: &mut RuntimeSettings, key: &str, value: &Value) -> Result<(), String> {
    let spec = spec(key).ok_or_else(|| format!("{key} is not a setting that can be changed here."))?;
    match spec.kind {
        SettingKind::Boolean => {
            let flag = value
                .as_bool()
                .ok_or_else(|| format!("{} must be true or false.", spec.label))?;
            match key {
                "allow_registration" => settings.allow_registration = flag,
                "blob_gc" => settings.maintenance.blob_gc = flag,
                "log_to_file" => settings.logging.to_file = flag,
                _ => unreachable!("every boolean setting is handled"),
            }
        }
        SettingKind::Choice => {
            let choice = value
                .as_str()
                .filter(|choice| spec.options.iter().any(|(option, _)| option == choice))
                .ok_or_else(|| {
                    format!(
                        "{} must be one of: {}.",
                        spec.label,
                        spec.options.iter().map(|(option, _)| *option).collect::<Vec<_>>().join(", ")
                    )
                })?;
            if key == "log_console_format" {
                settings.logging.console_format = ConsoleFormat::parse(choice).expect("listed formats parse");
            } else if key == "log_level" {
                if choice == "off" {
                    return Err(format!("{} cannot be off; turn single sources off instead.", spec.label));
                }
                settings.logging.level = choice.to_owned();
                // Levels set here replace a RUST_LOG filter from the environment.
                settings.logging.env_filter = None;
            } else {
                let category = CATEGORIES
                    .iter()
                    .find(|category| category_setting(category.key) == key)
                    .expect("every choice setting is handled");
                settings.logging.categories.insert(category.key, choice.to_owned());
                settings.logging.env_filter = None;
            }
        }
        SettingKind::Integer => {
            let number = value
                .as_i64()
                .filter(|number| (spec.min..=spec.max).contains(number))
                .ok_or_else(|| format!("{} must be a whole number from {} to {}.", spec.label, spec.min, spec.max))?;
            let count = number as u32;
            let seconds = number as u64;
            match key {
                "access_token_ttl_minutes" => settings.access_token_ttl_minutes = number,
                "refresh_token_ttl_days" => settings.refresh_token_ttl_days = number,
                "auth_requests_per_minute" => settings.auth_quota = Quota::per_minute(count),
                "login_max_failures" => settings.login_max_failures = count,
                "login_lockout_minutes" => settings.login_lockout = Duration::from_secs(seconds * 60),
                "ai_requests_per_hour" => settings.ai_quota = Quota::per_hour(count),
                "repo_poll_seconds" => {
                    if number > 0 && number < 30 {
                        return Err(format!("{} must be 0 (off) or at least 30 seconds.", spec.label));
                    }
                    settings.repo_poll_interval = (number > 0).then(|| Duration::from_secs(seconds));
                }
                "maintenance_interval_minutes" => {
                    settings.maintenance.interval = Duration::from_secs(seconds * 60)
                }
                "job_retention_days" => settings.maintenance.job_retention_days = number,
                "index_retry_hours" => {
                    settings.maintenance.index_retry_interval = Duration::from_secs(seconds * 3600)
                }
                "system_audit_retention_days" => settings.system_audit_retention_days = number,
                "log_retention_days" => settings.logging.retention_days = number,
                _ => unreachable!("every integer setting is handled"),
            }
        }
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct SettingView {
    key: &'static str,
    group: &'static str,
    label: &'static str,
    description: &'static str,
    kind: SettingKind,
    min: i64,
    max: i64,
    unit: &'static str,
    options: Vec<SettingOption>,
    value: Value,
    default_value: Value,
    /// Changed by a system administrator, overriding the server default.
    overridden: bool,
    updated_at: Option<String>,
    updated_by: Option<String>,
}

#[derive(Debug, Serialize)]
struct SettingOption {
    value: &'static str,
    label: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct SettingsResponse {
    settings: Vec<SettingView>,
    /// Settings only the server's environment can change, shown for reference.
    environment: Vec<EnvironmentFact>,
}

#[derive(Debug, Serialize)]
struct EnvironmentFact {
    label: &'static str,
    value: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateSettingsRequest {
    values: BTreeMap<String, Value>,
}

// ---------------------------------------------------------------------------
// What the server knows about itself

/// Facts about this server process, fixed at startup.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct InstanceInfo {
    pub service_name: String,
    pub version: &'static str,
    pub host_name: String,
    pub operating_system: &'static str,
    pub architecture: &'static str,
    pub process_id: u32,
    pub bind_address: String,
    pub data_dir: String,
    pub started_at: String,
    #[serde(skip)]
    pub started: Instant,
    #[serde(skip)]
    pub environment: Vec<(&'static str, String)>,
}

impl InstanceInfo {
    pub(crate) fn new(config: &crate::ServerConfig, libreoffice: bool) -> Self {
        let data_dir = std::fs::canonicalize(&config.data_dir)
            .unwrap_or_else(|_| config.data_dir.clone())
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_owned();
        let list = |items: &[String]| if items.is_empty() { "any".to_owned() } else { items.join(", ") };
        let environment = vec![
            ("Listening address", config.bind_address.to_string()),
            ("Data folder", data_dir.clone()),
            (
                "Allowed browser origins",
                config
                    .allowed_origins
                    .iter()
                    .filter_map(|origin| origin.to_str().ok())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            ("Behind a trusted proxy", yes_no(config.trust_proxy)),
            ("Largest upload", format!("{} MiB", config.max_upload_bytes / (1024 * 1024))),
            (
                "Secret key for stored credentials",
                if config.secret_key.is_some() { "from REPOMEMO_SECRET_KEY".to_owned() } else { "key file in the data folder".to_owned() },
            ),
            ("AI provider hosts", config.ai_allowed_hosts.as_deref().map_or_else(|| "any (metadata addresses refused)".to_owned(), list)),
            (
                "Repository folders",
                list(&config.repo_roots.iter().map(|root| root.to_string_lossy().into_owned()).collect::<Vec<_>>()),
            ),
            ("System administrators from the environment", config.system_admin_emails.len().to_string()),
            ("LibreOffice previews", if libreoffice { "available".to_owned() } else { "not installed".to_owned() }),
            (
                "Log filter from RUST_LOG",
                config.logging.env_filter.clone().unwrap_or_else(|| "not set".to_owned()),
            ),
        ];
        Self {
            service_name: config.service_name.clone(),
            version: env!("CARGO_PKG_VERSION"),
            host_name: std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .unwrap_or_else(|_| "unknown".to_owned()),
            operating_system: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            process_id: std::process::id(),
            bind_address: config.bind_address.to_string(),
            data_dir,
            started_at: Utc::now().to_rfc3339(),
            started: Instant::now(),
            environment,
        }
    }
}

fn yes_no(value: bool) -> String {
    if value { "yes" } else { "no" }.to_owned()
}

/// A browser or tool seen calling the API.
#[derive(Debug, Clone, Serialize)]
struct ClientSighting {
    origin: Option<String>,
    /// The `X-RepoMemo-Client` header the web client sends, such as `web`.
    client: Option<String>,
    agent: String,
    last_address: String,
    first_seen_at: String,
    last_seen_at: String,
    requests: u64,
    #[serde(skip)]
    last_seen: Instant,
}

const MAX_CLIENTS: usize = 500;
const TRAFFIC_MINUTES: usize = 60;

/// Request counters and the clients seen, kept in memory since startup.
#[derive(Default)]
pub(crate) struct Telemetry {
    clients: Mutex<HashMap<String, ClientSighting>>,
    requests: AtomicU64,
    client_errors: AtomicU64,
    server_errors: AtomicU64,
    unauthorized: AtomicU64,
    forbidden: AtomicU64,
    rate_limited: AtomicU64,
    /// Requests per minute for the last hour: (minute since the epoch, count).
    per_minute: Mutex<VecDeque<(u64, u64)>>,
    pub(crate) open_event_streams: AtomicUsize,
}

impl Telemetry {
    fn record(&self, status: StatusCode, sighting: Option<(String, Option<String>, Option<String>, String)>) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        match status.as_u16() {
            401 => self.unauthorized.fetch_add(1, Ordering::Relaxed),
            403 => self.forbidden.fetch_add(1, Ordering::Relaxed),
            429 => self.rate_limited.fetch_add(1, Ordering::Relaxed),
            _ => 0,
        };
        if status.is_client_error() {
            self.client_errors.fetch_add(1, Ordering::Relaxed);
        } else if status.is_server_error() {
            self.server_errors.fetch_add(1, Ordering::Relaxed);
        }
        let minute = jsonwebtoken::get_current_timestamp() / 60;
        if let Ok(mut per_minute) = self.per_minute.lock() {
            match per_minute.back_mut() {
                Some((last, count)) if *last == minute => *count += 1,
                _ => per_minute.push_back((minute, 1)),
            }
            while per_minute.front().is_some_and(|(first, _)| *first + TRAFFIC_MINUTES as u64 <= minute) {
                per_minute.pop_front();
            }
        }
        let Some((address, origin, client, agent)) = sighting else { return };
        let key = format!("{}|{}|{agent}", origin.as_deref().unwrap_or("-"), client.as_deref().unwrap_or("-"));
        let now = Utc::now().to_rfc3339();
        if let Ok(mut clients) = self.clients.lock() {
            if !clients.contains_key(&key) && clients.len() >= MAX_CLIENTS {
                return;
            }
            let entry = clients.entry(key).or_insert_with(|| ClientSighting {
                origin,
                client,
                agent,
                last_address: address.clone(),
                first_seen_at: now.clone(),
                last_seen_at: now.clone(),
                requests: 0,
                last_seen: Instant::now(),
            });
            entry.requests += 1;
            entry.last_address = address;
            entry.last_seen_at = now;
            entry.last_seen = Instant::now();
        }
    }

    /// Forgets clients not seen for a day.
    pub(crate) fn prune(&self) -> usize {
        let Ok(mut clients) = self.clients.lock() else { return 0 };
        let before = clients.len();
        clients.retain(|_, sighting| sighting.last_seen.elapsed() < Duration::from_secs(24 * 3600));
        before - clients.len()
    }

    fn clients(&self) -> Vec<ClientSighting> {
        let mut clients = self
            .clients
            .lock()
            .map(|clients| clients.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        clients.sort_by(|left, right| right.last_seen.cmp(&left.last_seen));
        clients
    }

    fn traffic(&self) -> TrafficView {
        let minute = jsonwebtoken::get_current_timestamp() / 60;
        let recent = self
            .per_minute
            .lock()
            .map(|per_minute| per_minute.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();
        let last_hour = (0..TRAFFIC_MINUTES as u64)
            .rev()
            .map(|ago| {
                let at = minute.saturating_sub(ago);
                recent.iter().find(|(bucket, _)| *bucket == at).map_or(0, |(_, count)| *count)
            })
            .collect::<Vec<_>>();
        TrafficView {
            requests: self.requests.load(Ordering::Relaxed),
            client_errors: self.client_errors.load(Ordering::Relaxed),
            server_errors: self.server_errors.load(Ordering::Relaxed),
            unauthorized: self.unauthorized.load(Ordering::Relaxed),
            forbidden: self.forbidden.load(Ordering::Relaxed),
            rate_limited: self.rate_limited.load(Ordering::Relaxed),
            requests_last_hour: last_hour.iter().sum(),
            requests_per_minute_last_hour: last_hour,
        }
    }
}

#[derive(Debug, Serialize)]
struct TrafficView {
    requests: u64,
    client_errors: u64,
    server_errors: u64,
    unauthorized: u64,
    forbidden: u64,
    rate_limited: u64,
    requests_last_hour: u64,
    /// Oldest first; the last entry is the current minute.
    requests_per_minute_last_hour: Vec<u64>,
}

/// Counts every request and notes which clients call the API. Health checks
/// from load balancers are counted but not listed as clients.
pub(crate) async fn track_requests(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let sighting = sighting(&state, &request);
    let response = next.run(request).await;
    state.telemetry.record(response.status(), sighting);
    response
}

/// Who sent a request: address, origin, client header and user agent.
fn sighting(state: &AppState, request: &Request) -> Option<(String, Option<String>, Option<String>, String)> {
    if request.uri().path().starts_with("/health") {
        return None;
    }
    let header = |name: &str| {
        request
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.chars().take(200).collect::<String>())
    };
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip());
    Some((
        client_ip(request.headers(), peer, state.settings().trust_proxy),
        header("origin"),
        header("x-repomemo-client"),
        describe_agent(header("user-agent").as_deref().unwrap_or("")),
    ))
}

/// A short "Browser on OS" description of a user agent.
fn describe_agent(agent: &str) -> String {
    let browser = [
        ("Edg/", "Edge"),
        ("OPR/", "Opera"),
        ("Firefox/", "Firefox"),
        ("Chrome/", "Chrome"),
        ("Safari/", "Safari"),
        ("PostmanRuntime", "Postman"),
        ("curl/", "curl"),
        ("Microsoft Office", "Microsoft Office"),
    ]
    .iter()
    .find(|(marker, _)| agent.contains(marker))
    .map(|(_, name)| (*name).to_owned())
    .or_else(|| agent.split(['/', ' ']).next().filter(|name| !name.is_empty()).map(str::to_owned))
    .unwrap_or_else(|| "Unknown client".to_owned());
    let system = [
        ("Windows", "Windows"),
        ("Android", "Android"),
        ("iPhone", "iOS"),
        ("iPad", "iPadOS"),
        ("Mac OS X", "macOS"),
        ("Linux", "Linux"),
    ]
    .iter()
    .find(|(marker, _)| agent.contains(marker))
    .map(|(_, name)| *name);
    match system {
        Some(system) => format!("{browser} on {system}"),
        None => browser,
    }
}

// ---------------------------------------------------------------------------
// Maintenance status

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct MaintenanceStatus {
    pub running: bool,
    pub last_started_at: Option<String>,
    pub last_finished_at: Option<String>,
    /// `scheduled` or `manual`.
    pub last_trigger: Option<&'static str>,
    pub last_report: Option<MaintenanceReport>,
    pub next_run_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Handlers

/// The system pages and API: open to system administrators and app administrators.
pub(crate) fn require_system_admin(subject: &AuthenticatedSubject) -> Result<(), ApiError> {
    if subject.is_system_admin || subject.is_app_admin {
        Ok(())
    } else {
        Err(ApiError::forbidden_because("Only system and app administrators can use this."))
    }
}

/// Granting or removing the wildcard role stays with system administrators, so
/// an app administrator cannot promote themselves into it.
fn require_full_system_admin(subject: &AuthenticatedSubject) -> Result<(), ApiError> {
    if subject.is_system_admin {
        Ok(())
    } else {
        Err(ApiError::forbidden_because("Only system administrators can grant or remove the system administrator role."))
    }
}

/// Records a system administration action durably and in the log.
async fn audit(state: &AppState, subject: &AuthenticatedSubject, action: &str, detail: String) {
    tracing::info!(target: "audit", user_id = %subject.user_id, action, detail = %detail, "System administration");
    if let Err(error) = state
        .storage
        .record_system_event(Some(&subject.user_id), action, &detail)
        .await
    {
        tracing::error!(error = %error, "Failed to record a system audit event");
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct OverviewResponse {
    instance: InstanceView,
    statistics: SystemStatistics,
    storage: StorageView,
    background: BackgroundView,
    protection: ProtectionView,
    traffic: TrafficView,
    clients: Vec<ClientSighting>,
    logs: LogsView,
}

#[derive(Debug, Serialize)]
struct InstanceView {
    #[serde(flatten)]
    info: InstanceInfo,
    uptime_seconds: u64,
    system_admin_count: i64,
}

#[derive(Debug, Serialize)]
struct StorageView {
    database_bytes: u64,
    write_ahead_log_bytes: u64,
    blob_bytes: i64,
    preview_bytes: u64,
    preview_count: u64,
}

#[derive(Debug, Serialize)]
struct BackgroundView {
    indexing_queued: usize,
    embedding_workspaces_waiting: usize,
    repository_syncs_running: usize,
    preview_conversions_running: usize,
    event_channels: usize,
    event_subscribers: usize,
    open_event_streams: usize,
    maintenance: MaintenanceStatus,
}

#[derive(Debug, Serialize)]
struct ProtectionView {
    tracked_rate_limit_keys: usize,
    locked_sign_in_keys: usize,
}

#[derive(Debug, Serialize)]
struct LogsView {
    capturing: bool,
    buffered: usize,
}

pub(crate) async fn overview(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<OverviewResponse>, ApiError> {
    require_system_admin(&subject)?;
    let statistics = state.storage.system_statistics().await.map_err(map_storage_error)?;
    let data_dir = PathBuf::from(&state.instance.data_dir);
    let file_size = |name: &str| std::fs::metadata(data_dir.join(name)).map_or(0, |metadata| metadata.len());
    let (preview_count, preview_bytes) = std::fs::read_dir(data_dir.join("previews"))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.metadata().ok().filter(|metadata| metadata.is_file()))
                .fold((0, 0), |(count, bytes), metadata| (count + 1, bytes + metadata.len()))
        })
        .unwrap_or((0, 0));
    let (event_channels, event_subscribers) = state.event_bus.stats();
    let (tracked_rate_limit_keys, locked_sign_in_keys) = state.guards.stats();
    Ok(Json(OverviewResponse {
        instance: InstanceView {
            info: (*state.instance).clone(),
            uptime_seconds: state.instance.started.elapsed().as_secs(),
            system_admin_count: statistics.system_admin_count,
        },
        storage: StorageView {
            database_bytes: file_size("repomemo.sqlite"),
            write_ahead_log_bytes: file_size("repomemo.sqlite-wal"),
            blob_bytes: statistics.blob_bytes,
            preview_bytes,
            preview_count,
        },
        statistics,
        background: BackgroundView {
            indexing_queued: state.index_queue.queued_count(),
            embedding_workspaces_waiting: state.embedding_queue.waiting_count(),
            repository_syncs_running: state.repo_sync.running_count(),
            preview_conversions_running: state.converter.running_count(),
            event_channels,
            event_subscribers,
            open_event_streams: state.telemetry.open_event_streams.load(Ordering::Relaxed),
            maintenance: maintenance_status(&state),
        },
        protection: ProtectionView {
            tracked_rate_limit_keys,
            locked_sign_in_keys,
        },
        traffic: state.telemetry.traffic(),
        clients: state.telemetry.clients(),
        logs: LogsView {
            capturing: logs::buffer().is_some(),
            buffered: logs::buffer().map_or(0, |buffer| buffer.len()),
        },
    }))
}

fn maintenance_status(state: &AppState) -> MaintenanceStatus {
    state
        .maintenance_status
        .lock()
        .map(|status| status.clone())
        .unwrap_or_default()
}

#[derive(Debug, Deserialize)]
pub(crate) struct UsageQuery {
    days: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UsageResponse {
    days: i64,
    activity_by_day: Vec<CountByLabel>,
    activity_by_action: Vec<CountByLabel>,
    top_users: Vec<CountByLabel>,
    workspaces: Vec<WorkspaceUsage>,
}

pub(crate) async fn usage(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<UsageResponse>, ApiError> {
    require_system_admin(&subject)?;
    let days = query.days.unwrap_or(30).clamp(1, 365);
    let since = (Utc::now() - chrono::Duration::days(days - 1)).date_naive();
    let since_text = format!("{since}T00:00:00Z");
    let counts = state
        .storage
        .system_activity_by_day(&since_text)
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .map(|entry| (entry.label, entry.count))
        .collect::<HashMap<_, _>>();
    let activity_by_day = (0..days)
        .map(|offset| {
            let day = (since + chrono::Duration::days(offset)).to_string();
            CountByLabel { count: counts.get(&day).copied().unwrap_or(0), label: day }
        })
        .collect();
    Ok(Json(UsageResponse {
        days,
        activity_by_day,
        activity_by_action: state
            .storage
            .system_activity_by_action(&since_text)
            .await
            .map_err(map_storage_error)?,
        top_users: state
            .storage
            .system_top_users(&since_text, 10)
            .await
            .map_err(map_storage_error)?,
        workspaces: state
            .storage
            .system_workspace_usage(&since_text)
            .await
            .map_err(map_storage_error)?,
    }))
}

pub(crate) async fn list_users(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<Vec<SystemUser>>, ApiError> {
    require_system_admin(&subject)?;
    state
        .storage
        .list_system_users()
        .await
        .map(Json)
        .map_err(map_storage_error)
}

#[derive(Debug, Deserialize)]
pub(crate) struct SystemAdminRequest {
    enabled: bool,
}

pub(crate) async fn set_system_admin(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Json(request): Json<SystemAdminRequest>,
) -> Result<StatusCode, ApiError> {
    require_full_system_admin(&subject)?;
    let user = state
        .storage
        .find_user(&user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("User was not found."))?;
    state
        .storage
        .set_system_admin(&user_id, request.enabled)
        .await
        .map_err(|error| {
            let message = error.to_string();
            if message.contains("last system administrator") {
                ApiError::conflict(message)
            } else {
                map_storage_error(error)
            }
        })?;
    let who = user.email.unwrap_or(user.display_name);
    if request.enabled {
        audit(&state, &subject, "system_admin_granted", format!("Made {who} a system administrator.")).await;
    } else {
        audit(&state, &subject, "system_admin_revoked", format!("Removed {who} from the system administrators.")).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn set_app_admin(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Json(request): Json<SystemAdminRequest>,
) -> Result<StatusCode, ApiError> {
    require_system_admin(&subject)?;
    let user = state
        .storage
        .find_user(&user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("User was not found."))?;
    state
        .storage
        .set_app_admin(&user_id, request.enabled)
        .await
        .map_err(map_storage_error)?;
    let who = user.email.unwrap_or(user.display_name);
    if request.enabled {
        audit(&state, &subject, "app_admin_granted", format!("Made {who} an app administrator.")).await;
    } else {
        audit(&state, &subject, "app_admin_revoked", format!("Removed {who} from the app administrators.")).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Signs a user out of every device.
pub(crate) async fn end_user_sessions(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_system_admin(&subject)?;
    let user = state
        .storage
        .find_user(&user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("User was not found."))?;
    state
        .storage
        .end_user_sessions(&user_id)
        .await
        .map_err(map_storage_error)?;
    let who = user.email.unwrap_or(user.display_name);
    audit(&state, &subject, "user_sessions_ended", format!("Signed {who} out of every device.")).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Lifts a sign-in lockout of a user's account, from every address.
pub(crate) async fn unlock_user(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    require_system_admin(&subject)?;
    let user = state
        .storage
        .find_user(&user_id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("User was not found."))?;
    let email = user.email.clone().unwrap_or_default().to_lowercase();
    let cleared = state.guards.logins.clear_account(&email);
    audit(&state, &subject, "user_unlocked", format!("Lifted sign-in lockouts of {email}.")).await;
    Ok(Json(json!({ "cleared": cleared })))
}

pub(crate) async fn get_settings(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<SettingsResponse>, ApiError> {
    require_system_admin(&subject)?;
    settings_response(&state).await.map(Json)
}

async fn settings_response(state: &AppState) -> Result<SettingsResponse, ApiError> {
    let stored = state
        .storage
        .list_system_settings()
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .map(|setting| (setting.key.clone(), setting))
        .collect::<HashMap<_, _>>();
    let current = state.settings();
    let defaults = &state.runtime.defaults;
    let settings = SETTINGS
        .iter()
        .map(|spec| {
            let stored = stored.get(spec.key);
            SettingView {
                key: spec.key,
                group: spec.group,
                label: spec.label,
                description: spec.description,
                kind: spec.kind,
                min: spec.min,
                max: spec.max,
                unit: spec.unit,
                options: spec
                    .options
                    .iter()
                    .map(|(value, label)| SettingOption { value, label })
                    .collect(),
                value: read_setting(&current, spec.key),
                default_value: read_setting(defaults, spec.key),
                overridden: stored.is_some(),
                updated_at: stored.map(|setting| setting.updated_at.clone()),
                updated_by: stored
                    .and_then(|setting| setting.updated_by.as_ref())
                    .map(|user| user.display_name.clone()),
            }
        })
        .collect();
    Ok(SettingsResponse {
        settings,
        environment: state
            .instance
            .environment
            .iter()
            .map(|(label, value)| EnvironmentFact { label, value: value.clone() })
            .collect(),
    })
}

/// Changes one or more settings. All values are checked before any is
/// saved, so a request either applies completely or not at all.
pub(crate) async fn update_settings(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Json(request): Json<UpdateSettingsRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    require_system_admin(&subject)?;
    if request.values.is_empty() {
        return Err(ApiError::bad_request("No settings were given."));
    }
    let mut updated = (*state.settings()).clone();
    for (key, value) in &request.values {
        apply_setting(&mut updated, key, value).map_err(ApiError::bad_request)?;
    }
    for (key, value) in &request.values {
        state
            .storage
            .set_system_setting(key, value, Some(&subject.user_id))
            .await
            .map_err(map_storage_error)?;
    }
    state.runtime.replace(updated);
    let changes = request
        .values
        .iter()
        .map(|(key, value)| format!("{} = {value}", spec(key).map_or(key.as_str(), |spec| spec.label)))
        .collect::<Vec<_>>()
        .join("; ");
    audit(&state, &subject, "system_settings_changed", format!("Changed {changes}.")).await;
    settings_response(&state).await.map(Json)
}

/// Restores a setting's server default.
pub(crate) async fn reset_setting(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<SettingsResponse>, ApiError> {
    require_system_admin(&subject)?;
    let spec = spec(&key).ok_or_else(|| ApiError::not_found(format!("{key} is not a setting.")))?;
    state
        .storage
        .delete_system_setting(&key)
        .await
        .map_err(map_storage_error)?;
    // Rebuild from the defaults and the remaining changes, so a reset can
    // also bring back what the environment chose (such as a RUST_LOG filter).
    let remaining = state
        .storage
        .list_system_settings()
        .await
        .map_err(map_storage_error)?
        .into_iter()
        .map(|setting| (setting.key, setting.value))
        .collect::<Vec<_>>();
    state.runtime.load_overrides(&remaining);
    let default = read_setting(&state.runtime.defaults, &key);
    audit(&state, &subject, "system_setting_reset", format!("Restored the default of {} ({default}).", spec.label)).await;
    settings_response(&state).await.map(Json)
}

#[derive(Debug, Deserialize)]
pub(crate) struct LogsQuery {
    level: Option<String>,
    /// Kept for older clients: the same as `category=security`.
    #[serde(default)]
    audit: bool,
    category: Option<String>,
    workspace: Option<String>,
    q: Option<String>,
    after: Option<u64>,
    limit: Option<usize>,
    /// Read this day's file (`YYYY-MM-DD`) instead of the recent events in
    /// memory.
    day: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LogsResponse {
    /// False when the server runs without log capture (only in tests).
    capturing: bool,
    /// `memory` (recent events) or `file` (a day's file).
    source: &'static str,
    records: Vec<LogRecord>,
}

pub(crate) async fn logs(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Query(query): Query<LogsQuery>,
) -> Result<Json<LogsResponse>, ApiError> {
    require_system_admin(&subject)?;
    let filter = LogQuery {
        min_level: query.level,
        category: query.category.or_else(|| query.audit.then(|| "security".to_owned())),
        workspace_id: query.workspace,
        contains: query.q,
        after: query.after,
        limit: query.limit.unwrap_or(300),
    };
    if let Some(day) = query.day.filter(|day| !day.is_empty()) {
        let data_dir = PathBuf::from(&state.instance.data_dir);
        let records = tokio::task::spawn_blocking(move || logs::read_log_file(&data_dir, &day, &filter))
            .await
            .map_err(ApiError::internal)?
            .map_err(|error| ApiError::not_found(format!("{error:#}")))?;
        return Ok(Json(LogsResponse { capturing: logs::buffer().is_some(), source: "file", records }));
    }
    let Some(buffer) = logs::buffer() else {
        return Ok(Json(LogsResponse { capturing: false, source: "memory", records: Vec::new() }));
    };
    Ok(Json(LogsResponse { capturing: true, source: "memory", records: buffer.query(&filter) }))
}

#[derive(Debug, Serialize)]
struct LogCategoryView {
    key: &'static str,
    label: &'static str,
    level: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct LogFilesResponse {
    files: Vec<LogFileInfo>,
    writing_files: bool,
    retention_days: i64,
    console_format: &'static str,
    /// The filter in force, in tracing's directive syntax.
    effective_filter: Option<String>,
    /// A `RUST_LOG` filter from the environment is in force instead of the
    /// levels below.
    environment_filter: bool,
    default_level: String,
    categories: Vec<LogCategoryView>,
}

/// The day files kept, and how logging is set up now.
pub(crate) async fn log_files(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<LogFilesResponse>, ApiError> {
    require_system_admin(&subject)?;
    let settings = state.settings();
    let data_dir = PathBuf::from(&state.instance.data_dir);
    let files = tokio::task::spawn_blocking(move || logs::list_log_files(&data_dir))
        .await
        .map_err(ApiError::internal)?;
    let logging = &settings.logging;
    Ok(Json(LogFilesResponse {
        files,
        writing_files: logs::writing_files(),
        retention_days: logging.retention_days,
        console_format: logging.console_format.as_str(),
        effective_filter: logs::effective_filter(),
        environment_filter: logging.env_filter.is_some(),
        default_level: logging.level.clone(),
        categories: CATEGORIES
            .iter()
            .map(|category| LogCategoryView {
                key: category.key,
                label: category.label,
                level: logging
                    .categories
                    .get(category.key)
                    .cloned()
                    .unwrap_or_else(|| category.default_level.to_owned()),
            })
            .collect(),
    }))
}

/// A day's log file as a download.
pub(crate) async fn download_log_file(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Path(day): Path<String>,
) -> Result<Response, ApiError> {
    require_system_admin(&subject)?;
    let path = logs::log_file_path(&PathBuf::from(&state.instance.data_dir), &day)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| ApiError::not_found(format!("There is no log file for {day}.")))?;
    audit(&state, &subject, "log_file_downloaded", format!("Downloaded the log file of {day}.")).await;
    let filename = path.file_name().and_then(|name| name.to_str()).unwrap_or("repomemo.jsonl").to_owned();
    let mut response = Response::new(axum::body::Body::from(bytes));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/x-ndjson"));
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    Ok(response)
}

#[derive(Debug, Deserialize)]
pub(crate) struct LimitQuery {
    limit: Option<i64>,
}

pub(crate) async fn audit_events(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Query(query): Query<LimitQuery>,
) -> Result<Json<Vec<SystemAuditEvent>>, ApiError> {
    require_system_admin(&subject)?;
    state
        .storage
        .list_system_events(query.limit.unwrap_or(200))
        .await
        .map(Json)
        .map_err(map_storage_error)
}

#[derive(Debug, Deserialize)]
pub(crate) struct JobsQuery {
    status: Option<String>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SystemJob {
    #[serde(flatten)]
    job: IndexingJobStatus,
    workspace_name: Option<String>,
}

pub(crate) async fn jobs(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Query(query): Query<JobsQuery>,
) -> Result<Json<Vec<SystemJob>>, ApiError> {
    require_system_admin(&subject)?;
    let names = state.storage.workspace_names().await.map_err(map_storage_error)?;
    let jobs = state
        .storage
        .list_all_jobs(query.status.as_deref().filter(|status| !status.is_empty()), query.limit.unwrap_or(100))
        .await
        .map_err(map_storage_error)?;
    Ok(Json(
        jobs.into_iter()
            .map(|job| SystemJob {
                workspace_name: names.get(&job.workspace_id).cloned(),
                job,
            })
            .collect(),
    ))
}

pub(crate) async fn get_maintenance(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<MaintenanceStatus>, ApiError> {
    require_system_admin(&subject)?;
    Ok(Json(maintenance_status(&state)))
}

/// Runs a maintenance pass now, including the steps that normally run less
/// often (retrying failed indexing, sweeping untracked files).
pub(crate) async fn run_maintenance(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<MaintenanceStatus>, ApiError> {
    require_system_admin(&subject)?;
    let report = maintenance::run_tracked(&state, "manual", true, true)
        .await
        .ok_or_else(|| ApiError::conflict("Maintenance is already running."))?;
    audit(
        &state,
        &subject,
        "maintenance_run",
        format!(
            "Ran maintenance: {} blobs removed, {} jobs pruned, {} tokens purged, {} failures.",
            report.blobs_removed, report.jobs_pruned, report.refresh_tokens_purged, report.failures
        ),
    )
    .await;
    Ok(Json(maintenance_status(&state)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_reads_back_what_was_applied() {
        let mut settings = crate::tests_support::runtime_settings();
        for spec in SETTINGS {
            let value = match spec.kind {
                SettingKind::Boolean => json!(false),
                SettingKind::Integer => json!(spec.max),
                SettingKind::Choice => json!(spec.options.last().unwrap().0),
            };
            apply_setting(&mut settings, spec.key, &value).unwrap();
            assert_eq!(read_setting(&settings, spec.key), value, "{}", spec.key);
        }
        assert!(apply_setting(&mut settings, "login_max_failures", &json!(-1)).is_err());
        assert!(apply_setting(&mut settings, "blob_gc", &json!("yes")).is_err());
        assert!(apply_setting(&mut settings, "repo_poll_seconds", &json!(10)).is_err());
        assert!(apply_setting(&mut settings, "jwt_secret", &json!("x")).is_err());
        apply_setting(&mut settings, "repo_poll_seconds", &json!(0)).unwrap();
        assert_eq!(settings.repo_poll_interval, None);
        assert!(apply_setting(&mut settings, "log_http_level", &json!("loud")).is_err());
        assert!(apply_setting(&mut settings, "log_level", &json!("off")).is_err(), "the default level stays on");

        // Choosing levels here replaces a RUST_LOG filter from the environment.
        let mut settings = crate::tests_support::runtime_settings();
        settings.logging.env_filter = Some("debug".to_owned());
        apply_setting(&mut settings, "log_console_format", &json!("json")).unwrap();
        assert_eq!(settings.logging.env_filter.as_deref(), Some("debug"), "the format does not touch the filter");
        apply_setting(&mut settings, "log_http_level", &json!("debug")).unwrap();
        assert_eq!(settings.logging.env_filter, None);
        assert!(settings.logging.directives().contains("tower_http=debug"));
    }

    #[test]
    fn user_agents_are_summarised() {
        assert_eq!(
            describe_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36 Edg/130.0"),
            "Edge on Windows"
        );
        assert_eq!(describe_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) Gecko/20100101 Firefox/131.0"), "Firefox on macOS");
        assert_eq!(describe_agent("curl/8.4.0"), "curl");
        assert_eq!(describe_agent(""), "Unknown client");
    }

    #[test]
    fn telemetry_counts_requests_and_clients() {
        let telemetry = Telemetry::default();
        let sighting = || Some(("10.0.0.1".to_owned(), Some("http://127.0.0.1:3021".to_owned()), Some("web".to_owned()), "Chrome on Windows".to_owned()));
        telemetry.record(StatusCode::OK, sighting());
        telemetry.record(StatusCode::TOO_MANY_REQUESTS, sighting());
        telemetry.record(StatusCode::INTERNAL_SERVER_ERROR, None);
        let traffic = telemetry.traffic();
        assert_eq!((traffic.requests, traffic.client_errors, traffic.server_errors, traffic.rate_limited), (3, 1, 1, 1));
        assert_eq!(traffic.requests_last_hour, 3);
        assert_eq!(traffic.requests_per_minute_last_hour.len(), TRAFFIC_MINUTES);
        let clients = telemetry.clients();
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0].requests, 2);
    }
}
