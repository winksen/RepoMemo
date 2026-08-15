import type {
  ArtifactDetail,
  ArtifactSummary,
  IndexingJobStatus,
  MemoryCard,
  MemoryCardSummary,
  Organization,
  SearchResult,
  SharedSession,
  SharedUser,
  SharedWorkspace,
  WorkspaceOverview,
} from "../types";

const API_URL = (import.meta.env.VITE_REPOMEMO_API_URL ?? "http://127.0.0.1:8787").replace(/\/$/, "");

export const sharedApiUrl = API_URL;

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

export function getSharedWorkspaceOverview(accessToken: string, workspaceId: string): Promise<WorkspaceOverview> {
  return request<WorkspaceOverview>(`/v1/workspaces/${workspaceId}/overview`, {}, accessToken);
}

export function listSharedArtifacts(accessToken: string, workspaceId: string): Promise<ArtifactSummary[]> {
  return request<ArtifactSummary[]>(`/v1/workspaces/${workspaceId}/artifacts`, {}, accessToken);
}

export function getSharedArtifact(accessToken: string, artifactId: string): Promise<ArtifactDetail> {
  return request<ArtifactDetail>(`/v1/artifacts/${artifactId}`, {}, accessToken);
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

export function indexSharedWorkspace(accessToken: string, workspaceId: string): Promise<IndexingJobStatus> {
  return request<IndexingJobStatus>(`/v1/workspaces/${workspaceId}/index`, { method: "POST" }, accessToken);
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

export function createSharedMemoryCard(accessToken: string, workspaceId: string, input: {
  title: string;
  bodyMarkdown: string;
  source: string;
}): Promise<MemoryCard> {
  return request<MemoryCard>(`/v1/workspaces/${workspaceId}/memory-cards`, {
    method: "POST",
    body: JSON.stringify({
      title: input.title,
      body_markdown: input.bodyMarkdown,
      source: input.source,
      confidence: null,
      citations: [],
    }),
  }, accessToken);
}
