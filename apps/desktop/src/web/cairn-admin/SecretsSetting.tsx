import type { Guardrails } from "../../types";
import { Dropdown } from "../../components/ui/dropdown";
import { Field } from "../../components/ui/field";
import { Checkbox } from "../../components/ui/checkbox";

const OPTIONS = [
  { label: "Replace them with a marker", value: "mask" },
  { label: "Send nothing", value: "refuse" },
];

/**
 * What happens to a secret (a private key, a provider key, a token, a password in configuration, a connection string) in text on its way
 * to a model outside the network (V0.9.94). A model on the network is not affected. A workspace can only choose the stricter.
 */
export function SecretsSetting({ disabled, guardrails, onChange, organizationMasksPersonal, organizationRefuses, organizationStrict }: { disabled?: boolean; guardrails: Guardrails; onChange: (next: Guardrails) => void; organizationMasksPersonal?: boolean; organizationRefuses?: boolean; organizationStrict?: boolean }) {
  const value = organizationRefuses ? "refuse" : (guardrails.secrets_for_cloud ?? "mask");
  return <>
    <Field hint={organizationRefuses ? "Your organization sends nothing that holds a secret to a cloud model." : "Applies to models outside your network. CAIRN counts what it removed in the run's history, never the secrets themselves."} label="A secret on its way to a cloud model">
    <Dropdown aria-label="A secret on its way to a cloud model" disabled={disabled || organizationRefuses} onValueChange={(mode) => onChange({ ...guardrails, secrets_for_cloud: mode as "mask" | "refuse" })} options={OPTIONS} value={value} />
  </Field>
    <Checkbox checked={Boolean(organizationMasksPersonal || guardrails.mask_personal_data)} description="Replaces them with a marker for models outside your network, and counts them in the run's history. Off by default." disabled={disabled || organizationMasksPersonal} label="Also mask email addresses and phone numbers" onCheckedChange={(on) => onChange({ ...guardrails, mask_personal_data: on })} />
    <Checkbox checked={Boolean(organizationStrict || guardrails.strict_citations)} description="An answer about the workspace's content that cites nothing CAIRN read is not shown; CAIRN says it could not ground it and lists what it read." disabled={disabled || organizationStrict} label="Strict citations" onCheckedChange={(on) => onChange({ ...guardrails, strict_citations: on })} />
  </>;
}
