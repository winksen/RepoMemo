import type { FormEvent } from "react";
import { SafeMarkdown } from "./SafeMarkdown";
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
import {
  cancelSharedJob,
  checkSharedRepository,
  clearSharedRepositoryToken,
  createSharedRepositoryWebhook,
  connectSharedRepository,
  deleteSharedRepository,
  deleteSharedRepositoryWebhook,
  getSharedRepositoryDetail,
  listSharedRepositories,
  listSharedRepositoryFiles,
  setSharedRepositoryDeployKey,
  setSharedRepositoryToken,
  summarizeSharedRepository,
  syncSharedRepository,
  updateSharedRepository,
} from "../lib/sharedApi";
import type { RepoAccessCheck, RepoFile, RepoSource, RepoSyncReport, RepositoryDetailResponse } from "../types";
import { formatBytes, formatTime, lastChangeLabel } from "../web/format";
import { LanguageChart } from "./LanguageChart";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { CopyButton } from "./ui/copy-button";
import { SearchField } from "./ui/search-field";
import { Table } from "./ui/table";
import { PanelHeading } from "./ui/panel-heading";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Input } from "./ui/input";
import { Textarea } from "./ui/textarea";
import { showToast } from "./ui/toast";
import { useLiveEvents } from "../lib/liveEvents";
import type { IndexingJobStatus, LiveEvent } from "../types";
import { ICON_SIZE } from "./ui/icon";

const POLL_INTERVAL_MS = 1500;
/** With the workspace event stream open, polling is only a safety net. */
const LIVE_FALLBACK_POLL_MS = 15_000;
/** Activity that changes the list of repositories (syncs arrive as job events). */
const REPOSITORY_LIST_ACTIONS = new Set(["repository_connected", "repository_updated", "repository_removed"]);
/** Files listed at once; a filter narrows larger repositories. */
const FILE_LIST_LIMIT = 300;

