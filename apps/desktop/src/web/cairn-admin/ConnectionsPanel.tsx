import { useState } from "react";
import { Cloud, HardDrives, PencilSimple, Plugs, Plus, Trash } from "@phosphor-icons/react";
import { createCairnConnection, deleteCairnConnection, listCairnConnections, testCairnConnection, updateCairnConnection } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import type { CairnConnection } from "../../types";
import { ActionMenu } from "../../components/ui/action-menu";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Checkbox } from "../../components/ui/checkbox";
import { ConfirmDialog } from "../../components/ui/confirm-dialog";
import { Dialog, DialogCancel } from "../../components/ui/dialog";
import { Dropdown } from "../../components/ui/dropdown";
import { EmptyState } from "../../components/ui/empty-state";
import { Field } from "../../components/ui/field";
import { ICON_SIZE } from "../../components/ui/icon";
import { Input } from "../../components/ui/input";
import { PanelHeading } from "../../components/ui/panel-heading";
import { SkeletonList } from "../../components/ui/skeleton";
import { showToast } from "../../components/ui/toast";
import { Table } from "../../components/ui/table";
import { apiMessage, formatTime } from "../format";
import { useSystemData } from "../system/shared";

const KINDS = [
  { label: "Ollama", value: "ollama" },
  { label: "OpenRouter", value: "openrouter" },
  { label: "OpenAI-compatible", value: "openai_compatible" },
];
const KIND_LABELS: Record<string, string> = Object.fromEntries(KINDS.map((kind) => [kind.value, kind.label]));

interface Draft {
  id: string | null;
  version: number | null;
  kind: string;
  name: string;
  baseUrl: string;
  apiKey: string;
  enabled: boolean;
  acknowledged: boolean;
  hadKey: boolean;
}

const BLANK: Draft = { id: null, version: null, kind: "ollama", name: "", baseUrl: "", apiKey: "", enabled: true, acknowledged: false, hadKey: false };

