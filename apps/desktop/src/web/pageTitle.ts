import { type Route } from "./router";

const SECTION_LABEL: Record<string, string> = {
  people: "People & Activity",
  jobs: "Jobs & maintenance",
  audit: "Audit trail",
};

function sectionLabel(section: string): string {
  return SECTION_LABEL[section] ?? `${section.charAt(0).toUpperCase()}${section.slice(1)}`;
}

/** What a page is called to someone who cannot see it: the tab title and the announcement after a move. */
export function pageTitle(route: Route, names: { workspace?: string; artifact?: string; card?: string } = {}): string {
  switch (route.name) {
    case "workspaces": return "Workspaces";
    case "dashboard": return "Dashboard";
    case "profile": return "Profile";
    case "notifications": return "Notifications";
    case "system": return `System · ${sectionLabel(route.section)}`;
    case "workspace": return `${names.workspace ?? "Workspace"} · ${sectionLabel(route.section)}`;
    case "artifact": return `${names.artifact ?? "Evidence file"} · ${names.workspace ?? "Workspace"}`;
    case "memory-card": return `${names.card ?? "Memory card"} · ${names.workspace ?? "Workspace"}`;
    case "not-found": return "Page not found";
  }
}

/** The page a person is on, apart from the section inside it: focus moves when this changes, not when a tab does. */
export function pageKey(route: Route): string {
  switch (route.name) {
    case "workspace": return `workspace:${route.workspaceId}`;
    case "artifact": return `artifact:${route.artifactId}`;
    case "memory-card": return `memory-card:${route.cardId}`;
    case "not-found": return `not-found:${route.workspaceId ?? ""}`;
    default: return route.name;
  }
}
