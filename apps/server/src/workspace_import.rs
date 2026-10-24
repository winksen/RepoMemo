//! Import of a workspace archive into a new workspace (roadmap O2b).
//!
//! The archive (see `workspace_export.rs`) is received into a temporary file,
//! never into memory, and read back entry by entry. `dry_run=true` reads it
//! all the same, verifies every checksum and checks the records against each
//! other and against this server's limits, and writes nothing. A real import
//! stores the files, creates the workspace and writes its records in one
//! transaction; if that fails the workspace is removed again.
//!
//! With `background=true` (V0.3.20) the archive is still received by the request, but the checking and the
//! writing run as a job the request does not wait for: it answers `202` with a job id, and
//! `GET /v1/workspaces/import/jobs/{id}` reports the stage and the progress, then the report. A job lives in the
//! server's memory, so a restart ends it; nothing is created until its last step, and a failed one leaves nothing.
//!
//! An archive is only as trustworthy as whoever wrote it, so the importer
//! gets what it holds and nothing more: records are credited to the importer
//! (the archive's names are kept in the text), no account is matched by
//! address, and nothing in it can carry a secret, a server path or a role.

use std::collections::{HashMap, HashSet};
use std::io::BufReader;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::{
    body::Body,
    extract::{Path, Query, State},
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
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;

use crate::quotas::{format_bytes, free_disk_bytes};
use crate::{
    consume_heavy_quota, map_storage_error, record_workspace_activity, ApiError, AppState, AuthenticatedSubject,
    ManagingSubject,
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
/// An archive may unpack to this many times its own size at the most. Text compresses ten to
/// thirty times; a file that claims more is a compression bomb.
const MAX_EXPANSION: u64 = 100;
/// Whatever the ratio, an archive may always unpack to this much.
const EXPANSION_ALLOWANCE: u64 = 64 * MIB;
/// Temporary files older than this are leftovers of an interrupted import.
const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);
/// Imports running at once on the whole server; a person has one at a time.
const MAX_RUNNING_IMPORTS: usize = 2;
/// A finished job is kept this long for its owner to read.
const FINISHED_KEPT: std::time::Duration = std::time::Duration::from_secs(60 * 60);
/// Finished jobs kept for one person, and for the whole server: a report can be large, and a person who checks
/// archives one after the other must not fill the server's memory for an hour (V0.3.26).
const FINISHED_PER_OWNER: usize = 5;
const FINISHED_IN_ALL: usize = 100;

/// Where a background import is: kept in memory, read by its owner (V0.3.20).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ImportJobStatus {
    pub(crate) id: String,
    /// `running`, `done` or `failed`.
    pub(crate) status: &'static str,
    /// `checking`, `storing_files`, `writing_records`, then `finished`.
    pub(crate) stage: &'static str,
    pub(crate) progress_current: u64,
    /// Bytes to store; 0 while not known or when the stage has no count.
    pub(crate) progress_total: u64,
    /// The report: of a dry run, or of the workspace made; also kept when a real import was refused with problems.
    pub(crate) report: Option<ArchiveImportReport>,
    pub(crate) error: Option<String>,
}

struct ImportJob {
    owner: String,
    finished: Option<Instant>,
    status: ImportJobStatus,
}

/// The background imports of the server.
#[derive(Clone, Default)]
pub(crate) struct ImportJobs(Arc<Mutex<HashMap<String, ImportJob>>>);

