use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

mod archive;
mod archive_import;
pub mod console;
mod error;

pub use archive::*;
pub use archive_import::*;
pub use error::{classify, DomainError, ErrorKind};

/// Temporary server-side identity shape. Production authentication replaces
/// the dummy session issuer, not this client-facing session contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedUser {
    pub id: String,
    pub display_name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceRole {
    Owner,
    Admin,
    Member,
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationRole {
    Owner,
    Admin,
    Member,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationMember {
    pub user: SharedUser,
    pub role: OrganizationRole,
    pub joined_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMembership {
    pub workspace_id: String,
    pub role: WorkspaceRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMember {
    pub user: SharedUser,
    pub role: WorkspaceRole,
    pub joined_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceActivityEvent {
    pub id: String,
    pub workspace_id: String,
    pub actor: Option<SharedUser>,
    pub action: String,
    pub subject_type: String,
    pub subject_id: Option<String>,
    pub summary: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationTask {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assignee: Option<SharedUser>,
    pub created_by: SharedUser,
    pub artifact_id: Option<String>,
    pub due_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSearch {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub query: String,
    pub artifact_types: Vec<ArtifactType>,
    pub languages: Vec<String>,
    pub source_ids: Vec<String>,
    pub result_limit: i64,
    pub created_by: SharedUser,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskChecklistItem {
    pub id: String,
    pub task_id: String,
    pub workspace_id: String,
    pub body: String,
    pub position: i64,
    pub completed_at: Option<String>,
    pub completed_by: Option<SharedUser>,
    pub created_by: SharedUser,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactComment {
    pub id: String,
    pub workspace_id: String,
    pub artifact_id: String,
    pub author: SharedUser,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactLifecycle {
    pub artifact_id: String,
    pub workspace_id: String,
    pub status: String,
    pub owner: Option<SharedUser>,
    pub review_note: String,
    pub reviewed_by: Option<SharedUser>,
    pub reviewed_at: Option<String>,
    pub superseded_by_artifact_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactLifecycleEvent {
    pub id: String,
    pub artifact_id: String,
    pub actor: Option<SharedUser>,
    pub action: String,
    pub detail: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedNotification {
    pub id: String,
    pub workspace_id: Option<String>,
    pub notification_type: String,
    pub title: String,
    pub body: String,
    pub href: String,
    pub read_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedSession {
    pub user: SharedUser,
    pub authentication: String,
    pub memberships: Vec<WorkspaceMembership>,
    /// The user administers the whole server: every organization and
    /// workspace, and the system settings.
    #[serde(default)]
    pub is_system_admin: bool,
    /// The user may use the System pages without being an administrator of
    /// every organization and workspace.
    #[serde(default)]
    pub is_app_admin: bool,
    /// The server lets this person create an organization (see the system setting `organization_creators`).
    #[serde(default)]
    pub can_create_organizations: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub role: OrganizationRole,
    pub created_at: String,
    pub updated_at: String,
    /// Listed only because the user belongs to some of its workspaces: they
    /// are not a member of the organization, `role` is then `member` for
    /// display, and nothing flows to its other workspaces.
    #[serde(default)]
    pub guest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedWorkspace {
    pub workspace: Workspace,
    pub organization_id: String,
    pub role: WorkspaceRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub settings: Value,
}

/// The template a workspace was given, as kept in its settings under `template`: the template's id, the
/// version it was chosen at and the few adjustments an administrator saved on top of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceTemplateRef {
    pub id: String,
    pub version: u32,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub overrides: Value,
}

/// The most the saved adjustments of a template may take, serialized.
pub const MAX_TEMPLATE_OVERRIDES_BYTES: usize = 16 * 1024;

impl WorkspaceTemplateRef {
    /// Reads the `template` setting of a workspace, or of an archive that came from another server. A value that is
    /// not shaped like a reference (an id that is not a short slug, a version of 0, adjustments that are not an
    /// object or are too large) is `None`, so the workspace is simply on `general`.
    pub fn from_setting(value: &Value) -> Option<Self> {
        let reference: Self = serde_json::from_value(value.clone()).ok()?;
        let slug = !reference.id.is_empty()
            && reference.id.len() <= 40
            && reference
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        let overrides_fit = match &reference.overrides {
            Value::Null => true,
            Value::Object(_) => reference.overrides.to_string().len() <= MAX_TEMPLATE_OVERRIDES_BYTES,
            _ => false,
        };
        (slug && reference.version >= 1 && overrides_fit).then_some(reference)
    }
}

/// A template as the picker lists it: what identifies it, none of its blocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateSummary {
    pub id: String,
    pub version: u32,
    pub name: String,
    pub summary: String,
    pub icon: String,
}

/// The template a workspace uses and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceTemplateView {
    /// The template in force: the workspace's choice, or `general`.
    pub template: TemplateSummary,
    /// `default` (no choice saved), `workspace` (its own choice) or `fallback` (the choice cannot be used on this server).
    pub source: String,
    /// Why `general` is in force when `source` is `fallback`.
    pub reason: Option<String>,
    /// The id the workspace chose; not the one in force when `source` is `fallback`.
    pub chosen_id: Option<String>,
    /// The adjustments saved on top of the template (none can be saved yet).
    pub overrides: Value,
    /// The tabs and what is kept under *More*.
    pub sections: TemplateSections,
    /// The names the template gives to sections and kinds of evidence.
    pub labels: TemplateLabels,
    /// The pulse cards of the Overview.
    pub overview: TemplateOverview,
    /// How the evidence ledger opens and which file types it lists first.
    pub evidence: TemplateEvidence,
    /// The assistant's order and suggestions.
    pub assistant: TemplateAssistant,
    /// What the Retrieval page suggests.
    pub retrieval: TemplateRetrieval,
}

/// The tabs of a workspace and the sections kept under *More*, as its template arranges them. Every section is in
/// one of the two lists; for `general` that is the usual twelve in the usual order and none under *More*.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateSections {
    pub primary: Vec<String>,
    pub more: Vec<String>,
    /// The section a workspace opens on when its address names none.
    pub landing: String,
}

/// The names a template gives to sections and kinds of evidence. Only the ones it renames are here; every other
/// keeps its usual name. Display text only: ids and addresses do not change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateLabels {
    pub sections: BTreeMap<String, String>,
    pub artifact_types: BTreeMap<String, String>,
}

/// The pulse cards the Overview shows, in order, as the template chooses; for `general` the usual nine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateOverview {
    pub cards: Vec<String>,
}

/// How the evidence ledger opens (`grid` or `list`, which a person's own choice overrides) and the file types its
/// filter lists first. For `general`: `grid` and no preference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateEvidence {
    pub default_view: String,
    pub types_first: Vec<String>,
}

/// What the assistant shows first and the questions it suggests. Both empty for `general`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateAssistant {
    /// Capabilities listed before the others, in this order; the rest follow in the server's order.
    pub order: Vec<String>,
    pub suggestions: Vec<String>,
}

/// The kinds of evidence the Retrieval page suggests as chips. Empty for `general`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateRetrieval {
    pub type_chips: Vec<String>,
}

/// Chooses a workspace's template; `null` returns the workspace to the default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateWorkspaceTemplateRequest {
    pub template_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceOverview {
    pub workspace_id: String,
    pub source_count: i64,
    pub artifact_count: i64,
    pub chunk_count: i64,
    pub symbol_count: i64,
    pub memory_card_count: i64,
}

/// Server-authoritative permissions for the currently authenticated member.
/// Clients may use this shape to decide which controls to present, but every
/// mutation remains enforced by the shared API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCapabilities {
    pub role: WorkspaceRole,
    pub can_read: bool,
    pub can_write_content: bool,
    pub can_delete_content: bool,
    pub can_manage_members: bool,
    pub can_assign_admin: bool,
    pub can_manage_workspace: bool,
    pub can_generate_ai_overview: bool,
    pub can_create_tasks: bool,
    pub can_comment: bool,
    pub can_moderate_comments: bool,
    /// Whether the member may inspect the stored index chunks of an artifact.
    pub can_inspect_index: bool,
    /// Whether the member may use AI features (answers, overviews, summaries,
    /// the assistant's AI actions), per the workspace's AI policy.
    #[serde(default)]
    pub can_use_ai: bool,
}

/// A citation-backed workspace briefing generated through an enabled provider.
/// When no provider is configured, the API returns this shape with no summary
/// and an actionable warning instead of fabricating an AI result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceAiOverview {
    pub provider_configured: bool,
    pub provider_name: Option<String>,
    pub summary_markdown: Option<String>,
    pub citations: Vec<Citation>,
    pub warnings: Vec<String>,
}

/// Safe-to-return provider metadata for the shared web client. Credentials
/// remain server-side and are never serialized in this response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedAiProviderSettings {
    pub id: String,
    pub provider_type: String,
    pub name: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub enabled: bool,
    /// `text` (answers, summaries, embeddings) or `vision` (image to text).
    pub purpose: String,
}

/// A rendered-for-reading view of an uploaded business document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DocumentPreview {
    /// Extracted text (Word, legacy PowerPoint, OneNote). `approximate` is set
    /// when the text was recovered heuristically and may be incomplete.
    Text {
        text: String,
        truncated: bool,
        approximate: bool,
    },
    Sheets {
        sheets: Vec<SheetPreview>,
        total_sheets: usize,
    },
    Slides {
        slides: Vec<SlidePreview>,
        total_slides: usize,
    },
    Email {
        subject: Option<String>,
        from: Option<String>,
        to: Vec<String>,
        cc: Vec<String>,
        date: Option<String>,
        body: String,
        truncated: bool,
        attachments: Vec<EmailAttachmentInfo>,
    },
    /// The browser renders the PDF itself; only the page count is needed.
    Pdf {
        page_count: Option<usize>,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheetPreview {
    pub name: String,
    pub rows: Vec<Vec<String>>,
    pub total_rows: usize,
    pub total_columns: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlidePreview {
    pub number: usize,
    pub title: Option<String>,
    pub text: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAttachmentInfo {
    pub name: String,
    pub size_bytes: usize,
}

/// Folders may be nested this many levels deep (a top-level folder is level 1).
pub const MAX_FOLDER_DEPTH: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub workspace_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub created_at: String,
    /// Files sitting directly in this folder (not in its subfolders); filled by the folder list.
    #[serde(default)]
    pub file_count: i64,
}

/// Why an artifact could not be indexed, after every retry was used.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactIndexFailure {
    pub artifact_id: String,
    pub message: String,
    pub attempts: i64,
    pub failed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Upload,
    Folder,
    GitRepo,
    Manual,
    Connector,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub workspace_id: String,
    pub source_type: SourceType,
    pub name: String,
    pub root_uri: Option<String>,
    pub last_indexed_at: Option<String>,
    pub status: String,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    File,
    MarkdownDoc,
    CodeFile,
    Image,
    Issue,
    Pr,
    Decision,
    Incident,
    Runbook,
    ApiSpec,
    Note,
    /// A whole git repository: its generated overview, kept current by syncs.
    Repository,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub workspace_id: String,
    pub source_id: String,
    pub artifact_type: ArtifactType,
    pub title: String,
    pub path: String,
    pub content_hash: String,
    pub mime_type: Option<String>,
    pub language: Option<String>,
    pub size_bytes: i64,
    pub created_at: String,
    pub updated_at: String,
    pub indexed_at: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSummary {
    pub id: String,
    pub workspace_id: String,
    pub source_id: String,
    pub source_name: String,
    pub artifact_type: ArtifactType,
    pub title: String,
    pub path: String,
    pub content_hash: String,
    pub mime_type: Option<String>,
    pub language: Option<String>,
    pub size_bytes: i64,
    pub created_at: String,
    pub updated_at: String,
    pub indexed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactDetail {
    pub summary: ArtifactSummary,
    pub metadata: Value,
    pub content_preview: Option<String>,
    pub content_truncated: bool,
    pub chunks: Vec<Chunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRequest {
    pub workspace_id: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSkippedItem {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub workspace_id: String,
    pub scanned: usize,
    pub imported: usize,
    pub duplicates: usize,
    pub skipped: usize,
    pub failed: usize,
    pub imported_artifacts: Vec<ArtifactSummary>,
    pub skipped_items: Vec<ImportSkippedItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub artifact_id: String,
    pub workspace_id: String,
    pub chunk_index: i64,
    pub text: String,
    pub token_count: Option<i64>,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub heading_path: Option<String>,
    pub content_hash: String,
    pub embedding_status: String,
    pub metadata: Value,
}

/// Job kind of a repository sync in the jobs table.
pub const REPO_SYNC_JOB_KIND: &str = "repo_sync";

/// What a repository source indexes. Stored in the source's metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSettings {
    /// Local branch to index; `None` follows whatever HEAD points at.
    #[serde(default)]
    pub branch: Option<String>,
    /// When not empty, only paths matching one of these patterns are indexed.
    #[serde(default)]
    pub include: Vec<String>,
    /// Paths matching these patterns are skipped, on top of the defaults.
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoCommit {
    pub sha: String,
    pub summary: String,
    pub author_name: String,
    pub committed_at: String,
    /// The branch the commit was read from, when HEAD was on one.
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSkipCount {
    pub reason: String,
    pub count: usize,
    /// A few of the skipped paths, to explain the reason.
    pub examples: Vec<String>,
}

/// What one sync changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSyncReport {
    pub commit_sha: String,
    /// Regular files in the commit's tree, before any rule is applied.
    pub files_in_tree: usize,
    pub added: usize,
    pub updated: usize,
    pub renamed: usize,
    pub removed: usize,
    pub restored: usize,
    pub unchanged: usize,
    pub skipped: usize,
    pub skipped_by_reason: Vec<RepoSkipCount>,
    pub indexed: usize,
    pub index_failed: usize,
    pub cancelled: bool,
    /// The sync stopped after resolving the commit: it, the settings and the
    /// indexer were the same as at the last complete sync, and nothing was left
    /// to index.
    #[serde(default)]
    pub nothing_changed: bool,
}

/// A git repository connected to a workspace. Its files are kept in step with
/// the repository by syncs: one artifact per tracked path, updated in place.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSource {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    /// Repository root on the server's disk, or the address of a remote
    /// repository.
    pub root_path: String,
    /// The repository is fetched from a host over `https`: `root_path` is its
    /// address, and the server keeps a partial clone of it.
    #[serde(default)]
    pub remote: bool,
    /// A remote repository has a token stored (never shown).
    #[serde(default)]
    pub has_credential: bool,
    /// Push webhooks are set up for this repository.
    #[serde(default)]
    pub has_webhook: bool,
    /// When a correctly signed webhook call last arrived.
    #[serde(default)]
    pub webhook_last_received_at: Option<String>,
    pub settings: RepoSettings,
    /// `pending` (never synced), `syncing`, `ready` or `error`.
    pub status: String,
    pub last_synced_commit: Option<RepoCommit>,
    pub last_synced_at: Option<String>,
    pub last_error: Option<String>,
    pub last_report: Option<RepoSyncReport>,
    /// Files currently tracked and indexed from the repository.
    pub file_count: i64,
    pub indexed_file_count: i64,
    pub active_job: Option<IndexingJobStatus>,
    /// The evidence item that stands for the whole repository.
    pub overview_artifact_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoLanguageShare {
    pub language: String,
    pub files: usize,
    pub bytes: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoFolderShare {
    /// A top-level folder, or "" for files at the repository root.
    pub path: String,
    pub files: usize,
}

/// A file worth opening first: a README, a manifest or an entry point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoKeyFile {
    pub path: String,
    pub artifact_id: String,
    /// `readme`, `manifest`, `entry_point` or `docs`.
    pub role: String,
}

/// One framework, tool or language detected in a repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoTechnology {
    pub name: String,
    /// `frontend`, `backend`, `meta-framework`, `desktop`, `mobile`, `database`,
    /// `testing`, `tooling`, `language`, ...
    pub category: String,
}

/// What kind of project a repository is, read from its manifests and layout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoStack {
    /// `frontend`, `backend`, `web_app`, `desktop_app`, `mobile_app`, `cli` or `project`.
    pub kind: String,
    /// For example "React single-page app (TypeScript)" or "Laravel PHP backend".
    pub summary: String,
    pub technologies: Vec<RepoTechnology>,
}

/// What a repository holds, computed from its indexed files at each sync.
/// Deterministic: no AI is involved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoOverview {
    pub commit: RepoCommit,
    pub generated_at: String,
    pub file_count: usize,
    pub total_bytes: i64,
    pub languages: Vec<RepoLanguageShare>,
    pub folders: Vec<RepoFolderShare>,
    pub key_files: Vec<RepoKeyFile>,
    /// The opening of the root README, when there is one.
    pub readme_excerpt: Option<String>,
    pub recent_commits: Vec<RepoCommit>,
    /// Absent on overviews written before stack detection existed.
    #[serde(default)]
    pub stack: Option<RepoStack>,
}

/// An AI summary of a repository, kept until someone regenerates it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSummary {
    pub summary_markdown: String,
    pub citations: Vec<Citation>,
    pub warnings: Vec<String>,
    pub provider_name: String,
    /// The commit the summary describes.
    pub commit_sha: String,
    pub generated_at: String,
}

/// Everything the repository page shows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoDetail {
    pub repository: RepoSource,
    pub overview: Option<RepoOverview>,
    pub summary: Option<RepoSummary>,
}

/// What the server found at a repository link, before it is connected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoAccessCheck {
    /// The repository root the link resolves to.
    pub root_path: String,
    /// The link is a remote repository: nothing was fetched, so the commit
    /// has only its id and the file counts are zero until the first sync.
    #[serde(default)]
    pub remote: bool,
    /// The name the repository gets unless another is chosen.
    pub name: String,
    /// The commit a sync would read.
    pub commit: RepoCommit,
    /// Files tracked in that commit.
    pub tracked_files: usize,
    /// Of those, files the rules would index (before checking content).
    pub indexable_files: usize,
    /// The workspace already has this repository.
    pub already_connected: bool,
}

/// One end of a mention: an issue or pull request that names a file, or a
/// file that an item names.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMention {
    pub artifact_id: String,
    pub title: String,
    pub artifact_type: ArtifactType,
    pub path: String,
    /// `file`, `symbol` or `commit`: what the text named.
    pub kind: String,
    /// The path, symbol or commit id as named.
    pub label: String,
}

