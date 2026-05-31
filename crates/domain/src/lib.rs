use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub settings: Value,
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
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    File,
    MarkdownDoc,
    CodeFile,
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
pub struct AppSettings {
    pub data_dir: String,
    pub ai_enabled: bool,
    pub active_provider: Option<String>,
}