impl ImportJobs {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, ImportJob>> {
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Starts a job for `owner`: refused while they have one running, or when the server has enough.
    pub(crate) fn start(&self, owner: &str) -> Result<String, ApiError> {
        let mut jobs = self.lock();
        jobs.retain(|_, job| job.finished.is_none_or(|at| at.elapsed() < FINISHED_KEPT));
        let running: Vec<&ImportJob> = jobs.values().filter(|job| job.finished.is_none()).collect();
        if running.iter().any(|job| job.owner == owner) {
            return Err(ApiError::conflict(
                "An import of yours is still running. Wait for it to finish, then import the next archive.",
            ));
        }
        if running.len() >= MAX_RUNNING_IMPORTS {
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "imports_busy",
                "The server is importing other archives right now. Try again in a few minutes.",
            ));
        }
        let id = crate::auth::random_secret();
        jobs.insert(
            id.clone(),
            ImportJob {
                owner: owner.to_owned(),
                finished: None,
                status: ImportJobStatus {
                    id: id.clone(),
                    status: "running",
                    stage: "checking",
                    progress_current: 0,
                    progress_total: 0,
                    report: None,
                    error: None,
                },
            },
        );
        Ok(id)
    }

    fn update(&self, id: &str, change: impl FnOnce(&mut ImportJobStatus)) {
        if let Some(job) = self.lock().get_mut(id) {
            change(&mut job.status);
        }
    }

    fn finish(&self, id: &str, report: Option<ArchiveImportReport>, error: Option<String>) {
        let mut jobs = self.lock();
        let Some(job) = jobs.get_mut(id) else {
            return;
        };
        job.finished = Some(Instant::now());
        job.status.status = if error.is_some() { "failed" } else { "done" };
        job.status.stage = "finished";
        job.status.progress_current = job.status.progress_total;
        job.status.report = report;
        job.status.error = error;
        let owner = job.owner.clone();
        // The oldest finished jobs of this person beyond five go, and then the oldest of all beyond a hundred.
        let mut own: Vec<(Instant, String)> = jobs
            .iter()
            .filter(|(_, job)| job.owner == owner)
            .filter_map(|(key, job)| job.finished.map(|at| (at, key.clone())))
            .collect();
        own.sort();
        for (_, key) in own.iter().take(own.len().saturating_sub(FINISHED_PER_OWNER)) {
            jobs.remove(key);
        }
        let all = jobs.values().filter(|job| job.finished.is_some()).count();
        if all > FINISHED_IN_ALL {
            let mut rest: Vec<(Instant, String)> = jobs
                .iter()
                .filter_map(|(key, job)| job.finished.map(|at| (at, key.clone())))
                .collect();
            rest.sort();
            for (_, key) in rest.into_iter().take(all - FINISHED_IN_ALL) {
                jobs.remove(&key);
            }
        }
    }

    /// The job, for the person who started it and for a system administrator.
    pub(crate) fn status_for(&self, id: &str, user_id: &str, is_system_admin: bool) -> Option<ImportJobStatus> {
        self.lock()
            .get(id)
            .filter(|job| job.owner == user_id || is_system_admin)
            .map(|job| job.status.clone())
    }
}

/// What a running import reports as it goes; does nothing for an import that waits for its answer.
struct Progress {
    jobs: ImportJobs,
    id: Option<String>,
}

impl Progress {
    fn none() -> Self {
        Self {
            jobs: ImportJobs::default(),
            id: None,
        }
    }

    fn stage(&self, stage: &'static str, total: u64) {
        if let Some(id) = &self.id {
            self.jobs.update(id, |status| {
                status.stage = stage;
                status.progress_current = 0;
                status.progress_total = total;
            });
        }
    }

