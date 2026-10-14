//! Backup and restore of an environment (roadmap O1).
//!
//! A backup is a folder in `<data folder>/backups/` holding a consistent
//! snapshot of the database (`VACUUM INTO`, taken while the server runs), a
//! copy of the stored files, the key file when there is one, and a manifest.
//! Stored files never change once written, so a backup reuses the files of the
//! one before it through hard links where it can: only new files cost space.
//!
//! A restore never touches the running environment. It copies a backup into
//! a new environment folder under `workspace-data/`, checks that it opens,
//! and leaves it to an administrator to attach it from the environment menu.
//!
//! The folder is inside the data folder, so it is on the same disk: copy it
//! elsewhere (the production guide shows how) for it to protect against a
//! lost disk. It contains the key file, so it needs the same care as the data.

use std::path::{Path, PathBuf};

use axum::{
    extract::{Path as UrlPath, State},
    Json,
};
use chrono::Utc;
use repomemo_storage::{StorageConfig, StorageEngine, SECRET_KEY_FILE_NAME};
use serde::{Deserialize, Serialize};

use crate::quotas::free_disk_bytes;
use crate::system::{audit, require_full_system_admin, require_system_admin, require_system_change};
use crate::{ApiError, AppState, AuthenticatedSubject};

/// The folder inside the data folder that holds backups.
pub(crate) const BACKUPS_DIR: &str = "backups";
const MANIFEST_FILE: &str = "manifest.json";
const DATABASE_FILE: &str = "repomemo.sqlite";
const PARTIAL_SUFFIX: &str = ".partial";
const BACKUP_PREFIX: &str = "backup-";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackupManifest {
    /// The folder name, which is how a backup is addressed.
    pub name: String,
    pub created_at: String,
    /// The server version that made it.
    pub version: String,
    /// `manual` or `scheduled`.
    pub trigger: String,
    pub database_bytes: u64,
    pub blob_count: u64,
    pub blob_bytes: u64,
    /// Files the database named that were gone by the time they were copied.
    pub missing_blobs: u64,
    /// Files that cost no space because the previous backup had them.
    pub reused_blobs: u64,
    pub includes_key: bool,
}

