import {
  getSharedRetrievalFacets, getSharedWorkspaceActivityCalendar, getSharedWorkspaceCapabilities, getSharedWorkspaceMetrics,
  getSharedWorkspaceOverview, listSharedArtifactWindow, listSharedCollaborationTasks, listSharedFolders, listSharedIndexFailures,
  listSharedMemoryCards, listSharedSavedSearches, listSharedWorkspaceActivity, listSharedWorkspaceAiProviders,
  listSharedWorkspaceJobs, listSharedWorkspaceMembers, querySharedArtifacts, ARTIFACT_WINDOW,
} from "../../lib/sharedApi";
import { useCallback, useRef, useState, type SetStateAction } from "react";
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

/** The newest files of the workspace (one page) and how many files it has in all. */
export type ArtifactWindow = { items: ArtifactSummary[]; total: number };

/**
 * The files a workspace page holds: the newest `ARTIFACT_WINDOW`, one request, which a live event
 * fetches again as one page. `[files, setFiles, query, total]`: when `total` is more than the files,
 * the others are reached with `useArtifactSearch`. The setter changes the files only.
 */
export function useArtifacts(accessToken: string, workspaceId: string, enabled = true) {
  const [held, setHeld, query] = useServerState<ArtifactWindow>({
    key: workspaceKeys.artifacts(workspaceId), fetch: () => listSharedArtifactWindow(accessToken, workspaceId), initial: { items: [], total: 0 }, enabled,
  });
  const setItems = useCallback((next: SetStateAction<ArtifactSummary[]>) => {
    setHeld((current) => ({ items: typeof next === "function" ? next(current.items) : next, total: current.total }));
  }, [setHeld]);
  return [held.items, setItems, query, held.total] as const;
}

/** The files whose name or path holds `text`, from the server, newest first: how a workspace with more files than a page is searched. */
export function useArtifactSearch(accessToken: string, workspaceId: string, text: string, enabled: boolean) {
  return useServerState<ArtifactSummary[]>({
    key: [...workspaceKeys.artifacts(workspaceId), "search", text],
    fetch: () => querySharedArtifacts(accessToken, workspaceId, { query: text, limit: ARTIFACT_WINDOW }),
    initial: [],
    enabled: enabled && text.length > 0,
    keepPrevious: true,
  });
}

/**
 * The files a picker offers: the newest ones while nothing is typed, and the server's matches for what
 * is typed, so a file of any age can be chosen. `find` and `known` also answer for files chosen earlier
 * and no longer listed. `[search]` goes to the picker's `onSearch`.
 */
export function useArtifactPicker(accessToken: string, workspaceId: string, enabled = true) {
  const [newest, , query] = useArtifacts(accessToken, workspaceId, enabled);
  const [text, search] = useState("");
  const [matches] = useArtifactSearch(accessToken, workspaceId, text, enabled && text.length > 0);
  const seen = useRef(new Map<string, ArtifactSummary>());
  for (const artifact of [...newest, ...matches]) seen.current.set(artifact.id, artifact);
  const files = text ? matches : newest;
  const find = useCallback((id: string) => seen.current.get(id), []);
  const known = useCallback(() => [...seen.current.values()], []);
  return { files, search, find, known, query };
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
