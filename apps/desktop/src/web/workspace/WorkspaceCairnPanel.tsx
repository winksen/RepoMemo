import { useState } from "react";
import { Cloud, HardDrives, Sparkle } from "@phosphor-icons/react";
import { chooseWorkspaceModel, getWorkspaceCairn, saveWorkspaceCairn } from "../../lib/sharedApi";
import { ConfirmDialog } from "../../components/ui/confirm-dialog";
import type { PurposeChoice } from "../../types";
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
import { limitsFor, RoleLimitSelect, withLimit } from "../cairn-admin/RoleLimit";
import { ToolSwitches } from "../cairn-admin/ToolSwitches";
import { ChangeLimitFields } from "../cairn-admin/ChangeLimitFields";
import { ClosedFolders } from "../cairn-admin/ClosedFolders";
import { SecretsSetting } from "../cairn-admin/SecretsSetting";
import { BudgetFields } from "../cairn-admin/BudgetFields";
import type { Guardrails } from "../../types";

const TOOL_LABELS: Record<string, string> = { none: "No tools", json: "Tools as JSON", native: "Native tools" };

/**
 * Settings › AI: the models of the workspace's organization, marked as coming from it, which of them the workspace offers and its
 * default (V0.9.85). A workspace is shown only what its organization offers, and never other models.
 */
export function WorkspaceCairnPanel({ accessToken, embeddingStatus, workspaceId }: { accessToken: string; embeddingStatus?: string | null; workspaceId: string }) {
  const [view, isLoading, reload] = useSystemData<WorkspaceCairnView>(cairnAdminKeys.workspace(workspaceId), () => getWorkspaceCairn(accessToken, workspaceId), { failure: "The workspace's models could not be loaded." });
  return <section className="shared-settings-group">
    <PanelHeading description={view?.organization_decided ? "Chosen by your organization" : "Your organization has not chosen yet"} icon={Sparkle} title="Models for CAIRN, Ask and summaries" />
    {view ? <PurposeModels accessToken={accessToken} onChanged={() => void reload()} status={embeddingStatus} view={view} workspaceId={workspaceId} /> : null}
    {view ? <Editor accessToken={accessToken} key={`${workspaceId}:${view.version}:${view.models.map((model) => model.id).join()}`} onSaved={() => void reload()} view={view} workspaceId={workspaceId} /> : isLoading ? <SkeletonList label="Loading the workspace's models" /> : null}
  </section>;
}

const NONE = "none";

/** The workspace's embedding and vision models, chosen from the catalog models its organization enabled (V0.9.89). Changing the embedding model asks first. */
function PurposeModels({ accessToken, onChanged, status, view, workspaceId }: { accessToken: string; onChanged: () => void; status?: string | null; view: WorkspaceCairnView; workspaceId: string }) {
  const [pending, setPending] = useState<{ purpose: "embedding" | "vision"; modelId: string | null; message: string } | null>(null);
  async function choose(purpose: "embedding" | "vision", modelId: string | null, confirm = false) {
    try {
      await chooseWorkspaceModel(accessToken, workspaceId, purpose, { model_id: modelId, confirm_rebuild: confirm });
      showToast("success", purpose === "embedding" ? "The embedding model was saved. Passages are embedded in the background." : "The vision model was saved.");
      setPending(null);
      onChanged();
    } catch (error) {
      const message = apiMessage(error, "The model could not be saved.");
      if (purpose === "embedding" && !confirm && /Confirm to continue/.test(message)) setPending({ purpose, modelId, message });
      else showToast("error", message);
    }
  }
  const field = (purpose: "embedding" | "vision", label: string, choice: PurposeChoice, hint: string) => <Field hint={choice.current_name && !choice.current_model_id ? `${hint} Now: ${choice.current_name}, a provider set up before models came from the catalog.` : hint} label={label}>
    <Dropdown aria-label={label} onValueChange={(id) => void choose(purpose, id === NONE ? null : id)} options={[{ label: choice.candidates.length || choice.current_name ? "None" : "None offered by your organization", value: NONE }, ...choice.candidates.filter((model) => model.selectable || model.id === choice.current_model_id).map((model) => ({ label: `${model.display_name} · ${model.locality === "cloud" ? "Cloud" : "Local"}`, value: model.id }))]} value={choice.current_model_id ?? NONE} />
  </Field>;
  return <div className="rm-system-budget-form">
    {field("embedding", "Embedding model", view.embedding, "Builds the vectors that search by meaning uses. Changing it removes the stored vectors and embeds every passage again.")}
    {status ? <p className="shared-ai-provider-status">{status}</p> : null}
    {field("vision", "Vision model", view.vision, "Describes the images of the workspace so they can be searched.")}
    <ConfirmDialog action="Change and rebuild" danger description={pending?.message} onCancel={() => setPending(null)} onConfirm={() => pending && void choose(pending.purpose, pending.modelId, true)} open={Boolean(pending)} title="Change the embedding model?" />
  </div>;
}

