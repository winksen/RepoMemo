import { Dropdown } from "../../components/ui/dropdown";

/** Who may use a model (V0.9.87). Owners and admins always may. */
export type RoleLimit = "all" | "members" | "admins";

const OPTIONS = [
  { label: "Everyone who may use AI", value: "all" },
  { label: "Members and up", value: "members" },
  { label: "Admins only", value: "admins" },
];

/** The limit a list of roles stands for; no entry is no limit. */
export function limitOf(roles: string[] | undefined): RoleLimit {
  if (!roles) return "all";
  return roles.includes("member") ? "members" : "admins";
}

/** The roles of a limit, as the server keeps them; `undefined` for no limit. */
export function rolesOf(limit: RoleLimit): string[] | undefined {
  if (limit === "members") return ["owner", "admin", "member"];
  if (limit === "admins") return ["owner", "admin"];
  return undefined;
}

/** The model roles with one model's limit set. */
export function withLimit(current: Record<string, string[]>, modelId: string, limit: RoleLimit): Record<string, string[]> {
  const next = { ...current };
  const roles = rolesOf(limit);
  if (roles) next[modelId] = roles;
  else delete next[modelId];
  return next;
}

/** Only the entries for models that are still chosen. */
export function limitsFor(current: Record<string, string[]>, modelIds: string[]): Record<string, string[]> {
  return Object.fromEntries(Object.entries(current).filter(([id]) => modelIds.includes(id)));
}

export function RoleLimitSelect({ disabled, label, onChange, roles }: { disabled?: boolean; label: string; onChange: (limit: RoleLimit) => void; roles: string[] | undefined }) {
  return <Dropdown aria-label={`Who may use ${label}`} disabled={disabled} onValueChange={(value) => onChange(value as RoleLimit)} options={OPTIONS} value={limitOf(roles)} />;
}
