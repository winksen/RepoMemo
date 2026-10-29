import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { IconApi as ApiIcon, IconPlus as Plus, IconTrash as Trash } from "@tabler/icons-react";
import { createPersonalToken, limitPersonalTokenWorkspaces, listPersonalTokens, listSharedWorkspaces, revokePersonalToken } from "../lib/sharedApi";
import type { PersonalAccessToken, SharedWorkspace } from "../types";
import { Switch } from "./ui/switch";
import { TokenWorkspacePicker } from "./TokenWorkspacePicker";
import { SecretDialog } from "./SecretDialog";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { PanelHeading } from "./ui/panel-heading";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Dropdown } from "./ui/dropdown";
import { EmptyState } from "./ui/empty-state";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { LoadingState } from "./ui/loading-state";
import { showToast } from "./ui/toast";
import { ICON_SIZE } from "./ui/icon";
import { apiMessage, formatDate } from "../web/format";

const EXPIRY_OPTIONS = [
  { label: "7 days", value: "7" },
  { label: "30 days", value: "30" },
  { label: "90 days", value: "90" },
  { label: "1 year", value: "365" },
];

const SCOPE_OPTIONS = [
  { label: "Read only: look and search", value: "read" },
  { label: "Read and write: everything you may do", value: "write" },
];
/** Also grants access, deletes, repoints, links and exports: a token that leaks can do all of it, so it is a deliberate choice. */
const ADMIN_SCOPE_OPTION = { label: "Admin: read, write and manage access, links and exports", value: "admin" };
/** Only for system and app administrators: also changes in the System area (maintenance, sessions, lockouts). */
const SYSTEM_SCOPE_OPTION = { label: "System: read, write and System changes", value: "system" };

/**
 * Profile › API tokens: personal access tokens for scripts, CI and the
 * terminal console (`REPOMEMO_TOKEN`). A token acts as its owner within its
 * scope, but never manages tokens, passwords or roles.
 */
