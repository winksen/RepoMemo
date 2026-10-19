import { useEffect, useRef, useState } from "react";
import { listSharedRepositories } from "../../lib/sharedApi";
import type { RepoSource } from "../../types";
import { apiMessage } from "../format";
import { showToast } from "../../components/ui/toast";
import { useLiveEvents } from "../../lib/liveEvents";
import { POLL_INTERVAL_MS, LIVE_FALLBACK_POLL_MS, REPOSITORY_LIST_ACTIONS, isRunning, repoSyncJob, reportSummary } from "./shared";

/** Loads the workspace's repositories, follows each sync's progress from the
 *  workspace event stream (polling while the stream is down), and announces
 *  each sync that finishes. */
export function useRepositories(accessToken: string, workspaceId: string, onChanged: () => void) {
  const [repositories, setRepositories] = useState<RepoSource[] | null>(null);
  // Jobs seen running, so the end of a sync can be announced once.
  const runningJobs = useRef(new Map<string, string>());

  async function load() {
    try {
      const next = await listSharedRepositories(accessToken, workspaceId);
      announceFinishedSyncs(next.repositories);
      setRepositories(next.repositories);
    } catch (error) {
      showToast("error", apiMessage(error, "Repositories could not be loaded."));
    }
  }

  function announceFinishedSyncs(next: RepoSource[]) {
    let finished = false;
    for (const repository of next) {
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
  }

  function track(repositoryId: string, jobId: string | undefined) {
    if (jobId) runningJobs.current.set(repositoryId, jobId);
  }

  useEffect(() => {
    setRepositories(null);
    runningJobs.current.clear();
    void load();
  }, [accessToken, workspaceId]);

  const live = useLiveEvents((event) => {
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

  const syncing = Boolean(repositories?.some((repository) => repository.active_job));
  useEffect(() => {
    if (!syncing) return;
    const timer = window.setInterval(() => void load(), live ? LIVE_FALLBACK_POLL_MS : POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [syncing, live, accessToken, workspaceId]);

  return { repositories, load, track };
}
