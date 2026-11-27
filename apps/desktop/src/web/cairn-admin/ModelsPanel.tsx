import { useState } from "react";
import { Cloud, Cpu, HardDrives, MagnifyingGlass, PencilSimple, Play, Trash } from "@phosphor-icons/react";
import { addCairnModel, changeCairnModel, deleteCairnModel, findCairnModels, listCairnConnections, listCairnModels, testCairnModel } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import type { AvailableCairnModel, CairnConnection, CairnModel } from "../../types";
import { toolChoiceNote } from "../../components/ModelPicker";
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
import { Table } from "../../components/ui/table";
import { Tooltip } from "../../components/ui/tooltip";
import { showToast } from "../../components/ui/toast";
import { apiMessage } from "../format";
import { useSystemData } from "../system/shared";

const PURPOSES = [
  { label: "Text (answers, CAIRN)", value: "text" },
  { label: "Vision (image descriptions)", value: "vision" },
  { label: "Embedding (search vectors)", value: "embedding" },
];
const TOOL_MODES = [
  { label: "No tools", value: "none" },
  { label: "Tools as JSON", value: "json" },
  { label: "Native tool calling", value: "native" },
];
const TOOL_LABELS: Record<string, string> = Object.fromEntries(TOOL_MODES.map((mode) => [mode.value, mode.label]));

interface Draft { model: CairnModel; displayName: string; toolMode: string; contextTokens: string; vision: boolean; enabled: boolean; fallbackId: string }

const NO_FALLBACK = "none";

