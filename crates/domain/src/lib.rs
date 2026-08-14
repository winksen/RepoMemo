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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMembership {
    pub workspace_id: String,
    pub role: WorkspaceRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedSession {
    pub user: SharedUser,
    pub authentication: String,
    pub memberships: Vec<WorkspaceMembership>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub status: String,
    pub stage: String,
    pub progress_current: i64,
    pub progress_total: Option<i64>,
    pub error_message: Option<String>,
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
