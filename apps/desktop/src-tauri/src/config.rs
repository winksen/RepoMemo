//! The app's configuration file (V0.4.47): `repomemo-desktop.json`, beside the executable or in the app's config folder, in which an
//! administrator says which server the app talks to, what it is called, whether the person can change the server, and where to
//! ask for help. The same executable with two files talks to two servers.
//!
//! The server is checked by the rules of a typed address ([`normalize_server_url`]). A file that cannot be read, is not valid JSON,
//! names a field the app does not know or holds a value that fails its rule is **named and not obeyed**: nothing in it is used,
//! and the app says which file and why instead of falling back to asking for an address, which would let a person go around it.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use url::Url;

use crate::connection::normalize_server_url;

/// The file's name, in either place.
pub const CONFIG_FILE: &str = "repomemo-desktop.json";
const MAX_NAME_CHARS: usize = 60;
const MAX_FILE_BYTES: u64 = 16 * 1024;
const MAX_PATH_CHARS: usize = 260;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    server: Option<String>,
    name: Option<String>,
    locked: Option<bool>,
    support: Option<String>,
    certificate: Option<String>,
}

/// What a valid file says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesktopConfig {
    /// The server's origin.
    pub server: Option<String>,
    /// What the app is called (the window's title and the page's).
    pub name: Option<String>,
    /// With a server: the person cannot choose another (the default). Without: the server is only suggested.
    pub locked: bool,
    /// An `http(s)` address where a person asks for help.
    pub support_url: Option<String>,
    /// A PEM bundle of the authority that signed the server's certificate (V0.4.55), as written: a name beside this file, or a path.
    pub certificate: Option<String>,
}

/// The file that was found, what it says, or why it cannot be used. Fixed at start.
#[derive(Debug, Default)]
pub struct ConfigState {
    pub source: Option<PathBuf>,
    pub config: DesktopConfig,
    /// The certificates read from [`DesktopConfig::certificate`], trusted for the file's server and for no other address.
    pub certificates: Vec<reqwest::Certificate>,
    pub problem: Option<String>,
}

/// What the page is told.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettings {
    pub name: Option<String>,
    pub support_url: Option<String>,
    /// A server to offer on the connect screen, when the file names one without locking it.
    pub suggested_server: Option<String>,
    pub source: Option<String>,
    pub problem: Option<String>,
}

/// Reads the text of a file; the first thing wrong with it ends the reading.
pub fn parse_config(text: &str) -> Result<DesktopConfig, String> {
    let raw: RawConfig =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|error| error.to_string())?;
    let server = match raw.server.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(address) => Some(normalize_server_url(address).map_err(|reason| format!("server: {reason}"))?),
    };
    let name = match raw
        .name
        .as_deref()
        .map(|name| name.split_whitespace().collect::<Vec<_>>().join(" "))
    {
        None => None,
        Some(name)
            if !name.is_empty() && name.chars().count() <= MAX_NAME_CHARS && !name.chars().any(char::is_control) =>
        {
            Some(name)
        }
        Some(_) => return Err(format!("name: must be one line of 1 to {MAX_NAME_CHARS} characters")),
    };
    let support_url = match raw.support.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(address) => Some(check_support_url(address)?),
    };
    if raw.locked.is_some() && server.is_none() {
        return Err("locked: there is no server to lock; add a \"server\"".to_owned());
    }
    let certificate = match raw.certificate.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(file) if file.chars().count() <= MAX_PATH_CHARS && !file.chars().any(char::is_control) => {
            Some(file.to_owned())
        }
        Some(_) => {
            return Err(format!(
                "certificate: must be the name of a file, up to {MAX_PATH_CHARS} characters"
            ))
        }
    };
    if certificate.is_some() && server.is_none() {
        return Err("certificate: it is trusted for a server; add a \"server\"".to_owned());
    }
    Ok(DesktopConfig {
        locked: server.is_some() && raw.locked.unwrap_or(true),
        server,
        name,
        support_url,
        certificate,
    })
}

/// The certificates the file names: read from beside the file (or from the absolute path given), whole or not at all.
pub fn read_certificates(config_file: &Path, config: &DesktopConfig) -> Result<Vec<reqwest::Certificate>, String> {
    let Some(name) = &config.certificate else {
        return Ok(Vec::new());
    };
    let path = config_file
        .parent()
        .map(|folder| folder.join(name))
        .unwrap_or_else(|| PathBuf::from(name));
    let size = fs::metadata(&path)
        .map_err(|error| format!("certificate: {} cannot be read ({error})", path.display()))?
        .len();
    if size > crate::trust::MAX_BUNDLE_BYTES {
        return Err(format!(
            "certificate: {} is {size} bytes; a bundle of authorities is a few kilobytes",
            path.display()
        ));
    }
    let pem = fs::read(&path).map_err(|error| format!("certificate: {} cannot be read ({error})", path.display()))?;
    crate::trust::parse_bundle(&pem).map_err(|reason| format!("certificate: {}: {reason}", path.display()))
}

