import type { FormEvent } from "react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  IconAlertCircle as AlertCircle,
  IconFolderCode as FolderCode,
  IconGitBranch as GitBranch,
  IconGitCommit as GitCommit,
  IconPencil as Pencil,
  IconPlus as Plus,
  IconRefresh as Refresh,
  IconTrash as Trash,
} from "@tabler/icons-react";
import {
  cancelSharedJob,
  connectSharedRepository,
  deleteSharedRepository,
  listSharedRepositories,
  listSharedRepositoryFiles,
  syncSharedRepository,
  updateSharedRepository,
} from "../lib/sharedApi";
import type { RepoFile, RepoSource, RepoSyncReport, RepositoryList } from "../types";
import { Button } from "./ui/button";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Input } from "./ui/input";
import { Textarea } from "./ui/textarea";
import { showToast } from "./ui/toast";

const POLL_INTERVAL_MS = 1500;
/** Files listed at once; a filter narrows larger repositories. */
const FILE_LIST_LIMIT = 300;

const STAGE_LABEL: Record<string, string> = {
  queued: "Waiting for another sync to finish",
  reading_repository: "Reading the repository",
  storing_files: "Storing changed files",
  indexing: "Indexing",
};

const STATUS_LABEL: Record<RepoSource["status"], string> = {
  pending: "Not synced yet",
  syncing: "Syncing",
  ready: "Up to date",
  error: "Sync failed",
};

type RepoDialog =
  | { kind: "connect" }
  | { kind: "settings"; repository: RepoSource }
  | { kind: "remove"; repository: RepoSource };

type RepoForm = { path: string; name: string; branch: string; include: string; exclude: string };

const EMPTY_FORM: RepoForm = { path: "", name: "", branch: "", include: "", exclude: "" };

function formatDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

function lines(value: string) {
  return value.split("\n").map((line) => line.trim()).filter(Boolean);
}

function reportSummary(report: RepoSyncReport) {
  const parts = [
    report.added + report.restored ? `${report.added + report.restored} added` : "",
    report.updated ? `${report.updated} updated` : "",
    report.renamed ? `${report.renamed} renamed` : "",
    report.removed ? `${report.removed} removed` : "",
  ].filter(Boolean);
  return parts.length ? parts.join(" · ") : "No file changes";
}

/** Git repositories connected to the workspace: what each one indexes, how
 *  far its last sync got, and the files it keeps in step with the repository. */
