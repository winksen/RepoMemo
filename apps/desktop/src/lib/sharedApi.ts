import type {
  AgentCapabilities,
  AgentConversation,
  AgentConversationDetail,
  AgentMessage,
  AgentTurnResponse,
  AskAnswer,
  KnowledgeMap,
  HealthAction,
  HealthActionResult,
  WorkspaceHealth,
  ArtifactDetail,
  ArtifactComment,
  ArtifactLifecycle,
  ArtifactLifecycleEvent,
  ArtifactSummary,
  ArtifactType,
  Chunk,
  IndexingJobStatus,
  MemoryCard,
  MemoryCardDetail,
  MemoryCardSummary,
  Organization,
  OrganizationMember,
  OrganizationRole,
  ArtifactIndexFailure,
  DocumentPreview,
  FileLink,
  Folder,
  ProviderTestResult,
  SearchResult,
  SharedSession,
  SharedNotification,
  SharedAiProviderSettings,
  SharedUser,
  UserProfile,
  WorkspaceMember,
  WorkspaceRole,
  WorkspaceActivityEvent,
  WorkspaceActivityCalendar,
  CollaborationTask,
  CollaborationTaskPriority,
  CollaborationTaskStatus,
  SavedSearch,
  TaskChecklistItem,
  WorkspaceAiOverview,
  WorkspaceCapabilities,
  SharedWorkspace,
  WorkspaceOverview,
  WorkspaceMetrics,
} from "../types";

const API_URL = (import.meta.env.VITE_REPOMEMO_API_URL ?? "http://127.0.0.1:3020").replace(/\/$/, "");

export const sharedApiUrl = API_URL;

export function getSharedHealth(): Promise<{ service: string; status: string; authentication: string }> {
  return request<{ service: string; status: string; authentication: string }>("/health");
}

export const SHARED_SESSION_STORAGE_KEY = "repomemo.shared.access-token";
const REFRESH_STORAGE_KEY = "repomemo.shared.refresh-token";

function readStorage(storage: "sessionStorage" | "localStorage", key: string): string | null {
  try {
    return window[storage].getItem(key);
  } catch {
    return null;
  }
}

function writeStorage(storage: "sessionStorage" | "localStorage", key: string, value: string | null) {
  try {
    if (value === null) window[storage].removeItem(key);
    else window[storage].setItem(key, value);
  } catch {
    // storage unavailable; the session simply won't survive a reload
  }
}

function storeTokens(response: { access_token: string; refresh_token: string }) {
  writeStorage("sessionStorage", SHARED_SESSION_STORAGE_KEY, response.access_token);
  writeStorage("localStorage", REFRESH_STORAGE_KEY, response.refresh_token);
}

/** Forgets the local session. Best-effort revokes the refresh token on the server. */
export function clearSharedSession() {
  const refreshToken = readStorage("localStorage", REFRESH_STORAGE_KEY);
  writeStorage("sessionStorage", SHARED_SESSION_STORAGE_KEY, null);
  writeStorage("localStorage", REFRESH_STORAGE_KEY, null);
  if (refreshToken) {
    fetch(`${API_URL}/v1/auth/logout`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ refresh_token: refreshToken }),
    }).catch(() => undefined);
  }
}

// Callers hold on to the access token they were given; once it is refreshed, requests using the old value are upgraded.
const refreshedTokens = new Map<string, string>();
let refreshInFlight: Promise<string | null> | null = null;

function latestToken(token: string): string {
  let current = token;
  for (let hops = 0; refreshedTokens.has(current) && hops < 50; hops += 1) current = refreshedTokens.get(current)!;
  return current;
}

/** Trades the stored refresh token for a new access token. Resolves to null when there is no valid refresh token. */
export function refreshSharedSession(staleAccessToken?: string): Promise<string | null> {
  if (refreshInFlight) return refreshInFlight;
  const refreshToken = readStorage("localStorage", REFRESH_STORAGE_KEY);
  if (!refreshToken) return Promise.resolve(null);
  refreshInFlight = (async () => {
    try {
      const response = await fetch(`${API_URL}/v1/auth/refresh`, {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify({ refresh_token: refreshToken }),
      });
      if (!response.ok) {
        // Only a definitive rejection ends the session; a flaky network keeps it.
        if (response.status === 401) writeStorage("localStorage", REFRESH_STORAGE_KEY, null);
        return null;
      }
      const tokens = await response.json() as TokenResponse;
      storeTokens(tokens);
      if (staleAccessToken) refreshedTokens.set(staleAccessToken, tokens.access_token);
      return tokens.access_token;
    } catch {
      return null;
    } finally {
      refreshInFlight = null;
    }
  })();
  return refreshInFlight;
}