fn check_support_url(address: &str) -> Result<String, String> {
    let message = "support: must be an http:// or https:// address without a login".to_owned();
    let url = Url::parse(address).map_err(|_| message.clone())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(message);
    }
    Ok(url.to_string())
}

/// The first file that exists, in the order given.
pub fn find_config(directories: &[PathBuf]) -> Option<PathBuf> {
    directories
        .iter()
        .map(|directory| directory.join(CONFIG_FILE))
        .find(|path| path.is_file())
}

/// Reads and checks the file at a path.
pub fn read_config(path: &Path) -> Result<DesktopConfig, String> {
    let size = fs::metadata(path).map_err(|error| error.to_string())?.len();
    if size > MAX_FILE_BYTES {
        return Err(format!("the file is {size} bytes; a configuration file is a few lines"));
    }
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    parse_config(&text)
}

/// Looks beside the executable first, then in the app's config folder.
pub fn load(app: &AppHandle) -> ConfigState {
    let mut directories = Vec::new();
    if let Some(directory) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        directories.push(directory);
    }
    if let Ok(directory) = app.path().app_config_dir() {
        directories.push(directory);
    }
    load_from(&directories)
}

/// The file in the first of these folders that has one, with the certificates it names; a file that is wrong in any way, including a
/// certificate that cannot be read, is a problem and nothing in it is kept.
pub fn load_from(directories: &[PathBuf]) -> ConfigState {
    let Some(path) = find_config(directories) else {
        return ConfigState::default();
    };
    match read_config(&path)
        .and_then(|config| read_certificates(&path, &config).map(|certificates| (config, certificates)))
    {
        Ok((config, certificates)) => ConfigState {
            source: Some(path),
            config,
            certificates,
            problem: None,
        },
        Err(reason) => {
            let problem = format!("{}: {reason}", path.display());
            ConfigState {
                source: Some(path),
                problem: Some(problem),
                ..ConfigState::default()
            }
        }
    }
}

