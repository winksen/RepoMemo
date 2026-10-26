//! Updates of the app (V0.4.56). The app can look for a newer version of itself in a feed and offer it. Every update is checked against
//! a public key before it is installed: an unsigned or wrongly signed one is refused by the updater, and so is an update announced
//! for a version other than the one its signature was made for (a replayed older release).
//!
//! Where the feed comes from: the administrator's own (`updates` in `repomemo-desktop.json`, with the key it is signed by), which the
//! app then checks without asking the person; or the project's release feed, when the app was built with the release key
//! (`REPOMEMO_UPDATE_PUBKEY`), which the person turns on for themselves. With neither, the app never looks.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use url::Url;

use crate::config::{ConfigState, UpdateFeed};

/// The release feed of the project: a `latest.json` that the release attaches (V0.4.56).
pub const RELEASE_FEED: &str = "https://github.com/repomemo/repomemo/releases/latest/download/latest.json";
/// The public key the release's updates are signed with, given when the app is built.
const RELEASE_KEY: Option<&str> = option_env!("REPOMEMO_UPDATE_PUBKEY");

/// Who decides whether the app looks for updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// There is no feed to look at.
    Off,
    /// The project's release feed; the person chooses.
    Optional,
    /// The administrator's feed; the app looks.
    Managed,
}

/// The feed to use and who decides, from the administrator's file and the key this build carries.
pub fn pick_feed(file: &ConfigState, release_key: Option<&str>) -> (Mode, Option<UpdateFeed>) {
    if let Some(feed) = &file.config.updates {
        return (Mode::Managed, Some(feed.clone()));
    }
    match release_key.map(str::trim).filter(|key| !key.is_empty()) {
        Some(key) => (
            Mode::Optional,
            Some(UpdateFeed {
                url: RELEASE_FEED.to_owned(),
                public_key: key.to_owned(),
            }),
        ),
        None => (Mode::Off, None),
    }
}

/// An update that was found and waits for the person's answer.
#[derive(Default)]
pub struct Found(Mutex<Option<Update>>);

/// What the page is told of an update.
#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

/// Who decides, so the page can show the choice only when it is the person's.
#[tauri::command]
pub fn update_mode(file: State<'_, ConfigState>) -> Mode {
    pick_feed(&file, RELEASE_KEY).0
}

/// Looks for a newer version. `enabled` is the person's own choice, which counts only for the project's feed. Answers nothing when
/// there is no feed, the person has not chosen, or the app is up to date.
#[tauri::command]
pub async fn check_update(
    app: AppHandle,
    file: State<'_, ConfigState>,
    found: State<'_, Found>,
    enabled: bool,
) -> Result<Option<UpdateInfo>, String> {
    let (mode, feed) = pick_feed(&file, RELEASE_KEY);
    let Some(feed) = feed else {
        return Ok(None);
    };
    if mode == Mode::Optional && !enabled {
        return Ok(None);
    }
    let url = Url::parse(&feed.url).map_err(|error| error.to_string())?;
    let updater = app
        .updater_builder()
        .pubkey(feed.public_key)
        .endpoints(vec![url])
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| error.to_string())?;
    let update = updater.check().await.map_err(|error| error.to_string())?;
    let info = update.as_ref().map(|update| UpdateInfo {
        version: update.version.clone(),
        notes: update.body.clone(),
    });
    *found.0.lock().map_err(|_| "state is locked".to_owned())? = update;
    Ok(info)
}

/// Downloads the update that was found, checks its signature, installs it and starts the app again.
#[tauri::command]
pub async fn install_update(app: AppHandle, found: State<'_, Found>) -> Result<(), String> {
    let update = found
        .0
        .lock()
        .map_err(|_| "state is locked".to_owned())?
        .take()
        .ok_or_else(|| "No update is waiting.".to_owned())?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| error.to_string())?;
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DesktopConfig;

    const KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDEyMzQ1Njc4OTBBQkNERUYKUldRMTIzNDU2Nzg5MEFCQ0RFRjEyMzQ1Njc4OTBBQkNERUYxMjM0NTY3ODkwQUJDREVG";

    fn file_with(feed: Option<UpdateFeed>) -> ConfigState {
        ConfigState {
            config: DesktopConfig {
                updates: feed,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn the_administrators_feed_wins_and_is_looked_at_without_asking() {
        let own = UpdateFeed {
            url: "https://updates.example.com/latest.json".to_owned(),
            public_key: KEY.to_owned(),
        };
        assert_eq!(
            pick_feed(&file_with(Some(own.clone())), Some("release-key")),
            (Mode::Managed, Some(own))
        );
    }

    #[test]
    fn the_projects_feed_is_the_persons_to_turn_on_and_only_in_a_build_that_has_its_key() {
        let (mode, feed) = pick_feed(&file_with(None), Some(KEY));
        assert_eq!(mode, Mode::Optional);
        let feed = feed.unwrap();
        assert_eq!((feed.url.as_str(), feed.public_key.as_str()), (RELEASE_FEED, KEY));
        assert_eq!(pick_feed(&file_with(None), None), (Mode::Off, None));
        assert_eq!(pick_feed(&file_with(None), Some("  ")), (Mode::Off, None));
    }

    #[test]
    fn the_release_feed_is_a_secure_address_on_the_projects_own_releases() {
        let url = Url::parse(RELEASE_FEED).unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(url.host_str(), Some("github.com"));
    }
}
