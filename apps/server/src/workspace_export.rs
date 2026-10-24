//! Export of a workspace as an archive (roadmap O2a).
//!
//! The archive is a gzip-compressed tar with a manifest, one JSON document
//! with the workspace's records and one entry per distinct stored file (see
//! `repomemo_domain::archive`). It is produced while it is sent: the records
//! are read first, then each file is read and compressed in turn, so the
//! server holds one file at a time and the download starts at once. A failure
//! half-way ends the response with an error instead of a complete-looking file.

use std::collections::BTreeMap;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use repomemo_api::ArchiveEncoder;
use repomemo_domain::{
    ArchiveBlob, ArchiveManifest, WorkspaceArchiveData, ARCHIVE_BLOB_PREFIX, ARCHIVE_DATA_ENTRY, ARCHIVE_FORMAT,
    ARCHIVE_FORMAT_VERSION, ARCHIVE_MANIFEST_ENTRY,
};
use repomemo_storage::StorageEngine;
use tokio::sync::mpsc::Sender;
use tokio_stream::wrappers::ReceiverStream;

use crate::{
    consume_heavy_quota, map_storage_error, record_workspace_activity, require_workspace_admin, ApiError, AppState,
    ManagingSubject,
};

/// Bytes of a file handed to the compressor at a time, so the stream moves
/// (and a slow client slows the export) while a large file is read.
const SLICE_BYTES: usize = 256 * 1024;
/// Pieces waiting for the client before the export pauses.
const PIPELINE_DEPTH: usize = 4;

type Piece = Result<Vec<u8>, std::io::Error>;