/// What the page needs: the name, the help link, a suggested server, and the problem with the file, if any.
#[tauri::command]
pub fn desktop_settings(state: State<'_, ConfigState>) -> DesktopSettings {
    DesktopSettings {
        name: state.config.name.clone(),
        support_url: state.config.support_url.clone(),
        suggested_server: state.config.server.clone().filter(|_| !state.config.locked),
        source: state.source.as_ref().map(|path| path.display().to_string()),
        problem: state.problem.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("repomemo-config-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn a_file_names_a_server_a_name_and_a_help_link() {
        let config = parse_config(
            r#"{ "server": "memo.example.com", "name": "  Acme   Memo ", "support": "https://help.example.com/memo" }"#,
        )
        .unwrap();
        assert_eq!(config.server.as_deref(), Some("https://memo.example.com"));
        assert_eq!(config.name.as_deref(), Some("Acme Memo"));
        assert_eq!(config.support_url.as_deref(), Some("https://help.example.com/memo"));
        assert!(config.locked, "a server is locked unless the file says otherwise");
    }

    #[test]
    fn the_example_for_administrators_is_a_valid_file() {
        let config = parse_config(include_str!("../../../../deploy/desktop/repomemo-desktop.example.json")).unwrap();
        assert_eq!(config.server.as_deref(), Some("https://memo.example.com"));
        assert_eq!(config.name.as_deref(), Some("Acme Memo"));
        assert!(config.locked && config.support_url.is_some());
    }

    #[test]
    fn a_server_can_be_left_open_and_a_byte_order_mark_is_forgiven() {
        let config = parse_config("\u{feff}{ \"server\": \"http://192.168.1.20:3020\", \"locked\": false }").unwrap();
        assert_eq!(config.server.as_deref(), Some("http://192.168.1.20:3020"));
        assert!(!config.locked);
        assert_eq!(parse_config("{}").unwrap(), DesktopConfig::default());
    }

    #[test]
    fn the_server_is_checked_like_a_typed_address() {
        for bad in [
            "http://memo.example.com",
            "https://memo.example.com/app",
            "ftp://x.example.com",
            "https://u:p@x.example.com",
        ] {
            let message = parse_config(&format!("{{ \"server\": \"{bad}\" }}")).unwrap_err();
            assert!(message.starts_with("server: "), "{bad}: {message}");
        }
    }

    #[test]
    fn what_is_not_valid_is_named() {
        assert!(parse_config("not json").unwrap_err().contains("expected"));
        assert!(parse_config(r#"{ "servr": "x" }"#).unwrap_err().contains("servr"));
        assert!(parse_config(r#"{ "name": "" }"#).unwrap_err().starts_with("name: "));
        assert!(parse_config(&format!("{{ \"name\": \"{}\" }}", "n".repeat(61)))
            .unwrap_err()
            .starts_with("name: "));
        assert!(
            parse_config(r#"{ "name": "two\nlines" }"#).is_ok(),
            "line breaks are folded into spaces"
        );
        assert!(parse_config(r#"{ "name": "bell\u0007" }"#)
            .unwrap_err()
            .starts_with("name: "));
        for bad in [
            "javascript:alert(1)",
            "file:///etc/passwd",
            "https://u:p@x.example.com",
            "help",
        ] {
            let message = parse_config(&format!("{{ \"support\": \"{bad}\" }}")).unwrap_err();
            assert!(message.starts_with("support: "), "{bad}: {message}");
        }
        assert!(parse_config(r#"{ "locked": true }"#)
            .unwrap_err()
            .starts_with("locked: "));
        assert!(parse_config(r#"{ "locked": "yes", "server": "x.example.com" }"#).is_err());
    }

    #[test]
    fn the_file_beside_the_executable_wins_over_the_one_in_the_config_folder() {
        let (beside, config) = (folder("beside"), folder("config"));
        assert_eq!(find_config(&[beside.clone(), config.clone()]), None);
        fs::write(config.join(CONFIG_FILE), "{}").unwrap();
        assert_eq!(
            find_config(&[beside.clone(), config.clone()]),
            Some(config.join(CONFIG_FILE))
        );
        fs::write(beside.join(CONFIG_FILE), "{}").unwrap();
        assert_eq!(
            find_config(&[beside.clone(), config.clone()]),
            Some(beside.join(CONFIG_FILE))
        );
        let _ = fs::remove_dir_all(beside);
        let _ = fs::remove_dir_all(config);
    }

    #[test]
    fn a_certificate_bundle_belongs_to_the_files_server() {
        let config = parse_config(r#"{ "server": "memo.example.com", "certificate": " memo-ca.pem " }"#).unwrap();
        assert_eq!(config.certificate.as_deref(), Some("memo-ca.pem"));
        assert!(parse_config(r#"{ "certificate": "memo-ca.pem" }"#)
            .unwrap_err()
            .starts_with("certificate: "));
        assert!(
            parse_config(r#"{ "server": "memo.example.com", "certificate": "a\u0007.pem" }"#)
                .unwrap_err()
                .starts_with("certificate: ")
        );
        let long = format!(
            r#"{{ "server": "memo.example.com", "certificate": "{}" }}"#,
            "c".repeat(300)
        );
        assert!(parse_config(&long).unwrap_err().starts_with("certificate: "));
        assert_eq!(
            parse_config(r#"{ "server": "memo.example.com", "certificate": "" }"#)
                .unwrap()
                .certificate,
            None
        );
    }

    #[test]
    fn the_bundle_is_read_from_beside_the_file_and_a_bad_one_spoils_the_whole_file() {
        let directory = folder("bundle");
        let file = directory.join(CONFIG_FILE);
        let text = r#"{ "server": "memo.example.com", "certificate": "memo-ca.pem" }"#;
        fs::write(&file, text).unwrap();
        // The bundle is not there.
        let state = load_from(std::slice::from_ref(&directory));
        assert!(
            state.problem.as_deref().unwrap().contains("memo-ca.pem cannot be read")
                || state.problem.as_deref().unwrap().contains("memo-ca.pem"),
            "{:?}",
            state.problem
        );
        assert!(
            state.certificates.is_empty() && state.config.server.is_none(),
            "nothing of a spoiled file is kept"
        );
        // It is there, and good.
        fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/test-ca.pem"),
            directory.join("memo-ca.pem"),
        )
        .unwrap();
        let state = load_from(std::slice::from_ref(&directory));
        assert_eq!(state.problem, None);
        assert_eq!(state.certificates.len(), 1);
        assert_eq!(state.config.server.as_deref(), Some("https://memo.example.com"));
        // It is there, and not a certificate.
        fs::write(directory.join("memo-ca.pem"), "not a certificate").unwrap();
        let state = load_from(std::slice::from_ref(&directory));
        assert!(
            state.problem.as_deref().unwrap().contains("no certificate"),
            "{:?}",
            state.problem
        );
        assert!(state.certificates.is_empty());
        // It is far too big.
        fs::write(directory.join("memo-ca.pem"), "x".repeat(300_000)).unwrap();
        assert!(load_from(std::slice::from_ref(&directory))
            .problem
            .unwrap()
            .contains("bytes"));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn a_file_is_read_whole_or_not_at_all() {
        let directory = folder("read");
        let path = directory.join(CONFIG_FILE);
        fs::write(&path, r#"{ "server": "memo.example.com" }"#).unwrap();
        assert_eq!(
            read_config(&path).unwrap().server.as_deref(),
            Some("https://memo.example.com")
        );
        fs::write(&path, "x".repeat(20_000)).unwrap();
        assert!(read_config(&path).unwrap_err().contains("bytes"));
        assert!(read_config(&directory.join("missing.json")).is_err());
        let _ = fs::remove_dir_all(directory);
    }
}
