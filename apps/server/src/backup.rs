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

use crate::backup_crypto::{BackupCipher, BackupEncryption, KeyError, SEALED_SUFFIX};
use crate::quotas::free_disk_bytes;
use crate::system::{audit, require_full_system_admin, require_system_admin, require_system_change};
use crate::{ApiError, AppState, AuthenticatedSubject};

/// The folder inside the data folder that holds backups.
const MANIFEST_FILE: &str = "manifest.json";
const COPY_RECORD_FILE: &str = "copy.json";
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
    /// How the files were sealed; absent for a plain backup. Holds nothing that can open the backup.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encryption: Option<BackupEncryption>,
    /// Files in the backup folder (the manifest excepted) and their total size:
    /// what a second copy must add up to. Zero for backups made before V0.2.43.
    #[serde(default)]
    pub files: u64,
    #[serde(default)]
    pub bytes: u64,
    /// Where the second copy went, once it was made and checked. Read from the
    /// backup's `copy.json` when listing; never part of the manifest file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy: Option<BackupCopyRecord>,
    /// Which configured passphrase opens a sealed backup: `current`, `previous` or `neither`. Worked out when
    /// listing; never part of the manifest file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed_with: Option<String>,
}

/// Written beside a backup (`copy.json`) when its second copy was made and checked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackupCopyRecord {
    pub destination: String,
    pub copied_at: String,
    pub files: u64,
    pub bytes: u64,
    /// Files linked to the previous copy instead of sent again.
    pub linked: u64,
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
    /// Whether new backups are sealed (`REPOMEMO_BACKUP_KEY` is set).
    pub encryption_enabled: bool,
    /// Whether the key before the last change is kept (`REPOMEMO_BACKUP_KEY_PREVIOUS`), so that backups made with
    /// it can be opened and sealed again.
    pub previous_key_set: bool,
    /// The folder every backup is copied to, when one is set.
    pub copy_directory: Option<String>,
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
    state.storage.backup_dir().to_path_buf()
}

/// What a folder copy did.
#[derive(Debug, Default, PartialEq, Eq)]
struct CopyStats {
    files: u64,
    bytes: u64,
    missing: u64,
    reused: u64,
}

