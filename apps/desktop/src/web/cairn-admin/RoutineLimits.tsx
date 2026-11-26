import type { Guardrails } from "../../types";
import { Checkbox } from "../../components/ui/checkbox";
import { Field } from "../../components/ui/field";
import { Input } from "../../components/ui/input";

const KINDS = [{ name: "daily", label: "Daily" }, { name: "weekly", label: "Weekly" }, { name: "monthly", label: "Monthly" }];

function parse(text: string): number | null {
  const value = Number(text);
  return text.trim() !== "" && Number.isInteger(value) && value > 0 ? value : null;
}

/** Limits on CAIRN's routines (V0.9.99). Routines over a new limit are paused with the reason, not deleted. A workspace can only be stricter. */
export function RoutineLimits({ disabled, guardrails, onChange }: { disabled?: boolean; guardrails: Guardrails; onChange: (next: Guardrails) => void }) {
  const allowed = guardrails.routine_schedules ?? KINDS.map((kind) => kind.name);
  return <>
    <Field hint="Empty keeps 10. Routines over a new limit are paused with the reason, not deleted." label="Routines per person in a workspace">
      <Input disabled={disabled} inputMode="numeric" max={10} min={1} onChange={(event) => onChange({ ...guardrails, routines_per_person: parse(event.target.value) })} type="number" value={guardrails.routines_per_person ?? ""} />
    </Field>
    <Field label="Routines per workspace">
      <Input disabled={disabled} inputMode="numeric" max={10} min={1} onChange={(event) => onChange({ ...guardrails, routines_per_workspace: parse(event.target.value) })} type="number" value={guardrails.routines_per_workspace ?? ""} />
    </Field>
    <div>
      <p className="shared-eyebrow">Schedules routines may have</p>
      {KINDS.map((kind) => <Checkbox checked={allowed.includes(kind.name)} disabled={disabled} key={kind.name} label={kind.label} onCheckedChange={(on) => { const next = on ? [...allowed, kind.name] : allowed.filter((name) => name !== kind.name); onChange({ ...guardrails, routine_schedules: next.length === KINDS.length ? null : next }); }} />)}
    </div>
    <Checkbox checked={guardrails.event_routines !== false} disabled={disabled} label="Routines that run on an event" onCheckedChange={(on) => onChange({ ...guardrails, event_routines: on ? null : false })} />
  </>;
}
