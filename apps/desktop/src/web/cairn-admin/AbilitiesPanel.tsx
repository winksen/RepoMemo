import { useState } from "react";
import { PencilSimple, Plus, Power, Sparkle, Trash } from "@phosphor-icons/react";
import { createCairnAbility, deleteCairnAbility, listCairnAbilities, listCairnTools, publishCairnAbility, setCairnAbilityEnabled } from "../../lib/sharedApi";
import { cairnAdminKeys } from "../../lib/queryClient";
import type { CairnAbility, CairnTool } from "../../types";
import { ActionMenu } from "../../components/ui/action-menu";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Checkbox } from "../../components/ui/checkbox";
import { ConfirmDialog } from "../../components/ui/confirm-dialog";
import { Dialog, DialogCancel } from "../../components/ui/dialog";
import { EmptyState } from "../../components/ui/empty-state";
import { Field } from "../../components/ui/field";
import { ICON_SIZE } from "../../components/ui/icon";
import { Input } from "../../components/ui/input";
import { PanelHeading } from "../../components/ui/panel-heading";
import { SkeletonList } from "../../components/ui/skeleton";
import { Table } from "../../components/ui/table";
import { Textarea } from "../../components/ui/textarea";
import { showToast } from "../../components/ui/toast";
import { apiMessage, formatTime } from "../format";
import { useSystemData } from "../system/shared";

interface Draft {
  id: string | null;
  version: number;
  name: string;
  command: string;
  description: string;
  instructions: string;
  tools: string[];
  /** One field per line, as `name: Label`. */
  inputs: string;
  outputTemplate: string;
}

const BLANK: Draft = { id: null, version: 0, name: "", command: "", description: "", instructions: "", tools: [], inputs: "", outputTemplate: "" };

function parseInputs(text: string): { name: string; label: string }[] {
  return text.split("\n").map((line) => line.trim()).filter(Boolean).map((line) => {
    const [name, ...rest] = line.split(":");
    const label = rest.join(":").trim();
    return { name: name.trim(), label: label || name.trim() };
  });
}

/**
 * The abilities of an organization or a workspace (V0.9.105): a name, a /command, instructions, the tools it may use and the fields it
 * asks for. Publishing makes a new version; the older ones are kept. An ability can be turned off without losing it. Trying one comes
 * with running abilities.
 */
