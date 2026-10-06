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
export type OrganizationRole = "owner" | "admin" | "member";

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

export interface WorkspaceActivityCalendar {
  total_activity_count: number;
  activity_by_day: WorkspaceMetricBreakdown[];
}

export type CollaborationTaskStatus = "open" | "in_progress" | "blocked" | "done";
export type CollaborationTaskPriority = "low" | "medium" | "high" | "urgent";

export interface CollaborationTask {
  id: string;
  workspace_id: string;
  title: string;
  description: string;
  status: CollaborationTaskStatus;
  priority: CollaborationTaskPriority;
  assignee: SharedUser | null;
  created_by: SharedUser;
  artifact_id: string | null;
  due_at: string | null;
  completed_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface SavedSearch {
  id: string; workspace_id: string; name: string; query: string; artifact_types: ArtifactType[]; languages: string[]; source_ids: string[]; result_limit: number; created_by: SharedUser; created_at: string; updated_at: string;
}

export interface TaskChecklistItem {
  id: string; task_id: string; workspace_id: string; body: string; position: number; completed_at: string | null; completed_by: SharedUser | null; created_by: SharedUser; created_at: string; updated_at: string;
}

export interface ArtifactComment {
  id: string;
  workspace_id: string;
  artifact_id: string;
  author: SharedUser;
  body: string;
  created_at: string;
  updated_at: string;
}

export type ArtifactLifecycleStatus = "active" | "needs_review" | "verified" | "outdated" | "superseded";

export interface ArtifactLifecycle {
  artifact_id: string;
  workspace_id: string;
  status: ArtifactLifecycleStatus;
  owner: SharedUser | null;
  review_note: string;
  reviewed_by: SharedUser | null;
  reviewed_at: string | null;
  superseded_by_artifact_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface ArtifactLifecycleEvent {
  id: string;
  artifact_id: string;
  actor: SharedUser | null;
  action: string;
  detail: string;
  created_at: string;
}

export interface SharedNotification {
  id: string;
  workspace_id: string | null;
  notification_type: "task_assigned" | "evidence_mention" | string;
  title: string;
  body: string;
  href: string;
  read_at: string | null;
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
  role: OrganizationRole;
  created_at: string;
  updated_at: string;
}

export interface OrganizationMember {
  user: SharedUser;
  role: OrganizationRole;
  joined_at: string;
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
  /** Passages with a search vector; null when no embedding provider is set up. */
  embedded_chunk_count: number | null;
  symbol_count: number;
  memory_card_count: number;
  open_task_count: number;
  in_progress_task_count: number;
  blocked_task_count: number;
  completed_task_count: number;
  overdue_task_count: number;
  comment_count: number;
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
  can_create_tasks: boolean;
  can_comment: boolean;
  can_moderate_comments: boolean;
  can_inspect_index: boolean;
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
  purpose: "text" | "vision" | "embedding";
}

export interface SheetPreview {
  name: string;
  rows: string[][];
  total_rows: number;
  total_columns: number;
}

export interface SlidePreview {
  number: number;
  title: string | null;
  text: string[];
  notes: string | null;
}

export interface EmailAttachmentInfo {
  name: string;
  size_bytes: number;
}

export type DocumentPreview =
  | { kind: "text"; text: string; truncated: boolean; approximate: boolean }
  | { kind: "sheets"; sheets: SheetPreview[]; total_sheets: number }
  | { kind: "slides"; slides: SlidePreview[]; total_slides: number }
  | {
      kind: "email";
      subject: string | null;
      from: string | null;
      to: string[];
      cc: string[];
      date: string | null;
      body: string;
      truncated: boolean;
      attachments: EmailAttachmentInfo[];
    }
  | { kind: "pdf"; page_count: number | null }
  | { kind: "unavailable"; reason: string };

export interface FileLink {
  token: string;
  filename: string;
  expires_in_seconds: number;
}

export interface Folder {
  id: string;
  workspace_id: string;
  parent_id: string | null;
  name: string;
  created_at: string;
}

export interface ArtifactIndexFailure {
  artifact_id: string;
  message: string;
  attempts: number;
  failed_at: string;
}

export interface ArtifactSummary {
  folder_id?: string | null;
  /** Set when the file is synced from a connected git repository. */
  repository_id?: string | null;
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
  kind?: string;
  cancel_requested?: boolean;
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

export type IndexState = "indexed" | "pending" | "failed";

export interface KnowledgePipeline {
  file_count: number;
  indexed_count: number;
  pending_count: number;
  failed_count: number;
  passage_count: number;
  /** null when no embedding provider is set up. */
  embedded_count: number | null;
  embedding_model: string | null;
}

export interface KnowledgeCoverage {
  /** A readable kind such as "PDF", "Markdown" or "Code". */
  label: string;
  file_count: number;
  indexed_count: number;
  pending_count: number;
  failed_count: number;
  passage_count: number;
  embedded_count: number;
}

export interface KnowledgeNode {
  id: string;
  kind: "file" | "memory";
  title: string;
  path: string | null;
  artifact_type: ArtifactType | null;
  state: IndexState | null;
  passage_count: number;
  embedded_count: number;
}

export interface KnowledgeEdge {
  source: string;
  target: string;
  kind: "similar" | "cites";
  weight: number;
}

export interface KnowledgeMap {
  pipeline: KnowledgePipeline;
  coverage: KnowledgeCoverage[];
  nodes: KnowledgeNode[];
  edges: KnowledgeEdge[];
  hidden_file_count: number;
  similarity_available: boolean;
}

export type HealthDetector =
  | "older_version_active"
  | "duplicate_content"
  | "removed_symbol_mentioned"
  | "outdated_evidence_referenced"
  | "index_failed"
  | "unconnected";

export type HealthAction = "supersede" | "needs_review" | "mark_outdated" | "create_task" | "dismiss";

export interface HealthFile {
  artifact_id: string;
  title: string;
  path: string;
  created_at: string;
}

export interface HealthEvidence {
  artifact_id: string;
  title: string;
  start_line: number | null;
  end_line: number | null;
  excerpt: string;
}

export interface HealthFinding {
  fingerprint: string;
  detector: HealthDetector;
  severity: "warning" | "info";
  title: string;
  detail: string;
  files: HealthFile[];
  evidence: HealthEvidence[];
  keep_artifact_id: string | null;
  actions: HealthAction[];
}

export interface HealthDetectorStats {
  detector: HealthDetector;
  open_count: number;
  acted_count: number;
  dismissed_count: number;
}

export interface WorkspaceHealth {
  findings: HealthFinding[];
  detectors: HealthDetectorStats[];
  checked_file_count: number;
  similarity_available: boolean;
}

export interface HealthActionResult {
  action: HealthAction;
  updated_artifact_ids: string[];
  task_id: string | null;
}

export type AgentCapability = "find_files" | "search_content" | "summarize_file" | "ask_question" | "workspace_overview";

export interface AgentCapabilityInfo {
  id: AgentCapability;
  label: string;
  description: string;
  placeholder: string;
  requires_ai: boolean;
  available: boolean;
}

export interface AgentCapabilities {
  capabilities: AgentCapabilityInfo[];
  provider_name: string | null;
}

export interface AgentMessage {
  message: string;
  capability?: AgentCapability | null;
  artifact_id?: string | null;
}

export interface AgentConversation {
  id: string;
  workspace_id: string;
  title: string;
  turn_count: number;
  created_at: string;
  updated_at: string;
}

export interface AgentTurn {
  id: string;
  conversation_id: string;
  position: number;
  label: string;
  request: AgentMessage;
  reply: AgentReply;
  created_at: string;
}

export interface AgentConversationDetail {
  conversation: AgentConversation;
  turns: AgentTurn[];
}

export interface AgentTurnResponse {
  conversation: AgentConversation;
  turn: AgentTurn;
}

export interface AgentReply {
  capability: AgentCapability | null;
  routing: "explicit" | "rules" | "model" | "unmatched";
  subject: string;
  reply_markdown: string;
  generated: boolean;
  files: ArtifactSummary[];
  matches: SearchResult[];
  citations: Citation[];
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

export interface RepoSettings {
  branch: string | null;
  include: string[];
  exclude: string[];
}

export interface RepoCommit {
  sha: string;
  summary: string;
  author_name: string;
  committed_at: string;
  branch: string | null;
}

export interface RepoSkipCount {
  reason: string;
  count: number;
  examples: string[];
}

export interface RepoSyncReport {
  commit_sha: string;
  files_in_tree: number;
  added: number;
  updated: number;
  renamed: number;
  removed: number;
  restored: number;
  unchanged: number;
  skipped: number;
  skipped_by_reason: RepoSkipCount[];
  indexed: number;
  index_failed: number;
  cancelled: boolean;
}

export interface RepoSource {
  id: string;
  workspace_id: string;
  name: string;
  root_path: string;
  settings: RepoSettings;
  status: "pending" | "syncing" | "ready" | "error";
  last_synced_commit: RepoCommit | null;
  last_synced_at: string | null;
  last_error: string | null;
  last_report: RepoSyncReport | null;
  file_count: number;
  indexed_file_count: number;
  active_job: IndexingJobStatus | null;
  created_at: string;
}

export interface RepoFile {
  path: string;
  artifact_id: string;
  artifact_type: ArtifactType;
  language: string | null;
  size_bytes: number;
  indexed: boolean;
  index_failure: string | null;
  commit_sha: string;
}

export interface RepositoryList {
  repositories: RepoSource[];
}

/** What the server found at a repository link, before it is linked. */
export interface RepoAccessCheck {
  root_path: string;
  name: string;
  commit: RepoCommit;
  tracked_files: number;
  indexable_files: number;
  already_connected: boolean;
}
