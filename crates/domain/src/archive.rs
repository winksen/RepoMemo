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
/// Bumped when an archive written by a newer server cannot be read by an older one. Version 2 (V0.3.19) adds the
/// earlier versions of files and the saved searches; version 3 (V0.6.14) adds each memory card's revisions, the content
/// its citations recorded, its approvals and its review assignment; version 4 (V0.7.19) adds the work of the 0.7 line
/// (projects, milestones, task numbers and relations, labels, workflow states, schedules and project updates); version 5
/// (V0.8.19) adds where connector items came from and the record of publications, neither of which is a live integration. A
/// server reads versions 1 to 5.
pub const ARCHIVE_FORMAT_VERSION: u32 = 5;
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
    /// Earlier versions of uploaded files (format 2).
    #[serde(default)]
    pub versions: u64,
    /// Saved searches (format 2).
    #[serde(default)]
    pub saved_searches: u64,
    /// Memory card revisions (format 3).
    #[serde(default)]
    pub revisions: u64,
    /// Approval decisions about memory card revisions (format 3).
    #[serde(default)]
    pub approvals: u64,
    /// Review assignments of memory cards (format 3).
    #[serde(default)]
    pub reviews: u64,
    /// Deleted memory cards and files that were in the workspace's trash and are not part of the archive (V0.6.28).
    #[serde(default)]
    pub trash_left_out: u64,
    /// Projects, milestones, labels, named workflow states, recurring schedules and project updates (format 4).
    #[serde(default)]
    pub projects: u64,
    #[serde(default)]
    pub milestones: u64,
    #[serde(default)]
    pub labels: u64,
    #[serde(default)]
    pub workflow_states: u64,
    #[serde(default)]
    pub recurring: u64,
    #[serde(default)]
    pub project_updates: u64,
    /// Comments on tasks, and the blocker links between tasks (format 4).
    #[serde(default)]
    pub task_comments: u64,
    #[serde(default)]
    pub dependencies: u64,
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
    /// Every saved revision of the card, oldest first (format 3). Empty in an older archive.
    #[serde(default)]
    pub revisions: Vec<ArchiveRevision>,
    /// Every approval decision, oldest first (format 3).
    #[serde(default)]
    pub approvals: Vec<ArchiveApproval>,
    /// The accountable reviewer and the next review date (format 3).
    #[serde(default)]
    pub review: Option<ArchiveReview>,
}

/// One citation of a saved revision as the archive carries it (format 3): the file, whether it was a passage, and the
/// content hash the revision recorded for it, so an import shows the same citation meaning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveRevisionCitation {
    /// The cited file, named by the archive's own artifact id; none when it was already gone when the revision was saved.
    pub artifact_id: Option<String>,
    /// True when the revision cited a passage of the file. An import keeps the file, because the passage's identity is
    /// not recoverable for a historical revision.
    #[serde(default)]
    pub passage: bool,
    pub title: String,
    pub path: String,
    pub confidence: Option<f64>,
    /// The hash of the file's content when the revision was saved; none when the revision did not record one.
    #[serde(default)]
    pub content_hash: Option<String>,
    #[serde(default)]
    pub missing_at_save: bool,
}

/// A saved revision of a memory card (format 3). Its author is a name and an address, as everywhere in an archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveRevision {
    pub revision: i64,
    pub title: String,
    pub body_markdown: String,
    pub source: String,
    pub confidence: Option<f64>,
    pub citations: Vec<ArchiveRevisionCitation>,
    pub author: Option<ArchivePerson>,
    pub created_at: String,
    pub restored_from: Option<i64>,
}

/// A decision about a revision of a memory card (format 3). An import keeps it as a record of what was decided
/// elsewhere; it never approves anything on the server that reads it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveApproval {
    /// True for a decision that was itself imported from another server before this archive was made (V0.6.28); the
    /// archive keeps it as the record it is.
    #[serde(default)]
    pub imported: bool,
    pub revision: i64,
    /// `approved` or `withdrawn`.
    pub decision: String,
    pub by: Option<ArchivePerson>,
    pub note: String,
    pub created_at: String,
}

/// Who is accountable for a memory card's review and when it is next due (format 3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveReview {
    pub reviewer: Option<ArchivePerson>,
    pub next_review_at: Option<String>,
    pub last_reviewed_at: Option<String>,
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