export function AbilitiesPanel({ accessToken, canChange, scope, scopeId, scopeName }: { accessToken: string; canChange: boolean; scope: "organization" | "workspace"; scopeId: string; scopeName: string }) {
  const [abilities, isLoading, reload] = useSystemData<CairnAbility[]>(cairnAdminKeys.abilities(scope, scopeId), () => listCairnAbilities(accessToken, scope, scopeId), { failure: "The abilities could not be loaded." });
  const [tools] = useSystemData<CairnTool[]>(cairnAdminKeys.tools(), () => listCairnTools(accessToken), { failure: "The list of what CAIRN can do could not be loaded." });
  const [draft, setDraft] = useState<Draft | null>(null);
  const [removing, setRemoving] = useState<CairnAbility | null>(null);
  const [isBusy, setIsBusy] = useState(false);

  async function save() {
    if (!draft) return;
    setIsBusy(true);
    try {
      const body = { name: draft.name, command: draft.command, description: draft.description, instructions: draft.instructions, tools: draft.tools, inputs: parseInputs(draft.inputs), output_template: draft.outputTemplate };
      if (draft.id) await publishCairnAbility(accessToken, draft.id, { ...body, expected_version: draft.version });
      else await createCairnAbility(accessToken, scope, scopeId, body);
      showToast("success", draft.id ? `${draft.name} has a new version.` : `${draft.name} was added.`);
      setDraft(null);
      await reload();
    } catch (error) { showToast("error", apiMessage(error, "The ability could not be saved.")); }
    finally { setIsBusy(false); }
  }

  async function toggle(ability: CairnAbility) {
    try {
      await setCairnAbilityEnabled(accessToken, ability.id, !ability.enabled);
      showToast("success", ability.enabled ? `${ability.name} is turned off.` : `${ability.name} is turned on.`);
      await reload();
    } catch (error) { showToast("error", apiMessage(error, "The ability could not be changed.")); }
  }

  async function remove() {
    if (!removing) return;
    try {
      await deleteCairnAbility(accessToken, removing.id);
      showToast("success", `${removing.name} was deleted.`);
      setRemoving(null);
      await reload();
    } catch (error) { showToast("error", apiMessage(error, "The ability could not be deleted.")); setRemoving(null); }
  }

  const toolName = (name: string) => tools?.find((tool) => tool.name === name)?.label ?? name;
  return <section className="shared-settings-group">
    <PanelHeading actions={canChange ? <Button onClick={() => setDraft({ ...BLANK })} type="button" variant="main"><Plus size={ICON_SIZE.md} /> Add ability</Button> : undefined} description={abilities ? `${abilities.length} for ${scopeName}` : undefined} icon={Sparkle} title="Abilities" />
    {abilities ? abilities.length ? <Table className="rm-dense-table">
      <thead><tr><th scope="col">Ability</th><th scope="col">Command</th><th scope="col">Tools</th><th scope="col">Version</th><th scope="col">Last change</th><th scope="col">State</th><th scope="col"><span className="sr-only">Actions</span></th></tr></thead>
      <tbody>{abilities.map((ability) => <tr key={ability.id}>
        <th scope="row">{ability.name}{ability.description ? <small className="shared-muted-copy"> · {ability.description}</small> : null}</th>
        <td><code>{ability.command}</code></td>
        <td>{ability.tools.length ? ability.tools.map(toolName).join(", ") : <span className="shared-muted-copy">None</span>}</td>
        <td><Badge>{`Version ${ability.version}`}</Badge></td>
        <td>{formatTime(ability.updated_at)}{ability.updated_by ? ` · ${ability.updated_by}` : ""}</td>
        <td>{ability.enabled ? "On" : <span className="shared-muted-copy">Off</span>}</td>
        <td>{canChange ? <ActionMenu items={[
          { label: "Edit and publish", icon: <PencilSimple size={ICON_SIZE.sm} />, onSelect: () => setDraft({ id: ability.id, version: ability.version, name: ability.name, command: ability.command, description: ability.description, instructions: ability.instructions, tools: ability.tools, inputs: ability.inputs.map((input) => `${input.name}: ${input.label}`).join("\n"), outputTemplate: ability.output_template }) },
          { label: ability.enabled ? "Turn off" : "Turn on", icon: <Power size={ICON_SIZE.sm} />, onSelect: () => void toggle(ability) },
          { label: "Delete", icon: <Trash size={ICON_SIZE.sm} />, onSelect: () => setRemoving(ability), destructive: true },
        ]} label={`Actions for ${ability.name}`} /> : null}</td>
      </tr>)}</tbody>
    </Table> : <EmptyState description={canChange ? "Teach CAIRN a procedure people run with a /command." : "A CAIRN administrator adds them."} icon={<Sparkle size={25} />} title="No ability yet" /> : isLoading ? <SkeletonList label="Loading the abilities" /> : null}
    <Dialog description="An ability uses only the tools ticked here, and only those the run already has. It never gives CAIRN a tool or a right the person lacks. Publishing makes a new version; the older ones are kept." footer={<><DialogCancel onClick={() => setDraft(null)} /><Button busy={isBusy} disabled={!draft?.name.trim() || !draft.command.trim() || !draft.instructions.trim()} type="submit" variant="main">{draft?.id ? "Publish version" : "Add ability"}</Button></>} onClose={() => setDraft(null)} onSubmit={() => void save()} open={Boolean(draft)} title={draft?.id ? "Edit ability" : "Add ability"}>
      {draft ? <>
        <Field label="Name"><Input autoComplete="off" onChange={(event) => setDraft({ ...draft, name: event.target.value })} value={draft.name} /></Field>
        <Field hint="2 to 30 lower case letters, digits or hyphens, like /weekly-status." label="Command"><Input autoComplete="off" onChange={(event) => setDraft({ ...draft, command: event.target.value })} value={draft.command} /></Field>
        <Field label="Description"><Input autoComplete="off" maxLength={300} onChange={(event) => setDraft({ ...draft, description: event.target.value })} value={draft.description} /></Field>
        <Field hint="What CAIRN does, step by step." label="Instructions"><Textarea onChange={(event) => setDraft({ ...draft, instructions: event.target.value })} rows={8} value={draft.instructions} /></Field>
        <Field hint="One per line, as name: Label. CAIRN asks for them before it starts." label="Fields to ask"><Textarea onChange={(event) => setDraft({ ...draft, inputs: event.target.value })} rows={3} value={draft.inputs} /></Field>
        <Field hint="The shape of the answer. Empty leaves it to CAIRN." label="Output template"><Textarea onChange={(event) => setDraft({ ...draft, outputTemplate: event.target.value })} rows={3} value={draft.outputTemplate} /></Field>
        {tools ? <div>
          <p className="shared-eyebrow">Tools it may use</p>
          {tools.filter((tool) => tool.kind === "tool").map((tool) => <Checkbox checked={draft.tools.includes(tool.name)} key={tool.name} label={tool.label} onCheckedChange={(on) => setDraft({ ...draft, tools: on ? [...draft.tools, tool.name] : draft.tools.filter((name) => name !== tool.name) })} />)}
        </div> : null}
      </> : null}
    </Dialog>
    <ConfirmDialog action="Delete ability" danger description="Its versions are deleted with it. Turn it off instead to keep it." onCancel={() => setRemoving(null)} onConfirm={() => void remove()} open={Boolean(removing)} title={`Delete ${removing?.name ?? "the ability"}?`} />
  </section>;
}