export function RepositoriesPanel({
  accessToken,
  canManage,
  canSync,
  onChanged,
  onOpenArtifact,
  workspaceId,
}: {
  accessToken: string;
  canManage: boolean;
  canSync: boolean;
  onChanged: () => void;
  onOpenArtifact: (artifactId: string) => void;
  workspaceId: string;
}) {
  const [list, setList] = useState<RepositoryList | null>(null);
  const [dialog, setDialog] = useState<RepoDialog | null>(null);
  const [form, setForm] = useState<RepoForm>(EMPTY_FORM);
  const [busy, setBusy] = useState(false);
  const [openFilesId, setOpenFilesId] = useState<string | null>(null);
  // Jobs seen running, so the end of a sync can be announced once.
  const runningJobs = useRef(new Map<string, string>());

  async function load() {
    try {
      const next = await listSharedRepositories(accessToken, workspaceId);
      announceFinishedSyncs(next.repositories);
      setList(next);
    } catch (error) {
      showToast("error", error instanceof Error ? error.message : "Repositories could not be loaded.");
    }
  }

  function announceFinishedSyncs(repositories: RepoSource[]) {
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
  }

  useEffect(() => {
    setList(null);
    runningJobs.current.clear();
    void load();
  }, [accessToken, workspaceId]);

  const syncing = Boolean(list?.repositories.some((repository) => repository.active_job));
  useEffect(() => {
    if (!syncing) return;
    const timer = window.setInterval(() => void load(), POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [syncing, accessToken, workspaceId]);

  function openConnect() {
    setForm(EMPTY_FORM);
    setDialog({ kind: "connect" });
  }

  function openSettings(repository: RepoSource) {
    setForm({
      path: repository.root_path,
      name: repository.name,
      branch: repository.settings.branch ?? "",
      include: repository.settings.include.join("\n"),
      exclude: repository.settings.exclude.join("\n"),
    });
    setDialog({ kind: "settings", repository });
  }

  async function submitConnect(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      const { repository, job } = await connectSharedRepository(accessToken, workspaceId, {
        path: form.path.trim(),
        name: form.name.trim() || undefined,
        branch: form.branch.trim() || null,
        include: lines(form.include),
        exclude: lines(form.exclude),
      });
      if (job) runningJobs.current.set(repository.id, job.id);
      setDialog(null);
      showToast("success", `Connected ${repository.name}. The first sync has started.`);
      await load();
    } catch (error) {
      showToast("error", error instanceof Error ? error.message : "The repository could not be connected.");
    } finally {
      setBusy(false);
    }
  }

  async function submitSettings(event: FormEvent, repository: RepoSource) {
    event.preventDefault();
    setBusy(true);
    try {
      await updateSharedRepository(accessToken, repository.id, {
        name: form.name.trim() || undefined,
        branch: form.branch.trim() || null,
        include: lines(form.include),
        exclude: lines(form.exclude),
      });
      setDialog(null);
      showToast("success", "Settings saved. They apply from the next sync.");
      await load();
    } catch (error) {
      showToast("error", error instanceof Error ? error.message : "The settings could not be saved.");
    } finally {
      setBusy(false);
    }
  }

  async function remove(repository: RepoSource) {
    setBusy(true);
    try {
      await deleteSharedRepository(accessToken, repository.id);
      setDialog(null);
      if (openFilesId === repository.id) setOpenFilesId(null);
      showToast("success", `Removed ${repository.name} and its files.`);
      await load();
      onChanged();
    } catch (error) {
      showToast("error", error instanceof Error ? error.message : "The repository could not be removed.");
    } finally {
      setBusy(false);
    }
  }

  async function sync(repository: RepoSource) {
    try {
      const { job } = await syncSharedRepository(accessToken, repository.id);
      if (job) runningJobs.current.set(repository.id, job.id);
      await load();
    } catch (error) {
      showToast("error", error instanceof Error ? error.message : "The sync could not be started.");
    }
  }

  async function stop(repository: RepoSource) {
    if (!repository.active_job) return;
    try {
      await cancelSharedJob(accessToken, repository.active_job.id);
      await load();
    } catch (error) {
      showToast("error", error instanceof Error ? error.message : "The sync could not be stopped.");
    }
  }

  if (!list) {
    return <section className="rm-repos" aria-busy="true"><p className="shared-muted-copy">Loading repositories…</p></section>;
  }

  const count = list.repositories.length;
  return <section className="rm-repos">
    <div className="rm-map-toolbar">
      <span>{count ? `${count} ${count === 1 ? "repository" : "repositories"}` : "No repositories connected"}</span>
      {canManage && list.local_repositories_enabled ? <Button onClick={openConnect} type="button" variant="main"><Plus size={15} /> Connect repository</Button> : null}
    </div>

    {!list.local_repositories_enabled ? <p className="rm-map-notice">
      {canManage
        ? <>Local repositories are turned off on this server. To allow them, restart the server with <code>REPOMEMO_REPO_ROOTS</code> set to the folders that hold your repositories (separate several with <code>;</code> on Windows, <code>:</code> elsewhere).</>
        : "Local repositories are turned off on this server. An administrator can turn them on."}
    </p> : null}

    {count ? <ul className="rm-repo-list">
      {list.repositories.map((repository) => <RepositoryCard
        canManage={canManage}
        canSync={canSync && list.local_repositories_enabled}
        filesOpen={openFilesId === repository.id}
        key={repository.id}
        onOpenArtifact={onOpenArtifact}
        onRemove={() => setDialog({ kind: "remove", repository })}
        onSettings={() => openSettings(repository)}
        onStop={() => void stop(repository)}
        onSync={() => void sync(repository)}
        onToggleFiles={() => setOpenFilesId((current) => current === repository.id ? null : repository.id)}
        accessToken={accessToken}
        repository={repository}
      />)}
    </ul> : list.local_repositories_enabled ? <div className="shared-empty-state">
      <FolderCode size={25} />
      <strong>No repositories yet</strong>
      <span>{canManage ? "Connect a git repository on this server. Its committed files are indexed and kept in step with each sync." : "An administrator can connect a git repository to this workspace."}</span>
    </div> : null}

    {dialog?.kind === "connect" ? <Dialog
      className="rm-dialog-wide"
      description={<>Choose a git checkout on the server. RepoMemo reads what is committed on the branch, never uncommitted changes or ignored files.</>}
      footer={<><DialogCancel onClick={() => setDialog(null)} /><Button disabled={busy || !form.path.trim()} form="rm-repo-form" type="submit" variant="main">Connect and sync</Button></>}
      onClose={() => setDialog(null)}
      open
      title="Connect a repository"
    >
      <RepositoryForm allowedRoots={list.allowed_roots} form={form} id="rm-repo-form" onChange={setForm} onSubmit={submitConnect} showPath />
    </Dialog> : null}

    {dialog?.kind === "settings" ? <Dialog
      className="rm-dialog-wide"
      description={<>Changes apply from the next sync. Files no longer matched leave search; files newly matched are added.</>}
      footer={<><DialogCancel onClick={() => setDialog(null)} /><Button disabled={busy} form="rm-repo-form" type="submit" variant="main">Save settings</Button></>}
      onClose={() => setDialog(null)}
      open
      title={`${dialog.repository.name} settings`}
    >
      <RepositoryForm allowedRoots={[]} form={form} id="rm-repo-form" onChange={setForm} onSubmit={(event) => void submitSettings(event, dialog.repository)} showPath={false} />
    </Dialog> : null}

    {dialog?.kind === "remove" ? <Dialog
      description={<>Remove <strong>{dialog.repository.name}</strong> and its {dialog.repository.file_count.toLocaleString()} files from the workspace? Their index, comments and memory-card links are deleted too. The repository on disk is not touched.</>}
      footer={<><DialogCancel onClick={() => setDialog(null)} /><Button className="rm-button-danger" disabled={busy} onClick={() => void remove(dialog.repository)} type="button" variant="secondary">Remove repository</Button></>}
      onClose={() => setDialog(null)}
      open
      title="Remove repository"
    /> : null}
  </section>;
}

function RepositoryForm({
  allowedRoots,
  form,
  id,
  onChange,
  onSubmit,
  showPath,
}: {
  allowedRoots: string[];
  form: RepoForm;
  id: string;
  onChange: (form: RepoForm) => void;
  onSubmit: (event: FormEvent) => void;
  showPath: boolean;
}) {
  const set = (key: keyof RepoForm) => (event: { target: { value: string } }) => onChange({ ...form, [key]: event.target.value });
  return <form className="rm-repo-form" id={id} onSubmit={onSubmit}>
    {showPath ? <label>Folder
      <Input autoFocus onChange={set("path")} placeholder={allowedRoots[0] ? `${allowedRoots[0]}${allowedRoots[0].includes("\\") ? "\\" : "/"}my-repo` : "Path to a git checkout"} required value={form.path} />
      {allowedRoots.length ? <span className="rm-map-muted">Allowed under {allowedRoots.join(", ")}. Any folder inside a repository works; its root is used.</span> : null}
    </label> : null}
    <div className="rm-repo-form-row">
      <label>Name<Input onChange={set("name")} placeholder={showPath ? "The folder name" : ""} value={form.name} /></label>
      <label>Branch<Input onChange={set("branch")} placeholder="The checked-out branch" value={form.branch} /></label>
    </div>
    <label>Include only
      <Textarea onChange={set("include")} placeholder={"docs/**\nsrc/"} rows={3} value={form.include} />
      <span className="rm-map-muted">One pattern per line. Leave empty to index everything.</span>
    </label>
    <label>Exclude
      <Textarea onChange={set("exclude")} placeholder={"**/*.test.ts\nfixtures/"} rows={3} value={form.exclude} />
      <span className="rm-map-muted">Added to the defaults: lock files, minified files, <code>node_modules/</code>, <code>vendor/</code>, <code>dist/</code>, <code>build/</code>, <code>target/</code>, images and files over 1 MB.</span>
    </label>
  </form>;
}

function RepositoryCard({
  accessToken,
  canManage,
  canSync,
  filesOpen,
  onOpenArtifact,
  onRemove,
  onSettings,
  onStop,
  onSync,
  onToggleFiles,
  repository,
}: {
  accessToken: string;
  canManage: boolean;
  canSync: boolean;
  filesOpen: boolean;
  onOpenArtifact: (artifactId: string) => void;
  onRemove: () => void;
  onSettings: () => void;
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
      <span className={`rm-repo-status status-${status}`}>{STATUS_LABEL[status]}</span>
    </div>

    <dl className="rm-repo-facts">
      <div><dt><GitBranch size={14} /> Branch</dt><dd>{commit?.branch ?? settings.branch ?? "Checked-out branch"}{settings.branch ? null : <span className="rm-map-muted"> (follows HEAD)</span>}</dd></div>
      <div><dt><GitCommit size={14} /> Commit</dt><dd>{commit ? <><code>{commit.sha.slice(0, 7)}</code> {commit.summary} <span className="rm-map-muted">· {commit.author_name} · {formatDate(commit.committed_at)}</span></> : <span className="rm-map-muted">Not synced yet</span>}</dd></div>
      <div><dt>Files</dt><dd>{repository.indexed_file_count === repository.file_count ? repository.file_count.toLocaleString() : `${repository.indexed_file_count.toLocaleString()} of ${repository.file_count.toLocaleString()} indexed`}{repository.last_synced_at ? <span className="rm-map-muted"> · synced {formatDate(repository.last_synced_at)}</span> : null}</dd></div>
      {settings.include.length || settings.exclude.length ? <div><dt>Rules</dt><dd className="rm-repo-rules">{settings.include.map((pattern) => <code key={`i-${pattern}`}>+ {pattern}</code>)}{settings.exclude.map((pattern) => <code key={`e-${pattern}`}>− {pattern}</code>)}</dd></div> : null}
    </dl>

    {job ? <div className="rm-repo-progress" role="status">
      <div><span>{STAGE_LABEL[job.stage] ?? job.stage}</span><span>{job.progress_total ? `${job.progress_current.toLocaleString()} / ${job.progress_total.toLocaleString()}` : ""}</span></div>
      <progress aria-label={`Sync progress for ${repository.name}`} max={100} value={progress ?? undefined} />
    </div> : null}

    {!job && repository.status === "error" && repository.last_error ? <p className="rm-repo-error"><AlertCircle size={15} /> {repository.last_error}</p> : null}

    {!job && report ? <div className="rm-repo-report">
      <span>Last sync: {reportSummary(report)}{report.cancelled ? " · stopped before the end" : ""}{report.index_failed ? ` · ${report.index_failed} could not be indexed` : ""}</span>
      {report.skipped ? <details>
        <summary>{report.skipped.toLocaleString()} of {report.files_in_tree.toLocaleString()} files skipped</summary>
        <ul>{report.skipped_by_reason.map((skip) => <li key={skip.reason}><strong>{skip.count.toLocaleString()}</strong> {skip.reason}<span className="rm-map-muted"> — {skip.examples.join(", ")}{skip.count > skip.examples.length ? ", …" : ""}</span></li>)}</ul>
      </details> : null}
    </div> : null}

    <div className="rm-health-actions">
      {job ? (canSync ? <Button disabled={job.cancel_requested} onClick={onStop} type="button" variant="secondary">{job.cancel_requested ? "Stopping…" : "Stop sync"}</Button> : null)
        : canSync ? <Button onClick={onSync} type="button" variant="secondary"><Refresh size={15} /> Sync now</Button> : null}
      <Button aria-expanded={filesOpen} disabled={!repository.file_count} onClick={onToggleFiles} type="button" variant="secondary">{filesOpen ? "Hide files" : "Browse files"}</Button>
      {canManage ? <Button disabled={Boolean(job)} onClick={onSettings} type="button" variant="secondary"><Pencil size={15} /> Settings</Button> : null}
      {canManage ? <Button className="rm-button-danger" disabled={Boolean(job)} onClick={onRemove} type="button" variant="secondary"><Trash size={15} /> Remove</Button> : null}
    </div>

    {filesOpen ? <RepositoryFiles accessToken={accessToken} onOpenArtifact={onOpenArtifact} repository={repository} /> : null}
  </li>;
}

function RepositoryFiles({
  accessToken,
  onOpenArtifact,
  repository,
}: {
  accessToken: string;
  onOpenArtifact: (artifactId: string) => void;
  repository: RepoSource;
}) {
  const [files, setFiles] = useState<RepoFile[] | null>(null);
  const [filter, setFilter] = useState("");
  const commitSha = repository.last_synced_commit?.sha;

  useEffect(() => {
    let cancelled = false;
    listSharedRepositoryFiles(accessToken, repository.id)
      .then((next) => { if (!cancelled) setFiles(next); })
      .catch((error) => showToast("error", error instanceof Error ? error.message : "The files could not be loaded."));
    return () => { cancelled = true; };
  }, [accessToken, repository.id, commitSha, repository.indexed_file_count]);

  const matching = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    return (files ?? []).filter((file) => !needle || file.path.toLowerCase().includes(needle));
  }, [files, filter]);

  if (!files) return <p className="shared-muted-copy">Loading files…</p>;
  return <div className="rm-repo-files">
    <Input aria-label={`Filter files of ${repository.name}`} onChange={(event) => setFilter(event.target.value)} placeholder="Filter by path" value={filter} />
    <ul>
      {matching.slice(0, FILE_LIST_LIMIT).map((file) => <li key={file.artifact_id}>
        <button className="rm-map-link" onClick={() => onOpenArtifact(file.artifact_id)} type="button">{file.path}</button>
        <span className="rm-map-muted">{file.language ?? ""}</span>
        {file.indexed ? null : <span className="rm-map-muted" title={file.index_failure ?? undefined}>{file.index_failure ? "Not indexed" : "Indexing…"}</span>}
      </li>)}
    </ul>
    <span className="rm-map-muted">{matching.length > FILE_LIST_LIMIT ? `Showing ${FILE_LIST_LIMIT} of ${matching.length.toLocaleString()} files. Filter to narrow the list.` : `${matching.length.toLocaleString()} ${matching.length === 1 ? "file" : "files"}`}</span>
  </div>;
}
