import type { Guardrails } from "../../types";
import { Checkbox } from "../../components/ui/checkbox";
import { Field } from "../../components/ui/field";
import { Input } from "../../components/ui/input";

/** What outside coding agents may do (V0.9.101): read through the MCP endpoint, and propose into the inbox. A workspace can only turn them off or lower the bound. */
export function McpPolicy({ disabled, guardrails, onChange }: { disabled?: boolean; guardrails: Guardrails; onChange: (next: Guardrails) => void }) {
  return <>
    <Checkbox checked={guardrails.mcp_enabled !== false} disabled={disabled} label="Serve outside agents through the MCP endpoint" onCheckedChange={(on) => onChange({ ...guardrails, mcp_enabled: on ? null : false })} />
    <Checkbox checked={guardrails.inbox_enabled !== false} disabled={disabled} label="Let outside agents propose into a person's inbox" onCheckedChange={(on) => onChange({ ...guardrails, inbox_enabled: on ? null : false })} />
    <Field hint="Proposals that wait at once for a person. Empty keeps 50." label="Inbox bound">
      <Input disabled={disabled} inputMode="numeric" max={50} min={1} onChange={(event) => { const value = Number(event.target.value); onChange({ ...guardrails, inbox_bound: event.target.value.trim() !== "" && Number.isInteger(value) && value > 0 ? value : null }); }} type="number" value={guardrails.inbox_bound ?? ""} />
    </Field>
  </>;
}
