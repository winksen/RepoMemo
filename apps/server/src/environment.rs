//! The platform host: which environment folder the server is attached to.
//!
//! All data of a server (database, blobs, logs, key file) lives in one
//! *environment* folder, and environments only exist directly inside
//! `workspace-data/` (git-ignored), so a new project can never end up tracked.
//! The server can start with no environment attached; it then answers only
//! the platform routes below, and an administrator with the **platform code**
//! (printed on the console, or `REPOMEMO_SETUP_CODE`) picks one in the web
//! app: create a new folder, or reroute to an existing one. Before anything
//! is attached the folder is verified: it must be empty, or already be a
//! RepoMemo environment this version can open.
//!
//! A system administrator can detach the environment from System settings:
//! every session is ended, background work stops, the database is closed and
//! the server goes back to waiting for the platform code.
//!
//! `REPOMEMO_SERVER_DATA_DIR` attaches at startup instead. It takes a folder
//! name (`main`) or `workspace-data/main`, and is verified the
//! same way; a value outside `workspace-data/` or a folder with the wrong
//! structure stops the server from starting.
//!
//! Two modes follow from that, so a choice never silently fails to stick:
//! - **Pinned** (the variable is set): every start attaches that environment.
//!   Detaching and choosing another one in the web app still works, but only
//!   until the next restart; the web app says so and names the fallback.
//! - **Managed** (the variable is not set): the environment chosen in the web
//!   app is written to `workspace-data/.last-environment` and attached again
//!   at the next start. Detaching forgets it, so the next start shows the menu.

use std::{
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{Arc, Weak},
    time::Duration,
};

use anyhow::{bail, Context, Result};
use axum::{
    extract::{ConnectInfo, FromRequestParts, Request, State},
    http::{request::Parts, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use repomemo_storage::{inspect_environment, EnvironmentState};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};
use tower::ServiceExt;

use crate::{
    build_state, cors_layer,
    security::{client_ip, security_headers, too_many_requests, wait_text, Guards},
    setup::{codes_match, display_code, generate_code, normalise_code},
    ApiError, AppState, ServerConfig,
};

/// The only folder environments may live in, relative to the working directory.
pub const ENVIRONMENTS_DIR: &str = "workspace-data";
/// In managed mode, the environment to attach again at startup. Its leading
/// dot keeps it out of the environment list (names cannot start with one).
const LAST_ENVIRONMENT_FILE: &str = ".last-environment";
const MAX_NAME_CHARS: usize = 48;
/// Wrong platform codes from one address before it must wait.
const MAX_CODE_FAILURES: u32 = 10;
const CODE_LOCKOUT: Duration = Duration::from_secs(15 * 60);

/// What a running attachment owns, so detaching can stop it.
struct Attachment {
    name: String,
    router: Router,
    state: AppState,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

pub(crate) struct Host {
    config: ServerConfig,
    /// `workspace-data` as an absolute path.
    root: PathBuf,
    /// The code that unlocks the platform menu, normalised.
    code: String,
    /// The environment `REPOMEMO_SERVER_DATA_DIR` names (pinned mode), or
    /// `None` when the web app's choice is remembered instead.
    pinned: Option<String>,
    guards: Guards,
    current: RwLock<Option<Attachment>>,
    /// One attach or detach at a time.
    switching: Mutex<()>,
}

/// The absolute `workspace-data` folder for the current working directory.
pub fn environments_root() -> Result<PathBuf> {
    Ok(std::env::current_dir()
        .context("the working directory cannot be read")?
        .join(ENVIRONMENTS_DIR))
}

/// Checks that `name` is a plain folder name that is safe on every platform.
pub fn validate_name(name: &str) -> Result<(), String> {
    let name_ok = !name.is_empty()
        && name.len() <= MAX_NAME_CHARS
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && name.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && !name.ends_with('.');
    if !name_ok {
        return Err(format!(
            "An environment name uses letters, digits, '-', '_' and '.', starts with a letter or digit, and has at most {MAX_NAME_CHARS} characters."
        ));
    }
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4 && stem.ends_with(|c: char| c.is_ascii_digit()));
    if reserved {
        return Err("This name is reserved by the operating system.".to_owned());
    }
    Ok(())
}

/// Turns the value of `REPOMEMO_SERVER_DATA_DIR` into an environment folder,
/// refusing anything that is not directly inside `workspace-data/`.
pub fn resolve_setting(root: &Path, value: &str) -> Result<PathBuf> {
    let value = value.trim();
    let path = Path::new(value);
    let name = if path.is_absolute() {
        let parent = path.parent().context("REPOMEMO_SERVER_DATA_DIR has no parent folder")?;
        if !same_folder(parent, root) {
            bail!("REPOMEMO_SERVER_DATA_DIR must be a folder directly inside {}", root.display());
        }
        path.file_name().and_then(|n| n.to_str()).map(str::to_owned)
    } else {
        let relative = path.strip_prefix(ENVIRONMENTS_DIR).unwrap_or(path);
        let mut parts = relative.components();
        match (parts.next(), parts.next()) {
            (Some(std::path::Component::Normal(part)), None) => part.to_str().map(str::to_owned),
            _ => None,
        }
    }
    .with_context(|| {
        format!("REPOMEMO_SERVER_DATA_DIR must name one folder inside {ENVIRONMENTS_DIR}/ (for example `{ENVIRONMENTS_DIR}/main`), not `{value}`")
    })?;
    validate_name(&name).map_err(anyhow::Error::msg).context("REPOMEMO_SERVER_DATA_DIR")?;
    Ok(root.join(name))
}

fn same_folder(left: &Path, right: &Path) -> bool {
    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

/// Refuses a folder that is a link or whose real location is outside `root`.
fn ensure_inside(root: &Path, dir: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err("Links are not allowed as environment folders.".to_owned());
        }
        Ok(_) => {
            let real = std::fs::canonicalize(dir).map_err(|error| error.to_string())?;
            let root = std::fs::canonicalize(root).map_err(|error| error.to_string())?;
            if real.parent() != Some(root.as_path()) {
                return Err(format!("Environments must be directly inside {ENVIRONMENTS_DIR}/."));
            }
        }
        Err(_) => {}
    }
    Ok(())
}