/** The models of the connections, added from what a server offers and tested for the tools they take (V0.9.77). */
export function ModelsPanel({ accessToken, canChange }: { accessToken: string; canChange: boolean }) {
  const [models, isLoading, reload] = useSystemData<CairnModel[]>(cairnAdminKeys.models(), () => listCairnModels(accessToken), { failure: "The models could not be loaded." });
  const [connections] = useSystemData<CairnConnection[]>(cairnAdminKeys.connections(), () => listCairnConnections(accessToken), { failure: "The connections could not be loaded." });
  const [finding, setFinding] = useState<CairnConnection | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [removing, setRemoving] = useState<CairnModel | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const nameOf = (id: string) => connections?.find((connection) => connection.id === id)?.name ?? "";

  async function save() {
    if (!draft) return;
    setIsBusy(true);
    try {
      await changeCairnModel(accessToken, draft.model.id, { display_name: draft.displayName, tool_mode: draft.toolMode, context_tokens: Number(draft.contextTokens) || 0, vision: draft.vision, enabled: draft.enabled, fallback_model_id: draft.fallbackId === NO_FALLBACK ? null : draft.fallbackId, expected_version: draft.model.version });
      showToast("success", `${draft.displayName} was saved.`);
      setDraft(null);
      await reload();
    } catch (error) { showToast("error", apiMessage(error, "The model could not be saved.")); }
    finally { setIsBusy(false); }
  }

  async function test(model: CairnModel) {
    try {
      const result = await testCairnModel(accessToken, model.id);
      showToast(result.success ? "success" : "error", `${model.display_name}: ${result.message}`);
      await reload();
    } catch (error) { showToast("error", apiMessage(error, "The model could not be tested.")); }
  }

  async function remove() {
    if (!removing) return;
    try {
      await deleteCairnModel(accessToken, removing.id);
      showToast("success", `${removing.display_name} was deleted.`);
      await reload();
    } catch (error) { showToast("error", apiMessage(error, "The model could not be deleted.")); }
    setRemoving(null);
  }

  const usable = (connections ?? []).filter((connection) => connection.enabled);
  return <section className="shared-settings-group">
    <PanelHeading actions={canChange && usable.length ? <Dropdown aria-label="Find the models of a connection" onValueChange={(id) => setFinding(usable.find((connection) => connection.id === id) ?? null)} options={usable.map((connection) => ({ label: `Find models on ${connection.name}`, value: connection.id }))} placeholder="Find models…" value="" /> : undefined} description={models ? `${models.length} model${models.length === 1 ? "" : "s"}` : undefined} icon={Cpu} title="Models" />
    {models ? models.length ? <Table className="rm-dense-table">
      <thead><tr><th scope="col">Name</th><th scope="col">Connection</th><th scope="col">Where</th><th scope="col">Purpose</th><th scope="col">Tools</th><th scope="col">Context</th><th scope="col">Evaluation</th><th scope="col">State</th><th scope="col"><span className="sr-only">Actions</span></th></tr></thead>
      <tbody>{models.map((model) => <tr key={model.id}>
        <th scope="row">{model.display_name}<small className="shared-muted-copy"> · <code>{model.model_id}</code></small></th>
        <td>{nameOf(model.connection_id)}</td>
        <td><Tooltip content={model.locality === "cloud" ? "Runs on a server outside your network" : "Runs on your own network"}><span><Badge icon={model.locality === "cloud" ? <Cloud size={ICON_SIZE.sm} /> : <HardDrives size={ICON_SIZE.sm} />}>{model.locality === "cloud" ? "Cloud" : "Local"}</Badge></span></Tooltip></td>
        <td><Badge>{model.purpose}{model.vision && model.purpose !== "vision" ? " · sees images" : ""}</Badge></td>
        <td>{model.purpose === "text" ? (TOOL_LABELS[model.tool_mode] ?? model.tool_mode) : <span className="shared-muted-copy">—</span>}</td>
        <td>{model.context_tokens ? model.context_tokens.toLocaleString() : <span className="shared-muted-copy">Not tested</span>}</td>
        <td>{model.purpose === "text" ? <Evaluation model={model} /> : <span className="shared-muted-copy">—</span>}</td>
        <td>{model.enabled ? "On" : <span className="shared-muted-copy">Off</span>}{model.enabled && !model.selectable ? <small className="shared-muted-copy"> · connection off or not confirmed</small> : null}{model.uses ? ` · used ${model.uses}×` : ""}{model.fallback_model_id ? <small className="shared-muted-copy"> · falls back to {models?.find((other) => other.id === model.fallback_model_id)?.display_name ?? "another model"}</small> : null}</td>
        <td>{canChange ? <ActionMenu items={[
          { label: "Test", icon: <Play size={ICON_SIZE.sm} />, onSelect: () => void test(model) },
          { label: "Edit", icon: <PencilSimple size={ICON_SIZE.sm} />, onSelect: () => setDraft({ model, displayName: model.display_name, toolMode: model.tool_mode, contextTokens: String(model.context_tokens), vision: model.vision, enabled: model.enabled, fallbackId: model.fallback_model_id ?? NO_FALLBACK }) },
          { label: "Delete", icon: <Trash size={ICON_SIZE.sm} />, onSelect: () => setRemoving(model), destructive: true },
        ]} label={`Actions for ${model.display_name}`} /> : null}</td>
      </tr>)}</tbody>
    </Table> : <EmptyState description={canChange ? "Add a connection above, then find the models its server offers." : "A CAIRN administrator adds them."} icon={<Cpu size={25} />} title="No model yet" /> : isLoading ? <SkeletonList label="Loading the models" /> : null}
    {finding ? <FindModels accessToken={accessToken} connection={finding} onAdded={() => void reload()} onClose={() => setFinding(null)} /> : null}
    <Dialog description="Test fills the tools and the context size from the server. Correct them here when the test is wrong; testing again fills them again." footer={<><DialogCancel onClick={() => setDraft(null)} /><Button busy={isBusy} disabled={!draft?.displayName.trim()} type="submit" variant="main">Save model</Button></>} onClose={() => setDraft(null)} onSubmit={() => void save()} open={Boolean(draft)} title="Edit model">
      {draft ? <>
        <Field label="Name"><Input autoComplete="off" onChange={(event) => setDraft({ ...draft, displayName: event.target.value })} value={draft.displayName} /></Field>
        {draft.model.purpose === "text" ? <Field label="Tools"><Dropdown aria-label="How the model takes tools" onValueChange={(toolMode) => setDraft({ ...draft, toolMode })} options={TOOL_MODES} value={draft.toolMode} /></Field> : null}
        <Field label="Context size (tokens)"><Input inputMode="numeric" min={0} onChange={(event) => setDraft({ ...draft, contextTokens: event.target.value })} type="number" value={draft.contextTokens} /></Field>
        <Field hint="Answers when this model times out, answers with a server error or is rate limited, once for the run. A local model never falls back to a cloud one." label="Fallback model"><Dropdown aria-label="Fallback model" onValueChange={(fallbackId) => setDraft({ ...draft, fallbackId })} options={[{ label: "No fallback", value: NO_FALLBACK }, ...(models ?? []).filter((other) => other.id !== draft.model.id && other.purpose === draft.model.purpose).map((other) => ({ label: `${other.display_name} · ${nameOf(other.connection_id)}`, value: other.id }))]} value={draft.fallbackId} /></Field>
        <Checkbox checked={draft.vision} label="Understands images" onCheckedChange={(vision) => setDraft({ ...draft, vision })} />
        <Checkbox checked={draft.enabled} label="On" onCheckedChange={(enabled) => setDraft({ ...draft, enabled })} />
      </> : null}
    </Dialog>
    <ConfirmDialog action="Delete model" danger description={removing?.uses ? `It is used in ${removing.uses} place${removing.uses === 1 ? "" : "s"} and cannot be deleted until it is not.` : "It leaves the catalog. The server keeps the model."} onCancel={() => setRemoving(null)} onConfirm={() => void remove()} open={Boolean(removing)} title={`Delete ${removing?.display_name ?? "the model"}?`} />
  </section>;
}

