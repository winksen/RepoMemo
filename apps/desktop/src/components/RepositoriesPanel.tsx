import type { FormEvent } from "react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  IconCircleCheck as CircleCheck,
  IconFolderCode as FolderCode,
  IconGitBranch as GitBranch,
  IconGitCommit as GitCommit,
  IconPencil as Pencil,
  IconPlus as Plus,
  IconRefresh as Refresh,
  IconSettings as Settings,
  IconSparkles as Sparkles,
  IconStack2 as Layers,
  IconTrash as Trash,
} from "@tabler/icons-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  cancelSharedJob,
  checkSharedRepository,
  connectSharedRepository,
  deleteSharedRepository,
  getSharedRepositoryDetail,
  listSharedRepositories,
  listSharedRepositoryFiles,
  summarizeSharedRepository,
  syncSharedRepository,
  updateSharedRepository,
} from "../lib/sharedApi";
import type { RepoAccessCheck, RepoFile, RepoSource, RepoSyncReport, RepositoryDetailResponse } from "../types";
import { LanguageChart } from "./LanguageChart";
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

type RepoForm = { link: string; name: string; branch: string; include: string; exclude: string };

const EMPTY_FORM: RepoForm = { link: "", name: "", branch: "", include: "", exclude: "" };

function formatDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

function lines(value: string) {
  return value.split("\n").map((line) => line.trim()).filter(Boolean);
}

function formSettings(form: RepoForm) {
  return { branch: form.branch.trim() || null, include: lines(form.include), exclude: lines(form.exclude) };
}

function errorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
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

/** Loads the workspace's repositories, polls while one syncs, and announces
 *  each sync that finishes. */
