import { useState } from "react";
import { previewCairnRetention } from "../../lib/sharedApi";
import type { Guardrails } from "../../types";
import { Button } from "../../components/ui/button";
import { Field } from "../../components/ui/field";
import { Input } from "../../components/ui/input";
import { showToast } from "../../components/ui/toast";
import { apiMessage } from "../format";

function parse(text: string): number | null {
  const value = Number(text);
  return text.trim() !== "" && Number.isInteger(value) && value > 0 ? value : null;
}

/** How long the organization keeps CAIRN's run history, chats and personal memory (V0.9.100). The shorter of this and the server's setting applies; the count is shown before anything is deleted. */
export function RetentionFields({ accessToken, disabled, guardrails, onChange, organizationId }: { accessToken: string; disabled?: boolean; guardrails: Guardrails; onChange: (next: Guardrails) => void; organizationId: string }) {
  const [counts, setCounts] = useState<string | null>(null);
  async function preview() {
    try {
      const result = await previewCairnRetention(accessToken, organizationId, guardrails);
      setCounts(`Saving would delete now: ${result.run_histories.toLocaleString()} run histories, ${result.chats.toLocaleString()} chats, ${result.personal_memories.toLocaleString()} personal memories.`);
    } catch (error) { showToast("error", apiMessage(error, "The count could not be made.")); }
  }
  const field = (label: string, key: "retention_trace_days" | "retention_chat_days" | "retention_memory_days", hint?: string) => <Field hint={hint ?? "Days. Empty keeps what the server keeps."} label={label}>
    <Input disabled={disabled} inputMode="numeric" min={1} onChange={(event) => { setCounts(null); onChange({ ...guardrails, [key]: parse(event.target.value) }); }} type="number" value={guardrails[key] ?? ""} />
  </Field>;
  return <>
    {field("Keep run histories for", "retention_trace_days", "Days. Never longer than the server's own setting.")}
    {field("Keep chats for", "retention_chat_days")}
    {field("Keep personal memory for", "retention_memory_days")}
    <div><Button disabled={disabled} onClick={() => void preview()} type="button" variant="secondary">Show what this would delete</Button>{counts ? <p className="shared-muted-copy">{counts}</p> : null}</div>
  </>;
}
