import { QueryClient } from "@tanstack/react-query";

/**
 * The one cache of server state for the web app.
 *
 * Data is served from the cache at once and refreshed in the background when
 * it is older than `staleTime`; live events and mutations mark exactly the
 * entries they touched as stale (see `workspaceKeys`).
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      gcTime: 5 * 60_000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

/** Cache keys of one workspace's server state. Everything starts with `["workspace", id]`. */
export const workspaceKeys = {
  all: (workspaceId: string) => ["workspace", workspaceId] as const,
  capabilities: (workspaceId: string) => ["workspace", workspaceId, "capabilities"] as const,
  overview: (workspaceId: string) => ["workspace", workspaceId, "overview"] as const,
  metrics: (workspaceId: string) => ["workspace", workspaceId, "metrics"] as const,
  facets: (workspaceId: string) => ["workspace", workspaceId, "retrieval-facets"] as const,
  artifacts: (workspaceId: string) => ["workspace", workspaceId, "artifacts"] as const,
  indexFailures: (workspaceId: string) => ["workspace", workspaceId, "index-failures"] as const,
  folders: (workspaceId: string) => ["workspace", workspaceId, "folders"] as const,
  memoryCards: (workspaceId: string) => ["workspace", workspaceId, "memory-cards"] as const,
  runningJobs: (workspaceId: string) => ["workspace", workspaceId, "running-jobs"] as const,
  members: (workspaceId: string) => ["workspace", workspaceId, "members"] as const,
  aiProviders: (workspaceId: string) => ["workspace", workspaceId, "ai-providers"] as const,
  activity: (workspaceId: string) => ["workspace", workspaceId, "activity"] as const,
  activityCalendar: (workspaceId: string) => ["workspace", workspaceId, "activity-calendar"] as const,
  tasks: (workspaceId: string) => ["workspace", workspaceId, "tasks"] as const,
  savedSearches: (workspaceId: string) => ["workspace", workspaceId, "saved-searches"] as const,
  repositories: (workspaceId: string) => ["workspace", workspaceId, "repositories"] as const,
  connectors: (workspaceId: string) => ["workspace", workspaceId, "connectors"] as const,
  knowledgeMap: (workspaceId: string) => ["workspace", workspaceId, "knowledge-map"] as const,
  template: (workspaceId: string) => ["workspace", workspaceId, "template"] as const,
  assistantCapabilities: (workspaceId: string) => ["workspace", workspaceId, "assistant-capabilities"] as const,
  assistantChats: (workspaceId: string) => ["workspace", workspaceId, "assistant-chats"] as const,
  // What the document viewer reads about one file.
  artifactPreview: (workspaceId: string, artifactId: string) => ["workspace", workspaceId, "artifact", artifactId, "preview"] as const,
  artifactRenderStatus: (workspaceId: string, artifactId: string) => ["workspace", workspaceId, "artifact", artifactId, "render-status"] as const,
  artifactRendered: (workspaceId: string, artifactId: string) => ["workspace", workspaceId, "artifact", artifactId, "rendered"] as const,
  artifactFile: (workspaceId: string, artifactId: string) => ["workspace", workspaceId, "artifact", artifactId, "file"] as const,
  // Detail pages live under their workspace, so a full reload of the workspace refreshes them too.
  artifact: (workspaceId: string, artifactId: string) => ["workspace", workspaceId, "artifact", artifactId] as const,
  artifactComments: (workspaceId: string, artifactId: string) => ["workspace", workspaceId, "artifact", artifactId, "comments"] as const,
  artifactLifecycle: (workspaceId: string, artifactId: string) => ["workspace", workspaceId, "artifact", artifactId, "lifecycle"] as const,
  artifactLifecycleEvents: (workspaceId: string, artifactId: string) => ["workspace", workspaceId, "artifact", artifactId, "lifecycle-events"] as const,
  artifactChunks: (workspaceId: string, artifactId: string) => ["workspace", workspaceId, "artifact", artifactId, "chunks"] as const,
  memoryCard: (workspaceId: string, cardId: string) => ["workspace", workspaceId, "memory-card", cardId] as const,
};

/** Cache keys of one repository's pages (its page and its file list). A repository page does not know its workspace. */
export const repositoryKeys = {
  detail: (repositoryId: string) => ["repository", repositoryId, "detail"] as const,
  files: (repositoryId: string, commit: string | undefined, indexedFiles: number) => ["repository", repositoryId, "files", commit ?? "", indexedFiles] as const,
};

/** Cache keys of what the server offers to everyone who is signed in. The cache is cleared when the session ends. */
export const serverKeys = {
  templates: () => ["server", "templates"] as const,
  template: (templateId: string) => ["server", "templates", templateId] as const,
};

/** Cache keys of what belongs to the signed-in person, not to a workspace. The cache is cleared when the session ends. */
export const accountKeys = {
  dashboard: (workspaceIds: string) => ["dashboard", workspaceIds] as const,
  profile: (userId: string) => ["account", userId, "profile"] as const,
  profileTasks: (userId: string) => ["account", userId, "profile-tasks"] as const,
  notifications: (userId: string) => ["account", userId, "notifications"] as const,
  notificationPreferences: (userId: string) => ["account", userId, "notification-preferences"] as const,
};

/** Cache keys of the System area (administrators only). Every key starts with `["system"]`, so one call clears them all. */
export const systemKeys = {
  all: ["system"] as const,
  overview: () => ["system", "overview"] as const,
  usage: (days: number) => ["system", "usage", days] as const,
  users: () => ["system", "users"] as const,
  settings: () => ["system", "settings"] as const,
  platform: () => ["system", "platform"] as const,
  logFiles: () => ["system", "log-files"] as const,
  logs: (filter: Record<string, string | undefined>) => ["system", "logs", filter] as const,
  backups: () => ["system", "backups"] as const,
  jobs: (status: string) => ["system", "jobs", status] as const,
  jobsAll: () => ["system", "jobs"] as const,
  audit: () => ["system", "audit"] as const,
};

/** What a kind of change makes stale (P7): live events name one of these instead of reloading the page. */
export const workspaceInvalidation = {
  ledger: (workspaceId: string) => [workspaceKeys.artifacts(workspaceId), workspaceKeys.indexFailures(workspaceId), workspaceKeys.folders(workspaceId)],
  memory: (workspaceId: string) => [workspaceKeys.memoryCards(workspaceId)],
  tasks: (workspaceId: string) => [workspaceKeys.tasks(workspaceId)],
  metrics: (workspaceId: string) => [workspaceKeys.overview(workspaceId), workspaceKeys.metrics(workspaceId), workspaceKeys.facets(workspaceId)],
} as const;

/** Marks entries stale; the ones on screen are refetched, the others the next time they are shown. */
export function invalidateKeys(keys: ReadonlyArray<readonly unknown[]>): Promise<void> {
  return Promise.all(keys.map((queryKey) => queryClient.invalidateQueries({ queryKey }))).then(() => undefined);
}