/// What the last backup or restore did, for the page and the console.
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct BackupStatus {
    pub running: bool,
    /// `backup` or `restore`, while running and after.
    pub operation: Option<&'static str>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub succeeded: Option<bool>,
    /// What was made, or why it failed.
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct BackupsResponse {
    pub backups: Vec<BackupManifest>,
    pub status: BackupStatus,
    pub directory: String,
    pub keep: u32,
    pub interval_hours: u64,
    pub disk_free_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RestoreRequest {
    /// The name of the new environment folder.
    pub environment: String,
}

// ---------------------------------------------------------------------------
// Files

/// A backup's folder name: only what [`create`] writes.
fn valid_backup_name(name: &str) -> bool {
    name.starts_with(BACKUP_PREFIX)
        && name.len() <= 64
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn backups_root(state: &AppState) -> PathBuf {
    state.storage.data_dir().join(BACKUPS_DIR)
}

/// What a folder copy did.
#[derive(Debug, Default, PartialEq, Eq)]
struct CopyStats {
    files: u64,
    bytes: u64,
    missing: u64,
    reused: u64,
}

/// Copies the files under `from` into `to`. A file that `reuse_from` has at
/// the same place and size is linked to it instead of copied. A file that
/// vanishes while the copy runs (deleted by garbage collection) is counted.
fn copy_files(from: &Path, to: &Path, reuse_from: Option<&Path>) -> std::io::Result<CopyStats> {
    let mut stats = CopyStats::default();
    if !from.is_dir() {
        return Ok(stats);
    }
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        std::fs::create_dir_all(to.join(&relative))?;
        for entry in std::fs::read_dir(from.join(&relative))? {
            let entry = entry?;
            let child = relative.join(entry.file_name());
            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(_) => {
                    stats.missing += 1;
                    continue;
                }
            };
            if kind.is_dir() {
                pending.push(child);
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            let source = from.join(&child);
            let target = to.join(&child);
            let Ok(size) = std::fs::metadata(&source).map(|metadata| metadata.len()) else {
                stats.missing += 1;
                continue;
            };
            let reusable = reuse_from
                .map(|previous| previous.join(&child))
                .filter(|previous| std::fs::metadata(previous).is_ok_and(|metadata| metadata.len() == size));
            if let Some(previous) = reusable {
                if std::fs::hard_link(&previous, &target).is_ok() {
                    stats.files += 1;
                    stats.bytes += size;
                    stats.reused += 1;
                    continue;
                }
            }
            match std::fs::copy(&source, &target) {
                Ok(copied) => {
                    stats.files += 1;
                    stats.bytes += copied;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => stats.missing += 1,
                Err(error) => return Err(error),
            }
        }
    }
    Ok(stats)
}

fn read_manifest(folder: &Path) -> Option<BackupManifest> {
    let text = std::fs::read_to_string(folder.join(MANIFEST_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Complete backups, newest first.
fn list_backups(root: &Path) -> Vec<BackupManifest> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut backups = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let manifest = read_manifest(&entry.path())?;
            (entry.file_name().to_string_lossy() == manifest.name && valid_backup_name(&manifest.name))
                .then_some(manifest)
        })
        .collect::<Vec<_>>();
    backups.sort_by(|left, right| right.created_at.cmp(&left.created_at).then(right.name.cmp(&left.name)));
    backups
}

// ---------------------------------------------------------------------------
// Operations

fn set_status(state: &AppState, update: impl FnOnce(&mut BackupStatus)) {
    if let Ok(mut status) = state.backup_status.lock() {
        update(&mut status);
    }
}

/// Marks a backup or restore as running; `false` when one already is.
fn begin(state: &AppState, operation: &'static str) -> bool {
    let Ok(mut status) = state.backup_status.lock() else {
        return false;
    };
    if status.running {
        return false;
    }
    *status = BackupStatus {
        running: true,
        operation: Some(operation),
        started_at: Some(Utc::now().to_rfc3339()),
        ..BackupStatus::default()
    };
    true
}

fn finish(state: &AppState, result: &Result<String, String>) {
    set_status(state, |status| {
        status.running = false;
        status.finished_at = Some(Utc::now().to_rfc3339());
        status.succeeded = Some(result.is_ok());
        status.message = Some(match result {
            Ok(message) | Err(message) => message.clone(),
        });
    });
}

/// Makes a backup. Returns its manifest.
async fn make_backup(state: &AppState, trigger: &str) -> anyhow::Result<BackupManifest> {
    use anyhow::Context;
    let root = backups_root(state);
    tokio::fs::create_dir_all(&root)
        .await
        .context("the backup folder could not be created")?;
    let name = format!("{BACKUP_PREFIX}{}", Utc::now().format("%Y%m%d-%H%M%S"));
    if root.join(&name).exists() {
        anyhow::bail!("A backup named {name} exists already; try again in a moment.");
    }
    let partial = root.join(format!("{name}{PARTIAL_SUFFIX}"));
    if partial.exists() {
        let _ = tokio::fs::remove_dir_all(&partial).await;
    }
    tokio::fs::create_dir_all(&partial).await?;
    let outcome = fill_backup(state, &root, &partial, &name, trigger).await;
    match outcome {
        Ok(manifest) => {
            rename_with_retries(&partial, &root.join(&name))
                .await
                .context("the finished backup could not be put in place")?;
            prune(state, &root).await;
            Ok(manifest)
        }
        Err(error) => {
            let _ = tokio::fs::remove_dir_all(&partial).await;
            Err(error)
        }
    }
}

async fn fill_backup(
    state: &AppState,
    root: &Path,
    partial: &Path,
    name: &str,
    trigger: &str,
) -> anyhow::Result<BackupManifest> {
    use anyhow::Context;
    // Taken first: every stored file the snapshot names exists by then.
    state.storage.snapshot_database(&partial.join(DATABASE_FILE)).await?;
    let database_bytes = tokio::fs::metadata(partial.join(DATABASE_FILE)).await?.len();

    let previous = list_backups(root)
        .into_iter()
        .next()
        .map(|backup| root.join(backup.name));
    let (source, target) = (state.storage.blob_dir().to_path_buf(), partial.join("blobs"));
    let stats = tokio::task::spawn_blocking(move || {
        copy_files(&source, &target, previous.map(|folder| folder.join("blobs")).as_deref())
    })
    .await
    .context("the file copy stopped unexpectedly")?
    .context("the stored files could not be copied")?;

    let key = state.storage.data_dir().join(SECRET_KEY_FILE_NAME);
    let includes_key = tokio::fs::copy(&key, partial.join(SECRET_KEY_FILE_NAME)).await.is_ok();
    if includes_key {
        // A copy takes the folder's permissions on Windows: make it owner only, like the original.
        repomemo_storage::restrict_to_owner(&partial.join(SECRET_KEY_FILE_NAME));
    }

    let manifest = BackupManifest {
        name: name.to_owned(),
        created_at: Utc::now().to_rfc3339(),
        version: state.instance.version.to_owned(),
        trigger: trigger.to_owned(),
        database_bytes,
        blob_count: stats.files,
        blob_bytes: stats.bytes,
        missing_blobs: stats.missing,
        reused_blobs: stats.reused,
        includes_key,
    };
    tokio::fs::write(partial.join(MANIFEST_FILE), serde_json::to_vec_pretty(&manifest)?).await?;
    Ok(manifest)
}

/// Renames a folder, trying again for a few seconds: on Windows a virus
/// scanner or the search indexer may hold the files of a folder that was just
/// written, and the rename is refused until they let go.
async fn rename_with_retries(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut attempt = 0;
    loop {
        match tokio::fs::rename(from, to).await {
            Ok(()) => return Ok(()),
            Err(error) if attempt < 25 => {
                tracing::debug!(error = %error, attempt, "A folder could not be renamed yet");
                attempt += 1;
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Deletes the oldest backups beyond the number to keep, and leftovers of
/// backups that did not finish.
async fn prune(state: &AppState, root: &Path) {
    let keep = state.settings().backup_keep.max(1) as usize;
    for old in list_backups(root).into_iter().skip(keep) {
        if let Err(error) = tokio::fs::remove_dir_all(root.join(&old.name)).await {
            tracing::warn!(backup = %old.name, error = %error, "An old backup could not be deleted");
        }
    }
    if let Ok(mut entries) = tokio::fs::read_dir(root).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.file_name().to_string_lossy().ends_with(PARTIAL_SUFFIX) {
                let _ = tokio::fs::remove_dir_all(entry.path()).await;
            }
        }
    }
}

/// Starts a backup in the background. A conflict when one is running.
pub(crate) fn start_backup(state: &AppState, trigger: &'static str) -> Result<(), ApiError> {
    if !begin(state, "backup") {
        return Err(ApiError::conflict("A backup or restore is already running."));
    }
    let state = state.clone();
    tokio::spawn(async move {
        let result = match make_backup(&state, trigger).await {
            Ok(manifest) => {
                tracing::info!(target: "audit", backup = %manifest.name, trigger, "Backup finished");
                Ok(format!(
                    "Backed up to {} ({} files, {} reused).",
                    manifest.name, manifest.blob_count, manifest.reused_blobs
                ))
            }
            Err(error) => {
                tracing::error!(error = %format!("{error:#}"), "Backup failed");
                Err(format!("The backup failed: {error:#}"))
            }
        };
        finish(&state, &result);
    });
    Ok(())
}

/// Runs a backup when automatic backups are on and the last one is older than
/// the interval. Called from the maintenance loop.
pub(crate) fn run_scheduled(state: &AppState) {
    let hours = state.settings().backup_interval_hours;
    if hours == 0 {
        return;
    }
    let newest = list_backups(&backups_root(state)).into_iter().next();
    let due = newest.map_or(true, |backup| {
        chrono::DateTime::parse_from_rfc3339(&backup.created_at)
            .map(|created| Utc::now().signed_duration_since(created) >= chrono::Duration::hours(hours as i64))
            .unwrap_or(true)
    });
    if due && start_backup(state, "scheduled").is_err() {
        tracing::debug!("A scheduled backup waits for the one that is running");
    }
}

async fn restore_backup(state: &AppState, name: &str, environment: &str) -> anyhow::Result<String> {
    use anyhow::Context;
    let source = backups_root(state).join(name);
    let manifest = read_manifest(&source).context("The backup is incomplete or its manifest cannot be read.")?;
    let root = crate::environment::environments_root()?;
    let target = root.join(environment);
    let staging = root.join(format!("{environment}.restoring"));
    if target.exists() || staging.exists() {
        anyhow::bail!("An environment named {environment} exists already; choose another name.");
    }
    tokio::fs::create_dir_all(&staging).await?;
    let copied = copy_into(&source, &staging).await;
    if let Err(error) = copied {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error);
    }
    rename_with_retries(&staging, &target)
        .await
        .context("the restored environment could not be put in place")?;

    // The proof that it works: open it, count what is in it, close it.
    let opened = StorageEngine::open(StorageConfig {
        data_dir: target.clone(),
        master_key: crate::config::env_text("REPOMEMO_SECRET_KEY"),
        read_connections: 1,
        vector_index_mb: 0,
    })
    .await;
    let engine = match opened {
        Ok(engine) => engine,
        Err(error) => {
            let _ = tokio::fs::remove_dir_all(&target).await;
            return Err(error.context("The restored database did not open, so it was removed."));
        }
    };
    let statistics = engine.system_statistics().await;
    engine.close().await;
    let statistics = statistics.context("The restored database could not be read.")?;
    Ok(format!(
        "Restored {name} (made by {}) as the environment {environment}: {} workspaces, {} files, {} people. Attach it from the environment menu.",
        manifest.version, statistics.workspace_count, statistics.artifact_count, statistics.user_count
    ))
}

async fn copy_into(source: &Path, staging: &Path) -> anyhow::Result<()> {
    use anyhow::Context;
    tokio::fs::copy(source.join(DATABASE_FILE), staging.join(DATABASE_FILE))
        .await
        .context("the database could not be copied")?;
    if source.join(SECRET_KEY_FILE_NAME).exists() {
        tokio::fs::copy(source.join(SECRET_KEY_FILE_NAME), staging.join(SECRET_KEY_FILE_NAME))
            .await
            .context("the key file could not be copied")?;
        repomemo_storage::restrict_to_owner(&staging.join(SECRET_KEY_FILE_NAME));
    }
    let (from, to) = (source.join("blobs"), staging.join("blobs"));
    tokio::task::spawn_blocking(move || copy_files(&from, &to, None))
        .await
        .context("the file copy stopped unexpectedly")?
        .context("the stored files could not be copied")?;
    Ok(())
}

/// Starts a restore into a new environment folder in the background.
pub(crate) fn start_restore(state: &AppState, name: &str, environment: &str) -> Result<(), ApiError> {
    if !valid_backup_name(name) || !backups_root(state).join(name).is_dir() {
        return Err(ApiError::not_found("There is no backup with that name."));
    }
    crate::environment::validate_name(environment).map_err(ApiError::bad_request)?;
    if !begin(state, "restore") {
        return Err(ApiError::conflict("A backup or restore is already running."));
    }
    let (state, name, environment) = (state.clone(), name.to_owned(), environment.to_owned());
    tokio::spawn(async move {
        let result = restore_backup(&state, &name, &environment)
            .await
            .map_err(|error| format!("The restore failed: {error:#}"));
        if let Err(message) = &result {
            tracing::error!(error = %message, "Restore failed");
        }
        finish(&state, &result);
    });
    Ok(())
}

/// Deletes one backup.
pub(crate) async fn delete_backup(state: &AppState, name: &str) -> Result<(), ApiError> {
    let folder = backups_root(state).join(name);
    if !valid_backup_name(name) || !folder.is_dir() {
        return Err(ApiError::not_found("There is no backup with that name."));
    }
    if state.backup_status.lock().is_ok_and(|status| status.running) {
        return Err(ApiError::conflict(
            "A backup or restore is running; try again when it is done.",
        ));
    }
    tokio::fs::remove_dir_all(&folder).await.map_err(ApiError::internal)
}

/// Everything the page and the console show.
pub(crate) fn overview(state: &AppState) -> BackupsResponse {
    let root = backups_root(state);
    let settings = state.settings();
    BackupsResponse {
        backups: list_backups(&root),
        status: state
            .backup_status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default(),
        directory: root.to_string_lossy().trim_start_matches(r"\\?\").to_owned(),
        keep: settings.backup_keep,
        interval_hours: settings.backup_interval_hours,
        disk_free_bytes: free_disk_bytes(state.storage.data_dir()),
    }
}

// ---------------------------------------------------------------------------
// Handlers

pub(crate) async fn list(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<BackupsResponse>, ApiError> {
    require_system_admin(&subject)?;
    Ok(Json(overview(&state)))
}

pub(crate) async fn create(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<(axum::http::StatusCode, Json<BackupsResponse>), ApiError> {
    require_full_system_admin(&subject)?;
    require_system_change(&subject)?;
    start_backup(&state, "manual")?;
    audit(&state, &subject, "backup_started", "Started a backup.".to_owned()).await;
    Ok((axum::http::StatusCode::ACCEPTED, Json(overview(&state))))
}

pub(crate) async fn delete(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    UrlPath(name): UrlPath<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    require_full_system_admin(&subject)?;
    require_system_change(&subject)?;
    delete_backup(&state, &name).await?;
    audit(
        &state,
        &subject,
        "backup_deleted",
        format!("Deleted the backup {name}."),
    )
    .await;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub(crate) async fn restore(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    UrlPath(name): UrlPath<String>,
    Json(request): Json<RestoreRequest>,
) -> Result<(axum::http::StatusCode, Json<BackupsResponse>), ApiError> {
    require_full_system_admin(&subject)?;
    require_system_change(&subject)?;
    let environment = request.environment.trim().to_owned();
    start_restore(&state, &name, &environment)?;
    audit(
        &state,
        &subject,
        "backup_restore_started",
        format!("Started restoring the backup {name} into the environment {environment}."),
    )
    .await;
    Ok((axum::http::StatusCode::ACCEPTED, Json(overview(&state))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("repomemo-backup-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn backup_names_are_only_what_the_server_writes() {
        assert!(valid_backup_name("backup-20261014-101500"));
        assert!(!valid_backup_name("backup-../etc"));
        assert!(!valid_backup_name("other"));
        assert!(!valid_backup_name(&format!("backup-{}", "x".repeat(80))));
    }

    #[test]
    fn copying_links_unchanged_files_and_counts_what_it_did() {
        let (live, previous, target) = (folder("live"), folder("previous"), folder("target"));
        std::fs::create_dir_all(live.join("ab").join("cd")).unwrap();
        std::fs::write(live.join("ab").join("cd").join("same"), b"unchanged").unwrap();
        std::fs::write(live.join("ab").join("cd").join("new"), b"fresh content").unwrap();
        std::fs::create_dir_all(previous.join("ab").join("cd")).unwrap();
        std::fs::write(previous.join("ab").join("cd").join("same"), b"unchanged").unwrap();

        let stats = copy_files(&live, &target, Some(&previous)).unwrap();
        assert_eq!(
            stats,
            CopyStats {
                files: 2,
                bytes: 9 + 13,
                missing: 0,
                reused: 1
            }
        );
        assert_eq!(
            std::fs::read(target.join("ab").join("cd").join("new")).unwrap(),
            b"fresh content"
        );
        assert_eq!(
            std::fs::read(target.join("ab").join("cd").join("same")).unwrap(),
            b"unchanged"
        );
        // A folder that does not exist is an empty copy.
        assert_eq!(
            copy_files(&live.join("none"), &folder("none"), None).unwrap(),
            CopyStats::default()
        );
        for path in [live, previous, target] {
            let _ = std::fs::remove_dir_all(path);
        }
    }

    #[test]
    fn only_complete_backups_are_listed_newest_first() {
        let root = folder("list");
        let manifest = |name: &str, created_at: &str| BackupManifest {
            name: name.to_owned(),
            created_at: created_at.to_owned(),
            version: "0.1.75".to_owned(),
            trigger: "manual".to_owned(),
            database_bytes: 1,
            blob_count: 0,
            blob_bytes: 0,
            missing_blobs: 0,
            reused_blobs: 0,
            includes_key: false,
        };
        for (name, at) in [
            ("backup-20261001-000000", "2026-10-01T00:00:00Z"),
            ("backup-20261002-000000", "2026-10-02T00:00:00Z"),
        ] {
            std::fs::create_dir_all(root.join(name)).unwrap();
            std::fs::write(
                root.join(name).join(MANIFEST_FILE),
                serde_json::to_vec(&manifest(name, at)).unwrap(),
            )
            .unwrap();
        }
        std::fs::create_dir_all(root.join("backup-20261003-000000.partial")).unwrap();
        std::fs::create_dir_all(root.join("backup-20261004-000000")).unwrap();
        let listed = list_backups(&root)
            .into_iter()
            .map(|backup| backup.name)
            .collect::<Vec<_>>();
        assert_eq!(listed, ["backup-20261002-000000", "backup-20261001-000000"]);
        let _ = std::fs::remove_dir_all(root);
    }
}
