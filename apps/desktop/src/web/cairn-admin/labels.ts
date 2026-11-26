import type { CairnAdminSection } from "../constants";

/** What each section of the CAIRN administration page is called. */
export const CAIRN_ADMIN_LABELS: Record<CairnAdminSection, string> = {
  overview: "Overview",
  models: "Models",
  organizations: "Organizations",
  guardrails: "Guardrails",
  abilities: "Abilities",
  memory: "Memory",
  usage: "Usage",
  history: "History",
};
