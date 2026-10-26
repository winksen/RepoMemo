//! Which server the desktop app talks to, and where its session is kept (V0.2.49).
//!
//! The window always shows the bundled web client. That client reaches the server through the
//! HTTP plugin, and the plugin may only call the server the person chose: its address is added to
//! the window's permissions at run time (nothing is allowed before one is chosen). The refresh
//! token of the session is kept in the operating system's credential store, never in a file or in
//! the page's storage. The window itself never leaves the bundled client (see [`is_app_url`]).

use std::{
    fs,
    net::Ipv4Addr,
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{ipc::CapabilityBuilder, AppHandle, Manager, State};
use url::{Host, Url};

use crate::config::ConfigState;

const KEYRING_SERVICE: &str = "RepoMemo";
const SERVER_FILE: &str = "server.json";

/// The server this app was packaged for (V0.4.46), fixed when it was compiled: `REPOMEMO_PRESET_SERVER=https://memo.example.com
/// npm run tauri:build`. With one, the app never asks for an address, ignores the one saved by an earlier run and cannot be
/// pointed anywhere else.
const PRESET_SERVER: Option<&str> = option_env!("REPOMEMO_PRESET_SERVER");

/// The server this run talks to: its origin (`https://host:port`), set at start from the preset, the saved
/// address or by [`prepare_server`]. Locked when the app was packaged for a server.
#[derive(Default)]
pub struct Connection {
    current: Mutex<Option<String>>,
    locked: AtomicBool,
}

/// The preset server as an origin, `None` when the app was packaged without one. A preset that is not a usable address is an
/// error, checked by the same rules as a typed one, so a mistake stops the app at start instead of showing a blank screen.
pub fn resolve_preset(value: Option<&str>) -> Result<Option<String>, String> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some(text) => normalize_server_url(text).map(Some).map_err(|reason| {
            format!("The server address this app was packaged with ({text}) cannot be used. {reason}")
        }),
    }
}

/// Refuses a change of server in an app that was packaged for one.
fn ensure_unlocked(connection: &Connection) -> Result<(), String> {
    if connection.locked.load(Ordering::SeqCst) {
        let origin = current_origin(connection).unwrap_or_default();
        return Err(format!(
            "This app is set up for {origin}; its server cannot be changed."
        ));
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct SavedServer {
    server_url: String,
}

/// Turns what a person typed into the origin of a server, or says why it cannot be one.
///
/// `https` is assumed when no scheme is typed. Plain `http` is only accepted for a machine on the
/// person's own network (loopback, private and link-local addresses, `localhost`, `.local` and
/// single-word names), because a session sent over `http` across the internet can be read.
/// Addresses with a login, a query, a fragment or a path are refused, since the client calls
/// `/v1/...` on the server's root.
pub fn normalize_server_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Enter the address of your RepoMemo server.".to_owned());
    }
    let with_scheme = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    let url = Url::parse(&with_scheme).map_err(|_| "That is not a web address.".to_owned())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("The address must start with https:// (or http:// on your own network).".to_owned());
    }
    let Some(host) = url.host() else {
        return Err("The address has no host name.".to_owned());
    };
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Leave the user name and password out of the address; you sign in on the next screen.".to_owned());
    }
    if url.query().is_some() || url.fragment().is_some() || !matches!(url.path(), "" | "/") {
        return Err("Enter only the server's address, such as https://repomemo.example.com.".to_owned());
    }
    if url.scheme() == "http" && !is_local_host(&host) {
        return Err(
            "A server on the internet must use https://. Plain http:// is only for a server on your own network."
                .to_owned(),
        );
    }
    Ok(url.origin().ascii_serialization())
}

fn is_local_host(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(name) => {
            let name = name.to_ascii_lowercase();
            name == "localhost"
                || !name.contains('.')
                || [".localhost", ".local", ".lan", ".internal", ".home.arpa"]
                    .iter()
                    .any(|suffix| name.ends_with(suffix))
        }
        Host::Ipv4(address) => is_private_v4(*address),
        Host::Ipv6(address) => {
            let first = address.segments()[0];
            address.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
    }
}

