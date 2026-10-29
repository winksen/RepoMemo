import type {
  ArchiveImportReport,
  ServerSetupStatus,
  SetupCheck,
  AiMinRole,
  AiPolicy,
  ConsoleResponse,
  ConsoleWelcome,
  SystemAuditEvent,
  SystemJob,
  SystemLogFiles,
  SystemLogs,
  SystemOverview,
  SystemSettings,
  SystemUsage,
  SystemUser,
  BackupsOverview,
  MaintenanceStatus,
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
  ArtifactMentions,
  ConnectorAccessCheck,
  ConnectorSource,
  RepoFileAtCommit,
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
  TemplateDetail,
  TemplateSummary,
  WorkspaceTemplateView,
  WorkspaceOverview,
  WorkspaceMetrics,
  RepoAccessCheck,
  RepoFile,
  RepoSummary,
  RepositoryDetailResponse,
  RepoSettings,
  RepoSource,
  RepositoryList,
  Invitation,
  CreatedInvitation,
  InvitationPreview,
  AcceptedInvitation,
  IssuedPasswordReset,
  PasswordResetPreview,
  PersonalAccessToken,
  CreatedPersonalToken,
  SymbolSearchResult,
} from "../types";

import { queryClient } from "./queryClient";
import { openExternal, sessionStore } from "./desktop";
import { browserTransport, type Transport } from "./transport";

let API_URL = (import.meta.env.VITE_REPOMEMO_API_URL ?? "http://127.0.0.1:3020").replace(/\/$/, "");

/** Identifies this client to the server, which lists it under System › Overview. */
let clientName = "web";
const clientHeader = (): [string, string] => ["X-RepoMemo-Client", clientName];

/** The address of the server this client talks to (a live binding: the desktop app sets it once the person has chosen a server). */
export let sharedApiUrl = API_URL;

// Where requests go out (V0.4.44): the one transport (lib/transport.ts). The browser's own `fetch` unless the desktop app
// gave the shell's HTTP client, which is limited to the chosen server and has no cookies. No other `fetch` is used here.
let httpFetch: Transport = browserTransport;
let desktopSession = false;

/** Points the web client at the API the host that serves it names (V0.4.14); an empty address is the page's own host. */
export function configureWebApi(apiUrl: string) {
  API_URL = apiUrl.replace(/\/$/, "");
  sharedApiUrl = API_URL;
}

/**
 * Points this client at a server chosen in the desktop app (V0.2.49). There is no cookie jar there,
 * so the client does not say it is the web app: the server then returns the refresh token in the
 * response, and it is kept in the operating system's credential store instead of the page.
 */
export function configureDesktopApi(serverUrl: string, transport: Transport) {
  API_URL = serverUrl.replace(/\/$/, "");
  sharedApiUrl = API_URL;
  clientName = "desktop";
  httpFetch = transport;
  desktopSession = true;
}

/** What `/health` answers; the build fields name the exact server (V0.4.3). */
export type SharedHealth = {
  service: string;
  status: string;
  authentication: string;
  setup_required?: boolean;
  version?: string;
  commit?: string;
  built_at?: string;
  image?: string;
};

export function getSharedHealth(): Promise<SharedHealth> {
  return request<SharedHealth>("/health");
}

export const SHARED_SESSION_STORAGE_KEY = "repomemo.shared.access-token";
// Before V0.1.68 the refresh token itself was kept here. It now lives in an
// HttpOnly cookie that scripts cannot read; this key only holds the old token
// until it has been exchanged, and `SESSION_MARKER_KEY` records that this
// browser has a session worth renewing.
const REFRESH_STORAGE_KEY = "repomemo.shared.refresh-token";
const SESSION_MARKER_KEY = "repomemo.shared.session";

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

function storeTokens(response: { access_token: string; refresh_token?: string }) {
  writeStorage("sessionStorage", SHARED_SESSION_STORAGE_KEY, response.access_token);
  writeStorage("localStorage", SESSION_MARKER_KEY, "1");
  writeStorage("localStorage", REFRESH_STORAGE_KEY, null);
  // The desktop app has no cookie: the refresh token goes to the operating system's credential store.
  if (desktopSession && response.refresh_token) void sessionStore.save(response.refresh_token).catch(() => undefined);
}

function forgetSession(keepStoredToken = false) {
  // Nothing of the account may outlive its session in this tab.
  queryClient.clear();
  writeStorage("sessionStorage", SHARED_SESSION_STORAGE_KEY, null);
  writeStorage("localStorage", SESSION_MARKER_KEY, null);
  writeStorage("localStorage", REFRESH_STORAGE_KEY, null);
  if (desktopSession && !keepStoredToken) void sessionStore.clear().catch(() => undefined);
}

/** Ends the provider's own session too, when the provider has an end-session page: this window leaves for it, the desktop app opens the system browser. */
async function endProviderSession() {
  try {
    const { logoutUrl } = await fetchSignInProvider();
    if (!logoutUrl) return;
    if (desktopSession) await openExternal(logoutUrl);
    else window.location.assign(logoutUrl);
  } catch {
    // The local session is already gone; the provider session simply ends at its own deadline.
  }
}

/** Forgets the local session. Best-effort revokes the refresh token on the server, which also clears the cookie (in the desktop app, the credential store entry). */
export function clearSharedSession() {
  const providerSession = readStorage("localStorage", PROVIDER_SESSION_KEY) !== null;
  writeStorage("localStorage", PROVIDER_SESSION_KEY, null);
  const hadSession = readStorage("localStorage", SESSION_MARKER_KEY) !== null || readStorage("localStorage", REFRESH_STORAGE_KEY) !== null;
  const legacyToken = readStorage("localStorage", REFRESH_STORAGE_KEY);
  // The stored token is read before it is forgotten, so the server can be told to end that session.
  const stored = desktopSession ? sessionStore.load().catch(() => null) : Promise.resolve(null);
  forgetSession(true);
  if (hadSession || desktopSession) {
    void stored
      .then((storedToken) => {
        const token = storedToken ?? legacyToken;
        return httpFetch(`${API_URL}/v1/auth/logout`, {
          method: "POST",
          credentials: "include",
          headers: { "Content-Type": "application/json", ...Object.fromEntries([clientHeader()]) },
          body: JSON.stringify(token ? { refresh_token: token } : {}),
        }).catch(() => undefined);
      })
      .finally(() => { if (desktopSession) void sessionStore.clear().catch(() => undefined); })
      .then(() => (providerSession ? endProviderSession() : undefined));
  }
}

// Callers hold on to the access token they were given; once it is refreshed, requests using the old value are upgraded.
const refreshedTokens = new Map<string, string>();
let refreshInFlight: Promise<string | null> | null = null;

/**
 * The tabs of one browser share one refresh cookie, and renewing rotates it: two tabs renewing at the same
 * moment would present the same token. They take this lock in turn, so the second presents the cookie the
 * first one received.
 */
const REFRESH_LOCK = "repomemo-refresh";
/** When the server answers 409, another tab renewed a moment ago; asking again presents the new cookie. */
const CONFLICT_RETRIES = 2;
const CONFLICT_DELAY_MS = 300;

async function whileNoOtherTabRenews<T>(work: () => Promise<T>): Promise<T> {
  const locks = typeof navigator === "undefined" ? undefined : navigator.locks;
  if (!locks) return work();
  return (await locks.request(REFRESH_LOCK, work)) as unknown as T;
}

function latestToken(token: string): string {
  let current = token;
  for (let hops = 0; refreshedTokens.has(current) && hops < 50; hops += 1) current = refreshedTokens.get(current)!;
  return current;
}

