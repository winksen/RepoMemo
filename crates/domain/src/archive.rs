//! The workspace archive: one file holding everything a workspace has, so it
//! can be kept, inspected or moved to another server (roadmap O2).
//!
//! The file is a gzip-compressed tar. It holds `manifest.json` (this module's
//! [`ArchiveManifest`]), `workspace.json` ([`WorkspaceArchiveData`]) and one
//! `blobs/<sha-256>` entry per distinct stored file. Nothing in it names a
//! user by id: people are carried as a name and an address, so an archive
//! means the same on a server whose accounts have other ids.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The `format` field of a manifest.
pub const ARCHIVE_FORMAT: &str = "repomemo-workspace-archive";
/// Bumped when an archive written by a newer server cannot be read by an older one.
pub const ARCHIVE_FORMAT_VERSION: u32 = 1;
pub const ARCHIVE_MANIFEST_ENTRY: &str = "manifest.json";
pub const ARCHIVE_DATA_ENTRY: &str = "workspace.json";
pub const ARCHIVE_BLOB_PREFIX: &str = "blobs/";

/// A person a record names: a name, and the address when the account has one
/// (an anonymized account is carried as "Former member" with none).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchivePerson {
    pub name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveCounts {
    pub sources: u64,
    pub folders: u64,
    pub artifacts: u64,
    pub blobs: u64,
    pub memory_cards: u64,
    pub citations: u64,
    pub tasks: u64,
    pub checklist_items: u64,
    pub comments: u64,
    pub lifecycle_events: u64,
    /// Citations left out because what they cite no longer exists.
    pub omitted_citations: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveBlob {
    /// SHA-256 of the content, which is also the entry's name under `blobs/`.
    pub hash: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveManifest {
    pub format: String,
    pub format_version: u32,
    pub created_at: String,
    /// The server version that wrote it.
    pub app_version: String,
    pub workspace_name: String,
    pub counts: ArchiveCounts,
    /// SHA-256 and length of the `workspace.json` entry.
    pub data_sha256: String,
    pub data_bytes: u64,
    pub blobs: Vec<ArchiveBlob>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveWorkspace {
    pub name: String,
    pub created_at: String,
    /// The workspace's own settings (such as the AI policy).
    pub settings: Value,
}

/// Where evidence came from. Links to a repository or a tracker are not
/// carried: what they brought is evidence like any other.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveSource {
    pub id: String,
    pub name: String,
    /// `upload`, `folder`, `git_repo`, `manual` or `connector`.
    pub source_type: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveFolder {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveArtifact {
    pub id: String,
    pub source_id: String,
    pub folder_id: Option<String>,
    pub artifact_type: String,
    pub title: String,
    pub path: String,
    /// The `blobs/` entry holding the file's bytes.
    pub content_hash: String,
    pub mime_type: Option<String>,
    pub language: Option<String>,
    pub size_bytes: i64,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: Value,
}

/// A passage of a file, named by what identifies it after the file is
/// indexed again: its place in the file and a hash of its text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveChunkRef {
    pub chunk_index: i64,
    pub content_hash: String,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub heading_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveCitation {
    pub artifact_id: String,
    /// The cited passage; none when the whole file is cited.
    pub chunk: Option<ArchiveChunkRef>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveMemoryCard {
    pub id: String,
    pub title: String,
    pub body_markdown: String,
    pub source: String,
    pub confidence: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: Value,
    pub citations: Vec<ArchiveCitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveChecklistItem {
    pub body: String,
    pub position: i64,
    pub completed_at: Option<String>,
    pub completed_by: Option<ArchivePerson>,
    pub created_by: ArchivePerson,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assignee: Option<ArchivePerson>,
    pub created_by: ArchivePerson,
    pub artifact_id: Option<String>,
    pub due_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub checklist: Vec<ArchiveChecklistItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveComment {
    pub id: String,
    pub artifact_id: String,
    pub author: ArchivePerson,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveLifecycleEvent {
    pub action: String,
    pub detail: String,
    pub actor: Option<ArchivePerson>,
    pub created_at: String,
}

/// A file's lifecycle: its state, who owns and reviewed it, and the history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveLifecycle {
    pub artifact_id: String,
    pub status: String,
    pub owner: Option<ArchivePerson>,
    pub review_note: String,
    pub reviewed_by: Option<ArchivePerson>,
    pub reviewed_at: Option<String>,
    pub superseded_by_artifact_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Oldest first.
    pub events: Vec<ArchiveLifecycleEvent>,
}

/// The content of `workspace.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceArchiveData {
    pub workspace: ArchiveWorkspace,
    pub sources: Vec<ArchiveSource>,
    pub folders: Vec<ArchiveFolder>,
    pub artifacts: Vec<ArchiveArtifact>,
    pub lifecycle: Vec<ArchiveLifecycle>,
    pub memory_cards: Vec<ArchiveMemoryCard>,
    pub tasks: Vec<ArchiveTask>,
    pub comments: Vec<ArchiveComment>,
    /// Citations dropped because what they cite is gone.
    #[serde(default)]
    pub omitted_citations: u64,
}

impl WorkspaceArchiveData {
    /// What the archive holds, for its manifest.
    pub fn counts(&self, blobs: u64) -> ArchiveCounts {
        ArchiveCounts {
            sources: self.sources.len() as u64,
            folders: self.folders.len() as u64,
            artifacts: self.artifacts.len() as u64,
            blobs,
            memory_cards: self.memory_cards.len() as u64,
            citations: self.memory_cards.iter().map(|card| card.citations.len() as u64).sum(),
            tasks: self.tasks.len() as u64,
            checklist_items: self.tasks.iter().map(|task| task.checklist.len() as u64).sum(),
            comments: self.comments.len() as u64,
            lifecycle_events: self.lifecycle.iter().map(|entry| entry.events.len() as u64).sum(),
            omitted_citations: self.omitted_citations,
        }
    }
}
