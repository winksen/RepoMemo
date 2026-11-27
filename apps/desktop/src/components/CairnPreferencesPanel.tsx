import { useEffect, useState } from "react";
import { SlidersHorizontal, Trash } from "@phosphor-icons/react";
import type { CairnPreferences, OfferedModel, SharedWorkspace } from "../types";
import { getCairnPreferences, getSharedAgentCapabilities, saveCairnPreferences } from "../lib/sharedApi";
import { ASSISTANT_NAME } from "../lib/assistantName";
import { apiMessage } from "../web/format";
import { Button } from "./ui/button";
import { Dropdown } from "./ui/dropdown";
import { Field } from "./ui/field";
import { ICON_SIZE } from "./ui/icon";
import { Input } from "./ui/input";
import { PanelHeading } from "./ui/panel-heading";
import { showToast } from "./ui/toast";

const NONE = "none";
const LENGTHS = [{ label: "No preference", value: NONE }, { label: "Brief", value: "brief" }, { label: "Standard", value: "standard" }, { label: "Detailed", value: "detailed" }];
const STEPS = [{ label: "No preference (closed)", value: NONE }, { label: "Open", value: "open" }, { label: "Closed", value: "closed" }];
const MODES = [{ label: "No preference", value: NONE }, { label: "Agent", value: "agent" }, { label: "Jobs", value: "jobs" }];

/**
 * The person's CAIRN preferences (V0.9.114): the model they prefer in each workspace, the language and length of answers, whether the
 * steps of a run show open, and Agent or Jobs. They are settings the person chose, not memory; only their own runs read them, and a
 * preferred model that a workspace no longer offers is not used.
 */
export function CairnPreferencesPanel({ accessToken, workspaces }: { accessToken: string; workspaces: SharedWorkspace[] }) {
  const [saved, setSaved] = useState<CairnPreferences | null>(null);
  const [draft, setDraft] = useState<CairnPreferences | null>(null);
  const [busy, setBusy] = useState(false);
  const [pickWorkspace, setPickWorkspace] = useState<string>(workspaces[0]?.workspace.id ?? "");
  const [offered, setOffered] = useState<OfferedModel[]>([]);

  useEffect(() => {
    void getCairnPreferences(accessToken).then((preferences) => { setSaved(preferences); setDraft(preferences); })
      .catch((error) => showToast("error", apiMessage(error, `Your ${ASSISTANT_NAME} preferences could not be loaded.`)));
  }, [accessToken]);

  useEffect(() => {
    if (!pickWorkspace) return;
    let live = true;
    void getSharedAgentCapabilities(accessToken, pickWorkspace).then((capabilities) => { if (live) setOffered(capabilities.models ?? []); }).catch(() => { if (live) setOffered([]); });
    return () => { live = false; };
  }, [accessToken, pickWorkspace]);

  if (!draft || !saved) return null;
  const dirty = JSON.stringify(draft) !== JSON.stringify(saved);
  const nameOf = (workspaceId: string) => workspaces.find((entry) => entry.workspace.id === workspaceId)?.workspace.name ?? "A workspace you no longer see";

  async function save() {
    if (!draft) return;
    setBusy(true);
    try {
      const next = await saveCairnPreferences(accessToken, draft);
      setSaved(next);
      setDraft(next);
      showToast("success", `Your ${ASSISTANT_NAME} preferences were saved.`);
    } catch (error) { showToast("error", apiMessage(error, "Your preferences could not be saved.")); }
    finally { setBusy(false); }
  }

  const models = Object.entries(draft.models);
  return <section className="shared-profile-panel" id="cairn-preferences">
    <PanelHeading description="Your own choices, used only in your requests" icon={SlidersHorizontal} title={`${ASSISTANT_NAME} preferences`} />
    <form onSubmit={(event) => { event.preventDefault(); void save(); }}>
      <Field hint="For example French or Português. Empty leaves it to your organization or to the language you write in." label="Answer language"><Input maxLength={40} onChange={(event) => setDraft({ ...draft, language: event.target.value })} value={draft.language ?? ""} /></Field>
      <Field label="Length of answers"><Dropdown aria-label="Length of answers" onValueChange={(value) => setDraft({ ...draft, length: value === NONE ? null : (value as CairnPreferences["length"]) })} options={LENGTHS} value={draft.length ?? NONE} /></Field>
      <Field label={`Steps under an answer ("What ${ASSISTANT_NAME} did")`}><Dropdown aria-label="Steps under an answer" onValueChange={(value) => setDraft({ ...draft, steps_open: value === NONE ? null : value === "open" })} options={STEPS} value={draft.steps_open == null ? NONE : draft.steps_open ? "open" : "closed"} /></Field>
      <Field hint="Where a workspace's model can run in both. Jobs answers by CAIRN's rules, and abilities need Agent." label="Agent or Jobs"><Dropdown aria-label="Agent or Jobs" onValueChange={(value) => setDraft({ ...draft, mode: value === NONE ? null : (value as CairnPreferences["mode"]) })} options={MODES} value={draft.mode ?? NONE} /></Field>
      <div>
        <p className="shared-eyebrow">Preferred model by workspace</p>
        {models.length ? <ul className="shared-personal-memory">{models.map(([workspaceId, modelId]) => <li key={workspaceId}>
          <span>{nameOf(workspaceId)} · {modelId}</span>
          <Button aria-label={`Remove the preferred model of ${nameOf(workspaceId)}`} onClick={() => { const rest = { ...draft.models }; delete rest[workspaceId]; setDraft({ ...draft, models: rest }); }} type="button" variant="secondary"><Trash size={ICON_SIZE.sm} /></Button>
        </li>)}</ul> : <p className="shared-muted-copy">None. A chat then uses what the workspace answers with.</p>}
        {workspaces.length ? <>
          <Field label="Workspace"><Dropdown aria-label="Workspace" onValueChange={setPickWorkspace} options={workspaces.map((entry) => ({ label: entry.workspace.name, value: entry.workspace.id }))} value={pickWorkspace} /></Field>
          <Field hint="Only the models this workspace offers you." label="Model"><Dropdown aria-label="Preferred model" disabled={offered.length === 0} onValueChange={(modelId) => setDraft({ ...draft, models: { ...draft.models, [pickWorkspace]: modelId } })} options={offered.length ? offered.map((model) => ({ label: model.name, value: model.id })) : [{ label: "No model to choose", value: "" }]} value={draft.models[pickWorkspace] ?? ""} /></Field>
        </> : null}
      </div>
      <Button busy={busy} disabled={!dirty} type="submit" variant="main">Save preferences</Button>
    </form>
  </section>;
}
