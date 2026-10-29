// System › Users: every live API token on the server, who owns it, how far it reaches and when it ends.
import { useCallback, useEffect, useState } from "react";
import { IconApi as ApiIcon } from "@tabler/icons-react";
import { request } from "../lib/sharedApi";
import { apiMessage, formatDate } from "../web/format";
import { Button } from "./ui/button";
import { PanelHeading } from "./ui/panel-heading";
import { Table } from "./ui/table";
import { showToast } from "./ui/toast";

interface Entry { id: string; display_name: string; email: string | null; name: string; prefix: string; scope: string; expires_at: string; last_used_at: string | null; workspaces: string[] | null }

export function TokenInventory({ accessToken }: { accessToken: string }) {
  const [tokens, setTokens] = useState<Entry[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const load = useCallback(async () => {
    try { setTokens((await request<{ tokens: Entry[] }>("/v1/system/tokens", {}, accessToken)).tokens); }
    catch (error) { showToast("error", apiMessage(error, "The API token inventory could not be loaded.")); setTokens([]); }
  }, [accessToken]);
  useEffect(() => { void load(); }, [load]);
  async function revoke(entry: Entry) {
    setBusy(entry.id);
    try {
      await request(`/v1/system/tokens/${entry.id}`, { method: "DELETE" }, accessToken);
      showToast("success", `${entry.name} of ${entry.display_name} was revoked.`);
      await load();
    } catch (error) { showToast("error", apiMessage(error, "The token could not be revoked.")); }
    finally { setBusy(null); }
  }
  if (!tokens?.length) return null;
  return <section aria-label="API token inventory" className="shared-settings-group">
    <PanelHeading description={tokens.length} icon={ApiIcon} title="API token inventory" />
    <p className="shared-muted-copy">Every token that is not revoked or expired. A token limited to workspaces reaches only those; the rest reach every workspace their owner can open, until the owner limits them. Secrets are never shown.</p>
    <Table>
      <thead><tr><th scope="col">Owner</th><th scope="col">Token</th><th scope="col">Scope</th><th scope="col">Reaches</th><th scope="col">Expires</th><th scope="col">Last used</th><th scope="col"><span className="sr-only">Actions</span></th></tr></thead>
      <tbody>{tokens.map((entry) => <tr key={entry.id}>
        <th scope="row">{entry.display_name}{entry.email ? ` · ${entry.email}` : ""}</th>
        <td>{entry.name} <code>{entry.prefix}…</code></td>
        <td>{entry.scope}</td>
        <td>{entry.workspaces ? `${entry.workspaces.length} workspace${entry.workspaces.length === 1 ? "" : "s"}` : "Everything the owner can open"}</td>
        <td>{formatDate(entry.expires_at, "never")}</td>
        <td>{formatDate(entry.last_used_at, "never")}</td>
        <td><Button aria-label={`Revoke ${entry.name} of ${entry.display_name}`} busy={busy === entry.id} onClick={() => void revoke(entry)} type="button" variant="secondary">Revoke</Button></td>
      </tr>)}</tbody>
    </Table>
  </section>;
}
