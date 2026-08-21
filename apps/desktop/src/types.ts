export interface Workspace {
  id: string;
  name: string;
  created_at: string;
  updated_at: string;
  settings: Record<string, unknown>;
}

export interface SharedUser {
  id: string;
  display_name: string;
  email: string | null;
}

export type WorkspaceRole = "owner" | "admin" | "member" | "viewer";

export interface WorkspaceMembership {
  workspace_id: string;
  role: WorkspaceRole;
}

export interface WorkspaceMember {
  user: SharedUser;
  role: WorkspaceRole;
  joined_at: string;
  updated_at: string;
}

export interface WorkspaceActivityEvent {
  id: string;
  workspace_id: string;
  actor: SharedUser | null;
  action: string;
  subject_type: string;
  subject_id: string | null;
  summary: string;
  created_at: string;
}

export interface SharedSession {
  user: SharedUser;
  authentication: "jwt";
  memberships: WorkspaceMembership[];
}

export interface UserProfile {
  user: SharedUser;
  created_at: string;
  updated_at: string;
  last_connected_at: string | null;
  workspace_count: number;
  recent_activity_count: number;
  activity_by_day: WorkspaceMetricBreakdown[];
}

export interface Organization {
  id: string;
  name: string;
  created_at: string;
  updated_at: string;
}

export interface SharedWorkspace {
  workspace: Workspace;
  organization_id: string;
  role: WorkspaceRole;
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
  | "image"
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

export interface WorkspaceMetricBreakdown {
  label: string;
  value: number;
}

export interface WorkspaceMetrics {
  workspace_id: string;
  generated_at: string;
  source_count: number;
  member_count: number;
  artifact_count: number;
  indexed_artifact_count: number;
  pending_artifact_count: number;
  total_artifact_bytes: number;
  indexed_artifact_bytes: number;
  pending_artifact_bytes: number;
  chunk_count: number;
  symbol_count: number;
  memory_card_count: number;
  recent_activity_count: number;
  artifacts_created_last_7_days: number;
  artifacts_updated_last_7_days: number;
  activity_actions: WorkspaceMetricBreakdown[];
  activity_by_day: WorkspaceMetricBreakdown[];
  member_roles: WorkspaceMetricBreakdown[];
  artifact_types: WorkspaceMetricBreakdown[];
  artifact_bytes_by_type: WorkspaceMetricBreakdown[];
  languages: WorkspaceMetricBreakdown[];
}

export interface WorkspaceCapabilities {
  role: WorkspaceRole;
  can_read: boolean;
  can_write_content: boolean;
  can_delete_content: boolean;
  can_manage_members: boolean;
  can_assign_admin: boolean;
  can_manage_workspace: boolean;
  can_generate_ai_overview: boolean;
}

export interface WorkspaceAiOverview {
  provider_configured: boolean;
  provider_name: string | null;
  summary_markdown: string | null;
  citations: Citation[];
  warnings: string[];
}

export interface SharedAiProviderSettings {
  id: string;
  provider_type: "ollama" | "openrouter";
  name: string;
  base_url: string | null;
  model: string | null;
  enabled: boolean;
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

export interface RetrievalFacets {
  artifact_types: ArtifactType[];
  languages: string[];
  sources: Array<{
    id: string;
    name: string;
  }>;
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

export interface AskAnswer {
  answer_markdown: string;
  citations: Citation[];
  retrieved_context: SearchResult[];
  confidence: number | null;
  warnings: string[];
}

export interface MemoryCard {
  id: string;
  workspace_id: string;
  title: string;
  body_markdown: string;
  source: string;
  confidence: number | null;
  created_at: string;
  updated_at: string;
  metadata: Record<string, unknown>;
}

export interface MemoryCardSummary {
  id: string;
  workspace_id: string;
  title: string;
  body_excerpt: string;
  source: string;
  evidence_count: number;
  created_at: string;
  updated_at: string;
}

export interface MemoryEvidence {
  link_id: string;
  target_id: string;
  target_type: string;
  artifact_id: string | null;
  chunk_id: string | null;
  title: string | null;
  path: string | null;
  start_line: number | null;
  end_line: number | null;
  exists: boolean;
}

export interface MemoryCardDetail {
  card: MemoryCard;
  evidence: MemoryEvidence[];
}

export interface CreateMemoryCardRequest {
  workspace_id: string;
  title: string;
  body_markdown: string;
  source: string;
  confidence: number | null;
  citations: Citation[];
}

export interface UpdateMemoryCardRequest {
  card_id: string;
  title: string;
  body_markdown: string;
  source: string;
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
