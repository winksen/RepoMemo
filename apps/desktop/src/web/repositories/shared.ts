import type { BadgeTone } from "../../components/ui/badge";
import type { RepoSource, RepoSyncReport, IndexingJobStatus, LiveEvent, WorkspaceTemplateView } from "../../types";

export const POLL_INTERVAL_MS = 1500;
/** With the workspace event stream open, polling is only a safety net. */
export const LIVE_FALLBACK_POLL_MS = 15_000;
/** Activity that changes the list of repositories (syncs arrive as job events). */
export const REPOSITORY_LIST_ACTIONS = new Set(["repository_connected", "repository_updated", "repository_removed"]);

export const STAGE_LABEL: Record<string, string> = {
  queued: "Waiting for another sync to finish",
  fetching_repository: "Fetching from the host",
  reading_repository: "Reading the repository",
  storing_files: "Storing changed files",
  indexing: "Indexing",
};

/** The badge tone of a repository's sync status. */
export const STATUS_TONE: Record<RepoSource["status"], BadgeTone> = {
  pending: "neutral",
  syncing: "info",
  ready: "success",
  error: "danger",
};

export const STATUS_LABEL: Record<RepoSource["status"], string> = {
  pending: "Not synced yet",
  syncing: "Syncing",
  ready: "Up to date",
  error: "Sync failed",
};

export type RepoForm = {
  link: string; name: string; branch: string; include: string; exclude: string;
  token: string; tokenUsername: string; deployKey: string; hostKey: string;
};

export const EMPTY_FORM: RepoForm = { link: "", name: "", branch: "", include: "", exclude: "", token: "", tokenUsername: "", deployKey: "", hostKey: "" };

/** The include and exclude rules a workspace's template offers the link form (V0.2.186), as the form's own lines of text. */
export type RepoOffer = { include: string; exclude: string; templateName: string | null };

export const NO_OFFER: RepoOffer = { include: "", exclude: "", templateName: null };

/** What the template offers; nothing for a template that has no rules (`general`), so its form is empty as ever. */
export function repoOffer(template: WorkspaceTemplateView | null): RepoOffer {
  if (!template) return NO_OFFER;
  const { include, exclude } = template.repositories;
  if (!include.length && !exclude.length) return NO_OFFER;
  return { include: include.join("\n"), exclude: exclude.join("\n"), templateName: template.template.name };
}

/** An empty form carrying the offer; the person keeps, edits or clears it before anything is linked. */
export function formWithOffer(offer: RepoOffer): RepoForm {
  return { ...EMPTY_FORM, include: offer.include, exclude: offer.exclude };
}

/** An ssh address: `ssh://…` or the `git@host:owner/repo` form. */
export function isSshLink(link: string) {
  return /^ssh:\/\//i.test(link.trim()) || /^[^\s\\/]+@[^\s/]+:/.test(link.trim());
}

/** An address of a repository on a host, as opposed to a folder on the server. */
export function isRemoteLink(link: string) {
  return /^(https?|ssh|git):\/\//i.test(link.trim()) || /^[^\s\\/]+@[^\s/]+:/.test(link.trim());
}

/** What the server is given for a token: nothing when none was typed. */
export function formToken(form: RepoForm) {
  if (isSshLink(form.link)) {
    const deployKey = form.deployKey.trim();
    return deployKey ? { deploy_key: deployKey, host_key: form.hostKey.trim() || undefined } : {};
  }
  const token = form.token.trim();
  return token ? { token, token_username: form.tokenUsername.trim() || undefined } : {};
}

export function lines(value: string) {
  return value.split("\n").map((line) => line.trim()).filter(Boolean);
}

export function formSettings(form: RepoForm) {
  return { branch: form.branch.trim() || null, include: lines(form.include), exclude: lines(form.exclude) };
}

export function isRunning(job: IndexingJobStatus) {
  return job.status === "running" || job.status === "pending";
}

/** The repository sync job an event is about, if any. */
export function repoSyncJob(event: LiveEvent): IndexingJobStatus | null {
  return event.type === "job" && event.job.kind === "repo_sync" && event.job.source_id ? event.job : null;
}

export function reportSummary(report: RepoSyncReport) {
  if (report.nothing_changed) return "Nothing changed";
  const parts = [
    report.added + report.restored ? `${report.added + report.restored} added` : "",
    report.updated ? `${report.updated} updated` : "",
    report.renamed ? `${report.renamed} renamed` : "",
    report.removed ? `${report.removed} removed` : "",
  ].filter(Boolean);
  return parts.length ? parts.join(" · ") : "No file changes";
}
