import type { Guardrails } from "../../types";
import { Field } from "../../components/ui/field";
import { Input } from "../../components/ui/input";

/** Parses a limit typed in a field: empty keeps the default. */
function limit(text: string): number | null {
  const value = Number(text);
  return text.trim() !== "" && Number.isInteger(value) && value > 0 ? value : null;
}

/**
 * How much CAIRN may change at once (V0.9.92): the most changes one confirmation may make, and the number above which the person types
 * it to confirm. A workspace can only be stricter than its organization.
 */
export function ChangeLimitFields({ disabled, guardrails, onChange, ceiling }: { disabled?: boolean; guardrails: Guardrails; onChange: (next: Guardrails) => void; ceiling?: { max_changes?: number | null; type_to_confirm_from?: number | null } }) {
  return <>
    <Field hint={`Files moved or trashed, tasks created, cards changed. Empty keeps ${ceiling?.max_changes ?? 200}, from 1 to ${ceiling?.max_changes ?? 200}.`} label="Most changes in one confirmation">
      <Input disabled={disabled} inputMode="numeric" max={ceiling?.max_changes ?? 200} min={1} onChange={(event) => onChange({ ...guardrails, max_changes: limit(event.target.value) })} type="number" value={guardrails.max_changes ?? ""} />
    </Field>
    <Field hint={ceiling?.type_to_confirm_from ? `The person types the number of changes to confirm more than this many. Your organization asks from ${ceiling.type_to_confirm_from}; choose that or less.` : "The person types the number of changes to confirm more than this many. Empty keeps only the rule for content read from outside the workspace."} label="Type the number to confirm from">
      <Input disabled={disabled} inputMode="numeric" max={ceiling?.type_to_confirm_from ?? undefined} min={1} onChange={(event) => onChange({ ...guardrails, type_to_confirm_from: limit(event.target.value) })} type="number" value={guardrails.type_to_confirm_from ?? ""} />
    </Field>
  </>;
}
