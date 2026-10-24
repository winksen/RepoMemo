import { SafeMarkdown } from "../../components/SafeMarkdown";
import { useEffect, useRef, useState } from "react";
import {
  IconFolderCode as FolderCode,
  IconGitBranch as GitBranch,
  IconGitCommit as GitCommit,
  IconRefresh as Refresh,
  IconSettings as Settings,
  IconSparkles as Sparkles,
  IconStack2 as Layers,
} from "@tabler/icons-react";
import { cancelSharedJob, getSharedRepositoryDetail, summarizeSharedRepository, syncSharedRepository } from "../../lib/sharedApi";
import type { RepositoryDetailResponse } from "../../types";
import { apiMessage, formatBytes, formatTime } from "../format";
import { LanguageChart } from "../../components/LanguageChart";
import { LoadingState } from "../../components/ui/loading-state";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { CopyButton } from "../../components/ui/copy-button";
import { Table } from "../../components/ui/table";
import { PanelHeading } from "../../components/ui/panel-heading";
import { showToast } from "../../components/ui/toast";
import { useLiveEvents } from "../../lib/liveEvents";
import { ICON_SIZE } from "../../components/ui/icon";
import {
  POLL_INTERVAL_MS,
  LIVE_FALLBACK_POLL_MS,
  STAGE_LABEL,
  STATUS_LABEL,
  STATUS_TONE,
  isRunning,
  repoSyncJob,
  reportSummary,
} from "./shared";
import { RepositoryFiles } from "./FileBrowser";
import { repositoryKeys } from "../../lib/queryClient";
import { useServerState } from "../../lib/useServerState";
import { ViewAtCommit } from "../common";

/** Order and labels of the technology groups in the project analysis. */
const STACK_CATEGORIES: [string, string][] = [
  ["language", "Language"], ["meta-framework", "Framework"], ["frontend", "Frontend"], ["backend", "Backend"],
  ["desktop", "Desktop"], ["mobile", "Mobile"], ["cli", "Command line"], ["game", "Game"], ["data", "Data"],
  ["database", "Database"], ["runtime", "Runtime"], ["testing", "Testing"], ["tooling", "Tooling"],
];

const KEY_FILE_ROLE: Record<string, string> = {
  readme: "README",
  manifest: "Manifest",
  entry_point: "Entry point",
  docs: "Docs",
};

/** The page of the evidence item that stands for a whole repository: its
 *  sync state, an AI summary on request, and the overview of what it holds. */
