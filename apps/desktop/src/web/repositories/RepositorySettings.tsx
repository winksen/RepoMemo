import type { FormEvent } from "react";
import { useState } from "react";
import { IconGitBranch as GitBranch, IconPencil as Pencil, IconPlus as Plus, IconRefresh as Refresh, IconSettings as Settings, IconTrash as Trash } from "@tabler/icons-react";
import {
  checkSharedRepository,
  clearSharedRepositoryToken,
  createSharedRepositoryWebhook,
  connectSharedRepository,
  deleteSharedRepository,
  deleteSharedRepositoryWebhook,
  setSharedRepositoryDeployKey,
  setSharedRepositoryToken,
  updateSharedRepository,
} from "../../lib/sharedApi";
import type { RepoAccessCheck, RepoSource } from "../../types";
import { apiMessage, formatTime } from "../format";
import { Button } from "../../components/ui/button";
import { CopyButton } from "../../components/ui/copy-button";
import { PanelHeading } from "../../components/ui/panel-heading";
import { Dialog, DialogCancel } from "../../components/ui/dialog";
import { showToast } from "../../components/ui/toast";
import { ICON_SIZE } from "../../components/ui/icon";
import { STATUS_LABEL, RepoForm, EMPTY_FORM, isSshLink, formToken, formSettings } from "./shared";
import { useRepositories } from "./useRepositories";
import { RepositoryCheckResult, RepositoryFields } from "./LinkForm";

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
      showToast("error", apiMessage(error, "The repository link could not be checked."));
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
      showToast("error", apiMessage(error, "The repository could not be linked."));
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
      showToast("error", apiMessage(error, "The settings could not be saved."));
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
      showToast("error", apiMessage(error, "The webhook could not be set up."));
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
      showToast("error", apiMessage(error, "The webhook could not be removed."));
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
      showToast("error", apiMessage(error, "The token could not be removed."));
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
      showToast("error", apiMessage(error, "The repository could not be removed."));
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