function redirectToSignIn() {
  writeStorage("sessionStorage", SHARED_SESSION_STORAGE_KEY, null);
  if (window.location.pathname !== "/login") {
    window.location.replace("/login");
  }
}

/** fetch for authenticated calls. A 401 first tries a silent refresh and one retry; if that fails the session is over, so go straight to sign-in instead of leaving protected content on screen. */
async function authFetch(input: string, init: RequestInit): Promise<Response> {
  const headers = new Headers(init.headers);
  const bearer = headers.get("Authorization")?.replace(/^Bearer /, "");
  if (!bearer) return fetch(input, init);

  const token = latestToken(bearer);
  headers.set("Authorization", `Bearer ${token}`);
  const response = await fetch(input, { ...init, headers });
  if (response.status !== 401) return response;

  const refreshed = await refreshSharedSession(token);
  if (!refreshed) {
    redirectToSignIn();
    return response;
  }
  headers.set("Authorization", `Bearer ${refreshed}`);
  const retry = await fetch(input, { ...init, headers });
  if (retry.status === 401) redirectToSignIn();
  return retry;
}

export class SharedApiError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = "SharedApiError";
    this.status = status;
  }
}

interface TokenResponse {
  access_token: string;
  refresh_token: string;
  token_type: "Bearer";
  expires_in: number;
  user: SharedUser;
}

async function request<T>(
  path: string,
  options: RequestInit = {},
  accessToken?: string,
): Promise<T> {
  const headers = new Headers(options.headers);
  headers.set("Accept", "application/json");
  if (options.body) {
    headers.set("Content-Type", "application/json");
  }
  if (accessToken) {
    headers.set("Authorization", `Bearer ${accessToken}`);
  }

  const response = await authFetch(`${API_URL}${path}`, { ...options, headers });
  const payload = await response.json().catch(() => null) as { error?: { message?: string } } | T | null;
  if (!response.ok) {
    const message = payload && typeof payload === "object" && "error" in payload
      ? payload.error?.message
      : null;
    throw new SharedApiError(response.status, message ?? `The shared API returned ${response.status}.`);
  }
  return payload as T;
}