impl Host {
    /// Builds the host and, when the configuration names an environment,
    /// verifies and attaches it (the server does not start otherwise).
    pub(crate) async fn start(config: ServerConfig) -> Result<Arc<Self>> {
        let code = normalise_code(config.setup_code.as_deref().unwrap_or(&generate_code()));
        let pinned = config
            .data_dir
            .as_ref()
            .and_then(|dir| dir.file_name())
            .and_then(|name| name.to_str())
            .map(str::to_owned);
        let host = Arc::new(Self {
            root: environments_root()?,
            code,
            pinned,
            guards: Guards::default(),
            current: RwLock::new(None),
            switching: Mutex::new(()),
            config,
        });
        match host.config.data_dir.clone() {
            Some(dir) => {
                match inspect_environment(&dir).await {
                    EnvironmentState::Invalid(reason) => {
                        bail!("{} is not a usable RepoMemo environment: {reason}", dir.display());
                    }
                    EnvironmentState::Empty | EnvironmentState::Valid => {}
                }
                host.attach_dir(&dir).await?;
            }
            None => {
                if !host.reattach_remembered().await {
                    host.print_code_banner();
                }
            }
        }
        Ok(host)
    }

    /// Managed mode: attaches the environment last chosen in the web app, if
    /// it is still a RepoMemo environment. Anything else is forgotten and the
    /// menu is shown. Returns whether an environment was attached.
    async fn reattach_remembered(self: &Arc<Self>) -> bool {
        let Some(name) = read_last_environment(&self.root) else {
            return false;
        };
        let problem = match self.folder(&name) {
            Err(_) => Some("its name is not valid".to_owned()),
            Ok(dir) => match inspect_environment(&dir).await {
                EnvironmentState::Valid => match self.attach_dir(&dir).await {
                    Ok(()) => None,
                    Err(error) => Some(format!("{error:#}")),
                },
                EnvironmentState::Empty => Some("the folder no longer holds an environment".to_owned()),
                EnvironmentState::Invalid(reason) => Some(reason),
            },
        };
        match problem {
            None => {
                tracing::info!(target: "audit", environment = %name, "Attached the environment last chosen in the web app");
                // The code still opens a first-run setup that was never
                // finished, and the menu after a detach; print it as usual.
                if self.config.setup_code.is_none() {
                    eprintln!(
                        "\n================================================================\n  RepoMemo attached {ENVIRONMENTS_DIR}/{name}, the environment last\n  chosen in the web app. Code for its first-run setup, or for the\n  environment menu after detaching:\n\n      {}\n\n  It changes every time the server starts.\n================================================================\n",
                        display_code(&self.code)
                    );
                }
                true
            }
            Some(reason) => {
                tracing::warn!(target: "audit", environment = %name, %reason, "The environment last chosen in the web app cannot be attached; showing the environment menu");
                write_last_environment(&self.root, None);
                false
            }
        }
    }

