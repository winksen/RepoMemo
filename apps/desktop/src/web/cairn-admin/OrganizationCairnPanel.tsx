import { useState } from "react";
import { Cloud, HardDrives, Sparkle } from "@phosphor-icons/react";
import { getOrganizationCairn, getSharedOrganizationOverview, listOrganizationCairnModels, saveOrganizationCairn } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import type { CairnModel, OrganizationCairnSettings } from "../../types";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Checkbox } from "../../components/ui/checkbox";
import { Dropdown } from "../../components/ui/dropdown";
import { EmptyState } from "../../components/ui/empty-state";
import { Field } from "../../components/ui/field";
import { Textarea } from "../../components/ui/textarea";
import { ICON_SIZE } from "../../components/ui/icon";
import { PanelHeading } from "../../components/ui/panel-heading";
import { SkeletonList } from "../../components/ui/skeleton";
import { Table } from "../../components/ui/table";
import { showToast } from "../../components/ui/toast";
import { Tooltip } from "../../components/ui/tooltip";
import { apiMessage, formatTime } from "../format";
import { useSystemData } from "../system/shared";
import { limitsFor, RoleLimitSelect, withLimit } from "./RoleLimit";
import { ToolSwitches } from "./ToolSwitches";
import { ChangeLimitFields } from "./ChangeLimitFields";
import { SecretsSetting } from "./SecretsSetting";
import { BudgetFields } from "./BudgetFields";
import { RoutineLimits } from "./RoutineLimits";
import { RetentionFields } from "./RetentionFields";
import { McpPolicy } from "./McpPolicy";
import { PersonalMemoryPolicy } from "./PersonalMemoryPolicy";
import { PreferenceDefaults, type PreferenceDefaultsValue } from "./PreferenceDefaults";
import type { Guardrails } from "../../types";

const MODES = [
  { label: "Automatic: Agent only for a model that passed the live evaluation", value: "auto" },
  { label: "Agent: every workspace that has not chosen", value: "agent" },
  { label: "Jobs: every workspace that has not chosen", value: "jobs" },
];
const TOOL_LABELS: Record<string, string> = { none: "No tools", json: "Tools as JSON", native: "Native tools" };