fn is_private_v4(address: Ipv4Addr) -> bool {
    address.is_loopback() || address.is_private() || address.is_link_local()
}

/// Whether the window may show this address. The bundled client and the development server are
/// the only pages that may ever be in it, so a link, a redirect or a script cannot take the window
/// (and with it the session) to another site.
pub fn is_app_url(url: &Url) -> bool {
    match url.scheme() {
        "tauri" | "asset" | "ipc" | "blob" | "data" | "about" => true,
        "http" | "https" => {
            matches!(
                url.host_str(),
                Some("tauri.localhost" | "ipc.localhost" | "asset.localhost")
            ) || (cfg!(debug_assertions)
                && matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
                && url.port() == Some(3021))
        }
        _ => false,
    }
}

fn saved_server_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app.path().app_config_dir().map_err(|error| error.to_string())?;
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    Ok(directory.join(SERVER_FILE))
}

/// The address saved by an earlier run, if it is still a usable one.
pub fn read_saved_server(app: &AppHandle) -> Option<String> {
    let text = fs::read_to_string(saved_server_path(app).ok()?).ok()?;
    let saved: SavedServer = serde_json::from_str(&text).ok()?;
    normalize_server_url(&saved.server_url).ok()
}

/// Lets the window's pages call this server, and no other.
pub fn allow_server(app: &AppHandle, origin: &str) -> Result<(), String> {
    let identifier = format!("server-{}", origin.replace(|c: char| !c.is_ascii_alphanumeric(), "-"));
    let capability = CapabilityBuilder::new(identifier).window("main").permission_scoped(
        "http:default",
        vec![json!({ "url": format!("{origin}/*") })],
        Vec::<serde_json::Value>::new(),
    );
    app.add_capability(capability).map_err(|error| error.to_string())
}

/// The server a run starts on and whether it is locked, from the three places that can name one, strongest first: the address the
/// app was packaged with (V0.4.46), a locked server in the configuration file (V0.4.47), and the address saved by an earlier run.
/// A configuration file with a problem starts nothing: the app says what is wrong instead of asking for an address that would go
/// around it.
pub fn choose_start(preset: Option<String>, file: &ConfigState, saved: Option<String>) -> (Option<String>, bool) {
    if let Some(origin) = preset {
        return (Some(origin), true);
    }
    if file.problem.is_some() {
        return (None, false);
    }
    match (&file.config.server, file.config.locked) {
        (Some(origin), true) => (Some(origin.clone()), true),
        _ => (saved, false),
    }
}

/// Starts a run on the server [`choose_start`] picks, if there is one.
pub fn start(app: &AppHandle, connection: &Connection, file: &ConfigState) -> Result<(), String> {
    let preset = resolve_preset(PRESET_SERVER)?;
    let saved = if preset.is_some() || file.problem.is_some() {
        None
    } else {
        read_saved_server(app)
    };
    let (origin, locked) = choose_start(preset, file, saved);
    if let Some(origin) = origin {
        allow_server(app, &origin)?;
        *connection.current.lock().map_err(|_| "state is locked".to_owned())? = Some(origin);
    }
    connection.locked.store(locked, Ordering::SeqCst);
    Ok(())
}

/// The flag with which the uninstaller (V0.4.50) asks the program to forget what it keeps for a person and leave, without a window.
pub const FORGET_FLAG: &str = "--forget-session";

/// Whether the program was started to forget the saved session.
pub fn asked_to_forget<I: IntoIterator<Item = String>>(arguments: I) -> bool {
    arguments.into_iter().skip(1).any(|argument| argument == FORGET_FLAG)
}

/// Every server this computer may hold a session for, once each: the packaged address, the configuration file's, the saved one.
pub fn servers_to_forget(preset: Option<String>, file: &ConfigState, saved: Option<String>) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for origin in [preset, file.config.server.clone(), saved].into_iter().flatten() {
        if !found.contains(&origin) {
            found.push(origin);
        }
    }
    found
}

