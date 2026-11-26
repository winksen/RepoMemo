import type { Guardrails } from "../../types";
import { Checkbox } from "../../components/ui/checkbox";
import { Field } from "../../components/ui/field";
import { Input } from "../../components/ui/input";
import { Dropdown } from "../../components/ui/dropdown";

function parse(text: string): number | null {
  const value = Number(text);
  return text.trim() !== "" && Number.isInteger(value) && value > 0 ? value : null;
}

/** Each person's daily CAIRN budgets (V0.9.96), counted from the stored runs since midnight UTC. A workspace can only lower them. */
export function BudgetFields({ canExempt, disabled, guardrails, onChange }: { canExempt?: boolean; disabled?: boolean; guardrails: Guardrails; onChange: (next: Guardrails) => void }) {
  return <>
    <Field hint="Empty sets no limit. The composer tells a person what is left once it is below a fifth." label="Runs per person and day">
      <Input disabled={disabled} inputMode="numeric" min={1} onChange={(event) => onChange({ ...guardrails, daily_runs_per_person: parse(event.target.value) })} type="number" value={guardrails.daily_runs_per_person ?? ""} />
    </Field>
    <Field label="Tokens per person and day">
      <Input disabled={disabled} inputMode="numeric" min={1} onChange={(event) => onChange({ ...guardrails, daily_tokens_per_person: parse(event.target.value) })} type="number" value={guardrails.daily_tokens_per_person ?? ""} />
    </Field>
    {canExempt ? <>
      <Field hint="Counted from the stored runs of all the organization's workspaces, per calendar month. Its owners and admins are told at 80% and at 100%." label="Tokens per month for the organization">
        <Input disabled={disabled} inputMode="numeric" min={1} onChange={(event) => onChange({ ...guardrails, monthly_token_budget: parse(event.target.value) })} type="number" value={guardrails.monthly_token_budget ?? ""} />
      </Field>
      <Field label="At the monthly budget">
        <Dropdown aria-label="At the monthly budget" disabled={disabled} onValueChange={(mode) => onChange({ ...guardrails, at_budget: mode as "stop" | "local_only" })} options={[{ label: "CAIRN stops", value: "stop" }, { label: "Only models on our network answer", value: "local_only" }]} value={guardrails.at_budget ?? "stop"} />
      </Field>
    </> : null}
    {canExempt ? <Checkbox checked={Boolean(guardrails.budget_exempt_admins)} disabled={disabled} label="Owners and admins are not stopped by these budgets" onCheckedChange={(on) => onChange({ ...guardrails, budget_exempt_admins: on })} /> : null}
  </>;
}