    /// Managed mode only: remembers (or forgets) the web app's choice for the
    /// next start. A pinned server always starts with its variable.
    fn remember(&self, name: Option<&str>) {
        if self.pinned.is_none() {
            write_last_environment(&self.root, name);
        }
    }

    fn print_code_banner(&self) {
        if self.config.setup_code.is_some() {
            tracing::warn!(target: "audit", "No environment is attached: open the web app and enter the setup code from REPOMEMO_SETUP_CODE");
            return;
        }
        eprintln!(
            "\n================================================================\n  RepoMemo has no environment attached.\n  Open the web app, choose or create one in {ENVIRONMENTS_DIR}/\n  and enter this code when asked:\n\n      {}\n\n  It changes every time the server starts.\n================================================================\n",
            display_code(&self.code)
        );
        tracing::warn!(target: "audit", "No environment is attached: open the web app and enter the platform code printed on the console");
    }

    /// Opens storage in `dir` and starts serving it. `dir` must already be
    /// verified.
    async fn attach_dir(self: &Arc<Self>, dir: &Path) -> Result<()> {
        let _switch = self.switching.lock().await;
        if self.current.read().await.is_some() {
            bail!("An environment is already attached; detach it first.");
        }
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("environment").to_owned();
        let mut scoped = self.config.clone();
        scoped.data_dir = Some(dir.to_path_buf());
        // The platform code also opens the first-run onboarding of a new environment.
        scoped.setup_code.get_or_insert_with(|| self.code.clone());
        let (state, tasks) = build_state(&scoped, Arc::downgrade(self)).await?;
        let router = crate::routes(state.clone(), &scoped);
        tracing::info!(target: "audit", environment = %name, "Environment attached");
        *self.current.write().await = Some(Attachment { name, router, state, tasks });
        Ok(())
    }

    /// Ends every session, stops background work, closes the database and
    /// waits for the platform code again.
    pub(crate) async fn detach(&self) -> Result<(), ApiError> {
        let _switch = self.switching.lock().await;
        let state = {
            let current = self.current.read().await;
            let Some(attachment) = current.as_ref() else {
                return Err(ApiError::conflict("No environment is attached."));
            };
            attachment.state.clone()
        };
        state.storage.end_all_sessions().await.map_err(ApiError::internal)?;
        state.closing.store(true, std::sync::atomic::Ordering::SeqCst);
        let attachment = self.current.write().await.take();
        if let Some(attachment) = attachment {
            for task in &attachment.tasks {
                task.abort();
            }
            attachment.state.storage.close().await;
            tracing::info!(target: "audit", environment = %attachment.name, "Environment detached");
        }
        crate::logs::stop_file_output();
        self.remember(None);
        self.print_code_banner();
        Ok(())
    }

    /// The full application: platform routes first, everything else goes to
    /// the attached environment's router.
    pub(crate) fn router(self: &Arc<Self>) -> Router {
        let platform = Router::new()
            .route("/v1/platform", get(status))
            .route("/v1/platform/environments/list", post(list))
            .route("/v1/platform/environments/create", post(create))
            .route("/v1/platform/environments/attach", post(attach))
            .layer(axum::middleware::from_fn(security_headers))
            .layer(tower_http::trace::TraceLayer::new_for_http());
        Router::new()
            .merge(platform)
            .fallback(delegate)
            .layer(cors_layer(&self.config))
            .with_state(self.clone())
    }

    async fn name(&self) -> Option<String> {
        self.current.read().await.as_ref().map(|attachment| attachment.name.clone())
    }

