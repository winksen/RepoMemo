// Types of the API's data transfer objects that the Rust domain crate defines are
// generated (src/generated/domain.ts, `npm run types:generate`) and re-exported here.
// The declarations below are the ones the client shapes differently from the wire.
import type {
  AgentCapability,
  AgentCapabilityInfo,
  AgentConversation,
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
  ConnectorAccessCheck,
  CrossSearchResponse,
  ReviewItem,
  ReviewItemKind,
  ReviewQueue,
  ArtifactVersion,
  ArtifactVersionText,
  DiffKind,
  VersionComparison,
  VersionDiffLine,
  ConnectorSource,
  ConnectorSyncReport,
  OrganizationMember,
  OrganizationRole,
  ProviderTestResult,
  RepoAccessCheck,
  RepoCommit,
  RepoFile,
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
  AgentCapability,
  AgentCapabilityInfo,
  AgentConversation,
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
  ConnectorAccessCheck,
  CrossSearchResponse,
  ReviewItem,
  ReviewItemKind,
  ReviewQueue,
  ArtifactVersion,
  ArtifactVersionText,
  DiffKind,
  VersionComparison,
  VersionDiffLine,
  ConnectorSource,
  ConnectorSyncReport,
  OrganizationMember,
  OrganizationRole,
  ProviderTestResult,
  RepoAccessCheck,
  RepoCommit,
  RepoFile,
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
  /** Listed only for workspaces the user was added to directly: not a member of the organization, so `role` is only for display. */
  guest?: boolean;
}

export interface SharedWorkspace {
  workspace: Workspace;
  organization_id: string;
  role: WorkspaceRole;
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

/** The lowest workspace role allowed to use AI features. */
export type AiMinRole = "viewer" | "member" | "admin";

export interface AiPolicy {
  min_role: AiMinRole;
  /** AI requests each person may make per hour on this server; 0 is unlimited. */
  requests_per_hour: number;
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
}

export interface AgentMessage {
  message: string;
  capability?: AgentCapability | null;
  artifact_id?: string | null;
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
  /** Earlier turns of the chat that went with the question. */
  context_turns?: number;
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
}

export interface BackupStatus {
  running: boolean;
  operation: "backup" | "restore" | "copy" | null;
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
    embedding_workspaces_waiting: number;
    repository_syncs_running: number;
    preview_conversions_running: number;
    office_previews: { available: boolean; version: string | null; sandboxed: boolean };
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
}

export interface SystemUsage {
  days: number;
  activity_by_day: CountByLabel[];
  activity_by_action: CountByLabel[];
  top_users: CountByLabel[];
  workspaces: WorkspaceUsage[];
}

export interface SystemUser {
  id: string;
  email: string;
  display_name: string;
  created_at: string;
  last_connected_at: string | null;
  is_system_admin: boolean;
  is_app_admin: boolean;
  deactivated_at: string | null;
  anonymized_at: string | null;
  organization_count: number;
  workspace_count: number;
  active_sessions: number;
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
  /** What became of the mail with the link: `queued`, `not_set_up`, `no_public_address` or `failed`. */
  mail: "queued" | "not_set_up" | "no_public_address" | "failed";
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
  scope: "read" | "write" | "system";
  created_at: string;
  expires_at: string;
  last_used_at: string | null;
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
