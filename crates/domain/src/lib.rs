use serde::{Deserialize, Serialize};
use serde_json::Value;

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub role: OrganizationRole,
    pub created_at: String,
    pub updated_at: String,
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
    Pdf { page_count: Option<usize> },
    Unavailable { reason: String },
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub data_dir: String,
    pub ai_enabled: bool,
    pub active_provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub workspace_id: String,
    pub query: String,
    pub artifact_types: Vec<ArtifactType>,
    pub languages: Vec<String>,
    pub source_ids: Vec<String>,
    pub limit: Option<i64>,
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

/// A job the workspace assistant knows how to do. The assistant only ever runs
/// one of these; a message that matches none of them gets a plain reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCapability {
    FindFiles,
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
}