/** The defaults as they are kept: empty text is no default. */
function normalizedDefaults(value: PreferenceDefaultsValue): PreferenceDefaultsValue {
  return { language: value.language?.trim() ? value.language.trim() : null, length: value.length ?? null, steps_open: value.steps_open ?? null, mode: value.mode ?? null };
}

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
  const [roles, setRoles] = useState<Record<string, string[]>>(settings.model_roles ?? {});
  const [guardrails, setGuardrails] = useState<Guardrails>(settings.guardrails ?? { tools_off: [] });
  const toolsOff = guardrails.tools_off;
  const setToolsOff = (tools_off: string[]) => setGuardrails((current) => ({ ...current, tools_off }));
  const [instructions, setInstructions] = useState<string>(settings.instructions ?? "");
  const [knowledgeId, setKnowledgeId] = useState<string>(settings.knowledge_workspace_id ?? "none");
  const [workspaces] = useSystemData<{ workspace_id: string; name: string }[]>(cairnAdminKeys.organizationWorkspaces(organizationId), async () => (await getSharedOrganizationOverview(accessToken, organizationId)).workspaces, { failure: "The organization's workspaces could not be loaded.", silentError: true });
  const [defaults, setDefaults] = useState<PreferenceDefaultsValue>(settings.preference_defaults ?? {});
  const [isBusy, setIsBusy] = useState(false);
  const textModels = models;
  const defaultChoices = models.filter((model) => model.purpose === "text");
  const defaultId = chosen.includes(fallbackDefault) ? fallbackDefault : (defaultChoices.find((model) => chosen.includes(model.id))?.id ?? "");
  const dirty = JSON.stringify(normalizedDefaults(defaults)) !== JSON.stringify(normalizedDefaults(settings.preference_defaults ?? {})) || instructions.trim() !== (settings.instructions ?? "").trim() || knowledgeId !== (settings.knowledge_workspace_id ?? "none") || JSON.stringify([[...chosen].sort(), defaultId || null, mode, limitsFor(roles, chosen), { ...guardrails, tools_off: [...toolsOff].sort() }]) !== JSON.stringify([[...settings.enabled_model_ids].sort(), settings.default_model_id, settings.agent_mode, settings.model_roles ?? {}, { ...(settings.guardrails ?? { tools_off: [] }), tools_off: [...(settings.guardrails?.tools_off ?? [])].sort() }]);

  async function save() {
    setIsBusy(true);
    try {
      await saveOrganizationCairn(accessToken, organizationId, { enabled_model_ids: chosen, default_model_id: defaultId || null, agent_mode: mode, model_roles: limitsFor(roles, chosen), guardrails, instructions, knowledge_workspace_id: knowledgeId === "none" ? null : knowledgeId, preference_defaults: normalizedDefaults(defaults), expected_version: settings.version });
      showToast("success", "The organization's CAIRN settings were saved.");
      onSaved();
    } catch (error) { showToast("error", apiMessage(error, "The settings could not be saved.")); }
    finally { setIsBusy(false); }
  }

  if (!textModels.length) return <EmptyState description="A CAIRN administrator adds connections and models on the CAIRN administration page, Models section." title="No model can be chosen yet" />;
  return <form className="rm-system-budget-form" onSubmit={(event) => { event.preventDefault(); void save(); }}>
    <Table className="rm-dense-table">
      <thead><tr><th scope="col">Use</th><th scope="col">Model</th><th scope="col">For</th><th scope="col">Where</th><th scope="col">Tools</th><th scope="col">Context</th><th scope="col">Evaluation</th><th scope="col">Who may use it</th></tr></thead>
      <tbody>{textModels.map((model) => <tr key={model.id}>
        <td><Checkbox checked={chosen.includes(model.id)} disabled={!canChange || (!model.selectable && !chosen.includes(model.id))} label={<span className="sr-only">Use {model.display_name}</span>} onCheckedChange={(on) => setChosen((current) => on ? [...current, model.id] : current.filter((id) => id !== model.id))} /></td>
        <th scope="row">{model.display_name}{!model.selectable ? <small className="shared-muted-copy"> · off or not confirmed</small> : null}</th>
        <td>{model.purpose}</td>
        <td><Badge icon={model.locality === "cloud" ? <Cloud size={ICON_SIZE.sm} /> : <HardDrives size={ICON_SIZE.sm} />}>{model.locality === "cloud" ? "Cloud" : "Local"}</Badge></td>
        <td>{model.purpose === "text" ? (TOOL_LABELS[model.tool_mode] ?? model.tool_mode) : <span className="shared-muted-copy">—</span>}</td>
        <td>{model.context_tokens ? model.context_tokens.toLocaleString() : <span className="shared-muted-copy">Not tested</span>}</td>
        <td>{model.evaluation ? <Tooltip content={`Citation precision ${model.evaluation.citation_precision.toFixed(2)}, tool accuracy ${model.evaluation.tool_accuracy.toFixed(2)}`}><span><Badge>{model.evaluation.passed ? "Passed" : "Failed"} · {model.evaluation.run_on}</Badge></span></Tooltip> : <span className="shared-muted-copy">Not evaluated</span>}</td>
        <td><RoleLimitSelect disabled={!canChange || !chosen.includes(model.id)} label={model.display_name} onChange={(limit) => setRoles((current) => withLimit(current, model.id, limit))} roles={roles[model.id]} /></td>
      </tr>)}</tbody>
    </Table>
    <Field hint="The model the organization's workspaces use unless they choose another of those above." label="Default model"><Dropdown aria-label="Default model" disabled={!canChange || chosen.length === 0} onValueChange={setDefault} options={defaultChoices.filter((model) => chosen.includes(model.id)).map((model) => ({ label: model.display_name, value: model.id }))} placeholder="Choose models first" value={defaultId} /></Field>
    <Field hint="A workspace that chose its own mode keeps it." label="Agent mode"><Dropdown aria-label="Agent mode" disabled={!canChange} onValueChange={setMode} options={MODES} value={mode} /></Field>
    <Field hint={`Read into every run in the organization's workspaces, after the rules and before a workspace's own CAIRN.md. They never change the rules or what a person may do. Each save is a new version, named in the run's trace. At most 4,000 characters.`} label="Instructions for CAIRN"><Textarea disabled={!canChange} maxLength={4000} onChange={(event) => setInstructions(event.target.value)} rows={5} value={instructions} /></Field>
    <Field hint="Its current memory cards and verified playbooks are offered in the organization's other workspaces, only to the people who can read it." label="Knowledge workspace"><Dropdown aria-label="Knowledge workspace" disabled={!canChange} onValueChange={setKnowledgeId} options={[{ label: "None", value: "none" }, ...(workspaces ?? []).map((workspace) => ({ label: workspace.name, value: workspace.workspace_id }))]} value={knowledgeId} /></Field>
    <PreferenceDefaults disabled={!canChange} onChange={setDefaults} value={defaults} />
    <ToolSwitches accessToken={accessToken} disabled={!canChange} off={toolsOff} onChange={setToolsOff} />
    <ChangeLimitFields disabled={!canChange} guardrails={guardrails} onChange={setGuardrails} />
    <SecretsSetting disabled={!canChange} guardrails={guardrails} onChange={setGuardrails} />
    <RetentionFields accessToken={accessToken} disabled={!canChange} guardrails={guardrails} onChange={setGuardrails} organizationId={organizationId} />
    <McpPolicy disabled={!canChange} guardrails={guardrails} onChange={setGuardrails} />
    <PersonalMemoryPolicy disabled={!canChange} guardrails={guardrails} onChange={setGuardrails} withCap />
    <RoutineLimits disabled={!canChange} guardrails={guardrails} onChange={setGuardrails} />
    <BudgetFields canExempt disabled={!canChange} guardrails={guardrails} onChange={setGuardrails} />
    {canChange ? <Button busy={isBusy} disabled={!dirty} type="submit" variant="main">Save</Button> : null}
  </form>;
}
