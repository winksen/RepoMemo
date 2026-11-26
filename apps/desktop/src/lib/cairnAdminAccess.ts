import type { Organization, SharedSession } from "../types";
import { CAIRN_ADMIN_SECTIONS, type CairnAdminSection } from "../web/constants";

/** What an organization's owners and admins reach: only what concerns their organizations. The server has the same list. */
const ORGANIZATION_SECTIONS: readonly CairnAdminSection[] = ["organizations", "guardrails", "abilities", "memory", "usage"];

/**
 * The sections of the CAIRN administration page a person may open, as the rail and the palette show them (V0.9.73): every one for
 * system, CAIRN and app administrators, five for owners and admins of an organization, none for everyone else. The server decides
 * again when the page opens.
 */
export function cairnAdminSections(session: Pick<SharedSession, "is_system_admin" | "is_app_admin" | "is_cairn_admin">, organizations: Pick<Organization, "role" | "guest">[]): CairnAdminSection[] {
  if (session.is_system_admin || session.is_cairn_admin || session.is_app_admin) return [...CAIRN_ADMIN_SECTIONS];
  return organizations.some((organization) => !organization.guest && (organization.role === "owner" || organization.role === "admin")) ? [...ORGANIZATION_SECTIONS] : [];
}