/// Forgets the sessions in the credential store and the saved server, and says how many sessions there were to forget. The
/// administrator's configuration file is not touched.
pub fn forget_saved(app: &AppHandle, file: &ConfigState) -> usize {
    let preset = resolve_preset(PRESET_SERVER).ok().flatten();
    let origins = servers_to_forget(preset, file, read_saved_server(app));
    let mut forgotten = 0;
    for origin in &origins {
        if let Ok(entry) = session_entry(origin) {
            if entry.delete_credential().is_ok() {
                forgotten += 1;
            }
        }
    }
    if let Ok(path) = saved_server_path(app) {
        let _ = fs::remove_file(path);
    }
    forgotten
}

fn current_origin(connection: &Connection) -> Result<String, String> {
    connection
        .current
        .lock()
        .map_err(|_| "state is locked".to_owned())?
        .clone()
        .ok_or_else(|| "No server is chosen yet.".to_owned())
}

fn session_entry(origin: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, origin).map_err(|error| error.to_string())
}

/// The address of the server this run talks to, if one is chosen.
#[tauri::command]
pub fn current_server(connection: State<'_, Connection>) -> Option<String> {
    connection.current.lock().ok().and_then(|current| current.clone())
}

/// Whether the app was packaged for one server, which then cannot be changed.
#[tauri::command]
pub fn server_locked(connection: State<'_, Connection>) -> bool {
    connection.locked.load(Ordering::SeqCst)
}

/// Checks an address a person typed and lets the window call that server. Nothing is saved until
/// [`remember_server`], after the page has found a RepoMemo server there.
#[tauri::command]
pub fn prepare_server(app: AppHandle, connection: State<'_, Connection>, address: String) -> Result<String, String> {
    ensure_unlocked(&connection)?;
    let origin = normalize_server_url(&address)?;
    allow_server(&app, &origin)?;
    *connection.current.lock().map_err(|_| "state is locked".to_owned())? = Some(origin.clone());
    Ok(origin)
}

/// Keeps the chosen server for the next start.
#[tauri::command]
pub fn remember_server(app: AppHandle, connection: State<'_, Connection>) -> Result<(), String> {
    ensure_unlocked(&connection)?;
    let origin = current_origin(&connection)?;
    let text = serde_json::to_string_pretty(&SavedServer { server_url: origin }).map_err(|error| error.to_string())?;
    fs::write(saved_server_path(&app)?, text).map_err(|error| error.to_string())
}

/// Forgets the server and its session and starts again, so the next run asks for an address.
#[tauri::command]
pub fn forget_server(app: AppHandle, connection: State<'_, Connection>) -> Result<(), String> {
    ensure_unlocked(&connection)?;
    if let Ok(origin) = current_origin(&connection) {
        if let Ok(entry) = session_entry(&origin) {
            // Nothing stored is fine.
            let _ = entry.delete_credential();
        }
    }
    if let Ok(path) = saved_server_path(&app) {
        let _ = fs::remove_file(path);
    }
    app.restart()
}

/// Keeps the session's refresh token in the operating system's credential store.
#[tauri::command]
pub fn save_session(connection: State<'_, Connection>, refresh_token: String) -> Result<(), String> {
    if refresh_token.is_empty() {
        return Ok(());
    }
    session_entry(&current_origin(&connection)?)?
        .set_password(&refresh_token)
        .map_err(|error| error.to_string())
}

