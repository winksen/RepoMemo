import { type FormEvent, useCallback, useEffect, useState } from "react";
import {
  IconBrandGithub as GitHub,
  IconCircleCheck as CircleCheck,
  IconKey as Key,
  IconPlus as Plus,
  IconRefresh as Refresh,
  IconTrash as Trash,
  IconWebhook as Webhook,
} from "@tabler/icons-react";
import {
  checkSharedConnector,
  connectSharedConnector,
  createSharedConnectorWebhook,
  deleteSharedConnector,
  deleteSharedConnectorWebhook,
  listSharedConnectors,
  setSharedConnectorToken,
  syncSharedConnector,
} from "../lib/sharedApi";
import type { ConnectorAccessCheck, ConnectorSource } from "../types";
import { Button } from "./ui/button";
import { CopyButton } from "./ui/copy-button";
import { PanelHeading } from "./ui/panel-heading";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Dropdown } from "./ui/dropdown";
import { Input } from "./ui/input";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";

function errorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

function formatDate(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

const STATUS_LABEL: Record<string, string> = {
  pending: "Not synced yet",
  syncing: "Syncing",
  ready: "Up to date",
  error: "Sync failed",
  paused: "Paused",
};

/** Workspace Settings › Issues: trackers whose issues are kept as evidence.
 *  A link is checked first, so an administrator sees whether the server can
 *  read it with the token given. */
export function ConnectorSettings({
  accessToken,
  onChanged,
  workspaceId,
}: {
  accessToken: string;
  onChanged: () => void;
  workspaceId: string;
}) {
  const [connectors, setConnectors] = useState<ConnectorSource[] | null>(null);
  const [link, setLink] = useState("");
  // "" lets the address tell GitHub from GitLab.
  const [provider, setProvider] = useState("");
  const [token, setToken] = useState("");
  const [check, setCheck] = useState<ConnectorAccessCheck | null>(null);
  const [checkedFor, setCheckedFor] = useState("");
  const [busy, setBusy] = useState(false);
  const [removing, setRemoving] = useState<ConnectorSource | null>(null);
  const [replacing, setReplacing] = useState<ConnectorSource | null>(null);
  const [newToken, setNewToken] = useState("");
  // The webhook dialog; the secret is shown once, when it is made.
  const [hooking, setHooking] = useState<ConnectorSource | null>(null);
  const [hookSecret, setHookSecret] = useState<{ secret: string; address: string } | null>(null);

  const load = useCallback(async () => {
    try {
      setConnectors((await listSharedConnectors(accessToken, workspaceId)).connectors);
    } catch (error) {
      showToast("error", errorMessage(error, "The connectors could not be loaded."));
      setConnectors((current) => current ?? []);
    }
  }, [accessToken, workspaceId]);

  useEffect(() => {
    void load();
  }, [load]);

  // While a sync runs, follow it; the workspace's evidence changes with it.
  const syncing = connectors?.some((connector) => connector.status === "syncing" || connector.active_job) ?? false;
  useEffect(() => {
    if (!syncing) return undefined;
    const timer = window.setInterval(() => void load(), 1500);
    return () => window.clearInterval(timer);
  }, [syncing, load]);
  useEffect(() => {
    if (connectors && !syncing) onChanged();
    // Only when a sync finishes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [syncing]);

  const formKey = JSON.stringify([link.trim(), provider, token]);
  const checkIsCurrent = check !== null && checkedFor === formKey;

  async function runCheck(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    try {
      const result = await checkSharedConnector(accessToken, workspaceId, { link: link.trim(), provider: provider || undefined, token: token.trim() || undefined });
      setCheck(result);
      setCheckedFor(formKey);
    } catch (error) {
      setCheck(null);
      showToast("error", errorMessage(error, "The tracker could not be checked."));
    } finally {
      setBusy(false);
    }
  }

  async function connect() {
    setBusy(true);
    try {
      const { connector } = await connectSharedConnector(accessToken, workspaceId, { link: link.trim(), provider: provider || undefined, token: token.trim() || undefined });
      setLink("");
      setToken("");
      setCheck(null);
      showToast("success", `Connected ${connector.name}. The first sync has started.`);
      await load();
    } catch (error) {
      showToast("error", errorMessage(error, "The tracker could not be connected."));
    } finally {
      setBusy(false);
    }
  }

  async function sync(connector: ConnectorSource) {
    try {
      await syncSharedConnector(accessToken, connector.id);
      await load();
    } catch (error) {
      showToast("error", errorMessage(error, "The sync could not be started."));
    }
  }

  async function replaceToken(connector: ConnectorSource) {
    setBusy(true);
    try {
      await setSharedConnectorToken(accessToken, connector.id, newToken.trim());
      setReplacing(null);
      setNewToken("");
      showToast("success", "The token was replaced.");
      await load();
    } catch (error) {
      showToast("error", errorMessage(error, "The token could not be replaced."));
    } finally {
      setBusy(false);
    }
  }

  async function makeWebhook(connector: ConnectorSource) {
    setBusy(true);
    try {
      const made = await createSharedConnectorWebhook(accessToken, connector.id);
      setHookSecret({ secret: made.secret, address: made.url ?? `${window.location.origin}${made.path}` });
      showToast("success", "The webhook secret is shown below. Copy it now: it is not shown again.");
      await load();
    } catch (error) {
      showToast("error", errorMessage(error, "The webhook could not be set up."));
    } finally {
      setBusy(false);
    }
  }

  async function removeWebhook(connector: ConnectorSource) {
    setBusy(true);
    try {
      await deleteSharedConnectorWebhook(accessToken, connector.id);
      setHookSecret(null);
      showToast("success", "Webhooks were turned off for this connector.");
      await load();
    } catch (error) {
      showToast("error", errorMessage(error, "The webhook could not be removed."));
    } finally {
      setBusy(false);
    }
  }

  async function remove(connector: ConnectorSource) {
    setBusy(true);
    try {
      await deleteSharedConnector(accessToken, connector.id);
      setRemoving(null);
      showToast("success", `Removed ${connector.name} and its items.`);
      await load();
      onChanged();
    } catch (error) {
      showToast("error", errorMessage(error, "The connector could not be removed."));
    } finally {
      setBusy(false);
    }
  }

  return <section className="shared-settings-group">
    <PanelHeading description={<>{connectors?.length ?? 0} connected</>} icon={GitHub} title="Issues and pull requests" />
    <p className="shared-muted-copy">Keep a GitHub repository's or GitLab project's issues and pull (merge) requests, with their comments, reviews and commits, as searchable and citable evidence. Closed issues and abandoned requests are marked outdated and come back when reopened; merged ones stay. A read-only token is only needed for a private repository or a higher rate limit.</p>

    {connectors?.length ? <ul className="rm-repo-links">
      {connectors.map((connector) => {
        const running = connector.status === "syncing" || Boolean(connector.active_job);
        return <li key={connector.id}>
          <div>
            <strong>{connector.name}</strong>
            <code className="rm-repo-path">{connector.location}</code>
            <span className="rm-map-muted">
              {STATUS_LABEL[running ? "syncing" : connector.status] ?? connector.status}
              {" · "}{connector.open_items.toLocaleString()} open · {connector.closed_items.toLocaleString()} closed
              {connector.last_synced_at ? ` · last sync ${formatDate(connector.last_synced_at)}` : ""}
              {connector.has_credential ? " · with a token" : " · no token"}
              {connector.last_report?.more_to_fetch ? " · more to fetch on the next sync" : ""}
            </span>
            {!running && connector.status === "error" && connector.last_error ? <span className="rm-map-muted">{connector.last_error}</span> : null}
            {!running && connector.status === "paused" ? <span className="rm-map-muted">
              Paused{connector.paused_until ? ` until ${formatDate(connector.paused_until)}` : " until the token is replaced"}: {connector.paused_reason}
            </span> : null}
            {connector.has_webhook ? <span className="rm-map-muted">Webhook on{connector.webhook_last_received_at ? ` · last call ${formatDate(connector.webhook_last_received_at)}` : " · no call yet"}</span> : null}
          </div>
          <div className="rm-health-actions">
            <Button disabled={running} onClick={() => void sync(connector)} type="button" variant="secondary"><Refresh size={ICON_SIZE.sm} /> Sync now</Button>
            <Button disabled={running} onClick={() => { setNewToken(""); setReplacing(connector); }} type="button" variant="secondary"><Key size={ICON_SIZE.sm} /> Token</Button>
            <Button disabled={running} onClick={() => { setHookSecret(null); setHooking(connector); }} type="button" variant="secondary"><Webhook size={ICON_SIZE.sm} /> Webhook</Button>
            <Button disabled={running} onClick={() => setRemoving(connector)} type="button" variant="secondary"><Trash size={ICON_SIZE.sm} /> Remove</Button>
          </div>
        </li>;
      })}
    </ul> : null}

    <form className="rm-repo-form" onSubmit={runCheck}>
      <h3>Connect a repository's issues</h3>
      <label>Repository address
        <Input onChange={(event) => setLink(event.target.value)} placeholder="https://github.com/acme/app or https://gitlab.com/group/project" value={link} />
        <span className="rm-map-muted">A github.com or gitlab.com repository, or a self-hosted GitHub Enterprise Server or GitLab one. The server's operator names the hosts a tracker may be on.</span>
      </label>
      <label>Tracker
        <Dropdown aria-label="Tracker" onValueChange={(value) => setProvider(value === "__detect__" ? "" : value)} options={[{ label: "Detect from the address", value: "__detect__" }, { label: "GitHub", value: "github" }, { label: "GitLab", value: "gitlab" }]} value={provider || "__detect__"} />
        <span className="rm-map-muted">Needed only for a self-hosted address whose name does not say which it is.</span>
      </label>
      <label>Access token
        <Input autoComplete="off" onChange={(event) => setToken(event.target.value)} placeholder="Only for a private repository" type="password" value={token} />
        <span className="rm-map-muted">A read-only token with access to issues. It is stored encrypted and never shown again.</span>
      </label>
      {checkIsCurrent && check ? <div className="rm-repo-check">
        <strong><CircleCheck size={ICON_SIZE.md} /> The server can read this {check.provider === "gitlab" ? "project" : "repository"}'s issues</strong>
        <dl className="rm-repo-facts">
          <div><dt>Repository</dt><dd>{check.name} <span className="rm-map-muted">{check.private ? "private" : "public"}</span></dd></div>
          <div><dt>Open issues</dt><dd>{check.open_issues.toLocaleString()}</dd></div>
        </dl>
        {check.already_connected ? <span className="rm-map-muted">This repository's issues are already connected to the workspace.</span> : null}
      </div> : null}
      <div className="rm-health-actions">
        <Button disabled={busy || !link.trim()} type="submit" variant={checkIsCurrent ? "secondary" : "main"}><Refresh size={ICON_SIZE.sm} /> {checkIsCurrent ? "Check again" : "Check access"}</Button>
        <Button disabled={busy || !checkIsCurrent || check?.already_connected} onClick={() => void connect()} type="button" variant={checkIsCurrent ? "main" : "secondary"}><Plus size={ICON_SIZE.sm} /> Connect and sync</Button>
      </div>
    </form>

    {replacing ? <Dialog
      description={<>Replace the token of <strong>{replacing.name}</strong>. The new token is tried against the tracker before it replaces the old one.</>}
      footer={<><DialogCancel onClick={() => setReplacing(null)} /><Button disabled={busy || !newToken.trim()} onClick={() => void replaceToken(replacing)} type="button" variant="main">Replace token</Button></>}
      onClose={() => setReplacing(null)}
      open
      title="Replace the token"
    >
      <label>New access token
        <Input autoComplete="off" onChange={(event) => setNewToken(event.target.value)} type="password" value={newToken} />
      </label>
    </Dialog> : null}

    {hooking ? <Dialog
      description={<>Webhooks make <strong>{hooking.name}</strong> follow the tracker within seconds instead of at the next scheduled check. Add one in the tracker's settings for issue, comment and pull or merge request events, with content type JSON.</>}
      footer={<DialogCancel label="Close" onClick={() => { setHooking(null); setHookSecret(null); }} />}
      onClose={() => { setHooking(null); setHookSecret(null); }}
      open
      title="Webhook"
    >
      {(() => {
        const live = connectors?.find((connector) => connector.id === hooking.id) ?? hooking;
        return <div className="rm-repo-form">
          <span className="rm-map-muted">{live.has_webhook ? `On${live.webhook_last_received_at ? ` · last call ${formatDate(live.webhook_last_received_at)}` : " · no call yet"}.` : "Off: the connector is checked on a schedule."}</span>
          {hookSecret ? <div className="rm-repo-check">
            <dl className="rm-repo-facts">
              <div><dt>Payload URL</dt><dd><code className="rm-repo-path">{hookSecret.address}</code> <CopyButton label="Copy the payload URL" value={hookSecret.address} /></dd></div>
              <div><dt>Secret</dt><dd><code className="rm-repo-path">{hookSecret.secret}</code> <CopyButton label="Copy the secret" value={hookSecret.secret} /></dd></div>
            </dl>
            <span className="rm-map-muted">Shown once: renewing it ends the old one. GitHub signs with it; for GitLab paste it as the secret token.</span>
          </div> : null}
          <div className="rm-health-actions">
            <Button disabled={busy} onClick={() => void makeWebhook(live)} type="button" variant="secondary">{live.has_webhook ? "Make a new secret" : "Set up a webhook"}</Button>
            {live.has_webhook ? <Button disabled={busy} onClick={() => void removeWebhook(live)} type="button" variant="secondary">Turn off</Button> : null}
          </div>
        </div>;
      })()}
    </Dialog> : null}

    {removing ? <Dialog
      description={<>Remove <strong>{removing.name}</strong> and its {(removing.open_items + removing.closed_items).toLocaleString()} items from the workspace? Their index, comments and memory-card links are deleted too. Nothing changes in the tracker.</>}
      footer={<><DialogCancel onClick={() => setRemoving(null)} /><Button disabled={busy} onClick={() => void remove(removing)} type="button" variant="secondary">Remove connector</Button></>}
      onClose={() => setRemoving(null)}
      open
      title="Remove connector"
    /> : null}
  </section>;
}
