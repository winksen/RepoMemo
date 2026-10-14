import {
  getSharedRetrievalFacets, getSharedWorkspaceActivityCalendar, getSharedWorkspaceCapabilities, getSharedWorkspaceMetrics,
  getSharedWorkspaceOverview, listSharedArtifacts, listSharedCollaborationTasks, listSharedFolders, listSharedIndexFailures,
  listSharedMemoryCards, listSharedSavedSearches, listSharedWorkspaceActivity, listSharedWorkspaceAiProviders,
  listSharedWorkspaceJobs, listSharedWorkspaceMembers,
} from "../../lib/sharedApi";
import { workspaceKeys } from "../../lib/queryClient";
import { useServerState } from "../../lib/useServerState";
import {
  type ArtifactIndexFailure, type ArtifactSummary, type CollaborationTask, type Folder, type IndexingJobStatus,
  type MemoryCardSummary, type RetrievalFacets, type SavedSearch, type SharedAiProviderSettings,
  type WorkspaceActivityCalendar, type WorkspaceActivityEvent, type WorkspaceCapabilities, type WorkspaceMember,
  type WorkspaceMetrics, type WorkspaceOverview,
} from "../../types";

/**
 * The server state of one workspace, as hooks over the query cache. A page and
 * its sections call the same hook with the same arguments and share one entry:
 * the cache fetches once and every reader sees the same data.
 */

export function useCapabilities(accessToken: string, workspaceId: string) {
  return useServerState<WorkspaceCapabilities | null>({
    key: workspaceKeys.capabilities(workspaceId), fetch: () => getSharedWorkspaceCapabilities(accessToken, workspaceId), initial: null,
  });
}

export function useOverview(accessToken: string, workspaceId: string) {
  return useServerState<WorkspaceOverview | null>({
    key: workspaceKeys.overview(workspaceId), fetch: () => getSharedWorkspaceOverview(accessToken, workspaceId), initial: null,
  });
}

export function useMetrics(accessToken: string, workspaceId: string) {
  return useServerState<WorkspaceMetrics | null>({
    key: workspaceKeys.metrics(workspaceId), fetch: () => getSharedWorkspaceMetrics(accessToken, workspaceId), initial: null,
  });
}

export function useArtifacts(accessToken: string, workspaceId: string) {
  return useServerState<ArtifactSummary[]>({
    key: workspaceKeys.artifacts(workspaceId), fetch: () => listSharedArtifacts(accessToken, workspaceId), initial: [],
  });
}

export function useIndexFailures(accessToken: string, workspaceId: string) {
  return useServerState<Record<string, ArtifactIndexFailure>>({
    key: workspaceKeys.indexFailures(workspaceId),
    fetch: async () => Object.fromEntries((await listSharedIndexFailures(accessToken, workspaceId)).map((failure) => [failure.artifact_id, failure])),
    initial: {},
  });
}

export function useFolders(accessToken: string, workspaceId: string) {
  return useServerState<Folder[]>({
    key: workspaceKeys.folders(workspaceId), fetch: () => listSharedFolders(accessToken, workspaceId), initial: [],
  });
}

export function useMemoryCards(accessToken: string, workspaceId: string) {
  return useServerState<MemoryCardSummary[]>({
    key: workspaceKeys.memoryCards(workspaceId), fetch: () => listSharedMemoryCards(accessToken, workspaceId), initial: [],
  });
}

export function useRetrievalFacets(accessToken: string, workspaceId: string) {
  return useServerState<RetrievalFacets | null>({
    key: workspaceKeys.facets(workspaceId), fetch: () => getSharedRetrievalFacets(accessToken, workspaceId), initial: null,
  });
}

/** Running jobs seed the live progress; the stream keeps them current. */
export function useRunningJobs(accessToken: string, workspaceId: string) {
  return useServerState<IndexingJobStatus[]>({
    key: workspaceKeys.runningJobs(workspaceId),
    fetch: () => listSharedWorkspaceJobs(accessToken, workspaceId, "running").catch(() => [] as IndexingJobStatus[]),
    initial: [],
  });
}

export function useMembers(accessToken: string, workspaceId: string, enabled: boolean) {
  return useServerState<WorkspaceMember[]>({
    key: workspaceKeys.members(workspaceId), fetch: () => listSharedWorkspaceMembers(accessToken, workspaceId), initial: [], enabled,
  });
}

export function useAiProviders(accessToken: string, workspaceId: string, enabled: boolean) {
  return useServerState<SharedAiProviderSettings[]>({
    key: workspaceKeys.aiProviders(workspaceId), fetch: () => listSharedWorkspaceAiProviders(accessToken, workspaceId), initial: [], enabled,
  });
}

export function useActivity(accessToken: string, workspaceId: string, enabled: boolean) {
  return useServerState<WorkspaceActivityEvent[]>({
    key: workspaceKeys.activity(workspaceId), fetch: () => listSharedWorkspaceActivity(accessToken, workspaceId), initial: [], enabled,
  });
}

export function useActivityCalendar(accessToken: string, workspaceId: string, enabled: boolean) {
  return useServerState<WorkspaceActivityCalendar | null>({
    key: workspaceKeys.activityCalendar(workspaceId), fetch: () => getSharedWorkspaceActivityCalendar(accessToken, workspaceId), initial: null, enabled,
  });
}

export function useTasks(accessToken: string, workspaceId: string, enabled: boolean) {
  return useServerState<CollaborationTask[]>({
    key: workspaceKeys.tasks(workspaceId), fetch: () => listSharedCollaborationTasks(accessToken, workspaceId), initial: [], enabled,
  });
}

export function useSavedSearches(accessToken: string, workspaceId: string, enabled: boolean) {
  return useServerState<SavedSearch[]>({
    key: workspaceKeys.savedSearches(workspaceId), fetch: () => listSharedSavedSearches(accessToken, workspaceId), initial: [], enabled,
  });
}