const STAGE_LABEL: Record<string, string> = {
  queued: "Waiting for another sync to finish",
  fetching_repository: "Fetching from the host",
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

type RepoForm = {
  link: string; name: string; branch: string; include: string; exclude: string;
  token: string; tokenUsername: string; deployKey: string; hostKey: string;
};

const EMPTY_FORM: RepoForm = { link: "", name: "", branch: "", include: "", exclude: "", token: "", tokenUsername: "", deployKey: "", hostKey: "" };

/** An ssh address: `ssh://…` or the `git@host:owner/repo` form. */
function isSshLink(link: string) {
  return /^ssh:\/\//i.test(link.trim()) || /^[^\s\\/]+@[^\s/]+:/.test(link.trim());
}

/** An address of a repository on a host, as opposed to a folder on the server. */
function isRemoteLink(link: string) {
  return /^(https?|ssh|git):\/\//i.test(link.trim()) || /^[^\s\\/]+@[^\s/]+:/.test(link.trim());
}

/** What the server is given for a token: nothing when none was typed. */
function formToken(form: RepoForm) {
  if (isSshLink(form.link)) {
    const deployKey = form.deployKey.trim();
    return deployKey ? { deploy_key: deployKey, host_key: form.hostKey.trim() || undefined } : {};
  }
  const token = form.token.trim();
  return token ? { token, token_username: form.tokenUsername.trim() || undefined } : {};
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

function isRunning(job: IndexingJobStatus) {
  return job.status === "running" || job.status === "pending";
}

/** The repository sync job an event is about, if any. */
function repoSyncJob(event: LiveEvent): IndexingJobStatus | null {
  return event.type === "job" && event.job.kind === "repo_sync" && event.job.source_id ? event.job : null;
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

/** Loads the workspace's repositories, follows each sync's progress from the
 *  workspace event stream (polling while the stream is down), and announces
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
      {canManage ? <Button onClick={onOpenSettings} type="button" variant="secondary"><Settings size={ICON_SIZE.sm} /> Manage links in Settings</Button> : null}
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
      {canManage ? <Button onClick={onOpenSettings} type="button" variant="main"><Plus size={ICON_SIZE.sm} /> Add a repository link</Button> : null}
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
  // Shown once, when a webhook is made or renewed.
  const [webhookSecret, setWebhookSecret] = useState<{ secret: string; address: string } | null>(null);

  const formKey = JSON.stringify([form.link.trim(), formSettings(form), form.token, form.tokenUsername, form.deployKey, form.hostKey]);
  const checkIsCurrent = check !== null && checkedFor === formKey;
  // The repository being edited as the list holds it now (a webhook made in
  // the dialog changes it).
  const liveEditing = editing ? (repositories?.find((item) => item.id === editing.id) ?? editing) : null;

  async function runCheck(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      const result = await checkSharedRepository(accessToken, workspaceId, { link: form.link.trim(), ...formToken(form), ...formSettings(form) });
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
        ...formToken(form),
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
    setWebhookSecret(null);
    setEditForm({
      link: repository.root_path,
      name: repository.name,
      branch: repository.settings.branch ?? "",
      include: repository.settings.include.join("\n"),
      exclude: repository.settings.exclude.join("\n"),
      token: "",
      tokenUsername: "",
      deployKey: "",
      hostKey: "",
    });
    setEditing(repository);
  }

  async function saveEdit(event: FormEvent) {
    event.preventDefault();
    if (!editing) return;
    setBusy(true);
    try {
      if (editing.remote && isSshLink(editing.root_path)) {
        if (editForm.deployKey.trim() || editForm.hostKey.trim()) {
          await setSharedRepositoryDeployKey(accessToken, editing.id, { deploy_key: editForm.deployKey.trim(), host_key: editForm.hostKey.trim() || undefined });
        }
      } else if (editing.remote && editForm.token.trim()) {
        await setSharedRepositoryToken(accessToken, editing.id, { token: editForm.token.trim(), username: editForm.tokenUsername.trim() || undefined });
      }
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

  async function makeWebhook(repository: RepoSource) {
    setBusy(true);
    try {
      const made = await createSharedRepositoryWebhook(accessToken, repository.id);
      setWebhookSecret({ secret: made.secret, address: made.url ?? `${window.location.origin}${made.path}` });
      showToast("success", "The webhook secret is shown below. Copy it now: it is not shown again.");
      await load();
    } catch (error) {
      showToast("error", errorMessage(error, "The webhook could not be set up."));
    } finally {
      setBusy(false);
    }
  }

  async function removeWebhook(repository: RepoSource) {
    setBusy(true);
    try {
      await deleteSharedRepositoryWebhook(accessToken, repository.id);
      setWebhookSecret(null);
      showToast("success", "Push webhooks were turned off for this repository.");
      await load();
    } catch (error) {
      showToast("error", errorMessage(error, "The webhook could not be removed."));
    } finally {
      setBusy(false);
    }
  }

  async function removeToken(repository: RepoSource) {
    setBusy(true);
    try {
      await clearSharedRepositoryToken(accessToken, repository.id);
      setEditing(null);
      showToast("success", `The token of ${repository.name} was removed. It is fetched without signing in from the next sync.`);
      await load();
    } catch (error) {
      showToast("error", errorMessage(error, "The token could not be removed."));
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
    <PanelHeading description={<>{repositories?.length ?? 0} linked</>} icon={GitBranch} title="Repositories" />
    <p className="shared-muted-copy">Link git repositories to this workspace. RepoMemo reads what a branch has committed, never uncommitted changes or ignored files, and keeps every file in step with each sync. A link is a folder on the server; the server must be able to read it.</p>

    {repositories?.length ? <ul className="rm-repo-links">
      {repositories.map((repository) => <li key={repository.id}>
        <div>
          <strong>{repository.name}</strong>
          <code className="rm-repo-path">{repository.root_path}</code>
          <span className="rm-map-muted">{repository.remote ? (isSshLink(repository.root_path) ? "Remote · deploy key · " : repository.has_credential ? "Remote · with a token · " : "Remote · no token · ") : ""}{repository.settings.branch ? `Branch ${repository.settings.branch}` : repository.remote ? "Follows the default branch" : "Follows the checked-out branch"}{repository.settings.include.length ? ` · ${repository.settings.include.length} include` : ""}{repository.settings.exclude.length ? ` · ${repository.settings.exclude.length} exclude` : ""} · {STATUS_LABEL[repository.active_job ? "syncing" : repository.status]}</span>
        </div>
        <div className="rm-health-actions">
          <Button disabled={Boolean(repository.active_job)} onClick={() => startEditing(repository)} type="button" variant="secondary"><Pencil size={ICON_SIZE.sm} /> Edit</Button>
          <Button disabled={Boolean(repository.active_job)} onClick={() => setRemoving(repository)} type="button" variant="secondary"><Trash size={ICON_SIZE.sm} /> Remove</Button>
        </div>
      </li>)}
    </ul> : null}

    <form className="rm-repo-form" onSubmit={runCheck}>
      <h3>Add a repository link</h3>
      <RepositoryFields form={form} onChange={setForm} showLink />
      {checkIsCurrent && check ? <RepositoryCheckResult check={check} /> : null}
      <div className="rm-health-actions">
        <Button disabled={busy || !form.link.trim()} type="submit" variant={checkIsCurrent ? "secondary" : "main"}><Refresh size={ICON_SIZE.sm} /> {checkIsCurrent ? "Check again" : "Check access"}</Button>
        <Button disabled={busy || !checkIsCurrent || check?.already_connected} onClick={() => void connect()} type="button" variant={checkIsCurrent ? "main" : "secondary"}><Plus size={ICON_SIZE.sm} /> Link and sync</Button>
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
        <RepositoryFields form={{ ...editForm, link: editing.root_path }} onChange={(next) => setEditForm({ ...next, link: editForm.link })} remote={editing.remote} replacingToken showLink={false} />
        <div className="rm-repo-webhook">
          <strong>Push webhooks</strong>
          <span className="rm-map-muted">
            {liveEditing?.has_webhook
              ? `On${liveEditing?.webhook_last_received_at ? ` · last call ${formatTime(liveEditing?.webhook_last_received_at)}` : " · no call yet"}.`
              : "Off. The repository is checked for new commits on a schedule."}
            {" "}GitHub, GitLab or any sender that signs with HMAC-SHA256 (<code>X-RepoMemo-Signature</code>) can ask for a sync as soon as someone pushes.
          </span>
          {webhookSecret ? <div className="rm-repo-check">
            <dl className="rm-repo-facts">
              <div><dt>Payload URL</dt><dd><code className="rm-repo-path">{webhookSecret.address}</code> <CopyButton label="Copy the payload URL" value={webhookSecret.address} /></dd></div>
              <div><dt>Secret</dt><dd><code className="rm-repo-path">{webhookSecret.secret}</code> <CopyButton label="Copy the secret" value={webhookSecret.secret} /></dd></div>
            </dl>
            <span className="rm-map-muted">Content type JSON, push events only. Shown once: renewing it ends the old one.</span>
          </div> : null}
          <div className="rm-health-actions">
            <Button disabled={busy} onClick={() => void makeWebhook(editing)} type="button" variant="secondary">{liveEditing?.has_webhook ? "Make a new secret" : "Set up a webhook"}</Button>
            {liveEditing?.has_webhook ? <Button disabled={busy} onClick={() => void removeWebhook(editing)} type="button" variant="secondary">Turn off</Button> : null}
          </div>
        </div>
        {editing.remote && editing.has_credential && !isSshLink(editing.root_path) ? <div><Button disabled={busy} onClick={() => void removeToken(editing)} type="button" variant="secondary">Remove the token</Button></div> : null}
      </form>
    </Dialog> : null}

    {removing ? <Dialog
      description={<>Remove <strong>{removing.name}</strong> and its {removing.file_count.toLocaleString()} files from the workspace? Their index, comments and memory-card links are deleted too. The repository on disk is not touched.</>}
      footer={<><DialogCancel onClick={() => setRemoving(null)} /><Button disabled={busy} onClick={() => void remove(removing)} type="button" variant="secondary">Remove repository</Button></>}
      onClose={() => setRemoving(null)}
      open
      title="Remove repository"
    /> : null}
  </section>;
}

function RepositoryCheckResult({ check }: { check: RepoAccessCheck }) {
  return <div className="rm-repo-check">
    <strong><CircleCheck size={ICON_SIZE.md} /> {check.remote ? "The host answered: the server can reach this repository" : "The server can read this repository"}</strong>
    <dl className="rm-repo-facts">
      <div><dt>Repository</dt><dd>{check.name} <code className="rm-repo-path">{check.root_path}</code></dd></div>
      <div><dt><GitBranch size={ICON_SIZE.sm} /> Branch</dt><dd>{check.commit.branch ?? <span className="rm-map-muted">Detached HEAD</span>}</dd></div>
      {check.remote
        ? <div><dt><GitCommit size={ICON_SIZE.sm} /> Commit</dt><dd><code>{check.commit.sha.slice(0, 7)}</code> <span className="rm-map-muted">· the files are counted after the first sync, which clones the repository</span></dd></div>
        : <>
          <div><dt><GitCommit size={ICON_SIZE.sm} /> Commit</dt><dd><code>{check.commit.sha.slice(0, 7)}</code> {check.commit.summary} <span className="rm-map-muted">· {check.commit.author_name} · {formatTime(check.commit.committed_at)}</span></dd></div>
          <div><dt>Files</dt><dd>About {check.indexable_files.toLocaleString()} of {check.tracked_files.toLocaleString()} committed files will be indexed</dd></div>
        </>}
    </dl>
    {check.already_connected ? <span className="rm-map-muted">This repository is already linked to the workspace.</span> : null}
  </div>;
}

function RepositoryFields({
  form,
  onChange,
  showLink,
  remote = false,
  replacingToken = false,
}: {
  form: RepoForm;
  onChange: (form: RepoForm) => void;
  showLink: boolean;
  /** The repository is a remote one (an existing link, in the edit form). */
  remote?: boolean;
  replacingToken?: boolean;
}) {
  const set = (key: keyof RepoForm) => (event: { target: { value: string } }) => onChange({ ...form, [key]: event.target.value });
  const sshLink = showLink ? isSshLink(form.link) : replacingToken && isSshLink(form.link);
  const asksForToken = showLink ? isRemoteLink(form.link) && !sshLink : remote && !sshLink;
  return <>
    {showLink ? <label>Repository link
      <Input onChange={set("link")} placeholder="C:\code\my-repo, /srv/code/my-repo or https://github.com/acme/app.git" value={form.link} />
      <span className="rm-map-muted">A folder on the server, anywhere inside the repository, or the https address of a repository on a host the server operator allows.</span>
    </label> : null}
    {sshLink ? <>
      <label>{replacingToken ? "New deploy key" : "Deploy key"}
        <Textarea autoComplete="off" onChange={set("deployKey")} placeholder={replacingToken ? "Leave empty to keep the current key" : "-----BEGIN OPENSSH PRIVATE KEY-----"} rows={4} value={form.deployKey} />
        <span className="rm-map-muted">A read-only deploy key without a passphrase. It is stored encrypted and never shown again.</span>
      </label>
      <label>Host key
        <Textarea autoComplete="off" onChange={set("hostKey")} placeholder="ssh-ed25519 AAAA…" rows={2} value={form.hostKey} />
        <span className="rm-map-muted">The host's public key, as printed by <code>ssh-keyscan -t ed25519 host</code>. The connection only goes through to a host that presents exactly this key.</span>
      </label>
    </> : null}
    {asksForToken ? <div className="rm-repo-form-row">
      <label>{replacingToken ? "New access token" : "Access token"}
        <Input autoComplete="off" onChange={set("token")} placeholder={replacingToken ? "Leave empty to keep the current token" : "Only for a private repository"} type="password" value={form.token} />
        <span className="rm-map-muted">A read-only token. It is stored encrypted and never shown again.</span>
      </label>
      <label>Token account name
        <Input autoComplete="off" onChange={set("tokenUsername")} placeholder="x-access-token" value={form.tokenUsername} />
        <span className="rm-map-muted">Optional; <code>oauth2</code> for GitLab.</span>
      </label>
    </div> : null}
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
      {job ? (canSync ? <Button disabled={job.cancel_requested} onClick={onStop} type="button" variant="secondary">{job.cancel_requested ? "Stopping…" : "Stop sync"}</Button> : null)
        : canSync ? <Button onClick={onSync} type="button" variant="secondary"><Refresh size={ICON_SIZE.sm} /> Sync now</Button> : null}
      {repository.overview_artifact_id ? <Button onClick={() => onOpenArtifact(repository.overview_artifact_id!)} type="button" variant="secondary"><FolderCode size={ICON_SIZE.sm} /> Open overview</Button> : null}
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
    <SearchField aria-label={`Filter files of ${repository.name}`} onValueChange={setFilter} placeholder="Filter by path" value={filter} />
    <ul>
      {matching.slice(0, FILE_LIST_LIMIT).map((file) => <li key={file.artifact_id}>
        <button className="rm-map-link" onClick={() => onOpenArtifact(file.artifact_id)} type="button">{file.path}</button>
        {file.last_author ? <span className="rm-map-muted"> · {lastChangeLabel(file.last_author, file.last_changed_at)}</span> : null}
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
  const live = useLiveEvents((event) => {
    if (event.type === "resync") { void load(); return; }
    const job = repoSyncJob(event);
    if (!job || job.source_id !== repositoryId) return;
    if (!isRunning(job)) { void load(); return; }
    runningJob.current = job.id;
    setDetail((current) => current ? { ...current, repository: { ...current.repository, active_job: job } } : current);
  });
  const syncing = Boolean(detail?.repository.active_job);
  useEffect(() => {
    if (!syncing) return;
    const timer = window.setInterval(() => void load(), live ? LIVE_FALLBACK_POLL_MS : POLL_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [syncing, live, accessToken, repositoryId]);

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
      <PanelHeading actions={<span className={`rm-repo-status status-${status}`}>{STATUS_LABEL[status]}</span>} icon={GitBranch} title="Repository" />
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
          ? <Button disabled={job.cancel_requested} onClick={() => void stop()} type="button" variant="secondary">{job.cancel_requested ? "Stopping…" : "Stop sync"}</Button>
          : <Button onClick={() => void sync()} type="button" variant="secondary"><Refresh size={ICON_SIZE.sm} /> Sync now</Button> : null}
        <Button aria-expanded={filesOpen} disabled={!repository.file_count} onClick={() => setFilesOpen((open) => !open)} type="button" variant="secondary">{filesOpen ? "Hide files" : "Browse files"}</Button>
      </div>
      {filesOpen ? <RepositoryFiles accessToken={accessToken} onOpenArtifact={onOpenArtifact} repository={repository} /> : null}
    </section>

    <section className="shared-record-panel rm-repo-page">
      <PanelHeading description={summary ? summary.provider_name : null} icon={Sparkles} title="Summary" />
      {summary ? <>
        <div className="shared-markdown rm-repo-summary"><SafeMarkdown>{summary.summary_markdown}</SafeMarkdown></div>
        <p className="rm-map-muted">Written for commit <code>{summary.commit_sha.slice(0, 7)}</code> on {formatTime(summary.generated_at)}.{summaryIsStale ? " The repository has changed since; regenerate it to describe the current commit." : ""} {summary.warnings.join(" ")}</p>
        {citedFiles.length ? <div className="rm-repo-cited"><span className="rm-map-muted">Based on</span>{citedFiles.map(([artifactId, path]) => <button className="rm-map-link" key={artifactId} onClick={() => onOpenArtifact(artifactId)} type="button">{path}</button>)}</div> : null}
      </> : <p className="shared-muted-copy">{detail.ai_available ? "No summary yet. A summary describes what the repository is, its main parts and where to start reading, from the overview below and the opening of its key files." : "A summary needs a text AI provider, which an administrator can set up in Settings. The overview below needs no AI."}</p>}
      {canSync && detail.ai_available && overview ? <div className="rm-health-actions"><Button disabled={summarizing} onClick={() => void summarize()} type="button" variant={summary && !summaryIsStale ? "secondary" : "main"}><Sparkles size={ICON_SIZE.sm} /> {summarizing ? "Summarizing…" : summary ? "Regenerate summary" : "Generate summary"}</Button></div> : null}
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
