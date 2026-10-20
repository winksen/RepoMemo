import { IconFolderCode as FolderCode, IconGitBranch as GitBranch, IconGitCommit as GitCommit, IconRefresh as Refresh } from "@tabler/icons-react";
import type { RepoSource } from "../../types";
import { formatTime } from "../format";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { CopyButton } from "../../components/ui/copy-button";
import { ICON_SIZE } from "../../components/ui/icon";
import { STAGE_LABEL, STATUS_LABEL, STATUS_TONE, reportSummary } from "./shared";
import { RepositoryFiles } from "./FileBrowser";

export function RepositoryCard({
  accessToken,
  canSync,
  filesOpen,
  onOpenArtifact,
  onStop,
  onSync,
  onToggleFiles,
  repository,
}: {
  accessToken: string;
  canSync: boolean;
  filesOpen: boolean;
  onOpenArtifact: (artifactId: string) => void;
  onStop: () => void;
  onSync: () => void;
  onToggleFiles: () => void;
  repository: RepoSource;
}) {
  const job = repository.active_job;
  const commit = repository.last_synced_commit;
  const report = repository.last_report;
  const status = job ? "syncing" : repository.status;
  const progress = job?.progress_total ? Math.min(100, Math.round((job.progress_current / job.progress_total) * 100)) : null;
  const settings = repository.settings;

  return <li className={`rm-repo-card status-${status}`}>
    <div className="rm-repo-head">
      <div>
        <h3>{repository.name}</h3>
        <code className="rm-repo-path">{repository.root_path}</code>
      </div>
      <Badge tone={STATUS_TONE[status]}>{STATUS_LABEL[status]}</Badge>
    </div>

    <dl className="rm-repo-facts">
      <div><dt><GitBranch size={ICON_SIZE.sm} /> Branch</dt><dd>{commit?.branch ?? settings.branch ?? "Checked-out branch"}{settings.branch ? null : <span className="rm-map-muted"> (follows HEAD)</span>}</dd></div>
      <div><dt><GitCommit size={ICON_SIZE.sm} /> Commit</dt><dd>{commit ? <><code>{commit.sha.slice(0, 7)}</code> <CopyButton label="Copy the commit hash" value={commit.sha} /> {commit.summary} <span className="rm-map-muted">· {commit.author_name} · {formatTime(commit.committed_at)}</span></> : <span className="rm-map-muted">Not synced yet</span>}</dd></div>
      <div><dt>Files</dt><dd>{repository.indexed_file_count === repository.file_count ? repository.file_count.toLocaleString() : `${repository.indexed_file_count.toLocaleString()} of ${repository.file_count.toLocaleString()} indexed`}{repository.last_synced_at ? <span className="rm-map-muted"> · synced {formatTime(repository.last_synced_at)}</span> : null}</dd></div>
      {settings.include.length || settings.exclude.length ? <div><dt>Rules</dt><dd className="rm-repo-rules">{settings.include.map((pattern) => <code key={`i-${pattern}`}>+ {pattern}</code>)}{settings.exclude.map((pattern) => <code key={`e-${pattern}`}>− {pattern}</code>)}</dd></div> : null}
      {!job && repository.status === "error" && repository.last_error ? <div><dt>Last error</dt><dd>{repository.last_error}</dd></div> : null}
    </dl>

    {job ? <div className="rm-repo-progress" role="status">
      <div><span>{STAGE_LABEL[job.stage] ?? job.stage}</span><span>{job.progress_total ? `${job.progress_current.toLocaleString()} / ${job.progress_total.toLocaleString()}` : ""}</span></div>
      <progress aria-label={`Sync progress for ${repository.name}`} max={100} value={progress ?? undefined} />
    </div> : null}

    {!job && report ? <div className="rm-repo-report">
      <span>Last sync: {reportSummary(report)}{report.cancelled ? " · stopped before the end" : ""}{report.index_failed ? ` · ${report.index_failed} could not be indexed` : ""}</span>
      {report.skipped ? <details>
        <summary>{report.skipped.toLocaleString()} of {report.files_in_tree.toLocaleString()} files skipped</summary>
        <ul>{report.skipped_by_reason.map((skip) => <li key={skip.reason}><strong>{skip.count.toLocaleString()}</strong> {skip.reason}<span className="rm-map-muted"> — {skip.examples.join(", ")}{skip.count > skip.examples.length ? ", …" : ""}</span></li>)}</ul>
      </details> : null}
    </div> : null}

    <div className="rm-health-actions">
      {job ? (canSync ? <Button disabled={job.cancel_requested} onClick={onStop} variant="secondary">{job.cancel_requested ? "Stopping…" : "Stop sync"}</Button> : null)
        : canSync ? <Button onClick={onSync} variant="secondary"><Refresh size={ICON_SIZE.sm} /> Sync now</Button> : null}
      {repository.overview_artifact_id ? <Button onClick={() => onOpenArtifact(repository.overview_artifact_id!)} variant="secondary"><FolderCode size={ICON_SIZE.sm} /> Open overview</Button> : null}
      <Button aria-expanded={filesOpen} disabled={!repository.file_count} onClick={onToggleFiles} variant="secondary">{filesOpen ? "Hide files" : "Browse files"}</Button>
    </div>

    {filesOpen ? <RepositoryFiles accessToken={accessToken} onOpenArtifact={onOpenArtifact} repository={repository} /> : null}
  </li>;
}