/// An earlier version of an uploaded file (format 2): the content it had until a newer upload replaced it. The current
/// content is the artifact's own. Whoever uploaded the next version is carried as a name, as everywhere in an archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveVersion {
    pub artifact_id: String,
    /// 1 for the first upload; the artifact holds the highest.
    pub version: i64,
    /// The `blobs/` entry holding this version's bytes.
    pub content_hash: String,
    pub size_bytes: i64,
    pub mime_type: Option<String>,
    pub became_current_at: String,
    pub replaced_at: String,
    pub replaced_by: Option<ArchivePerson>,
}

/// A saved search (format 2). The sources it keeps to are named by the archive's own source ids.
fn previously_shared() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveSavedSearch {
    pub name: String,
    pub query: String,
    pub artifact_types: Vec<String>,
    pub languages: Vec<String>,
    pub source_ids: Vec<String>,
    pub result_limit: i64,
    #[serde(default = "previously_shared")]
    pub shared: bool,
    pub created_by: ArchivePerson,
    pub created_at: String,
    pub updated_at: String,
}

/// A memory card or piece of evidence a project or a task cites (format 4), by the archive's own id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveReference {
    /// `evidence` or `memory_card`.
    pub kind: String,
    pub target_id: String,
    pub note: String,
    pub added_by: Option<ArchivePerson>,
    pub created_at: String,
}

/// A project (format 4) with the decisions and evidence its brief cites.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveProject {
    pub id: String,
    pub name: String,
    pub goal: String,
    /// `active`, `on_hold`, `done` or `archived`.
    pub status: String,
    pub owner: Option<ArchivePerson>,
    pub created_by: Option<ArchivePerson>,
    pub target_date: Option<String>,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub references: Vec<ArchiveReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveMilestone {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub due_date: Option<String>,
    /// `all_tasks` or `manual`.
    pub completion_rule: String,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveLabel {
    pub id: String,
    pub name: String,
    pub color: String,
}

/// A named workflow state (format 4); the four built-in states are not carried.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveWorkflowState {
    pub id: String,
    pub name: String,
    pub category: String,
    pub position: i64,
}

/// A comment on a task (format 4). Like every comment in an archive, it comes back credited to the person who imports, with
/// the name the archive gives kept in the text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveTaskComment {
    pub author: ArchivePerson,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

/// What a task has beyond the basics (format 4), named by the archive's own ids: its number, project, milestone, parent, labels,
/// named state, citations, discussion and the tasks that block it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveTaskExtra {
    pub task_id: String,
    /// The number in the workspace it came from, kept so `T-12` names the same task after a move.
    pub number: i64,
    pub project_id: Option<String>,
    pub milestone_id: Option<String>,
    pub parent_task_id: Option<String>,
    #[serde(default)]
    pub label_ids: Vec<String>,
    pub state_id: Option<String>,
    #[serde(default)]
    pub references: Vec<ArchiveReference>,
    #[serde(default)]
    pub comments: Vec<ArchiveTaskComment>,
    /// The archive's ids of the tasks that block this one.
    #[serde(default)]
    pub blocked_by: Vec<String>,
}

/// One occurrence date a schedule looked at (format 4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveOccurrence {
    pub occurrence_date: String,
    /// `created` or `skipped`.
    pub outcome: String,
    pub task_id: Option<String>,
    pub created_at: String,
}

/// A recurring schedule (format 4). It always comes back **paused** (or stopped, if it was): nothing is made on the server that
/// reads the archive until someone there resumes it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveRecurring {
    pub id: String,
    pub title: String,
    pub description: String,
    pub priority: String,
    pub assignee: Option<ArchivePerson>,
    pub project_id: Option<String>,
    pub due_after_days: Option<i64>,
    pub schedule: crate::RecurringSchedule,
    pub missed_policy: String,
    /// `active`, `paused` or `stopped`.
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub stopped_at: Option<String>,
    #[serde(default)]
    pub occurrences: Vec<ArchiveOccurrence>,
}

/// What a project update pointed at (format 4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveUpdateLink {
    /// `milestone`, `task`, `evidence` or `memory_card`.
    pub kind: String,
    pub target_id: String,
    pub label: String,
}

/// A project status update (format 4). It comes back as a record: the person who wrote it is named in its text, and the health is
/// still the one that person chose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveProjectUpdate {
    pub id: String,
    pub project_id: String,
    pub author: Option<ArchivePerson>,
    pub health: String,
    pub summary: String,
    pub reason: String,
    pub snapshot: crate::ProjectSnapshot,
    #[serde(default)]
    pub changes: Vec<String>,
    #[serde(default)]
    pub links: Vec<ArchiveUpdateLink>,
    pub created_at: String,
    pub retracted_at: Option<String>,
}

