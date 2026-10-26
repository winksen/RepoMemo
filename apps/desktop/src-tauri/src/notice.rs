//! Native notices and the badge (V0.4.53). The client watches the account's notices and the review queue (its polling is the
//! web client's, with the person's own notification preferences deciding what exists to be shown) and asks the shell to show a
//! system notification when the window is in the background, and to mark the app's icon with what waits.

#[cfg(windows)]
use tauri::image::Image;
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::{NotificationExt, PermissionState};

const MAX_TITLE_CHARS: usize = 100;
const MAX_BODY_CHARS: usize = 300;
const BADGE_SIZE: u32 = 16;

/// What goes in a notification: one line of title and a short body, with no control characters (they would be shown as boxes or end
/// the text early) and never empty.
pub fn clean_notice(title: &str, body: &str) -> (String, String) {
    let tidy = |text: &str, limit: usize| -> String {
        let one_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
        one_line
            .chars()
            .filter(|character| !character.is_control())
            .take(limit)
            .collect()
    };
    let title = tidy(title, MAX_TITLE_CHARS);
    (
        if title.is_empty() { "RepoMemo".to_owned() } else { title },
        tidy(body, MAX_BODY_CHARS),
    )
}

#[cfg_attr(not(windows), allow(dead_code))]
/// A small red dot with a white rim, as raw RGBA, for the taskbar of a system that has no number to show (Windows).
pub fn badge_pixels() -> Vec<u8> {
    let size = BADGE_SIZE as f32;
    let centre = (size - 1.0) / 2.0;
    let mut pixels = Vec::with_capacity((BADGE_SIZE * BADGE_SIZE * 4) as usize);
    for y in 0..BADGE_SIZE {
        for x in 0..BADGE_SIZE {
            let distance = ((x as f32 - centre).powi(2) + (y as f32 - centre).powi(2)).sqrt();
            let pixel: [u8; 4] = if distance <= 5.0 {
                [0xd9, 0x2d, 0x20, 0xff]
            } else if distance <= 7.0 {
                [0xff, 0xff, 0xff, 0xff]
            } else {
                [0, 0, 0, 0]
            };
            pixels.extend_from_slice(&pixel);
        }
    }
    pixels
}

/// Shows a system notification. Answers whether it was shown: `false` when the person has not allowed notifications.
#[tauri::command]
pub fn notify(app: AppHandle, title: String, body: String) -> Result<bool, String> {
    let notifications = app.notification();
    let mut state = notifications.permission_state().map_err(|error| error.to_string())?;
    if state != PermissionState::Granted {
        state = notifications.request_permission().map_err(|error| error.to_string())?;
    }
    if state != PermissionState::Granted {
        return Ok(false);
    }
    let (title, body) = clean_notice(&title, &body);
    notifications
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|error| error.to_string())?;
    Ok(true)
}

/// Marks the app's icon with what waits: a count where the system has one (macOS, Linux), a dot on the taskbar button where it has
/// not (Windows). Zero takes the mark away.
#[tauri::command]
pub fn set_badge(app: AppHandle, count: u32) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    #[cfg(windows)]
    {
        let icon = (count > 0).then(|| Image::new_owned(badge_pixels(), BADGE_SIZE, BADGE_SIZE));
        window.set_overlay_icon(icon).map_err(|error| error.to_string())
    }
    #[cfg(not(windows))]
    {
        window
            .set_badge_count((count > 0).then_some(i64::from(count)))
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_notice_is_one_line_with_no_control_characters_and_never_empty() {
        assert_eq!(
            clean_notice("  Review\nneeded \t now ", "A file\u{7} is waiting."),
            ("Review needed now".to_owned(), "A file is waiting.".to_owned())
        );
        assert_eq!(clean_notice("", "body").0, "RepoMemo");
        assert_eq!(clean_notice("\u{0}\u{1}", "").0, "RepoMemo");
        assert_eq!(
            clean_notice(&"t".repeat(500), &"b".repeat(900)).0.chars().count(),
            MAX_TITLE_CHARS
        );
        assert_eq!(clean_notice("t", &"b".repeat(900)).1.chars().count(), MAX_BODY_CHARS);
    }

    #[test]
    fn the_badge_is_a_red_dot_with_a_white_rim_on_nothing() {
        let pixels = badge_pixels();
        assert_eq!(pixels.len(), (BADGE_SIZE * BADGE_SIZE * 4) as usize);
        let at = |x: u32, y: u32| &pixels[((y * BADGE_SIZE + x) * 4) as usize..][..4];
        assert_eq!(at(8, 8), &[0xd9, 0x2d, 0x20, 0xff], "the middle is red");
        assert_eq!(at(8, 1)[3], 0xff, "the rim is opaque");
        assert_eq!(at(8, 1)[..3], [0xff, 0xff, 0xff], "and white");
        assert_eq!(at(0, 0), &[0, 0, 0, 0], "the corner is clear");
    }
}
