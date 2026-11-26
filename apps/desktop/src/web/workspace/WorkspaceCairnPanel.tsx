import { useState } from "react";
import { Cloud, HardDrives, Sparkle } from "@phosphor-icons/react";
import { getWorkspaceCairn, saveWorkspaceCairn } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import type { WorkspaceCairnView } from "../../types";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Checkbox } from "../../components/ui/checkbox";
import { Dropdown } from "../../components/ui/dropdown";
import { Field } from "../../components/ui/field";
import { ICON_SIZE } from "../../components/ui/icon";
import { PanelHeading } from "../../components/ui/panel-heading";
import { SkeletonList } from "../../components/ui/skeleton";
import { Table } from "../../components/ui/table";
import { showToast } from "../../components/ui/toast";
import { apiMessage } from "../format";
import { useSystemData } from "../system/shared";

const TOOL_LABELS: Record<string, string> = { none: "No tools", json: "Tools as JSON", native: "Native tools" };

/**
 * Settings › AI: the models of the workspace's organization, marked as coming from it, which of them the workspace offers and its
 * default (V0.9.85). A workspace is shown only what its organization offers, and never other models.
 */
export function WorkspaceCairnPanel({ accessToken, workspaceId }: { accessToken: string; workspaceId: string }) {
  const [view, isLoading, reload] = useSystemData<WorkspaceCairnView>(cairnAdminKeys.workspace(workspaceId), () => getWorkspaceCairn(accessToken, workspaceId), { failure: "The workspace's models could not be loaded." });
  return <section className="shared-settings-group">
    <PanelHeading description={view?.organization_decided ? "Chosen by your organization" : "Your organization has not chosen yet"} icon={Sparkle} title="Models for CAIRN, Ask and summaries" />
    {view ? <Editor accessToken={accessToken} key={`${workspaceId}:${view.version}:${view.models.map((model) => model.id).join()}`} onSaved={() => void reload()} view={view} workspaceId={workspaceId} /> : isLoading ? <SkeletonList label="Loading the workspace's models" /> : null}
  </section>;
}

function Editor({ accessToken, onSaved, view, workspaceId }: { accessToken: string; onSaved: () => void; view: WorkspaceCairnView; workspaceId: string }) {
  const [offered, setOffered] = useState<string[]>(view.models.filter((model) => model.offered).map((model) => model.id));
  const [preferred, setPreferred] = useState<string>(view.default_model_id ?? "");
  const [isBusy, setIsBusy] = useState(false);
  const defaultId = offered.includes(preferred) ? preferred : "";
  const saved = view.models.filter((model) => model.offered).map((model) => model.id).sort();
  const dirty = JSON.stringify([[...offered].sort(), defaultId || null]) !== JSON.stringify([saved, view.default_model_id]);

  async function save() {
    setIsBusy(true);
    try {
      await saveWorkspaceCairn(accessToken, workspaceId, { offered_model_ids: offered, default_model_id: defaultId || null, expected_version: view.version });
      showToast("success", "The workspace's models were saved.");
      onSaved();
    } catch (error) { showToast("error", apiMessage(error, "The models could not be saved.")); }
    finally { setIsBusy(false); }
  }

  if (!view.models.length) return <p className="shared-muted-copy">Your organization offers no model yet. An organization or CAIRN administrator chooses them; until then the workspace keeps its own provider below.</p>;
  return <form className="rm-system-budget-form" onSubmit={(event) => { event.preventDefault(); void save(); }}>
    <Table className="rm-dense-table">
      <thead><tr><th scope="col">Offer</th><th scope="col">Model</th><th scope="col">Where</th><th scope="col">Tools</th><th scope="col">Context</th></tr></thead>
      <tbody>{view.models.map((model) => <tr key={model.id}>
        <td><Checkbox checked={offered.includes(model.id)} disabled={!model.selectable && !offered.includes(model.id)} label={<span className="sr-only">Offer {model.display_name}</span>} onCheckedChange={(on) => setOffered((current) => on ? [...current, model.id] : current.filter((id) => id !== model.id))} /></td>
        <th scope="row">{model.display_name} <Badge>From your organization</Badge>{model.answering ? <small className="shared-muted-copy"> · answers now</small> : null}{view.organization_default_model_id === model.id ? <small className="shared-muted-copy"> · organization default</small> : null}</th>
        <td><Badge icon={model.locality === "cloud" ? <Cloud size={ICON_SIZE.sm} /> : <HardDrives size={ICON_SIZE.sm} />}>{model.locality === "cloud" ? "Cloud" : "Local"}</Badge></td>
        <td>{TOOL_LABELS[model.tool_mode] ?? model.tool_mode}</td>
        <td>{model.context_tokens ? model.context_tokens.toLocaleString() : <span className="shared-muted-copy">Not tested</span>}</td>
      </tr>)}</tbody>
    </Table>
    <Field hint="The model this workspace uses unless a person chooses another of those it offers. Without one, its own provider answers while the organization offers it, then the organization's default." label="Workspace default"><Dropdown aria-label="Workspace default model" disabled={offered.length === 0} onValueChange={setPreferred} options={view.models.filter((model) => offered.includes(model.id)).map((model) => ({ label: model.display_name, value: model.id }))} placeholder="Offer models first" value={defaultId} /></Field>
    <Button busy={isBusy} disabled={!dirty} type="submit" variant="main">Save</Button>
  </form>;
}