    /// Counts a wrong code against the client address, or clears its record.
    fn check_code(&self, client: &str, code: &str) -> Result<(), ApiError> {
        let key = format!("platform:{client}");
        if let Some(wait) = self.guards.logins.locked_for(&key) {
            return Err(too_many_requests(
                format!("Too many wrong codes from this address. Try again in {}.", wait_text(wait)),
                wait,
            ));
        }
        if !codes_match(&self.code, code) {
            let locked = self.guards.logins.record_failure(&key, MAX_CODE_FAILURES, CODE_LOCKOUT);
            tracing::warn!(target: "audit", client, locked, "Wrong platform code");
            return Err(ApiError::forbidden_because(
                "The code is not correct. It is printed on the server console when the server starts.",
            ));
        }
        self.guards.logins.clear(&key);
        Ok(())
    }

    async fn require_detached(&self) -> Result<(), ApiError> {
        if self.current.read().await.is_some() {
            return Err(ApiError::conflict("An environment is attached already. Detach it in System settings first."));
        }
        Ok(())
    }

    /// The folder for `name`, after the name and location checks.
    fn folder(&self, name: &str) -> Result<PathBuf, ApiError> {
        validate_name(name).map_err(ApiError::bad_request)?;
        let dir = self.root.join(name);
        ensure_inside(&self.root, &dir).map_err(ApiError::bad_request)?;
        Ok(dir)
    }
}

/// The address a request came from, for lockouts.
struct Peer(String);

impl FromRequestParts<Arc<Host>> for Peer {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, host: &Arc<Host>) -> Result<Self, Self::Rejection> {
        let peer: Option<IpAddr> = parts.extensions.get::<ConnectInfo<SocketAddr>>().map(|info| info.0.ip());
        let headers: &HeaderMap = &parts.headers;
        Ok(Self(client_ip(headers, peer, host.config.trust_proxy)))
    }
}

#[derive(Serialize)]
struct PlatformStatus {
    attached: bool,
    environment: Option<String>,
    folder: &'static str,
    /// The environment every start attaches (`REPOMEMO_SERVER_DATA_DIR`).
    /// `None`: the choice made in the web app is remembered instead.
    pinned_environment: Option<String>,
}

async fn status(State(host): State<Arc<Host>>) -> Json<PlatformStatus> {
    let environment = host.name().await;
    Json(PlatformStatus {
        attached: environment.is_some(),
        environment,
        folder: ENVIRONMENTS_DIR,
        pinned_environment: host.pinned.clone(),
    })
}

/// The environment name remembered in managed mode, if any.
fn read_last_environment(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(LAST_ENVIRONMENT_FILE)).ok()?;
    let name = text.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// Writes or removes the remembered environment. A failure only means the
/// next start shows the menu, so it is logged rather than returned.
fn write_last_environment(root: &Path, name: Option<&str>) {
    let path = root.join(LAST_ENVIRONMENT_FILE);
    let result = match name {
        Some(name) => std::fs::create_dir_all(root).and_then(|()| std::fs::write(&path, name)),
        None => match std::fs::remove_file(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        },
    };
    if let Err(error) = result {
        tracing::warn!(target: "audit", path = %path.display(), %error, "The environment choice could not be saved for the next start");
    }
}

#[derive(Deserialize)]
struct CodeRequest {
    code: String,
}

#[derive(Serialize)]
struct EnvironmentEntry {
    name: String,
    /// `valid`, `empty` or `invalid`.
    status: &'static str,
    detail: String,
}

async fn describe(dir: &Path, name: String) -> EnvironmentEntry {
    let (status, detail) = match inspect_environment(dir).await {
        EnvironmentState::Valid => ("valid", "A RepoMemo environment that can be attached.".to_owned()),
        EnvironmentState::Empty => ("empty", "Empty: a new environment will be started here.".to_owned()),
        EnvironmentState::Invalid(reason) => ("invalid", reason),
    };
    EnvironmentEntry { name, status, detail }
}

