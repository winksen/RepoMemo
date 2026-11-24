import { useEffect, useState } from "react";
import { Brain, Pencil, Plus, Trash } from "@phosphor-icons/react";
import type { PersonalMemory } from "../generated/domain";
import { addPersonalMemory, deletePersonalMemory, getPersonalMemory, setPersonalMemoryEnabled, updatePersonalMemory } from "../lib/sharedApi";
import { apiMessage } from "../web/format";
import { Button } from "./ui/button";
import { Checkbox } from "./ui/checkbox";
import { Field } from "./ui/field";
import { ICON_SIZE } from "./ui/icon";
import { Input } from "./ui/input";
import { PanelHeading } from "./ui/panel-heading";
import { showToast } from "./ui/toast";
import { ASSISTANT_NAME } from "../lib/assistantName";

/** What CAIRN remembers about the person (V0.9.51): their entries to read, add, edit and delete, and a switch. It is read into their own requests only. */
export function PersonalMemoryPanel({ accessToken }: { accessToken: string }) {
  const [memory, setMemory] = useState<PersonalMemory | null>(null);
  const [draft, setDraft] = useState("");
  const [editing, setEditing] = useState<{ id: string; text: string } | null>(null);
  const [busy, setBusy] = useState(false);

  async function load() {
    try { setMemory(await getPersonalMemory(accessToken)); }
    catch (error) { showToast("error", apiMessage(error, `What ${ASSISTANT_NAME} remembers about you could not be loaded.`)); }
  }
  useEffect(() => { void load(); }, [accessToken]);

  async function act(work: () => Promise<unknown>, failure: string) {
    setBusy(true);
    try { await work(); await load(); }
    catch (error) { showToast("error", apiMessage(error, failure)); }
    finally { setBusy(false); }
  }

  return <section className="shared-profile-panel" id="cairn-memory">
    <PanelHeading description={memory ? `${memory.entries.length} of 40` : undefined} icon={Brain} title={`${ASSISTANT_NAME} memory`} />
    <p className="shared-muted-copy">What {ASSISTANT_NAME} remembers about you, such as the language you prefer. It uses this only in your own requests, never in anyone else's, and it adds something only when you confirm it in a chat.</p>
    {memory ? <>
      <Checkbox checked={memory.enabled} disabled={busy} label={`Let ${ASSISTANT_NAME} remember things about me`} onCheckedChange={(enabled) => void act(() => setPersonalMemoryEnabled(accessToken, enabled), "The setting could not be saved.")} />
      {memory.entries.length ? <ul className="shared-personal-memory">{memory.entries.map((entry) => <li key={entry.id}>
        {editing?.id === entry.id
          ? <form onSubmit={(event) => { event.preventDefault(); void act(async () => { await updatePersonalMemory(accessToken, entry.id, editing.text); setEditing(null); }, "The entry could not be saved."); }}>
              <Field label="Entry"><Input maxLength={280} onChange={(event) => setEditing({ id: entry.id, text: event.target.value })} value={editing.text} /></Field>
              <Button busy={busy} type="submit" variant="main">Save</Button> <Button disabled={busy} onClick={() => setEditing(null)} type="button" variant="secondary">Cancel</Button>
            </form>
          : <><span>{entry.text}</span>
              <Button aria-label={`Edit "${entry.text}"`} disabled={busy} onClick={() => setEditing({ id: entry.id, text: entry.text })} type="button" variant="secondary"><Pencil size={ICON_SIZE.sm} /></Button>
              <Button aria-label={`Delete "${entry.text}"`} disabled={busy} onClick={() => void act(() => deletePersonalMemory(accessToken, entry.id), "The entry could not be deleted.")} type="button" variant="secondary"><Trash size={ICON_SIZE.sm} /></Button></>}
      </li>)}</ul> : <p className="shared-muted-copy">Nothing yet. Say “remember that I prefer answers in French” to {ASSISTANT_NAME}, or add something here.</p>}
      <form onSubmit={(event) => { event.preventDefault(); if (!draft.trim()) return; void act(async () => { await addPersonalMemory(accessToken, draft); setDraft(""); }, "That could not be added."); }}>
        <Field label="Add something"><Input maxLength={280} onChange={(event) => setDraft(event.target.value)} placeholder="For example: I work on the billing project" value={draft} /></Field>
        <Button busy={busy} type="submit" variant="secondary"><Plus size={ICON_SIZE.md} /> Add</Button>
      </form>
    </> : null}
  </section>;
}
