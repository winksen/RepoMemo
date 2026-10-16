//! Import of a workspace archive into a new workspace (roadmap O2b).
//!
//! The archive (see `workspace_export.rs`) is received into a temporary file,
//! never into memory, and read back entry by entry. `dry_run=true` reads it
//! all the same, verifies every checksum and checks the records against each
//! other and against this server's limits, and writes nothing. A real import
//! stores the files, creates the workspace and writes its records in one
//! transaction; if that fails the workspace is removed again.
//!
//! An archive is only as trustworthy as whoever wrote it, so the importer
//! gets what it holds and nothing more: records are credited to the importer
//! (the archive's names are kept in the text), no account is matched by
//! address, and nothing in it can carry a secret, a server path or a role.

use std::collections::{HashMap, HashSet};
use std::io::BufReader;
use std::path::{Path as FsPath, PathBuf};

use axum::{
    body::Body,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use repomemo_api::ArchiveReader;
use repomemo_domain::{
    ArchiveImportReport, ArchiveManifest, WorkspaceArchiveData, ARCHIVE_BLOB_PREFIX, ARCHIVE_DATA_ENTRY,
    ARCHIVE_MANIFEST_ENTRY,
};
use repomemo_storage::StorageEngine;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;

use crate::quotas::{format_bytes, free_disk_bytes};
use crate::{
    consume_heavy_quota, map_storage_error, record_workspace_activity, ApiError, AppState, AuthenticatedSubject,
};

const MIB: u64 = 1024 * 1024;
/// The manifest is small; a larger one is not one.
const MANIFEST_MAX_BYTES: u64 = 8 * MIB;
/// The records of a workspace, as JSON.
const DATA_MAX_BYTES: u64 = 256 * MIB;
/// One stored file, as the server accepts it for upload at the most.
const BLOB_MAX_BYTES: u64 = 1024 * MIB;
/// Passages of at most this many files are found again during the import
/// (they have to be indexed to be found); the rest widen to their file.
const INLINE_INDEX_FILES: usize = 100;
/// Temporary files older than this are leftovers of an interrupted import.
const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Deserialize)]
pub(crate) struct ImportQuery {
    organization_id: String,
    #[serde(default)]
    dry_run: bool,
    /// A name for the new workspace; the archive's own by default.
    name: Option<String>,
}

/// Removes the temporary file however the import ends.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// `POST /v1/workspaces/import?organization_id=&dry_run=&name=`: the body is
/// the archive. 200 with a report for a dry run, 201 with the new workspace.
pub(crate) async fn import_workspace(
    subject: AuthenticatedSubject,
    State(state): State<AppState>,
    Query(query): Query<ImportQuery>,
    body: Body,
) -> Result<Response, ApiError> {
    crate::verification::require_verified(&state, &subject.user_id).await?;
    if !query.dry_run {
        crate::quotas::check_new_workspace(&state, &subject).await?;
    }
    state
        .storage
        .check_can_create_workspace(&subject.user_id, &query.organization_id)
        .await
        .map_err(map_storage_error)?;
    consume_heavy_quota(&state, &subject)?;
    let started = std::time::Instant::now();

    let temporary = receive(&state, body).await?;
    let report = read_archive(&state, &subject, &query, &temporary.0).await?;
    drop(temporary);

    if !report.problems.is_empty() && !query.dry_run {
        return Err(ApiError::bad_request(summarize(&report.problems)));
    }
    crate::record_timing("workspace_import", &query.organization_id, started, report.counts.artifacts as usize);
    let status = if report.dry_run { StatusCode::OK } else { StatusCode::CREATED };
    Ok((status, Json(report)).into_response())
}

fn summarize(problems: &[String]) -> String {
    problems.iter().take(3).cloned().collect::<Vec<_>>().join(" ")
}

/// Streams the request body into a file in the data folder, refusing more
/// than the server accepts.
async fn receive(state: &AppState, body: Body) -> Result<TempFile, ApiError> {
    let directory = state.storage.data_dir().join("imports");
    tokio::fs::create_dir_all(&directory).await.map_err(ApiError::internal)?;
    remove_stale(&directory).await;
    let limit = state.max_import_bytes;
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let stamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let temporary = TempFile(directory.join(format!("import-{stamp}-{sequence}.tmp")));
    let mut file = tokio::fs::File::create(&temporary.0).await.map_err(ApiError::internal)?;
    let mut stream = body.into_data_stream();
    let mut received = 0_u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| ApiError::bad_request("The upload was interrupted."))?;
        received += chunk.len() as u64;
        if received > limit {
            return Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "archive_too_large",
                format!(
                    "This archive is larger than the {} this server imports. An administrator can raise REPOMEMO_MAX_IMPORT_MB.",
                    format_bytes(limit)
                ),
            ));
        }
        file.write_all(&chunk).await.map_err(ApiError::internal)?;
    }
    file.flush().await.map_err(ApiError::internal)?;
    if received == 0 {
        return Err(ApiError::bad_request("Send the archive file as the request body."));
    }
    Ok(temporary)
}