/// How [`copy_files`] treats the bytes it moves.
#[derive(Clone, Copy)]
enum CopyMode<'a> {
    /// A plain copy.
    Plain,
    /// Seals each file into `<name>.enc`.
    Seal(&'a BackupCipher),
    /// Opens each `<name>.enc` into `<name>`.
    Open(&'a BackupCipher),
}

/// A path with `suffix` added to its file name.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().map(|name| name.to_os_string()).unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

/// Copies the files under `from` into `to`. A file that `reuse_from` has at
/// the same place and size is linked to it instead of copied (a sealed copy
/// is linked only to a sealed file of the size it would have). A file that
/// vanishes while the copy runs (deleted by garbage collection) is counted.
fn copy_files(from: &Path, to: &Path, reuse_from: Option<&Path>, mode: CopyMode<'_>) -> std::io::Result<CopyStats> {
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
            // Where the copy goes, and (for a sealed copy) the size it will have.
            let target_child = match mode {
                CopyMode::Plain => child.clone(),
                CopyMode::Seal(_) => with_suffix(&child, SEALED_SUFFIX),
                CopyMode::Open(_) => match child.to_string_lossy().strip_suffix(SEALED_SUFFIX) {
                    Some(plain) => PathBuf::from(plain),
                    None => continue,
                },
            };
            let target = to.join(&target_child);
            let Ok(size) = std::fs::metadata(&source).map(|metadata| metadata.len()) else {
                stats.missing += 1;
                continue;
            };
            let expected_len = match mode {
                CopyMode::Seal(_) => BackupCipher::sealed_len(size),
                _ => size,
            };
            let reusable = reuse_from
                .map(|previous| previous.join(&target_child))
                .filter(|previous| std::fs::metadata(previous).is_ok_and(|metadata| metadata.len() == expected_len));
            if let Some(previous) = reusable {
                if std::fs::hard_link(&previous, &target).is_ok() {
                    stats.files += 1;
                    stats.bytes += size;
                    stats.reused += 1;
                    continue;
                }
            }
            let copied = match mode {
                CopyMode::Plain => std::fs::copy(&source, &target),
                CopyMode::Seal(cipher) => cipher.seal_file(&source, &target).map_err(into_io_error),
                CopyMode::Open(cipher) => cipher.open_file(&source, &target).map(|()| size).map_err(into_io_error),
            };
            match copied {
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

/// An error from sealing or opening, as an I/O error that keeps "not found"
/// (a file deleted while the copy ran) apart from the rest.
fn into_io_error(error: anyhow::Error) -> std::io::Error {
    match error.downcast_ref::<std::io::Error>() {
        Some(io) if io.kind() == std::io::ErrorKind::NotFound => std::io::Error::from(std::io::ErrorKind::NotFound),
        _ => std::io::Error::other(format!("{error:#}")),
    }
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
            if !(entry.file_name().to_string_lossy() == manifest.name && valid_backup_name(&manifest.name)) {
                return None;
            }
            let mut manifest = manifest;
            manifest.copy = std::fs::read_to_string(entry.path().join(COPY_RECORD_FILE))
                .ok()
                .and_then(|text| serde_json::from_str(&text).ok());
            Some(manifest)
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
    // Chosen first, so a bad key fails the backup before any file is copied.
    let cipher = choose_cipher(state, &root).await?;
    tokio::fs::create_dir_all(&partial).await?;
    let outcome = fill_backup(state, &root, &partial, &name, trigger, cipher).await;
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

/// The cipher new backups are sealed with: none when `REPOMEMO_BACKUP_KEY` is
/// not set. The salt of the newest backup is kept while the passphrase still
/// opens it, so unchanged files keep sealing to the same bytes.
async fn choose_cipher(state: &AppState, root: &Path) -> anyhow::Result<Option<std::sync::Arc<BackupCipher>>> {
    use anyhow::Context;
    let Some(passphrase) = state.backup_key.clone() else {
        return Ok(None);
    };
    let previous = list_backups(root)
        .into_iter()
        .next()
        .and_then(|backup| backup.encryption);
    let cipher = tokio::task::spawn_blocking(move || {
        if let Some(previous) = previous {
            if let Ok(cipher) = BackupCipher::open(&passphrase, &previous) {
                return Ok(cipher);
            }
        }
        BackupCipher::create(&passphrase)
    })
    .await
    .context("the backup key could not be prepared")?
    .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(Some(std::sync::Arc::new(cipher)))
}

async fn fill_backup(
    state: &AppState,
    root: &Path,
    partial: &Path,
    name: &str,
    trigger: &str,
    cipher: Option<std::sync::Arc<BackupCipher>>,
) -> anyhow::Result<BackupManifest> {
    use anyhow::Context;
    // Taken first: every stored file the snapshot names exists by then.
    state.storage.snapshot_database(&partial.join(DATABASE_FILE)).await?;
    let database_bytes = tokio::fs::metadata(partial.join(DATABASE_FILE)).await?.len();
    if let Some(cipher) = cipher.clone() {
        // The plain snapshot is sealed and removed at once.
        let plain = partial.join(DATABASE_FILE);
        let sealed = partial.join(format!("{DATABASE_FILE}{SEALED_SUFFIX}"));
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            cipher.seal_file(&plain, &sealed)?;
            std::fs::remove_file(&plain)?;
            Ok(())
        })
        .await
        .context("the sealing stopped unexpectedly")?
        .context("the database could not be sealed")?;
    }

    // Unchanged files are shared with the backup before, but only one sealed
    // the same way (the same salt makes the same key).
    let description = cipher.as_ref().map(|cipher| cipher.description());
    let previous = list_backups(root)
        .into_iter()
        .next()
        .filter(|backup| {
            backup.encryption.as_ref().map(|sealed| &sealed.salt) == description.as_ref().map(|sealed| &sealed.salt)
        })
        .map(|backup| root.join(backup.name));
    let (source, target) = (state.storage.blob_dir().to_path_buf(), partial.join("blobs"));
    let blob_cipher = cipher.clone();
    let stats = tokio::task::spawn_blocking(move || {
        let mode = match blob_cipher.as_deref() {
            Some(cipher) => CopyMode::Seal(cipher),
            None => CopyMode::Plain,
        };
        copy_files(
            &source,
            &target,
            previous.map(|folder| folder.join("blobs")).as_deref(),
            mode,
        )
    })
    .await
    .context("the file copy stopped unexpectedly")?
    .context("the stored files could not be copied")?;

    let key = state.storage.data_dir().join(SECRET_KEY_FILE_NAME);
    let includes_key = match cipher.clone() {
        Some(cipher) => {
            let sealed = partial.join(format!("{SECRET_KEY_FILE_NAME}{SEALED_SUFFIX}"));
            let sealed_to = sealed.clone();
            let present = key.exists()
                && tokio::task::spawn_blocking(move || cipher.seal_file(&key, &sealed_to))
                    .await
                    .context("the sealing stopped unexpectedly")?
                    .context("the key file could not be sealed")
                    .is_ok();
            if present {
                repomemo_storage::restrict_to_owner(&sealed);
            }
            present
        }
        None => {
            let copied = tokio::fs::copy(&key, partial.join(SECRET_KEY_FILE_NAME)).await.is_ok();
            if copied {
                // A copy takes the folder's permissions on Windows: make it owner only, like the original.
                repomemo_storage::restrict_to_owner(&partial.join(SECRET_KEY_FILE_NAME));
            }
            copied
        }
    };

    let (files, bytes) = crate::backup_copy::folder_totals(partial)?;
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
        encryption: description,
        files,
        bytes,
        copy: None,
        sealed_with: None,
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
                let message = format!(
                    "Backed up to {} ({} files, {} reused).",
                    manifest.name, manifest.blob_count, manifest.reused_blobs
                );
                // The second copy is part of the job: a share that is down is a failure to see.
                match copy_pending(&state).await {
                    None => Ok(message),
                    Some(Ok(copied)) => Ok(format!("{message} {copied}")),
                    Some(Err(failed)) => Err(format!("{message} {failed}")),
                }
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
    // A copy that failed (the share was down) is tried again on each pass.
    if state.backup_copy_dir.is_some() && copies_waiting(state) {
        let _ = start_copy(state);
    }
    let hours = state.settings().backup_interval_hours;
    if hours == 0 {
        return;
    }
    let newest = list_backups(&backups_root(state)).into_iter().next();
    let due = newest.is_none_or(|backup| {
        chrono::DateTime::parse_from_rfc3339(&backup.created_at)
            .map(|created| Utc::now().signed_duration_since(created) >= chrono::Duration::hours(hours as i64))
            .unwrap_or(true)
    });
    if due && start_backup(state, "scheduled").is_err() {
        tracing::debug!("A scheduled backup waits for the one that is running");
    }
}

/// Whether a backup within the number kept has no checked second copy yet.
fn copies_waiting(state: &AppState) -> bool {
    let keep = state.settings().backup_keep.max(1) as usize;
    list_backups(&backups_root(state))
        .iter()
        .take(keep)
        .any(|backup| backup.copy.is_none())
}

/// Copies the backups that have no checked second copy yet (oldest first, so
/// each can share files with the one before), then trims the destination to
/// the number kept. `None` when no second copy is set up.
async fn copy_pending(state: &AppState) -> Option<Result<String, String>> {
    let destination = state.backup_copy_dir.clone()?;
    let root = backups_root(state);
    let keep = state.settings().backup_keep.max(1) as usize;
    let mut waiting = list_backups(&root)
        .into_iter()
        .take(keep)
        .filter(|backup| backup.copy.is_none())
        .collect::<Vec<_>>();
    waiting.reverse();
    let shown = destination.to_string_lossy().trim_start_matches(r"\\?\").to_owned();
    if waiting.is_empty() {
        return Some(Ok(format!("The second copy in {shown} is up to date.")));
    }
    let mut copied = 0;
    for backup in waiting {
        let (source, target) = (root.join(&backup.name), destination.clone());
        let expected = (backup.files > 0).then_some((backup.files, backup.bytes));
        let outcome =
            tokio::task::spawn_blocking(move || crate::backup_copy::copy_backup(&source, &target, expected)).await;
        let report = match outcome {
            Ok(Ok(report)) => report,
            Ok(Err(error)) => {
                tracing::error!(backup = %backup.name, error = %format!("{error:#}"), "A second copy failed");
                return Some(Err(format!("The copy to {shown} failed: {error:#}")));
            }
            Err(error) => return Some(Err(format!("The copy to {shown} stopped unexpectedly: {error}"))),
        };
        let record = BackupCopyRecord {
            destination: shown.clone(),
            copied_at: Utc::now().to_rfc3339(),
            files: report.files,
            bytes: report.bytes,
            linked: report.linked,
        };
        let written = match serde_json::to_vec_pretty(&record) {
            Ok(bytes) => tokio::fs::write(root.join(&backup.name).join(COPY_RECORD_FILE), bytes).await,
            Err(error) => Err(std::io::Error::other(error)),
        };
        if let Err(error) = written {
            return Some(Err(format!(
                "The copy to {shown} was made but could not be recorded: {error}"
            )));
        }
        tracing::info!(target: "audit", backup = %backup.name, destination = %shown, files = report.files, "Backup copied and checked");
        copied += 1;
    }
    let trimmed = destination.clone();
    let _ = tokio::task::spawn_blocking(move || crate::backup_copy::prune_copies(&trimmed, keep)).await;
    Some(Ok(format!(
        "Copied {copied} backup{} to {shown} and checked {}.",
        if copied == 1 { "" } else { "s" },
        if copied == 1 { "it" } else { "them" }
    )))
}

/// Starts copying the backups that wait for their second copy.
pub(crate) fn start_copy(state: &AppState) -> Result<(), ApiError> {
    if state.backup_copy_dir.is_none() {
        return Err(ApiError::bad_request(
            "No second copy is set up. Set REPOMEMO_BACKUP_COPY_DIR to a folder on another disk or a share.",
        ));
    }
    if !begin(state, "copy") {
        return Err(ApiError::conflict("A backup or restore is already running."));
    }
    let state = state.clone();
    tokio::spawn(async move {
        let result = copy_pending(&state).await.unwrap_or_else(|| Ok(String::new()));
        finish(&state, &result);
    });
    Ok(())
}

async fn restore_backup(state: &AppState, name: &str, environment: &str) -> anyhow::Result<String> {
    use anyhow::Context;
    let source = backups_root(state).join(name);
    let manifest = read_manifest(&source).context("The backup is incomplete or its manifest cannot be read.")?;
    // A sealed backup needs its key, and the key is checked first: a wrong
    // one is refused before any folder is made.
    let (cipher, with_previous) = match manifest.encryption.clone() {
        None => (None, false),
        Some(description) => {
            let (current, previous) = (state.backup_key.clone(), state.backup_previous_key.clone());
            let (opened, with_previous) = tokio::task::spawn_blocking(move || {
                open_with_configured(current.as_deref(), previous.as_deref(), &description)
            })
            .await
            .context("the backup key could not be prepared")?
            .map_err(|error: KeyError| anyhow::anyhow!("{error}"))?;
            (Some(std::sync::Arc::new(opened)), with_previous)
        }
    };
    let root = crate::environment::environments_root()?;
    let target = root.join(environment);
    let staging = root.join(format!("{environment}.restoring"));
    if target.exists() || staging.exists() {
        anyhow::bail!("An environment named {environment} exists already; choose another name.");
    }
    tokio::fs::create_dir_all(&staging).await?;
    let copied = copy_into(&source, &staging, cipher).await;
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
        db_dir: None,
        blob_dir: None,
        backup_dir: None,
        master_key: crate::config::env_secret("REPOMEMO_SECRET_KEY").ok().flatten(),
        read_connections: 1,
        vector_index_mb: 0,
        read_cache_mb: 0,
        read_mmap_mb: 0,
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
        "Restored {name} (made by {}) as the environment {environment}: {} workspaces, {} files, {} people{}. Attach it from the environment menu.",
        manifest.version,
        statistics.workspace_count,
        statistics.artifact_count,
        statistics.user_count,
        if with_previous {
            "; it was opened with REPOMEMO_BACKUP_KEY_PREVIOUS, so seal it again to retire that key"
        } else {
            ""
        }
    ))
}

async fn copy_into(source: &Path, staging: &Path, cipher: Option<std::sync::Arc<BackupCipher>>) -> anyhow::Result<()> {
    use anyhow::Context;
    match cipher.clone() {
        None => {
            tokio::fs::copy(source.join(DATABASE_FILE), staging.join(DATABASE_FILE))
                .await
                .context("the database could not be copied")?;
            if source.join(SECRET_KEY_FILE_NAME).exists() {
                tokio::fs::copy(source.join(SECRET_KEY_FILE_NAME), staging.join(SECRET_KEY_FILE_NAME))
                    .await
                    .context("the key file could not be copied")?;
                repomemo_storage::restrict_to_owner(&staging.join(SECRET_KEY_FILE_NAME));
            }
        }
        Some(cipher) => {
            let (from, to) = (source.to_path_buf(), staging.to_path_buf());
            tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                cipher.open_file(
                    &from.join(format!("{DATABASE_FILE}{SEALED_SUFFIX}")),
                    &to.join(DATABASE_FILE),
                )?;
                let sealed_key = from.join(format!("{SECRET_KEY_FILE_NAME}{SEALED_SUFFIX}"));
                if sealed_key.exists() {
                    cipher.open_file(&sealed_key, &to.join(SECRET_KEY_FILE_NAME))?;
                    repomemo_storage::restrict_to_owner(&to.join(SECRET_KEY_FILE_NAME));
                }
                Ok(())
            })
            .await
            .context("the opening stopped unexpectedly")?
            .context("the database could not be opened")?;
        }
    }
    let (from, to) = (source.join("blobs"), staging.join("blobs"));
    tokio::task::spawn_blocking(move || {
        let mode = match cipher.as_deref() {
            Some(cipher) => CopyMode::Open(cipher),
            None => CopyMode::Plain,
        };
        copy_files(&from, &to, None, mode)
    })
    .await
    .context("the file copy stopped unexpectedly")?
    .context("the stored files could not be copied")?;
    Ok(())
}