/// What names an artifact, and what it names (roadmap F17d).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArtifactMentions {
    pub mentioned_in: Vec<ArtifactMention>,
    pub mentions: Vec<ArtifactMention>,
}

/// The job kind of a connector's sync.
pub const CONNECTOR_SYNC_JOB_KIND: &str = "connector_sync";

/// A tracker connected to a workspace: its issues (and later pull requests)
/// are kept as evidence (roadmap F17).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorSource {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    /// `github`.
    pub provider: String,
    /// What it reads, such as `https://github.com/acme/app`.
    pub location: String,
    /// `pending`, `syncing`, `ready` or `error`.
    pub status: String,
    pub last_synced_at: Option<String>,
    pub last_error: Option<String>,
    pub last_report: Option<ConnectorSyncReport>,
    /// Items kept, by the state the tracker reports (a merged pull request
    /// counts as closed).
    pub open_items: i64,
    pub closed_items: i64,
    /// A token is stored (never shown).
    pub has_credential: bool,
    pub active_job: Option<IndexingJobStatus>,
    /// Webhooks are set up for this connector.
    #[serde(default)]
    pub has_webhook: bool,
    /// When a correctly signed webhook call last arrived.
    #[serde(default)]
    pub webhook_last_received_at: Option<String>,
    /// Why the connector stopped syncing by itself: the tracker's rate limit
    /// (until `paused_until`) or a token it refused (until it is replaced).
    #[serde(default)]
    pub paused_reason: Option<String>,
    #[serde(default)]
    pub paused_until: Option<String>,
    pub created_at: String,
}