/// The refresh token kept for the chosen server, if any.
#[tauri::command]
pub fn load_session(connection: State<'_, Connection>) -> Result<Option<String>, String> {
    match session_entry(&current_origin(&connection)?)?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

/// Forgets the refresh token kept for the chosen server.
#[tauri::command]
pub fn clear_session(connection: State<'_, Connection>) -> Result<(), String> {
    match session_entry(&current_origin(&connection)?)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

/// Opens a web page in the system browser. The window cannot go to other sites itself, so links
/// to the outside come here; only `http` and `https` are opened.
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    let parsed = Url::parse(&url).map_err(|_| "That is not a web address.".to_owned())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Only web pages are opened.".to_owned());
    }
    let address = parsed.as_str();
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("rundll32");
        command.args(["url.dll,FileProtocolHandler", address]);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(address);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(address);
        command
    };
    command.spawn().map(|_| ()).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_host_gets_https_and_loses_the_slash() {
        assert_eq!(
            normalize_server_url("repomemo.example.com").unwrap(),
            "https://repomemo.example.com"
        );
        assert_eq!(
            normalize_server_url("  https://repomemo.example.com/ ").unwrap(),
            "https://repomemo.example.com"
        );
        assert_eq!(
            normalize_server_url("https://Repomemo.Example.com:8443").unwrap(),
            "https://repomemo.example.com:8443"
        );
        assert_eq!(
            normalize_server_url("https://repomemo.example.com:443").unwrap(),
            "https://repomemo.example.com"
        );
    }

    #[test]
    fn plain_http_is_only_for_the_own_network() {
        for local in [
            "http://localhost:3020",
            "http://127.0.0.1:3020",
            "http://192.168.1.20",
            "http://10.0.0.5:3020",
            "http://repomemo",
            "http://nas.local",
            "http://[::1]:3020",
        ] {
            assert!(normalize_server_url(local).is_ok(), "{local} should be accepted");
        }
        for public in [
            "http://repomemo.example.com",
            "http://8.8.8.8",
            "http://203.0.113.9:3020",
        ] {
            assert!(
                normalize_server_url(public).unwrap_err().contains("https"),
                "{public} should be refused"
            );
        }
    }

    #[test]
    fn what_is_not_a_server_address_is_refused() {
        for bad in [
            "",
            "   ",
            "ftp://example.com",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://user:secret@example.com",
            "https://example.com/app",
            "https://example.com/?a=1",
            "https://example.com/#x",
            "https://",
        ] {
            assert!(normalize_server_url(bad).is_err(), "{bad:?} should be refused");
        }
    }

    #[test]
    fn an_app_packaged_without_a_server_has_no_preset() {
        assert_eq!(resolve_preset(None).unwrap(), None);
        assert_eq!(resolve_preset(Some("")).unwrap(), None);
        assert_eq!(resolve_preset(Some("   ")).unwrap(), None);
    }

    #[test]
    fn a_preset_is_checked_by_the_rules_of_a_typed_address() {
        assert_eq!(
            resolve_preset(Some(" memo.example.com ")).unwrap().as_deref(),
            Some("https://memo.example.com")
        );
        assert_eq!(
            resolve_preset(Some("http://192.168.1.20:3020")).unwrap().as_deref(),
            Some("http://192.168.1.20:3020")
        );
        for bad in [
            "http://memo.example.com",
            "https://memo.example.com/app",
            "ftp://x",
            "https://u:p@x.com",
        ] {
            let message = resolve_preset(Some(bad)).unwrap_err();
            assert!(message.contains(bad), "{message} should name {bad}");
            assert!(
                message.contains("packaged"),
                "{message} should say where the address came from"
            );
        }
    }

    fn file_with(server: Option<&str>, locked: bool, problem: Option<&str>) -> ConfigState {
        ConfigState {
            source: None,
            config: crate::config::DesktopConfig {
                server: server.map(str::to_owned),
                locked,
                ..Default::default()
            },
            problem: problem.map(str::to_owned),
        }
    }

    #[test]
    fn the_strongest_place_that_names_a_server_decides() {
        let (preset, file_server, saved) = (
            "https://preset.example.com",
            "https://file.example.com",
            "https://saved.example.com",
        );
        let own = |text: &str| Some(text.to_owned());
        // The address the app was packaged with beats everything.
        assert_eq!(
            choose_start(own(preset), &file_with(Some(file_server), true, None), own(saved)),
            (own(preset), true)
        );
        // A locked server in the file beats what a person typed before.
        assert_eq!(
            choose_start(None, &file_with(Some(file_server), true, None), own(saved)),
            (own(file_server), true)
        );
        // An unlocked one is only offered: the saved address, or none, is used.
        assert_eq!(
            choose_start(None, &file_with(Some(file_server), false, None), own(saved)),
            (own(saved), false)
        );
        assert_eq!(
            choose_start(None, &file_with(Some(file_server), false, None), None),
            (None, false)
        );
        // No file at all: as before.
        assert_eq!(
            choose_start(None, &ConfigState::default(), own(saved)),
            (own(saved), false)
        );
        assert_eq!(choose_start(None, &ConfigState::default(), None), (None, false));
    }

    #[test]
    fn the_uninstaller_asks_with_one_flag_and_only_that_flag() {
        let args = |list: &[&str]| list.iter().map(|text| (*text).to_owned()).collect::<Vec<_>>();
        assert!(asked_to_forget(args(&["RepoMemo.exe", "--forget-session"])));
        assert!(!asked_to_forget(args(&["RepoMemo.exe"])));
        assert!(!asked_to_forget(args(&[
            "RepoMemo.exe",
            "--forget-sessions",
            "forget-session"
        ])));
        // The program's own name is never taken for the flag.
        assert!(!asked_to_forget(args(&["--forget-session"])));
    }

    #[test]
    fn every_server_with_a_session_is_forgotten_once() {
        let file = file_with(Some("https://file.example.com"), true, None);
        let own = |text: &str| Some(text.to_owned());
        assert_eq!(
            servers_to_forget(
                own("https://preset.example.com"),
                &file,
                own("https://file.example.com")
            ),
            vec![
                "https://preset.example.com".to_owned(),
                "https://file.example.com".to_owned()
            ]
        );
        assert!(servers_to_forget(None, &ConfigState::default(), None).is_empty());
        assert_eq!(
            servers_to_forget(None, &ConfigState::default(), own("https://saved.example.com")).len(),
            1
        );
    }

    #[test]
    fn a_file_with_a_problem_starts_nothing_and_leaves_no_way_round_it() {
        let broken = file_with(None, false, Some("repomemo-desktop.json: server: nope"));
        assert_eq!(
            choose_start(None, &broken, Some("https://saved.example.com".to_owned())),
            (None, false)
        );
    }

    #[test]
    fn a_locked_connection_refuses_a_change_and_an_open_one_allows_it() {
        let connection = Connection::default();
        assert!(ensure_unlocked(&connection).is_ok());
        *connection.current.lock().unwrap() = Some("https://memo.example.com".to_owned());
        connection.locked.store(true, Ordering::SeqCst);
        let message = ensure_unlocked(&connection).unwrap_err();
        assert!(message.contains("https://memo.example.com") && message.contains("cannot be changed"));
    }

    #[test]
    fn the_window_only_shows_the_bundled_client() {
        for address in [
            "tauri://localhost/",
            "http://tauri.localhost/",
            "https://tauri.localhost/index.html",
            "blob:http://tauri.localhost/abc",
        ] {
            assert!(is_app_url(&Url::parse(address).unwrap()), "{address} should be allowed");
        }
        for address in [
            "https://example.com/",
            "http://127.0.0.1:3020/",
            "https://tauri.localhost.evil.com/",
            "ms-word:ofe|u|https://x",
            "ftp://tauri.localhost/",
        ] {
            assert!(
                !is_app_url(&Url::parse(address).unwrap()),
                "{address} should be refused"
            );
        }
    }

    #[test]
    fn only_web_pages_are_opened_outside() {
        assert!(open_external("file:///C:/Windows/System32/calc.exe".to_owned()).is_err());
        assert!(open_external("ms-word:ofe|u|https://x".to_owned()).is_err());
        assert!(open_external("not a url".to_owned()).is_err());
    }
}
