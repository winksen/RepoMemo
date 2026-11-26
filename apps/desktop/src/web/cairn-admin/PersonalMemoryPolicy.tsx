import type { Guardrails } from "../../types";
import { Checkbox } from "../../components/ui/checkbox";
import { Field } from "../../components/ui/field";
import { Input } from "../../components/ui/input";

/**
 * Personal memory (V0.9.111): whether CAIRN reads and proposes what it remembers about a person in the workspaces, and how many entries
 * it keeps for each. A workspace can only turn it off; the number is the organization's. People can export and delete their own memory
 * in their profile whatever is set here.
 */
export function PersonalMemoryPolicy({ disabled, guardrails, onChange, withCap, lockedOff }: { disabled?: boolean; guardrails: Guardrails; onChange: (next: Guardrails) => void; withCap?: boolean; lockedOff?: boolean }) {
  return <>
    <Checkbox checked={guardrails.personal_memory_enabled !== false && !lockedOff} description={lockedOff ? "Turned off by your organization" : undefined} disabled={disabled || lockedOff} label="Let CAIRN read and add to what it remembers about a person" onCheckedChange={(on) => onChange({ ...guardrails, personal_memory_enabled: on ? null : false })} />
    {withCap ? <Field hint="The most entries CAIRN keeps about one person, across the organization. Empty keeps 40." label="Entries per person">
      <Input disabled={disabled} inputMode="numeric" max={40} min={1} onChange={(event) => { const value = Number(event.target.value); onChange({ ...guardrails, personal_memory_max_entries: event.target.value.trim() !== "" && Number.isInteger(value) && value > 0 ? value : null }); }} type="number" value={guardrails.personal_memory_max_entries ?? ""} />
    </Field> : null}
  </>;
}