async function requestText(path: string, accessToken: string): Promise<string> {
  const response = await authFetch(`${API_URL}${path}`, {
    headers: { Accept: "text/markdown", Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok) {
    const payload = await response.json().catch(() => null) as { error?: { message?: string } } | null;
    throw new SharedApiError(response.status, payload?.error?.message ?? `The shared API returned ${response.status}.`);
  }
  return response.text();
}

export async function registerSharedUser(input: {
  email: string;
  displayName: string;
  password: string;
}): Promise<TokenResponse> {
  const response = await request<TokenResponse>("/v1/auth/register", {
    method: "POST",
    body: JSON.stringify({
      email: input.email,
      display_name: input.displayName,
      password: input.password,
    }),
  });
  storeTokens(response);
  return response;
}

export async function loginSharedUser(input: {
  email: string;
  password: string;
}): Promise<TokenResponse> {
  const response = await request<TokenResponse>("/v1/auth/login", {
    method: "POST",
    body: JSON.stringify(input),
  });
  storeTokens(response);
  return response;
}

export function getSharedSession(accessToken: string): Promise<SharedSession> {
  return request<SharedSession>("/v1/session", {}, accessToken);
}

export function getSharedProfile(accessToken: string): Promise<UserProfile> {
  return request<UserProfile>("/v1/profile", {}, accessToken);
}

export function listSharedProfileTasks(accessToken: string): Promise<CollaborationTask[]> {
  return request<CollaborationTask[]>("/v1/profile/tasks", {}, accessToken);
}

export function listSharedNotifications(accessToken: string): Promise<SharedNotification[]> {
  return request<SharedNotification[]>("/v1/notifications", {}, accessToken);
}

export function markSharedNotificationRead(accessToken: string, notificationId: string): Promise<SharedNotification> {
  return request<SharedNotification>(`/v1/notifications/${notificationId}/read`, { method: "POST" }, accessToken);
}

export function markAllSharedNotificationsRead(accessToken: string): Promise<void> {
  return request<void>("/v1/notifications/read-all", { method: "POST" }, accessToken);
}

export function updateSharedProfile(accessToken: string, displayName: string): Promise<SharedUser> {
  return request<SharedUser>("/v1/profile", {
    method: "PUT",
    body: JSON.stringify({ display_name: displayName }),
  }, accessToken);
}

export function changeSharedPassword(accessToken: string, input: { currentPassword: string; newPassword: string }): Promise<void> {
  return request<void>("/v1/profile/password", {
    method: "POST",
    body: JSON.stringify({ current_password: input.currentPassword, new_password: input.newPassword }),
  }, accessToken);
}

export function listSharedOrganizations(accessToken: string): Promise<Organization[]> {
  return request<Organization[]>("/v1/organizations", {}, accessToken);
}

export function createSharedOrganization(accessToken: string, name: string): Promise<Organization> {
  return request<Organization>("/v1/organizations", {
    method: "POST",
    body: JSON.stringify({ name }),
  }, accessToken);
}

export function updateSharedOrganization(accessToken: string, organizationId: string, name: string): Promise<Organization> {
  return request<Organization>(`/v1/organizations/${organizationId}`, {
    method: "PUT",
    body: JSON.stringify({ name }),
  }, accessToken);
}

export function listSharedOrganizationMembers(accessToken: string, organizationId: string): Promise<OrganizationMember[]> {
  return request<OrganizationMember[]>(`/v1/organizations/${organizationId}/members`, {}, accessToken);
}

export function upsertSharedOrganizationMember(accessToken: string, organizationId: string, input: { email: string; role: OrganizationRole }): Promise<OrganizationMember> {
  return request<OrganizationMember>(`/v1/organizations/${organizationId}/members`, {
    method: "PUT",
    body: JSON.stringify({ email: input.email, role: input.role }),
  }, accessToken);
}

export function removeSharedOrganizationMember(accessToken: string, organizationId: string, userId: string): Promise<void> {
  return request<void>(`/v1/organizations/${organizationId}/members/${userId}`, { method: "DELETE" }, accessToken);
}

export function listSharedWorkspaces(accessToken: string): Promise<SharedWorkspace[]> {
  return request<SharedWorkspace[]>("/v1/workspaces", {}, accessToken);
}

export function createSharedWorkspace(
  accessToken: string,
  organizationId: string,
  name: string,
): Promise<SharedWorkspace> {
  return request<SharedWorkspace>("/v1/workspaces", {
    method: "POST",
    body: JSON.stringify({ organization_id: organizationId, name }),
  }, accessToken);
}

export function updateSharedWorkspace(accessToken: string, workspaceId: string, name: string) {
  return request<SharedWorkspace["workspace"]>(`/v1/workspaces/${workspaceId}`, {
    method: "PUT",
    body: JSON.stringify({ name }),
  }, accessToken);
}

export function deleteSharedWorkspace(accessToken: string, workspaceId: string): Promise<void> {
  return request<void>(`/v1/workspaces/${workspaceId}`, { method: "DELETE" }, accessToken);
}

export function getSharedWorkspaceOverview(accessToken: string, workspaceId: string): Promise<WorkspaceOverview> {
  return request<WorkspaceOverview>(`/v1/workspaces/${workspaceId}/overview`, {}, accessToken);
}

export function getSharedWorkspaceMetrics(accessToken: string, workspaceId: string): Promise<WorkspaceMetrics> {
  return request<WorkspaceMetrics>(`/v1/workspaces/${workspaceId}/metrics`, {}, accessToken);
}

export function getSharedWorkspaceCapabilities(accessToken: string, workspaceId: string): Promise<WorkspaceCapabilities> {
  return request<WorkspaceCapabilities>(`/v1/workspaces/${workspaceId}/capabilities`, {}, accessToken);
}

export function generateSharedWorkspaceAiOverview(accessToken: string, workspaceId: string): Promise<WorkspaceAiOverview> {
  return request<WorkspaceAiOverview>(`/v1/workspaces/${workspaceId}/ai-overview`, { method: "POST" }, accessToken);
}

export function askSharedWorkspace(accessToken: string, workspaceId: string, question: string): Promise<AskAnswer> {
  return request<AskAnswer>(`/v1/workspaces/${workspaceId}/ask`, {
    method: "POST",
    body: JSON.stringify({ question, limit: 8 }),
  }, accessToken);
}

export function getSharedKnowledgeMap(accessToken: string, workspaceId: string): Promise<KnowledgeMap> {
  return request<KnowledgeMap>(`/v1/workspaces/${workspaceId}/knowledge-map`, {}, accessToken);
}

export function getSharedWorkspaceHealth(accessToken: string, workspaceId: string): Promise<WorkspaceHealth> {
  return request<WorkspaceHealth>(`/v1/workspaces/${workspaceId}/health`, {}, accessToken);
}

/** Workspace administrators only. `keepArtifactId` picks the file kept by `supersede`. */
export function applySharedHealthAction(accessToken: string, workspaceId: string, fingerprint: string, action: HealthAction, keepArtifactId?: string): Promise<HealthActionResult> {
  return request<HealthActionResult>(`/v1/workspaces/${workspaceId}/health/actions`, {
    method: "POST",
    body: JSON.stringify({ fingerprint, action, keep_artifact_id: keepArtifactId ?? null }),
  }, accessToken);
}

export function getSharedAgentCapabilities(accessToken: string, workspaceId: string): Promise<AgentCapabilities> {
  return request<AgentCapabilities>(`/v1/workspaces/${workspaceId}/agent/capabilities`, {}, accessToken);
}

/** Sends a message; without `conversationId` the server starts a new conversation. */
export function sendSharedAgentMessage(accessToken: string, workspaceId: string, message: AgentMessage, conversationId?: string): Promise<AgentTurnResponse> {
  return request<AgentTurnResponse>(`/v1/workspaces/${workspaceId}/agent/messages`, {
    method: "POST",
    body: JSON.stringify({ ...message, conversation_id: conversationId }),
  }, accessToken);
}

export function listSharedAgentConversations(accessToken: string, workspaceId: string): Promise<AgentConversation[]> {
  return request<AgentConversation[]>(`/v1/workspaces/${workspaceId}/agent/conversations`, {}, accessToken);
}

export function getSharedAgentConversation(accessToken: string, conversationId: string): Promise<AgentConversationDetail> {
  return request<AgentConversationDetail>(`/v1/agent/conversations/${conversationId}`, {}, accessToken);
}

export function renameSharedAgentConversation(accessToken: string, conversationId: string, title: string): Promise<AgentConversation> {
  return request<AgentConversation>(`/v1/agent/conversations/${conversationId}`, { method: "PUT", body: JSON.stringify({ title }) }, accessToken);
}

export function deleteSharedAgentConversation(accessToken: string, conversationId: string): Promise<void> {
  return request<void>(`/v1/agent/conversations/${conversationId}`, { method: "DELETE" }, accessToken);
}

export function listSharedWorkspaceAiProviders(accessToken: string, workspaceId: string): Promise<SharedAiProviderSettings[]> {
  return request<SharedAiProviderSettings[]>(`/v1/workspaces/${workspaceId}/ai-providers`, {}, accessToken);
}

export function saveSharedWorkspaceAiProvider(accessToken: string, workspaceId: string, input: {
  id?: string;
  providerType: "ollama" | "openrouter";
  name: string;
  baseUrl?: string;
  model: string;
  apiKey?: string;
  enabled: boolean;
  cloudContentAcknowledged: boolean;
  purpose: "text" | "vision" | "embedding";
}): Promise<SharedAiProviderSettings> {
  return request<SharedAiProviderSettings>(`/v1/workspaces/${workspaceId}/ai-providers`, {
    method: "PUT",
    body: JSON.stringify({
      id: input.id ?? null,
      provider_type: input.providerType,
      name: input.name,
      base_url: input.baseUrl || null,
      model: input.model,
      api_key: input.apiKey || null,
      enabled: input.enabled,
      cloud_content_acknowledged: input.cloudContentAcknowledged,
      purpose: input.purpose,
    }),
  }, accessToken);
}

export function testSharedWorkspaceAiProvider(accessToken: string, workspaceId: string, providerId: string): Promise<ProviderTestResult> {
  return request<ProviderTestResult>(`/v1/workspaces/${workspaceId}/ai-providers/${providerId}/test`, {
    method: "POST",
  }, accessToken);
}

export function listSharedWorkspaceActivity(accessToken: string, workspaceId: string): Promise<WorkspaceActivityEvent[]> {
  return request<WorkspaceActivityEvent[]>(`/v1/workspaces/${workspaceId}/activity`, {}, accessToken);
}

export function getSharedWorkspaceActivityCalendar(accessToken: string, workspaceId: string): Promise<WorkspaceActivityCalendar> {
  return request<WorkspaceActivityCalendar>(`/v1/workspaces/${workspaceId}/activity/calendar`, {}, accessToken);
}

export interface CollaborationTaskInput {
  title: string;
  description: string;
  status: CollaborationTaskStatus;
  priority: CollaborationTaskPriority;
  assigneeUserId?: string;
  artifactId?: string;
  dueAt?: string;
}

export function listSharedCollaborationTasks(accessToken: string, workspaceId: string): Promise<CollaborationTask[]> {
  return request<CollaborationTask[]>(`/v1/workspaces/${workspaceId}/tasks`, {}, accessToken);
}

export function createSharedCollaborationTask(accessToken: string, workspaceId: string, input: CollaborationTaskInput): Promise<CollaborationTask> {
  return request<CollaborationTask>(`/v1/workspaces/${workspaceId}/tasks`, {
    method: "POST",
    body: JSON.stringify({
      title: input.title,
      description: input.description,
      status: input.status,
      priority: input.priority,
      assignee_user_id: input.assigneeUserId || null,
      artifact_id: input.artifactId || null,
      due_at: input.dueAt || null,
    }),
  }, accessToken);
}

export function updateSharedCollaborationTask(accessToken: string, taskId: string, input: CollaborationTaskInput): Promise<CollaborationTask> {
  return request<CollaborationTask>(`/v1/tasks/${taskId}`, {
    method: "PUT",
    body: JSON.stringify({
      title: input.title,
      description: input.description,
      status: input.status,
      priority: input.priority,
      assignee_user_id: input.assigneeUserId || null,
      artifact_id: input.artifactId || null,
      due_at: input.dueAt || null,
    }),
  }, accessToken);
}

export function deleteSharedCollaborationTask(accessToken: string, taskId: string): Promise<void> {
  return request<void>(`/v1/tasks/${taskId}`, { method: "DELETE" }, accessToken);
}

export function listSharedSavedSearches(accessToken: string, workspaceId: string): Promise<SavedSearch[]> { return request<SavedSearch[]>(`/v1/workspaces/${workspaceId}/saved-searches`, {}, accessToken); }
export function createSharedSavedSearch(accessToken: string, workspaceId: string, input: { name: string; query: string; artifactTypes: ArtifactType[]; languages: string[]; sourceIds: string[]; resultLimit: number }): Promise<SavedSearch> { return request<SavedSearch>(`/v1/workspaces/${workspaceId}/saved-searches`, { method: "POST", body: JSON.stringify({ name: input.name, query: input.query, artifact_types: input.artifactTypes, languages: input.languages, source_ids: input.sourceIds, result_limit: input.resultLimit }) }, accessToken); }
export function deleteSharedSavedSearch(accessToken: string, searchId: string): Promise<void> { return request<void>(`/v1/saved-searches/${searchId}`, { method: "DELETE" }, accessToken); }
export function listSharedTaskChecklist(accessToken: string, taskId: string): Promise<TaskChecklistItem[]> { return request<TaskChecklistItem[]>(`/v1/tasks/${taskId}/checklist`, {}, accessToken); }
export function createSharedTaskChecklistItem(accessToken: string, taskId: string, body: string): Promise<TaskChecklistItem> { return request<TaskChecklistItem>(`/v1/tasks/${taskId}/checklist`, { method: "POST", body: JSON.stringify({ body }) }, accessToken); }
export function toggleSharedTaskChecklistItem(accessToken: string, itemId: string, completed: boolean): Promise<TaskChecklistItem> { return request<TaskChecklistItem>(`/v1/task-checklist/${itemId}`, { method: "PUT", body: JSON.stringify({ completed }) }, accessToken); }
export function deleteSharedTaskChecklistItem(accessToken: string, itemId: string): Promise<void> { return request<void>(`/v1/task-checklist/${itemId}`, { method: "DELETE" }, accessToken); }

export function listSharedWorkspaceMembers(accessToken: string, workspaceId: string): Promise<WorkspaceMember[]> {
  return request<WorkspaceMember[]>(`/v1/workspaces/${workspaceId}/members`, {}, accessToken);
}

export function upsertSharedWorkspaceMember(
  accessToken: string,
  workspaceId: string,
  input: { email: string; role: WorkspaceRole },
): Promise<WorkspaceMember> {
  return request<WorkspaceMember>(`/v1/workspaces/${workspaceId}/members`, {
    method: "PUT",
    body: JSON.stringify(input),
  }, accessToken);
}

export function removeSharedWorkspaceMember(accessToken: string, workspaceId: string, userId: string): Promise<void> {
  return request<void>(`/v1/workspaces/${workspaceId}/members/${userId}`, { method: "DELETE" }, accessToken);
}

export function listSharedIndexFailures(accessToken: string, workspaceId: string): Promise<ArtifactIndexFailure[]> {
  return request<ArtifactIndexFailure[]>(`/v1/workspaces/${workspaceId}/artifacts/index-failures`, {}, accessToken);
}

export function listSharedArtifacts(accessToken: string, workspaceId: string): Promise<ArtifactSummary[]> {
  return request<ArtifactSummary[]>(`/v1/workspaces/${workspaceId}/artifacts`, {}, accessToken);
}

export function querySharedArtifacts(accessToken: string, workspaceId: string, input: {
  query: string;
  artifactTypes?: ArtifactType[];
  languages?: string[];
  sourceIds?: string[];
  indexed?: boolean;
}): Promise<ArtifactSummary[]> {
  return request<ArtifactSummary[]>(`/v1/workspaces/${workspaceId}/artifacts/query`, {
    method: "POST",
    body: JSON.stringify({
      query: input.query,
      artifact_types: input.artifactTypes ?? [],
      languages: input.languages ?? [],
      source_ids: input.sourceIds ?? [],
      indexed: input.indexed ?? null,
    }),
  }, accessToken);
}

export function getSharedArtifact(accessToken: string, artifactId: string): Promise<ArtifactDetail> {
  return request<ArtifactDetail>(`/v1/artifacts/${artifactId}`, {}, accessToken);
}

export function updateSharedArtifact(accessToken: string, artifactId: string, title: string): Promise<ArtifactSummary> {
  return request<ArtifactSummary>(`/v1/artifacts/${artifactId}`, {
    method: "PUT",
    body: JSON.stringify({ title }),
  }, accessToken);
}

export function deleteSharedArtifact(accessToken: string, artifactId: string): Promise<void> {
  return request<void>(`/v1/artifacts/${artifactId}`, { method: "DELETE" }, accessToken);
}

export function getSharedArtifactLifecycle(accessToken: string, artifactId: string): Promise<ArtifactLifecycle> {
  return request<ArtifactLifecycle>(`/v1/artifacts/${artifactId}/lifecycle`, {}, accessToken);
}

export function updateSharedArtifactLifecycle(accessToken: string, artifactId: string, input: {
  status: ArtifactLifecycle["status"];
  ownerUserId: string | null;
  reviewNote: string;
  supersededByArtifactId: string | null;
}): Promise<ArtifactLifecycle> {
  return request<ArtifactLifecycle>(`/v1/artifacts/${artifactId}/lifecycle`, {
    method: "PUT",
    body: JSON.stringify({
      status: input.status,
      owner_user_id: input.ownerUserId,
      review_note: input.reviewNote,
      superseded_by_artifact_id: input.supersededByArtifactId,
    }),
  }, accessToken);
}

export function listSharedArtifactLifecycleEvents(accessToken: string, artifactId: string): Promise<ArtifactLifecycleEvent[]> {
  return request<ArtifactLifecycleEvent[]>(`/v1/artifacts/${artifactId}/lifecycle/history`, {}, accessToken);
}

export function listSharedArtifactComments(accessToken: string, artifactId: string): Promise<ArtifactComment[]> {
  return request<ArtifactComment[]>(`/v1/artifacts/${artifactId}/comments`, {}, accessToken);
}

export function createSharedArtifactComment(accessToken: string, artifactId: string, body: string): Promise<ArtifactComment> {
  return request<ArtifactComment>(`/v1/artifacts/${artifactId}/comments`, {
    method: "POST",
    body: JSON.stringify({ body }),
  }, accessToken);
}

export function updateSharedArtifactComment(accessToken: string, commentId: string, body: string): Promise<ArtifactComment> {
  return request<ArtifactComment>(`/v1/comments/${commentId}`, {
    method: "PUT",
    body: JSON.stringify({ body }),
  }, accessToken);
}

export function deleteSharedArtifactComment(accessToken: string, commentId: string): Promise<void> {
  return request<void>(`/v1/comments/${commentId}`, { method: "DELETE" }, accessToken);
}

export function getSharedDocumentPreview(accessToken: string, artifactId: string): Promise<DocumentPreview> {
  return request<DocumentPreview>(`/v1/artifacts/${artifactId}/document-preview`, {}, accessToken);
}

export interface RenderStatus {
  state: "ready" | "converting" | "failed" | "disabled" | "unsupported";
  message: string | null;
}

/** Asks for the layout-accurate preview; the server starts the conversion if needed. */
export function getSharedRenderStatus(accessToken: string, artifactId: string): Promise<RenderStatus> {
  return request<RenderStatus>(`/v1/artifacts/${artifactId}/rendered-preview/status`, {}, accessToken);
}

export async function downloadSharedRenderedPreview(accessToken: string, artifactId: string): Promise<Blob> {
  const response = await authFetch(`${API_URL}/v1/artifacts/${artifactId}/rendered-preview`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok) {
    const payload = await response.json().catch(() => null) as { error?: { message?: string } } | null;
    throw new SharedApiError(response.status, payload?.error?.message ?? `The shared API returned ${response.status}.`);
  }
  return response.blob();
}

export function createSharedFileLink(accessToken: string, artifactId: string): Promise<FileLink> {
  return request<FileLink>(`/v1/artifacts/${artifactId}/open-link`, { method: "POST", body: JSON.stringify({}) }, accessToken);
}

/** The stored original file, fetched with the user's session. */
export async function downloadSharedArtifactFile(accessToken: string, artifactId: string): Promise<Blob> {
  const response = await authFetch(`${API_URL}/v1/artifacts/${artifactId}/file`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok) {
    const payload = await response.json().catch(() => null) as { error?: { message?: string } } | null;
    throw new SharedApiError(response.status, payload?.error?.message ?? `The shared API returned ${response.status}.`);
  }
  return response.blob();
}

export function listSharedFolders(accessToken: string, workspaceId: string): Promise<Folder[]> {
  return request<Folder[]>(`/v1/workspaces/${workspaceId}/folders`, {}, accessToken);
}

export function createSharedFolder(accessToken: string, workspaceId: string, input: {
  name: string;
  parentId: string | null;
}): Promise<Folder> {
  return request<Folder>(`/v1/workspaces/${workspaceId}/folders`, {
    method: "POST",
    body: JSON.stringify({ name: input.name, parent_id: input.parentId }),
  }, accessToken);
}

export function renameSharedFolder(accessToken: string, workspaceId: string, folderId: string, name: string): Promise<Folder> {
  return request<Folder>(`/v1/workspaces/${workspaceId}/folders/${folderId}`, {
    method: "PATCH",
    body: JSON.stringify({ name }),
  }, accessToken);
}

export function deleteSharedFolder(accessToken: string, workspaceId: string, folderId: string): Promise<void> {
  return request<void>(`/v1/workspaces/${workspaceId}/folders/${folderId}`, { method: "DELETE" }, accessToken);
}

export function moveSharedArtifact(accessToken: string, artifactId: string, folderId: string | null): Promise<ArtifactSummary> {
  return request<ArtifactSummary>(`/v1/artifacts/${artifactId}/folder`, {
    method: "PUT",
    body: JSON.stringify({ folder_id: folderId }),
  }, accessToken);
}

export function createSharedTextArtifact(accessToken: string, workspaceId: string, input: {
  title: string;
  content: string;
  language?: string;
  folder_id?: string | null;
}): Promise<ArtifactSummary> {
  return request<ArtifactSummary>(`/v1/workspaces/${workspaceId}/artifacts/text`, {
    method: "POST",
    body: JSON.stringify(input),
  }, accessToken);
}

export async function uploadSharedArtifact(accessToken: string, workspaceId: string, file: File, folderId?: string | null): Promise<ArtifactSummary> {
  const response = await authFetch(`${API_URL}/v1/workspaces/${workspaceId}/artifacts/upload`, {
    method: "POST",
    headers: {
      Accept: "application/json",
      Authorization: `Bearer ${accessToken}`,
      "Content-Type": file.type || "application/octet-stream",
      "X-RepoMemo-Filename": file.name,
      ...(folderId ? { "X-RepoMemo-Folder-Id": folderId } : {}),
    },
    body: file,
  });
  const payload = await response.json().catch(() => null) as { error?: { message?: string } } | ArtifactSummary | null;
  if (!response.ok) {
    const message = payload && typeof payload === "object" && "error" in payload
      ? payload.error?.message
      : null;
    throw new SharedApiError(response.status, message ?? `The shared API returned ${response.status}.`);
  }
  return payload as ArtifactSummary;
}

export function indexSharedWorkspace(accessToken: string, workspaceId: string): Promise<IndexingJobStatus> {
  return request<IndexingJobStatus>(`/v1/workspaces/${workspaceId}/index`, { method: "POST" }, accessToken);
}

export function indexSharedArtifact(accessToken: string, artifactId: string): Promise<IndexingJobStatus> {
  return request<IndexingJobStatus>(`/v1/artifacts/${artifactId}/index`, { method: "POST" }, accessToken);
}

/** Administrator-only view of the stored index chunks of one artifact. */
export function listSharedArtifactChunks(accessToken: string, artifactId: string): Promise<Chunk[]> {
  return request<Chunk[]>(`/v1/artifacts/${artifactId}/chunks`, {}, accessToken);
}

export function getSharedRetrievalFacets(accessToken: string, workspaceId: string): Promise<import("../types").RetrievalFacets> {
  return request<import("../types").RetrievalFacets>(`/v1/workspaces/${workspaceId}/retrieval-facets`, {}, accessToken);
}

export function searchSharedWorkspace(accessToken: string, workspaceId: string, input: {
  query: string;
  artifactTypes?: ArtifactType[];
  languages?: string[];
  sourceIds?: string[];
  limit?: number;
}): Promise<SearchResult[]> {
  return request<SearchResult[]>(`/v1/workspaces/${workspaceId}/search`, {
    method: "POST",
    body: JSON.stringify({
      query: input.query,
      artifact_types: input.artifactTypes ?? [],
      languages: input.languages ?? [],
      source_ids: input.sourceIds ?? [],
      limit: input.limit ?? 20,
    }),
  }, accessToken);
}

export function listSharedMemoryCards(accessToken: string, workspaceId: string): Promise<MemoryCardSummary[]> {
  return request<MemoryCardSummary[]>(`/v1/workspaces/${workspaceId}/memory-cards`, {}, accessToken);
}

export function searchSharedMemoryCards(accessToken: string, workspaceId: string, query: string): Promise<MemoryCardSummary[]> {
  return request<MemoryCardSummary[]>(`/v1/workspaces/${workspaceId}/memory-cards/search`, {
    method: "POST",
    body: JSON.stringify({ query }),
  }, accessToken);
}

export function createSharedMemoryCard(accessToken: string, workspaceId: string, input: {
  title: string;
  bodyMarkdown: string;
  source: string;
  citations?: Array<{
    artifact_id: string;
    chunk_id: string | null;
    title: string;
    path: string;
    start_line: number | null;
    end_line: number | null;
    confidence: number | null;
  }>;
}): Promise<MemoryCard> {
  return request<MemoryCard>(`/v1/workspaces/${workspaceId}/memory-cards`, {
    method: "POST",
    body: JSON.stringify({
      title: input.title,
      body_markdown: input.bodyMarkdown,
      source: input.source,
      confidence: null,
      citations: input.citations ?? [],
    }),
  }, accessToken);
}

export function getSharedMemoryCard(accessToken: string, cardId: string): Promise<MemoryCardDetail> {
  return request<MemoryCardDetail>(`/v1/memory-cards/${cardId}`, {}, accessToken);
}

export function updateSharedMemoryCard(accessToken: string, cardId: string, input: {
  title: string;
  bodyMarkdown: string;
  source: string;
  confidence?: number | null;
}): Promise<MemoryCard> {
  return request<MemoryCard>(`/v1/memory-cards/${cardId}`, {
    method: "PUT",
    body: JSON.stringify({
      title: input.title,
      body_markdown: input.bodyMarkdown,
      source: input.source,
      confidence: input.confidence ?? null,
    }),
  }, accessToken);
}

export function deleteSharedMemoryCard(accessToken: string, cardId: string): Promise<void> {
  return request<void>(`/v1/memory-cards/${cardId}`, { method: "DELETE" }, accessToken);
}

export function exportSharedMemoryCard(accessToken: string, cardId: string): Promise<string> {
  return requestText(`/v1/memory-cards/${cardId}/export`, accessToken);
}