/** The connections to model servers: the first half of the model catalog (V0.9.76). The key is write-only. */
export function ConnectionsPanel({ accessToken, canChange }: { accessToken: string; canChange: boolean }) {
  const [connections, isLoading, reload] = useSystemData<CairnConnection[]>(cairnAdminKeys.connections(), () => listCairnConnections(accessToken), { failure: "The connections could not be loaded." });
  const [draft, setDraft] = useState<Draft | null>(null);
  const [removing, setRemoving] = useState<CairnConnection | null>(null);
  const [isBusy, setIsBusy] = useState(false);

  async function save() {
    if (!draft) return;
    setIsBusy(true);
    try {
      const body = { kind: draft.kind, name: draft.name, base_url: draft.baseUrl || undefined, api_key: draft.apiKey ? draft.apiKey : undefined, enabled: draft.enabled, cloud_content_acknowledged: draft.acknowledged };
      if (draft.id) await updateCairnConnection(accessToken, draft.id, { ...body, expected_version: draft.version ?? 0 });
      else await createCairnConnection(accessToken, body);
      showToast("success", draft.id ? `${draft.name} was saved.` : `${draft.name} was added.`);
      setDraft(null);
      await reload();
    } catch (error) { showToast("error", apiMessage(error, "The connection could not be saved.")); }
    finally { setIsBusy(false); }
  }

  async function test(connection: CairnConnection) {
    try {
      const report = await testCairnConnection(accessToken, connection.id);
      showToast(report.reachable && report.key_accepted !== false ? "success" : "error", `${connection.name}: ${report.message}`);
    } catch (error) { showToast("error", apiMessage(error, "The connection could not be tested.")); }
  }

  async function remove() {
    if (!removing) return;
    try {
      await deleteCairnConnection(accessToken, removing.id);
      showToast("success", `${removing.name} was deleted.`);
      setRemoving(null);
      await reload();
    } catch (error) { showToast("error", apiMessage(error, "The connection could not be deleted.")); setRemoving(null); }
  }

  const cloud = draft?.kind === "openrouter" || (draft?.kind === "openai_compatible" && draft.baseUrl.trim() !== "" && !/^https?:\/\/(localhost|127\.|10\.|192\.168\.|\[::1\])/i.test(draft.baseUrl.trim()));
  return <section className="shared-settings-group">
    <PanelHeading actions={canChange ? <Button onClick={() => setDraft({ ...BLANK })} type="button" variant="main"><Plus size={ICON_SIZE.md} /> Add connection</Button> : undefined} description={connections ? `${connections.length} connection${connections.length === 1 ? "" : "s"}` : undefined} icon={Plugs} title="Connections" />
    {connections ? connections.length ? <Table className="rm-dense-table">
      <thead><tr><th scope="col">Name</th><th scope="col">Kind</th><th scope="col">Address</th><th scope="col">Where</th><th scope="col">Key</th><th scope="col">State</th><th scope="col"><span className="sr-only">Actions</span></th></tr></thead>
      <tbody>{connections.map((connection) => <tr key={connection.id}>
        <th scope="row">{connection.name}<small className="shared-muted-copy"> · changed {formatTime(connection.updated_at)}</small></th>
        <td>{KIND_LABELS[connection.kind] ?? connection.kind}</td>
        <td><code>{connection.base_url}</code></td>
        <td><Badge icon={connection.locality === "cloud" ? <Cloud size={ICON_SIZE.sm} /> : <HardDrives size={ICON_SIZE.sm} />}>{connection.locality === "cloud" ? "Cloud" : "Local"}</Badge></td>
        <td>{connection.has_key ? "Set" : <span className="shared-muted-copy">None</span>}</td>
        <td>{connection.enabled ? "On" : <span className="shared-muted-copy">Off</span>}</td>
        <td>{canChange ? <ActionMenu items={[
          { label: "Test connection", icon: <Plugs size={ICON_SIZE.sm} />, onSelect: () => void test(connection) },
          { label: "Edit", icon: <PencilSimple size={ICON_SIZE.sm} />, onSelect: () => setDraft({ id: connection.id, version: connection.version, kind: connection.kind, name: connection.name, baseUrl: connection.base_url, apiKey: "", enabled: connection.enabled, acknowledged: Boolean(connection.cloud_acknowledged_at), hadKey: connection.has_key }) },
          { label: "Delete", icon: <Trash size={ICON_SIZE.sm} />, onSelect: () => setRemoving(connection), destructive: true },
        ]} label={`Actions for ${connection.name}`} /> : null}</td>
      </tr>)}</tbody>
    </Table> : <EmptyState description={canChange ? "Add the Ollama, OpenRouter or OpenAI-compatible server CAIRN's models run on." : "A CAIRN administrator adds them."} icon={<Plugs size={25} />} title="No connection yet" /> : isLoading ? <SkeletonList label="Loading the connections" /> : null}
    <Dialog description="The address is checked like a workspace's provider: only the hosts the server allows, and what CAIRN sends there goes only there. The key is never shown again." footer={<><DialogCancel onClick={() => setDraft(null)} /><Button busy={isBusy} disabled={!draft?.name.trim()} type="submit" variant="main">{draft?.id ? "Save connection" : "Add connection"}</Button></>} onClose={() => setDraft(null)} onSubmit={() => void save()} open={Boolean(draft)} title={draft?.id ? "Edit connection" : "Add connection"}>
      {draft ? <>
        <Field label="Kind"><Dropdown aria-label="Kind of connection" disabled={Boolean(draft.id)} onValueChange={(kind) => setDraft({ ...draft, kind })} options={KINDS} value={draft.kind} /></Field>
        <Field label="Name"><Input autoComplete="off" onChange={(event) => setDraft({ ...draft, name: event.target.value })} value={draft.name} /></Field>
        <Field hint={draft.kind === "ollama" ? "Leave empty for the Ollama on this machine." : draft.kind === "openrouter" ? "Leave empty for openrouter.ai." : "For example https://api.example.com/v1."} label="Address"><Input autoComplete="off" onChange={(event) => setDraft({ ...draft, baseUrl: event.target.value })} value={draft.baseUrl} /></Field>
        <Field hint={draft.hadKey ? "A key is saved. Leave empty to keep it, or enter a new one. It is only sent to the address it was saved for." : "Sent only to this address."} label="API key"><Input autoComplete="off" onChange={(event) => setDraft({ ...draft, apiKey: event.target.value })} type="password" value={draft.apiKey} /></Field>
        <Checkbox checked={draft.enabled} label="On" onCheckedChange={(enabled) => setDraft({ ...draft, enabled })} />
        {cloud ? <Checkbox checked={draft.acknowledged} description="Needed before a connection at this address can be on." label="What CAIRN sends to this address leaves my network" onCheckedChange={(acknowledged) => setDraft({ ...draft, acknowledged })} /> : null}
      </> : null}
    </Dialog>
    <ConfirmDialog action="Delete connection" danger description="Its models that nothing uses are removed with it. A connection with a model in use cannot be deleted." onCancel={() => setRemoving(null)} onConfirm={() => void remove()} open={Boolean(removing)} title={`Delete ${removing?.name ?? "the connection"}?`} />
  </section>;
}
