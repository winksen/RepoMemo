import { Dropdown } from "../../components/ui/dropdown";
import { Field } from "../../components/ui/field";
import { Input } from "../../components/ui/input";

export interface PreferenceDefaultsValue {
  language?: string | null;
  length?: string | null;
  steps_open?: boolean | null;
  mode?: string | null;
}

const NONE = "none";
const LENGTHS = [{ label: "No default", value: NONE }, { label: "Brief", value: "brief" }, { label: "Standard", value: "standard" }, { label: "Detailed", value: "detailed" }];
const STEPS = [{ label: "No default (closed)", value: NONE }, { label: "Open", value: "open" }, { label: "Closed", value: "closed" }];
const MODES = [{ label: "No default", value: NONE }, { label: "Agent", value: "agent" }, { label: "Jobs", value: "jobs" }];

/** What people who have not chosen get (V0.9.115). A person's own choice in their profile always wins: these are defaults, not limits. */
export function PreferenceDefaults({ disabled, onChange, value }: { disabled?: boolean; onChange: (next: PreferenceDefaultsValue) => void; value: PreferenceDefaultsValue }) {
  return <>
    <p className="shared-eyebrow">Defaults for people who have not chosen</p>
    <Field hint="For example French. A person's own choice in their profile wins." label="Answer language"><Input disabled={disabled} maxLength={40} onChange={(event) => onChange({ ...value, language: event.target.value })} value={value.language ?? ""} /></Field>
    <Field label="Length of answers"><Dropdown aria-label="Default length of answers" disabled={disabled} onValueChange={(next) => onChange({ ...value, length: next === NONE ? null : next })} options={LENGTHS} value={value.length ?? NONE} /></Field>
    <Field label="Steps under an answer"><Dropdown aria-label="Default for the steps under an answer" disabled={disabled} onValueChange={(next) => onChange({ ...value, steps_open: next === NONE ? null : next === "open" })} options={STEPS} value={value.steps_open == null ? NONE : value.steps_open ? "open" : "closed"} /></Field>
    <Field hint="Where a workspace's model can run in both." label="Agent or Jobs"><Dropdown aria-label="Default mode" disabled={disabled} onValueChange={(next) => onChange({ ...value, mode: next === NONE ? null : next })} options={MODES} value={value.mode ?? NONE} /></Field>
  </>;
}