/** Trades the session cookie for a new access token. Resolves to null when there is no valid session. */
export function refreshSharedSession(staleAccessToken?: string): Promise<string | null> {
  if (refreshInFlight) return refreshInFlight;
  // A token kept by an earlier version is sent once, so the server can move it into the cookie.
  const legacyToken = desktopSession ? null : readStorage("localStorage", REFRESH_STORAGE_KEY);
  if (!desktopSession && !legacyToken && readStorage("localStorage", SESSION_MARKER_KEY) === null) return Promise.resolve(null);
  refreshInFlight = (async () => {
    try {
      // In the desktop app the credential store is the only place a session is kept between runs.
      const token = desktopSession ? await sessionStore.load().catch(() => null) : legacyToken;
      if (desktopSession && !token) return null;
      const send = () => httpFetch(`${API_URL}/v1/auth/refresh`, {
        method: "POST",
        credentials: "include",
        headers: { "Content-Type": "application/json", Accept: "application/json", ...Object.fromEntries([clientHeader()]) },
        body: JSON.stringify(token ? { refresh_token: token } : {}),
      });
      const response = await whileNoOtherTabRenews(async () => {
        let reply = await send();
        for (let attempt = 0; reply.status === 409 && attempt < CONFLICT_RETRIES; attempt += 1) {
          await new Promise<void>((resolve) => { window.setTimeout(resolve, CONFLICT_DELAY_MS); });
          reply = await send();
        }
        return reply;
      });
      if (!response.ok) {
        // Only a definitive rejection ends the session; a flaky network keeps it.
        if (response.status === 401) forgetSession();
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
  if (!bearer) return httpFetch(input, init);

  const token = latestToken(bearer);
  headers.set("Authorization", `Bearer ${token}`);
  headers.set(...clientHeader());
  const response = await httpFetch(input, { ...init, headers });
  if (response.status !== 401) return response;

  const refreshed = await refreshSharedSession(token);
  if (!refreshed) {
    redirectToSignIn();
    return response;
  }
  headers.set("Authorization", `Bearer ${refreshed}`);
  const retry = await httpFetch(input, { ...init, headers });
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

export interface TokenResponse {
  access_token: string;
  refresh_token: string;
  token_type: "Bearer";
  expires_in: number;
  user: SharedUser;
}

export async function request<T>(
  path: string,
  options: RequestInit = {},
  accessToken?: string,
  onResponse?: (response: Response) => void,
): Promise<T> {
  const headers = new Headers(options.headers);
  headers.set("Accept", "application/json");
  headers.set(...clientHeader());
  if (options.body) {
    headers.set("Content-Type", "application/json");
  }
  if (accessToken) {
    headers.set("Authorization", `Bearer ${accessToken}`);
  }

  // Credentials: the session cookie travels to the API, which may sit on another port of the same site.
  const response = await authFetch(`${API_URL}${path}`, { ...options, headers, credentials: "include" });
  onResponse?.(response);
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
  /** An invitation for this email: allows the account even when registration is closed. */
  invitationToken?: string;
}): Promise<TokenResponse> {
  const response = await request<TokenResponse>("/v1/auth/register", {
    method: "POST",
    body: JSON.stringify({
      email: input.email,
      display_name: input.displayName,
      password: input.password,
      ...(input.invitationToken ? { invitation_token: input.invitationToken } : {}),
    }),
  });
  storeTokens(response);
  return response;
}

/**
 * Creating an account from the sign-up form. A server that sends mail answers
 * the same for every address (no session, a message to show); one that does
 * not answers with a session.
 */
export async function signUpSharedUser(input: {
  email: string;
  displayName: string;
  password: string;
}): Promise<{ session: TokenResponse } | { message: string }> {
  const response = await request<TokenResponse | { message: string }>("/v1/auth/register", {
    method: "POST",
    body: JSON.stringify({ email: input.email, display_name: input.displayName, password: input.password }),
  });
  if ("access_token" in response) {
    storeTokens(response);
    return { session: response };
  }
  return { message: response.message };
}

export async function loginSharedUser(input: {
  email: string;
  password: string;
}): Promise<TokenResponse> {
  // The session cookie, when this browser still has one, shows it is one of
  // the account's own devices, which gets past a lockout others caused.
  const response = await request<TokenResponse>("/v1/auth/login", {
    method: "POST",
    body: JSON.stringify(input),
  });
  storeTokens(response);
  return response;
}

export type ProviderFlow = { authorization_url: string; state: string; proof: string; expires_in: number };
export type ProviderLink = [issuer: string, linkedAt: string];

/** The enabled sign-in provider's display name (null when the server offers password sign-in only) and whether it replaces passwords. */
export async function fetchSignInProvider(): Promise<{ name: string | null; enforced: boolean; logoutUrl: string | null }> {
  const answer = await request<{ name: string | null; enforced?: boolean; logout_url?: string | null }>("/v1/auth/identity");
  return { name: answer.name ?? null, enforced: Boolean(answer.name && answer.enforced), logoutUrl: answer.logout_url ?? null };
}

const PROVIDER_SESSION_KEY = "repomemo-provider-session";

export function startProviderFlow(invitationToken?: string): Promise<ProviderFlow> {
  return request<ProviderFlow>("/v1/auth/identity/start", {
    method: "POST",
    body: JSON.stringify(invitationToken ? { invitation_token: invitationToken } : {}),
  });
}

export function startProviderLink(accessToken: string): Promise<ProviderFlow> {
  return request<ProviderFlow>("/v1/profile/identity", { method: "POST" }, accessToken);
}

export function listProviderLinks(accessToken: string): Promise<{ links: ProviderLink[] }> {
  return request<{ links: ProviderLink[] }>("/v1/profile/identity", {}, accessToken);
}

/** Asks once for the outcome of a provider sign-in: null while the provider page is open or the confirmation code is still to be typed. */
export async function pollProviderFlow(flow: ProviderFlow, keepSession = true, code?: string): Promise<TokenResponse | null> {
  const response = await request<TokenResponse | { status: "pending" | "confirm" }>("/v1/auth/identity/complete", {
    method: "POST",
    body: JSON.stringify({ state: flow.state, proof: flow.proof, ...(code ? { code } : {}) }),
  });
  if (!("access_token" in response)) return null;
  if (keepSession) {
    storeTokens(response);
    writeStorage("localStorage", PROVIDER_SESSION_KEY, "1");
  }
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

/** Changes the password. The server ends every other session and returns a fresh one for this tab, which replaces the current tokens. */
export async function changeSharedPassword(accessToken: string, input: { currentPassword: string; newPassword: string }): Promise<void> {
  const response = await request<TokenResponse | null>("/v1/profile/password", {
    method: "POST",
    body: JSON.stringify({ current_password: input.currentPassword, new_password: input.newPassword }),
  }, accessToken);
  if (response?.access_token) {
    storeTokens(response);
    // Components still hold the old token; route their next requests to the new one.
    refreshedTokens.set(latestToken(accessToken), response.access_token);
  }
}

/** Signs out of every session on every device, this one included. */
export async function signOutEverywhere(accessToken: string): Promise<void> {
  await request<void>("/v1/auth/logout-all", { method: "POST" }, accessToken);
  forgetSession();
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

/** Creates a workspace. With a template id the workspace starts with that template and its folders (`general` and no id start with none). */
export function createSharedWorkspace(
  accessToken: string,
  organizationId: string,
  name: string,
  templateId?: string,
): Promise<SharedWorkspace> {
  return request<SharedWorkspace>("/v1/workspaces", {
    method: "POST",
    body: JSON.stringify({ organization_id: organizationId, name, ...(templateId ? { template_id: templateId } : {}) }),
  }, accessToken);
}

/** The templates this server offers, `general` first. */
export function listSharedTemplates(accessToken: string): Promise<TemplateSummary[]> {
  return request<TemplateSummary[]>("/v1/templates", {}, accessToken);
}

/** The template the workspace uses and where it came from. */
export function getSharedWorkspaceTemplate(accessToken: string, workspaceId: string): Promise<WorkspaceTemplateView> {
  return request<WorkspaceTemplateView>(`/v1/workspaces/${workspaceId}/template`, {}, accessToken);
}

/** A template of the catalogue as a person looks at it before choosing it. */
export function getSharedTemplate(accessToken: string, templateId: string): Promise<TemplateDetail> {
  return request<TemplateDetail>(`/v1/templates/${encodeURIComponent(templateId)}`, {}, accessToken);
}

/**
 * Chooses the workspace's template (`null` for the default). `overrides` are the adjustments saved on it
 * (`landing`, `more`, `suggestions`): left out they stay, an empty object clears them. Administrators and owners only.
 */
export function saveSharedWorkspaceTemplate(accessToken: string, workspaceId: string, templateId: string | null, overrides?: Record<string, unknown>): Promise<WorkspaceTemplateView> {
  return request<WorkspaceTemplateView>(`/v1/workspaces/${workspaceId}/template`, {
    method: "PUT",
    body: JSON.stringify({ template_id: templateId, ...(overrides === undefined ? {} : { overrides }) }),
  }, accessToken);
}

/**
 * The workspace's setup as a template file (V0.2.191): the template in force with its adjustments, under the id and
 * name given, ready for the folder an operator names in REPOMEMO_TEMPLATES_DIR. Nothing of the workspace's content is
 * in it. Administrators and owners only.
 */
export async function downloadSharedWorkspaceTemplateFile(accessToken: string, workspaceId: string, input: { id: string; name: string }): Promise<{ blob: Blob; filename: string }> {
  const query = new URLSearchParams({ id: input.id, name: input.name });
  const response = await authFetch(`${API_URL}/v1/workspaces/${workspaceId}/template/file?${query}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok) {
    const payload = await response.json().catch(() => null) as { error?: { message?: string } } | null;
    throw new SharedApiError(response.status, payload?.error?.message ?? `The shared API returned ${response.status}.`);
  }
  const named = /filename="([^"]+)"/.exec(response.headers.get("Content-Disposition") ?? "");
  return { blob: await response.blob(), filename: named?.[1] ?? `${input.id}.json` };
}

export function updateSharedWorkspace(accessToken: string, workspaceId: string, name: string) {
  return request<SharedWorkspace["workspace"]>(`/v1/workspaces/${workspaceId}`, {
    method: "PUT",
    body: JSON.stringify({ name }),
  }, accessToken);
}

/** Chooses the workspace's icon (`null` for the default). Administrators and owners only. */
export function saveSharedWorkspaceIcon(accessToken: string, workspaceId: string, icon: string | null) {
  return request<{ icon: string | null }>(`/v1/workspaces/${workspaceId}/icon`, {
    method: "PUT",
    body: JSON.stringify({ icon }),
  }, accessToken);
}

export function deleteSharedWorkspace(accessToken: string, workspaceId: string): Promise<void> {
  return request<void>(`/v1/workspaces/${workspaceId}`, { method: "DELETE" }, accessToken);
}

export function getSharedWorkspaceOverview(accessToken: string, workspaceId: string): Promise<WorkspaceOverview> {
  return request<WorkspaceOverview>(`/v1/workspaces/${workspaceId}/overview`, {}, accessToken);
}

/** What names a workspace's template on its card; absent for General. */
export interface DashboardTemplate {
  id: string;
  name: string;
  /** The one figure the template puts on the card (a pulse card and its count), when it names one. */
  headline: { card: string; value: number } | null;
}

/** The dashboard's figures for every workspace the person can open. */
export interface DashboardWorkspaceFigures {
  workspace_id: string;
  artifact_count: number;
  /** Tasks that are open or in progress. */
  active_task_count: number;
  memory_card_count: number;
  template: DashboardTemplate | null;
}

export function getSharedDashboard(accessToken: string): Promise<{ workspaces: DashboardWorkspaceFigures[] }> {
  return request<{ workspaces: DashboardWorkspaceFigures[] }>("/v1/dashboard", {}, accessToken);
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

/** What waits for a person in the workspace: files to review, stale memory cards and open health findings. */
export function getSharedReviewQueue(accessToken: string, workspaceId: string): Promise<import("../types").ReviewQueue> {
  return request<import("../types").ReviewQueue>(`/v1/workspaces/${workspaceId}/review`, {}, accessToken);
}

/** Hides an item of the review queue from the person's list until a time, when it comes back (V0.3.10). */
export async function snoozeSharedReviewItem(accessToken: string, workspaceId: string, item: { kind: import("../types").ReviewItemKind; id: string }, until: string): Promise<void> {
  await request<void>(`/v1/workspaces/${workspaceId}/review/snoozes`, { method: "PUT", body: JSON.stringify({ kind: item.kind, id: item.id, until }) }, accessToken);
}

/** Brings a snoozed item back to the person's list now. */
export async function bringBackSharedReviewItem(accessToken: string, workspaceId: string, item: { kind: import("../types").ReviewItemKind; id: string }): Promise<void> {
  await request<void>(`/v1/workspaces/${workspaceId}/review/snoozes?kind=${encodeURIComponent(item.kind)}&id=${encodeURIComponent(item.id)}`, { method: "DELETE" }, accessToken);
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

export function getSharedAiPolicy(accessToken: string, workspaceId: string): Promise<AiPolicy> {
  return request<AiPolicy>(`/v1/workspaces/${workspaceId}/ai-policy`, {}, accessToken);
}

export function saveSharedAiPolicy(accessToken: string, workspaceId: string, minRole: AiMinRole): Promise<AiPolicy> {
  return request<AiPolicy>(`/v1/workspaces/${workspaceId}/ai-policy`, {
    method: "PUT",
    body: JSON.stringify({ min_role: minRole }),
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

export function setSharedSavedSearchSharing(accessToken: string, searchId: string, shared: boolean): Promise<SavedSearch> {
  return request<SavedSearch>(`/v1/saved-searches/${searchId}/sharing`, { method: "PUT", body: JSON.stringify({ shared }) }, accessToken);
}
export function deleteSharedSavedSearch(accessToken: string, searchId: string): Promise<void> { return request<void>(`/v1/saved-searches/${searchId}`, { method: "DELETE" }, accessToken); }
export function listSharedTaskChecklist(accessToken: string, taskId: string): Promise<TaskChecklistItem[]> { return request<TaskChecklistItem[]>(`/v1/tasks/${taskId}/checklist`, {}, accessToken); }
export function createSharedTaskChecklistItem(accessToken: string, taskId: string, body: string): Promise<TaskChecklistItem> { return request<TaskChecklistItem>(`/v1/tasks/${taskId}/checklist`, { method: "POST", body: JSON.stringify({ body }) }, accessToken); }
export function toggleSharedTaskChecklistItem(accessToken: string, itemId: string, completed: boolean): Promise<TaskChecklistItem> { return request<TaskChecklistItem>(`/v1/task-checklist/${itemId}`, { method: "PUT", body: JSON.stringify({ completed }) }, accessToken); }
export function deleteSharedTaskChecklistItem(accessToken: string, itemId: string): Promise<void> { return request<void>(`/v1/task-checklist/${itemId}`, { method: "DELETE" }, accessToken); }

/** Members of the workspace a comment can mention that match what was typed after the `@` (people who can comment only). */
export function listSharedMentionSuggestions(accessToken: string, workspaceId: string, typed: string): Promise<SharedUser[]> {
  return request<SharedUser[]>(`/v1/workspaces/${workspaceId}/mention-suggestions?q=${encodeURIComponent(typed)}`, {}, accessToken);
}

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

/** How many of a workspace's newest files a page holds; the others are reached by searching. The server pages at 500 at most. */
export const ARTIFACT_WINDOW = 500;

/** The newest files of a workspace: one page of the paged list, with the number of files in all. */
export function listSharedArtifactWindow(accessToken: string, workspaceId: string): Promise<{ items: ArtifactSummary[]; total: number }> {
  return listSharedArtifactsPage(accessToken, workspaceId, { limit: ARTIFACT_WINDOW, offset: 0 });
}

/** One server-side page of the artifact list, with the total across all pages. */
export async function listSharedArtifactsPage(accessToken: string, workspaceId: string, page: { limit: number; offset: number }): Promise<{ items: ArtifactSummary[]; total: number }> {
  const response = await authFetch(`${API_URL}/v1/workspaces/${workspaceId}/artifacts?limit=${page.limit}&offset=${page.offset}`, {
    headers: { Accept: "application/json", Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok) {
    const payload = await response.json().catch(() => null) as { error?: { message?: string } } | null;
    throw new SharedApiError(response.status, payload?.error?.message ?? `The shared API returned ${response.status}.`);
  }
  const items = await response.json() as ArtifactSummary[];
  const total = Number(response.headers.get("X-Total-Count"));
  return { items, total: Number.isFinite(total) && total > 0 ? total : items.length };
}

/** One kind of file: all of these hold together; several kinds are alternatives. */
export interface ArtifactKindFilter {
  artifact_types?: ArtifactType[];
  not_artifact_types?: ArtifactType[];
  /** The path ends in one of these (letters and digits, no dot). */
  extensions?: string[];
  /** The path ends in none of these. */
  not_extensions?: string[];
}

/**
 * One page of a view of the evidence section, cut by the server: files that match a name, kinds of file, and
 * a folder (`null` for the top level, `undefined` for anywhere), with the number that match in all.
 */
export async function querySharedArtifactPage(accessToken: string, workspaceId: string, input: {
  query: string;
  kinds?: ArtifactKindFilter[];
  folderId?: string | null;
  ledgerOnly?: boolean;
  limit: number;
  offset: number;
  cursor?: string | null;
}): Promise<{ items: ArtifactSummary[]; total: number; nextCursor: string | null }> {
  const params = new URLSearchParams({ limit: String(input.limit), offset: String(input.offset) });
  if (input.cursor) params.set("cursor", input.cursor);
  const response = await authFetch(`${API_URL}/v1/workspaces/${workspaceId}/artifacts/query?${params}`, {
    method: "POST",
    headers: { Accept: "application/json", "Content-Type": "application/json", Authorization: `Bearer ${accessToken}` },
    body: JSON.stringify({
      query: input.query,
      kinds: input.kinds ?? [],
      folder_id: typeof input.folderId === "string" ? input.folderId : null,
      top_level: input.folderId === null,
      ledger_only: input.ledgerOnly ?? false,
    }),
  });
  if (!response.ok) {
    const payload = await response.json().catch(() => null) as { error?: { message?: string } } | null;
    throw new SharedApiError(response.status, payload?.error?.message ?? `The shared API returned ${response.status}.`);
  }
  const items = await response.json() as ArtifactSummary[];
  const header = response.headers.get("X-Total-Count");
  return { items, total: header === null || !Number.isFinite(Number(header)) ? items.length : Number(header), nextCursor: response.headers.get("X-Next-Cursor") };
}

export function querySharedArtifacts(accessToken: string, workspaceId: string, input: {
  query: string;
  artifactTypes?: ArtifactType[];
  languages?: string[];
  sourceIds?: string[];
  indexed?: boolean;
  /** At most this many matches (the server allows 500); the newest first. */
  limit?: number;
}): Promise<ArtifactSummary[]> {
  const paging = input.limit ? `?limit=${Math.min(Math.max(Math.floor(input.limit), 1), ARTIFACT_WINDOW)}` : "";
  return request<ArtifactSummary[]>(`/v1/workspaces/${workspaceId}/artifacts/query${paging}`, {
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

/** The owners `CODEOWNERS` names for a repository file, each with the workspace member it stands for (V0.3.15). */
export function getSharedArtifactCodeOwners(accessToken: string, artifactId: string): Promise<import("../types").CodeOwnerMatch[]> {
  return request<import("../types").CodeOwnerMatch[]>(`/v1/artifacts/${artifactId}/code-owners`, {}, accessToken);
}

/** Asks a member that `CODEOWNERS` names for a file to review it: a task is assigned to them (V0.3.15). */
export function requestSharedArtifactReview(accessToken: string, artifactId: string, userId: string, note = ""): Promise<import("../types").CollaborationTask> {
  return request<import("../types").CollaborationTask>(`/v1/artifacts/${artifactId}/review-requests`, { method: "POST", body: JSON.stringify({ user_id: userId, note }) }, accessToken);
}

/** A repository file's text as it was at a commit (what a citation names). */
export function getSharedArtifactAtCommit(accessToken: string, artifactId: string, commit: string): Promise<RepoFileAtCommit> {
  return request<RepoFileAtCommit>(`/v1/artifacts/${artifactId}/at/${encodeURIComponent(commit)}`, {}, accessToken);
}

/** The items that name an artifact, and what it names (roadmap F17d). */
export function getSharedArtifactMentions(accessToken: string, artifactId: string): Promise<ArtifactMentions> {
  return request<ArtifactMentions>(`/v1/artifacts/${artifactId}/mentions`, {}, accessToken);
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

/** The workspace as one archive file (owners and administrators). The name comes from the server's `Content-Disposition`. */
export async function downloadSharedWorkspaceArchive(accessToken: string, workspaceId: string): Promise<{ blob: Blob; filename: string }> {
  const response = await authFetch(`${API_URL}/v1/workspaces/${workspaceId}/export`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok) {
    const payload = await response.json().catch(() => null) as { error?: { message?: string } } | null;
    throw new SharedApiError(response.status, payload?.error?.message ?? `The shared API returned ${response.status}.`);
  }
  const named = /filename="([^"]+)"/.exec(response.headers.get("Content-Disposition") ?? "");
  return { blob: await response.blob(), filename: named?.[1] ?? "repomemo-workspace.tar.gz" };
}

/** Reads a workspace archive into a new workspace of the organization, or only checks it (`dryRun`). */
/** How often a background import is asked how far it has got. */
export const IMPORT_POLL_MS = 700;

/**
 * Sends a workspace archive and follows the import to its end (V0.3.20). The archive goes as the request body, which
 * the browser reads from the file as it sends and does not keep in memory; the server answers when it has it, and the
 * checking and the writing run as a job that is asked about until it is done. `onProgress` hears the stages and, while
 * the files are stored, how many bytes are done. A server that answers the request with the result itself (an older
 * one) is understood too.
 */
export async function importSharedWorkspaceArchive(accessToken: string, input: {
  organizationId: string;
  archive: Blob;
  dryRun: boolean;
  name?: string;
  onProgress?: (progress: import("../types").ArchiveImportProgress) => void;
}): Promise<ArchiveImportReport> {
  const query = new URLSearchParams({ organization_id: input.organizationId, background: "true" });
  if (input.dryRun) query.set("dry_run", "true");
  if (input.name?.trim()) query.set("name", input.name.trim());
  input.onProgress?.({ stage: "uploading", current: 0, total: 0 });
  const response = await authFetch(`${API_URL}/v1/workspaces/import?${query}`, {
    method: "POST",
    headers: { Authorization: `Bearer ${accessToken}`, "Content-Type": "application/gzip", Accept: "application/json" },
    body: input.archive,
    credentials: "include",
  });
  const payload = await response.json().catch(() => null) as { error?: { message?: string } } | { job_id?: string } | ArchiveImportReport | null;
  if (!response.ok) {
    const message = payload && "error" in payload ? payload.error?.message : null;
    throw new SharedApiError(response.status, message ?? `The shared API returned ${response.status}.`);
  }
  if (response.status !== 202 || !payload || !("job_id" in payload) || !payload.job_id) return payload as ArchiveImportReport;

  const jobPath = `/v1/workspaces/import/jobs/${payload.job_id}`;
  for (;;) {
    let job: import("../types").ArchiveImportJob;
    try {
      job = await request<import("../types").ArchiveImportJob>(jobPath, {}, accessToken);
    } catch (error) {
      if (error instanceof SharedApiError && error.status === 404) {
        throw new SharedApiError(404, "The import stopped before it finished, perhaps because the server restarted. Nothing was created; try again.");
      }
      throw error;
    }
    input.onProgress?.({ stage: job.stage, current: job.progress_current, total: job.progress_total });
    if (job.status === "failed") throw new SharedApiError(400, job.error ?? "The archive could not be imported.");
    if (job.status === "done" && job.report) return job.report;
    await new Promise((resolve) => window.setTimeout(resolve, IMPORT_POLL_MS));
  }
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
      // Percent-encoded so names outside ASCII survive the header.
      "X-RepoMemo-Filename": encodeURIComponent(file.name),
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

/** Makes an earlier version of a file its current content again, as a new version (V0.3.8). */
export function restoreSharedArtifactVersion(accessToken: string, artifactId: string, version: number): Promise<ArtifactSummary> {
  return request<ArtifactSummary>(`/v1/artifacts/${artifactId}/versions/${version}/restore`, { method: "POST", body: "{}" }, accessToken);
}

/** A new version of a file, into the same artifact (comments, folder and memory links stay). */
/** Every version of an uploaded file, the current one first. */
export function listSharedArtifactVersions(accessToken: string, artifactId: string): Promise<import("../types").ArtifactVersion[]> {
  return request<import("../types").ArtifactVersion[]>(`/v1/artifacts/${artifactId}/versions`, {}, accessToken);
}

/** The text of one version, current or earlier. */
export function getSharedArtifactVersionText(accessToken: string, artifactId: string, version: number): Promise<import("../types").ArtifactVersionText> {
  return request<import("../types").ArtifactVersionText>(`/v1/artifacts/${artifactId}/versions/${version}`, {}, accessToken);
}

/** Two versions of a file compared line by line. */
export function compareSharedArtifactVersions(accessToken: string, artifactId: string, from: number, to: number): Promise<import("../types").VersionComparison> {
  return request<import("../types").VersionComparison>(`/v1/artifacts/${artifactId}/compare?from=${from}&to=${to}`, {}, accessToken);
}

export async function uploadSharedArtifactVersion(accessToken: string, artifactId: string, file: File): Promise<ArtifactSummary> {
  const response = await authFetch(`${API_URL}/v1/artifacts/${artifactId}/versions`, {
    method: "POST",
    headers: {
      Accept: "application/json",
      Authorization: `Bearer ${accessToken}`,
      "Content-Type": file.type || "application/octet-stream",
      "X-RepoMemo-Filename": encodeURIComponent(file.name),
    },
    body: file,
  });
  const payload = await response.json().catch(() => null) as { error?: { message?: string } } | ArtifactSummary | null;
  if (!response.ok) {
    const message = payload && typeof payload === "object" && "error" in payload ? payload.error?.message : null;
    throw new SharedApiError(response.status, message ?? `The shared API returned ${response.status}.`);
  }
  return payload as ArtifactSummary;
}

export function moveSharedFolder(accessToken: string, workspaceId: string, folderId: string, parentId: string | null): Promise<Folder> {
  return request<Folder>(`/v1/workspaces/${workspaceId}/folders/${folderId}/move`, {
    method: "POST",
    body: JSON.stringify({ parent_id: parentId }),
  }, accessToken);
}

export interface BulkArtifactsResult {
  done: string[];
  failed: Array<{ artifact_id: string; message: string }>;
}

/** One action over many files; each is handled on its own and the answer lists what was left alone. */
export function bulkSharedArtifacts(accessToken: string, workspaceId: string, input: {
  action: "move" | "delete" | "lifecycle";
  artifactIds: string[];
  folderId?: string | null;
  status?: "active" | "needs_review" | "verified" | "outdated";
}): Promise<BulkArtifactsResult> {
  return request<BulkArtifactsResult>(`/v1/workspaces/${workspaceId}/artifacts/bulk`, {
    method: "POST",
    body: JSON.stringify({
      action: input.action,
      artifact_ids: input.artifactIds,
      folder_id: input.folderId ?? null,
      status: input.status ?? null,
    }),
  }, accessToken);
}

/** Hands a workspace to another member (owners only). */
export function transferSharedWorkspaceOwnership(accessToken: string, workspaceId: string, userId: string): Promise<void> {
  return request<void>(`/v1/workspaces/${workspaceId}/transfer-ownership`, {
    method: "POST",
    body: JSON.stringify({ user_id: userId }),
  }, accessToken);
}

/** Hands an organization to another member (owners only). */
export function transferSharedOrganizationOwnership(accessToken: string, organizationId: string, userId: string): Promise<void> {
  return request<void>(`/v1/organizations/${organizationId}/transfer-ownership`, {
    method: "POST",
    body: JSON.stringify({ user_id: userId }),
  }, accessToken);
}

/** Starts re-indexing a workspace (administrators). The server answers `202` with the running job: follow it with `getSharedJob`, or on the event stream. */
export function indexSharedWorkspace(accessToken: string, workspaceId: string): Promise<IndexingJobStatus> {
  return request<IndexingJobStatus>(`/v1/workspaces/${workspaceId}/index`, { method: "POST" }, accessToken);
}

/** Starts re-indexing one file. Answers `202` with the running job, as `indexSharedWorkspace` does. */
export function indexSharedArtifact(accessToken: string, artifactId: string): Promise<IndexingJobStatus> {
  return request<IndexingJobStatus>(`/v1/artifacts/${artifactId}/index`, { method: "POST" }, accessToken);
}

export function getSharedJob(accessToken: string, jobId: string): Promise<IndexingJobStatus> {
  return request<IndexingJobStatus>(`/v1/jobs/${jobId}`, {}, accessToken);
}

const JOB_POLL_MS = 800;

/** Waits for a job the server accepted to leave `running`, polling its row. Resolves with the finished job. */
export async function waitForSharedJob(
  accessToken: string,
  job: IndexingJobStatus,
  isCancelled: () => boolean = () => false,
): Promise<IndexingJobStatus> {
  let current = job;
  while ((current.status === "running" || current.status === "pending") && !isCancelled()) {
    await new Promise((resolve) => setTimeout(resolve, JOB_POLL_MS));
    current = await getSharedJob(accessToken, current.id);
  }
  return current;
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
  /** Evidence states to search; empty hides outdated and superseded evidence. */
  lifecycleStatuses?: string[];
  folderId?: string | null;
  /** `YYYY-MM-DD` */
  modifiedAfter?: string | null;
  modifiedBefore?: string | null;
  signal?: AbortSignal;
}): Promise<{ results: SearchResult[]; semanticAvailable: boolean | null; timedOut: boolean }> {
  let semanticAvailable: boolean | null = null;
  let timedOut = false;
  return request<SearchResult[]>(`/v1/workspaces/${workspaceId}/search`, {
    method: "POST",
    signal: input.signal,
    body: JSON.stringify({
      query: input.query,
      artifact_types: input.artifactTypes ?? [],
      languages: input.languages ?? [],
      source_ids: input.sourceIds ?? [],
      limit: input.limit ?? 20,
      lifecycle_statuses: input.lifecycleStatuses ?? [],
      folder_id: input.folderId || null,
      modified_after: input.modifiedAfter || null,
      modified_before: input.modifiedBefore || null,
    }),
  }, accessToken, (response) => {
    const state = response.headers.get("x-repomemo-semantic-search");
    semanticAvailable = state === "available" ? true : state === "unavailable" ? false : null;
    timedOut = response.headers.get("x-repomemo-query-state") === "timed_out";
  }).then((results) => ({ results, semanticAvailable, timedOut }));
}

/** Searches every workspace the account can read; results are merged by score and name their workspace. */
export function searchAllWorkspaces(accessToken: string, input: {
  query: string;
  workspaceIds?: string[];
  limit?: number;
  /** Only these kinds of file (V0.3.12). */
  artifactTypes?: string[];
  /** Only files in these languages or formats. */
  languages?: string[];
  /** Only evidence updated on or after this day (YYYY-MM-DD) … */
  modifiedAfter?: string;
  /** … and on or before this one. */
  modifiedBefore?: string;
  /** Also look in memory cards; they are left out when a type or a language is given. */
  memoryCards?: boolean;
}): Promise<import("../types").CrossSearchResponse> {
  return request<import("../types").CrossSearchResponse>("/v1/search", {
    method: "POST",
    body: JSON.stringify({
      query: input.query,
      workspace_ids: input.workspaceIds ?? [],
      limit: input.limit ?? 20,
      artifact_types: input.artifactTypes ?? [],
      languages: input.languages ?? [],
      modified_after: input.modifiedAfter || null,
      modified_before: input.modifiedBefore || null,
      memory_cards: input.memoryCards ?? true,
    }),
  }, accessToken);
}

/** Where code symbols whose name contains `query` are defined. */
export function searchSharedSymbols(accessToken: string, workspaceId: string, query: string): Promise<SymbolSearchResult[]> {
  return request<SymbolSearchResult[]>(`/v1/workspaces/${workspaceId}/symbols?q=${encodeURIComponent(query)}`, {}, accessToken);
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
  /** Replaces the cited evidence; leave out to keep it. */
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
  return request<MemoryCard>(`/v1/memory-cards/${cardId}`, {
    method: "PUT",
    body: JSON.stringify({
      title: input.title,
      body_markdown: input.bodyMarkdown,
      source: input.source,
      confidence: input.confidence ?? null,
      ...(input.citations ? { citations: input.citations } : {}),
    }),
  }, accessToken);
}

export function deleteSharedMemoryCard(accessToken: string, cardId: string): Promise<void> {
  return request<void>(`/v1/memory-cards/${cardId}`, { method: "DELETE" }, accessToken);
}

export function exportSharedMemoryCard(accessToken: string, cardId: string): Promise<string> {
  return requestText(`/v1/memory-cards/${cardId}/export`, accessToken);
}

export function listSharedConnectors(accessToken: string, workspaceId: string): Promise<{ connectors: ConnectorSource[] }> {
  return request(`/v1/workspaces/${workspaceId}/connectors`, {}, accessToken);
}

export function checkSharedConnector(accessToken: string, workspaceId: string, input: { link: string; provider?: string; token?: string }): Promise<ConnectorAccessCheck> {
  return request<ConnectorAccessCheck>(`/v1/workspaces/${workspaceId}/connectors/check`, { method: "POST", body: JSON.stringify(input) }, accessToken);
}

export function connectSharedConnector(accessToken: string, workspaceId: string, input: { link: string; provider?: string; name?: string; token?: string }): Promise<{ connector: ConnectorSource; job: IndexingJobStatus | null }> {
  return request(`/v1/workspaces/${workspaceId}/connectors`, { method: "POST", body: JSON.stringify(input) }, accessToken);
}

export function syncSharedConnector(accessToken: string, connectorId: string): Promise<{ connector: ConnectorSource; job: IndexingJobStatus | null }> {
  return request(`/v1/connectors/${connectorId}/sync`, { method: "POST", body: JSON.stringify({}) }, accessToken);
}

export function setSharedConnectorToken(accessToken: string, connectorId: string, token: string): Promise<ConnectorSource> {
  return request<ConnectorSource>(`/v1/connectors/${connectorId}/token`, { method: "PUT", body: JSON.stringify({ token }) }, accessToken);
}

/** The secret is returned once, when the webhook is made or renewed. */
export function createSharedConnectorWebhook(accessToken: string, connectorId: string): Promise<{ secret: string; path: string; url: string | null }> {
  return request(`/v1/connectors/${connectorId}/webhook`, { method: "POST", body: JSON.stringify({}) }, accessToken);
}

/** The connectors and the repositories whose webhooks keep a list of the calls they received (V0.3.22). */
export type WebhookSourceKind = "connectors" | "repositories";

export function listSharedWebhookDeliveries(accessToken: string, kind: WebhookSourceKind, sourceId: string): Promise<import("../types").WebhookDelivery[]> {
  return request(`/v1/${kind}/${sourceId}/webhook/deliveries`, {}, accessToken);
}

/** Asks for the sync a call asked for once more; the answer says whether it was queued and keeps the new call. */
export function resendSharedWebhookDelivery(accessToken: string, kind: WebhookSourceKind, sourceId: string, deliveryId: string): Promise<{ queued: boolean; note: string; delivery: import("../types").WebhookDelivery }> {
  return request(`/v1/${kind}/${sourceId}/webhook/deliveries/${deliveryId}/resend`, { method: "POST", body: JSON.stringify({}) }, accessToken);
}

export function deleteSharedConnectorWebhook(accessToken: string, connectorId: string): Promise<void> {
  return request<void>(`/v1/connectors/${connectorId}/webhook`, { method: "DELETE" }, accessToken);
}

export function deleteSharedConnector(accessToken: string, connectorId: string): Promise<void> {
  return request<void>(`/v1/connectors/${connectorId}`, { method: "DELETE" }, accessToken);
}

export function listSharedRepositories(accessToken: string, workspaceId: string): Promise<RepositoryList> {
  return request<RepositoryList>(`/v1/workspaces/${workspaceId}/repositories`, {}, accessToken);
}

/** A token for a private remote repository, sent once with the link and never returned. */
export type RepositoryToken = { token?: string; token_username?: string; deploy_key?: string; host_key?: string };

export function checkSharedRepository(accessToken: string, workspaceId: string, input: { link: string } & RepositoryToken & RepoSettings): Promise<RepoAccessCheck> {
  return request<RepoAccessCheck>(`/v1/workspaces/${workspaceId}/repositories/check`, { method: "POST", body: JSON.stringify(input) }, accessToken);
}

export function connectSharedRepository(accessToken: string, workspaceId: string, input: { link: string; name?: string } & RepositoryToken & RepoSettings): Promise<{ repository: RepoSource; job: IndexingJobStatus | null }> {
  return request(`/v1/workspaces/${workspaceId}/repositories`, { method: "POST", body: JSON.stringify(input) }, accessToken);
}

export function updateSharedRepository(accessToken: string, repositoryId: string, input: { name?: string } & RepoSettings): Promise<RepoSource> {
  return request<RepoSource>(`/v1/repositories/${repositoryId}`, { method: "PUT", body: JSON.stringify(input) }, accessToken);
}

export function setSharedRepositoryToken(accessToken: string, repositoryId: string, input: { token: string; username?: string }): Promise<RepoSource> {
  return request<RepoSource>(`/v1/repositories/${repositoryId}/token`, { method: "PUT", body: JSON.stringify(input) }, accessToken);
}

export function setSharedRepositoryDeployKey(accessToken: string, repositoryId: string, input: { deploy_key: string; host_key?: string }): Promise<RepoSource> {
  return request<RepoSource>(`/v1/repositories/${repositoryId}/deploy-key`, { method: "PUT", body: JSON.stringify(input) }, accessToken);
}

/** The secret is returned once, when the webhook is made or renewed. */
export function createSharedRepositoryWebhook(accessToken: string, repositoryId: string): Promise<{ secret: string; path: string; url: string | null }> {
  return request(`/v1/repositories/${repositoryId}/webhook`, { method: "POST", body: JSON.stringify({}) }, accessToken);
}

export function deleteSharedRepositoryWebhook(accessToken: string, repositoryId: string): Promise<void> {
  return request<void>(`/v1/repositories/${repositoryId}/webhook`, { method: "DELETE" }, accessToken);
}

export function clearSharedRepositoryToken(accessToken: string, repositoryId: string): Promise<void> {
  return request<void>(`/v1/repositories/${repositoryId}/token`, { method: "DELETE" }, accessToken);
}

export function deleteSharedRepository(accessToken: string, repositoryId: string): Promise<void> {
  return request<void>(`/v1/repositories/${repositoryId}`, { method: "DELETE" }, accessToken);
}

export function syncSharedRepository(accessToken: string, repositoryId: string): Promise<{ repository: RepoSource; job: IndexingJobStatus | null }> {
  return request(`/v1/repositories/${repositoryId}/sync`, { method: "POST" }, accessToken);
}

export function getSharedRepositoryDetail(accessToken: string, repositoryId: string): Promise<RepositoryDetailResponse> {
  return request<RepositoryDetailResponse>(`/v1/repositories/${repositoryId}/detail`, {}, accessToken);
}

export function summarizeSharedRepository(accessToken: string, repositoryId: string): Promise<RepoSummary> {
  return request<RepoSummary>(`/v1/repositories/${repositoryId}/summary`, { method: "POST" }, accessToken);
}

export function listSharedRepositoryFiles(accessToken: string, repositoryId: string): Promise<RepoFile[]> {
  return request<RepoFile[]>(`/v1/repositories/${repositoryId}/files`, {}, accessToken);
}

export function cancelSharedJob(accessToken: string, jobId: string): Promise<IndexingJobStatus> {
  return request<IndexingJobStatus>(`/v1/jobs/${jobId}/cancel`, { method: "POST" }, accessToken);
}

export function listSharedWorkspaceJobs(accessToken: string, workspaceId: string, status?: string): Promise<IndexingJobStatus[]> {
  const query = status ? `?status=${encodeURIComponent(status)}` : "";
  return request<IndexingJobStatus[]>(`/v1/workspaces/${workspaceId}/jobs${query}`, {}, accessToken);
}

/** Opens a server-sent-events stream such as `/v1/workspaces/{id}/events`.
 *  Uses fetch rather than `EventSource`, which cannot send the auth header;
 *  an expired token is refreshed like any other request. */
export function openSharedEventStream(path: string, accessToken: string, signal: AbortSignal): Promise<Response> {
  return authFetch(`${API_URL}${path}`, {
    headers: { Accept: "text/event-stream", Authorization: `Bearer ${accessToken}` },
    cache: "no-store",
    signal,
  });
}

// System administration (system administrators only)

export function getSystemOverview(accessToken: string): Promise<SystemOverview> {
  return request<SystemOverview>("/v1/system/overview", {}, accessToken);
}

export function getSystemUsage(accessToken: string, days: number): Promise<SystemUsage> {
  return request<SystemUsage>(`/v1/system/usage?days=${days}`, {}, accessToken);
}

export function listSystemUsers(accessToken: string): Promise<SystemUser[]> {
  return request<SystemUser[]>("/v1/system/users", {}, accessToken);
}

export function setAppAdmin(accessToken: string, userId: string, enabled: boolean): Promise<void> {
  return request<void>(`/v1/system/users/${encodeURIComponent(userId)}/app-admin`, {
    method: "PUT",
    body: JSON.stringify({ enabled }),
  }, accessToken);
}

export function setSystemAdmin(accessToken: string, userId: string, enabled: boolean): Promise<void> {
  return request<void>(`/v1/system/users/${encodeURIComponent(userId)}/system-admin`, {
    method: "PUT",
    body: JSON.stringify({ enabled }),
  }, accessToken);
}

export function endSystemUserSessions(accessToken: string, userId: string): Promise<void> {
  return request<void>(`/v1/system/users/${encodeURIComponent(userId)}/sessions/end`, { method: "POST" }, accessToken);
}

export function unlockSystemUser(accessToken: string, userId: string): Promise<{ cleared: number }> {
  return request<{ cleared: number }>(`/v1/system/users/${encodeURIComponent(userId)}/unlock`, { method: "POST" }, accessToken);
}

export function deactivateSystemUser(accessToken: string, userId: string): Promise<{ personal_tokens_revoked: number; invitations_withdrawn: number }> {
  return request<{ personal_tokens_revoked: number; invitations_withdrawn: number }>(`/v1/system/users/${encodeURIComponent(userId)}/deactivate`, { method: "POST" }, accessToken);
}

export function anonymizeSystemUser(accessToken: string, userId: string, confirm: string): Promise<{ memberships_removed: number; notices_removed: number; conversations_removed: number; summaries_rewritten: number }> {
  return request<{ memberships_removed: number; notices_removed: number; conversations_removed: number; summaries_rewritten: number }>(`/v1/system/users/${encodeURIComponent(userId)}/anonymize`, { method: "POST", body: JSON.stringify({ confirm }) }, accessToken);
}

export function reactivateSystemUser(accessToken: string, userId: string): Promise<void> {
  return request<void>(`/v1/system/users/${encodeURIComponent(userId)}/reactivate`, { method: "POST" }, accessToken);
}

export function getSystemSettings(accessToken: string): Promise<SystemSettings> {
  return request<SystemSettings>("/v1/system/settings", {}, accessToken);
}

export function saveSystemSettings(accessToken: string, values: Record<string, boolean | number | string>): Promise<SystemSettings> {
  return request<SystemSettings>("/v1/system/settings", {
    method: "PUT",
    body: JSON.stringify({ values }),
  }, accessToken);
}

export function resetSystemSetting(accessToken: string, key: string): Promise<SystemSettings> {
  return request<SystemSettings>(`/v1/system/settings/${encodeURIComponent(key)}`, { method: "DELETE" }, accessToken);
}

export function getSystemLogs(accessToken: string, filters: { level?: string; category?: string; workspace?: string; day?: string; q?: string; limit?: number }): Promise<SystemLogs> {
  const params = new URLSearchParams();
  if (filters.level) params.set("level", filters.level);
  if (filters.category) params.set("category", filters.category);
  if (filters.workspace) params.set("workspace", filters.workspace);
  if (filters.day) params.set("day", filters.day);
  if (filters.q) params.set("q", filters.q);
  params.set("limit", String(filters.limit ?? 300));
  return request<SystemLogs>(`/v1/system/logs?${params.toString()}`, {}, accessToken);
}

export function getSystemLogFiles(accessToken: string): Promise<SystemLogFiles> {
  return request<SystemLogFiles>("/v1/system/logs/files", {}, accessToken);
}

/** A day's log file (JSON lines), fetched with the user's session. */
export async function downloadSystemLogFile(accessToken: string, day: string): Promise<Blob> {
  const response = await authFetch(`${API_URL}/v1/system/logs/files/${encodeURIComponent(day)}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok) {
    const payload = await response.json().catch(() => null) as { error?: { message?: string } } | null;
    throw new SharedApiError(response.status, payload?.error?.message ?? `The shared API returned ${response.status}.`);
  }
  return response.blob();
}

/** The audit trail of a date range as a file (full system administrators). The download is itself audited. */
export async function downloadSystemAuditExport(accessToken: string, input: {
  format: "jsonl" | "csv";
  /** A day such as 2026-10-01; empty for the start of the trail. */
  from?: string;
  /** The last day to include; empty for now. */
  to?: string;
  /** Only the provider, linking, sign-in, enforcement and group-mapping events. */
  identityOnly?: boolean;
}): Promise<{ blob: Blob; filename: string; count: number }> {
  const query = new URLSearchParams({ format: input.format });
  if (input.from) query.set("from", input.from);
  if (input.to) query.set("to", input.to);
  if (input.identityOnly) query.set("scope", "identity");
  const response = await authFetch(`${API_URL}/v1/system/audit/export?${query}`, {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok) {
    const payload = await response.json().catch(() => null) as { error?: { message?: string } } | null;
    throw new SharedApiError(response.status, payload?.error?.message ?? `The shared API returned ${response.status}.`);
  }
  const named = /filename="([^"]+)"/.exec(response.headers.get("Content-Disposition") ?? "");
  return {
    blob: await response.blob(),
    filename: named?.[1] ?? `repomemo-audit.${input.format}`,
    count: Number(response.headers.get("X-Total-Count") ?? 0),
  };
}

export function listSystemAuditEvents(accessToken: string): Promise<SystemAuditEvent[]> {
  return request<SystemAuditEvent[]>("/v1/system/audit?limit=200", {}, accessToken);
}

export function listSystemJobs(accessToken: string, status?: string): Promise<SystemJob[]> {
  const query = status ? `?status=${encodeURIComponent(status)}&limit=100` : "?limit=100";
  return request<SystemJob[]>(`/v1/system/jobs${query}`, {}, accessToken);
}

export async function listSystemJobsPage(accessToken: string, status: string | undefined, cursor: string | null, limit = 24): Promise<{ items: SystemJob[]; total: number; nextCursor: string | null }> {
  const params = new URLSearchParams({ limit: String(limit) });
  if (status) params.set("status", status);
  if (cursor) params.set("cursor", cursor);
  const response = await authFetch(`${API_URL}/v1/system/jobs?${params}`, { headers: { Accept: "application/json", Authorization: `Bearer ${accessToken}` } });
  if (!response.ok) {
    const payload = await response.json().catch(() => null) as { error?: { message?: string } } | null;
    throw new SharedApiError(response.status, payload?.error?.message ?? `The shared API returned ${response.status}.`);
  }
  const items = await response.json() as SystemJob[];
  const total = Number(response.headers.get("X-Total-Count"));
  return { items, total: Number.isFinite(total) ? total : items.length, nextCursor: response.headers.get("X-Next-Cursor") };
}

export function getSystemMaintenance(accessToken: string): Promise<MaintenanceStatus> {
  return request<MaintenanceStatus>("/v1/system/maintenance", {}, accessToken);
}

export function runSystemMaintenance(accessToken: string): Promise<MaintenanceStatus> {
  return request<MaintenanceStatus>("/v1/system/maintenance/run", { method: "POST" }, accessToken);
}

/** What the mail page of System › Settings needs to save. `password`: absent keeps it, `""` removes it. */
export interface MailSettingsInput {
  enabled: boolean;
  host: string;
  port: number;
  security: string;
  username: string;
  password?: string;
  from_address: string;
  from_name: string;
}

/** "Forgot your password?": the answer is the same for every address, so it says nothing about who has an account. */
export function requestPasswordResetMail(email: string): Promise<{ message: string }> {
  return request<{ message: string }>("/v1/auth/forgot-password", { method: "POST", body: JSON.stringify({ email }) });
}

/** Uses the link mailed to an address; works signed in or not. */
export function verifyEmailLink(token: string): Promise<{ email: string }> {
  return request<{ email: string }>("/v1/auth/verify-email", { method: "POST", body: JSON.stringify({ token }) });
}

/** Sends the verification link of the signed-in account's address again. */
export function resendEmailVerification(accessToken: string): Promise<{ verified: boolean; sent: boolean }> {
  return request<{ verified: boolean; sent: boolean }>("/v1/profile/email/verify", { method: "POST", body: "{}" }, accessToken);
}

/** Changes the account's address (the password is asked again); it starts unverified. */
export function changeEmailAddress(accessToken: string, input: { email: string; currentPassword: string }): Promise<SharedUser> {
  return request<SharedUser>("/v1/profile/email", { method: "PUT", body: JSON.stringify({ email: input.email, current_password: input.currentPassword }) }, accessToken);
}

export interface NotificationPreference {
  kind: string;
  label: string;
  /** `both`, `app`, `mail` or `none`. */
  channel: string;
}

export function getNotificationPreferences(accessToken: string): Promise<NotificationPreference[]> {
  return request<NotificationPreference[]>("/v1/profile/notification-preferences", {}, accessToken);
}

export function setNotificationPreference(accessToken: string, kind: string, channel: string): Promise<NotificationPreference[]> {
  return request<NotificationPreference[]>("/v1/profile/notification-preferences", { method: "PUT", body: JSON.stringify({ kind, channel }) }, accessToken);
}

export function getSystemMail(accessToken: string): Promise<import("../types").MailOverview> {
  return request<import("../types").MailOverview>("/v1/system/mail", {}, accessToken);
}

export function saveSystemMail(accessToken: string, input: MailSettingsInput): Promise<import("../types").MailOverview> {
  return request<import("../types").MailOverview>("/v1/system/mail", { method: "PUT", body: JSON.stringify(input) }, accessToken);
}

/** Queues a test message (to the caller when `to` is absent) and answers at once with its outbox entry. */
export function sendSystemTestMail(accessToken: string, to?: string): Promise<import("../types").MailOutboxEntry> {
  return request<import("../types").MailOutboxEntry>("/v1/system/mail/test", { method: "POST", body: JSON.stringify(to ? { to } : {}) }, accessToken);
}

export function getSystemBackups(accessToken: string): Promise<BackupsOverview> {
  return request<BackupsOverview>("/v1/system/backups", {}, accessToken);
}

export function createSystemBackup(accessToken: string): Promise<BackupsOverview> {
  return request<BackupsOverview>("/v1/system/backups", { method: "POST", body: JSON.stringify({}) }, accessToken);
}

/** Copies the backups that have no checked second copy yet. */
export function copySystemBackups(accessToken: string): Promise<BackupsOverview> {
  return request<BackupsOverview>("/v1/system/backups/copy", { method: "POST", body: JSON.stringify({}) }, accessToken);
}

export function deleteSystemBackup(accessToken: string, name: string): Promise<void> {
  return request<void>(`/v1/system/backups/${encodeURIComponent(name)}`, { method: "DELETE" }, accessToken);
}

/** Seals a backup made with REPOMEMO_BACKUP_KEY_PREVIOUS again with the current key (V0.3.24). */
export function resealSystemBackup(accessToken: string, name: string): Promise<BackupsOverview> {
  return request<BackupsOverview>(`/v1/system/backups/${encodeURIComponent(name)}/reseal`, { method: "POST", body: JSON.stringify({}) }, accessToken);
}

export function restoreSystemBackup(accessToken: string, name: string, environment: string): Promise<BackupsOverview> {
  return request<BackupsOverview>(`/v1/system/backups/${encodeURIComponent(name)}/restore`, {
    method: "POST",
    body: JSON.stringify({ environment }),
  }, accessToken);
}

/** What the admin console shows when it opens: logo, greeting and the commands, for help and completion. */
export function getConsoleWelcome(accessToken: string): Promise<ConsoleWelcome> {
  return request<ConsoleWelcome>("/v1/system/console", {}, accessToken);
}

/** Runs one admin console line, such as `jobs list --status failed`. */
export function runConsoleCommand(accessToken: string, command: string): Promise<ConsoleResponse> {
  return request<ConsoleResponse>("/v1/system/console", {
    method: "POST",
    body: JSON.stringify({ command }),
  }, accessToken);
}

// Profile pictures. <img> cannot send the bearer token, so pictures are fetched
// and shown through object URLs, remembered per user for this page load.
const avatarCache = new Map<string, Promise<string | null>>();

/** The user's picture as an object URL, or null when they have none (or it is not visible to the caller). */
export function fetchSharedAvatar(userId: string): Promise<string | null> {
  const cached = avatarCache.get(userId);
  if (cached) return cached;
  const token = readStorage("sessionStorage", SHARED_SESSION_STORAGE_KEY);
  const pending = (async () => {
    if (!token) return null;
    try {
      const response = await authFetch(`${API_URL}/v1/users/${encodeURIComponent(userId)}/avatar`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      if (response.status !== 200) return null;
      const type = response.headers.get("Content-Type") ?? "";
      if (!/^image\/(png|jpeg|webp)$/.test(type)) return null;
      return URL.createObjectURL(await response.blob());
    } catch {
      return null;
    }
  })();
  avatarCache.set(userId, pending);
  // A failed lookup is retried next time instead of being remembered.
  void pending.then((url) => { if (!url && avatarCache.get(userId) === pending) avatarCache.delete(userId); });
  return pending;
}

const avatarListeners = new Set<(userId: string) => void>();

/** Calls back whenever a picture is replaced or removed in this tab, so shown copies refresh. */
export function onSharedAvatarChanged(listener: (userId: string) => void): () => void {
  avatarListeners.add(listener);
  return () => { avatarListeners.delete(listener); };
}

function forgetAvatar(userId: string) {
  const cached = avatarCache.get(userId);
  avatarCache.delete(userId);
  void cached?.then((url) => { if (url) setTimeout(() => URL.revokeObjectURL(url), 5000); });
  avatarListeners.forEach((listener) => listener(userId));
}

async function sendAvatar(accessToken: string, userId: string, init: RequestInit) {
  const response = await authFetch(`${API_URL}/v1/profile/avatar`, {
    ...init,
    headers: { ...(init.headers as Record<string, string> | undefined), Authorization: `Bearer ${accessToken}`, Accept: "application/json" },
  });
  if (!response.ok) {
    const payload = await response.json().catch(() => null) as { error?: { message?: string } } | null;
    throw new SharedApiError(response.status, payload?.error?.message ?? `The shared API returned ${response.status}.`);
  }
  forgetAvatar(userId);
}

export function uploadSharedAvatar(accessToken: string, userId: string, picture: Blob): Promise<void> {
  return sendAvatar(accessToken, userId, { method: "PUT", body: picture, headers: { "Content-Type": picture.type } });
}

export function deleteSharedAvatar(accessToken: string, userId: string): Promise<void> {
  return sendAvatar(accessToken, userId, { method: "DELETE" });
}

// First-run server setup

/** Public: whether this server still has to be set up. */
export function getServerSetup(): Promise<ServerSetupStatus> {
  return request<ServerSetupStatus>("/v1/setup");
}

/** Public, new server only: checks the one-time setup code from the server console. */
export function verifySetupCode(code: string): Promise<void> {
  return request<void>("/v1/setup/verify-code", { method: "POST", body: JSON.stringify({ code }) });
}

/** Public, new server only: creates the first system administrator and signs them in. */
export async function createSetupAdmin(input: { code: string; email: string; displayName: string; password: string }): Promise<TokenResponse> {
  const response = await request<TokenResponse>("/v1/setup/admin", {
    method: "POST",
    body: JSON.stringify({ code: input.code, email: input.email, display_name: input.displayName, password: input.password }),
  });
  storeTokens(response);
  return response;
}

export function getSetupChecks(accessToken: string): Promise<SetupCheck[]> {
  return request<SetupCheck[]>("/v1/setup/checks", {}, accessToken);
}

/** Completes the setup for good. */
export function completeServerSetup(accessToken: string): Promise<ServerSetupStatus> {
  return request<ServerSetupStatus>("/v1/setup/complete", { method: "POST" }, accessToken);
}

// Platform: which environment folder the server is attached to

export interface PlatformStatus {
  attached: boolean;
  environment: string | null;
  /** The only folder environments may live in, such as "workspace-data". */
  folder: string;
  /** The environment every start attaches (`REPOMEMO_SERVER_DATA_DIR`). Null: the
   *  environment chosen in the web app is remembered and attached again at startup. */
  pinned_environment: string | null;
}

export interface EnvironmentEntry {
  name: string;
  status: "valid" | "empty" | "invalid";
  detail: string;
}

/** Public: whether the server has an environment attached. */
export function getPlatformStatus(): Promise<PlatformStatus> {
  return request<PlatformStatus>("/v1/platform");
}

/** The folders of the environments directory with their verification; the code proves access to the server console. */
export function listEnvironments(code: string): Promise<EnvironmentEntry[]> {
  return request<EnvironmentEntry[]>("/v1/platform/environments/list", { method: "POST", body: JSON.stringify({ code }) });
}

export function createEnvironment(code: string, name: string): Promise<PlatformStatus> {
  return request<PlatformStatus>("/v1/platform/environments/create", { method: "POST", body: JSON.stringify({ code, name }) });
}

export function attachEnvironment(code: string, name: string): Promise<PlatformStatus> {
  return request<PlatformStatus>("/v1/platform/environments/attach", { method: "POST", body: JSON.stringify({ code, name }) });
}

/** System administrators: ends every session and detaches the environment; the server then waits for one to be chosen. */
export async function detachEnvironment(accessToken: string): Promise<void> {
  await request<void>("/v1/system/environment/detach", { method: "POST" }, accessToken);
}

// Invitations, password reset links and personal access tokens

export type InvitationTarget = "organization" | "workspace";

/** Creates an invitation link; its secret is returned only now. */
export function createSharedInvitation(accessToken: string, targetType: InvitationTarget, targetId: string, input: { email: string; role: string }): Promise<CreatedInvitation> {
  const base = targetType === "workspace" ? "workspaces" : "organizations";
  return request<CreatedInvitation>(`/v1/${base}/${encodeURIComponent(targetId)}/invitations`, { method: "POST", body: JSON.stringify(input) }, accessToken);
}

export function listSharedInvitations(accessToken: string, targetType: InvitationTarget, targetId: string): Promise<Invitation[]> {
  const base = targetType === "workspace" ? "workspaces" : "organizations";
  return request<Invitation[]>(`/v1/${base}/${encodeURIComponent(targetId)}/invitations`, {}, accessToken);
}

export function revokeSharedInvitation(accessToken: string, invitationId: string): Promise<void> {
  return request<void>(`/v1/invitations/${encodeURIComponent(invitationId)}`, { method: "DELETE" }, accessToken);
}

/** Public: what an invitation link is for. */
export function previewSharedInvitation(token: string): Promise<InvitationPreview> {
  return request<InvitationPreview>("/v1/invitations/preview", { method: "POST", body: JSON.stringify({ token }) });
}

export function acceptSharedInvitation(accessToken: string, token: string): Promise<AcceptedInvitation> {
  return request<AcceptedInvitation>("/v1/invitations/accept", { method: "POST", body: JSON.stringify({ token }) }, accessToken);
}

/** System administrators: a one-time link that lets the user choose a new password. */
export function issueSharedPasswordReset(accessToken: string, userId: string): Promise<IssuedPasswordReset> {
  return request<IssuedPasswordReset>(`/v1/system/users/${encodeURIComponent(userId)}/password-reset`, { method: "POST" }, accessToken);
}

export function previewSharedPasswordReset(token: string): Promise<PasswordResetPreview> {
  return request<PasswordResetPreview>("/v1/auth/password-reset/preview", { method: "POST", body: JSON.stringify({ token }) });
}

/** Sets the new password and signs this browser in; every other session of the account ends. */
export async function completeSharedPasswordReset(token: string, password: string): Promise<TokenResponse> {
  const response = await request<TokenResponse>("/v1/auth/password-reset", { method: "POST", body: JSON.stringify({ token, password }) });
  storeTokens(response);
  return response;
}

export function listPersonalTokens(accessToken: string): Promise<PersonalAccessToken[]> {
  return request<PersonalAccessToken[]>("/v1/profile/tokens", {}, accessToken);
}

/** Creates a personal access token; its secret is returned only now. */
export function createPersonalToken(accessToken: string, input: { name: string; scope: "read" | "write" | "admin" | "system"; expiresInDays: number }): Promise<CreatedPersonalToken> {
  return request<CreatedPersonalToken>("/v1/profile/tokens", {
    method: "POST",
    body: JSON.stringify({ name: input.name, scope: input.scope, expires_in_days: input.expiresInDays }),
  }, accessToken);
}

export function revokePersonalToken(accessToken: string, tokenId: string): Promise<void> {
  return request<void>(`/v1/profile/tokens/${encodeURIComponent(tokenId)}`, { method: "DELETE" }, accessToken);
}

