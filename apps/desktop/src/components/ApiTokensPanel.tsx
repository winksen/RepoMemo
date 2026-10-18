import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { IconApi as ApiIcon, IconPlus as Plus, IconTrash as Trash } from "@tabler/icons-react";
import { createPersonalToken, listPersonalTokens, revokePersonalToken } from "../lib/sharedApi";
import type { PersonalAccessToken } from "../types";
import { SecretDialog } from "./SecretDialog";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { PanelHeading } from "./ui/panel-heading";
import { Dialog, DialogCancel } from "./ui/dialog";
import { Dropdown } from "./ui/dropdown";
import { Input } from "./ui/input";
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
  const [scope, setScope] = useState<"read" | "write" | "system">("read");
  const [expiry, setExpiry] = useState("90");
  const [secret, setSecret] = useState<string | null>(null);
  const [revoking, setRevoking] = useState<PersonalAccessToken | null>(null);
  const [isBusy, setIsBusy] = useState(false);

  useEffect(() => {
    listPersonalTokens(accessToken).then(setTokens).catch((error) => { showToast("error", apiMessage(error, "API tokens could not be loaded.")); setTokens([]); });
  }, [accessToken]);

  async function create(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsBusy(true);
    try {
      const created = await createPersonalToken(accessToken, { name: name.trim(), scope, expiresInDays: Number(expiry) });
      setTokens((current) => [created.token, ...(current ?? [])]);
      setSecret(created.secret);
      setName("");
    } catch (error) { showToast("error", apiMessage(error, "The token could not be created.")); }
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
    <p className="shared-muted-copy">For scripts, CI and the terminal console: send <code>Authorization: Bearer &lt;token&gt;</code>, or set <code>REPOMEMO_TOKEN</code> for <code>repomemo-console</code>. A token acts as you, so it only reaches what you can. It cannot manage tokens, change your password or grant roles. It keeps working when you change your password; revoke it here, or ask an administrator to sign you out everywhere.</p>
    <form className="rm-tokens-form" onSubmit={create}>
      <label>Name<Input maxLength={80} onChange={(event) => setName(event.target.value)} placeholder="CI release notes" required value={name} /></label>
      <label>Access<Dropdown aria-label="Token access" onValueChange={(value) => setScope(value as "read" | "write" | "system")} options={isAdministrator ? [...SCOPE_OPTIONS, SYSTEM_SCOPE_OPTION] : SCOPE_OPTIONS} value={scope} /></label>
      <label>Expires after<Dropdown aria-label="Token lifetime" onValueChange={setExpiry} options={EXPIRY_OPTIONS} value={expiry} /></label>
      <Button disabled={isBusy || !name.trim()} type="submit" variant="secondary"><Plus size={ICON_SIZE.md} /> Create token</Button>
    </form>
    {tokens?.length ? <ul className="rm-tokens-list">{tokens.map((token) => {
      const expired = token.expires_at <= now;
      return <li className={expired ? "expired" : ""} key={token.id}>
        <span className="rm-tokens-text"><strong>{token.name} <Badge>{token.scope === "system" ? "system" : token.scope === "write" ? "read and write" : "read only"}</Badge>{expired ? <Badge tone="danger">expired</Badge> : null}</strong><small><code>{token.prefix}…</code> · created {formatDate(token.created_at, "never")} · {expired ? "expired" : "expires"} {formatDate(token.expires_at, "never")} · last used {formatDate(token.last_used_at, "never")}</small></span>
        <Button aria-label={`Revoke ${token.name}`} disabled={isBusy} onClick={() => setRevoking(token)} type="button" variant="secondary"><Trash size={ICON_SIZE.sm} /> {expired ? "Remove" : "Revoke"}</Button>
      </li>;
    })}</ul> : tokens ? <p className="shared-muted-copy">No tokens yet.</p> : null}
    {secret ? <SecretDialog description="Use it as a bearer token, or set it as REPOMEMO_TOKEN for repomemo-console. Treat it like a password." onClose={() => setSecret(null)} title="New API token" value={secret} /> : null}
    <Dialog description={revoking ? `Scripts using ${revoking.name} (${revoking.prefix}…) stop working at once. This cannot be undone.` : undefined} footer={<><DialogCancel onClick={() => setRevoking(null)} /><Button disabled={isBusy} onClick={() => void revoke()} type="button" variant="main">Revoke token</Button></>} onClose={() => setRevoking(null)} open={Boolean(revoking)} title={`Revoke ${revoking?.name ?? "the token"}?`} />
  </section>;
}