async fn remove_stale(directory: &FsPath) {
    let Ok(mut entries) = tokio::fs::read_dir(directory).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let old = entry
            .metadata()
            .await
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > STALE_AFTER);
        if old {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
}

fn damaged(error: impl std::fmt::Display) -> ApiError {
    ApiError::bad_request(format!("This file is not a readable workspace archive: {error}"))
}

/// Reads and checks the archive; unless `dry_run`, stores its files and
/// creates the workspace.
async fn read_archive(
    state: &AppState,
    subject: &AuthenticatedSubject,
    query: &ImportQuery,
    path: &FsPath,
) -> Result<ArchiveImportReport, ApiError> {
    let file = std::fs::File::open(path).map_err(ApiError::internal)?;
    let mut reader = ArchiveReader::new(BufReader::new(file));

    let (name, bytes) = reader
        .next_entry(MANIFEST_MAX_BYTES)
        .map_err(damaged)?
        .ok_or_else(|| damaged("it is empty"))?;
    if name != ARCHIVE_MANIFEST_ENTRY {
        return Err(damaged("it does not start with a manifest"));
    }
    let manifest: ArchiveManifest = serde_json::from_slice(&bytes).map_err(|_| damaged("its manifest cannot be read"))?;
    let mut problems = manifest.check().problems;
    if !problems.is_empty() {
        // Nothing else in it can be trusted to mean what this server expects.
        return Ok(report(&manifest, None, problems, Vec::new(), query.dry_run));
    }

    let (name, bytes) = reader
        .next_entry(DATA_MAX_BYTES)
        .map_err(damaged)?
        .ok_or_else(|| damaged("its data is missing"))?;
    if name != ARCHIVE_DATA_ENTRY {
        return Err(damaged("its data does not follow its manifest"));
    }
    if bytes.len() as u64 != manifest.data_bytes || StorageEngine::content_hash(&bytes) != manifest.data_sha256 {
        return Err(damaged("its data does not match its checksum, so it was damaged or changed"));
    }
    let data: WorkspaceArchiveData = serde_json::from_slice(&bytes).map_err(|_| damaged("its data cannot be read"))?;
    drop(bytes);
    let check = data.check();
    problems.extend(check.problems);
    let mut warnings = check.warnings;

    // Every file a record names has to be listed, and listed once.
    let mut expected: HashMap<&str, u64> = HashMap::new();
    for blob in &manifest.blobs {
        if expected.insert(blob.hash.as_str(), blob.size_bytes).is_some() {
            problems.push(format!("The file {} is listed twice.", blob.hash));
        }
    }
    let unlisted = data
        .artifacts
        .iter()
        .filter(|artifact| !expected.contains_key(artifact.content_hash.as_str()))
        .count();
    if unlisted > 0 {
        problems.push(format!("{unlisted} file(s) are missing from the archive's list of stored files."));
    }
    problems.extend(room_problems(state, manifest.blob_bytes()));
    if !problems.is_empty() {
        return Ok(report(&manifest, Some(&data), problems, warnings, query.dry_run));
    }

    // The stored files: each checked against the hash it is named by, and
    // kept unless this is a dry run.
    let mut mimes: HashMap<&str, &str> = HashMap::new();
    for artifact in &data.artifacts {
        if let Some(mime) = artifact.mime_type.as_deref() {
            mimes.entry(artifact.content_hash.as_str()).or_insert(mime);
        }
    }
    let mut seen = HashSet::new();
    let mut read = 0_u64;
    let budget = manifest.blob_bytes();
    while let Some((name, bytes)) = reader.next_entry(BLOB_MAX_BYTES).map_err(damaged)? {
        let Some(hash) = name.strip_prefix(ARCHIVE_BLOB_PREFIX) else {
            warnings.push(format!("{name} is not something this server knows and was skipped."));
            continue;
        };
        let Some(size) = expected.get(hash) else {
            problems.push(format!("The archive holds a file ({hash}) its manifest does not list."));
            break;
        };
        read += bytes.len() as u64;
        if read > budget || bytes.len() as u64 != *size || StorageEngine::content_hash(&bytes) != hash {
            problems.push(format!("The stored file {hash} does not match its checksum, so it was damaged or changed."));
            break;
        }
        if seen.insert(hash.to_owned()) && !query.dry_run {
            state
                .storage
                .store_blob(hash, &bytes, mimes.get(hash).copied())
                .await
                .map_err(ApiError::internal)?;
        }
    }
    if problems.is_empty() {
        let missing = expected.keys().filter(|hash| !seen.contains(**hash)).count();
        if missing > 0 {
            problems.push(format!("{missing} stored file(s) listed in the manifest are not in the archive."));
        }
    }
    if !problems.is_empty() || query.dry_run {
        return Ok(report(&manifest, Some(&data), problems, warnings, query.dry_run));
    }

    let mut finished = create_workspace(state, subject, query, &data).await?;
    let mut summary = report(&manifest, Some(&data), problems, warnings, false);
    summary.widened_citations = finished.widened;
    summary.workspace = Some(finished.workspace.take().expect("created above"));
    Ok(summary)
}