/// Opens a backup with the current key, or with the previous one when that is what it was made with; the second
/// answer says which.
fn open_with_configured(
    current: Option<&str>,
    previous: Option<&str>,
    description: &BackupEncryption,
) -> Result<(BackupCipher, bool), KeyError> {
    let Some(current) = current else {
        return Err(KeyError::Unusable(
            "This backup is encrypted. Set REPOMEMO_BACKUP_KEY to the key it was made with and restart the server."
                .to_owned(),
        ));
    };
    match BackupCipher::open(current, description) {
        Ok(cipher) => Ok((cipher, false)),
        Err(KeyError::Mismatch) => match previous {
            Some(previous) => BackupCipher::open(previous, description).map(|cipher| (cipher, true)),
            None => Err(KeyError::Mismatch),
        },
        Err(other) => Err(other),
    }
}

/// Which configured passphrase opens a way of sealing.
fn key_use(current: Option<&str>, previous: Option<&str>, description: &BackupEncryption) -> &'static str {
    if current.is_some_and(|key| BackupCipher::open(key, description).is_ok()) {
        "current"
    } else if previous.is_some_and(|key| BackupCipher::open(key, description).is_ok()) {
        "previous"
    } else {
        "neither"
    }
}

/// Fills in `sealed_with` of the sealed backups. A key takes a moment to derive, so it is done once for each way
/// of sealing (backups made one after the other share it) and remembered.
async fn tell_keys(state: &AppState, backups: &mut [BackupManifest]) {
    for backup in backups.iter_mut() {
        let Some(description) = backup.encryption.clone() else {
            continue;
        };
        let memo = format!("{}:{}", description.salt, description.key_check);
        let known = state
            .backup_key_use
            .lock()
            .ok()
            .and_then(|known| known.get(&memo).copied());
        let found = match known {
            Some(found) => found,
            None => {
                let (current, previous) = (state.backup_key.clone(), state.backup_previous_key.clone());
                let found =
                    tokio::task::spawn_blocking(move || key_use(current.as_deref(), previous.as_deref(), &description))
                        .await
                        .unwrap_or("neither");
                if let Ok(mut known) = state.backup_key_use.lock() {
                    known.insert(memo, found);
                }
                found
            }
        };
        backup.sealed_with = Some(found.to_owned());
    }
}

