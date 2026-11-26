import { useState } from "react";
import { Cloud, HardDrives, Sparkle } from "@phosphor-icons/react";
import { getOrganizationCairn, listOrganizationCairnModels, saveOrganizationCairn } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import type { CairnModel, OrganizationCairnSettings } from "../../types";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Checkbox } from "../../components/ui/checkbox";
import { Dropdown } from "../../components/ui/dropdown";
import { EmptyState } from "../../components/ui/empty-state";
import { Field } from "../../components/ui/field";
import { ICON_SIZE } from "../../components/ui/icon";
import { PanelHeading } from "../../components/ui/panel-heading";
import { SkeletonList } from "../../components/ui/skeleton";
import { Table } from "../../components/ui/table";
import { showToast } from "../../components/ui/toast";
import { Tooltip } from "../../components/ui/tooltip";
import { apiMessage, formatTime } from "../format";
import { useSystemData } from "../system/shared";

const MODES = [
  { label: "Automatic: Agent only for a model that passed the live evaluation", value: "auto" },
  { label: "Agent: every workspace that has not chosen", value: "agent" },
  { label: "Jobs: every workspace that has not chosen", value: "jobs" },
];
const TOOL_LABELS: Record<string, string> = { none: "No tools", json: "Tools as JSON", native: "Native tools" };

/** Which catalog models an organization uses, its default and its Agent-mode default (V0.9.83). Shown on the CAIRN administration page and in the organization's own administration. */
export function OrganizationCairnPanel({ accessToken, canChange, organizationId, organizationName }: { accessToken: string; canChange: boolean; organizationId: string; organizationName: string }) {
  const [settings, isLoading, reload] = useSystemData<OrganizationCairnSettings>(cairnAdminKeys.organization(organizationId), () => getOrganizationCairn(accessToken, organizationId), { failure: "The organization's CAIRN settings could not be loaded." });
  const [models] = useSystemData<CairnModel[]>(cairnAdminKeys.organizationModels(organizationId), () => listOrganizationCairnModels(accessToken, organizationId), { failure: "The models could not be loaded.", silentError: !canChange });
  return <section className="shared-settings-group">
    <PanelHeading description={settings?.updated_at ? `Saved ${formatTime(settings.updated_at)}${settings.updated_by ? ` by ${settings.updated_by}` : ""}` : "Not saved yet"} icon={Sparkle} title={`CAIRN in ${organizationName}`} />
    {settings && models ? <Editor accessToken={accessToken} canChange={canChange} key={`${organizationId}:${settings.version}`} models={models} onSaved={() => void reload()} organizationId={organizationId} settings={settings} /> : isLoading ? <SkeletonList label="Loading the organization's CAIRN settings" /> : <EmptyState title="The settings are not available" />}
  </section>;
}

function Editor({ accessToken, canChange, models, onSaved, organizationId, settings }: { accessToken: string; canChange: boolean; models: CairnModel[]; onSaved: () => void; organizationId: string; settings: OrganizationCairnSettings }) {
  const [chosen, setChosen] = useState<string[]>(settings.enabled_model_ids);
  const [fallbackDefault, setDefault] = useState<string>(settings.default_model_id ?? "");
  const [mode, setMode] = useState<string>(settings.agent_mode);
  const [isBusy, setIsBusy] = useState(false);
  const textModels = models.filter((model) => model.purpose === "text");
  const defaultId = chosen.includes(fallbackDefault) ? fallbackDefault : (chosen[0] ?? "");
  const dirty = JSON.stringify([[...chosen].sort(), defaultId || null, mode]) !== JSON.stringify([[...settings.enabled_model_ids].sort(), settings.default_model_id, settings.agent_mode]);

  async function save() {
    setIsBusy(true);
    try {
      await saveOrganizationCairn(accessToken, organizationId, { enabled_model_ids: chosen, default_model_id: defaultId || null, agent_mode: mode, expected_version: settings.version });
      showToast("success", "The organization's CAIRN settings were saved.");
      onSaved();
    } catch (error) { showToast("error", apiMessage(error, "The settings could not be saved.")); }
    finally { setIsBusy(false); }
  }

  if (!textModels.length) return <EmptyState description="A CAIRN administrator adds connections and models on the CAIRN administration page, Models section." title="No model can be chosen yet" />;
  return <form className="rm-system-budget-form" onSubmit={(event) => { event.preventDefault(); void save(); }}>
    <Table className="rm-dense-table">
      <thead><tr><th scope="col">Use</th><th scope="col">Model</th><th scope="col">Where</th><th scope="col">Tools</th><th scope="col">Context</th><th scope="col">Evaluation</th></tr></thead>
      <tbody>{textModels.map((model) => <tr key={model.id}>
        <td><Checkbox checked={chosen.includes(model.id)} disabled={!canChange || (!model.selectable && !chosen.includes(model.id))} label={<span className="sr-only">Use {model.display_name}</span>} onCheckedChange={(on) => setChosen((current) => on ? [...current, model.id] : current.filter((id) => id !== model.id))} /></td>
        <th scope="row">{model.display_name}{!model.selectable ? <small className="shared-muted-copy"> · off or not confirmed</small> : null}</th>
        <td><Badge icon={model.locality === "cloud" ? <Cloud size={ICON_SIZE.sm} /> : <HardDrives size={ICON_SIZE.sm} />}>{model.locality === "cloud" ? "Cloud" : "Local"}</Badge></td>
        <td>{TOOL_LABELS[model.tool_mode] ?? model.tool_mode}</td>
        <td>{model.context_tokens ? model.context_tokens.toLocaleString() : <span className="shared-muted-copy">Not tested</span>}</td>
        <td>{model.evaluation ? <Tooltip content={`Citation precision ${model.evaluation.citation_precision.toFixed(2)}, tool accuracy ${model.evaluation.tool_accuracy.toFixed(2)}`}><span><Badge>{model.evaluation.passed ? "Passed" : "Failed"} · {model.evaluation.run_on}</Badge></span></Tooltip> : <span className="shared-muted-copy">Not evaluated</span>}</td>
      </tr>)}</tbody>
    </Table>
    <Field hint="The model the organization's workspaces use unless they choose another of those above." label="Default model"><Dropdown aria-label="Default model" disabled={!canChange || chosen.length === 0} onValueChange={setDefault} options={textModels.filter((model) => chosen.includes(model.id)).map((model) => ({ label: model.display_name, value: model.id }))} placeholder="Choose models first" value={defaultId} /></Field>
    <Field hint="A workspace that chose its own mode keeps it." label="Agent mode"><Dropdown aria-label="Agent mode" disabled={!canChange} onValueChange={setMode} options={MODES} value={mode} /></Field>
    {canChange ? <Button busy={isBusy} disabled={!dirty} type="submit" variant="main">Save</Button> : null}
  </form>;
}