fn report(
    manifest: &ArchiveManifest,
    data: Option<&WorkspaceArchiveData>,
    problems: Vec<String>,
    warnings: Vec<String>,
    dry_run: bool,
) -> ArchiveImportReport {
    ArchiveImportReport {
        dry_run,
        ok: problems.is_empty(),
        workspace_name: manifest.workspace_name.clone(),
        archive_format_version: manifest.format_version,
        source_version: manifest.app_version.clone(),
        exported_at: manifest.created_at.clone(),
        counts: manifest.counts.clone(),
        total_bytes: manifest.blob_bytes(),
        people: data.map(WorkspaceArchiveData::people).unwrap_or_default(),
        problems,
        warnings,
        widened_citations: 0,
        workspace: None,
    }
}

/// Whether the archive's files fit: in a workspace's allowance and on the disk.
fn room_problems(state: &AppState, incoming: u64) -> Vec<String> {
    let settings = state.settings();
    let mut problems = Vec::new();
    if settings.workspace_storage_mb > 0 && incoming > settings.workspace_storage_mb * MIB {
        problems.push(format!(
            "The archive holds {} of files, more than the {} a workspace may hold on this server.",
            format_bytes(incoming),
            format_bytes(settings.workspace_storage_mb * MIB)
        ));
    }
    if settings.disk_floor_mb > 0 {
        let free = free_disk_bytes(state.storage.data_dir());
        if free.is_some_and(|free| free < settings.disk_floor_mb * MIB + incoming) {
            problems.push("The server does not have the disk space to store this archive's files.".to_owned());
        }
    }
    problems
}

struct Created {
    workspace: Option<repomemo_domain::SharedWorkspace>,
    widened: u64,
}

async fn create_workspace(
    state: &AppState,
    subject: &AuthenticatedSubject,
    query: &ImportQuery,
    data: &WorkspaceArchiveData,
) -> Result<Created, ApiError> {
    let name = query
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(data.workspace.name.as_str());
    let workspace = state
        .storage
        .create_shared_workspace(&subject.user_id, &query.organization_id, name)
        .await
        .map_err(map_storage_error)?;
    let workspace_id = workspace.workspace.id.clone();
    let imported = match state
        .storage
        .import_workspace_data(&workspace_id, &subject.user_id, data)
        .await
    {
        Ok(imported) => imported,
        Err(error) => {
            // Nothing of a failed import stays.
            let _ = state.storage.delete_workspace(&workspace_id).await;
            return Err(ApiError::internal(error));
        }
    };

    // Passages can be found again once their file is indexed; do that for the
    // cited files now, and let the rest of the workspace index in the background.
    let mut widened = 0_u64;
    let mut cited: Vec<&str> = imported
        .pending_passages
        .iter()
        .map(|passage| passage.artifact_id.as_str())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    cited.sort_unstable();
    let mut chunks_by_file = HashMap::new();
    for artifact_id in cited.into_iter().take(INLINE_INDEX_FILES) {
        if state.core.index_artifact(artifact_id.to_owned()).await.is_ok() {
            if let Ok(chunks) = state.storage.list_chunks_for_artifact(artifact_id).await {
                chunks_by_file.insert(artifact_id.to_owned(), chunks);
            }
        }
    }
    for passage in &imported.pending_passages {
        let found = chunks_by_file.get(&passage.artifact_id).and_then(|chunks| {
            chunks
                .iter()
                .find(|chunk| chunk.content_hash == passage.chunk.content_hash)
                .or_else(|| {
                    chunks.iter().find(|chunk| {
                        passage.chunk.start_line.is_some()
                            && chunk.start_line == passage.chunk.start_line
                            && chunk.end_line == passage.chunk.end_line
                    })
                })
        });
        if found.is_none() {
            widened += 1;
        }
        let linked = state
            .storage
            .link_imported_citation(
                &workspace_id,
                &passage.card_id,
                &passage.artifact_id,
                found.map(|chunk| chunk.id.as_str()),
                passage.confidence,
            )
            .await;
        if let Err(error) = linked {
            tracing::error!(error = %error, workspace_id, "A citation could not be linked after an import");
        }
    }
    // Files were stored as indexed as of their last change; this builds their index.
    state.index_queue.resume_pending().await;

    tracing::info!(target: "audit", user_id = %subject.user_id, workspace_id = %workspace_id, "Workspace imported");
    record_workspace_activity(
        state,
        &workspace_id,
        &subject.user_id,
        "workspace_imported",
        "workspace",
        Some(&workspace_id),
        format!(
            "Imported the workspace from an archive: {} files, {} memory cards, {} tasks, {} comments.",
            data.artifacts.len(),
            data.memory_cards.len(),
            data.tasks.len(),
            data.comments.len()
        ),
    )
    .await;
    Ok(Created {
        workspace: Some(workspace),
        widened,
    })
}