    fn advance(&self, current: u64) {
        if let Some(id) = &self.id {
            self.jobs.update(id, |status| {
                status.progress_current = current.min(status.progress_total.max(current))
            });
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ImportQuery {
    organization_id: String,
    #[serde(default)]
    dry_run: bool,
    /// A name for the new workspace; the archive's own by default.
    name: Option<String>,
    /// Answer `202` with a job to follow once the archive is received, instead of waiting for the result (V0.3.20).
    #[serde(default)]
    background: bool,
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
    ManagingSubject(subject): ManagingSubject,
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

    // A background import takes its place first, so a person's second archive, or one more than the server
    // takes at once, is refused before the body is read (V0.3.26) and not after two gibibytes were sent.
    let job = if query.background {
        Some(state.import_jobs.start(&subject.user_id)?)
    } else {
        None
    };
    let temporary = match receive(&state, body).await {
        Ok(temporary) => temporary,
        Err(error) => {
            if let Some(id) = &job {
                state.import_jobs.finish(id, None, Some(error.message.clone()));
            }
            return Err(error);
        }
    };
    if let Some(id) = job {
        // The checking and the writing run on their own; this answers as soon as the archive is here.
        let progress = Progress {
            jobs: state.import_jobs.clone(),
            id: Some(id.clone()),
        };
        let answer = serde_json::json!({ "job_id": id });
        let task_state = state.clone();
        tokio::spawn(async move {
            let outcome = read_archive(&task_state, &subject, &query, &temporary.0, &progress).await;
            drop(temporary);
            match outcome {
                Ok(report) if report.problems.is_empty() || report.dry_run => {
                    crate::record_timing(
                        "workspace_import",
                        &query.organization_id,
                        started,
                        report.counts.artifacts as usize,
                    );
                    task_state.import_jobs.finish(&id, Some(report), None)
                }
                Ok(report) => {
                    let message = summarize(&report.problems);
                    task_state.import_jobs.finish(&id, Some(report), Some(message))
                }
                Err(error) => task_state.import_jobs.finish(&id, None, Some(error.message)),
            }
        });
        return Ok((StatusCode::ACCEPTED, Json(answer)).into_response());
    }
    let report = read_archive(&state, &subject, &query, &temporary.0, &Progress::none()).await?;
    drop(temporary);

    if !report.problems.is_empty() && !query.dry_run {
        return Err(ApiError::bad_request(summarize(&report.problems)));
    }
    crate::record_timing(
        "workspace_import",
        &query.organization_id,
        started,
        report.counts.artifacts as usize,
    );
    let status = if report.dry_run {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(report)).into_response())
}

fn summarize(problems: &[String]) -> String {
    problems.iter().take(3).cloned().collect::<Vec<_>>().join(" ")
}

/// `GET /v1/workspaces/import/jobs/{id}`: where a background import is. Only the person who started it
/// (or a system administrator) can ask; anyone else is told there is no such job.
pub(crate) async fn import_job(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> Result<Json<ImportJobStatus>, ApiError> {
    state
        .import_jobs
        .status_for(&job_id, &subject.user_id, subject.is_system_admin)
        .map(Json)
        .ok_or_else(|| {
            ApiError::not_found("That import was not found. It may have finished long ago, or the server restarted.")
        })
}

/// Streams the request body into a file in the data folder, refusing more
/// than the server accepts.
async fn receive(state: &AppState, body: Body) -> Result<TempFile, ApiError> {
    let directory = state.storage.data_dir().join("imports");
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(ApiError::internal)?;
    remove_stale(&directory).await;
    let limit = state.max_import_bytes;
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let stamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let temporary = TempFile(directory.join(format!("import-{stamp}-{sequence}.tmp")));
    let mut file = tokio::fs::File::create(&temporary.0)
        .await
        .map_err(ApiError::internal)?;
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
    progress: &Progress,
) -> Result<ArchiveImportReport, ApiError> {
    progress.stage("checking", 0);
    let file = std::fs::File::open(path).map_err(ApiError::internal)?;
    let mut reader = ArchiveReader::new(BufReader::new(file));

    let (name, bytes) = reader
        .next_entry(MANIFEST_MAX_BYTES)
        .map_err(damaged)?
        .ok_or_else(|| damaged("it is empty"))?;
    if name != ARCHIVE_MANIFEST_ENTRY {
        return Err(damaged("it does not start with a manifest"));
    }
    let manifest: ArchiveManifest =
        serde_json::from_slice(&bytes).map_err(|_| damaged("its manifest cannot be read"))?;
    let mut problems = manifest.check().problems;
    if !problems.is_empty() {
        // Nothing else in it can be trusted to mean what this server expects.
        return Ok(report(&manifest, None, problems, Vec::new(), query.dry_run));
    }

    // A small file that claims to hold something huge is refused before any of it is unpacked, and
    // what the manifest claims is also the most any entry may unpack to.
    let compressed = std::fs::metadata(path).map(|metadata| metadata.len()).unwrap_or(0);
    let declared = manifest.data_bytes.saturating_add(manifest.blob_bytes());
    let allowed = compressed.saturating_mul(MAX_EXPANSION).max(EXPANSION_ALLOWANCE);
    if declared > allowed {
        return Ok(report(
            &manifest,
            None,
            vec![format!(
                "The file is {} but claims to hold {} once unpacked, more than the {MAX_EXPANSION} to 1 a real archive reaches. It was refused as a possible compression bomb.",
                format_bytes(compressed),
                format_bytes(declared)
            )],
            Vec::new(),
            query.dry_run,
        ));
    }

    let (name, bytes) = reader
        .next_entry(manifest.data_bytes.min(DATA_MAX_BYTES))
        .map_err(damaged)?
        .ok_or_else(|| damaged("its data is missing"))?;
    if name != ARCHIVE_DATA_ENTRY {
        return Err(damaged("its data does not follow its manifest"));
    }
    if bytes.len() as u64 != manifest.data_bytes || StorageEngine::content_hash(&bytes) != manifest.data_sha256 {
        return Err(damaged(
            "its data does not match its checksum, so it was damaged or changed",
        ));
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
        .stored_files()
        .iter()
        .filter(|(hash, _)| !expected.contains_key(hash))
        .count();
    if unlisted > 0 {
        problems.push(format!(
            "{unlisted} file(s) are missing from the archive's list of stored files."
        ));
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
    for version in &data.versions {
        if let Some(mime) = version.mime_type.as_deref() {
            mimes.entry(version.content_hash.as_str()).or_insert(mime);
        }
    }
    let mut seen = HashSet::new();
    let mut read = 0_u64;
    let budget = manifest.blob_bytes();
    progress.stage("storing_files", budget);
    while let Some((name, bytes)) = reader.next_entry(budget.min(BLOB_MAX_BYTES)).map_err(damaged)? {
        let Some(hash) = name.strip_prefix(ARCHIVE_BLOB_PREFIX) else {
            warnings.push(format!("{name} is not something this server knows and was skipped."));
            continue;
        };
        let Some(size) = expected.get(hash) else {
            problems.push(format!("The archive holds a file ({hash}) its manifest does not list."));
            break;
        };
        read += bytes.len() as u64;
        progress.advance(read);
        if read > budget || bytes.len() as u64 != *size || StorageEngine::content_hash(&bytes) != hash {
            problems.push(format!(
                "The stored file {hash} does not match its checksum, so it was damaged or changed."
            ));
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
            problems.push(format!(
                "{missing} stored file(s) listed in the manifest are not in the archive."
            ));
        }
    }
    if !problems.is_empty() || query.dry_run {
        return Ok(report(&manifest, Some(&data), problems, warnings, query.dry_run));
    }

    // The sizes the records state are the sizes of the files that were just read and checked.
    let mut data = data;
    let corrected = data.use_stored_sizes(&expected);
    if corrected > 0 {
        warnings.push(format!(
            "{corrected} size(s) stated in the archive did not match the stored file and were corrected."
        ));
    }
    progress.stage("writing_records", 0);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_person_has_one_import_at_a_time_and_the_server_a_few_and_only_the_owner_reads_a_job() {
        let jobs = ImportJobs::default();
        let first = jobs.start("ada").unwrap();
        assert_eq!(
            jobs.start("ada").unwrap_err().status,
            StatusCode::CONFLICT,
            "one at a time for a person"
        );
        let second = jobs.start("grace").unwrap();
        let busy = jobs.start("alan").unwrap_err();
        assert_eq!(
            busy.status,
            StatusCode::TOO_MANY_REQUESTS,
            "the limit of the whole server"
        );
        assert_ne!(first, second);
        assert!(first.len() >= 32, "an id nobody guesses");

        // Only the person who started it reads it, or a system administrator.
        assert!(jobs.status_for(&first, "ada", false).is_some());
        assert!(jobs.status_for(&first, "grace", false).is_none());
        assert!(jobs.status_for(&first, "grace", true).is_some());
        assert!(jobs.status_for("not-a-job", "ada", true).is_none());

        // Finishing frees the person and the room, and the answer stays to be read.
        jobs.finish(&first, None, Some("It failed.".to_owned()));
        let done = jobs.status_for(&first, "ada", false).unwrap();
        assert_eq!(
            (done.status, done.stage, done.error.as_deref()),
            ("failed", "finished", Some("It failed."))
        );
        let again = jobs.start("ada").unwrap();
        assert_ne!(again, first);
        jobs.finish(&second, None, None);
        assert_eq!(jobs.status_for(&second, "grace", false).unwrap().status, "done");
    }

    #[test]
    fn progress_is_reported_by_stage_and_a_finished_job_fills_its_bar() {
        let jobs = ImportJobs::default();
        let id = jobs.start("ada").unwrap();
        let progress = Progress {
            jobs: jobs.clone(),
            id: Some(id.clone()),
        };
        let now = || jobs.status_for(&id, "ada", false).unwrap();
        assert_eq!((now().stage, now().progress_total), ("checking", 0));
        progress.stage("storing_files", 1_000);
        progress.advance(250);
        assert_eq!(
            (now().stage, now().progress_current, now().progress_total),
            ("storing_files", 250, 1_000)
        );
        progress.advance(1_000);
        assert_eq!(now().progress_current, 1_000);
        // A new stage starts again from nothing; finishing fills the bar.
        progress.stage("writing_records", 0);
        assert_eq!((now().stage, now().progress_current), ("writing_records", 0));
        jobs.finish(&id, None, None);
        assert_eq!((now().status, now().stage), ("done", "finished"));
        // An import that waits for its answer reports nothing, anywhere.
        let silent = Progress::none();
        silent.stage("storing_files", 5);
        silent.advance(5);
    }

    #[test]
    fn finished_jobs_are_kept_five_to_a_person_and_a_hundred_in_all() {
        let jobs = ImportJobs::default();
        // One person checks archives one after the other: only the newest five reports stay.
        let mut ids = Vec::new();
        for _ in 0..8 {
            let id = jobs.start("ada").unwrap();
            jobs.finish(&id, None, None);
            ids.push(id);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let kept: Vec<bool> = ids
            .iter()
            .map(|id| jobs.status_for(id, "ada", false).is_some())
            .collect();
        assert_eq!(kept, vec![false, false, false, true, true, true, true, true]);
        // Another person's jobs are their own to keep, and a running job is never dropped.
        let running = jobs.start("grace").unwrap();
        let other = jobs.start("grace").unwrap_err();
        assert_eq!(other.status, StatusCode::CONFLICT);
        assert!(jobs.status_for(&running, "grace", false).is_some());
        jobs.finish(&running, None, None);
        assert!(jobs.status_for(&ids[7], "ada", false).is_some());
        assert!(jobs.status_for(&running, "grace", false).is_some());

        // The whole server keeps a hundred, whoever they belong to.
        let many = ImportJobs::default();
        let mut first = None;
        for person in 0..130 {
            let owner = format!("person-{person}");
            let id = many.start(&owner).unwrap();
            many.finish(&id, None, None);
            first.get_or_insert((owner, id));
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let (owner, id) = first.unwrap();
        assert!(
            many.status_for(&id, &owner, false).is_none(),
            "the oldest of them is gone"
        );
        let left = (0..130)
            .filter(|person| {
                let owner = format!("person-{person}");
                many.lock().values().any(|job| job.owner == owner)
            })
            .count();
        assert_eq!(left, FINISHED_IN_ALL);
    }
}