/// The work of the 0.7 line (format 4): projects and what hangs on them, and the relations between tasks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArchiveWork {
    /// The last task number given in the workspace, so a number that was deleted is not given out again after a move.
    #[serde(default)]
    pub task_counter: i64,
    #[serde(default)]
    pub projects: Vec<ArchiveProject>,
    #[serde(default)]
    pub milestones: Vec<ArchiveMilestone>,
    #[serde(default)]
    pub labels: Vec<ArchiveLabel>,
    #[serde(default)]
    pub workflow_states: Vec<ArchiveWorkflowState>,
    #[serde(default)]
    pub task_extras: Vec<ArchiveTaskExtra>,
    #[serde(default)]
    pub recurring: Vec<ArchiveRecurring>,
    #[serde(default)]
    pub updates: Vec<ArchiveProjectUpdate>,
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
    /// Earlier versions of uploaded files, oldest first within a file (format 2).
    #[serde(default)]
    pub versions: Vec<ArchiveVersion>,
    /// The workspace's saved searches (format 2).
    #[serde(default)]
    pub saved_searches: Vec<ArchiveSavedSearch>,
    /// Items in the workspace's trash when the archive was made, which an archive does not carry (V0.6.28).
    #[serde(default)]
    pub trash_left_out: u64,
    /// Projects, task numbers and relations, labels, workflow states, schedules and updates (format 4). Empty in an older archive.
    #[serde(default)]
    pub work: ArchiveWork,
    /// Where connector items came from and what was published (format 5). Records only: nothing comes back connected, sending or public.
    #[serde(default)]
    pub integrations: ArchiveIntegrations,
}

/// Provenance and publication history (V0.8.19). It never holds a credential, a secret or an address that is not the tracker's own.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArchiveIntegrations {
    #[serde(default)]
    pub connectors: Vec<ArchiveConnectorProvenance>,
    #[serde(default)]
    pub publications: Vec<ArchivePublication>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveConnectorProvenance {
    /// The archive's own id of the source.
    pub source_id: String,
    pub name: String,
    /// `github`, `gitlab` or `jira`.
    pub provider: String,
    /// The tracker's address of the project, such as https://github.com/acme/app.
    pub location: String,
    pub items: Vec<ArchiveConnectorItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveConnectorItem {
    /// The archive's own id of the file.
    pub artifact_id: String,
    pub external_id: String,
    pub kind: String,
    pub state: String,
    pub remote_updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivePublication {
    /// The archive's own id of the file it was made from, when that file is in the archive.
    pub artifact_id: Option<String>,
    pub title: String,
    pub body_markdown: String,
    pub status_at_export: String,
    pub version: i64,
    pub published_at: String,
}

impl WorkspaceArchiveData {
    /// Every stored file the records name, as `(content hash, what to call it in a message)`: the files themselves,
    /// then their earlier versions. A hash can repeat; the archive holds each once.
    pub fn stored_files(&self) -> Vec<(&str, String)> {
        let files = self
            .artifacts
            .iter()
            .map(|artifact| (artifact.content_hash.as_str(), artifact.path.clone()));
        let versions = self.versions.iter().map(|version| {
            let path = self
                .artifacts
                .iter()
                .find(|artifact| artifact.id == version.artifact_id)
                .map_or(version.artifact_id.as_str(), |artifact| artifact.path.as_str());
            (
                version.content_hash.as_str(),
                format!("{path} (version {})", version.version),
            )
        });
        files.chain(versions).collect()
    }

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
            versions: self.versions.len() as u64,
            saved_searches: self.saved_searches.len() as u64,
            revisions: self.memory_cards.iter().map(|card| card.revisions.len() as u64).sum(),
            approvals: self.memory_cards.iter().map(|card| card.approvals.len() as u64).sum(),
            reviews: self.memory_cards.iter().filter(|card| card.review.is_some()).count() as u64,
            trash_left_out: self.trash_left_out,
            projects: self.work.projects.len() as u64,
            milestones: self.work.milestones.len() as u64,
            labels: self.work.labels.len() as u64,
            workflow_states: self.work.workflow_states.len() as u64,
            recurring: self.work.recurring.len() as u64,
            project_updates: self.work.updates.len() as u64,
            task_comments: self
                .work
                .task_extras
                .iter()
                .map(|extra| extra.comments.len() as u64)
                .sum(),
            dependencies: self
                .work
                .task_extras
                .iter()
                .map(|extra| extra.blocked_by.len() as u64)
                .sum(),
        }
    }
}