export function ApiTokensPanel({ accessToken, isAdministrator }: { accessToken: string; isAdministrator: boolean }) {
  const [tokens, setTokens] = useState<PersonalAccessToken[] | null>(null);
  const [name, setName] = useState("");
  const [scope, setScope] = useState<"read" | "write" | "admin" | "system">("read");
  const [expiry, setExpiry] = useState("90");
  const [secret, setSecret] = useState<string | null>(null);
  const [revoking, setRevoking] = useState<PersonalAccessToken | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [workspaces, setWorkspaces] = useState<SharedWorkspace[]>([]);
  const [limited, setLimited] = useState(false);
  const [chosen, setChosen] = useState<string[]>([]);
  const [limiting, setLimiting] = useState<PersonalAccessToken | null>(null);
  const [limitChosen, setLimitChosen] = useState<string[]>([]);

  useEffect(() => {
    listSharedWorkspaces(accessToken).then(setWorkspaces).catch(() => setWorkspaces([]));
  }, [accessToken]);

  useEffect(() => {
    listPersonalTokens(accessToken).then(setTokens).catch((error) => { showToast("error", apiMessage(error, "API tokens could not be loaded.")); setTokens([]); });
  }, [accessToken]);

  async function create(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsBusy(true);
    try {
      const created = await createPersonalToken(accessToken, { name: name.trim(), scope, expiresInDays: Number(expiry), ...(limited ? { workspaceIds: chosen } : {}) });
      setTokens((current) => [created.token, ...(current ?? [])]);
      setSecret(created.secret);
      setName("");
      setLimited(false);
      setChosen([]);
    } catch (error) { showToast("error", apiMessage(error, "The token could not be created.")); }
    finally { setIsBusy(false); }
  }

  async function limit() {
    if (!limiting) return;
    setIsBusy(true);
    try {
      await limitPersonalTokenWorkspaces(accessToken, limiting.id, limitChosen);
      setTokens((current) => current?.map((entry) => entry.id === limiting.id ? { ...entry, workspaces: limitChosen } : entry) ?? current);
      showToast("success", `${limiting.name} now reaches only the chosen workspaces.`);
      setLimiting(null);
    } catch (error) { showToast("error", apiMessage(error, "The token could not be limited.")); }
    finally { setIsBusy(false); }
  }

  async function revoke() {
    if (!revoking) return;
    setIsBusy(true);
    try {
      await revokePersonalToken(accessToken, revoking.id);
      setTokens((current) => current?.filter((entry) => entry.id !== revoking.id) ?? current);
      showToast("success", `${revoking.name} was revoked. Scripts using it stop working now.`);
      setRevoking(null);
    } catch (error) { showToast("error", apiMessage(error, "The token could not be revoked.")); }
    finally { setIsBusy(false); }
  }

  const now = new Date().toISOString();
  return <section className="shared-profile-panel rm-tokens">
    <PanelHeading description={tokens?.length ?? 0} icon={ApiIcon} title="API tokens" />
    <p className="shared-muted-copy">For scripts, CI and the terminal console: send <code>Authorization: Bearer &lt;token&gt;</code>, or set <code>REPOMEMO_TOKEN</code> for <code>repomemo-console</code>. A token acts as you, so it only reaches what you can. It cannot manage tokens or change your password; granting access, deleting, linking and exporting need the admin scope. It keeps working when you change your password; revoke it here, or ask an administrator to sign you out everywhere.</p>
    <form className="rm-tokens-form" onSubmit={create}>
      <Field label="Name" required><Input maxLength={80} onChange={(event) => setName(event.target.value)} placeholder="CI release notes" value={name} /></Field>
      <Field label="Access">{(control) => <Dropdown id={control.id} onValueChange={(value) => setScope(value as "read" | "write" | "admin" | "system")} options={isAdministrator ? [...SCOPE_OPTIONS, ADMIN_SCOPE_OPTION, SYSTEM_SCOPE_OPTION] : [...SCOPE_OPTIONS, ADMIN_SCOPE_OPTION]} value={scope} />}</Field>
      <Field label="Expires after">{(control) => <Dropdown id={control.id} onValueChange={setExpiry} options={EXPIRY_OPTIONS} value={expiry} />}</Field>
      {scope === "system" ? null : <Switch checked={limited} label="Limit to chosen workspaces" onCheckedChange={setLimited} />}
      {limited && scope !== "system" ? <TokenWorkspacePicker chosen={chosen} onChange={setChosen} workspaces={workspaces} /> : null}
      <Button busy={isBusy} disabled={!name.trim() || (limited && scope !== "system" && chosen.length === 0)} type="submit" variant="secondary"><Plus size={ICON_SIZE.md} /> Create token</Button>
    </form>
    {scope === "admin" ? <p className="shared-muted-copy">An admin token can invite people, change roles, delete workspaces, repoint AI providers, link repositories, set webhooks, and export or import a workspace, as you. Give it only to a script that needs that, and keep it short-lived.</p> : null}
    {tokens?.length ? <ul className="rm-tokens-list">{tokens.map((token) => {
      const expired = token.expires_at <= now;
      return <li className={expired ? "expired" : ""} key={token.id}>
        <span className="rm-tokens-text"><strong>{token.name} <Badge>{token.scope === "system" ? "system" : token.scope === "admin" ? "admin" : token.scope === "write" ? "read and write" : "read only"}</Badge>{expired ? <Badge tone="danger">expired</Badge> : null}</strong><small><code>{token.prefix}…</code> · created {formatDate(token.created_at, "never")} · {expired ? "expired" : "expires"} {formatDate(token.expires_at, "never")} · last used {formatDate(token.last_used_at, "never")} · {token.workspaces ? `limited to ${token.workspaces.map((id) => workspaces.find((entry) => entry.workspace.id === id)?.workspace.name ?? "a workspace you can no longer open").join(", ")}` : "reaches every workspace you can open"}</small></span>
        {!expired && !token.workspaces && token.scope !== "system" ? <Button aria-label={`Limit ${token.name} to workspaces`} disabled={isBusy} onClick={() => { setLimitChosen([]); setLimiting(token); }} variant="secondary">Limit…</Button> : null}
        <Button aria-label={`Revoke ${token.name}`} disabled={isBusy} onClick={() => setRevoking(token)} variant="secondary"><Trash size={ICON_SIZE.sm} /> {expired ? "Remove" : "Revoke"}</Button>
      </li>;
    })}</ul> : tokens ? <EmptyState description="Create one above to use the API from a script." title="No tokens yet" /> : <LoadingState label="Loading API tokens" />}
    <Dialog description="The token then reaches only these workspaces, and it cannot be made unlimited again; create a new token for that." footer={<><DialogCancel onClick={() => setLimiting(null)} /><Button busy={isBusy} disabled={limitChosen.length === 0} type="submit" variant="main">Limit token</Button></>} onClose={() => setLimiting(null)} onSubmit={() => void limit()} open={limiting !== null} title={limiting ? `Limit ${limiting.name}?` : "Limit token"}>
      <TokenWorkspacePicker chosen={limitChosen} onChange={setLimitChosen} workspaces={workspaces} />
    </Dialog>
    {secret ? <SecretDialog description="Use it as a bearer token, or set it as REPOMEMO_TOKEN for repomemo-console. Treat it like a password." onClose={() => setSecret(null)} title="New API token" value={secret} /> : null}
    <Dialog description={revoking ? `Scripts using ${revoking.name} (${revoking.prefix}…) stop working at once. This cannot be undone.` : undefined} footer={<><DialogCancel onClick={() => setRevoking(null)} /><Button busy={isBusy} onClick={() => void revoke()} variant="main">Revoke token</Button></>} onClose={() => setRevoking(null)} open={Boolean(revoking)} title={`Revoke ${revoking?.name ?? "the token"}?`} />
  </section>;
}