/// Folders of `workspace-data/` with their verification, after the code.
async fn list(
    State(host): State<Arc<Host>>,
    Peer(client): Peer,
    Json(request): Json<CodeRequest>,
) -> Result<Json<Vec<EnvironmentEntry>>, ApiError> {
    host.check_code(&client, &request.code)?;
    host.require_detached().await?;
    let mut entries = Vec::new();
    if let Ok(mut folders) = tokio::fs::read_dir(&host.root).await {
        while let Ok(Some(folder)) = folders.next_entry().await {
            let Some(name) = folder.file_name().to_str().map(str::to_owned) else { continue };
            let is_folder = folder.file_type().await.is_ok_and(|kind| kind.is_dir());
            if !is_folder || validate_name(&name).is_err() {
                continue;
            }
            entries.push(describe(&folder.path(), name).await);
        }
    }
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(Json(entries))
}

#[derive(Deserialize)]
struct EnvironmentRequest {
    code: String,
    name: String,
}

/// Creates `workspace-data/<name>` and attaches it.
async fn create(
    State(host): State<Arc<Host>>,
    Peer(client): Peer,
    Json(request): Json<EnvironmentRequest>,
) -> Result<(StatusCode, Json<PlatformStatus>), ApiError> {
    host.check_code(&client, &request.code)?;
    host.require_detached().await?;
    let name = request.name.trim().to_owned();
    let dir = host.folder(&name)?;
    if dir.exists() {
        return Err(ApiError::conflict("A folder with this name exists already. Choose it from the list instead."));
    }
    tokio::fs::create_dir_all(&dir).await.map_err(ApiError::internal)?;
    if let Err(error) = host.attach_dir(&dir).await {
        let _ = tokio::fs::remove_dir(&dir).await;
        return Err(ApiError::internal(format!("{error:#}")));
    }
    host.remember(Some(&name));
    Ok((StatusCode::CREATED, status(State(host)).await))
}

/// Verifies an existing folder of `workspace-data/` and attaches it.
async fn attach(
    State(host): State<Arc<Host>>,
    Peer(client): Peer,
    Json(request): Json<EnvironmentRequest>,
) -> Result<Json<PlatformStatus>, ApiError> {
    host.check_code(&client, &request.code)?;
    host.require_detached().await?;
    let name = request.name.trim().to_owned();
    let dir = host.folder(&name)?;
    match inspect_environment(&dir).await {
        EnvironmentState::Invalid(reason) => {
            return Err(ApiError::bad_request(format!("This folder cannot be attached: {reason}")));
        }
        EnvironmentState::Empty if !dir.exists() => {
            return Err(ApiError::not_found("This environment does not exist. Create it instead."));
        }
        EnvironmentState::Empty | EnvironmentState::Valid => {}
    }
    host.attach_dir(&dir).await.map_err(|error| ApiError::internal(format!("{error:#}")))?;
    host.remember(Some(&name));
    Ok(status(State(host)).await)
}

/// Everything else: the attached environment answers, or none is attached.
async fn delegate(State(host): State<Arc<Host>>, request: Request) -> Response {
    let router = host.current.read().await.as_ref().map(|attachment| attachment.router.clone());
    match router {
        Some(router) => match router.oneshot(request).await {
            Ok(response) => response,
            Err(never) => match never {},
        },
        None => unattached(&host, request.uri().path()),
    }
}

fn unattached(host: &Host, path: &str) -> Response {
    match path {
        "/health" => Json(serde_json::json!({
            "service": host.config.service_name,
            "status": "ok",
            "authentication": "jwt",
            "registration_open": false,
            "setup_required": false,
            "environment_required": true,
        }))
        .into_response(),
        "/health/ready" => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "status": "unavailable", "database": "detached" })),
        )
            .into_response(),
        _ => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "environment_required",
            "No environment is attached. An administrator must choose one first.",
        )
        .into_response(),
    }
}