function Editor({ accessToken, onSaved, view, workspaceId }: { accessToken: string; onSaved: () => void; view: WorkspaceCairnView; workspaceId: string }) {
  const [offered, setOffered] = useState<string[]>(view.models.filter((model) => model.offered).map((model) => model.id));
  const [preferred, setPreferred] = useState<string>(view.default_model_id ?? "");
  const [roles, setRoles] = useState<Record<string, string[]>>(view.model_roles ?? {});
  const [guardrails, setGuardrails] = useState<Guardrails>(view.guardrails ?? { tools_off: [] });
  const toolsOff = guardrails.tools_off;
  const setToolsOff = (tools_off: string[]) => setGuardrails((current) => ({ ...current, tools_off }));
  const [isBusy, setIsBusy] = useState(false);
  const defaultId = offered.includes(preferred) ? preferred : "";
  const saved = view.models.filter((model) => model.offered).map((model) => model.id).sort();
  const dirty = JSON.stringify([[...offered].sort(), defaultId || null, limitsFor(roles, offered), { ...guardrails, tools_off: [...toolsOff].sort() }]) !== JSON.stringify([saved, view.default_model_id, view.model_roles ?? {}, { ...(view.guardrails ?? { tools_off: [] }), tools_off: [...(view.guardrails?.tools_off ?? [])].sort() }]);

  async function save() {
    setIsBusy(true);
    try {
      await saveWorkspaceCairn(accessToken, workspaceId, { offered_model_ids: offered, default_model_id: defaultId || null, model_roles: limitsFor(roles, offered), guardrails, expected_version: view.version });
      showToast("success", "The workspace's models were saved.");
      onSaved();
    } catch (error) { showToast("error", apiMessage(error, "The models could not be saved.")); }
    finally { setIsBusy(false); }
  }

  if (!view.models.length) return <p className="shared-muted-copy">Your organization offers no model yet. An organization or CAIRN administrator chooses them; until then the workspace keeps its own provider below.</p>;
  return <form className="rm-system-budget-form" onSubmit={(event) => { event.preventDefault(); void save(); }}>
    <Table className="rm-dense-table">
      <thead><tr><th scope="col">Offer</th><th scope="col">Model</th><th scope="col">Where</th><th scope="col">Tools</th><th scope="col">Context</th><th scope="col">Who may use it</th></tr></thead>
      <tbody>{view.models.map((model) => <tr key={model.id}>
        <td><Checkbox checked={offered.includes(model.id)} disabled={!model.selectable && !offered.includes(model.id)} label={<span className="sr-only">Offer {model.display_name}</span>} onCheckedChange={(on) => setOffered((current) => on ? [...current, model.id] : current.filter((id) => id !== model.id))} /></td>
        <th scope="row">{model.display_name} <Badge>From your organization</Badge>{model.answering ? <small className="shared-muted-copy"> · answers now</small> : null}{view.organization_default_model_id === model.id ? <small className="shared-muted-copy"> · organization default</small> : null}</th>
        <td><Badge icon={model.locality === "cloud" ? <Cloud size={ICON_SIZE.sm} /> : <HardDrives size={ICON_SIZE.sm} />}>{model.locality === "cloud" ? "Cloud" : "Local"}</Badge></td>
        <td>{TOOL_LABELS[model.tool_mode] ?? model.tool_mode}</td>
        <td>{model.context_tokens ? model.context_tokens.toLocaleString() : <span className="shared-muted-copy">Not tested</span>}</td>
        <td><RoleLimitSelect disabled={!offered.includes(model.id)} label={model.display_name} onChange={(limit) => setRoles((current) => withLimit(current, model.id, limit))} roles={roles[model.id]} /></td>
      </tr>)}</tbody>
    </Table>
    <Field hint="The model this workspace uses unless a person chooses another of those it offers. Without one, its own provider answers while the organization offers it, then the organization's default." label="Workspace default"><Dropdown aria-label="Workspace default model" disabled={offered.length === 0} onValueChange={setPreferred} options={view.models.filter((model) => offered.includes(model.id)).map((model) => ({ label: model.display_name, value: model.id }))} placeholder="Offer models first" value={defaultId} /></Field>
    <ToolSwitches accessToken={accessToken} locked={view.organization_tools_off} off={toolsOff} onChange={setToolsOff} />
    <ChangeLimitFields ceiling={view.organization_limits} guardrails={guardrails} onChange={setGuardrails} />
    <BudgetFields guardrails={guardrails} onChange={setGuardrails} />
    <SecretsSetting guardrails={guardrails} onChange={setGuardrails} organizationMasksPersonal={Boolean(view.organization_limits.mask_personal_data)} organizationRefuses={view.organization_limits.secrets_for_cloud === "refuse"} />
    <ClosedFolders closed={guardrails.closed_folder_ids ?? []} folders={view.folders} onChange={(closed_folder_ids) => setGuardrails((current) => ({ ...current, closed_folder_ids }))} />
    <Button busy={isBusy} disabled={!dirty} type="submit" variant="main">Save</Button>
  </form>;
}
