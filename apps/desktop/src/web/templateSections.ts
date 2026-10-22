import { ADMIN_WORKSPACE_SECTIONS, WORKSPACE_SECTIONS, type WorkspaceSection } from "./constants";
import type { TemplateSections } from "../generated/domain";

/** The tabs of a workspace and the sections kept under the *More* menu. */
export type SectionArrangement = { tabs: WorkspaceSection[]; more: WorkspaceSection[] };

const isSection = (value: string): value is WorkspaceSection => (WORKSPACE_SECTIONS as string[]).includes(value);

/** The section a workspace opens on when its address names none: its template's, or Overview when there is none or this client has no such page. */
export function landingSection(sections: Pick<TemplateSections, "landing"> | null | undefined): WorkspaceSection {
  const landing = sections?.landing ?? "overview";
  return isSection(landing) && !ADMIN_WORKSPACE_SECTIONS.has(landing) ? landing : "overview";
}

/**
 * Arranges the sections as the workspace's template asks. The template is only an order: a section it does not
 * name still has a tab, one this client has no page for is ignored, the administrators' sections stay for
 * administrators, and the section that is open is always a tab even when the template keeps it under *More*.
 * Without a template (not loaded yet, or an older server) it is the usual twelve in the usual order.
 */
export function arrangeSections(sections: TemplateSections | null | undefined, activeSection: WorkspaceSection, isWorkspaceAdmin: boolean): SectionArrangement {
  const allowed = (section: WorkspaceSection) => isWorkspaceAdmin || !ADMIN_WORKSPACE_SECTIONS.has(section);
  let primary = (sections?.primary ?? []).filter(isSection).filter(allowed);
  let more = (sections?.more ?? []).filter(isSection).filter(allowed);
  primary = primary.filter((section, index) => primary.indexOf(section) === index && !more.includes(section));
  more = more.filter((section, index) => more.indexOf(section) === index);
  for (const section of WORKSPACE_SECTIONS) {
    if (allowed(section) && !primary.includes(section) && !more.includes(section)) primary.push(section);
  }
  if (more.includes(activeSection)) {
    more = more.filter((section) => section !== activeSection);
    primary = [...primary, activeSection];
  }
  return { tabs: primary, more };
}
