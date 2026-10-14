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
