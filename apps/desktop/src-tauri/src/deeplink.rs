//! `repomemo://` links (V0.4.54). A page of the web client (and a notice on it) can offer *Open in the app*, and the system hands
//! the link to the app, which shows the same page. A link is only a request to show one of the app's own pages: it carries a path
//! of the client and, optionally, the server it was made on, and nothing else. A path that is not one of the client's, a link for
//! another server, or anything that tries to leave the client is refused with a message the page shows.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State};
use url::Url;

use crate::{connection::normalize_server_url, files::percent_decode};

/// The scheme the installer registers with the system.
pub const SCHEME: &str = "repomemo";
/// The first parts of the paths the client has pages under (its router's: `src/web/router.ts`).
const APP_AREAS: [&str; 5] = ["workspaces", "dashboard", "profile", "notifications", "system"];
const MAX_PATH_CHARS: usize = 2048;

/// The link that arrived before the page was ready to show it (the app was started by the link).
#[derive(Default)]
pub struct Links {
    pending: Mutex<Option<String>>,
}

/// Whether this is a path of the client: `/workspaces/...`, `/dashboard`, `/profile`, `/notifications`, `/system/...`, with an
/// optional query and fragment. Never another site, another scheme, a folder above the client or a character that is not text.
pub fn is_app_path(path: &str) -> bool {
    if path.chars().count() > MAX_PATH_CHARS || !path.starts_with('/') || path.starts_with("//") {
        return false;
    }
    if path
        .chars()
        .any(|character| character.is_control() || character == '\\')
    {
        return false;
    }
    let pathname = path.split(['?', '#']).next().unwrap_or("");
    let mut parts = pathname.split('/').filter(|part| !part.is_empty());
    let Some(first) = parts.next() else {
        return false;
    };
    if !APP_AREAS.contains(&first) {
        return false;
    }
    std::iter::once(first)
        .chain(parts)
        .all(|part| match percent_decode(part) {
            Some(decoded) => {
                decoded != ".."
                    && decoded != "."
                    && !decoded.contains(['/', '\\'])
                    && !decoded.chars().any(char::is_control)
            }
            None => false,
        })
}

/// The path a link asks the app to show, or why it cannot. `current` is the server the app talks to; a link made for another one is
/// refused, because its path means nothing here.
pub fn parse_link(link: &str, current: Option<&str>) -> Result<String, String> {
    let url = Url::parse(link).map_err(|_| "That link is not a web address.".to_owned())?;
    if url.scheme() != SCHEME
        || url.host_str() != Some("open")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err("That is not a link this app opens.".to_owned());
    }
    let mut path = None;
    let mut server = None;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "path" => path = Some(value.into_owned()),
            "server" => server = Some(value.into_owned()),
            _ => return Err("That link carries something this app does not take.".to_owned()),
        }
    }
    let path = path.ok_or_else(|| "That link names no page.".to_owned())?;
    if !is_app_path(&path) {
        return Err("That link is not for a page of RepoMemo.".to_owned());
    }
    let Some(current) = current else {
        return Err("Choose a server in the app first, then open the link again.".to_owned());
    };
    if let Some(server) = server {
        let wanted =
            normalize_server_url(&server).map_err(|_| "That link names a server this app cannot use.".to_owned())?;
        if wanted != current {
            return Err(format!(
                "That link is for {wanted}, and this app is connected to {current}."
            ));
        }
    }
    Ok(path)
}

/// Takes the first link of those the system handed over that can be shown; tells the page, or says why not.
pub fn handle(app: &AppHandle, links: &[String]) {
    let current = app.state::<crate::connection::Connection>().current_origin_or_none();
    let mut refusal = None;
    for link in links {
        match parse_link(link, current.as_deref()) {
            Ok(path) => {
                if let Ok(mut pending) = app.state::<Links>().pending.lock() {
                    *pending = Some(path.clone());
                }
                let _ = app.emit_to("main", "open-path", path);
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                return;
            }
            Err(reason) => refusal = Some(reason),
        }
    }
    if let Some(reason) = refusal {
        let _ = app.emit_to("main", "open-path-refused", reason);
    }
}

/// The link the app was started with, once: the page asks when it is ready to show it.
#[tauri::command]
pub fn take_pending_link(links: State<'_, Links>) -> Option<String> {
    links.pending.lock().ok().and_then(|mut pending| pending.take())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HERE: Option<&str> = Some("https://memo.example.com");

    #[test]
    fn only_the_clients_own_paths_are_taken() {
        for good in [
            "/dashboard",
            "/profile",
            "/notifications",
            "/workspaces",
            "/workspaces/w1",
            "/workspaces/w1/evidence?q=a%20b#top",
            "/workspaces/w1/artifacts/a1",
            "/system/jobs",
        ] {
            assert!(is_app_path(good), "{good} should be taken");
        }
        for bad in [
            "",
            "/",
            "dashboard",
            "//evil.example.com/workspaces",
            "/\\evil.example.com",
            "https://evil.example.com/dashboard",
            "javascript:alert(1)",
            "/login",
            "/v1/system/backups",
            "/workspaces/../../etc/passwd",
            "/workspaces/%2e%2e/system",
            "/workspaces/w1%2Fx",
            "/workspaces/w1\u{0}",
            "/workspaces/%zz",
            "/dashboard\n/x",
        ] {
            assert!(!is_app_path(bad), "{bad:?} should be refused");
        }
        assert!(!is_app_path(&format!("/workspaces/{}", "a".repeat(2100))));
    }

    #[test]
    fn a_link_for_this_server_names_the_page() {
        assert_eq!(
            parse_link(
                "repomemo://open?server=https%3A%2F%2Fmemo.example.com&path=%2Fworkspaces%2Fw1%2Fevidence",
                HERE
            )
            .unwrap(),
            "/workspaces/w1/evidence"
        );
        assert_eq!(
            parse_link("repomemo://open?path=%2Fnotifications", HERE).unwrap(),
            "/notifications",
            "the server is optional"
        );
        assert_eq!(
            parse_link("repomemo://open?server=Memo.Example.com&path=%2Fdashboard", HERE).unwrap(),
            "/dashboard",
            "the server is read like a typed address"
        );
    }

    #[test]
    fn a_link_that_is_not_ours_is_refused_with_a_reason() {
        for (link, word) in [
            ("https://memo.example.com/dashboard", "not a link"),
            ("repomemo://elsewhere?path=%2Fdashboard", "not a link"),
            ("repomemo://user:pw@open?path=%2Fdashboard", "not a link"),
            ("repomemo://open", "names no page"),
            ("repomemo://open?path=%2Flogin", "not for a page"),
            ("repomemo://open?path=https%3A%2F%2Fevil.example.com", "not for a page"),
            ("repomemo://open?path=%2Fdashboard&run=calc", "does not take"),
            (
                "repomemo://open?server=https%3A%2F%2Fother.example.com&path=%2Fdashboard",
                "connected to",
            ),
            (
                "repomemo://open?server=http%3A%2F%2Fmemo.example.com&path=%2Fdashboard",
                "cannot use",
            ),
            ("not a url", "not a web address"),
        ] {
            let message = parse_link(link, HERE).unwrap_err();
            assert!(message.contains(word), "{link}: {message}");
        }
    }

    #[test]
    fn without_a_server_there_is_nowhere_to_show_it() {
        assert!(parse_link("repomemo://open?path=%2Fdashboard", None)
            .unwrap_err()
            .contains("Choose a server"));
    }
}