function useRepositories(accessToken: string, workspaceId: string, onChanged: () => void) {
  const [repositories, setRepositories] = useState<RepoSource[] | null>(null);
  // Jobs seen running, so the end of a sync can be announced once.
  const runningJobs = useRef(new Map<string, string>());

  async function load() {
    try {
      const next = await listSharedRepositories(accessToken, workspaceId);
      announceFinishedSyncs(next.repositories);
      setRepositories(next.repositories);
    } catch (error) {
      showToast("error", errorMessage(error, "Repositories could not be loaded."));
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

  const syncing = Boolean(repositories?.some((repository) => repository.active_job));
  useEffect(() => {
    if (!syncing) return;
    const timer = window.setInterval(() => void load(), POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [syncing, accessToken, workspaceId]);

  return { repositories, load, track };
}

/** The Repositories section: each linked repository's sync state, its last
 *  sync, and the files it keeps in step with the repository. Links are added
 *  and changed in workspace Settings. */
export function RepositoriesPanel({
  accessToken,
  canManage,
  canSync,
  onChanged,
  onOpenArtifact,
  onOpenSettings,
  workspaceId,
}: {
  accessToken: string;
  canManage: boolean;
  canSync: boolean;
  onChanged: () => void;
  onOpenArtifact: (artifactId: string) => void;
  onOpenSettings: () => void;
  workspaceId: string;
}) {
  const { repositories, load, track } = useRepositories(accessToken, workspaceId, onChanged);
  const [openFilesId, setOpenFilesId] = useState<string | null>(null);

  async function sync(repository: RepoSource) {
    try {
      const { job } = await syncSharedRepository(accessToken, repository.id);
      track(repository.id, job?.id);
      await load();
    } catch (error) {
      showToast("error", errorMessage(error, "The sync could not be started."));
    }
  }

  async function stop(repository: RepoSource) {
    if (!repository.active_job) return;
    try {
      await cancelSharedJob(accessToken, repository.active_job.id);
      await load();
    } catch (error) {
      showToast("error", errorMessage(error, "The sync could not be stopped."));
    }
  }

  if (!repositories) {
    return <section className="rm-repos" aria-busy="true"><p className="shared-muted-copy">Loading repositories…</p></section>;
  }

  const count = repositories.length;
  return <section className="rm-repos">
    <div className="rm-map-toolbar">
      <span>{count ? `${count} ${count === 1 ? "repository" : "repositories"}` : "No repositories linked"}</span>
      {canManage ? <Button onClick={onOpenSettings} type="button" variant="secondary"><Settings size={15} /> Manage links in Settings</Button> : null}
    </div>

    {count ? <ul className="rm-repo-list">
      {repositories.map((repository) => <RepositoryCard
        accessToken={accessToken}
        canSync={canSync}
        filesOpen={openFilesId === repository.id}
        key={repository.id}
        onOpenArtifact={onOpenArtifact}
        onStop={() => void stop(repository)}
        onSync={() => void sync(repository)}
        onToggleFiles={() => setOpenFilesId((current) => current === repository.id ? null : repository.id)}
        repository={repository}
      />)}
    </ul> : <div className="shared-empty-state">
      <FolderCode size={25} />
      <strong>No repositories yet</strong>
      <span>{canManage ? "Add a repository link in this workspace's Settings. Its committed files are indexed and kept in step with each sync." : "Owners and administrators can link a git repository to this workspace in Settings."}</span>
      {canManage ? <Button onClick={onOpenSettings} type="button" variant="main"><Plus size={15} /> Add a repository link</Button> : null}
    </div>}
  </section>;
}

/** Workspace Settings › Repositories: the workspace's repository links. A
 *  link is checked first, so an administrator sees whether the server can
 *  read it and what it would index before anything is stored. */
export function RepositorySettings({
  accessToken,
  onChanged,
  workspaceId,
}: {
  accessToken: string;
  onChanged: () => void;
  workspaceId: string;
}) {
  const { repositories, load, track } = useRepositories(accessToken, workspaceId, onChanged);
  const [form, setForm] = useState<RepoForm>(EMPTY_FORM);
  const [check, setCheck] = useState<RepoAccessCheck | null>(null);
  // The form values the check was run with; any change asks for a new check.
  const [checkedFor, setCheckedFor] = useState("");
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState<RepoSource | null>(null);
  const [editForm, setEditForm] = useState<RepoForm>(EMPTY_FORM);
  const [removing, setRemoving] = useState<RepoSource | null>(null);

  const formKey = JSON.stringify([form.link.trim(), formSettings(form)]);
  const checkIsCurrent = check !== null && checkedFor === formKey;

  async function runCheck(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      const result = await checkSharedRepository(accessToken, workspaceId, { link: form.link.trim(), ...formSettings(form) });
      setCheck(result);
      setCheckedFor(formKey);
    } catch (error) {
      setCheck(null);
      showToast("error", errorMessage(error, "The repository link could not be checked."));
    } finally {
      setBusy(false);
    }
  }

  async function connect() {
    setBusy(true);
    try {
      const { repository, job } = await connectSharedRepository(accessToken, workspaceId, {
        link: form.link.trim(),
        name: form.name.trim() || undefined,
        ...formSettings(form),
      });
      track(repository.id, job?.id);
      setForm(EMPTY_FORM);
      setCheck(null);
      showToast("success", `Linked ${repository.name}. The first sync has started.`);
      await load();
    } catch (error) {
      showToast("error", errorMessage(error, "The repository could not be linked."));
    } finally {
      setBusy(false);
    }
  }

  function startEditing(repository: RepoSource) {
    setEditForm({
      link: repository.root_path,
      name: repository.name,
      branch: repository.settings.branch ?? "",
      include: repository.settings.include.join("\n"),
      exclude: repository.settings.exclude.join("\n"),
    });
    setEditing(repository);
  }

  async function saveEdit(event: FormEvent) {
    event.preventDefault();
    if (!editing) return;
    setBusy(true);
    try {
      await updateSharedRepository(accessToken, editing.id, { name: editForm.name.trim() || undefined, ...formSettings(editForm) });
      setEditing(null);
      showToast("success", "Settings saved. They apply from the next sync.");
      await load();
    } catch (error) {
      showToast("error", errorMessage(error, "The settings could not be saved."));
    } finally {
      setBusy(false);
    }
  }

  async function remove(repository: RepoSource) {
    setBusy(true);
    try {
      await deleteSharedRepository(accessToken, repository.id);
      setRemoving(null);
      showToast("success", `Removed ${repository.name} and its files.`);
      await load();
      onChanged();
    } catch (error) {
      showToast("error", errorMessage(error, "The repository could not be removed."));
    } finally {
      setBusy(false);
    }
  }

  return <section className="shared-settings-group rm-repo-settings">
    <div className="shared-panel-heading"><div><GitBranch size={18} /><h2>Repositories</h2></div><span>{repositories?.length ?? 0} linked</span></div>
    <p className="shared-muted-copy">Link git repositories to this workspace. RepoMemo reads what a branch has committed, never uncommitted changes or ignored files, and keeps every file in step with each sync. A link is a folder on the server; the server must be able to read it.</p>

    {repositories?.length ? <ul className="rm-repo-links">
      {repositories.map((repository) => <li key={repository.id}>
        <div>
          <strong>{repository.name}</strong>
          <code className="rm-repo-path">{repository.root_path}</code>
          <span className="rm-map-muted">{repository.settings.branch ? `Branch ${repository.settings.branch}` : "Follows the checked-out branch"}{repository.settings.include.length ? ` · ${repository.settings.include.length} include` : ""}{repository.settings.exclude.length ? ` · ${repository.settings.exclude.length} exclude` : ""} · {STATUS_LABEL[repository.active_job ? "syncing" : repository.status]}</span>
        </div>
        <div className="rm-health-actions">
          <Button disabled={Boolean(repository.active_job)} onClick={() => startEditing(repository)} type="button" variant="secondary"><Pencil size={15} /> Edit</Button>
          <Button className="rm-button-danger" disabled={Boolean(repository.active_job)} onClick={() => setRemoving(repository)} type="button" variant="secondary"><Trash size={15} /> Remove</Button>
        </div>
      </li>)}
    </ul> : null}

    <form className="rm-repo-form" onSubmit={runCheck}>
      <h3>Add a repository link</h3>
      <RepositoryFields form={form} onChange={setForm} showLink />
      {checkIsCurrent && check ? <RepositoryCheckResult check={check} /> : null}
      <div className="rm-health-actions">
        <Button disabled={busy || !form.link.trim()} type="submit" variant={checkIsCurrent ? "secondary" : "main"}><Refresh size={15} /> {checkIsCurrent ? "Check again" : "Check access"}</Button>
        <Button disabled={busy || !checkIsCurrent || check?.already_connected} onClick={() => void connect()} type="button" variant={checkIsCurrent ? "main" : "secondary"}><Plus size={15} /> Link and sync</Button>
      </div>
    </form>

    {editing ? <Dialog
      className="rm-dialog-wide"
      description={<>Changes apply from the next sync. Files no longer matched leave search; files newly matched are added.</>}
      footer={<><DialogCancel onClick={() => setEditing(null)} /><Button disabled={busy} form="rm-repo-edit-form" type="submit" variant="main">Save</Button></>}
      onClose={() => setEditing(null)}
      open
      title={`Edit ${editing.name}`}
    >
      <form className="rm-repo-form" id="rm-repo-edit-form" onSubmit={saveEdit}>
        <RepositoryFields form={editForm} onChange={setEditForm} showLink={false} />
      </form>
    </Dialog> : null}

    {removing ? <Dialog
      description={<>Remove <strong>{removing.name}</strong> and its {removing.file_count.toLocaleString()} files from the workspace? Their index, comments and memory-card links are deleted too. The repository on disk is not touched.</>}
      footer={<><DialogCancel onClick={() => setRemoving(null)} /><Button className="rm-button-danger" disabled={busy} onClick={() => void remove(removing)} type="button" variant="secondary">Remove repository</Button></>}
      onClose={() => setRemoving(null)}
      open
      title="Remove repository"
    /> : null}
  </section>;
}

function RepositoryCheckResult({ check }: { check: RepoAccessCheck }) {
  return <div className="rm-repo-check">
    <strong><CircleCheck size={16} /> The server can read this repository</strong>
    <dl className="rm-repo-facts">
      <div><dt>Repository</dt><dd>{check.name} <code className="rm-repo-path">{check.root_path}</code></dd></div>
      <div><dt><GitBranch size={14} /> Branch</dt><dd>{check.commit.branch ?? <span className="rm-map-muted">Detached HEAD</span>}</dd></div>
      <div><dt><GitCommit size={14} /> Commit</dt><dd><code>{check.commit.sha.slice(0, 7)}</code> {check.commit.summary} <span className="rm-map-muted">· {check.commit.author_name} · {formatDate(check.commit.committed_at)}</span></dd></div>
      <div><dt>Files</dt><dd>About {check.indexable_files.toLocaleString()} of {check.tracked_files.toLocaleString()} committed files will be indexed</dd></div>
    </dl>
    {check.already_connected ? <span className="rm-map-muted">This repository is already linked to the workspace.</span> : null}
  </div>;
}

function RepositoryFields({
  form,
  onChange,
  showLink,
}: {
  form: RepoForm;
  onChange: (form: RepoForm) => void;
  showLink: boolean;
}) {
  const set = (key: keyof RepoForm) => (event: { target: { value: string } }) => onChange({ ...form, [key]: event.target.value });
  return <>
    {showLink ? <label>Repository link
      <Input onChange={set("link")} placeholder="C:\code\my-repo or /srv/code/my-repo" value={form.link} />
      <span className="rm-map-muted">A folder on the server, anywhere inside the repository. Remote URLs are not supported yet.</span>
    </label> : null}
    <div className="rm-repo-form-row">
      <label>Name<Input onChange={set("name")} placeholder={showLink ? "The folder name" : ""} value={form.name} /></label>
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
  </>;
}

function RepositoryCard({
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
      <span className={`rm-repo-status status-${status}`}>{STATUS_LABEL[status]}</span>
    </div>

    <dl className="rm-repo-facts">
      <div><dt><GitBranch size={14} /> Branch</dt><dd>{commit?.branch ?? settings.branch ?? "Checked-out branch"}{settings.branch ? null : <span className="rm-map-muted"> (follows HEAD)</span>}</dd></div>
      <div><dt><GitCommit size={14} /> Commit</dt><dd>{commit ? <><code>{commit.sha.slice(0, 7)}</code> {commit.summary} <span className="rm-map-muted">· {commit.author_name} · {formatDate(commit.committed_at)}</span></> : <span className="rm-map-muted">Not synced yet</span>}</dd></div>
      <div><dt>Files</dt><dd>{repository.indexed_file_count === repository.file_count ? repository.file_count.toLocaleString() : `${repository.indexed_file_count.toLocaleString()} of ${repository.file_count.toLocaleString()} indexed`}{repository.last_synced_at ? <span className="rm-map-muted"> · synced {formatDate(repository.last_synced_at)}</span> : null}</dd></div>
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
      {job ? (canSync ? <Button disabled={job.cancel_requested} onClick={onStop} type="button" variant="secondary">{job.cancel_requested ? "Stopping…" : "Stop sync"}</Button> : null)
        : canSync ? <Button onClick={onSync} type="button" variant="secondary"><Refresh size={15} /> Sync now</Button> : null}
      {repository.overview_artifact_id ? <Button onClick={() => onOpenArtifact(repository.overview_artifact_id!)} type="button" variant="secondary"><FolderCode size={15} /> Open overview</Button> : null}
      <Button aria-expanded={filesOpen} disabled={!repository.file_count} onClick={onToggleFiles} type="button" variant="secondary">{filesOpen ? "Hide files" : "Browse files"}</Button>
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
      .catch((error) => showToast("error", errorMessage(error, "The files could not be loaded.")));
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

/** Order and labels of the technology groups in the project analysis. */
const STACK_CATEGORIES: [string, string][] = [
  ["language", "Language"], ["meta-framework", "Framework"], ["frontend", "Frontend"], ["backend", "Backend"],
  ["desktop", "Desktop"], ["mobile", "Mobile"], ["cli", "Command line"], ["game", "Game"], ["data", "Data"],
  ["database", "Database"], ["runtime", "Runtime"], ["testing", "Testing"], ["tooling", "Tooling"],
];

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${bytes} bytes`;
}

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
  const [detail, setDetail] = useState<RepositoryDetailResponse | null>(null);
  const [summarizing, setSummarizing] = useState(false);
  const [filesOpen, setFilesOpen] = useState(false);
  const runningJob = useRef<string | null>(null);

  async function load() {
    try {
      const next = await getSharedRepositoryDetail(accessToken, repositoryId);
      const job = next.repository.active_job;
      if (job) runningJob.current = job.id;
      else if (runningJob.current) {
        runningJob.current = null;
        if (next.repository.status === "error") showToast("error", `${next.repository.name}: ${next.repository.last_error ?? "the sync failed."}`);
        else if (next.repository.last_report) showToast("success", `${next.repository.name} synced. ${reportSummary(next.repository.last_report)}.`);
      }
      setDetail(next);
    } catch (error) {
      showToast("error", errorMessage(error, "The repository could not be loaded."));
    }
  }

  useEffect(() => { setDetail(null); runningJob.current = null; void load(); }, [accessToken, repositoryId]);
  const syncing = Boolean(detail?.repository.active_job);
  useEffect(() => {
    if (!syncing) return;
    const timer = window.setInterval(() => void load(), POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [syncing, accessToken, repositoryId]);

  async function sync() {
    try {
      const { job } = await syncSharedRepository(accessToken, repositoryId);
      if (job) runningJob.current = job.id;
      await load();
    } catch (error) {
      showToast("error", errorMessage(error, "The sync could not be started."));
    }
  }

  async function stop() {
    const job = detail?.repository.active_job;
    if (!job) return;
    try {
      await cancelSharedJob(accessToken, job.id);
      await load();
    } catch (error) {
      showToast("error", errorMessage(error, "The sync could not be stopped."));
    }
  }

  async function summarize() {
    setSummarizing(true);
    try {
      const summary = await summarizeSharedRepository(accessToken, repositoryId);
      setDetail((current) => current ? { ...current, summary } : current);
      showToast("success", "Summary generated.");
    } catch (error) {
      showToast("error", errorMessage(error, "The summary could not be generated."));
    } finally {
      setSummarizing(false);
    }
  }

  if (!detail) return <section className="shared-record-panel" aria-busy="true"><p className="shared-muted-copy">Loading the repository…</p></section>;

  const { repository, overview, summary } = detail;
  const job = repository.active_job;
  const status = job ? "syncing" : repository.status;
  const commit = repository.last_synced_commit;
  const summaryIsStale = Boolean(summary && commit && summary.commit_sha !== commit.sha);
  const citedFiles = summary ? Array.from(new Map(summary.citations.map((citation) => [citation.artifact_id, citation.path])).entries()) : [];
  const progress = job?.progress_total ? Math.min(100, Math.round((job.progress_current / job.progress_total) * 100)) : null;

  return <>
    <section className="shared-record-panel rm-repo-page">
      <div className="shared-panel-heading"><div><GitBranch size={18} /><h2>Repository</h2></div><span className={`rm-repo-status status-${status}`}>{STATUS_LABEL[status]}</span></div>
      <dl className="rm-repo-facts">
        <div><dt>Location</dt><dd><code className="rm-repo-path">{repository.root_path}</code></dd></div>
        <div><dt><GitBranch size={14} /> Branch</dt><dd>{commit?.branch ?? repository.settings.branch ?? "Checked-out branch"}</dd></div>
        <div><dt><GitCommit size={14} /> Commit</dt><dd>{commit ? <><code>{commit.sha.slice(0, 7)}</code> {commit.summary} <span className="rm-map-muted">· {commit.author_name} · {formatDate(commit.committed_at)}</span></> : <span className="rm-map-muted">Not synced yet</span>}</dd></div>
        <div><dt>Files</dt><dd>{repository.file_count.toLocaleString()} indexed{repository.last_synced_at ? <span className="rm-map-muted"> · synced {formatDate(repository.last_synced_at)}</span> : null}</dd></div>
        {!job && repository.status === "error" && repository.last_error ? <div><dt>Last error</dt><dd>{repository.last_error}</dd></div> : null}
      </dl>
      {job ? <div className="rm-repo-progress" role="status">
        <div><span>{STAGE_LABEL[job.stage] ?? job.stage}</span><span>{job.progress_total ? `${job.progress_current.toLocaleString()} / ${job.progress_total.toLocaleString()}` : ""}</span></div>
        <progress aria-label={`Sync progress for ${repository.name}`} max={100} value={progress ?? undefined} />
      </div> : null}
      <div className="rm-health-actions">
        {canSync ? job
          ? <Button disabled={job.cancel_requested} onClick={() => void stop()} type="button" variant="secondary">{job.cancel_requested ? "Stopping…" : "Stop sync"}</Button>
          : <Button onClick={() => void sync()} type="button" variant="secondary"><Refresh size={15} /> Sync now</Button> : null}
        <Button aria-expanded={filesOpen} disabled={!repository.file_count} onClick={() => setFilesOpen((open) => !open)} type="button" variant="secondary">{filesOpen ? "Hide files" : "Browse files"}</Button>
      </div>
      {filesOpen ? <RepositoryFiles accessToken={accessToken} onOpenArtifact={onOpenArtifact} repository={repository} /> : null}
    </section>

    <section className="shared-record-panel rm-repo-page">
      <div className="shared-panel-heading"><div><Sparkles size={18} /><h2>Summary</h2></div>{summary ? <span>{summary.provider_name}</span> : null}</div>
      {summary ? <>
        <div className="shared-markdown rm-repo-summary"><ReactMarkdown remarkPlugins={[remarkGfm]}>{summary.summary_markdown}</ReactMarkdown></div>
        <p className="rm-map-muted">Written for commit <code>{summary.commit_sha.slice(0, 7)}</code> on {formatDate(summary.generated_at)}.{summaryIsStale ? " The repository has changed since; regenerate it to describe the current commit." : ""} {summary.warnings.join(" ")}</p>
        {citedFiles.length ? <div className="rm-repo-cited"><span className="rm-map-muted">Based on</span>{citedFiles.map(([artifactId, path]) => <button className="rm-map-link" key={artifactId} onClick={() => onOpenArtifact(artifactId)} type="button">{path}</button>)}</div> : null}
      </> : <p className="shared-muted-copy">{detail.ai_available ? "No summary yet. A summary describes what the repository is, its main parts and where to start reading, from the overview below and the opening of its key files." : "A summary needs a text AI provider, which an administrator can set up in Settings. The overview below needs no AI."}</p>}
      {canSync && detail.ai_available && overview ? <div className="rm-health-actions"><Button disabled={summarizing} onClick={() => void summarize()} type="button" variant={summary && !summaryIsStale ? "secondary" : "main"}><Sparkles size={15} /> {summarizing ? "Summarizing…" : summary ? "Regenerate summary" : "Generate summary"}</Button></div> : null}
    </section>

    {overview?.stack ? <section className="shared-record-panel rm-repo-page">
      <div className="shared-panel-heading"><div><Layers size={18} /><h2>Project analysis</h2></div><span>detected from manifests</span></div>
      <p className="rm-stack-summary">{overview.stack.summary}</p>
      <dl className="rm-stack-groups">
        {STACK_CATEGORIES.map(([category, label]) => {
          const items = overview.stack!.technologies.filter((technology) => technology.category === category);
          return items.length ? <div key={category}><dt>{label}</dt><dd>{items.map((technology) => <span className="rm-stack-chip" key={technology.name}>{technology.name}</span>)}</dd></div> : null;
        })}
      </dl>
    </section> : null}

    <section className="shared-record-panel rm-repo-page">
      <div className="shared-panel-heading"><div><FolderCode size={18} /><h2>Content overview</h2></div>{overview ? <span>{overview.file_count.toLocaleString()} files · {formatBytes(overview.total_bytes)}</span> : null}</div>
      {overview ? <div className="rm-repo-overview">
        <div className="rm-repo-languages">
          <h3 className="rm-repo-caption">Languages</h3>
          <LanguageChart languages={overview.languages} totalBytes={overview.total_bytes} />
        </div>
        <div className="rm-map-coverage">
          <table>
            <caption className="rm-repo-caption">Structure</caption>
            <thead><tr><th scope="col">Folder</th><th scope="col">Files</th></tr></thead>
            <tbody>{overview.folders.map((folder) => <tr key={folder.path}><th scope="row">{folder.path ? <code>{folder.path}/</code> : "Repository root"}</th><td>{folder.files.toLocaleString()}</td></tr>)}</tbody>
          </table>
        </div>
        {overview.key_files.length ? <div className="rm-repo-block">
          <h3>Key files</h3>
          <ul className="rm-repo-keyfiles">{overview.key_files.map((file) => <li key={file.artifact_id}><button className="rm-map-link" onClick={() => onOpenArtifact(file.artifact_id)} type="button">{file.path}</button><span className="rm-health-fate">{KEY_FILE_ROLE[file.role] ?? file.role}</span></li>)}</ul>
        </div> : null}
        {overview.readme_excerpt ? <div className="rm-repo-block">
          <h3>From the README</h3>
          <div className="shared-markdown rm-repo-readme"><ReactMarkdown remarkPlugins={[remarkGfm]}>{overview.readme_excerpt}</ReactMarkdown></div>
        </div> : null}
        {overview.recent_commits.length ? <div className="rm-repo-block">
          <h3>Recent commits</h3>
          <ul className="rm-repo-commits">{overview.recent_commits.map((entry) => <li key={entry.sha}><code>{entry.sha.slice(0, 7)}</code><span>{entry.summary}</span><span className="rm-map-muted">{entry.author_name} · {formatDate(entry.committed_at)}</span></li>)}</ul>
        </div> : null}
      </div> : <p className="shared-muted-copy">The overview appears after the first complete sync.</p>}
    </section>
  </>;
}