/// What one connector sync did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorSyncReport {
    pub items_seen: usize,
    /// Of those, pull requests.
    #[serde(default)]
    pub pull_requests_seen: usize,
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub indexed: usize,
    pub index_failed: usize,
    /// The sync stopped at its page limit; the next one carries on.
    pub more_to_fetch: bool,
}

/// What the tracker answered for a link, before it is connected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorAccessCheck {
    pub provider: String,
    pub name: String,
    pub location: String,
    pub open_issues: i64,
    pub private: bool,
    pub already_connected: bool,
}

/// A repository file's text as it was at a commit, for a citation that names
/// one. It is not the file's current content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoFileAtCommit {
    pub artifact_id: String,
    pub path: String,
    /// The full commit id.
    pub commit: String,
    pub summary: String,
    pub author_name: String,
    pub committed_at: String,
    /// The commit the workspace's current copy is from, when it differs.
    pub current_commit: Option<String>,
    pub text: String,
}

/// A file of a repository source that is currently in its tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoFile {
    pub path: String,
    pub artifact_id: String,
    pub artifact_type: ArtifactType,
    pub language: Option<String>,
    pub size_bytes: i64,
    pub indexed: bool,
    pub index_failure: Option<String>,
    pub commit_sha: String,
    /// Who last changed the file, from the repository's history.
    #[serde(default)]
    pub last_author: Option<String>,
    /// When that change was authored.
    #[serde(default)]
    pub last_changed_at: Option<String>,
    /// The commit of that change.
    #[serde(default)]
    pub last_change_commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingJobStatus {
    pub id: String,
    pub workspace_id: String,
    pub source_id: Option<String>,
    pub kind: String,
    pub status: String,
    pub stage: String,
    pub progress_current: i64,
    pub progress_total: Option<i64>,
    pub error_message: Option<String>,
    pub cancel_requested: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Class,
    Method,
    Interface,
    Enum,
    Route,
    Endpoint,
    Config,
    Test,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: String,
    pub artifact_id: String,
    pub workspace_id: String,
    pub kind: SymbolKind,
    pub name: String,
    pub signature: Option<String>,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSearchResult {
    pub symbol: Symbol,
    pub title: String,
    pub path: String,
    pub language: Option<String>,
    pub source_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCard {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub body_markdown: String,
    pub source: String,
    pub confidence: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCardSummary {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub body_excerpt: String,
    pub source: String,
    pub evidence_count: i64,
    pub created_at: String,
    pub updated_at: String,
    /// Evidence the card cites has changed, disappeared or been retired
    /// since the card was last written (see [`MemoryReviewReason`]).
    pub needs_review: bool,
}

/// Why a memory card needs another look.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryReviewReason {
    /// `missing`: the cited passage or file no longer exists. `retired`: the
    /// cited file is marked outdated or superseded. `changed`: a cited file
    /// was updated after the card was last written.
    pub kind: String,
    pub artifact_id: Option<String>,
    pub title: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEvidence {
    pub link_id: String,
    pub target_id: String,
    pub target_type: String,
    pub artifact_id: Option<String>,
    pub chunk_id: Option<String>,
    pub title: Option<String>,
    pub path: Option<String>,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCardDetail {
    pub card: MemoryCard,
    pub evidence: Vec<MemoryEvidence>,
    /// Empty when the card is up to date with its evidence.
    pub review: Vec<MemoryReviewReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMemoryCardRequest {
    pub workspace_id: String,
    pub title: String,
    pub body_markdown: String,
    pub source: String,
    pub confidence: Option<f64>,
    pub citations: Vec<Citation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMemoryCardRequest {
    pub card_id: String,
    pub title: String,
    pub body_markdown: String,
    pub source: String,
    pub confidence: Option<f64>,
    /// Replaces the cited evidence when given; `None` keeps it.
    #[serde(default)]
    pub citations: Option<Vec<Citation>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub data_dir: String,
    pub ai_enabled: bool,
    pub active_provider: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchRequest {
    pub workspace_id: String,
    pub query: String,
    pub artifact_types: Vec<ArtifactType>,
    pub languages: Vec<String>,
    pub source_ids: Vec<String>,
    pub limit: Option<i64>,
    /// Lifecycle states to search (`active`, `needs_review`, `verified`,
    /// `outdated`, `superseded`). Empty means every state but `outdated` and
    /// `superseded`, so retired evidence is not served as current.
    #[serde(default)]
    pub lifecycle_statuses: Vec<String>,
    /// Only evidence in this folder or below it.
    #[serde(default)]
    pub folder_id: Option<String>,
    /// Only evidence updated on or after this day (`YYYY-MM-DD`) …
    #[serde(default)]
    pub modified_after: Option<String>,
    /// … and on or before this one.
    #[serde(default)]
    pub modified_before: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub artifact_id: String,
    pub chunk_id: String,
    pub title: String,
    pub path: String,
    pub artifact_type: ArtifactType,
    pub language: Option<String>,
    pub snippet: String,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub score: f64,
    pub source_name: String,
    /// For a file of a git repository: the commit its text is from.
    #[serde(default)]
    pub commit: Option<String>,
    /// For a file of a git repository: who last changed it.
    #[serde(default)]
    pub last_author: Option<String>,
    /// For a file of a git repository: when that change was authored.
    #[serde(default)]
    pub last_changed_at: Option<String>,
}

/// A search result from one of several workspaces searched together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSearchResult {
    pub workspace_id: String,
    pub workspace_name: String,
    pub result: SearchResult,
}

/// How one workspace took part in a search across workspaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSearchWorkspace {
    pub workspace_id: String,
    pub name: String,
    /// Results this workspace contributed before the merged list was cut.
    pub result_count: i64,
    /// True when the query was also matched by meaning, which sends it to
    /// the workspace's embedding provider.
    pub by_meaning: bool,
    /// Why this workspace was searched by keywords only, or not at all.
    pub note: Option<String>,
}

/// A search over every workspace the caller can read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSearchResponse {
    /// Merged by score, best first.
    pub results: Vec<CrossSearchResult>,
    pub workspaces: Vec<CrossSearchWorkspace>,
    /// Workspaces left out because the search is capped.
    pub not_searched: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub id: String,
    pub workspace_id: Option<String>,
    pub provider_type: String,
    pub name: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub embedding_model: Option<String>,
    pub enabled: bool,
    pub metadata: Value,
    #[serde(skip_serializing, default)]
    pub api_key: Option<String>,
}

impl ProviderSettings {
    /// What this provider is used for: `text` (answers and summaries),
    /// `vision` (image descriptions) or `embedding` (semantic search vectors).
    /// Providers saved before the split have no purpose and are text providers.
    pub fn purpose(&self) -> &str {
        match self.metadata.get("purpose").and_then(Value::as_str) {
            Some("vision") => "vision",
            Some("embedding") => "embedding",
            _ => "text",
        }
    }

    /// The model whose vectors this provider produces; stored with each
    /// embedding so vectors from different models are never compared.
    pub fn embedding_model_name(&self) -> &str {
        self.embedding_model
            .as_deref()
            .or(self.model.as_deref())
            .unwrap_or("unknown")
    }

    pub fn has_explicit_purpose(&self) -> bool {
        self.metadata.get("purpose").is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTestResult {
    pub provider_id: String,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub artifact_id: String,
    pub chunk_id: Option<String>,
    pub title: String,
    pub path: String,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub confidence: Option<f64>,
    /// For a file of a git repository: the commit the cited text is from.
    #[serde(default)]
    pub commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryResult {
    pub summary_markdown: String,
    pub citations: Vec<Citation>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskRequest {
    pub workspace_id: String,
    pub question: String,
    pub provider_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskAnswer {
    pub answer_markdown: String,
    pub citations: Vec<Citation>,
    pub retrieved_context: Vec<SearchResult>,
    pub confidence: Option<f64>,
    pub warnings: Vec<String>,
}

/// Everything the knowledge map page draws: how far evidence has moved
/// through indexing and embedding, and how files relate to each other.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeMap {
    pub pipeline: KnowledgePipeline,
    pub coverage: Vec<KnowledgeCoverage>,
    pub nodes: Vec<KnowledgeNode>,
    pub edges: Vec<KnowledgeEdge>,
    /// Files left out of the graph to keep it readable (the smallest ones).
    pub hidden_file_count: i64,
    /// False when there is no embedding provider or nothing is embedded
    /// yet, so the graph can only show memory links.
    pub similarity_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgePipeline {
    pub file_count: i64,
    pub indexed_count: i64,
    pub pending_count: i64,
    pub failed_count: i64,
    pub passage_count: i64,
    /// `None` when the workspace has no embedding provider.
    pub embedded_count: Option<i64>,
    pub embedding_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeCoverage {
    /// A readable kind such as "PDF", "Markdown" or "Code", from the
    /// artifact type and, for uploaded documents, the file extension.
    pub label: String,
    pub file_count: i64,
    pub indexed_count: i64,
    pub pending_count: i64,
    pub failed_count: i64,
    pub passage_count: i64,
    pub embedded_count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeNodeKind {
    File,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexState {
    Indexed,
    Pending,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeNode {
    /// The artifact id for files, the memory card id for memory.
    pub id: String,
    pub kind: KnowledgeNodeKind,
    pub title: String,
    pub path: Option<String>,
    pub artifact_type: Option<ArtifactType>,
    pub state: Option<IndexState>,
    pub passage_count: i64,
    pub embedded_count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeEdgeKind {
    /// Two files whose content means similar things (cosine of their
    /// average passage embeddings).
    Similar,
    /// A memory card citing a file or one of its passages.
    Cites,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEdge {
    pub source: String,
    pub target: String,
    pub kind: KnowledgeEdgeKind,
    /// Cosine similarity for `similar`; 1 for `cites`.
    pub weight: f64,
}

/// A deterministic check the Workspace Health page runs. None of them call
/// an AI provider; each finding is backed by stored facts the page shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthDetector {
    /// An earlier upload of the same file is still active next to a newer one.
    OlderVersionActive,
    /// Files with byte-identical content under different names.
    DuplicateContent,
    /// A document mentions a code symbol that the latest version of its
    /// file no longer defines.
    RemovedSymbolMentioned,
    /// A document mentions a file marked outdated or superseded.
    OutdatedEvidenceReferenced,
    /// A file that could not be indexed, so search never sees it.
    IndexFailed,
    /// A memory card cites evidence that changed, disappeared or was retired.
    StaleMemoryCitation,
    /// A file with no close neighbour by meaning and no memory citing it.
    /// A weak signal: shown as information, never as a removal.
    Unconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthSeverity {
    Warning,
    Info,
}

/// What an administrator can do with a finding. Every action is reversible
/// from the evidence lifecycle or the task list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthAction {
    /// Mark the finding's other files superseded by the one kept.
    Supersede,
    NeedsReview,
    MarkOutdated,
    CreateTask,
    Dismiss,
}

impl HealthAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supersede => "supersede",
            Self::NeedsReview => "needs_review",
            Self::MarkOutdated => "mark_outdated",
            Self::CreateTask => "create_task",
            Self::Dismiss => "dismiss",
        }
    }
}

impl HealthDetector {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OlderVersionActive => "older_version_active",
            Self::DuplicateContent => "duplicate_content",
            Self::RemovedSymbolMentioned => "removed_symbol_mentioned",
            Self::OutdatedEvidenceReferenced => "outdated_evidence_referenced",
            Self::IndexFailed => "index_failed",
            Self::StaleMemoryCitation => "stale_memory_citation",
            Self::Unconnected => "unconnected",
        }
    }

    pub const ALL: [HealthDetector; 7] = [
        Self::OlderVersionActive,
        Self::DuplicateContent,
        Self::RemovedSymbolMentioned,
        Self::OutdatedEvidenceReferenced,
        Self::StaleMemoryCitation,
        Self::IndexFailed,
        Self::Unconnected,
    ];
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthFile {
    pub artifact_id: String,
    pub title: String,
    pub path: String,
    pub created_at: String,
}

/// A passage that shows why a finding was raised.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthEvidence {
    pub artifact_id: String,
    pub title: String,
    pub start_line: Option<i64>,
    pub end_line: Option<i64>,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthFinding {
    /// Stable for as long as the underlying facts are unchanged, so a
    /// handled finding stays hidden until something new happens.
    pub fingerprint: String,
    pub detector: HealthDetector,
    pub severity: HealthSeverity,
    pub title: String,
    pub detail: String,
    /// The files an action applies to.
    pub files: Vec<HealthFile>,
    pub evidence: Vec<HealthEvidence>,
    /// For `supersede`: the file kept by default.
    pub keep_artifact_id: Option<String>,
    pub actions: Vec<HealthAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthDetectorStats {
    pub detector: HealthDetector,
    pub open_count: i64,
    /// Findings an administrator acted on (anything but dismiss).
    pub acted_count: i64,
    pub dismissed_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceHealth {
    pub findings: Vec<HealthFinding>,
    pub detectors: Vec<HealthDetectorStats>,
    /// Files still in use (not outdated or superseded) that were checked.
    pub checked_file_count: i64,
    /// False when there is no embedding provider, so `unconnected` is skipped.
    pub similarity_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthActionRequest {
    pub fingerprint: String,
    pub action: HealthAction,
    /// For `supersede`: which of the finding's files to keep.
    pub keep_artifact_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthActionResult {
    pub action: HealthAction,
    pub updated_artifact_ids: Vec<String>,
    pub task_id: Option<String>,
}

/// One version of an uploaded file. Uploading a new version keeps the one it
/// replaces, so earlier text stays readable and can be compared.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactVersion {
    /// 1 is the first upload; the current version has the highest number.
    pub version: i64,
    pub content_hash: String,
    pub size_bytes: i64,
    /// When this content became the file's current content.
    pub created_at: String,
    /// Who uploaded it; the first upload's author is not recorded.
    pub uploaded_by: Option<String>,
    /// When a newer version replaced it; none for the current one.
    pub replaced_at: Option<String>,
    pub is_current: bool,
}

/// The text of one version, as search would read it: the file's own text, or
/// what is extracted from a business document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactVersionText {
    pub version: i64,
    /// None when the version has no text to show (an image, or a binary file).
    pub text: Option<String>,
    pub truncated: bool,
    /// Why there is no text.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Same,
    Added,
    Removed,
    /// Unchanged lines left out of the comparison; `text` says how many.
    Gap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionDiffLine {
    pub kind: DiffKind,
    pub text: String,
    /// The line's number in the older version; none for an added line.
    pub old_line: Option<i64>,
    /// The line's number in the newer version; none for a removed line.
    pub new_line: Option<i64>,
}

/// Two versions of a file compared line by line, with a few unchanged lines
/// around each change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionComparison {
    pub from_version: i64,
    pub to_version: i64,
    pub lines: Vec<VersionDiffLine>,
    pub added: i64,
    pub removed: i64,
    /// The files were long: only the first lines of each were compared.
    pub truncated: bool,
    /// Why nothing is shown (a version has no text, or the two are the same).
    pub note: Option<String>,
}

/// What is waiting in a review queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewItemKind {
    /// A file someone marked as needing review.
    Evidence,
    /// A memory card whose cited evidence changed, disappeared or was retired.
    MemoryCard,
    /// An open Workspace Health finding.
    HealthFinding,
}

/// Who can resolve a review item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewResolver {
    /// Anyone who can change the workspace's content.
    Writer,
    /// Workspace owners and administrators.
    Admin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewItem {
    pub kind: ReviewItemKind,
    /// The file's or memory card's id, or the finding's fingerprint.
    pub id: String,
    pub title: String,
    /// Why it is waiting, in a sentence.
    pub reason: String,
    /// Where it lives: a file's path, a card's evidence, a finding's files.
    pub detail: String,
    /// The file to open: the file itself, or the first file of a finding.
    pub artifact_id: Option<String>,
    /// When it started waiting; open findings have no date.
    pub since: Option<String>,
    pub resolver: ReviewResolver,
}

/// Everything in a workspace that needs a person's attention, in one list:
/// evidence marked *needs review*, memory cards that went stale and open
/// health findings. An item leaves the list when it is resolved where it is
/// worked on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewQueue {
    /// The oldest first; items without a date last.
    pub items: Vec<ReviewItem>,
    pub evidence_count: i64,
    pub memory_card_count: i64,
    pub finding_count: i64,
}

/// A job the workspace assistant knows how to do. The assistant only ever runs
/// one of these; a message that matches none of them gets a plain reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCapability {
    FindFiles,
    /// Where a function, class or other code symbol is defined.
    FindSymbol,
    SearchContent,
    SummarizeFile,
    AskQuestion,
    WorkspaceOverview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapabilityInfo {
    pub id: AgentCapability,
    pub label: String,
    pub description: String,
    pub placeholder: String,
    pub requires_ai: bool,
    /// False when the capability needs AI and no text provider is enabled.
    pub available: bool,
}

/// How a message was matched to a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRouting {
    /// The user picked the capability.
    Explicit,
    /// Keyword rules recognised the request; no AI was involved.
    Rules,
    /// The enabled provider classified the request.
    Model,
    Unmatched,
}

/// What the user sent: typed text, optionally pinned to a capability or file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentMessage {
    #[serde(default)]
    pub message: String,
    pub capability: Option<AgentCapability>,
    pub artifact_id: Option<String>,
}

/// A saved assistant chat. Only the user who started it can see it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConversation {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub turn_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// One exchange: the request as the user saw it and the assistant's reply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTurn {
    pub id: String,
    pub conversation_id: String,
    pub position: i64,
    pub label: String,
    pub request: AgentMessage,
    pub reply: AgentReply,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConversationDetail {
    pub conversation: AgentConversation,
    pub turns: Vec<AgentTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub workspace_id: String,
    pub message: String,
    pub capability: Option<AgentCapability>,
    /// Target file for capabilities that work on one artifact.
    pub artifact_id: Option<String>,
    pub provider_id: Option<String>,
    /// Why AI may not be used for this request even though a provider may be
    /// enabled (for example the workspace keeps AI to some roles). Shown in
    /// place of the "no provider" reply when an AI capability is asked for.
    #[serde(default)]
    pub ai_unavailable_reason: Option<String>,
    /// Earlier turns of the same conversation, oldest first, so a follow-up
    /// such as "and what about its retries?" can be understood. The server
    /// fills this from the stored conversation, never from the client; the
    /// core keeps the newest ones that fit its token budget.
    #[serde(default)]
    pub history: Vec<AgentHistoryTurn>,
}

/// One earlier exchange as the assistant is reminded of it: what was asked,
/// what was answered and which files the answer was about.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHistoryTurn {
    pub question: String,
    pub answer: String,
    /// The files the answer was about: those it listed or summarized, else
    /// those it cited.
    #[serde(default)]
    pub files: Vec<AgentHistoryFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHistoryFile {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentReply {
    pub capability: Option<AgentCapability>,
    pub routing: AgentRouting,
    /// What the capability ran on after routing stripped the instruction.
    pub subject: String,
    pub reply_markdown: String,
    /// True when `reply_markdown` was written by an AI provider rather than
    /// assembled from stored facts.
    pub generated: bool,
    pub files: Vec<ArtifactSummary>,
    pub matches: Vec<SearchResult>,
    pub citations: Vec<Citation>,
    pub warnings: Vec<String>,
    /// How many earlier turns went with this question (0 for the first one,
    /// and for lookups that never read them).
    #[serde(default)]
    pub context_turns: i64,
}

/// An account as system administrators see it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemUser {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub created_at: String,
    pub last_connected_at: Option<String>,
    pub is_system_admin: bool,
    #[serde(default)]
    pub is_app_admin: bool,
    /// When the account was deactivated, if it is. It cannot sign in or use
    /// tokens until it is reactivated.
    #[serde(default)]
    pub deactivated_at: Option<String>,
    /// When the account's personal data was removed, if it was. The account
    /// then shows as "Former member" and cannot be reactivated.
    #[serde(default)]
    pub anonymized_at: Option<String>,
    pub organization_count: i64,
    pub workspace_count: i64,
    /// Refresh tokens that are neither revoked nor expired.
    pub active_sessions: i64,
    /// The workspaces the account is a direct member of, with its role in each.
    /// System administrators additionally act as administrators everywhere.
    #[serde(default)]
    pub workspaces: Vec<SystemUserWorkspace>,
    /// The organizations the account is a direct member of, with its role.
    #[serde(default)]
    pub organizations: Vec<SystemUserOrganization>,
}

/// One organization membership in the system user list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemUserOrganization {
    pub organization_id: String,
    pub name: String,
    /// `owner`, `admin` or `member`.
    pub role: String,
}

/// One workspace membership in the system user list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemUserWorkspace {
    pub workspace_id: String,
    pub name: String,
    /// `owner`, `admin`, `member` or `viewer`.
    pub role: String,
    /// The organization the workspace belongs to, if any.
    #[serde(default)]
    pub organization_id: Option<String>,
    #[serde(default)]
    pub organization_name: Option<String>,
}

/// Totals across the whole server.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemStatistics {
    pub user_count: i64,
    pub system_admin_count: i64,
    pub users_active_last_7_days: i64,
    pub users_active_last_30_days: i64,
    pub active_session_count: i64,
    pub organization_count: i64,
    pub workspace_count: i64,
    pub artifact_count: i64,
    pub artifact_bytes: i64,
    pub indexed_artifact_count: i64,
    pub blob_count: i64,
    pub blob_bytes: i64,
    pub chunk_count: i64,
    pub embedding_count: i64,
    pub memory_card_count: i64,
    pub task_count: i64,
    pub comment_count: i64,
    pub repository_count: i64,
    pub enabled_provider_count: i64,
    pub cloud_provider_count: i64,
    /// Jobs per status (`running`, `completed`, `failed`, `cancelled`).
    pub jobs_by_status: Vec<CountByLabel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CountByLabel {
    pub label: String,
    pub count: i64,
}

/// One workspace's footprint, for the system usage page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceUsage {
    pub workspace_id: String,
    pub workspace_name: String,
    pub organization_id: Option<String>,
    pub organization_name: Option<String>,
    pub member_count: i64,
    pub artifact_count: i64,
    pub artifact_bytes: i64,
    pub chunk_count: i64,
    pub activity_last_30_days: i64,
    pub ai_requests_last_30_days: i64,
    pub last_activity_at: Option<String>,
}

/// A durable record of something a system administrator (or the system
/// itself) did at the system level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemAuditEvent {
    pub id: String,
    pub created_at: String,
    pub actor: Option<SharedUser>,
    pub action: String,
    pub detail: String,
}

/// A pending invitation to an organization or a workspace. The link token
/// itself is never stored or listed; it is returned once, when created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invitation {
    pub id: String,
    pub email: String,
    /// `organization` or `workspace`.
    pub target_type: String,
    pub target_id: String,
    /// An organization role (`admin`, `member`) or a workspace role (`admin`,
    /// `member`, `viewer`), depending on `target_type`.
    pub role: String,
    pub invited_by: Option<SharedUser>,
    pub created_at: String,
    pub expires_at: String,
}

/// A personal access token as its owner sees it: never the secret itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalAccessToken {
    pub id: String,
    pub name: String,
    /// The start of the token (`rmp_` and a few characters), to tell tokens apart.
    pub prefix: String,
    /// `read` (look and search) or `write` (everything the owner may do).
    pub scope: String,
    pub created_at: String,
    pub expires_at: String,
    pub last_used_at: Option<String>,
}

/// How the server reaches its outgoing mail server, as administrators see it.
/// The password is never part of it: only whether one is saved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailSettingsView {
    /// A server and a sender are saved (whether or not sending is switched on).
    pub configured: bool,
    pub enabled: bool,
    pub host: String,
    pub port: i64,
    /// `tls` (TLS from the first byte) or `starttls`.
    pub security: String,
    pub username: String,
    pub password_set: bool,
    pub from_address: String,
    pub from_name: String,
    pub updated_at: Option<String>,
}

/// One message in the outbox, without its body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailOutboxEntry {
    pub id: String,
    pub to_address: String,
    pub subject: String,
    /// What the mail is for: `test`, and later `invitation`, `password_reset`, …
    pub kind: String,
    /// `queued`, `sending`, `sent` or `failed`.
    pub status: String,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub created_at: String,
    pub next_attempt_at: Option<String>,
    pub sent_at: Option<String>,
}

/// The mail page of System › Settings: the settings, what the outbox holds and
/// the latest messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailOverview {
    pub settings: MailSettingsView,
    pub queued_count: i64,
    pub sent_last_day: i64,
    pub failed_last_day: i64,
    pub entries: Vec<MailOutboxEntry>,
}
