//! Moving a local workspace to a server (V0.2.50).
//!
//! The local workspace is written as a workspace archive, the same file the server's export makes,
//! into the `exports` folder of the app's data. The page then reads it back and hands it to the
//! server's import (which checks it first and writes nothing until the person has seen what is in
//! it). Nothing here talks to a server, and the commands only touch files of the `exports` folder.

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use repomemo_domain::ArchiveCounts;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::AppState;

/// Archives older than this are removed when a new one is made; a person who wants to keep one saves a copy.
const KEEP_FOR: Duration = Duration::from_secs(7 * 24 * 3600);

#[derive(Serialize)]
pub struct PreparedArchive {
    /// Where the archive is. Handed back to `read_export`, `copy_export` and `discard_export`.
    pub path: String,
    pub file_name: String,
    pub workspace_name: String,
    pub size_bytes: u64,
    pub counts: ArchiveCounts,
}

fn exports_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("exports");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    Ok(directory)
}

/// `path` as a file of the exports folder, or why it is not one (no other file is ever read, copied or deleted).
fn export_file(app: &AppHandle, path: &str) -> Result<PathBuf, String> {
    resolve_in(&exports_dir(app)?, path)
}

fn resolve_in(directory: &Path, path: &str) -> Result<PathBuf, String> {
    let directory = directory.canonicalize().map_err(|error| error.to_string())?;
    let file = Path::new(path)
        .canonicalize()
        .map_err(|_| "That export is not there any more.".to_owned())?;
    if file.parent() != Some(directory.as_path()) || !file.is_file() {
        return Err("That is not one of this app's exports.".to_owned());
    }
    Ok(file)
}

/// `repomemo-<workspace name>-<time>.tar.gz`, safe for a disk.
fn archive_file_name(workspace_name: &str, stamp: &str) -> String {
    let mut slug = String::new();
    for character in workspace_name.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "workspace" } else { slug };
    let slug: String = slug.chars().take(40).collect();
    format!("repomemo-{}-{stamp}.tar.gz", slug.trim_end_matches('-'))
}

fn remove_old_exports(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else { return };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age > KEEP_FOR);
        if old && entry.path().is_file() {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// Writes a local workspace as an archive in the exports folder.
#[tauri::command]
pub async fn export_workspace_archive(
    app: AppHandle,
    state: State<'_, AppState>,
    workspace_id: String,
) -> Result<PreparedArchive, String> {
    let directory = exports_dir(&app)?;
    remove_old_exports(&directory);
    let workspace = state
        .core
        .list_workspaces()
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|workspace| workspace.id == workspace_id)
        .ok_or_else(|| "That workspace was not found.".to_owned())?;
    let file_name = archive_file_name(&workspace.name, &chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string());
    let destination = directory.join(&file_name);
    let manifest = state
        .core
        .export_workspace_archive(&workspace_id, &destination)
        .await
        .map_err(|error| error.to_string())?;
    let size_bytes = fs::metadata(&destination).map_err(|error| error.to_string())?.len();
    Ok(PreparedArchive {
        path: destination.to_string_lossy().into_owned(),
        file_name,
        workspace_name: manifest.workspace_name,
        size_bytes,
        counts: manifest.counts,
    })
}

/// The bytes of an export, for the page to hand to a server's import.
#[tauri::command]
pub fn read_export(app: AppHandle, path: String) -> Result<tauri::ipc::Response, String> {
    let bytes = fs::read(export_file(&app, &path)?).map_err(|error| error.to_string())?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Saves a copy of an export where the person chose (a save dialog on the page picks the place).
#[tauri::command]
pub fn copy_export(app: AppHandle, path: String, destination: String) -> Result<(), String> {
    let source = export_file(&app, &path)?;
    let destination = PathBuf::from(destination);
    let in_existing_folder = destination.parent().is_some_and(Path::is_dir);
    if destination.is_dir() || !in_existing_folder {
        return Err("Choose a file name in an existing folder.".to_owned());
    }
    fs::copy(source, destination)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Deletes an export once it has been imported, or is no longer wanted.
#[tauri::command]
pub fn discard_export(app: AppHandle, path: String) -> Result<(), String> {
    match export_file(&app, &path) {
        Ok(file) => fs::remove_file(file).map_err(|error| error.to_string()),
        // Already gone is as good as deleted.
        Err(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "repomemo-handoff-{name}-{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn only_files_directly_in_the_exports_folder_resolve() {
        let exports = folder("exports");
        let elsewhere = folder("elsewhere");
        let inside = exports.join("a.tar.gz");
        fs::write(&inside, b"x").unwrap();
        fs::create_dir_all(exports.join("sub")).unwrap();
        fs::write(exports.join("sub").join("b.tar.gz"), b"x").unwrap();
        fs::write(elsewhere.join("c.tar.gz"), b"x").unwrap();

        assert!(resolve_in(&exports, inside.to_str().unwrap()).is_ok());
        assert!(resolve_in(&exports, exports.join("sub").join("b.tar.gz").to_str().unwrap()).is_err());
        assert!(resolve_in(&exports, elsewhere.join("c.tar.gz").to_str().unwrap()).is_err());
        assert!(
            resolve_in(&exports, exports.join("sub").to_str().unwrap()).is_err(),
            "a folder is not an export"
        );
        // A way back out of the folder lands on a file that is not in it.
        let sneaky = exports.join("..").join(elsewhere.file_name().unwrap()).join("c.tar.gz");
        assert!(resolve_in(&exports, sneaky.to_str().unwrap()).is_err());
        assert!(resolve_in(&exports, exports.join("missing").to_str().unwrap()).is_err());
        let _ = fs::remove_dir_all(exports);
        let _ = fs::remove_dir_all(elsewhere);
    }

    #[test]
    fn the_file_name_is_a_safe_slug_of_the_workspace_name() {
        let name = archive_file_name("Platform & Docs / Équipe \"A\"", "20261016-101500");
        assert_eq!(name, "repomemo-platform-docs-quipe-a-20261016-101500.tar.gz");
        assert!(archive_file_name("???", "20261016-101500").starts_with("repomemo-workspace-"));
        assert!(!archive_file_name(&"x".repeat(200), "s").contains(&"x".repeat(41)));
    }

    #[test]
    fn old_exports_are_removed_and_new_ones_kept() {
        let exports = folder("old");
        fs::write(exports.join("new.tar.gz"), b"x").unwrap();
        remove_old_exports(&exports);
        assert!(exports.join("new.tar.gz").exists());
        let _ = fs::remove_dir_all(exports);
    }
}