/// Seals every file of the backup `from`, which `old` sealed, again with `new` into `to`; returns how many files.
/// The manifest and the record of a second copy are left to the caller.
fn reseal_folder(old: &BackupCipher, new: &BackupCipher, from: &Path, to: &Path) -> anyhow::Result<u64> {
    std::fs::create_dir_all(to)?;
    let mut files = 0;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let (path, name) = (entry.path(), entry.file_name().to_string_lossy().into_owned());
        let target = to.join(&name);
        if entry.file_type()?.is_dir() {
            files += reseal_folder(old, new, &path, &target)?;
        } else if name == MANIFEST_FILE || name == COPY_RECORD_FILE {
            continue;
        } else if name.ends_with(SEALED_SUFFIX) {
            old.reseal_file(new, &path, &target)?;
            if name == format!("{SECRET_KEY_FILE_NAME}{SEALED_SUFFIX}") {
                repomemo_storage::restrict_to_owner(&target);
            }
            files += 1;
        } else {
            std::fs::copy(&path, &target)?;
            files += 1;
        }
    }
    Ok(files)
}

/// Seals a backup again with the current key: its files are read with the previous key and written under
/// `<name>.resealing.partial`, then the new folder takes the old one's place. Nothing of the old backup is
/// touched until the new one is whole.
async fn reseal_backup(state: &AppState, name: &str) -> anyhow::Result<String> {
    use anyhow::Context;
    let root = backups_root(state);
    let source = root.join(name);
    let manifest = read_manifest(&source).context("The backup is incomplete or its manifest cannot be read.")?;
    let description = manifest
        .encryption
        .clone()
        .context("This backup is not sealed, so there is nothing to seal again.")?;
    let current = state
        .backup_key
        .clone()
        .context("Set REPOMEMO_BACKUP_KEY to the new key and restart the server.")?;
    let previous = state.backup_previous_key.clone();
    // The keys are checked first: it must be the previous one that opens the backup.
    let (old, new) = {
        let description = description.clone();
        tokio::task::spawn_blocking(move || -> Result<(BackupCipher, BackupCipher), String> {
            if BackupCipher::open(&current, &description).is_ok() {
                return Err("This backup is sealed with the current key already.".to_owned());
            }
            let Some(previous) = previous else {
                return Err(
                    "Set REPOMEMO_BACKUP_KEY_PREVIOUS to the key this backup was made with and restart the server."
                        .to_owned(),
                );
            };
            let old = BackupCipher::open(&previous, &description).map_err(|error| error.to_string())?;
            let new = BackupCipher::create(&current).map_err(|error| error.to_string())?;
            Ok((old, new))
        })
        .await
        .context("the backup keys could not be prepared")?
        .map_err(|message| anyhow::anyhow!(message))?
    };
    // The new folder is whole beside the old one before it replaces it.
    if let Some(free) = free_disk_bytes(state.storage.backup_dir()) {
        if free < manifest.bytes.saturating_add(manifest.database_bytes) {
            anyhow::bail!(
                "There is not enough free disk to seal {name} again: it needs about as much room as the backup takes."
            );
        }
    }
    let staging = root.join(format!("{name}.resealing{PARTIAL_SUFFIX}"));
    let replaced = root.join(format!("{name}.replaced{PARTIAL_SUFFIX}"));
    for leftover in [&staging, &replaced] {
        if leftover.exists() {
            let _ = tokio::fs::remove_dir_all(leftover).await;
        }
    }
    let new = std::sync::Arc::new(new);
    let (from, to, sealer) = (source.clone(), staging.clone(), new.clone());
    let outcome = async {
        tokio::task::spawn_blocking(move || reseal_folder(&old, &sealer, &from, &to))
            .await
            .context("the sealing stopped unexpectedly")??;
        let (files, bytes) = crate::backup_copy::folder_totals(&staging)?;
        let resealed = BackupManifest {
            encryption: Some(new.description()),
            files,
            bytes,
            copy: None,
            sealed_with: None,
            ..manifest.clone()
        };
        tokio::fs::write(staging.join(MANIFEST_FILE), serde_json::to_vec_pretty(&resealed)?).await?;
        anyhow::Ok(files)
    }
    .await;
    let files = match outcome {
        Ok(files) => files,
        Err(error) => {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(error);
        }
    };
    rename_with_retries(&source, &replaced)
        .await
        .context("the backup could not be set aside")?;
    if let Err(error) = rename_with_retries(&staging, &source).await {
        // Put the old one back: the backup is never left without a folder.
        let _ = rename_with_retries(&replaced, &source).await;
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(anyhow::Error::new(error).context("the sealed-again backup could not be put in place"));
    }
    let _ = tokio::fs::remove_dir_all(&replaced).await;
    Ok(format!(
        "Sealed {name} again with the current key ({files} files). It no longer needs REPOMEMO_BACKUP_KEY_PREVIOUS."
    ))
}

