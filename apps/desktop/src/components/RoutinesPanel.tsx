import { useEffect, useState } from "react";
import type { AgentRoutine, RecurringSchedule } from "../generated/domain";
import { createSharedRoutine, deleteSharedRoutine, listSharedRoutines, pauseSharedRoutine, resumeSharedRoutine, runSharedRoutine } from "../lib/sharedApi";
import { ASSISTANT_NAME } from "../lib/assistantName";
import { apiMessage } from "../web/format";
import { Button } from "./ui/button";
import { Dropdown } from "./ui/dropdown";
import { Field } from "./ui/field";
import { Input } from "./ui/input";
import { Textarea } from "./ui/textarea";
import { showToast } from "./ui/toast";

const WEEKDAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

function describe(routine: AgentRoutine) {
  if (routine.event_kind === "file_outdated") return "when a file is marked outdated";
  if (routine.event_kind === "files_indexed") return routine.event_folder_id ? "when files are indexed in its folder" : "when files are indexed";
  return describeSchedule(routine.schedule);
}

function describeSchedule(schedule: RecurringSchedule) {
  if (schedule.kind === "weekly") return `every ${schedule.interval > 1 ? `${schedule.interval} weeks on ` : ""}${WEEKDAYS[schedule.weekday ?? 0]}`;
  if (schedule.kind === "monthly") return `on day ${schedule.day_of_month ?? 1} of every ${schedule.interval > 1 ? `${schedule.interval} months` : "month"}`;
  return schedule.interval > 1 ? `every ${schedule.interval} days` : "every day";
}

/** The routines of a workspace (V0.9.63): prompts CAIRN runs on a schedule as their owner, with read tools only, into a chat of their own. */
export function RoutinesPanel({ accessToken, canWrite, workspaceId }: { accessToken: string; canWrite: boolean; workspaceId: string }) {
  const [routines, setRoutines] = useState<AgentRoutine[] | null>(null);
  const [title, setTitle] = useState("");
  const [prompt, setPrompt] = useState("");
  const [weekday, setWeekday] = useState("0");
  const [when, setWhen] = useState("weekly");
  const [busy, setBusy] = useState(false);

  async function load() {
    try { setRoutines(await listSharedRoutines(accessToken, workspaceId)); }
    catch { setRoutines([]); }
  }
  useEffect(() => { void load(); }, [accessToken, workspaceId]);

  async function act(work: () => Promise<unknown>, failure: string, success?: string) {
    setBusy(true);
    try { await work(); if (success) showToast("success", success); await load(); }
    catch (error) { showToast("error", apiMessage(error, failure)); }
    finally { setBusy(false); }
  }

  function create() {
    const schedule: RecurringSchedule = { kind: "weekly", interval: 1, weekday: Number(weekday), day_of_month: null, start_date: new Date().toISOString().slice(0, 10) };
    void act(async () => { await createSharedRoutine(accessToken, workspaceId, { title, prompt, schedule, event_kind: when === "weekly" ? null : when, event_folder_id: null }); setTitle(""); setPrompt(""); }, "The routine could not be made.", "Routine made. It runs on its next date.");
  }

  return <section aria-label={`${ASSISTANT_NAME} routines`} className="shared-assistant-routines">
    <h4>Routines</h4>
    <p className="shared-muted-copy">A routine runs a request on a schedule as its owner, with read tools only, and posts the answer in a chat of its own and tells its owner.</p>
    {routines?.length ? <ul className="rm-option-list">{routines.map((routine) => <li className="rm-option-row" key={routine.id}>
      <span className="rm-option-text"><strong>{routine.title}</strong><small>{describe(routine)} · {routine.owner_name} · {routine.state === "paused" ? `paused: ${routine.pause_reason ?? ""}` : `next ${routine.next_run_date}`}</small></span>
      <Button disabled={busy || routine.state === "paused"} onClick={() => void act(() => runSharedRoutine(accessToken, workspaceId, routine.id), "The routine could not run.", "The routine ran.")} title="Only its owner can run it now" type="button" variant="secondary">Run now</Button>
      {routine.state === "paused"
        ? <Button disabled={busy} onClick={() => void act(() => resumeSharedRoutine(accessToken, workspaceId, routine.id), "The routine could not be resumed.")} type="button" variant="secondary">Resume</Button>
        : <Button disabled={busy} onClick={() => void act(() => pauseSharedRoutine(accessToken, workspaceId, routine.id), "The routine could not be paused.")} type="button" variant="secondary">Pause</Button>}
      <Button disabled={busy} onClick={() => void act(() => deleteSharedRoutine(accessToken, workspaceId, routine.id), "The routine could not be deleted.")} type="button" variant="secondary">Delete</Button>
    </li>)}</ul> : <p className="shared-muted-copy">No routine yet.</p>}
    {canWrite ? <form onSubmit={(event) => { event.preventDefault(); create(); }}>
      <Field label="Title"><Input maxLength={80} onChange={(event) => setTitle(event.target.value)} value={title} /></Field>
      <Field label="What to ask"><Textarea maxLength={2000} onChange={(event) => setPrompt(event.target.value)} rows={3} value={prompt} /></Field>
      <Field label="Run">{(control) => <Dropdown id={control.id} onValueChange={setWhen} options={[{ label: "Every week", value: "weekly" }, { label: "When files are indexed", value: "files_indexed" }, { label: "When a file is marked outdated", value: "file_outdated" }]} value={when} />}</Field>
      {when === "weekly" ? <Field label="Every week on">{(control) => <Dropdown id={control.id} onValueChange={setWeekday} options={WEEKDAYS.map((name, index) => ({ label: name, value: String(index) }))} value={weekday} />}</Field> : <p className="shared-muted-copy">At most once every 15 minutes, told which files are new.</p>}
      <Button busy={busy} disabled={!title.trim() || !prompt.trim()} type="submit" variant="main">Make the routine</Button>
    </form> : null}
  </section>;
}
