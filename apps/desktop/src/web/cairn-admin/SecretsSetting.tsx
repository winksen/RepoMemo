import type { Guardrails } from "../../types";
import { Dropdown } from "../../components/ui/dropdown";
import { Field } from "../../components/ui/field";

const OPTIONS = [
  { label: "Replace them with a marker", value: "mask" },
  { label: "Send nothing", value: "refuse" },
];

/**
 * What happens to a secret (a private key, a provider key, a token, a password in configuration, a connection string) in text on its way
 * to a model outside the network (V0.9.94). A model on the network is not affected. A workspace can only choose the stricter.
 */
export function SecretsSetting({ disabled, guardrails, onChange, organizationRefuses }: { disabled?: boolean; guardrails: Guardrails; onChange: (next: Guardrails) => void; organizationRefuses?: boolean }) {
  const value = organizationRefuses ? "refuse" : (guardrails.secrets_for_cloud ?? "mask");
  return <Field hint={organizationRefuses ? "Your organization sends nothing that holds a secret to a cloud model." : "Applies to models outside your network. CAIRN counts what it removed in the run's history, never the secrets themselves."} label="A secret on its way to a cloud model">
    <Dropdown aria-label="A secret on its way to a cloud model" disabled={disabled || organizationRefuses} onValueChange={(mode) => onChange({ ...guardrails, secrets_for_cloud: mode as "mask" | "refuse" })} options={OPTIONS} value={value} />
  </Field>;
}