/// Starts sealing a backup again with the current key, in the background.
pub(crate) fn start_reseal(state: &AppState, name: &str) -> Result<(), ApiError> {
    if !valid_backup_name(name) || !backups_root(state).join(name).is_dir() {
        return Err(ApiError::not_found("There is no backup with that name."));
    }
    if !begin(state, "reseal") {
        return Err(ApiError::conflict("A backup or restore is already running."));
    }
    let (state, name) = (state.clone(), name.to_owned());
    tokio::spawn(async move {
        let result = reseal_backup(&state, &name)
            .await
            .map_err(|error| format!("Sealing the backup again failed: {error:#}"));
        match &result {
            Ok(message) => tracing::info!(target: "audit", backup = %name, "{message}"),
            Err(message) => tracing::error!(error = %message, "Sealing a backup again failed"),
        }
        finish(&state, &result);
    });
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
        disk_free_bytes: free_disk_bytes(state.storage.backup_dir()),
        encryption_enabled: state.backup_key.is_some(),
        previous_key_set: state.backup_previous_key.is_some(),
        copy_directory: state
            .backup_copy_dir
            .as_ref()
            .map(|folder| folder.to_string_lossy().trim_start_matches(r"\\?\").to_owned()),
    }
}

/// What the page and the console show, with which key opens each sealed backup.
pub(crate) async fn overview_with_keys(state: &AppState) -> BackupsResponse {
    let mut response = overview(state);
    tell_keys(state, &mut response.backups).await;
    response
}

// ---------------------------------------------------------------------------
// Handlers

pub(crate) async fn list(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<Json<BackupsResponse>, ApiError> {
    require_system_admin(&subject)?;
    Ok(Json(overview_with_keys(&state).await))
}

pub(crate) async fn create(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<(axum::http::StatusCode, Json<BackupsResponse>), ApiError> {
    require_full_system_admin(&subject)?;
    require_system_change(&subject)?;
    start_backup(&state, "manual")?;
    audit(&state, &subject, "backup_started", "Started a backup.".to_owned()).await;
    Ok((axum::http::StatusCode::ACCEPTED, Json(overview_with_keys(&state).await)))
}

/// Copies the backups that have no checked second copy yet, now.
pub(crate) async fn copy_now(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
) -> Result<(axum::http::StatusCode, Json<BackupsResponse>), ApiError> {
    require_full_system_admin(&subject)?;
    require_system_change(&subject)?;
    start_copy(&state)?;
    audit(
        &state,
        &subject,
        "backup_copy_started",
        "Started copying the backups to the second location.".to_owned(),
    )
    .await;
    Ok((axum::http::StatusCode::ACCEPTED, Json(overview_with_keys(&state).await)))
}

/// Seals a backup that the previous key made again with the current key.
pub(crate) async fn reseal(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    UrlPath(name): UrlPath<String>,
) -> Result<(axum::http::StatusCode, Json<BackupsResponse>), ApiError> {
    require_full_system_admin(&subject)?;
    require_system_change(&subject)?;
    start_reseal(&state, &name)?;
    audit(
        &state,
        &subject,
        "backup_resealed",
        format!("Started sealing the backup {name} again with the current key."),
    )
    .await;
    Ok((axum::http::StatusCode::ACCEPTED, Json(overview_with_keys(&state).await)))
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
    Ok((axum::http::StatusCode::ACCEPTED, Json(overview_with_keys(&state).await)))
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

        let stats = copy_files(&live, &target, Some(&previous), CopyMode::Plain).unwrap();
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
            copy_files(&live.join("none"), &folder("none"), None, CopyMode::Plain).unwrap(),
            CopyStats::default()
        );
        for path in [live, previous, target] {
            let _ = std::fs::remove_dir_all(path);
        }
    }

    #[test]
    fn sealed_copies_open_back_to_the_same_files_and_unchanged_ones_are_linked() {
        let (live, first, second, opened) = (
            folder("seal-live"),
            folder("seal-first"),
            folder("seal-second"),
            folder("seal-opened"),
        );
        std::fs::create_dir_all(live.join("ab")).unwrap();
        std::fs::write(live.join("ab").join("same"), b"unchanged content").unwrap();
        std::fs::write(live.join("ab").join("empty"), b"").unwrap();
        let cipher = BackupCipher::create("a long enough passphrase for tests").unwrap();

        let stats = copy_files(&live, &first, None, CopyMode::Seal(&cipher)).unwrap();
        assert_eq!((stats.files, stats.bytes, stats.reused), (2, 17, 0));
        assert!(first.join("ab").join("same.enc").is_file());
        assert!(
            !first.join("ab").join("same").exists(),
            "no plain copy is left beside the sealed one"
        );
        assert!(!std::fs::read(first.join("ab").join("same.enc"))
            .unwrap()
            .windows(9)
            .any(|window| window == b"unchanged"));

        // The next backup, sealed the same way, shares what has not changed.
        let stats = copy_files(&live, &second, Some(&first), CopyMode::Seal(&cipher)).unwrap();
        assert_eq!((stats.files, stats.reused), (2, 2));

        let stats = copy_files(&first, &opened, None, CopyMode::Open(&cipher)).unwrap();
        assert_eq!(stats.files, 2);
        assert_eq!(
            std::fs::read(opened.join("ab").join("same")).unwrap(),
            b"unchanged content"
        );
        assert_eq!(std::fs::read(opened.join("ab").join("empty")).unwrap(), b"");

        // Another key opens nothing.
        let other = BackupCipher::create("another passphrase that is long").unwrap();
        let wrong = folder("seal-wrong");
        assert!(copy_files(&first, &wrong, None, CopyMode::Open(&other)).is_err());
        for path in [live, first, second, opened, wrong] {
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
            encryption: None,
            files: 0,
            bytes: 0,
            copy: None,
            sealed_with: None,
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

    #[test]
    fn a_backup_opens_with_the_current_key_or_the_previous_one_and_says_which() {
        let (current, previous) = ("the current backup passphrase", "the previous backup passphrase");
        let by_previous = BackupCipher::create(previous).unwrap().description();
        let by_current = BackupCipher::create(current).unwrap().description();

        let (_, used_previous) = open_with_configured(Some(current), Some(previous), &by_previous).unwrap();
        assert!(used_previous);
        let (_, used_previous) = open_with_configured(Some(current), Some(previous), &by_current).unwrap();
        assert!(!used_previous, "the current key is tried first");
        // Without the previous key the old backup stays shut, and with neither nothing opens.
        assert!(matches!(
            open_with_configured(Some(current), None, &by_previous),
            Err(KeyError::Mismatch)
        ));
        assert!(matches!(
            open_with_configured(Some(current), Some("another passphrase, not either"), &by_previous),
            Err(KeyError::Mismatch)
        ));
        assert!(matches!(
            open_with_configured(None, None, &by_current),
            Err(KeyError::Unusable(_))
        ));

        assert_eq!(key_use(Some(current), Some(previous), &by_current), "current");
        assert_eq!(key_use(Some(current), Some(previous), &by_previous), "previous");
        assert_eq!(key_use(Some(current), None, &by_previous), "neither");
        assert_eq!(key_use(None, None, &by_current), "neither");
    }

    #[test]
    fn a_backup_folder_is_sealed_again_file_by_file_and_the_manifest_is_left_to_the_caller() {
        let (live, sealed, again, opened) = (
            folder("reseal-live"),
            folder("reseal-old"),
            folder("reseal-new"),
            folder("reseal-opened"),
        );
        std::fs::create_dir_all(live.join("ab")).unwrap();
        std::fs::write(live.join("ab").join("one"), b"first stored file").unwrap();
        std::fs::write(live.join("ab").join("empty"), b"").unwrap();
        std::fs::write(live.join("repomemo.sqlite"), vec![7u8; 200_000]).unwrap();
        let old = BackupCipher::create("the previous backup passphrase").unwrap();
        let new = BackupCipher::create("the current backup passphrase").unwrap();
        copy_files(&live, &sealed, None, CopyMode::Seal(&old)).unwrap();
        std::fs::write(sealed.join(MANIFEST_FILE), b"{}").unwrap();
        std::fs::write(sealed.join(COPY_RECORD_FILE), b"{}").unwrap();

        let files = reseal_folder(&old, &new, &sealed, &again).unwrap();
        assert_eq!(files, 3);
        assert!(!again.join(MANIFEST_FILE).exists());
        assert!(!again.join(COPY_RECORD_FILE).exists());
        assert!(again.join("ab").join("one.enc").is_file());
        // Only the new key opens the new files, and they hold what the old ones held.
        assert!(copy_files(&again, &folder("reseal-wrong"), None, CopyMode::Open(&old)).is_err());
        copy_files(&again, &opened, None, CopyMode::Open(&new)).unwrap();
        assert_eq!(
            std::fs::read(opened.join("ab").join("one")).unwrap(),
            b"first stored file"
        );
        assert_eq!(std::fs::read(opened.join("ab").join("empty")).unwrap(), b"");
        assert_eq!(
            std::fs::read(opened.join("repomemo.sqlite")).unwrap(),
            vec![7u8; 200_000]
        );
        // The old folder is untouched and still opens with the old key.
        assert!(sealed.join("ab").join("one.enc").is_file());
        for path in [live, sealed, again, opened] {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}