/// `GET /v1/workspaces/{id}/export`: the workspace as an archive, for its
/// owners and administrators.
pub(crate) async fn export_workspace(
    ManagingSubject(subject): ManagingSubject,
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
) -> Result<Response, ApiError> {
    require_workspace_admin(&state, &subject, &workspace_id).await?;
    consume_heavy_quota(&state, &subject)?;
    let started = std::time::Instant::now();

    let data = state
        .storage
        .export_workspace_data(&workspace_id)
        .await
        .map_err(map_storage_error)?;
    let blobs = distinct_blobs(&state.storage, &data).await?;
    let data_bytes = serde_json::to_vec_pretty(&data).map_err(ApiError::internal)?;
    let manifest = ArchiveManifest {
        format: ARCHIVE_FORMAT.to_owned(),
        format_version: ARCHIVE_FORMAT_VERSION,
        created_at: Utc::now().to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        workspace_name: data.workspace.name.clone(),
        counts: data.counts(blobs.len() as u64),
        data_sha256: StorageEngine::content_hash(&data_bytes),
        data_bytes: data_bytes.len() as u64,
        blobs,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(ApiError::internal)?;

    let summary = format!(
        "Exported the workspace as an archive: {} files, {} memory cards, {} tasks, {} comments.",
        manifest.counts.artifacts, manifest.counts.memory_cards, manifest.counts.tasks, manifest.counts.comments
    );
    tracing::info!(target: "audit", user_id = %subject.user_id, workspace_id = %workspace_id, "Workspace exported");
    record_workspace_activity(
        &state,
        &workspace_id,
        &subject.user_id,
        "workspace_exported",
        "workspace",
        Some(&workspace_id),
        summary,
    )
    .await;
    crate::record_timing("workspace_export", &workspace_id, started, manifest.blobs.len());

    let filename = archive_filename(&data.workspace.name);
    let (sender, receiver) = tokio::sync::mpsc::channel::<Piece>(PIPELINE_DEPTH);
    let storage = state.storage.clone();
    let blobs = manifest.blobs;
    tokio::spawn(async move {
        if let Err(error) = write_archive(&storage, &manifest_bytes, &data_bytes, &blobs, &sender).await {
            tracing::error!(error = %error, "A workspace export stopped half-way");
            let _ = sender.send(Err(std::io::Error::other(error.to_string()))).await;
        }
    });

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/gzip"));
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    Ok((
        StatusCode::OK,
        headers,
        Body::from_stream(ReceiverStream::new(receiver)),
    )
        .into_response())
}

/// The distinct stored files the artifacts name, with their sizes on disk.
/// An archive that cannot hold every file is refused up front rather than
/// found incomplete later.
async fn distinct_blobs(storage: &StorageEngine, data: &WorkspaceArchiveData) -> Result<Vec<ArchiveBlob>, ApiError> {
    let mut sizes: BTreeMap<&str, u64> = BTreeMap::new();
    let mut missing = Vec::new();
    // The files and, since format 2, their earlier versions.
    for (hash, name) in data.stored_files() {
        if sizes.contains_key(hash) {
            continue;
        }
        match tokio::fs::metadata(storage.blob_path_for_hash(hash)).await {
            Ok(metadata) if metadata.is_file() => {
                sizes.insert(hash, metadata.len());
            }
            _ => missing.push(name),
        }
    }
    if !missing.is_empty() {
        return Err(ApiError::conflict(format!(
            "{} file{} stored content is missing from the server, so the workspace cannot be exported whole (first: {}). Restore it from a backup or delete the file.",
            missing.len(),
            if missing.len() == 1 { "'s" } else { "s'" },
            missing[0]
        )));
    }
    Ok(sizes
        .into_iter()
        .map(|(hash, size_bytes)| ArchiveBlob {
            hash: hash.to_owned(),
            size_bytes,
        })
        .collect())
}

async fn write_archive(
    storage: &StorageEngine,
    manifest: &[u8],
    data: &[u8],
    blobs: &[ArchiveBlob],
    sender: &Sender<Piece>,
) -> anyhow::Result<()> {
    let now = Utc::now().timestamp();
    let mut encoder = ArchiveEncoder::new();
    encoder.add(ARCHIVE_MANIFEST_ENTRY, manifest, now)?;
    encoder.add(ARCHIVE_DATA_ENTRY, data, now)?;
    if !send(sender, encoder.drain()).await {
        return Ok(());
    }
    for blob in blobs {
        let bytes = storage.read_blob(&blob.hash).await?;
        if bytes.len() as u64 != blob.size_bytes {
            anyhow::bail!("the stored file {} changed while it was exported", blob.hash);
        }
        encoder.start(&format!("{ARCHIVE_BLOB_PREFIX}{}", blob.hash), blob.size_bytes, now)?;
        for slice in bytes.chunks(SLICE_BYTES) {
            encoder.write(slice)?;
            if !send(sender, encoder.drain()).await {
                return Ok(());
            }
        }
        encoder.end()?;
    }
    send(sender, encoder.finish()?).await;
    Ok(())
}

/// Hands a piece to the response; false when the client has gone away.
async fn send(sender: &Sender<Piece>, piece: Vec<u8>) -> bool {
    if piece.is_empty() {
        return true;
    }
    sender.send(Ok(piece)).await.is_ok()
}

/// `repomemo-<workspace name>-<date>.tar.gz`, safe for a header and a disk.
fn archive_filename(workspace_name: &str) -> String {
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
    format!(
        "repomemo-{}-{}.tar.gz",
        slug.trim_end_matches('-'),
        Utc::now().format("%Y%m%d")
    )
}

#[cfg(test)]
mod tests {
    use super::archive_filename;

    #[test]
    fn the_file_name_is_a_safe_slug_of_the_workspace_name() {
        let name = archive_filename("Platform & Docs / Équipe \"A\"");
        assert!(name.starts_with("repomemo-platform-docs-quipe-a-"), "{name}");
        assert!(name.ends_with(".tar.gz"));
        assert!(archive_filename("???").starts_with("repomemo-workspace-"));
        assert!(!archive_filename(&"x".repeat(200)).contains(&"x".repeat(41)));
    }
}