export function RepositoryDetailView({
  accessToken,
  canSync,
  onOpenArtifact,
  repositoryId,
}: {
  accessToken: string;
  canSync: boolean;
  onOpenArtifact: (artifactId: string) => void;
  repositoryId: string;
}) {
  const [summarizing, setSummarizing] = useState(false);
  const [filesOpen, setFilesOpen] = useState(false);
  const runningJob = useRef<string | null>(null);
  const streamIsUp = useRef(false);
  const [detail, setDetail, query] = useServerState<RepositoryDetailResponse | null>({
    key: repositoryKeys.detail(repositoryId),
    fetch: () => getSharedRepositoryDetail(accessToken, repositoryId),
    initial: null,
    staleTime: 0,
    // While a sync runs the page is read again: now and then if the stream shows its progress, often if not.
    refetchInterval: (current) => current?.repository.active_job ? (streamIsUp.current ? LIVE_FALLBACK_POLL_MS : POLL_INTERVAL_MS) : false,
  });
  const load = async () => { await query.refetch(); };

  useEffect(() => { runningJob.current = null; }, [repositoryId]);
  useEffect(() => {
    if (query.error) showToast("error", apiMessage(query.error, "The repository could not be loaded."));
  }, [query.error]);
  // Each time the page's data changes: a sync that was running and no longer is has finished.
  useEffect(() => {
    if (!detail) return;
    const job = detail.repository.active_job;
    if (job) runningJob.current = job.id;
    else if (runningJob.current) {
      runningJob.current = null;
      if (detail.repository.status === "error") showToast("error", `${detail.repository.name}: ${detail.repository.last_error ?? "the sync failed."}`);
      else if (detail.repository.last_report) showToast("success", `${detail.repository.name} synced. ${reportSummary(detail.repository.last_report)}.`);
    }
  }, [detail]);
  streamIsUp.current = useLiveEvents((event) => {
    if (event.type === "resync") { void load(); return; }
    const job = repoSyncJob(event);
    if (!job || job.source_id !== repositoryId) return;
    if (!isRunning(job)) { void load(); return; }
    runningJob.current = job.id;
    setDetail((current) => current ? { ...current, repository: { ...current.repository, active_job: job } } : current);
  });

  async function sync() {
    try {
      const { job } = await syncSharedRepository(accessToken, repositoryId);
      if (job) runningJob.current = job.id;
      await load();
    } catch (error) {
      showToast("error", apiMessage(error, "The sync could not be started."));
    }
  }

  async function stop() {
    const job = detail?.repository.active_job;
    if (!job) return;
    try {
      await cancelSharedJob(accessToken, job.id);
      await load();
    } catch (error) {
      showToast("error", apiMessage(error, "The sync could not be stopped."));
    }
  }

  async function summarize() {
    setSummarizing(true);
    try {
      const summary = await summarizeSharedRepository(accessToken, repositoryId);
      setDetail((current) => current ? { ...current, summary } : current);
      showToast("success", "Summary generated.");
    } catch (error) {
      showToast("error", apiMessage(error, "The summary could not be generated."));
    } finally {
      setSummarizing(false);
    }
  }

  if (!detail) return <section className="shared-record-panel" aria-busy="true"><LoadingState label="Loading the repository" /></section>;

  const { repository, overview, summary } = detail;
  const job = repository.active_job;
  const status = job ? "syncing" : repository.status;
  const commit = repository.last_synced_commit;
  const summaryIsStale = Boolean(summary && commit && summary.commit_sha !== commit.sha);
  // Each file the summary is based on, once, with the first passage cited and the commit the summary describes.
  const citedFiles = summary ? Array.from(new Map(summary.citations.map((citation) => [citation.artifact_id, citation])).values()) : [];
  const progress = job?.progress_total ? Math.min(100, Math.round((job.progress_current / job.progress_total) * 100)) : null;

  return <>
    <section className="shared-record-panel rm-repo-page">
      <PanelHeading actions={<Badge tone={STATUS_TONE[status]}>{STATUS_LABEL[status]}</Badge>} icon={GitBranch} title="Repository" />
      <dl className="rm-repo-facts">
        <div><dt>Location</dt><dd><code className="rm-repo-path">{repository.root_path}</code></dd></div>
        <div><dt><GitBranch size={ICON_SIZE.sm} /> Branch</dt><dd>{commit?.branch ?? repository.settings.branch ?? "Checked-out branch"}</dd></div>
        <div><dt><GitCommit size={ICON_SIZE.sm} /> Commit</dt><dd>{commit ? <><code>{commit.sha.slice(0, 7)}</code> <CopyButton label="Copy the commit hash" value={commit.sha} /> {commit.summary} <span className="rm-map-muted">· {commit.author_name} · {formatTime(commit.committed_at)}</span></> : <span className="rm-map-muted">Not synced yet</span>}</dd></div>
        <div><dt>Files</dt><dd>{repository.file_count.toLocaleString()} indexed{repository.last_synced_at ? <span className="rm-map-muted"> · synced {formatTime(repository.last_synced_at)}</span> : null}</dd></div>
        {!job && repository.status === "error" && repository.last_error ? <div><dt>Last error</dt><dd>{repository.last_error}</dd></div> : null}
      </dl>
      {job ? <div className="rm-repo-progress" role="status">
        <div><span>{STAGE_LABEL[job.stage] ?? job.stage}</span><span>{job.progress_total ? `${job.progress_current.toLocaleString()} / ${job.progress_total.toLocaleString()}` : ""}</span></div>
        <progress aria-label={`Sync progress for ${repository.name}`} max={100} value={progress ?? undefined} />
      </div> : null}
      <div className="rm-health-actions">
        {canSync ? job
          ? <Button disabled={job.cancel_requested} onClick={() => void stop()} variant="secondary">{job.cancel_requested ? "Stopping…" : "Stop sync"}</Button>
          : <Button onClick={() => void sync()} variant="secondary"><Refresh size={ICON_SIZE.sm} /> Sync now</Button> : null}
        <Button aria-expanded={filesOpen} disabled={!repository.file_count} onClick={() => setFilesOpen((open) => !open)} variant="secondary">{filesOpen ? "Hide files" : "Browse files"}</Button>
      </div>
      {filesOpen ? <RepositoryFiles accessToken={accessToken} onOpenArtifact={onOpenArtifact} repository={repository} /> : null}
    </section>

    <section className="shared-record-panel rm-repo-page">
      <PanelHeading description={summary ? summary.provider_name : null} icon={Sparkles} title="Summary" />
      {summary ? <>
        <div className="shared-markdown rm-repo-summary"><SafeMarkdown>{summary.summary_markdown}</SafeMarkdown></div>
        <p className="rm-map-muted">Written for commit <code>{summary.commit_sha.slice(0, 7)}</code> on {formatTime(summary.generated_at)}.{summaryIsStale ? " The repository has changed since; regenerate it to describe the current commit." : ""} {summary.warnings.join(" ")}</p>
        {citedFiles.length ? <div className="rm-repo-cited"><span className="rm-map-muted">Based on</span>{citedFiles.map((citation) => <span className="rm-repo-cited-file" key={citation.artifact_id}><button className="rm-map-link" onClick={() => onOpenArtifact(citation.artifact_id)} type="button">{citation.path}</button><ViewAtCommit accessToken={accessToken} citation={citation} /></span>)}</div> : null}
      </> : <p className="shared-muted-copy">{detail.ai_available ? "No summary yet. A summary describes what the repository is, its main parts and where to start reading, from the overview below and the opening of its key files." : "A summary needs a text AI provider, which an administrator can set up in Settings. The overview below needs no AI."}</p>}
      {canSync && detail.ai_available && overview ? <div className="rm-health-actions"><Button busy={summarizing} onClick={() => void summarize()} variant={summary && !summaryIsStale ? "secondary" : "main"}><Sparkles size={ICON_SIZE.sm} /> {summarizing ? "Summarizing…" : summary ? "Regenerate summary" : "Generate summary"}</Button></div> : null}
    </section>

    {overview?.stack ? <section className="shared-record-panel rm-repo-page">
      <PanelHeading description="detected from manifests" icon={Layers} title="Project analysis" />
      <p className="rm-stack-summary">{overview.stack.summary}</p>
      <dl className="rm-stack-groups">
        {STACK_CATEGORIES.map(([category, label]) => {
          const items = overview.stack!.technologies.filter((technology) => technology.category === category);
          return items.length ? <div key={category}><dt>{label}</dt><dd>{items.map((technology) => <Badge key={technology.name}>{technology.name}</Badge>)}</dd></div> : null;
        })}
      </dl>
    </section> : null}

    <section className="shared-record-panel rm-repo-page">
      <PanelHeading description={overview ? <>{overview.file_count.toLocaleString()} files · {formatBytes(overview.total_bytes)}</> : null} icon={FolderCode} title="Content overview" />
      {overview ? <div className="rm-repo-overview">
        <div className="rm-repo-languages">
          <h3 className="rm-repo-caption">Languages</h3>
          <LanguageChart languages={overview.languages} totalBytes={overview.total_bytes} />
        </div>
        <Table>
          
            <caption className="rm-repo-caption">Structure</caption>
            <thead><tr><th scope="col">Folder</th><th className="rm-table-number" scope="col">Files</th></tr></thead>
            <tbody>{overview.folders.map((folder) => <tr key={folder.path}><th scope="row">{folder.path ? <code>{folder.path}/</code> : "Repository root"}</th><td className="rm-table-number">{folder.files.toLocaleString()}</td></tr>)}</tbody>
          </Table>
        {overview.key_files.length ? <div className="rm-repo-block">
          <h3>Key files</h3>
          <ul className="rm-repo-keyfiles">{overview.key_files.map((file) => <li key={file.artifact_id}><button className="rm-map-link" onClick={() => onOpenArtifact(file.artifact_id)} type="button">{file.path}</button><span className="rm-health-fate">{KEY_FILE_ROLE[file.role] ?? file.role}</span></li>)}</ul>
        </div> : null}
        {overview.readme_excerpt ? <div className="rm-repo-block">
          <h3>From the README</h3>
          <div className="shared-markdown rm-repo-readme"><SafeMarkdown>{overview.readme_excerpt}</SafeMarkdown></div>
        </div> : null}
        {overview.recent_commits.length ? <div className="rm-repo-block">
          <h3>Recent commits</h3>
          <ul className="rm-repo-commits">{overview.recent_commits.map((entry) => <li key={entry.sha}><code>{entry.sha.slice(0, 7)}</code><span>{entry.summary}</span><span className="rm-map-muted">{entry.author_name} · {formatTime(entry.committed_at)}</span></li>)}</ul>
        </div> : null}
      </div> : <p className="shared-muted-copy">The overview appears after the first complete sync.</p>}
    </section>
  </>;
}
