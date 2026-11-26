// Types of the API's data transfer objects that the Rust domain crate defines are
// generated (src/generated/domain.ts, `npm run types:generate`) and re-exported here.
// The declarations below are the ones the client shapes differently from the wire.
import type {
  AgentCapability,
  AgentCapabilityInfo,
  AgentConversation,
  AgentFileSet,
  AgentProposal,
  AppSettings,
  ArtifactComment,
  ArtifactIndexFailure,
  ArtifactLifecycleEvent,
  ArtifactType,
  AskAnswer,
  AskRequest,
  Citation,
  CountByLabel,
  CreateMemoryCardRequest,
  DocumentPreview,
  EmailAttachmentInfo,
  Folder,
  HealthAction,
  HealthActionResult,
  HealthDetector,
  HealthDetectorStats,
  HealthEvidence,
  HealthFile,
  HealthFinding,
  ImportSkippedItem,
  IndexState,
  KnowledgeCoverage,
  KnowledgeEdge,
  KnowledgeMap,
  KnowledgeNode,
  KnowledgePipeline,
  MemoryCardSummary,
  MemoryEvidence,
  MemoryReviewReason,
  ArtifactMention,
  ArtifactMentions,
  WebhookDelivery,
  ConnectorAccessCheck,
  CrossSearchResponse,
  ReviewItem,
  ReviewItemKind,
  ReviewQueue,
  CitedSource,
  SearchFeedback,
  FolderMovePreview,
  TrashItem,
  TrashRestoreResult,
  MemoryChange,
  MemoryChanges,
  MemoryApprovalDecision,
  MemoryApprovalStatus,
  ReviewAssignmentView,
  MemoryRevision,
  MemoryRevisionCitation,
  MemoryRevisionComparison,
  MemoryRestoreResult,
  MyWork,
  MyWorkReview,
  OnboardingStep,
  WorkspaceOnboarding,
  ArtifactVersion,
  ArtifactVersionText,
  DiffKind,
  VersionComparison,
  VersionDiffLine,
  ConnectorSource,
  ConnectorSyncReport,
  OrganizationMember,
  OrganizationRole,
  AgentRun,
  AgentStep,
  ProviderTestResult,
  RepoAccessCheck,
  RepoCommit,
  RepoFile,
  CodeOwnerMatch,
  RepoFileAtCommit,
  RepoFolderShare,
  RepoKeyFile,
  RepoLanguageShare,
  RepoSettings,
  RepoSkipCount,
  RepoStack,
  RepoSummary,
  RepoSyncReport,
  RepoTechnology,
  SavedSearch,
  SearchRequest,
  SearchResult,
  SharedNotification,
  SharedUser,
  SheetPreview,
  SlidePreview,
  SourceType,
  SummaryResult,
  SymbolKind,
  SystemAuditEvent,
  SystemStatistics,
  TaskChecklistItem,
  UpdateMemoryCardRequest,
  WorkspaceActivityEvent,
  WorkspaceAiOverview,
  WorkspaceCapabilities,
  WorkspaceHealth,
  WorkspaceMember,
  WorkspaceMembership,
  WorkspaceOverview,
  WorkspaceRole,
  WorkspaceUsage,
} from "./generated/domain";
export type {
  MailOutboxEntry,
  MailOverview,
  MailSettingsView,
} from "./generated/domain";
export type {
  TemplateDetail,
  TemplateSummary,
  WorkspaceTemplateView,
} from "./generated/domain";
export type {
  AgentCapability,
  AgentCapabilityInfo,
  AgentConversation,
  AgentFileSet,
  AgentProposal,
  AppSettings,
  ArtifactComment,
  ArtifactIndexFailure,
  ArtifactLifecycleEvent,
  ArtifactType,
  AskAnswer,
  AskRequest,
  Citation,
  CountByLabel,
  CreateMemoryCardRequest,
  DocumentPreview,
  EmailAttachmentInfo,
  Folder,
  HealthAction,
  HealthActionResult,
  HealthDetector,
  HealthDetectorStats,
  HealthEvidence,
  HealthFile,
  HealthFinding,
  ImportSkippedItem,
  IndexState,
  KnowledgeCoverage,
  KnowledgeEdge,
  KnowledgeMap,
  KnowledgeNode,
  KnowledgePipeline,
  MemoryCardSummary,
  MemoryEvidence,
  MemoryReviewReason,
  ArtifactMention,
  ArtifactMentions,
  WebhookDelivery,
  ConnectorAccessCheck,
  CrossSearchResponse,
  ReviewItem,
  ReviewItemKind,
  ReviewQueue,
  CitedSource,
  SearchFeedback,
  FolderMovePreview,
  TrashItem,
  TrashRestoreResult,
  MemoryChange,
  MemoryChanges,
  MemoryApprovalDecision,
  MemoryApprovalStatus,
  ReviewAssignmentView,
  MemoryRevision,
  MemoryRevisionCitation,
  MemoryRevisionComparison,
  MemoryRestoreResult,
  MyWork,
  MyWorkReview,
  OnboardingStep,
  WorkspaceOnboarding,
  ArtifactVersion,
  ArtifactVersionText,
  DiffKind,
  VersionComparison,
  VersionDiffLine,
  ConnectorSource,
  ConnectorSyncReport,
  OrganizationMember,
  OrganizationRole,
  AgentRun,
  AgentStep,
  ProviderTestResult,
  RepoAccessCheck,
  RepoCommit,
  RepoFile,
  CodeOwnerMatch,
  RepoFileAtCommit,
  RepoFolderShare,
  RepoKeyFile,
  RepoLanguageShare,
  RepoSettings,
  RepoSkipCount,
  RepoStack,
  RepoSummary,
  RepoSyncReport,
  RepoTechnology,
  SavedSearch,
  SearchRequest,
  SearchResult,
  SharedNotification,
  SharedUser,
  SheetPreview,
  SlidePreview,
  SourceType,
  SummaryResult,
  SymbolKind,
  SystemAuditEvent,
  SystemStatistics,
  TaskChecklistItem,
  UpdateMemoryCardRequest,
  WorkspaceActivityEvent,
  WorkspaceAiOverview,
  WorkspaceCapabilities,
  WorkspaceHealth,
  WorkspaceMember,
  WorkspaceMembership,
  WorkspaceOverview,
  WorkspaceRole,
  WorkspaceUsage,
} from "./generated/domain";

