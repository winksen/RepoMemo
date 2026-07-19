export interface Workspace {
  id: string;
  name: string;
  created_at: string;
  updated_at: string;
  settings: Record<string, unknown>;
}

export interface AppSettings {
  data_dir: string;
  ai_enabled: boolean;
  active_provider: string | null;
}

export type SourceType = "upload" | "folder" | "git_repo" | "manual" | "connector";

export type ArtifactType =
  | "file"
  | "markdown_doc"
  | "code_file"
  | "issue"
  | "pr"
  | "decision"
  | "incident"
  | "runbook"
  | "api_spec"
  | "note";

export interface WorkspaceOverview {
  workspace_id: string;
  source_count: number;
  artifact_count: number;
  chunk_count: number;
  symbol_count: number;
  memory_card_count: number;
}

export interface ArtifactSummary {
  id: string;
  workspace_id: string;
  source_id: string;
  source_name: string;
  artifact_type: ArtifactType;
  title: string;
  path: string;
  content_hash: string;
  mime_type: string | null;
  language: string | null;
  size_bytes: number;
  created_at: string;
  updated_at: string;
  indexed_at: string | null;
}

export interface ArtifactDetail {
  summary: ArtifactSummary;
  metadata: Record<string, unknown>;
  content_preview: string | null;
  content_truncated: boolean;
  chunks: Chunk[];
}

export interface Chunk {
  id: string;
  artifact_id: string;
  workspace_id: string;
  chunk_index: number;
  text: string;
  token_count: number | null;
  start_line: number | null;
  end_line: number | null;
  heading_path: string | null;
  content_hash: string;
  embedding_status: string;
  metadata: Record<string, unknown>;
}

export interface IndexingJobStatus {
  id: string;
  workspace_id: string;
  source_id: string | null;
  status: string;
  stage: string;
  progress_current: number;
  progress_total: number | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

export interface ImportSkippedItem {
  path: string;
  reason: string;
}

export interface ImportReport {
  workspace_id: string;
  scanned: number;
  imported: number;
  duplicates: number;
  skipped: number;
  failed: number;
  imported_artifacts: ArtifactSummary[];
  skipped_items: ImportSkippedItem[];
}

export interface SearchRequest {
  workspace_id: string;
  query: string;
  artifact_types: ArtifactType[];
  languages: string[];
  source_ids: string[];
  limit: number | null;
}

export interface SearchResult {
  artifact_id: string;
  chunk_id: string;
  title: string;
  path: string;
  artifact_type: ArtifactType;
  language: string | null;
  snippet: string;
  start_line: number | null;
  end_line: number | null;
  score: number;
  source_name: string;
}

export type SymbolKind =
  | "function"
  | "class"
  | "method"
  | "interface"
  | "enum"
  | "route"
  | "endpoint"
  | "config"
  | "test";

export interface Symbol {
  id: string;
  artifact_id: string;
  workspace_id: string;
  kind: SymbolKind;
  name: string;
  signature: string | null;
  start_line: number | null;
  end_line: number | null;
  metadata: Record<string, unknown>;
}

export interface SymbolSearchResult {
  symbol: Symbol;
  title: string;
  path: string;
  language: string | null;
  source_name: string;
}

export interface ProviderSettings {
  id: string;
  workspace_id: string | null;
  provider_type: string;
  name: string;
  base_url: string | null;
  model: string | null;
  embedding_model: string | null;
  enabled: boolean;
  metadata: Record<string, unknown>;
  api_key?: string | null;
}

export interface ProviderTestResult {
  provider_id: string;
  success: boolean;
  message: string;
}

export interface Citation {
  artifact_id: string;
  chunk_id: string | null;
  title: string;
  path: string;
  start_line: number | null;
  end_line: number | null;
  confidence: number | null;
}

export interface SummaryResult {
  summary_markdown: string;
  citations: Citation[];
  warnings: string[];
}

export interface AskRequest {
  workspace_id: string;
  question: string;
  provider_id: string | null;
  limit: number | null;
}

export interface AskAnswer {
  answer_markdown: string;
  citations: Citation[];
  retrieved_context: SearchResult[];
  confidence: number | null;
  warnings: string[];
}
