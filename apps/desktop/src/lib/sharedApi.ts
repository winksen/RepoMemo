import type {
  AskAnswer,
  ArtifactDetail,
  ArtifactSummary,
  IndexingJobStatus,
  MemoryCard,
  MemoryCardDetail,
  MemoryCardSummary,
  Organization,
  ProviderTestResult,
  SearchResult,
  SharedSession,
  SharedAiProviderSettings,
  SharedUser,
  WorkspaceMember,
  WorkspaceRole,
  WorkspaceActivityEvent,
  WorkspaceAiOverview,
  WorkspaceCapabilities,
  SharedWorkspace,
  WorkspaceOverview,
} from "../types";

const API_URL = (import.meta.env.VITE_REPOMEMO_API_URL ?? "http://127.0.0.1:8787").replace(/\/$/, "");

export const sharedApiUrl = API_URL;

export function getSharedHealth(): Promise<{ service: string; status: string; authentication: string }> {
  return request<{ service: string; status: string; authentication: string }>("/health");
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

  const response = await fetch(`${API_URL}${path}`, { ...options, headers });
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
  const response = await fetch(`${API_URL}${path}`, {
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
  return request<TokenResponse>("/v1/auth/register", {
    method: "POST",
    body: JSON.stringify({
      email: input.email,
      display_name: input.displayName,
      password: input.password,
    }),
  });
}

export async function loginSharedUser(input: {
  email: string;
  password: string;
}): Promise<TokenResponse> {
  return request<TokenResponse>("/v1/auth/login", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function getSharedSession(accessToken: string): Promise<SharedSession> {
  return request<SharedSession>("/v1/session", {}, accessToken);
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

export function listSharedArtifacts(accessToken: string, workspaceId: string): Promise<ArtifactSummary[]> {
  return request<ArtifactSummary[]>(`/v1/workspaces/${workspaceId}/artifacts`, {}, accessToken);
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

export function createSharedTextArtifact(accessToken: string, workspaceId: string, input: {
  title: string;
  content: string;
  language?: string;
}): Promise<ArtifactSummary> {
  return request<ArtifactSummary>(`/v1/workspaces/${workspaceId}/artifacts/text`, {
    method: "POST",
    body: JSON.stringify(input),
  }, accessToken);
}

export async function uploadSharedArtifact(accessToken: string, workspaceId: string, file: File): Promise<ArtifactSummary> {
  const response = await fetch(`${API_URL}/v1/workspaces/${workspaceId}/artifacts/upload`, {
    method: "POST",
    headers: {
      Accept: "application/json",
      Authorization: `Bearer ${accessToken}`,
      "Content-Type": file.type || "application/octet-stream",
      "X-RepoMemo-Filename": file.name,
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

export function searchSharedWorkspace(accessToken: string, workspaceId: string, query: string): Promise<SearchResult[]> {
  return request<SearchResult[]>(`/v1/workspaces/${workspaceId}/search`, {
    method: "POST",
    body: JSON.stringify({ query, artifact_types: [], languages: [], source_ids: [], limit: 20 }),
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