export interface Workspace {
  id: string;
  name: string;
  created_at: string;
  updated_at: string;
  settings: Record<string, unknown>;
}

/** One message of a workspace (or System) event stream. `resync` means
 *  events were missed, so whatever is shown should be reloaded. */
export type LiveEvent =
  | { type: "job"; job: IndexingJobStatus }
  | { type: "activity"; event: WorkspaceActivityEvent }
  | { type: "resync"; missed?: number };

export interface WorkspaceActivityCalendar {
  total_activity_count: number;
  activity_by_day: WorkspaceMetricBreakdown[];
}

export type CollaborationTaskStatus = "open" | "in_progress" | "blocked" | "done";
export type CollaborationTaskPriority = "low" | "medium" | "high" | "urgent";

export interface CollaborationTask {
  id: string;
  workspace_id: string;
  number: number;
  key: string;
  version: number;
  title: string;
  description: string;
  status: CollaborationTaskStatus;
  priority: CollaborationTaskPriority;
  assignee: SharedUser | null;
  created_by: SharedUser;
  artifact_id: string | null;
  project_id: string | null;
  project_name: string | null;
  milestone_id: string | null;
  milestone_name: string | null;
  parent_task_id: string | null;
  parent_key: string | null;
  child_count: number;
  open_child_count: number;
  open_blocker_count: number;
  blocks_count: number;
  state_id: string;
  state_name: string;
  labels: { id: string; name: string; color: string; background_color: string; text_color: string }[];
  due_at: string | null;
  completed_at: string | null;
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

export interface SharedSession {
  user: SharedUser;
  authentication: "jwt";
  memberships: WorkspaceMembership[];
  /** Administers the whole server: every organization and workspace, and the System pages. */
  is_system_admin?: boolean;
  /** Uses the System pages without the wildcard administrator access. */
  is_app_admin?: boolean;
  /** Administers CAIRN's models, limits and settings: by a system administrator's grant, or by being one. */
  is_cairn_admin?: boolean;
  /** The server lets this person create an organization; absent on an older server, where everyone could. */
  can_create_organizations?: boolean;
}

export interface UserProfile {
  user: SharedUser;
  /** The address was confirmed, or came with an invitation. */
  email_verified?: boolean;
  /** Unverified accounts are held back on this server: registration is open and mail is set up. */
  verification_required?: boolean;
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
  /** A short description (up to 500 characters), shown on the organization's card and page. */
  description?: string | null;
  /** The key of the icon that stands for the organization; the client owns the set of icons. */
  icon?: string | null;
  /** Listed only for workspaces the user was added to directly: not a member of the organization, so `role` is only for display. */
  guest?: boolean;
}

export interface SharedWorkspace {
  workspace: Workspace;
  organization_id: string;
  role: WorkspaceRole;
}

/** One workspace's row in an organization's Overview. */
export interface OrganizationOverviewWorkspace {
  workspace_id: string;
  name: string;
  artifact_count: number;
  memory_card_count: number;
  /** Tasks that are open or in progress. */
  active_task_count: number;
  /** Events in the last 30 days; `null` when the person does not administer the workspace. */
  activity_count: number | null;
}

/** An organization's Overview: totals and rows for the workspaces of it the person can open. */
export interface OrganizationOverview {
  organization_id: string;
  member_count: number;
  workspace_count: number;
  artifact_count: number;
  memory_card_count: number;
  active_task_count: number;
  /** Events a day for the last 30 days, oldest first; empty when the person administers none of the workspaces. */
  activity_by_day: WorkspaceMetricBreakdown[];
  workspaces: OrganizationOverviewWorkspace[];
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
  /** Linked git repositories, and how many finished their last sync. */
  repository_count: number;
  repositories_in_sync: number;
  /** Files whose lifecycle was reviewed in the last 90 days. */
  reviewed_last_90_days: number;
  /** Files marked needs review, and memory cards gone stale. */
  needing_review_files: number;
  needing_review_cards: number;
  /** Files that repeat the exact content of another file. */
  duplicate_file_count: number;
  activity_actions: WorkspaceMetricBreakdown[];
  activity_by_day: WorkspaceMetricBreakdown[];
  member_roles: WorkspaceMetricBreakdown[];
  artifact_types: WorkspaceMetricBreakdown[];
  artifact_bytes_by_type: WorkspaceMetricBreakdown[];
  languages: WorkspaceMetricBreakdown[];
}

/** The lowest workspace role allowed to use AI features. */
export type AiMinRole = "viewer" | "member" | "admin";

export interface AiPolicy {
  min_role: AiMinRole;
  /** AI requests each person may make per hour on this server; 0 is unlimited. */
  requests_per_hour: number;
}

export interface SharedAiProviderSettings {
  id: string;
  provider_type: "ollama" | "openrouter" | "openai_compatible";
  name: string;
  base_url: string | null;
  model: string | null;
  enabled: boolean;
  purpose: "text" | "vision" | "embedding";
  /** How the model takes tools; set by Test connection or an administrator. */
  tool_mode?: "native" | "json" | "none" | null;
  /** The model's context window in tokens. */
  context_tokens?: number | null;
  /** How CAIRN answers in the workspace: one job per message, or a loop of tool calls. */
  cairn_mode?: "jobs" | "agent" | null;
}

export interface FileLink {
  token: string;
  filename: string;
  expires_in_seconds: number;
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
  index_version: number;
  source_synced_at: string | null;
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

export interface RetrievalFacets {
  artifact_types: ArtifactType[];
  languages: string[];
  sources: Array<{
    id: string;
    name: string;
  }>;
}

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

export interface AgentCapabilities {
  capabilities: AgentCapabilityInfo[];
  provider_name: string | null;
  /** The model the workspace's text provider is set to use, when it names one. */
  model?: string | null;
  /** Whether CAIRN works in a loop in this workspace, so a message can run live. */
  agent_mode?: boolean;
  /** The models a person can pick for a chat here (V0.9.86); the one that answers without a choice is marked `default`. */
  models?: OfferedModel[];
  /** What the person has left of today's CAIRN budgets; none when no budget is set (V0.9.96). */
  budget?: { runs_limit: number | null; tokens_limit: number | null; runs_used: number; tokens_used: number; resets_at: string } | null;
}

/** A model offered in a workspace, as the chat's picker shows it. */
export interface OfferedModel {
  id: string;
  name: string;
  locality: "local" | "cloud";
  /** It takes tools, so CAIRN can work in a loop with it. */
  agent: boolean;
  default: boolean;
}

export interface AgentMessage {
  message: string;
  capability?: AgentCapability | null;
  artifact_id?: string | null;
  /** Files selected in an earlier reply that the message is about (V0.9.26). */
  artifact_ids?: string[];
  /** The model picked for the chat (V0.9.86); the chat keeps it for the next messages. */
  model_id?: string | null;
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
  routing: "explicit" | "rules" | "model" | "agent" | "unmatched";
  subject: string;
  reply_markdown: string;
  generated: boolean;
  files: ArtifactSummary[];
  matches: SearchResult[];
  citations: Citation[];
  warnings: string[];
  /** Earlier turns of the chat that went with the question. */
  context_turns?: number;
  /** Actions CAIRN proposes; each waits for the person to confirm it (V0.9.10). */
  proposals?: AgentProposal[];
  /** Every file the reply found, by id, to select from (V0.9.26); `files` holds the first of them. */
  file_set?: AgentFileSet | null;
  /** The memory cards a reply found (V0.9.29). */
  memory_cards?: MemoryCardSummary[];
  /** The proposals of the reply as one set to apply together (V0.9.44). */
  changeset?: AgentChangeset | null;
  /** How far the sources the answer cites can be trusted (V0.9.45). */
  trust?: AgentTrust | null;
}

export interface AgentTrust {
  summary: string;
  sources: { artifact_id: string; state: string; untrusted?: string | null; synced_days_ago?: number | null }[];
}

export interface AgentChangeset {
  state: "pending" | "applied" | "partial" | "dismissed";
  outcome?: string | null;
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

export interface MemoryCardDetail {
  card: MemoryCard;
  evidence: MemoryEvidence[];
  /** Why the card needs another look; empty when it is up to date. */
  review: MemoryReviewReason[];
  /** The newest revision; an editor sends it back so a save based on an older one is refused (V0.6.4). */
  revision: number;
}

export interface RepoSource {
  id: string;
  workspace_id: string;
  name: string;
  /** A folder on the server, or the address of a remote repository. */
  root_path: string;
  /** Fetched from a host over https; the server keeps a partial clone. */
  remote: boolean;
  /** A remote repository has a token stored (never shown). */
  has_credential: boolean;
  /** Push webhooks are set up for this repository. */
  has_webhook: boolean;
  /** When a correctly signed webhook call last arrived. */
  webhook_last_received_at: string | null;
  settings: RepoSettings;
  status: "pending" | "syncing" | "ready" | "error";
  last_synced_commit: RepoCommit | null;
  last_synced_at: string | null;
  last_error: string | null;
  last_report: RepoSyncReport | null;
  file_count: number;
  indexed_file_count: number;
  active_job: IndexingJobStatus | null;
  /** The evidence item that stands for the whole repository. */
  overview_artifact_id: string | null;
  created_at: string;
}

export interface RepoOverview {
  commit: RepoCommit;
  generated_at: string;
  file_count: number;
  total_bytes: number;
  languages: RepoLanguageShare[];
  folders: RepoFolderShare[];
  key_files: RepoKeyFile[];
  readme_excerpt: string | null;
  recent_commits: RepoCommit[];
  /** Absent for repositories synced before stack detection existed. */
  stack?: RepoStack | null;
}

export interface RepositoryDetailResponse {
  repository: RepoSource;
  overview: RepoOverview | null;
  summary: RepoSummary | null;
  ai_available: boolean;
}

export interface RepositoryList {
  repositories: RepoSource[];
}

// System administration

export interface MaintenanceReport {
  refresh_tokens_purged: number;
  access_links_purged?: number;
  jobs_pruned: number;
  blobs_removed: number;
  blob_bytes_reclaimed: number;
  previews_removed: number;
  untracked_files_removed: number;
  counters_pruned: number;
  channels_pruned: number;
  system_events_pruned: number;
  failures: number;
}

export interface BackupManifest {
  name: string;
  created_at: string;
  version: string;
  trigger: "manual" | "scheduled";
  database_bytes: number;
  blob_count: number;
  blob_bytes: number;
  missing_blobs: number;
  reused_blobs: number;
  includes_key: boolean;
  /** Present when the backup is sealed; holds nothing that can open it. */
  encryption?: { cipher: string; kdf: string; iterations: number; salt: string; key_check: string; chunk_bytes: number } | null;
  /** Files in the backup and their size; what a second copy must add up to. */
  files?: number;
  bytes?: number;
  /** Present once the backup was copied to the second location and checked. */
  copy?: { destination: string; copied_at: string; files: number; bytes: number; linked: number } | null;
  /** Which configured key opens a sealed backup: the current one, the previous one (it can be sealed again) or neither. */
  sealed_with?: "current" | "previous" | "neither" | null;
}

export interface BackupStatus {
  running: boolean;
  operation: "backup" | "restore" | "copy" | "reseal" | null;
  started_at: string | null;
  finished_at: string | null;
  succeeded: boolean | null;
  message: string | null;
}

export interface BackupsOverview {
  backups: BackupManifest[];
  status: BackupStatus;
  directory: string;
  keep: number;
  interval_hours: number;
  disk_free_bytes: number | null;
  /** Whether new backups are sealed (REPOMEMO_BACKUP_KEY is set). */
  encryption_enabled: boolean;
  /** Whether the key before the last change is kept (REPOMEMO_BACKUP_KEY_PREVIOUS), so older backups can be opened and sealed again. */
  previous_key_set?: boolean;
  /** The folder every backup is copied to, when one is set. */
  copy_directory?: string | null;
}

export interface MaintenanceStatus {
  running: boolean;
  last_started_at: string | null;
  last_finished_at: string | null;
  last_trigger: "scheduled" | "manual" | null;
  last_report: MaintenanceReport | null;
  next_run_at: string | null;
}

export interface SystemClient {
  origin: string | null;
  client: string | null;
  agent: string;
  last_address: string;
  first_seen_at: string;
  last_seen_at: string;
  requests: number;
}

export interface SystemOverview {
  instance: {
    service_name: string;
    version: string;
    host_name: string;
    operating_system: string;
    architecture: string;
    process_id: number;
    bind_address: string;
    data_dir: string;
    started_at: string;
    uptime_seconds: number;
    system_admin_count: number;
  };
  statistics: SystemStatistics;
  storage: {
    database_bytes: number;
    write_ahead_log_bytes: number;
    blob_bytes: number;
    preview_bytes: number;
    preview_count: number;
    /** Free space on the disk of the data folder, when the server can tell. */
    disk_free_bytes: number | null;
    /** Uploads are refused below this; 0 when the check is off. */
    disk_floor_bytes: number;
    /** Storage one workspace may hold; 0 is unlimited. */
    workspace_storage_bytes: number;
    /** What the server's copies of remote repositories take. */
    remote_clone_bytes: number;
    /** The copies, largest first. */
    remote_clones: { source_id: string; workspace_id: string; name: string; bytes: number }[];
    /** The most one copy may grow to; 0 is no cap. */
    remote_clone_limit_bytes: number;
  };
  background: {
    indexing_queued: number;
    indexing_oldest_queue_seconds: number | null;
    indexing_retries_waiting: number;
    indexing_retries_scheduled: number;
    embedding_workspaces_waiting: number;
    embedding_oldest_queue_seconds: number | null;
    repository_syncs_running: number;
    preview_conversions_running: number;
    office_previews: { available: boolean; installed: boolean; requested: boolean; refused: string | null; version: string | null; sandboxed: boolean };
    event_channels: number;
    event_subscribers: number;
    open_event_streams: number;
    maintenance: MaintenanceStatus;
  };
  protection: { tracked_rate_limit_keys: number; locked_sign_in_keys: number };
  traffic: {
    requests: number;
    client_errors: number;
    server_errors: number;
    unauthorized: number;
    forbidden: number;
    rate_limited: number;
    requests_last_hour: number;
    requests_per_minute_last_hour: number[];
  };
  clients: SystemClient[];
  logs: { capturing: boolean; buffered: number };
  /** The operator's folder of workspace templates and what became of its files. */
  template_files: SystemTemplateFiles;
}

/** What REPOMEMO_TEMPLATES_DIR gave: the templates it added and the files it left out, each with why. */
export interface SystemTemplateFiles {
  folder: string | null;
  /** Every template the server offers, `general` first. */
  templates: { id: string; name: string; version: number; source: "code" | "built-in" | "custom"; file: string | null }[];
  loaded: { id: string; name: string; file: string }[];
  problems: { file: string; issues: { path: string; message: string; line: number | null; column: number | null }[] }[];
}

export interface SystemUsage {
  days: number;
  activity_by_day: CountByLabel[];
  activity_by_action: CountByLabel[];
  top_users: CountByLabel[];
  workspaces: WorkspaceUsage[];
}

/** A connection to a model server in the catalog (V0.9.76). The key is write-only: there is only whether one is set. */
export interface CairnConnection {
  id: string;
  kind: "ollama" | "openrouter" | "openai_compatible";
  name: string;
  base_url: string;
  has_key: boolean;
  locality: "local" | "cloud";
  enabled: boolean;
  cloud_acknowledged_at: string | null;
  cloud_acknowledged_by: string | null;
  version: number;
  created_at: string;
  updated_at: string;
  updated_by: string | null;
}

/** An organization's CAIRN settings (V0.9.83). `version` is 0 for an organization that never saved any. */
export interface OrganizationCairnSettings {
  organization_id: string;
  version: number;
  enabled_model_ids: string[];
  default_model_id: string | null;
  /** `auto` keeps the live-evaluation rule; `agent` and `jobs` set the mode of every workspace that has not chosen its own. */
  agent_mode: "auto" | "agent" | "jobs";
  /** The roles that may use a model, by model id (V0.9.87); a model with no entry is open to everyone who may use AI. */
  model_roles: Record<string, string[]>;
  /** What CAIRN may do in the organization's workspaces (V0.9.91 on). */
  guardrails: Guardrails;
  updated_at: string | null;
  updated_by: string | null;
}

/** What CAIRN may do, see, send and spend; rows add fields. */
export interface Guardrails {
  /** CAIRN jobs and tools turned off, by name (V0.9.91). */
  tools_off: string[];
  /** The most changes one confirmation may make (V0.9.92); none keeps the default of 200. */
  max_changes?: number | null;
  /** The number of changes above which the number is typed to confirm (V0.9.92); none keeps the rule for content from outside. */
  type_to_confirm_from?: number | null;
  /** Folders CAIRN does not read (V0.9.93): a workspace's own, with their subfolders. */
  closed_folder_ids?: string[];
  /** What happens to a secret on its way to a cloud model (V0.9.94): masked by default, or refused. */
  secrets_for_cloud?: "mask" | "refuse" | null;
  /** Also replace email addresses and phone numbers for cloud models (V0.9.95); off by default. */
  mask_personal_data?: boolean;
  /** The most runs and tokens a person may use in a day (V0.9.96). */
  daily_runs_per_person?: number | null;
  daily_tokens_per_person?: number | null;
  budget_exempt_admins?: boolean;
}

/** A CAIRN job or tool that can be turned off (V0.9.91). */
export interface CairnTool {
  name: string;
  label: string;
  kind: "job" | "tool";
  description: string;
  writes: boolean;
}

/** A workspace's CAIRN model settings and the models of its organization it can offer (V0.9.85). `version` is 0 when it never saved any. */
export interface WorkspaceCairnView {
  workspace_id: string;
  version: number;
  offered_model_ids: string[];
  default_model_id: string | null;
  model_roles: Record<string, string[]>;
  guardrails: Guardrails;
  updated_at: string | null;
  updated_by: string | null;
  /** What the organization turned off, which the workspace cannot turn on (V0.9.91). */
  organization_tools_off: string[];
  /** What the organization set for one confirmation, which the workspace can only tighten (V0.9.92). */
  organization_limits: { max_changes: number | null; type_to_confirm_from: number | null; secrets_for_cloud?: "mask" | "refuse" | null; mask_personal_data?: boolean };
  /** The workspace's folders, each marked when it is closed to CAIRN (V0.9.93). */
  folders: Folder[];
  organization_default_model_id: string | null;
  organization_decided: boolean;
  models: (CairnModel & { from_organization: boolean; offered: boolean; answering: boolean })[];
  /** The embedding and vision models of the catalog the organization enabled, and the one the workspace uses now (V0.9.89). */
  embedding: PurposeChoice;
  vision: PurposeChoice;
}

export interface PurposeChoice {
  current_model_id: string | null;
  current_name: string | null;
  candidates: CairnModel[];
}

/** A live evaluation report attached to a model (V0.9.79), as `npm run cairn:eval` measures it. */
export interface CairnEvaluation {
  passed: boolean;
  /** The day the run was made, `YYYY-MM-DD`. */
  run_on: string;
  citation_precision: number;
  tool_accuracy: number;
  forbidden_actions: number;
  cases_run: number;
  version: string | null;
}

/** A model of a connection in the catalog (V0.9.77). */
export interface CairnModel {
  id: string;
  connection_id: string;
  /** The name the model server knows it by. */
  model_id: string;
  display_name: string;
  purpose: "text" | "vision" | "embedding";
  tool_mode: "none" | "json" | "native";
  context_tokens: number;
  vision: boolean;
  evaluation: CairnEvaluation | null;
  fallback_model_id: string | null;
  roles: string[] | null;
  enabled: boolean;
  version: number;
  created_at: string;
  updated_at: string;
  updated_by: string | null;
  /** In how many places the model is used; a model in use cannot be deleted. */
  uses: number;
  /** Where the model runs, from its connection: shown wherever a model is listed or chosen (V0.9.81). */
  locality: "local" | "cloud";
  /** The model and its connection are on, and a connection that leaves the network has its confirmation. */
  selectable: boolean;
}

/** A model a connection's server offers, and the purposes it is already in the catalog for. */
export interface AvailableCairnModel {
  model_id: string;
  added_for: string[];
}

/** What testing a model found; `model` has the traits the test filled in. */
export interface CairnModelTest {
  success: boolean;
  message: string;
  tool_mode: string | null;
  context_tokens: number | null;
  model: CairnModel;
}

/** What testing a connection found: whether the server answered and took the key. */
export interface CairnConnectionTest {
  connection_id: string;
  reachable: boolean;
  key_accepted: boolean | null;
  model_count: number | null;
  message: string;
}

/** What a connection is saved with. */
export interface CairnConnectionInput {
  kind?: string;
  name: string;
  base_url?: string;
  api_key?: string;
  enabled: boolean;
  cloud_content_acknowledged: boolean;
  expected_version?: number;
}

/** A CAIRN run budget: a run-time setting a CAIRN administrator changes, applied to the next run (V0.9.74). */
export interface CairnBudget {
  key: string;
  label: string;
  description: string;
  unit: string;
  min: number;
  max: number;
  value: number;
  default_value: number;
  /** Changed on the page, overriding the environment's value. */
  overridden: boolean;
  updated_at: string | null;
  updated_by: string | null;
}

/** Who the CAIRN administration page is for and what it shows (V0.9.72). */
export interface CairnAdminView {
  /** `server` for system, CAIRN and app administrators; `organization` for owners and admins of organizations. */
  scope: "server" | "organization";
  /** App administrators read the page and change nothing. */
  can_change: boolean;
  sections: string[];
  organizations: { id: string; name: string }[];
  /** Today's setup, read only; present for `server` only. */
  overview: {
    workspaces: {
      workspace_id: string;
      workspace_name: string;
      organization_id: string;
      organization_name: string;
      provider_name: string | null;
      provider_type: string | null;
      model: string | null;
      mode: "agent" | "jobs" | null;
    }[];
    budgets: {
      /** What CAIRN administrators edit: steps, run seconds, run tokens, background files and trace days. */
      settings: CairnBudget[];
      background_ai_per_day: number;
      ai_requests_per_hour: number;
      eval_passed_models: string[];
    };
  } | null;
}

export interface SystemUser {
  id: string;
  email: string;
  display_name: string;
  created_at: string;
  last_connected_at: string | null;
  is_system_admin: boolean;
  is_app_admin: boolean;
  is_cairn_admin: boolean;
  deactivated_at: string | null;
  anonymized_at: string | null;
  organization_count: number;
  workspace_count: number;
  active_sessions: number;
  /** Whether the account has confirmed two-step sign-in. */
  two_step?: boolean;
  workspaces: SystemUserWorkspace[];
  organizations: SystemUserOrganization[];
}

export interface SystemUserOrganization {
  organization_id: string;
  name: string;
  role: "owner" | "admin" | "member";
}

export interface SystemUserWorkspace {
  workspace_id: string;
  name: string;
  role: "owner" | "admin" | "member" | "viewer";
  organization_id: string | null;
  organization_name: string | null;
}

export interface SystemSetting {
  key: string;
  group: string;
  label: string;
  description: string;
  kind: "boolean" | "integer" | "choice";
  min: number;
  max: number;
  unit: string;
  /** The allowed values of a choice, with their labels. */
  options: { value: string; label: string }[];
  value: boolean | number | string;
  default_value: boolean | number | string;
  overridden: boolean;
  updated_at: string | null;
  updated_by: string | null;
}

export interface SystemSettings {
  settings: SystemSetting[];
  environment: { label: string; value: string }[];
}

export interface SystemLogRecord {
  sequence: number;
  timestamp: string;
  level: "error" | "warn" | "info" | "debug" | "trace";
  target: string;
  /** security, activity, jobs, http, ai, database or server. */
  category: string;
  message: string;
  fields: string;
}

export interface SystemLogs {
  capturing: boolean;
  /** "memory" for recent events, "file" for a day's file. */
  source: "memory" | "file";
  records: SystemLogRecord[];
}

export interface SystemLogFiles {
  files: { day: string; bytes: number }[];
  writing_files: boolean;
  retention_days: number;
  console_format: "text" | "compact" | "json";
  effective_filter: string | null;
  /** A RUST_LOG filter from the environment is in force instead of the levels. */
  environment_filter: boolean;
  default_level: string;
  categories: { key: string; label: string; level: string }[];
}

export interface SystemJob extends IndexingJobStatus {
  workspace_name: string | null;
}

/** One command of the admin console's catalog. */
export interface ConsoleCommand {
  name: string;
  usage: string;
  summary: string;
  /** Changes something: a dry run unless the line has `--yes`. */
  changes: boolean;
}

/** How to show a console answer's `data`. */
export type ConsoleView =
  | { kind: "table"; columns: string[] }
  /** `data` is `[label, value]` pairs, in order. */
  | { kind: "facts" }
  | { kind: "text" };

/** What a console shows when it opens; the terminal client gets the same. */
export interface ConsoleWelcome {
  banner: string;
  tagline: string;
  tips: string;
  version: string;
  user: string;
  role: string;
  commands: ConsoleCommand[];
}

export interface ConsoleResponse {
  command: string;
  summary: string;
  /** Nothing changed because the line had no `--yes`. */
  dry_run: boolean;
  view: ConsoleView;
  data: unknown;
}

// First-run server setup

export interface ServerSetupStatus {
  /** "new": no account yet; "finishing": the system administrator exists; "complete": set up for good. */
  state: "new" | "finishing" | "complete";
  /** The web app should show the onboarding. */
  onboarding: boolean;
}

export interface SetupCheck {
  key: string;
  label: string;
  status: "ok" | "info" | "warning" | "error";
  detail: string;
  advice: string | null;
}

/** A pending invitation to an organization or workspace (never its secret). */
export interface Invitation {
  id: string;
  email: string;
  target_type: "organization" | "workspace";
  target_id: string;
  role: string;
  invited_by: SharedUser | null;
  created_at: string;
  expires_at: string;
}

export interface CreatedInvitation {
  invitation: Invitation;
  /** The secret, shown once. */
  token: string;
  /** `/invite#<token>`, to put after the web app's origin (the secret stays in the fragment). */
  path: string;
  /** What became of the mail with the link: `queued`, `not_set_up`, `no_public_address`, `rate_limited` (too much mail lately) or `failed`. */
  mail: "queued" | "not_set_up" | "no_public_address" | "rate_limited" | "failed";
}

export interface InvitationPreview {
  email: string;
  target_type: "organization" | "workspace";
  target_id: string;
  target_name: string;
  organization_name: string;
  role: string;
  invited_by: string | null;
  expires_at: string;
  account_exists: boolean;
}

export interface AcceptedInvitation {
  target_type: "organization" | "workspace";
  target_id: string;
  organization_id: string;
  role: string;
}

export interface IssuedPasswordReset {
  token: string;
  /** `/reset-password#<token>`, to put after the web app's origin (the secret stays in the fragment). */
  path: string;
  expires_at: string;
}

export interface PasswordResetPreview {
  email: string | null;
  display_name: string;
  expires_at: string;
}

export interface PersonalAccessToken {
  id: string;
  name: string;
  /** The start of the token, to tell tokens apart. */
  prefix: string;
  /** `system` (administrators only) also allows changes in the System area. */
  scope: "read" | "write" | "admin" | "system" | "mcp";
  created_at: string;
  expires_at: string;
  last_used_at: string | null;
  /** The workspaces the token is limited to; null means every workspace its owner can open. */
  workspaces?: string[] | null;
}

export interface CreatedPersonalToken {
  token: PersonalAccessToken;
  /** The token itself, shown once. */
  secret: string;
}

export interface ArchivePerson {
  name: string;
  email: string | null;
}

export interface ArchiveCounts {
  sources: number;
  folders: number;
  artifacts: number;
  blobs: number;
  memory_cards: number;
  citations: number;
  tasks: number;
  checklist_items: number;
  comments: number;
  lifecycle_events: number;
  omitted_citations: number;
  /** Earlier versions of uploaded files; absent from an archive of format 1. */
  versions?: number;
  /** Saved searches; absent from an archive of format 1. */
  saved_searches?: number;
}

/** Where a background import is (V0.3.20): the stage, how far through the stored files it is, then its report. */
export interface ArchiveImportJob {
  id: string;
  status: "running" | "done" | "failed";
  stage: "checking" | "storing_files" | "writing_records" | "finished";
  progress_current: number;
  progress_total: number;
  report: ArchiveImportReport | null;
  error: string | null;
}

/** What the import shows while it runs: sending the archive, then the server's own stages. */
export interface ArchiveImportProgress {
  stage: "uploading" | ArchiveImportJob["stage"];
  /** Bytes of stored files done, of `total`, in the stage that counts them; 0 and 0 otherwise. */
  current: number;
  total: number;
}

/** The answer to importing a workspace archive, or to checking one first (`dry_run`). */
export interface ArchiveImportReport {
  dry_run: boolean;
  ok: boolean;
  workspace_name: string;
  archive_format_version: number;
  source_version: string;
  exported_at: string;
  counts: ArchiveCounts;
  total_bytes: number;
  people: ArchivePerson[];
  problems: string[];
  warnings: string[];
  widened_citations: number;
  workspace: SharedWorkspace | null;
}
