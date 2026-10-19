import { useEffect, useRef } from "react";
import { listSharedRepositories } from "../../lib/sharedApi";
import type { RepoSource } from "../../types";
import { apiMessage } from "../format";
import { showToast } from "../../components/ui/toast";
import { useLiveEvents } from "../../lib/liveEvents";
import { workspaceKeys } from "../../lib/queryClient";
import { useServerState } from "../../lib/useServerState";
import { POLL_INTERVAL_MS, LIVE_FALLBACK_POLL_MS, REPOSITORY_LIST_ACTIONS, isRunning, repoSyncJob, reportSummary } from "./shared";

/** Loads the workspace's repositories through the query cache, follows each
 *  sync's progress from the workspace event stream (polling while a sync runs
 *  and the stream is down), and announces each sync that finishes. */
export function useRepositories(accessToken: string, workspaceId: string, onChanged: () => void) {
  // Jobs seen running, so the end of a sync can be announced once.
  const runningJobs = useRef(new Map<string, string>());
  const streamIsUp = useRef(false);
  const [repositories, setRepositories, query] = useServerState<RepoSource[] | null>({
    key: workspaceKeys.repositories(workspaceId),
    fetch: async () => (await listSharedRepositories(accessToken, workspaceId)).repositories,
    initial: null,
    staleTime: 0,
    // While a sync runs the list is read again: now and then if the stream shows its progress, often if not.
    refetchInterval: (current) => current?.some((repository) => repository.active_job) ? (streamIsUp.current ? LIVE_FALLBACK_POLL_MS : POLL_INTERVAL_MS) : false,
  });
  const load = async () => { await query.refetch(); };

  useEffect(() => { runningJobs.current.clear(); }, [workspaceId]);
  useEffect(() => {
    if (query.error) showToast("error", apiMessage(query.error, "Repositories could not be loaded."));
  }, [query.error]);

  // Each time the list changes: a sync that was running and no longer is has finished.
  useEffect(() => {
    if (!repositories) return;
    let finished = false;
    for (const repository of repositories) {
      const wasRunning = runningJobs.current.has(repository.id);
      if (repository.active_job) {
        runningJobs.current.set(repository.id, repository.active_job.id);
        continue;
      }
      if (!wasRunning) continue;
      runningJobs.current.delete(repository.id);
      finished = true;
      if (repository.status === "error") {
        showToast("error", `${repository.name}: ${repository.last_error ?? "the sync failed."}`);
      } else if (repository.last_report?.cancelled) {
        showToast("warning", `${repository.name}: sync stopped. Sync again to finish.`);
      } else if (repository.last_report) {
        showToast("success", `${repository.name} synced. ${reportSummary(repository.last_report)}.`);
      }
    }
    if (finished) onChanged();
  }, [repositories]);

  function track(repositoryId: string, jobId: string | undefined) {
    if (jobId) runningJobs.current.set(repositoryId, jobId);
  }

  streamIsUp.current = useLiveEvents((event) => {
    if (event.type === "resync") { void load(); return; }
    if (event.type === "activity") {
      if (REPOSITORY_LIST_ACTIONS.has(event.event.action)) void load();
      return;
    }
    const job = repoSyncJob(event);
    if (!job) return;
    if (!isRunning(job)) { void load(); return; }
    runningJobs.current.set(job.source_id!, job.id);
    setRepositories((current) => current?.map((repository) => repository.id === job.source_id ? { ...repository, active_job: job } : repository) ?? current);
  });

  return { repositories, load, track };
}
