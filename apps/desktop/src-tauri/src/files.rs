//! Saving a file the person asked for (V0.4.52): a native save dialog, then the bytes written where the person chose. A blob
//! download is not the same in every webview (some save nothing, some save without asking), so the client hands the bytes to the
//! shell instead. The shell writes nowhere the person did not point at, and takes the name only as a suggestion.

use std::fs;

use tauri::{
    ipc::{InvokeBody, Request},
    AppHandle,
};
use tauri_plugin_dialog::DialogExt;

/// The header that carries the suggested file name, percent-encoded (`encodeURIComponent`).
const NAME_HEADER: &str = "x-file-name";
const FALLBACK_NAME: &str = "download";
const MAX_NAME_CHARS: usize = 120;

/// Reads `%XX` escapes as UTF-8; `None` for a bad escape or bytes that are not UTF-8.
pub(crate) fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let pair = text.get(index + 1..index + 3)?;
            decoded.push(u8::from_str_radix(pair, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

/// A file name that is safe to suggest: no folders, no characters a file system refuses, no name Windows reserves, not empty, not
/// long. The person can still type anything in the dialog; this is only what it starts with.
pub fn safe_file_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|character| {
            if character.is_control() || r#"<>:"/\|?*"#.contains(character) {
                '-'
            } else {
                character
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    let mut name: String = trimmed.chars().take(MAX_NAME_CHARS).collect();
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.ends_with(|c: char| c.is_ascii_digit()));
    if name.is_empty() || name.chars().all(|c| c == '-') {
        name = FALLBACK_NAME.to_owned();
    } else if reserved {
        name = format!("{FALLBACK_NAME}-{name}");
    }
    name
}

/// Asks where to save, and writes the bytes there. Answers the path chosen, or nothing when the person cancelled.
#[tauri::command]
pub async fn save_file(app: AppHandle, request: Request<'_>) -> Result<Option<String>, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("The file's bytes were not sent.".to_owned());
    };
    let suggested = request
        .headers()
        .get(NAME_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(percent_decode)
        .map(|name| safe_file_name(&name))
        .unwrap_or_else(|| FALLBACK_NAME.to_owned());
    let Some(chosen) = app.dialog().file().set_file_name(&suggested).blocking_save_file() else {
        return Ok(None);
    };
    let path = chosen.into_path().map_err(|error| error.to_string())?;
    fs::write(&path, bytes).map_err(|error| format!("The file could not be saved to {}: {error}", path.display()))?;
    Ok(Some(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_arrives_percent_encoded_and_a_bad_escape_is_refused() {
        assert_eq!(
            percent_decode("workspace%20archive%20%C3%A9.zip").as_deref(),
            Some("workspace archive é.zip")
        );
        assert_eq!(percent_decode("plain.md").as_deref(), Some("plain.md"));
        assert_eq!(percent_decode("100%"), None);
        assert_eq!(percent_decode("%zz"), None);
        assert_eq!(percent_decode("%FF%FE"), None, "not UTF-8");
    }

    #[test]
    fn a_suggested_name_has_no_folders_and_no_forbidden_characters() {
        assert_eq!(safe_file_name("../../etc/passwd"), "-..-etc-passwd");
        assert_eq!(safe_file_name("C:\\Windows\\evil.exe"), "C--Windows-evil.exe");
        assert_eq!(safe_file_name("a<b>c:d\"e|f?g*h.txt"), "a-b-c-d-e-f-g-h.txt");
        assert_eq!(safe_file_name("tab\there\u{7}.md"), "tab-here-.md");
        assert_eq!(safe_file_name("  report.pdf  "), "report.pdf");
    }

    #[test]
    fn nothing_empty_reserved_or_endless_is_suggested() {
        assert_eq!(safe_file_name(""), "download");
        assert_eq!(safe_file_name("   "), "download");
        assert_eq!(safe_file_name("..."), "download");
        assert_eq!(safe_file_name("///"), "download");
        assert_eq!(safe_file_name("CON"), "download-CON");
        assert_eq!(safe_file_name("nul.txt"), "download-nul.txt");
        assert_eq!(safe_file_name("com1.log"), "download-com1.log");
        assert_eq!(safe_file_name("console.log"), "console.log");
        assert_eq!(safe_file_name(&"x".repeat(500)).chars().count(), MAX_NAME_CHARS);
    }
}