/// `Weak` handle the attached application keeps to ask for its own detaching.
pub(crate) type HostHandle = Weak<Host>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_plain_and_safe() {
        for good in ["main", "onboarding", "client-a_2", "v1.2"] {
            assert!(validate_name(good).is_ok(), "{good}");
        }
        for bad in ["", ".hidden", "-x", "a/b", "a\\b", "..", "con", "COM1", "nul.txt", "trailing.", "sp ace", &"x".repeat(49)] {
            assert!(validate_name(bad).is_err(), "{bad}");
        }
    }

    async fn call(app: &Router, method: &str, path: &str, body: Option<&str>) -> (StatusCode, serde_json::Value) {
        let request = axum::http::Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.unwrap_or("").to_owned()))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
    }

    #[tokio::test]
    async fn a_detached_server_only_offers_the_platform_menu_behind_its_code() {
        let mut config = ServerConfig::for_test(std::env::temp_dir());
        config.data_dir = None;
        config.setup_code = Some("ABCD-EFGH-JKLM".to_owned());
        let app = crate::router(config).await.unwrap();

        let (status, body) = call(&app, "GET", "/v1/platform", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["attached"], false);
        let (status, body) = call(&app, "GET", "/health", None).await;
        assert_eq!((status, body["environment_required"].clone()), (StatusCode::OK, serde_json::json!(true)));
        let (status, body) = call(&app, "GET", "/v1/workspaces", None).await;
        assert_eq!((status, body["error"]["code"].clone()), (StatusCode::SERVICE_UNAVAILABLE, serde_json::json!("environment_required")));

        let (status, _) = call(&app, "POST", "/v1/platform/environments/list", Some(r#"{"code":"WRONG-CODE-0000"}"#)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = call(&app, "POST", "/v1/platform/environments/list", Some(r#"{"code":"abcd-efgh-jklm"}"#)).await;
        assert_eq!(status, StatusCode::OK);
        for name in ["../escape", "a/b", ".hidden"] {
            let body = serde_json::json!({ "code": "ABCD-EFGH-JKLM", "name": name }).to_string();
            let (status, _) = call(&app, "POST", "/v1/platform/environments/create", Some(&body)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{name}");
        }
    }

    #[tokio::test]
    async fn an_attached_server_reports_its_environment_and_refuses_to_attach_again() {
        let dir = std::env::temp_dir().join(format!("repomemo-host-{}", uuid::Uuid::new_v4()));
        let mut config = ServerConfig::for_test(dir.clone());
        config.setup_code = Some("ABCD-EFGH-JKLM".to_owned());
        let app = crate::router(config).await.unwrap();
        let (_, body) = call(&app, "GET", "/v1/platform", None).await;
        assert_eq!(body["attached"], true);
        let (status, _) = call(&app, "GET", "/health", None).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = call(&app, "POST", "/v1/platform/environments/attach", Some(r#"{"code":"ABCD-EFGH-JKLM","name":"other"}"#)).await;
        assert_eq!(status, StatusCode::CONFLICT);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_web_app_choice_is_remembered_and_forgotten() {
        let base = std::env::temp_dir().join(format!("repomemo-last-{}", uuid::Uuid::new_v4()));
        let root = base.join(ENVIRONMENTS_DIR);
        assert_eq!(read_last_environment(&root), None, "nothing remembered yet");
        write_last_environment(&root, Some("server"));
        assert_eq!(read_last_environment(&root).as_deref(), Some("server"));
        write_last_environment(&root, None);
        assert_eq!(read_last_environment(&root), None);
        write_last_environment(&root, None);
        assert!(validate_name(LAST_ENVIRONMENT_FILE).is_err(), "the file is never listed as an environment");
        let _ = std::fs::remove_dir_all(base);
    }

    #[tokio::test]
    async fn a_pinned_server_reports_the_environment_it_starts_with() {
        let dir = std::env::temp_dir().join(format!("repomemo-pinned-{}", uuid::Uuid::new_v4()));
        let app = crate::router(ServerConfig::for_test(dir.clone())).await.unwrap();
        let (_, body) = call(&app, "GET", "/v1/platform", None).await;
        assert_eq!(body["pinned_environment"], serde_json::json!(dir.file_name().unwrap().to_str().unwrap()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_setting_must_name_one_folder_inside_the_root() {
        let root = std::env::temp_dir().join("repomemo-root-test").join(ENVIRONMENTS_DIR);
        assert_eq!(resolve_setting(&root, "main").unwrap(), root.join("main"));
        assert_eq!(resolve_setting(&root, "workspace-data/main").unwrap(), root.join("main"));
        for bad in ["..", "../main", "workspace-data/a/b", ".repomemo-server", "/etc", "workspace-data", ""] {
            assert!(resolve_setting(&root, bad).is_err(), "{bad}");
        }
    }
}