/** Whether a model passed the live evaluation, and when; the report is shown beside the model; Agent mode follows the model's tool calling (V0.9.129). */
function Evaluation({ model }: { model: CairnModel }) {
  const report = model.evaluation;
  if (!report) return <span className="shared-muted-copy">Not measured</span>;
  const detail = `Citation precision ${report.citation_precision.toFixed(2)}, tool accuracy ${report.tool_accuracy.toFixed(2)}, ${report.forbidden_actions} forbidden action${report.forbidden_actions === 1 ? "" : "s"}, ${report.cases_run} cases${report.version ? ` at V${report.version}` : ""}`;
  return <Tooltip content={detail}><span><Badge>{toolChoiceNote(report)}</Badge></span></Tooltip>;
}

/** What a server offers: the administrator chooses, and gives each model a purpose. */
function FindModels({ accessToken, connection, onAdded, onClose }: { accessToken: string; connection: CairnConnection; onAdded: () => void; onClose: () => void }) {
  const [available, isLoading] = useSystemData<AvailableCairnModel[]>(cairnAdminKeys.available(connection.id), () => findCairnModels(accessToken, connection.id), { failure: "The server's models could not be listed.", keepPrevious: false });
  const [purpose, setPurpose] = useState("text");
  const [filter, setFilter] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [added, setAdded] = useState<Set<string>>(new Set());
  const shown = (available ?? []).filter((model) => model.model_id.toLowerCase().includes(filter.trim().toLowerCase()));

  async function add(modelId: string) {
    setBusy(modelId);
    try {
      await addCairnModel(accessToken, { connection_id: connection.id, model_id: modelId, purpose });
      setAdded((current) => new Set(current).add(`${modelId}:${purpose}`));
      showToast("success", `${modelId} was added as a ${purpose} model. Test it to fill its tools and context size.`);
      onAdded();
    } catch (error) { showToast("error", apiMessage(error, "The model could not be added.")); }
    finally { setBusy(null); }
  }

  return <Dialog description={`The models ${connection.name} offers. Choose what a model is for, then add the ones CAIRN may use.`} footer={<DialogCancel label="Close" onClick={onClose} />} onClose={onClose} open size="lg" title={`Find models on ${connection.name}`}>
    <Field label="Add as"><Dropdown aria-label="What the added model is for" onValueChange={setPurpose} options={PURPOSES} value={purpose} /></Field>
    <Field label="Filter"><Input autoComplete="off" onChange={(event) => setFilter(event.target.value)} placeholder="Part of a model's name" value={filter} /></Field>
    {available ? shown.length ? <Table className="rm-dense-table">
      <thead><tr><th scope="col">Model</th><th scope="col">In the catalog for</th><th scope="col"><span className="sr-only">Add</span></th></tr></thead>
      <tbody>{shown.map((model) => {
        const already = model.added_for.includes(purpose) || added.has(`${model.model_id}:${purpose}`);
        return <tr key={model.model_id}>
          <th scope="row"><code>{model.model_id}</code></th>
          <td>{model.added_for.length ? model.added_for.join(", ") : <span className="shared-muted-copy">Nothing yet</span>}</td>
          <td><Button busy={busy === model.model_id} disabled={already} onClick={() => void add(model.model_id)} type="button" variant="secondary">{already ? "Added" : "Add"}</Button></td>
        </tr>;
      })}</tbody>
    </Table> : <EmptyState icon={<MagnifyingGlass size={25} />} title={available.length ? "No model matches" : "The server lists no model"} /> : isLoading ? <SkeletonList label="Asking the server" /> : <EmptyState description="Check the connection with Test connection, or add a model by the name its server knows it by." title="The server could not be asked" />}
  </Dialog>;
}
